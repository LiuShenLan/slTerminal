//! 设置页加密备份导出：类别选择、口令、后台收集与原子写。
//!
//! 设置取运行态白名单投影，工作区经唯一写线程收集持久事实；口令只在
//! 本次任务内存中存在，失败与成功都不进入日志或通知正文。

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::encrypted_backup::workspace::{
    WorkspaceCapture, WorkspaceGeometryBackup, WorkspaceViewStateBackup, capture_workspace,
};
use crate::encrypted_backup::{BackupCategory, BackupSelection, MIN_PASSPHRASE_CHARS};
use crate::i18n::Message;

/// 导出任务的口令最小长度与文案同源；只在本次任务内存中存在。
pub(super) struct BackupDialog {
    selection: BackupSelection,
    passphrase: Entity<InputState>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
    busy: bool,
    error: Option<Message>,
}

impl AppRoot {
    pub(super) fn open_backup_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.backup_dialog.is_some() {
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
        let passphrase = cx.new(|cx| InputState::new(window, cx));
        self.backup_dialog = Some(BackupDialog {
            selection: BackupSelection::default(),
            passphrase,
            focus_handle,
            registration,
            busy: false,
            error: None,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_backup_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.backup_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 类别切换保持至少一个选中，避免空备份。
    pub(super) fn toggle_backup_category(
        &mut self,
        category: BackupCategory,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        let next = !dialog.selection.get(category);
        let mut candidate = dialog.selection;
        candidate.set(category, next);
        if candidate.is_empty() {
            return;
        }
        dialog.selection = candidate;
        dialog.error = None;
        cx.notify();
    }

    /// 导出入口：任务在后台完成收集、密封、目标选择与原子写。
    pub(super) fn start_backup_export(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(dialog) = self.backup_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let passphrase_input = dialog.passphrase.clone();
        let selection = dialog.selection;
        let passphrase = passphrase_input.read(cx).value().to_string();
        if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            if let Some(dialog) = self.backup_dialog.as_mut() {
                dialog.error = Some(Message::BackupPassphraseShort);
            }
            cx.notify();
            return;
        }
        let Some(writer) = self.project_writer.clone() else {
            if let Some(dialog) = self.backup_dialog.as_mut() {
                dialog.error = Some(Message::BackupUnavailable);
            }
            cx.notify();
            return;
        };
        let settings = crate::settings_transfer::export_json(
            &crate::settings_transfer::export_settings(self.settings.values()),
        );
        let machine_key = self.machine_key.clone();
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::BackupExport;
        if let Some(dialog) = self.backup_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let written = executor.spawn(async move {
                    let parts = writer
                        .collect_workspace_parts(&machine_key)
                        .map_err(|error| format!("收集工作区失败: {error}"))?
                        .recv_timeout(std::time::Duration::from_secs(15))
                        .map_err(|_| "收集工作区超时".to_owned())??;
                    let workspace = capture_workspace(WorkspaceCapture {
                        snapshot: &parts.snapshot,
                        view_states: parts
                            .view_states
                            .into_iter()
                            .map(|(view_key, state)| WorkspaceViewStateBackup { view_key, state })
                            .collect(),
                        sidebar: parts.sidebar,
                        geometry: parts
                            .window_geometry
                            .map(|geometry| WorkspaceGeometryBackup {
                                machine_key: machine_key.clone(),
                                geometry,
                            }),
                        source_machine: &machine_key,
                    })?;
                    let archive = crate::encrypted_backup::collect(
                        selection,
                        Some(settings.as_bytes()),
                        Some(&workspace),
                    )?;
                    let packet = crate::encrypted_backup::seal(&archive, &passphrase)?;
                    let Some(target) = windows
                        .pick_save_target(owner, std::path::Path::new("slterm-backup.sltmbk"))
                        .map_err(|error| format!("选择备份目标失败: {error}"))?
                    else {
                        return Ok::<bool, String>(false);
                    };
                    crate::atomic_file::write(&target, &packet)
                        .map_err(|error| format!("写入备份文件失败: {error}"))?;
                    Ok(true)
                });
                cx.spawn(async move |this, cx| {
                    let result = written.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_backup_export(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("加密备份导出任务未登记");
        }
    }

    fn finish_backup_export(
        &mut self,
        result: Result<bool, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        dialog.busy = false;
        match result {
            Ok(true) => {
                let close = self.close_backup_dialog(window, cx);
                super::events::notify_backup_exported(cx);
                if let Err(error) = close {
                    log::warn!("备份对话框关闭失败: {error:?}");
                }
            }
            Ok(false) => cx.notify(),
            Err(error) => {
                log::warn!("加密备份导出失败: {error}");
                dialog.error = Some(Message::BackupExportFailed);
                cx.notify();
            }
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn set_backup_passphrase_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.backup_dialog.as_ref() {
            dialog
                .passphrase
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    /// 备份导出面：类别、固定排除说明、口令与导出动作。
    pub(super) fn render_backup_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.backup_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut categories = div().flex().flex_col().gap_2();
        for (category, label) in [
            (BackupCategory::Settings, Message::BackupCategorySettings),
            (BackupCategory::Workspace, Message::BackupCategoryWorkspace),
        ] {
            let selected = dialog.selection.get(category);
            let slug = match category {
                BackupCategory::Settings => "settings",
                BackupCategory::Workspace => "workspace",
            };
            categories = categories.child(
                div()
                    .id(gpui::ElementId::Name(
                        format!("backup-category-{slug}").into(),
                    ))
                    .debug_selector(move || format!("backup-category-{slug}"))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |root, _, _, cx| {
                            root.toggle_backup_category(category, cx);
                        }),
                    )
                    .child(
                        div()
                            .w(px(14.0))
                            .text_xs()
                            .text_color(if selected {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(if selected { "✓" } else { "–" }),
                    )
                    .child(div().text_sm().child(language.text(label))),
            );
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(language.text(Message::BackupTitle)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupInclude)),
            )
            .child(categories)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupExcluded)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text(Message::BackupPassphrase)),
                    )
                    .child(Input::new(&dialog.passphrase))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text(Message::BackupPassphraseHint)),
                    ),
            );
        if let Some(error) = dialog.error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(error)),
            );
        }
        let busy = dialog.busy;
        let mut actions = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("backup-cancel")
                    .debug_selector(|| "backup-cancel".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .child(language.text(Message::EditorCancel))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            if let Err(error) = root.close_backup_dialog(window, cx) {
                                log::warn!("备份对话框取消失败: {error:?}");
                            }
                        }),
                    ),
            );
        if busy {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupExporting)),
            );
        } else {
            actions = actions.child(
                div()
                    .id("backup-export")
                    .debug_selector(|| "backup-export".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(Message::BackupExport))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            root.start_backup_export(window, cx);
                        }),
                    ),
            );
        }
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "backup-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_backup_dialog(window, cx)
                {
                    log::warn!("备份对话框关闭失败: {error:?}");
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
