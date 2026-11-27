//! 设置页清理：所有者候选预览、分类确认与逐项结果。
//!
//! 候选只来自日志、更新下载与陈旧快照三个所有者；当前日志、当前下载
//! 会话与布局引用的快照由所有者排除。执行在后台完成并只经审计留痕。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::DocumentId;
use slterm_workspace::ViewKey;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::cleanup::{
    CleanupCandidate, CleanupCategory, CleanupOutcome, CleanupRequest, CleanupService,
};
use crate::i18n::Message;

/// 清理候选预览上限：完整结果仍写审计。
const PREVIEW_LIMIT: usize = 12;

pub(super) struct CleanupDialog {
    caches: bool,
    audit_logs: bool,
    candidates: Vec<CleanupCandidate>,
    loaded: bool,
    busy: bool,
    error: Option<Message>,
    result: Option<(usize, usize, usize)>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn open_cleanup_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.cleanup_dialog.is_some() {
            return Ok(());
        }
        let focus_handle = cx.focus_handle();
        let registration = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self.focus_manager.open_modal(
            self.focus_manager.workspace_token(),
            self.focus_manager.workspace_token(),
            registration.token(),
            window,
            cx,
        ) {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.cleanup_dialog = Some(CleanupDialog {
            caches: true,
            audit_logs: true,
            candidates: Vec::new(),
            loaded: false,
            busy: false,
            error: None,
            result: None,
            focus_handle,
            registration,
        });
        cx.notify();
        self.start_load_cleanup_candidates(window, cx);
        Ok(())
    }

    pub(super) fn close_cleanup_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.cleanup_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn toggle_cleanup_category(
        &mut self,
        category: CleanupCategory,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.cleanup_dialog.as_mut() else {
            return;
        };
        match category {
            CleanupCategory::AuditLogs => dialog.audit_logs = !dialog.audit_logs,
            _ => dialog.caches = !dialog.caches,
        }
        dialog.result = None;
        cx.notify();
    }

    /// 布局引用的 Document 不进入陈旧快照候选。
    fn referenced_documents(&self) -> std::collections::BTreeSet<DocumentId> {
        self.snapshot
            .registry()
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .filter_map(|view| match view {
                ViewKey::Editor { document_id, .. }
                | ViewKey::Markdown { document_id, .. }
                | ViewKey::Diff { document_id, .. } => Some(document_id),
                _ => None,
            })
            .collect()
    }

