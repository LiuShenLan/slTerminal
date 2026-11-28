//! 设置页主题库管理：内置与自定义主题的选择、导入、重命名与删除。
//!
//! 列表来自唯一 `ThemeLibraryStore`；导入、改名与删除都在后台执行，
//! 前台只持有行快照与封闭文案。当前主题删除前必须先另选，导入失败零写入。

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};
use slterm_settings::json::Value;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

/// 主题库行快照；builtin 行用显示名作身份，自定义行用 UUIDv7 文本。
#[derive(Clone, Debug)]
pub(super) struct ThemeLibraryRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) builtin: bool,
    pub(super) revision: u64,
    pub(super) active: bool,
}

/// 测试行视图：避免把元组裸露在用例里。
#[cfg(all(test, feature = "gpui-test-support"))]
#[derive(Clone, Debug)]
pub(crate) struct ThemeLibraryRowView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) builtin: bool,
    pub(crate) revision: u64,
    pub(crate) active: bool,
}

pub(super) struct ThemeLibraryDialog {
    rows: Vec<ThemeLibraryRow>,
    loading: bool,
    busy: bool,
    error: Option<Message>,
    /// 正在改名的自定义主题（id、原修订）；None = 未进入改名。
    rename_target: Option<(String, u64)>,
    /// 已请求删除、等待二次确认的主题身份。
    delete_confirm: Option<String>,
    rename: Entity<InputState>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn open_theme_library_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.theme_library_dialog.is_some() {
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
        let rename = cx.new(|cx| InputState::new(window, cx));
        self.theme_library_dialog = Some(ThemeLibraryDialog {
            rows: Vec::new(),
            loading: true,
            busy: false,
            error: None,
            rename_target: None,
            delete_confirm: None,
            rename,
            focus_handle,
            registration,
        });
        cx.notify();
        self.start_theme_library_load(window, cx);
        Ok(())
    }

    pub(super) fn close_theme_library_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.theme_library_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 当前主题身份：自定义主题 id 优先，否则内置主题显示名。
    fn active_theme_identity(&self) -> Option<String> {
        let runtime = self.settings.runtime();
        runtime
            .custom_theme
            .clone()
            .or_else(|| Some(runtime.theme.prompt_name().to_owned()))
    }

