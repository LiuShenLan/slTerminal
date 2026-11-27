//! 设置页加密备份：导出（类别、口令、后台原子写）与恢复登记。
//!
//! 设置取运行态白名单投影，工作区经唯一写线程收集持久事实；恢复只做
//! 选择、解密预览与维护请求登记，实际替换在启动维护期执行。口令只在
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
use crate::encrypted_backup::{
    BackupArchive, BackupCategory, BackupSelection, MIN_PASSPHRASE_CHARS,
};
use crate::i18n::Message;

/// 恢复文件读取上限：与归档明文上限一致。
const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;

enum RestorePhase {
    None,
    Picking,
    Passphrase(Vec<u8>),
    Opening,
    Preview(BackupArchive),
    Staging,
    Staged,
}

pub(super) struct BackupDialog {
    selection: BackupSelection,
    /// 恢复预览中的类别选择；只允许归档实际包含的类别。
    restore_selection: BackupSelection,
    passphrase: Entity<InputState>,
    phase: RestorePhase,
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
            restore_selection: BackupSelection::from_categories([]),
            passphrase,
            phase: RestorePhase::None,
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

    /// 类别切换保持至少一个选中；恢复预览时改恢复选择集。
    pub(super) fn toggle_backup_category(
        &mut self,
        category: BackupCategory,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        let in_preview = matches!(dialog.phase, RestorePhase::Preview(_));
        if in_preview {
            let allowed = match &dialog.phase {
                RestorePhase::Preview(archive) => archive.manifest.categories.contains(&category),
                _ => false,
            };
            if !allowed {
                return;
            }
        }
        let current = if in_preview {
            dialog.restore_selection
        } else {
            dialog.selection
        };
        let mut candidate = current;
        candidate.set(category, !current.get(category));
        if candidate.is_empty() {
            return;
        }
        if in_preview {
            dialog.restore_selection = candidate;
        } else {
            dialog.selection = candidate;
        }
        dialog.error = None;
        cx.notify();
    }

    fn set_backup_error(&mut self, message: Message, cx: &mut Context<'_, Self>) {
        if let Some(dialog) = self.backup_dialog.as_mut() {
            dialog.busy = false;
            dialog.error = Some(message);
        }
        cx.notify();
    }