    fn start_load_cleanup_candidates(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let referenced = self.referenced_documents();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::CleanupCandidates;
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let collected = executor.spawn(async move {
                    let mut candidates = Vec::new();
                    for (path, bytes) in crate::logging::cleanup_candidates() {
                        candidates.push(CleanupCandidate {
                            category: CleanupCategory::AuditLogs,
                            owner: "logging",
                            path,
                            active_generation: None,
                            bytes,
                        });
                    }
                    for (path, bytes) in crate::update_download::cleanup_candidates() {
                        candidates.push(CleanupCandidate {
                            category: CleanupCategory::FinishedUpdateDownloads,
                            owner: "update_download",
                            path,
                            active_generation: None,
                            bytes,
                        });
                    }
                    let store =
                        crate::persistence::document_snapshot::store::DocumentSnapshotStore::new(
                            crate::platform::dirs::app_paths(),
                        );
                    for (path, bytes) in
                        store.stale_candidates(&referenced, std::time::SystemTime::now())
                    {
                        candidates.push(CleanupCandidate {
                            category: CleanupCategory::StaleCrashSites,
                            owner: "document_snapshot",
                            path,
                            active_generation: None,
                            bytes,
                        });
                    }
                    Ok::<_, String>(candidates)
                });
                cx.spawn(async move |this, cx| {
                    let collected = collected.await;
                    let _ = window_handle.update(cx, |_, _, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            if let Some(dialog) = root.cleanup_dialog.as_mut() {
                                dialog.loaded = true;
                                match collected {
                                    Ok(candidates) => dialog.candidates = candidates,
                                    Err(error) => {
                                        log::warn!("清理候选收集失败: {error}");
                                        dialog.error = Some(Message::CleanupFailed);
                                    }
                                }
                            }
                            cx.notify();
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("清理候选任务未登记");
        }
    }

    /// 执行入口：后台按类别过滤并逐项执行，结果回前台汇总。
    pub(super) fn start_cleanup(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(dialog) = self.cleanup_dialog.as_ref() else {
            return;
        };
        if dialog.busy || !dialog.loaded {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let request = CleanupRequest {
            caches: dialog.caches,
            audit_logs: dialog.audit_logs,
        };
        let candidates = dialog.candidates.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::Cleanup;
        if let Some(dialog) = self.cleanup_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
            dialog.result = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let executed = executor.spawn(async move {
                    let planned = CleanupService::plan(candidates, request);
                    let report = CleanupService::execute(&planned, &|_| 0);
                    let mut removed = 0usize;
                    let mut skipped = 0usize;
                    let mut failed = 0usize;
                    for outcome in &report.outcomes {
                        match outcome {
                            CleanupOutcome::Removed { .. } => removed += 1,
                            CleanupOutcome::ExcludedActive { .. }
                            | CleanupOutcome::SkippedUnsafe { .. } => skipped += 1,
                            CleanupOutcome::Failed { .. } => failed += 1,
                        }
                    }
                    (removed, skipped, failed)
                });
                cx.spawn(async move |this, cx| {
                    let result = executed.await;
                    let _ = window_handle.update(cx, |_, _, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            if let Some(dialog) = root.cleanup_dialog.as_mut() {
                                dialog.busy = false;
                                dialog.result = Some(result);
                            }
                            cx.notify();
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("清理任务未登记");
        }
    }

    /// 清理面：分类候选、确认与结果。
    pub(super) fn render_cleanup_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.cleanup_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(language.text(Message::CleanupTitle)));
        for (category, label, checked) in [
            (
                CleanupCategory::FinishedUpdateDownloads,
                Message::CleanupCaches,
                dialog.caches,
            ),
            (
                CleanupCategory::AuditLogs,
                Message::CleanupAudit,
                dialog.audit_logs,
            ),
        ] {
            let slug = match category {
                CleanupCategory::AuditLogs => "audit",
                _ => "caches",
            };
            body = body.child(
                div()
                    .id(gpui::ElementId::Name(
                        format!("cleanup-category-{slug}").into(),
                    ))
                    .debug_selector(move || format!("cleanup-category-{slug}"))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |root, _, _, cx| {
                            root.toggle_cleanup_category(category, cx);
                        }),
                    )
                    .child(
                        div()
                            .w(px(14.0))
                            .text_xs()
                            .text_color(if checked {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(if checked { "✓" } else { "–" }),
                    )
                    .child(div().text_sm().child(language.text(label))),
            );
        }
        if !dialog.loaded {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupExporting)),
            );
        } else if dialog.candidates.is_empty() {
            body = body.child(
                div()
                    .debug_selector(|| "cleanup-empty".to_owned())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::CleanupEmpty)),
            );
        } else {
            let mut list = div()
                .id("cleanup-candidates")
                .debug_selector(|| "cleanup-candidates".to_owned())
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(220.0))
                .overflow_y_scroll()
                .text_xs()
                .text_color(cx.theme().muted_foreground);
            for candidate in dialog.candidates.iter().take(PREVIEW_LIMIT) {
                list = list.child(div().child(candidate.path.display().to_string()));
            }
            body = body.child(list);
        }
        if let Some((removed, skipped, failed)) = dialog.result {
            body = body.child(
                div()
                    .debug_selector(|| "cleanup-result".to_owned())
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.format(
                        Message::CleanupExecuted,
                        &[
                            ("removed", &removed.to_string()),
                            ("skipped", &skipped.to_string()),
                            ("failed", &failed.to_string()),
                        ],
                    )),
            );
        }
        if let Some(error) = dialog.error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(error)),
            );
        }
        let mut actions = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("cleanup-cancel")
                    .debug_selector(|| "cleanup-cancel".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .child(language.text(Message::EditorCancel))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            if let Err(error) = root.close_cleanup_dialog(window, cx) {
                                log::warn!("清理对话框取消失败: {error:?}");
                            }
                        }),
                    ),
            );
        if dialog.busy {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupExporting)),
            );
        } else {
            actions = actions.child(
                div()
                    .id("cleanup-execute")
                    .debug_selector(|| "cleanup-execute".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(Message::CleanupExecute))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            root.start_cleanup(window, cx);
                        }),
                    ),
            );
        }
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "cleanup-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_cleanup_dialog(window, cx)
                {
                    log::warn!("清理对话框关闭失败: {error:?}");
                }
            }))
            .w(px(480.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(body);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(90.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog_element),
                )
                .into_any_element(),
        )
    }
}
