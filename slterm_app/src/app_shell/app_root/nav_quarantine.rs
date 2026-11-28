//! 导航删除入口与历史隔离区执行面。
//!
//! 删除、撤销、导出与永久删除都消费同一持久批次记录与不可变清单；文件
//! 副作用只在后台执行体发生，前台只结算逐项结果并刷新导航隐藏集合。

use std::time::SystemTime;

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use sha2::{Digest as _, Sha256};
use slterm_workspace::SideViewId;

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};
use crate::history_quarantine::export::{DEFAULT_EXPORT_LIMITS, export_batch};
use crate::history_quarantine::maintenance::{
    QuarantineActionResult, QuarantineItemOutcome, confirm_permanent_delete, permanent_delete,
    undo_record,
};
use crate::history_quarantine::persist::{
    BatchStage, QuarantineBatchRecord, QuarantineItemState, QuarantineSnapshot,
};
use crate::history_quarantine::{BatchStatus, QuarantineBatchId, UNDO_WINDOW, quarantine_batch};
use crate::i18n::Message;

/// 导航隔离区的一行批次展示；只由持久记录派生。
#[derive(Clone, Debug)]
pub(super) struct NavQuarantineBatch {
    pub(super) status: BatchStatus,
    pub(super) count: usize,
    pub(super) remaining_seconds: Option<u64>,
    pub(super) can_undo: bool,
    pub(super) can_export: bool,
    pub(super) can_delete: bool,
    pub(super) items: Vec<NavQuarantineItem>,
}

#[derive(Clone, Debug)]
pub(super) struct NavQuarantineItem {
    pub(super) order: usize,
    pub(super) state: QuarantineItemState,
}

pub(super) struct NavQuarantineDialog {
    focus_handle: FocusHandle,
    registration: super::focus_manager::FocusRegistration,
    pending_delete: Option<QuarantineBatchId>,
    report: Option<QuarantineReport>,
}

struct QuarantineReport {
    text: String,
    failed: bool,
}

struct DeleteDone {
    snapshot: QuarantineSnapshot,
    failed_orders: Vec<usize>,
}

struct UndoDone {
    snapshot: QuarantineSnapshot,
    result: QuarantineActionResult,
}

struct DeleteBatchDone {
    snapshot: QuarantineSnapshot,
    result: QuarantineActionResult,
    removed: bool,
}

impl AppRoot {
    pub(super) fn sync_nav_quarantine(&mut self, cx: &mut Context<'_, Self>) {
        let Some(view) = self.side_views.instance(SideViewId::Nav).cloned() else {
            return;
        };
        let now = SystemTime::now();
        let batches = self
            .quarantine
            .records
            .iter()
            .map(|record| batch_view(record, now))
            .collect::<Vec<_>>();
        view.update(cx, |view, cx| view.nav_set_quarantine(batches, cx));
    }