    /// 导出入口：任务在后台完成收集、密封、目标选择与原子写。
    pub(super) fn start_backup_export(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(dialog) = self.backup_dialog.as_ref() else {
            return;
        };
        if dialog.busy || !matches!(dialog.phase, RestorePhase::None) {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let passphrase_input = dialog.passphrase.clone();
        let selection = dialog.selection;
        let passphrase = passphrase_input.read(cx).value().to_string();
        if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            self.set_backup_error(Message::BackupPassphraseShort, cx);
            return;
        }
        let Some(writer) = self.project_writer.clone() else {
            self.set_backup_error(Message::BackupUnavailable, cx);
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

    /// 恢复第一步：后台选择并读取备份文件，口令稍后单独输入。
    pub(super) fn start_restore_pick(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(dialog) = self.backup_dialog.as_ref() else {
            return;
        };
        if dialog.busy || !matches!(dialog.phase, RestorePhase::None) {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::BackupRestore;
        if let Some(dialog) = self.backup_dialog.as_mut() {
            dialog.phase = RestorePhase::Picking;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let loaded = executor.spawn(async move {
                    let Some(path) = windows
                        .pick_open_file(owner, None)
                        .map_err(|error| format!("选择备份文件失败: {error}"))?
                    else {
                        return Ok(None);
                    };
                    let metadata = std::fs::metadata(&path)
                        .map_err(|error| format!("读取备份文件元数据失败: {error}"))?;
                    if metadata.len() > MAX_BACKUP_BYTES {
                        return Err("备份文件超过大小上限".to_owned());
                    }
                    std::fs::read(&path)
                        .map(Some)
                        .map_err(|error| format!("读取备份文件失败: {error}"))
                });
                cx.spawn(async move |this, cx| {
                    let loaded = loaded.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_restore_pick(loaded, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("备份恢复选择任务未登记");
        }
    }

    fn finish_restore_pick(
        &mut self,
        loaded: Result<Option<Vec<u8>>, String>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        match loaded {
            Ok(Some(bytes)) => {
                dialog.phase = RestorePhase::Passphrase(bytes);
                dialog.error = None;
            }
            Ok(None) => dialog.phase = RestorePhase::None,
            Err(error) => {
                log::warn!("备份文件读取失败: {error}");
                dialog.phase = RestorePhase::None;
                dialog.error = Some(Message::BackupRestoreFailed);
            }
        }
        cx.notify();
    }

    /// 恢复第二步：后台用口令解密并全量校验，成功后进入预览。
    pub(super) fn start_restore_open(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(dialog) = self.backup_dialog.as_ref() else {
            return;
        };
        let RestorePhase::Passphrase(bytes) = &dialog.phase else {
            return;
        };
        let bytes = bytes.clone();
        let passphrase = dialog.passphrase.read(cx).value().to_string();
        if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            self.set_backup_error(Message::BackupPassphraseShort, cx);
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::BackupRestore;
        if let Some(dialog) = self.backup_dialog.as_mut() {
            dialog.phase = RestorePhase::Opening;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let opened = executor
                    .spawn(async move { crate::encrypted_backup::open(&bytes, &passphrase) });
                cx.spawn(async move |this, cx| {
                    let opened = opened.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_restore_open(opened, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("备份恢复解密任务未登记");
        }
    }

    fn finish_restore_open(
        &mut self,
        opened: Result<BackupArchive, String>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        match opened {
            Ok(archive) => {
                dialog.restore_selection =
                    BackupSelection::from_categories(archive.manifest.categories.iter().copied());
                dialog.phase = RestorePhase::Preview(archive);
                dialog.error = None;
            }
            Err(error) => {
                log::warn!("备份解密或校验失败: {error}");
                dialog.phase = RestorePhase::None;
                dialog.error = Some(Message::BackupRestoreFailed);
            }
        }
        cx.notify();
    }

    /// 恢复第三步：关闭预检通过后经唯一写线程登记维护请求。
    pub(super) fn confirm_restore_stage(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.backup_dialog.as_ref() else {
            return;
        };
        let RestorePhase::Preview(archive) = &dialog.phase else {
            return;
        };
        let archive = archive.clone();
        let selection = dialog.restore_selection;
        if selection.is_empty() {
            self.set_backup_error(Message::BackupRestoreFailed, cx);
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        // 关闭预检：存在未保存正文或 Hooks 草稿时拒绝登记，先让用户处理。
        let views = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .collect::<Vec<_>>();
        let dirty_documents = self.dirty_documents_for_views(&views).unwrap_or_default();
        let dirty_hooks = self.hook_drafts.dirty_drafts().next().is_some();
        if !dirty_documents.is_empty() || dirty_hooks {
            self.set_backup_error(Message::EditorCloseTitle, cx);
            return;
        }
        let Some(writer) = self.project_writer.clone() else {
            self.set_backup_error(Message::BackupUnavailable, cx);
            return;
        };
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::BackupRestore;
        if let Some(dialog) = self.backup_dialog.as_mut() {
            dialog.phase = RestorePhase::Staging;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let staged = executor.spawn(async move {
                    writer
                        .stage_backup_restore(&archive, selection)
                        .map_err(|error| format!("登记维护请求失败: {error}"))?
                        .recv_timeout(std::time::Duration::from_secs(15))
                        .map_err(|_| "登记维护请求超时".to_owned())?
                });
                cx.spawn(async move |this, cx| {
                    let staged = staged.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_restore_stage(staged, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("备份恢复登记任务未登记");
        }
    }

    fn finish_restore_stage(
        &mut self,
        staged: Result<crate::persistence::maintenance_request::MaintenanceRequest, String>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(dialog) = self.backup_dialog.as_mut() else {
            return;
        };
        match staged {
            Ok(_request) => {
                dialog.phase = RestorePhase::Staged;
                dialog.error = None;
            }
            Err(error) => {
                log::warn!("备份恢复登记失败: {error}");
                dialog.phase = RestorePhase::None;
                dialog.error = Some(Message::BackupRestoreFailed);
            }
        }
        cx.notify();
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

    /// 备份面：类别、固定排除说明、口令，以及导出或恢复动作。
    pub(super) fn render_backup_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.backup_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let in_preview = matches!(dialog.phase, RestorePhase::Preview(_));
        let mut categories = div().flex().flex_col().gap_2();
        for (category, label) in [
            (BackupCategory::Settings, Message::BackupCategorySettings),
            (BackupCategory::Workspace, Message::BackupCategoryWorkspace),
        ] {
            let selected = if in_preview {
                dialog.restore_selection.get(category)
            } else {
                dialog.selection.get(category)
            };
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
        let busy = dialog.busy
            || matches!(
                dialog.phase,
                RestorePhase::Picking | RestorePhase::Opening | RestorePhase::Staging
            );
        match &dialog.phase {
            RestorePhase::Preview(_) => {
                body = body.child(
                    div()
                        .debug_selector(|| "backup-restore-preview".to_owned())
                        .text_xs()
                        .text_color(cx.theme().accent)
                        .child(language.text(Message::BackupRestorePreview)),
                );
            }
            RestorePhase::Staged => {
                body = body.child(
                    div()
                        .debug_selector(|| "backup-restore-staged".to_owned())
                        .text_xs()
                        .text_color(cx.theme().accent)
                        .child(language.text(Message::BackupRestoreStaged)),
                );
            }
            _ => {}
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
        } else if matches!(dialog.phase, RestorePhase::Staged) {
            actions = actions.child(
                div()
                    .id("backup-close")
                    .debug_selector(|| "backup-close".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(Message::EditorCancel))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            if let Err(error) = root.close_backup_dialog(window, cx) {
                                log::warn!("备份对话框关闭失败: {error:?}");
                            }
                        }),
                    ),
            );
        } else {
            match &dialog.phase {
                RestorePhase::None => {
                    actions = actions.child(
                        div()
                            .id("backup-restore")
                            .debug_selector(|| "backup-restore".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .text_xs()
                            .child(language.text(Message::BackupRestore))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    root.start_restore_pick(window, cx);
                                }),
                            ),
                    );
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
                RestorePhase::Passphrase(_) => {
                    actions = actions.child(
                        div()
                            .id("backup-restore-confirm")
                            .debug_selector(|| "backup-restore-confirm".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .text_xs()
                            .text_color(cx.theme().accent)
                            .child(language.text(Message::BackupRestore))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    root.start_restore_open(window, cx);
                                }),
                            ),
                    );
                }
                RestorePhase::Preview(_) => {
                    actions = actions.child(
                        div()
                            .id("backup-restore-confirm")
                            .debug_selector(|| "backup-restore-confirm".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .text_xs()
                            .text_color(cx.theme().accent)
                            .child(language.text(Message::BackupRestore))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    root.confirm_restore_stage(window, cx);
                                }),
                            ),
                    );
                }
                _ => {}
            }
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
