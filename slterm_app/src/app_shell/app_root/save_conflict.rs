//! 保存冲突决定面与另存为生产入口。
//!
//! 磁盘冲突时由唯一模态面给出覆盖、另存为、重载磁盘、保留本地或取消；
//! 动作只回到既有保存 / 外部刷新 / 原生目标选择链，不另建写盘通道。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::DocumentId;
use slterm_path::CanonicalPath;

use super::AppRoot;
use super::commands::AppRootError;
use super::documents::DocumentSaveMode;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::document_external::DocumentExternalService;
use crate::app_shell::documents::ExistingTargetPolicy;
use crate::i18n::Message;

/// 冲突决定面：只持文档身份与焦点资源，所有动作回到既有命令链。
pub(super) struct PendingSaveConflict {
    document_id: DocumentId,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    /// 打开冲突决定面；同一 Document 重复请求只还焦。
    pub(super) fn open_save_conflict(
        &mut self,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if self
            .save_conflict
            .as_ref()
            .is_some_and(|host| host.document_id == document_id)
        {
            if let Some(host) = &self.save_conflict {
                self.focus_manager
                    .request_focus(host.registration.token(), window, cx)?;
            }
            return Ok(());
        }
        self.cancel_save_conflict(window, cx)?;
        // 编码选择与保存冲突不得叠加两层模态。
        self.cancel_encoding_prompt(window, cx)?;
        // 决定面属于该 Document：收合后还焦仍停在原编辑器，而不是工作区。
        let (trigger, project) = self.save_conflict_trigger(document_id, cx);
        let focus_handle = cx.focus_handle();
        let registration = self
            .focus_manager
            .register(&focus_handle, trigger, project, true)?;
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.save_conflict = Some(PendingSaveConflict {
            document_id,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    /// 决定面的还焦落点：该 Document 的既有面板，缺失时回退工作区。
    fn save_conflict_trigger(
        &self,
        document_id: DocumentId,
        cx: &gpui::App,
    ) -> (
        super::focus_manager::FocusToken,
        Option<slterm_workspace::ProjectId>,
    ) {
        for (project_id, project) in &self.projects {
            let panel_id = project
                .read(cx)
                .panels
                .values()
                .find(|panel| {
                    matches!(
                        &panel.view_key,
                        slterm_workspace::ViewKey::Editor { document_id: id, .. }
                            | slterm_workspace::ViewKey::Markdown { document_id: id, .. }
                            if *id == document_id
                    )
                })
                .map(|panel| panel.id);
            if let Some(panel_id) = panel_id
                && let Some(registration) = self.panel_focus.get(&panel_id)
            {
                return (registration.token(), Some(*project_id));
            }
        }
        (self.focus_manager.workspace_token(), None)
    }

    /// 取消失败时决定面已收合；调用方只记录诊断，不触碰 Document。
    pub(super) fn cancel_save_conflict(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.save_conflict.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    fn take_save_conflict(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Option<DocumentId>, AppRootError> {
        let Some(host) = self.save_conflict.take() else {
            return Ok(None);
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(Some(host.document_id))
    }

    /// 覆盖：用户已确认磁盘修改可被本地正文替换。
    pub(super) fn overwrite_conflicted_document(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Ok(Some(document_id)) = self.take_save_conflict(window, cx) else {
            return;
        };
        self.save_document_with_mode(document_id, DocumentSaveMode::Overwrite, None, cx);
    }

    pub(super) fn save_as_conflicted_document(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Ok(Some(document_id)) = self.take_save_conflict(window, cx) else {
            return;
        };
        self.start_save_document_as(document_id, window, cx);
    }

    pub(super) fn reload_conflicted_document(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Ok(Some(document_id)) = self.take_save_conflict(window, cx) else {
            return;
        };
        self.start_conflict_reload(document_id, window, cx);
    }

    /// 保留本地：只解除冲突事实，不写盘；后续保存仍会重新询问。
    pub(super) fn keep_local_conflicted_document(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Ok(Some(document_id)) = self.take_save_conflict(window, cx) else {
            return;
        };
        if let Err(error) = self.documents.keep_local_after_conflict(document_id) {
            log::warn!("清除 Document 冲突标记失败: {error}");
        }
        cx.notify();
    }

    /// 另存为入口：原生目标选择只在后台执行，选中后才进入保存闸门。
    pub(super) fn start_save_document_as(
        &mut self,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let pick_task = TaskId::DocumentSaveAsPick(document_id);
        if self.tasks.is_registered(pick_task)
            || self.tasks.is_registered(TaskId::DocumentSave(document_id))
            || self
                .tasks
                .is_registered(TaskId::DocumentExternal(document_id))
        {
            log::debug!("另存为已存在在途任务");
            return;
        }
        let Some(suggested) = self.documents.session(document_id).and_then(|session| {
            (session.text().is_some() && !session.is_read_only())
                .then(|| session.location().canonical_path().as_path().to_path_buf())
        }) else {
            return;
        };
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration =
            self.tasks
                .register_with(pick_task, CancellationToken::new(), move || {
                    let picking =
                        executor.spawn(async move { windows.pick_save_target(owner, &suggested) });
                    cx.spawn(async move |this, cx| {
                        let picked = picking.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.tasks.complete(pick_task);
                                match picked {
                                    Ok(Some(target)) => match CanonicalPath::new(target) {
                                        Ok(target) => root.start_save_document_as_write(
                                            document_id,
                                            target,
                                            true,
                                            window,
                                            cx,
                                        ),
                                        Err(_) => {
                                            log::warn!("另存为目标不是有效绝对路径");
                                        }
                                    },
                                    Ok(None) => {}
                                    Err(error) => log::warn!("保存目标选择失败: {error}"),
                                }
                                cx.notify();
                            });
                        });
                    })
                });
        if registration.is_err() {
            log::debug!("另存为目标选择任务未登记");
        }
    }

    /// 已定目标的另存为：后台解析 → 前台占用复核 → 后台原子写。
    pub(super) fn start_save_document_as_write(
        &mut self,
        document_id: DocumentId,
        target: CanonicalPath,
        overwrite_confirmed: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let save_task = TaskId::DocumentSave(document_id);
        if self.tasks.is_registered(save_task)
            || self
                .tasks
                .is_registered(TaskId::DocumentExternal(document_id))
            || self.pending_document_relocations.contains_key(&document_id)
        {
            log::debug!("另存为遇到在途保存或外部刷新");
            return;
        }
        let Some(session_location) = self
            .documents
            .session(document_id)
            .map(|session| session.location().canonical_path().clone())
        else {
            return;
        };
        // 目标与当前位置相同：按普通保存处理，避免 complete_save_as 拒绝同路径。
        if target == session_location {
            self.save_document_with_mode(document_id, DocumentSaveMode::Conditional, None, cx);
            return;
        }
        let current_generation = self.current_authorization_generation();
        let request = match self.documents.begin_save(document_id, current_generation) {
            Ok(request) => request,
            Err(error) => {
                log::warn!("另存为请求未建立: {error}");
                super::events::notify_document_save_state(error, cx);
                return;
            }
        };
        let save_generation = request.save_generation();
        let attempted_revision = request.text_revision();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration =
            self.tasks
                .register_with(save_task, CancellationToken::new(), move || {
                    let resolving = executor.spawn(async move {
                        let current = crate::app_shell::authorization::current_generation();
                        let result = crate::app_shell::documents::DocumentSaveService::default()
                            .resolve_save_as_target(&request, target, current);
                        (request, result)
                    });
                    cx.spawn(async move |this, cx| {
                        let (request, resolved) = resolving.await;
                        let save_target = match resolved {
                            Ok(target) => target,
                            Err(error) => {
                                finish_save_as_failure(
                                    this,
                                    cx,
                                    document_id,
                                    save_generation,
                                    attempted_revision,
                                    error,
                                );
                                return;
                            }
                        };
                        // 前台复核 Registry 占用；写盘前服务会再次解析目标。
                        let occupied = this
                            .update(cx, |root, _| {
                                root.documents.existing(save_target.resolved()).is_some()
                            })
                            .unwrap_or(true);
                        if occupied {
                            finish_save_as_failure(
                                this,
                                cx,
                                document_id,
                                save_generation,
                                attempted_revision,
                                crate::text_document::SaveError::TargetOpen,
                            );
                            return;
                        }
                        let policy = if overwrite_confirmed {
                            ExistingTargetPolicy::OverwriteConfirmed
                        } else {
                            ExistingTargetPolicy::RequireConfirmation
                        };
                        let writing = executor.spawn(async move {
                            let current = crate::app_shell::authorization::current_generation();
                            crate::app_shell::documents::DocumentSaveService::default().save_as(
                                request,
                                save_target,
                                current,
                                policy,
                            )
                        });
                        let result = writing.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.tasks.complete(save_task);
                                match result {
                                    Ok(receipt) => {
                                        if let Err(error) = root.documents.complete_save_as(receipt)
                                        {
                                            log::warn!("另存为回执未采纳: {error}");
                                            let _ = root
                                                .documents
                                                .fail_save(document_id, save_generation);
                                            root.document_autosave.save_failed(
                                                &root.documents,
                                                document_id,
                                                attempted_revision,
                                            );
                                            super::events::notify_document_save_state(error, cx);
                                        } else {
                                            root.reconcile_saved_document(document_id);
                                            root.apply_document_location_to_views(
                                                document_id,
                                                slterm_document::ExternalApplyOutcome::Relocated,
                                                None,
                                                window,
                                                cx,
                                            )
                                            .unwrap_or_else(|error| {
                                                log::warn!("另存为后的视图位置同步失败: {error:?}");
                                            });
                                            root.reconcile_fs_watch();
                                        }
                                    }
                                    Err(error) => {
                                        let _ =
                                            root.documents.fail_save(document_id, save_generation);
                                        root.document_autosave.save_failed(
                                            &root.documents,
                                            document_id,
                                            attempted_revision,
                                        );
                                        log::warn!("另存为失败: {error}");
                                        super::events::notify_document_save_error(&error, cx);
                                    }
                                }
                                if root.pending_document_saves.remove(&document_id) {
                                    root.save_document(document_id, cx);
                                }
                                root.resume_blocked_batch_saves(cx);
                                root.resume_pending_application_close(cx);
                                cx.notify();
                            });
                        });
                    })
                });
        if registration.is_err() {
            let _ = self.documents.fail_save(document_id, save_generation);
            log::warn!("另存为写盘任务未登记");
        }
    }

    /// 冲突决定面：遮罩拦截背景命中，Escape 等同取消。
    pub(super) fn render_save_conflict(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let host = self.save_conflict.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut buttons = div().flex().flex_row().flex_wrap().justify_end().gap_2();
        for (selector, message, action) in [
            (
                "save-conflict-overwrite",
                Message::EditorSaveConflictOverwrite,
                ConflictAction::Overwrite,
            ),
            (
                "save-conflict-save-as",
                Message::EditorSaveAs,
                ConflictAction::SaveAs,
            ),
            (
                "save-conflict-reload",
                Message::EditorSaveConflictReload,
                ConflictAction::Reload,
            ),
            (
                "save-conflict-keep-local",
                Message::EditorSaveConflictKeepLocal,
                ConflictAction::KeepLocal,
            ),
            (
                "save-conflict-cancel",
                Message::EditorCancel,
                ConflictAction::Cancel,
            ),
        ] {
            let label = language.text(message);
            buttons = buttons.child(
                div()
                    .id(selector)
                    .debug_selector(move || selector.to_owned())
                    .px_3()
                    .py_1()
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |root, _, window, cx| match action {
                            ConflictAction::Overwrite => {
                                root.overwrite_conflicted_document(window, cx)
                            }
                            ConflictAction::SaveAs => root.save_as_conflicted_document(window, cx),
                            ConflictAction::Reload => root.reload_conflicted_document(window, cx),
                            ConflictAction::KeepLocal => {
                                root.keep_local_conflicted_document(window, cx)
                            }
                            ConflictAction::Cancel => {
                                if let Err(error) = root.cancel_save_conflict(window, cx) {
                                    log::warn!("保存冲突决定面取消失败: {error:?}");
                                }
                            }
                        }),
                    )
                    .child(label)
                    .into_any_element(),
            );
        }
        let dialog = div()
            .debug_selector(|| "save-conflict-dialog".to_owned())
            .track_focus(&host.focus_handle)
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.cancel_save_conflict(window, cx)
                {
                    log::warn!("保存冲突决定面取消失败: {error:?}");
                }
            }))
            .w(px(460.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .text_sm()
                    .child(language.text(Message::EditorSaveConflictTitle)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::EditorSaveConflictDetail)),
            )
            .child(buttons);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .flex()
                .items_center()
                .justify_center()
                .child(dialog)
                .into_any_element(),
        )
    }

    /// 冲突重载：后台重读真实文件，前台强制替换本地草稿并广播新正文。
    fn start_conflict_reload(
        &mut self,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let task_id = TaskId::DocumentConflictReload(document_id);
        if self.tasks.is_registered(task_id)
            || self.tasks.is_registered(TaskId::DocumentSave(document_id))
            || self
                .tasks
                .is_registered(TaskId::DocumentExternal(document_id))
        {
            log::debug!("冲突重载已存在在途任务");
            return;
        }
        let current_generation = self.current_authorization_generation();
        let Some(request) = self
            .documents
            .external_request(document_id, current_generation)
            .ok()
        else {
            return;
        };
        let target = request.location().canonical_path().clone();
        let grants = self.document_read_grants(document_id);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let reading = executor.spawn(async move {
                    let current = crate::app_shell::authorization::current_generation();
                    DocumentExternalService::default().refresh_request(request, &grants, current)
                });
                cx.spawn(async move |this, cx| {
                    let result = reading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task_id);
                            match result {
                                Ok(Some(result)) => root.apply_conflict_reload_result(
                                    document_id,
                                    target.clone(),
                                    result,
                                    window,
                                    cx,
                                ),
                                Ok(None) => {
                                    // 磁盘已回到基线；冲突事实不再成立，保留本地草稿。
                                    let _ = root.documents.keep_local_after_conflict(document_id);
                                }
                                Err(error) => log::warn!("冲突重载失败: {error}"),
                            }
                            cx.notify();
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("冲突重载任务未登记");
        }
    }
}

#[derive(Clone, Copy)]
enum ConflictAction {
    Overwrite,
    SaveAs,
    Reload,
    KeepLocal,
    Cancel,
}

/// 另存为失败：释放保存闸门、暂停自动保存并投递同一保存错误提示。
fn finish_save_as_failure(
    this: gpui::WeakEntity<AppRoot>,
    cx: &mut gpui::AsyncApp,
    document_id: DocumentId,
    save_generation: u64,
    attempted_revision: u64,
    error: crate::text_document::SaveError,
) {
    let _ = this.update(cx, |root, cx| {
        root.tasks.complete(TaskId::DocumentSave(document_id));
        if let Err(release) = root.documents.fail_save(document_id, save_generation) {
            log::warn!("另存为失败闸门未释放: {release}");
        }
        root.document_autosave
            .save_failed(&root.documents, document_id, attempted_revision);
        log::warn!("另存为解析失败: {error}");
        super::events::notify_document_save_error(&error, cx);
        cx.notify();
    });
}