    pub(super) fn open_nav_quarantine(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if let Some(token) = self
            .quarantine_dialog
            .as_ref()
            .map(|dialog| dialog.registration.token())
        {
            self.focus_manager.request_focus(token, window, cx)?;
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
        self.quarantine_dialog = Some(NavQuarantineDialog {
            focus_handle,
            registration,
            pending_delete: None,
            report: None,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_nav_quarantine(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(dialog) = self.quarantine_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn prompt_nav_quarantine_delete(
        &mut self,
        id: QuarantineBatchId,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = &mut self.quarantine_dialog else {
            return;
        };
        dialog.pending_delete = Some(id);
        cx.notify();
    }

    pub(super) fn cancel_nav_quarantine_delete(&mut self, cx: &mut Context<'_, Self>) {
        let Some(dialog) = &mut self.quarantine_dialog else {
            return;
        };
        dialog.pending_delete = None;
        cx.notify();
    }

    /// 单条历史删除：哈希来源后立即形成独立批次；不弹确认。
    pub(super) fn handle_nav_delete_history(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        let Some(row) = self.nav_history.get(index).cloned() else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let Some(root) = self.quarantine_root.clone() else {
            return Ok(());
        };
        let task = TaskId::NavHistoryDelete;
        if self.tasks.is_registered(task) {
            return Ok(());
        }
        let source = row.source_path.clone();
        let writer = self.project_writer.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let id = QuarantineBatchId(uuid::Uuid::now_v7());
        let records = self.quarantine.records.clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let done = executor.spawn(async move {
                    let bytes = std::fs::read(&source)?;
                    let fingerprint: [u8; 32] = Sha256::digest(&bytes).into();
                    let now = SystemTime::now();
                    let outcome = quarantine_batch(&root, id, &[(source.clone(), fingerprint)])?;
                    let record = QuarantineBatchRecord::from_outcome(&outcome, now);
                    let removed = outcome
                        .material
                        .entries
                        .iter()
                        .map(|entry| entry.order)
                        .collect::<std::collections::BTreeSet<_>>();
                    for entry in &outcome.planned {
                        crate::audit::history_quarantine(
                            crate::audit::QuarantineAuditAction::Delete,
                            &entry.source,
                            removed.contains(&entry.order),
                        );
                    }
                    persist_quarantine_record(writer.as_ref(), &record);
                    let mut records = records;
                    replace_record(&mut records, record);
                    let snapshot = QuarantineSnapshot::from_records(&root, records);
                    Ok::<_, std::io::Error>(DeleteDone {
                        snapshot,
                        failed_orders: outcome
                            .failures
                            .iter()
                            .map(|failure| failure.entry_index)
                            .collect(),
                    })
                });
                cx.spawn(async move |this, cx| {
                    let result = done.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_nav_delete_history(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("导航历史删除任务未登记");
        }
        Ok(())
    }

    fn finish_nav_delete_history(
        &mut self,
        result: Result<DeleteDone, std::io::Error>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::NavHistoryDelete);
        match result {
            Ok(done) => {
                self.quarantine = done.snapshot;
                self.nav_history
                    .retain(|row| !self.quarantine.hidden_sources.contains(&row.source_path));
                self.sync_nav_history(cx);
                if !done.failed_orders.is_empty() {
                    if let Err(error) = self.open_nav_quarantine(window, cx) {
                        log::debug!("隔离区打开失败: {error}");
                    }
                    self.set_quarantine_report(failed_report(
                        cx,
                        Message::NavQuarantineUndoFailed,
                        &done.failed_orders,
                    ));
                }
            }
            Err(error) => {
                log::warn!("导航历史删除失败: {error:?}");
                super::events::notify_project_relocation_failed(cx);
            }
        }
        cx.notify();
    }

    /// 撤销入口：失败保留待处理事实，可再次点击重试。
    pub(super) fn handle_nav_quarantine_undo(
        &mut self,
        id: QuarantineBatchId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.ensure_not_closing().is_err() || self.action_in_flight() {
            return Ok(());
        }
        let Some(root) = self.quarantine_root.clone() else {
            return Ok(());
        };
        let Some(record) = self
            .records()
            .iter()
            .find(|record| record.id == id)
            .cloned()
        else {
            return Ok(());
        };
        let records = self.quarantine.records.clone();
        let writer = self.project_writer.clone();
        let executor = cx.background_executor().clone();
        let work = executor.spawn(async move {
            let result = undo_record(&root, &record, SystemTime::now());
            persist_quarantine_record(writer.as_ref(), &result.record);
            let mut records = records;
            replace_record(&mut records, result.record.clone());
            let snapshot = QuarantineSnapshot::from_records(&root, records);
            Some(UndoDone { snapshot, result })
        });
        self.spawn_nav_quarantine_task(window, cx, work);
        Ok(())
    }

    /// 导出入口：后台选择目标并写 ZIP；导出不清除任何隔离材料。
    pub(super) fn handle_nav_quarantine_export(
        &mut self,
        id: QuarantineBatchId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.action_in_flight() {
            return Ok(());
        }
        let Some(root) = self.quarantine_root.clone() else {
            return Ok(());
        };
        let Some(record) = self
            .records()
            .iter()
            .find(|record| record.id == id)
            .cloned()
        else {
            return Ok(());
        };
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::NavQuarantineAction;
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let work = executor.spawn(async move {
                    let target = windows
                        .pick_save_target(owner, std::path::Path::new("slterm-quarantine.zip"));
                    match target {
                        Ok(Some(destination)) => {
                            export_batch(&root, &record, &destination, DEFAULT_EXPORT_LIMITS)
                        }
                        Ok(None) => Ok(Default::default()),
                        Err(_) => Err(std::io::Error::other("export picker failed")),
                    }
                });
                cx.spawn(async move |this, cx| {
                    let result = work.await;
                    let _ = window_handle.update(cx, |_, _, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_nav_quarantine_export(result, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("隔离导出任务未登记");
        }
        Ok(())
    }

    fn finish_nav_quarantine_export(
        &mut self,
        result: Result<crate::history_quarantine::export::QuarantineExportReport, std::io::Error>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::NavQuarantineAction);
        let report = match result {
            Ok(report) if report.entries > 0 => QuarantineReport {
                text: cx
                    .global::<crate::app_shell::config::Settings>()
                    .language
                    .text(Message::NavQuarantineExported)
                    .to_owned(),
                failed: false,
            },
            Ok(_) => QuarantineReport {
                text: String::new(),
                failed: false,
            },
            Err(error) => {
                log::warn!("隔离导出失败: {error:?}");
                QuarantineReport {
                    text: cx
                        .global::<crate::app_shell::config::Settings>()
                        .language
                        .text(Message::NavQuarantineExportFailed)
                        .to_owned(),
                    failed: true,
                }
            }
        };
        self.set_quarantine_report(report);
        cx.notify();
    }

    /// 永久删除确认后的唯一执行入口。
    pub(super) fn handle_nav_quarantine_confirm_delete(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(id) = self
            .quarantine_dialog
            .as_mut()
            .and_then(|dialog| dialog.pending_delete.take())
        else {
            return Ok(());
        };
        if self.action_in_flight() {
            return Ok(());
        }
        let Some(root) = self.quarantine_root.clone() else {
            return Ok(());
        };
        if let Some(dialog) = &mut self.quarantine_dialog {
            dialog.report = None;
        }
        let Some(record) = self
            .records()
            .iter()
            .find(|record| record.id == id)
            .cloned()
        else {
            return Ok(());
        };
        let records = self.quarantine.records.clone();
        let writer = self.project_writer.clone();
        let executor = cx.background_executor().clone();
        let work = executor.spawn(async move {
            let now = SystemTime::now();
            let Ok(confirmation) = confirm_permanent_delete(&record, now) else {
                let result = QuarantineActionResult {
                    record,
                    items: Vec::new(),
                    applied: false,
                    complete: false,
                };
                let snapshot = QuarantineSnapshot::from_records(&root, records);
                return Some(DeleteBatchDone {
                    snapshot,
                    result,
                    removed: false,
                });
            };
            let result = permanent_delete(&root, &record, confirmation, now);
            let removed = if result.complete {
                remove_persisted_batch(writer.as_ref(), record.id)
            } else {
                persist_quarantine_record(writer.as_ref(), &result.record);
                false
            };
            let mut records = records;
            if removed {
                records.retain(|record| record.id != result.record.id);
            } else {
                replace_record(&mut records, result.record.clone());
            }
            let snapshot = QuarantineSnapshot::from_records(&root, records);
            Some(DeleteBatchDone {
                snapshot,
                result,
                removed,
            })
        });
        self.spawn_nav_quarantine_task(window, cx, work);
        Ok(())
    }

    fn action_in_flight(&self) -> bool {
        self.tasks.is_registered(TaskId::NavQuarantineAction)
    }

    fn spawn_nav_quarantine_task<Outcome, Work>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        work: Work,
    ) where
        Outcome: NavQuarantineOutcome,
        Work: std::future::Future<Output = Option<Outcome>> + 'static,
    {
        let window_handle = window.window_handle();
        let registration = self.tasks.register_with(
            TaskId::NavQuarantineAction,
            CancellationToken::new(),
            move || {
                cx.spawn(async move |this, cx| {
                    let result = work.await;
                    let _ = window_handle.update(cx, |_, _, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_nav_quarantine_action(result, cx);
                        });
                    });
                })
            },
        );
        if registration.is_err() {
            log::debug!("隔离区动作任务未登记");
        }
    }

    fn finish_nav_quarantine_action<Outcome: NavQuarantineOutcome>(
        &mut self,
        result: Option<Outcome>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::NavQuarantineAction);
        let Some(result) = result else {
            return;
        };
        self.quarantine = result.snapshot();
        result.apply(self, cx);
        self.sync_nav_history(cx);
        cx.notify();
    }

    fn records(&self) -> &[QuarantineBatchRecord] {
        &self.quarantine.records
    }

    fn set_quarantine_report(&mut self, report: QuarantineReport) {
        if let Some(dialog) = &mut self.quarantine_dialog {
            dialog.report = Some(report);
        }
    }

    /// 隔离区模态：批次、剩余期限、撤销/导出/永久删除与逐项失败。
    pub(super) fn render_nav_quarantine_dialog(
        &self,
        cx: &Context<'_, Self>,
    ) -> Option<AnyElement> {
        let dialog = self.quarantine_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let now = SystemTime::now();
        let mut list = div()
            .id("nav-quarantine-list")
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(420.0))
            .overflow_y_scroll();
        for (index, record) in self.quarantine.records.iter().enumerate() {
            let view = batch_view(record, now);
            let status = match view.status {
                BatchStatus::Completed => Message::NavQuarantineStatusCompleted,
                BatchStatus::NeedsAttention => Message::NavQuarantineStatusAttention,
                BatchStatus::Undone => Message::NavQuarantineStatusUndone,
                BatchStatus::Prepared => Message::NavQuarantineStatusAttention,
            };
            let mut line = div()
                .id(("nav-quarantine-batch", index))
                .debug_selector(move || format!("nav-quarantine-batch-{index}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .border_1()
                .border_color(cx.theme().border)
                .text_sm()
                .child(format!(
                    "{} · {} · {}",
                    &record.id.0.to_string()[..8],
                    language.format(
                        Message::NavQuarantineBatch,
                        &[("count", &view.count.to_string())],
                    ),
                    language.text(status),
                ));
            if let Some(seconds) = view.remaining_seconds {
                line = line.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.format(
                            Message::NavQuarantineRemaining,
                            &[("seconds", &seconds.to_string())],
                        )),
                );
            }
            let mut actions = div().flex().flex_row().gap_2();
            if view.can_undo {
                let label = if view.status == BatchStatus::NeedsAttention {
                    Message::NavQuarantineRetry
                } else {
                    Message::NavQuarantineUndo
                };
                let id = record.id;
                actions = actions.child(
                    div()
                        .id(("nav-quarantine-undo", index))
                        .debug_selector(move || format!("nav-quarantine-undo-{index}"))
                        .cursor_pointer()
                        .px_2()
                        .child(language.text(label))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |root, _: &gpui::MouseDownEvent, window, cx| {
                                if let Err(error) = root.handle_nav_quarantine_undo(id, window, cx)
                                {
                                    log::debug!("隔离撤销未开始: {error}");
                                }
                            }),
                        ),
                );
            }
            if view.can_export {
                let id = record.id;
                actions = actions.child(
                    div()
                        .id(("nav-quarantine-export", index))
                        .debug_selector(move || format!("nav-quarantine-export-{index}"))
                        .cursor_pointer()
                        .px_2()
                        .child(language.text(Message::NavQuarantineExport))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |root, _: &gpui::MouseDownEvent, window, cx| {
                                if let Err(error) =
                                    root.handle_nav_quarantine_export(id, window, cx)
                                {
                                    log::debug!("隔离导出未开始: {error}");
                                }
                            }),
                        ),
                );
            }
            if view.can_delete {
                let id = record.id;
                actions = actions.child(
                    div()
                        .id(("nav-quarantine-delete", index))
                        .debug_selector(move || format!("nav-quarantine-delete-{index}"))
                        .cursor_pointer()
                        .px_2()
                        .child(language.text(Message::NavQuarantineDelete))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |root, _: &gpui::MouseDownEvent, _, cx| {
                                root.prompt_nav_quarantine_delete(id, cx);
                            }),
                        ),
                );
            }
            line = line.child(actions);
            if dialog.pending_delete == Some(record.id) {
                line = line.child(
                    div()
                        .id("nav-quarantine-confirm")
                        .debug_selector(|| "nav-quarantine-confirm".to_owned())
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(language.text(Message::NavQuarantineDeleteTitle))
                        .child(language.format(
                            Message::NavQuarantineDeleteDetail,
                            &[("count", &view.count.to_string())],
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .id("nav-quarantine-delete-cancel")
                                        .debug_selector(|| {
                                            "nav-quarantine-delete-cancel".to_owned()
                                        })
                                        .cursor_pointer()
                                        .px_2()
                                        .child(language.text(Message::NavQuarantineCancel))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|root, _: &gpui::MouseDownEvent, _, cx| {
                                                root.cancel_nav_quarantine_delete(cx);
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("nav-quarantine-delete-confirm")
                                        .debug_selector(|| {
                                            "nav-quarantine-delete-confirm".to_owned()
                                        })
                                        .cursor_pointer()
                                        .px_2()
                                        .child(language.text(Message::NavQuarantineDeleteConfirm))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(
                                                |root, _: &gpui::MouseDownEvent, window, cx| {
                                                    if let Err(error) = root
                                                        .handle_nav_quarantine_confirm_delete(
                                                            window, cx,
                                                        )
                                                    {
                                                        log::debug!("隔离永久删除未开始: {error}");
                                                    }
                                                },
                                            ),
                                        ),
                                ),
                        ),
                );
            }
            for item in view
                .items
                .iter()
                .filter(|item| item.state == QuarantineItemState::Failed)
            {
                line = line.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.format(
                            Message::NavQuarantineItemFailed,
                            &[("order", &(item.order + 1).to_string())],
                        )),
                );
            }
            list = list.child(line);
        }
        let report = dialog.report.as_ref().map(|report| {
            div()
                .id("nav-quarantine-result")
                .debug_selector(|| "nav-quarantine-result".to_owned())
                .text_xs()
                .text_color(if report.failed {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(report.text.clone())
        });
        let panel = div()
            .debug_selector(|| "nav-quarantine-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_nav_quarantine(window, cx)
                {
                    log::debug!("隔离区关闭失败: {error:?}");
                }
            }))
            .w(px(560.0))
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
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_grow(1.0)
                            .child(language.text(Message::NavQuarantineTitle)),
                    )
                    .child(
                        div()
                            .id("nav-quarantine-close")
                            .debug_selector(|| "nav-quarantine-close".to_owned())
                            .cursor_pointer()
                            .px_1()
                            .child("×")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _: &gpui::MouseDownEvent, window, cx| {
                                    if let Err(error) = root.close_nav_quarantine(window, cx) {
                                        log::debug!("隔离区关闭失败: {error:?}");
                                    }
                                }),
                            ),
                    ),
            )
            .child(list)
            .children(report);
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
                        .child(panel),
                )
                .into_any_element(),
        )
    }
}

