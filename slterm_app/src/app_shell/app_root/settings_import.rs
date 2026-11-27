//! 普通设置导入：后台选择与解析、预览确认、写盘成功后一次采纳。
//!
//! 预检只接受当前版本、已登记且可导出的字段；写盘沿用设置存储的
//! 版本保护、`.bak` 备份与原子替换，失败不改变运行值。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_settings::json::Value;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

/// 导入文件上限：与设置文件单份持久上限一致。
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

type ImportUpdates = Vec<(&'static str, Value)>;

/// 已通过预检、等待用户确认的导入批次。
pub(super) struct PendingSettingsImport {
    updates: ImportUpdates,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn start_import_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let task = TaskId::SettingsImport;
        if self.tasks.is_registered(task) || self.settings_import.is_some() {
            return;
        }
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let loaded = executor.spawn(async move {
                    let Some(path) = windows.pick_open_file(owner, None)? else {
                        return Ok(None);
                    };
                    let metadata = std::fs::metadata(&path)?;
                    if metadata.len() > MAX_IMPORT_BYTES {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::FileTooLarge,
                            "导入文件超过大小上限",
                        ));
                    }
                    let bytes = std::fs::read(&path)?;
                    let export = crate::settings_transfer::parse_export(&bytes)
                        .map_err(|_| std::io::Error::other("导入文件结构非法"))?;
                    let updates = crate::settings_transfer::import_settings(&export)
                        .map_err(|_| std::io::Error::other("导入字段校验失败"))?;
                    Ok::<_, std::io::Error>(Some(updates))
                });
                cx.spawn(async move |this, cx| {
                    let loaded = loaded.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_settings_import_pick(loaded, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("设置导入任务未登记");
        }
    }

    fn finish_settings_import_pick(
        &mut self,
        loaded: std::io::Result<Option<ImportUpdates>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let updates = match loaded {
            Ok(Some(updates)) => updates,
            Ok(None) => return,
            Err(error) => {
                log::warn!("导入设置预检失败: {error}");
                super::events::notify_settings_import_failed(cx);
                return;
            }
        };
        if updates.is_empty() {
            return;
        }
        if let Err(error) = self.open_settings_import_preview(updates, window, cx) {
            log::warn!("导入预览打开失败: {error:?}");
            super::events::notify_settings_import_failed(cx);
        }
    }

    fn open_settings_import_preview(
        &mut self,
        updates: ImportUpdates,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
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
        self.settings_import = Some(PendingSettingsImport {
            updates,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn cancel_settings_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(preview) = self.settings_import.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(preview.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 确认后先写盘；写盘成功才把同一批更新采纳到内存并热应用。
    pub(super) fn confirm_settings_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(preview) = self.settings_import.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(preview.registration.token());
        cx.notify();
        close?;
        unregister?;
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        let updates = preview.updates;
        let adopt = updates.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::SettingsImport;
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let written = executor.spawn(async move {
                    let pairs: Vec<(&str, Value)> = updates
                        .iter()
                        .map(|(key, value)| (*key, value.clone()))
                        .collect();
                    crate::settings_store::save_settings_blocking(&pairs)
                });
                cx.spawn(async move |this, cx| {
                    let result = written.await;
                    let _ = window_handle.update(cx, |_, _window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            match result {
                                Ok(()) => {
                                    root.settings.adopt_imported(&adopt);
                                    root.reload_settings_snapshot(cx);
                                    root.bump_settings_revision();
                                    root.notify_settings_views(cx);
                                    super::events::notify_settings_imported(cx);
                                    cx.notify();
                                }
                                Err(error) => {
                                    log::warn!("导入设置写盘失败: {error}");
                                    super::events::notify_settings_import_failed(cx);
                                }
                            }
                        });
                    });
                })
            });
        if registration.is_err() {
            // 注册失败时前台已无可用回执通道；运行值与磁盘均未改变。
            log::warn!("设置导入写盘任务未登记");
        }
        Ok(())
    }

    /// 导入预览面：确认前不写盘、不改运行值。
    pub(super) fn render_settings_import(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let preview = self.settings_import.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let count = preview.updates.len();
        let detail = language.format(
            Message::SettingsImportPreviewDetail,
            &[("count", &count.to_string())],
        );
        let mut keys = div()
            .id("settings-import-keys")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(260.0))
            .overflow_y_scroll()
            .text_xs()
            .text_color(cx.theme().muted_foreground);
        for (key, _) in &preview.updates {
            keys = keys.child(div().child((*key).to_owned()));
        }
        let dialog = div()
            .debug_selector(|| "settings-import-preview".to_owned())
            .track_focus(&preview.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.cancel_settings_import(window, cx)
                {
                    log::warn!("导入预览取消失败: {error:?}");
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
                    .child(language.text(Message::SettingsImportPreviewTitle)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
            .child(keys)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("settings-import-cancel")
                            .debug_selector(|| "settings-import-cancel".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .child(language.text(Message::EditorCancel))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    if let Err(error) = root.cancel_settings_import(window, cx) {
                                        log::warn!("导入预览取消失败: {error:?}");
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-import-confirm")
                            .debug_selector(|| "settings-import-confirm".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .child(language.text(Message::SettingsImportConfirm))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    if let Err(error) = root.confirm_settings_import(window, cx) {
                                        log::warn!("导入设置确认失败: {error:?}");
                                    }
                                }),
                            ),
                    ),
            );
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(100.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog),
                )
                .into_any_element(),
        )
    }
}