    fn start_theme_library_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let store = self.theme_store();
        let active = self.active_theme_identity();
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.loading = true;
            dialog.error = None;
            // 行快照即将重读，旧删除确认不得跨代次沿用
            dialog.delete_confirm = None;
        }
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::ThemeLibraryList;
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let listed = executor.spawn(async move {
                    store
                        .list()
                        .map(|snapshot| {
                            let mut rows = Vec::new();
                            for document in &snapshot.builtins {
                                let name = document.name().to_owned();
                                rows.push(ThemeLibraryRow {
                                    active: active.as_deref() == Some(name.as_str()),
                                    id: name.clone(),
                                    name,
                                    builtin: true,
                                    revision: 0,
                                });
                            }
                            for document in &snapshot.custom {
                                let Some(id) = document.theme_id() else {
                                    continue;
                                };
                                let id = id.to_string();
                                rows.push(ThemeLibraryRow {
                                    active: active.as_deref() == Some(id.as_str()),
                                    id,
                                    name: document.name().to_owned(),
                                    builtin: false,
                                    revision: document.revision(),
                                });
                            }
                            rows
                        })
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = listed.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_theme_library_load(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("主题库列表任务未登记");
        }
    }

    fn finish_theme_library_load(
        &mut self,
        result: Result<Vec<ThemeLibraryRow>, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let active = self.active_theme_identity();
        let Some(dialog) = self.theme_library_dialog.as_mut() else {
            return;
        };
        if dialog.loading {
            dialog.loading = false;
            match result {
                Ok(rows) => {
                    dialog.rows = rows
                        .into_iter()
                        .map(|mut row| {
                            row.active = active.as_deref() == Some(row.id.as_str());
                            row
                        })
                        .collect();
                }
                Err(error) => {
                    log::warn!("主题库读取失败: {error}");
                    dialog.error = Some(Message::SettingsAppearanceThemeLibraryLoadFailed);
                }
            }
        }
        let _ = window;
        cx.notify();
    }

    /// 选择内置或自定义主题；两条写入都经唯一设置字段通道。
    pub(super) fn select_theme_library_theme(
        &mut self,
        id: String,
        builtin: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let key = if builtin { "theme" } else { "custom_theme" };
        self.apply_settings_field(key, Value::String(id), cx);
        let active = self.active_theme_identity();
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.error = None;
            dialog.rename_target = None;
            // 选择变化即离开原删除确认语境
            dialog.delete_confirm = None;
            for row in &mut dialog.rows {
                row.active = active.as_deref() == Some(row.id.as_str());
            }
        }
        cx.notify();
    }

    /// 进入改名：只允许自定义主题，输入框预填当前名称。
    pub(super) fn begin_theme_library_rename(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.theme_library_dialog.as_mut() else {
            return;
        };
        let Some(row) = dialog
            .rows
            .iter()
            .find(|row| !row.builtin && row.id == id)
            .cloned()
        else {
            return;
        };
        dialog.rename_target = Some((row.id, row.revision));
        dialog.error = None;
        // 改名与删除互斥，进入改名即取消未完成的删除确认
        dialog.delete_confirm = None;
        let name = row.name;
        dialog
            .rename
            .update(cx, |state, cx| state.set_value(name, window, cx));
        cx.notify();
    }

    pub(super) fn cancel_theme_library_rename(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.rename_target = None;
            dialog.error = None;
            cx.notify();
        }
    }

    /// 提交改名：后台按修订前置条件写入，冲突与过期都零写入。
    pub(super) fn save_theme_library_rename(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.theme_library_dialog.as_ref() else {
            return;
        };
        if dialog.busy || dialog.loading {
            return;
        }
        let Some((id, revision)) = dialog.rename_target.clone() else {
            return;
        };
        let name = dialog.rename.read(cx).value().to_string();
        let name = name.trim().to_owned();
        if name.is_empty() {
            if let Some(dialog) = self.theme_library_dialog.as_mut() {
                dialog.error = Some(Message::SettingsAppearanceThemeLibraryNameRequired);
            }
            cx.notify();
            return;
        }
        let store = self.theme_store();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::ThemeLibraryRename;
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let rename_id = id.clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let renamed = executor.spawn(async move {
                    store
                        .rename(&rename_id, &name, revision)
                        .map(|document| document.theme_id().map(|id| id.to_string()))
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = renamed.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_theme_library_rename(id, result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.theme_library_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn finish_theme_library_rename(
        &mut self,
        id: String,
        result: Result<Option<String>, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let active = self.active_theme_identity();
        let succeeded = result.is_ok();
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = false;
            match result {
                Ok(_) => dialog.rename_target = None,
                Err(error) => {
                    log::warn!("主题重命名失败: {error}");
                    dialog.error = Some(Message::SettingsAppearanceThemeLibraryRenameFailed);
                }
            }
        }
        if succeeded && active.as_deref() == Some(id.as_str()) {
            self.reload_theme_definition(id, cx);
        }
        if succeeded {
            self.start_theme_library_load(window, cx);
        }
        cx.notify();
    }

    /// 删除前先判定当前主题：必须另选后才允许删除。
    pub(super) fn delete_theme_library_entry(
        &mut self,
        id: String,
        revision: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.theme_library_dialog.as_ref()
            && (dialog.busy || dialog.loading)
        {
            return;
        }
        if self.active_theme_identity().as_deref() == Some(id.as_str()) {
            if let Some(dialog) = self.theme_library_dialog.as_mut() {
                dialog.error = Some(Message::SettingsAppearanceThemeLibraryDeleteActive);
            }
            cx.notify();
            return;
        }
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            if dialog.delete_confirm.as_deref() != Some(id.as_str()) {
                dialog.delete_confirm = Some(id);
                dialog.error = None;
                cx.notify();
                return;
            }
            dialog.delete_confirm = None;
            dialog.error = None;
        }
        let store = self.theme_store();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::ThemeLibraryDelete;
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let delete_id = id.clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let deleted = executor.spawn(async move {
                    store
                        .delete(&delete_id, revision)
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = deleted.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_theme_library_delete(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.theme_library_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn finish_theme_library_delete(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = false;
            if let Err(error) = &result {
                log::warn!("主题删除失败: {error}");
                dialog.error = Some(Message::SettingsAppearanceThemeLibraryDeleteFailed);
            }
        }
        if result.is_ok() {
            self.start_theme_library_load(window, cx);
        }
        cx.notify();
    }

    /// 导入单文件 JSON：后台选择、完整校验后入库，成功即选中。
    pub(super) fn start_theme_library_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.theme_library_dialog.as_ref()
            && (dialog.busy || dialog.loading)
        {
            return;
        }
        let store = self.theme_store();
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::ThemeLibraryImport;
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let imported = executor.spawn(async move {
                    let Some(path) = windows
                        .pick_open_file(owner, None)
                        .map_err(|error| format!("选择主题文件失败: {error}"))?
                    else {
                        return Ok(None);
                    };
                    store
                        .import_json_file(&path)
                        .map(|document| document.theme_id().map(|id| id.to_string()))
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = imported.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_theme_library_import(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.theme_library_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn finish_theme_library_import(
        &mut self,
        result: Result<Option<String>, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            dialog.busy = false;
        }
        match result {
            Ok(Some(id)) => {
                self.apply_settings_field("custom_theme", Value::String(id), cx);
                self.start_theme_library_load(window, cx);
            }
            Ok(None) => {}
            Err(error) => {
                log::warn!("主题导入失败: {error}");
                if let Some(dialog) = self.theme_library_dialog.as_mut() {
                    dialog.error = Some(Message::SettingsAppearanceThemeLibraryImportFailed);
                }
            }
        }
        cx.notify();
    }

    /// 测试读数：行快照与改名输入注入。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn theme_library_rows_for_test(&self) -> Option<Vec<ThemeLibraryRowView>> {
        self.theme_library_dialog.as_ref().map(|dialog| {
            dialog
                .rows
                .iter()
                .map(|row| ThemeLibraryRowView {
                    id: row.id.clone(),
                    name: row.name.clone(),
                    builtin: row.builtin,
                    revision: row.revision,
                    active: row.active,
                })
                .collect()
        })
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn theme_library_loading_for_test(&self) -> Option<bool> {
        self.theme_library_dialog
            .as_ref()
            .map(|dialog| dialog.loading || dialog.busy)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn theme_library_delete_confirm_for_test(&self) -> Option<String> {
        self.theme_library_dialog
            .as_ref()
            .and_then(|dialog| dialog.delete_confirm.clone())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_theme_library_rename_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.theme_library_dialog.as_mut() {
            let text = text.to_owned();
            dialog
                .rename
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    pub(super) fn render_theme_library_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.theme_library_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let active_name = dialog
            .rows
            .iter()
            .find(|row| row.active)
            .map(|row| row.name.clone());
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .child(language.text(Message::SettingsAppearanceThemeLibrary)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAppearanceThemeLibraryCurrent))
                    .child(active_name.unwrap_or_else(|| "–".to_owned())),
            );
        if dialog.loading || dialog.rows.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(if dialog.loading {
                        Message::SettingsAppearanceThemeLibraryBusy
                    } else {
                        Message::SettingsAppearanceThemeLibraryEmpty
                    })),
            );
        }
        let mut rows = div().flex().flex_col().gap_1();
        for (index, row) in dialog.rows.iter().enumerate() {
            let select_id = row.id.clone();
            let builtin = row.builtin;
            let accent = if row.active {
                cx.theme().accent
            } else {
                cx.theme().foreground
            };
            let mut row_element = div()
                .id(gpui::ElementId::Name(
                    format!("theme-library-row-{index}").into(),
                ))
                .debug_selector(move || format!("theme-library-row-{index}"))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_1()
                .py_1()
                .cursor_pointer()
                .text_color(accent)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |root, _, _, cx| {
                        root.select_theme_library_theme(select_id.clone(), builtin, cx);
                    }),
                )
                .child(
                    div()
                        .w(px(14.0))
                        .text_xs()
                        .child(if row.active { "✓" } else { "–" }),
                )
                .child(div().text_sm().child(row.name.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(if row.builtin {
                            Message::SettingsAppearanceThemeLibraryBuiltin
                        } else {
                            Message::SettingsAppearanceThemeLibraryCustom
                        })),
                );
            if !row.builtin {
                let rename_id = row.id.clone();
                row_element = row_element.child(
                    div()
                        .id(gpui::ElementId::Name(
                            format!("theme-library-rename-{index}").into(),
                        ))
                        .debug_selector(move || format!("theme-library-rename-{index}"))
                        .px_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAppearanceThemeLibraryRename))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |root, _, window, cx| {
                                cx.stop_propagation();
                                root.begin_theme_library_rename(rename_id.clone(), window, cx);
                            }),
                        ),
                );
                let delete_id = row.id.clone();
                let revision = row.revision;
                row_element = row_element.child(
                    div()
                        .id(gpui::ElementId::Name(
                            format!("theme-library-delete-{index}").into(),
                        ))
                        .debug_selector(move || format!("theme-library-delete-{index}"))
                        .px_1()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(language.text(
                            if dialog.delete_confirm.as_deref() == Some(delete_id.as_str()) {
                                Message::SettingsAppearanceThemeLibraryDeleteAgain
                            } else {
                                Message::SettingsAppearanceThemeLibraryDelete
                            },
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |root, _, window, cx| {
                                cx.stop_propagation();
                                root.delete_theme_library_entry(
                                    delete_id.clone(),
                                    revision,
                                    window,
                                    cx,
                                );
                            }),
                        ),
                );
            }
            rows = rows.child(row_element);
        }
        body = body.child(rows);
        if dialog.rename_target.is_some() {
            body =
                body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Input::new(&dialog.rename))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap_3()
                                .child(
                                    div()
                                        .id("theme-library-rename-cancel")
                                        .debug_selector(|| "theme-library-rename-cancel".to_owned())
                                        .cursor_pointer()
                                        .px_2()
                                        .text_xs()
                                        .child(language.text(
                                            Message::SettingsAppearanceThemeLibraryRenameCancel,
                                        ))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|root, _, _, cx| {
                                                root.cancel_theme_library_rename(cx);
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("theme-library-rename-save")
                                        .debug_selector(|| "theme-library-rename-save".to_owned())
                                        .cursor_pointer()
                                        .px_2()
                                        .text_xs()
                                        .text_color(cx.theme().accent)
                                        .child(language.text(
                                            Message::SettingsAppearanceThemeLibraryRenameSave,
                                        ))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|root, _, window, cx| {
                                                root.save_theme_library_rename(window, cx);
                                            }),
                                        ),
                                ),
                        ),
                );
        }
        if let Some(message) = dialog.error {
            body = body.child(
                div()
                    .debug_selector(|| "theme-library-error".to_owned())
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(message)),
            );
        }
        let mut actions = div().flex().flex_row().justify_end().items_center().gap_3();
        if dialog.busy {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAppearanceThemeLibraryBusy)),
            );
        } else {
            actions = actions.child(
                div()
                    .id("theme-library-import")
                    .debug_selector(|| "theme-library-import".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(Message::SettingsAppearanceThemeImportAction))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            root.start_theme_library_import(window, cx);
                        }),
                    ),
            );
        }
        actions = actions.child(
            div()
                .id("theme-library-close")
                .debug_selector(|| "theme-library-close".to_owned())
                .cursor_pointer()
                .px_2()
                .text_xs()
                .child(language.text(Message::EditorCancel))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|root, _, window, cx| {
                        if let Err(error) = root.close_theme_library_dialog(window, cx) {
                            log::warn!("主题库对话框关闭失败: {error:?}");
                        }
                    }),
                ),
        );
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "theme-library-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_theme_library_dialog(window, cx)
                {
                    log::warn!("主题库对话框关闭失败: {error:?}");
                }
            }))
            .w(px(520.0))
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