fn batch_view(record: &QuarantineBatchRecord, now: SystemTime) -> NavQuarantineBatch {
    let remaining_seconds = record.completed_at.and_then(|completed_at| {
        match now.duration_since(completed_at) {
            Ok(elapsed) if elapsed < UNDO_WINDOW => Some((UNDO_WINDOW - elapsed).as_secs()),
            Ok(_) => None,
            // 时钟回拨与状态机一致：仍视为在绝对窗口内。
            Err(_) => Some(UNDO_WINDOW.as_secs()),
        }
    });
    let can_undo = match record.status {
        BatchStatus::NeedsAttention => record.completed_at.is_some(),
        BatchStatus::Completed => remaining_seconds.is_some(),
        BatchStatus::Prepared | BatchStatus::Undone => false,
    };
    NavQuarantineBatch {
        status: record.status,
        count: record.items.len(),
        remaining_seconds,
        can_undo,
        can_export: record.stage != BatchStage::Prepared || !record.items.is_empty(),
        can_delete: confirm_permanent_delete(record, now).is_ok(),
        items: record
            .items
            .iter()
            .map(|item| NavQuarantineItem {
                order: item.order,
                state: item.state,
            })
            .collect(),
    }
}

fn replace_record(records: &mut Vec<QuarantineBatchRecord>, record: QuarantineBatchRecord) {
    if let Some(existing) = records.iter_mut().find(|existing| existing.id == record.id) {
        *existing = record;
    } else {
        records.push(record);
    }
}

trait NavQuarantineOutcome: Send + 'static {
    fn snapshot(&self) -> QuarantineSnapshot;
    fn apply(self, root: &mut AppRoot, cx: &mut Context<'_, AppRoot>);
}

impl NavQuarantineOutcome for UndoDone {
    fn snapshot(&self) -> QuarantineSnapshot {
        self.snapshot.clone()
    }

    fn apply(self, root: &mut AppRoot, cx: &mut Context<'_, AppRoot>) {
        let restored = self
            .result
            .items
            .iter()
            .filter(|item| item.outcome == QuarantineItemOutcome::Restored)
            .count();
        let failed = self
            .result
            .items
            .iter()
            .filter_map(|item| {
                matches!(item.outcome, QuarantineItemOutcome::Failed(_)).then_some(item.order)
            })
            .collect::<Vec<_>>();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let report = if self.result.applied {
            QuarantineReport {
                text: language.format(
                    Message::NavQuarantineRestored,
                    &[("count", &restored.to_string())],
                ),
                failed: false,
            }
        } else {
            failed_report(cx, Message::NavQuarantineUndoFailed, &failed)
        };
        root.set_quarantine_report(report);
        root.refresh_nav_history(cx);
    }
}

impl NavQuarantineOutcome for DeleteBatchDone {
    fn snapshot(&self) -> QuarantineSnapshot {
        self.snapshot.clone()
    }

    fn apply(self, root: &mut AppRoot, cx: &mut Context<'_, AppRoot>) {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let report = if self.removed {
            QuarantineReport {
                text: language.format(
                    Message::NavQuarantineDeleted,
                    &[("count", &self.result.items.len().to_string())],
                ),
                failed: false,
            }
        } else {
            let failed = self
                .result
                .items
                .iter()
                .filter_map(|item| {
                    matches!(item.outcome, QuarantineItemOutcome::Failed(_)).then_some(item.order)
                })
                .collect::<Vec<_>>();
            failed_report(cx, Message::NavQuarantineDeleteFailed, &failed)
        };
        root.set_quarantine_report(report);
    }
}

fn failed_report(
    cx: &Context<'_, AppRoot>,
    message: Message,
    failed_orders: &[usize],
) -> QuarantineReport {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let mut text = language.text(message).to_owned();
    if !failed_orders.is_empty() {
        let orders = failed_orders
            .iter()
            .map(|order| (order + 1).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        text.push(' ');
        text.push_str(&language.format(
            Message::NavQuarantineFailedItems,
            &[("orders", orders.as_str())],
        ));
    }
    QuarantineReport { text, failed: true }
}

fn persist_quarantine_record(
    writer: Option<&crate::persistence::workspace_writer::WorkspaceWriterClient>,
    record: &QuarantineBatchRecord,
) {
    let Some(writer) = writer else {
        return;
    };
    let Ok(response) = writer.upsert_quarantine_batch(record) else {
        log::warn!("隔离批次状态未提交");
        return;
    };
    if !matches!(response.recv(), Ok(Ok(()))) {
        log::warn!("隔离批次状态提交失败");
    }
}

fn remove_persisted_batch(
    writer: Option<&crate::persistence::workspace_writer::WorkspaceWriterClient>,
    id: QuarantineBatchId,
) -> bool {
    let Some(writer) = writer else {
        return true;
    };
    let Ok(response) = writer.remove_quarantine_batch(id) else {
        log::warn!("隔离批次记录未移除");
        return false;
    };
    matches!(response.recv(), Ok(Ok(true)))
}

#[cfg(test)]
mod nav_quarantine_tests {
    use super::*;
    use crate::history_quarantine::persist::{BatchStage, QuarantineItemRecord};

    fn record(status: BatchStatus, completed_at: Option<SystemTime>) -> QuarantineBatchRecord {
        QuarantineBatchRecord {
            id: QuarantineBatchId(
                uuid::Uuid::parse_str("019bfd70-80aa-7000-8000-000000000001").unwrap(),
            ),
            status,
            stage: BatchStage::Committed,
            created_at: SystemTime::UNIX_EPOCH,
            completed_at,
            manifest_name: "019bfd70-80aa-7000-8000-000000000001.manifest.json".into(),
            items: vec![QuarantineItemRecord {
                order: 0,
                state: QuarantineItemState::SourceRemoved,
                failure: None,
            }],
        }
    }

    #[test]
    fn view_exposes_undo_only_inside_window_and_delete_after_expiry() {
        let completed = record(BatchStatus::Completed, Some(SystemTime::UNIX_EPOCH));
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
        let view = batch_view(&completed, now);
        assert!(view.can_undo);
        assert!(!view.can_delete);

        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(6);
        let view = batch_view(&completed, now);
        assert!(!view.can_undo);
        assert!(view.can_delete);

        let pending = record(BatchStatus::NeedsAttention, None);
        let view = batch_view(&pending, now);
        assert!(!view.can_undo, "缺完成时间不得伪装可撤销");
        assert!(view.can_delete);
    }
}
