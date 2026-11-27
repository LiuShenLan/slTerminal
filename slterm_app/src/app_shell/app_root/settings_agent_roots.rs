//! 设置页 AgentRoot 管理：自定义根的增删、改名、换目录与能力开关。
//!
//! 候选根先经唯一 `AgentRootRegistry` 做真实目录查重，再经统一设置写
//! 通道路盘；只有写盘成功才采纳运行值与视图同步。所有路径选择与
//! 校验都在后台执行。

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};
use slterm_settings::AgentRoot;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

/// 测试注入的自定义根保存端口；生产走统一设置写通道。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) type AgentRootsSaveHook =
    std::sync::Arc<dyn Fn(&[AgentRoot]) -> std::io::Result<()> + Send + Sync>;

/// 目录选择后的目标：新增根或替换既有根路径。
#[derive(Clone, Copy, Debug)]
enum AgentRootPick {
    Add,
    Relocate(slterm_settings::AgentRootId),
}

/// 后台提交失败分类；前台只映射为封闭文案。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentRootsCommitError {
    Duplicate,
    DuplicateBuiltin,
    Invalid,
    Save,
}

pub(super) struct AgentRootsDialog {
    busy: bool,
    error: Option<Message>,
    rename_target: Option<slterm_settings::AgentRootId>,
    rename: Entity<InputState>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn open_agent_roots_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.agent_roots_dialog.is_some() {
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
        self.agent_roots_dialog = Some(AgentRootsDialog {
            busy: false,
            error: None,
            rename_target: None,
            rename,
            focus_handle,
            registration,
        });
        self.ensure_agent_roots_loading(cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn close_agent_roots_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.agent_roots_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 新增或换目录：后台选目录，再走统一候选提交。
    pub(super) fn start_agent_root_pick(
        &mut self,
        relocate: Option<slterm_settings::AgentRootId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.agent_roots_dialog.as_ref() else {
            return;
        };
        if dialog.busy || !self.agent_roots_loaded {
            return;
        }
        let pick = relocate.map_or(AgentRootPick::Add, AgentRootPick::Relocate);
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::AgentRootsSave;
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let picked = executor.spawn(async move {
                    let Some(path) = windows
                        .pick_folder(owner, None)
                        .map_err(|error| format!("选择 Agent 根目录失败: {error}"))?
                    else {
                        return Ok::<Option<String>, String>(None);
                    };
                    Ok(Some(path.to_string_lossy().into_owned()))
                });
                cx.spawn(async move |this, cx| {
                    let result = picked.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            match result {
                                Ok(Some(path)) => {
                                    root.apply_agent_root_pick(pick, path, window, cx);
                                }
                                Ok(None) => root.finish_agent_roots_cancel(cx),
                                Err(error) => {
                                    log::warn!("Agent 根目录选择失败: {error}");
                                    root.finish_agent_roots_cancel(cx);
                                }
                            }
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.agent_roots_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn apply_agent_root_pick(
        &mut self,
        pick: AgentRootPick,
        path: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let mut roots = self.custom_agent_roots.clone();
        let mut error = None;
        match pick {
            AgentRootPick::Add => {
                let name = agent_root_default_name(&path);
                let id = slterm_settings::AgentRootId::from_bytes(*uuid::Uuid::now_v7().as_bytes())
                    .expect("UUIDv7 字节必须可构造 AgentRootId");
                match AgentRoot::custom(id, name, path) {
                    Ok(root) => roots.push(root),
                    Err(_) => error = Some(Message::SettingsAgentRootsInvalid),
                }
            }
            AgentRootPick::Relocate(id) => {
                if let Some(index) = roots.iter().position(|root| root.id() == id) {
                    match roots[index].with_path(id, path) {
                        Ok(root) => roots[index] = root,
                        Err(_) => error = Some(Message::SettingsAgentRootsInvalid),
                    }
                }
            }
        }
        if let Some(message) = error {
            if let Some(dialog) = self.agent_roots_dialog.as_mut() {
                dialog.busy = false;
                dialog.error = Some(message);
            }
            cx.notify();
            return;
        }
        self.commit_agent_roots_candidate(roots, window, cx);
    }

    fn finish_agent_roots_cancel(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.busy = false;
            dialog.error = None;
        }
        cx.notify();
    }

    pub(super) fn begin_agent_root_rename(
        &mut self,
        id: slterm_settings::AgentRootId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(root) = self
            .custom_agent_roots
            .iter()
            .find(|root| root.id() == id)
            .cloned()
        else {
            return;
        };
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.rename_target = Some(id);
            dialog.error = None;
            let name = root.display_name().to_owned();
            dialog
                .rename
                .update(cx, |state, cx| state.set_value(name, window, cx));
        }
        cx.notify();
    }

    pub(super) fn cancel_agent_root_rename(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.rename_target = None;
            dialog.error = None;
        }
        cx.notify();
    }

    pub(super) fn save_agent_root_rename(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.agent_roots_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        let Some(id) = dialog.rename_target else {
            return;
        };
        let name = dialog.rename.read(cx).value().trim().to_owned();
        if name.is_empty() {
            if let Some(dialog) = self.agent_roots_dialog.as_mut() {
                dialog.error = Some(Message::SettingsAgentRootsNameRequired);
            }
            cx.notify();
            return;
        }
        let mut roots = self.custom_agent_roots.clone();
        let Some(index) = roots.iter().position(|root| root.id() == id) else {
            return;
        };
        let renamed = AgentRoot::custom(id, name, roots[index].path().to_owned())
            .map(|root| root.with_capabilities(roots[index].capabilities()));
        match renamed {
            Ok(root) => roots[index] = root,
            Err(_) => {
                if let Some(dialog) = self.agent_roots_dialog.as_mut() {
                    dialog.error = Some(Message::SettingsAgentRootsInvalid);
                }
                cx.notify();
                return;
            }
        }
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.rename_target = None;
        }
        self.commit_agent_roots_candidate(roots, window, cx);
    }

    /// 能力开关只改授权事实；提交成功后才推进运行值。
    pub(super) fn set_agent_root_capabilities(
        &mut self,
        id: slterm_settings::AgentRootId,
        write: bool,
        terminal: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.agent_roots_dialog.as_ref()
            && dialog.busy
        {
            return;
        }
        let mut roots = self.custom_agent_roots.clone();
        let Some(index) = roots.iter().position(|root| root.id() == id) else {
            return;
        };
        let capabilities = slterm_settings::AgentRootCapabilities { write, terminal };
        roots[index] = roots[index].clone().with_capabilities(capabilities);
        self.commit_agent_roots_candidate(roots, window, cx);
    }

    pub(super) fn delete_agent_root(
        &mut self,
        id: slterm_settings::AgentRootId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.agent_roots_dialog.as_ref()
            && dialog.busy
        {
            return;
        }
        let mut roots = self.custom_agent_roots.clone();
        roots.retain(|root| root.id() != id);
        if let Some(dialog) = self.agent_roots_dialog.as_mut()
            && dialog.rename_target == Some(id)
        {
            dialog.rename_target = None;
        }
        self.commit_agent_roots_candidate(roots, window, cx);
    }

    /// 统一候选提交：后台真实目录查重 + 统一设置写通道，成功才采纳。
    fn commit_agent_roots_candidate(
        &mut self,
        roots: Vec<AgentRoot>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let known_dirs = self.agent_global_dirs.clone();
        let task = TaskId::AgentRootsSave;
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let save_override = self.agent_roots_save_for_test.clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let saved = executor.spawn(async move {
                    let dirs = known_dirs.unwrap_or_else(crate::agent_dirs::agent_global_dirs);
                    let registry = crate::agent_root_service::validate_agent_roots(&dirs, &roots)
                        .map_err(|error| match error {
                        crate::agent_root_service::AgentRootsValidationError::Duplicate => {
                            AgentRootsCommitError::Duplicate
                        }
                        crate::agent_root_service::AgentRootsValidationError::DuplicateBuiltin => {
                            AgentRootsCommitError::DuplicateBuiltin
                        }
                        crate::agent_root_service::AgentRootsValidationError::Invalid => {
                            AgentRootsCommitError::Invalid
                        }
                    })?;
                    let persisted = {
                        #[cfg(all(test, feature = "gpui-test-support"))]
                        {
                            if let Some(save) = save_override {
                                save(&roots)
                            } else {
                                crate::agent_root_service::save_agent_roots(&roots)
                            }
                        }
                        #[cfg(not(all(test, feature = "gpui-test-support")))]
                        {
                            crate::agent_root_service::save_agent_roots(&roots)
                        }
                    };
                    persisted.map_err(|error| {
                        log::warn!("Agent 根保存失败: {error}");
                        AgentRootsCommitError::Save
                    })?;
                    Ok((dirs, roots, registry))
                });
                cx.spawn(async move |this, cx| {
                    let result = saved.await;
                    let _ = window_handle.update(cx, |_, _window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_agent_roots_commit(result, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.agent_roots_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn finish_agent_roots_commit(
        &mut self,
        result: Result<
            (
                Vec<crate::agent_dirs::AgentGlobalDir>,
                Vec<AgentRoot>,
                crate::agent_root_service::AgentRootRegistry,
            ),
            AgentRootsCommitError,
        >,
        cx: &mut Context<'_, Self>,
    ) {
        match result {
            Ok((dirs, roots, registry)) => {
                let capabilities_changed =
                    agent_root_capabilities_changed(&self.custom_agent_roots, &roots);
                self.agent_global_dirs = Some(dirs);
                self.custom_agent_roots = roots;
                self.agent_root_registry = Some(registry);
                self.agent_roots_error = None;
                self.agent_roots_loaded = true;
                if capabilities_changed {
                    crate::app_shell::authorization::advance_generation();
                }
                self.sync_agent_files_roots_after_load(cx);
                self.reconcile_fs_watch();
                if let Some(dialog) = self.agent_roots_dialog.as_mut() {
                    dialog.busy = false;
                    dialog.error = None;
                }
            }
            Err(error) => {
                if let Some(dialog) = self.agent_roots_dialog.as_mut() {
                    dialog.busy = false;
                    dialog.error = Some(match error {
                        AgentRootsCommitError::Duplicate => Message::SettingsAgentRootsDuplicate,
                        AgentRootsCommitError::DuplicateBuiltin => {
                            Message::SettingsAgentRootsDuplicateBuiltin
                        }
                        AgentRootsCommitError::Invalid => Message::SettingsAgentRootsInvalid,
                        AgentRootsCommitError::Save => Message::SettingsAgentRootsSaveFailed,
                    });
                }
            }
        }
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_agent_root_rename_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.agent_roots_dialog.as_mut() {
            let text = text.to_owned();
            dialog
                .rename
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    pub(super) fn render_agent_roots_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.agent_roots_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut body = div().flex().flex_col().gap_3().child(
            div()
                .text_sm()
                .child(language.text(Message::SettingsAgentRootsTitle)),
        );
        if self.custom_agent_roots.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAgentRootsEmpty)),
            );
        }
        let mut rows = div().flex().flex_col().gap_2();
        for (index, root) in self.custom_agent_roots.iter().enumerate() {
            let id = root.id();
            let capabilities = root.capabilities();
            let row = div()
                .flex()
                .flex_col()
                .gap_1()
                .py_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .debug_selector(move || format!("agent-root-row-{index}"))
                        .child(div().text_sm().child(root.display_name().to_owned()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(root.path().to_owned()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(agent_root_toggle(
                            cx,
                            format!("agent-root-write-{index}"),
                            language.text(Message::SettingsAgentRootsWrite),
                            capabilities.write,
                            move |root, window, cx| {
                                root.set_agent_root_capabilities(
                                    id,
                                    !capabilities.write,
                                    capabilities.terminal,
                                    window,
                                    cx,
                                );
                            },
                        ))
                        .child(agent_root_toggle(
                            cx,
                            format!("agent-root-terminal-{index}"),
                            language.text(Message::SettingsAgentRootsTerminal),
                            capabilities.terminal,
                            move |root, window, cx| {
                                root.set_agent_root_capabilities(
                                    id,
                                    capabilities.write,
                                    !capabilities.terminal,
                                    window,
                                    cx,
                                );
                            },
                        ))
                        .child(agent_root_action(
                            cx,
                            format!("agent-root-rename-{index}"),
                            language.text(Message::SettingsAgentRootsRename),
                            cx.theme().muted_foreground,
                            move |root, window, cx| {
                                root.begin_agent_root_rename(id, window, cx);
                            },
                        ))
                        .child(agent_root_action(
                            cx,
                            format!("agent-root-relocate-{index}"),
                            language.text(Message::SettingsAgentRootsChangePath),
                            cx.theme().muted_foreground,
                            move |root, window, cx| {
                                root.start_agent_root_pick(Some(id), window, cx);
                            },
                        ))
                        .child(agent_root_action(
                            cx,
                            format!("agent-root-delete-{index}"),
                            language.text(Message::SettingsAgentRootsDelete),
                            cx.theme().danger,
                            move |root, window, cx| {
                                root.delete_agent_root(id, window, cx);
                            },
                        )),
                );
            rows = rows.child(row);
        }
        body = body.child(rows);
        if dialog.rename_target.is_some() {
            body = body.child(
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
                            .child(agent_root_action(
                                cx,
                                "agent-root-rename-cancel".to_owned(),
                                language.text(Message::SettingsAgentRootsRenameCancel),
                                cx.theme().muted_foreground,
                                |root, _window, cx| root.cancel_agent_root_rename(cx),
                            ))
                            .child(agent_root_action(
                                cx,
                                "agent-root-rename-save".to_owned(),
                                language.text(Message::SettingsAgentRootsRenameSave),
                                cx.theme().accent,
                                |root, window, cx| root.save_agent_root_rename(window, cx),
                            )),
                    ),
            );
        }
        if let Some(message) = dialog.error {
            body = body.child(
                div()
                    .debug_selector(|| "agent-roots-error".to_owned())
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(message)),
            );
        }
        let mut actions = div().flex().flex_row().justify_end().items_center().gap_3();
        if dialog.busy || !self.agent_roots_loaded {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAgentRootsBusy)),
            );
        } else {
            actions = actions.child(agent_root_action(
                cx,
                "agent-root-add".to_owned(),
                language.text(Message::SettingsAgentRootsAdd),
                cx.theme().accent,
                |root, window, cx| root.start_agent_root_pick(None, window, cx),
            ));
        }
        actions = actions.child(agent_root_action(
            cx,
            "agent-roots-close".to_owned(),
            language.text(Message::EditorCancel),
            cx.theme().foreground,
            |root, window, cx| {
                if let Err(error) = root.close_agent_roots_dialog(window, cx) {
                    log::warn!("Agent 根对话框关闭失败: {error:?}");
                }
            },
        ));
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "agent-roots-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_agent_roots_dialog(window, cx)
                {
                    log::warn!("Agent 根对话框关闭失败: {error:?}");
                }
            }))
            .w(px(620.0))
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

fn agent_root_toggle(
    cx: &Context<'_, AppRoot>,
    id: String,
    label: &'static str,
    enabled: bool,
    action: impl Fn(&mut AppRoot, &mut Window, &mut Context<'_, AppRoot>) + 'static,
) -> AnyElement {
    div()
        .id(gpui::ElementId::Name(id.clone().into()))
        .debug_selector(move || id)
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .text_xs()
        .text_color(if enabled {
            cx.theme().accent
        } else {
            cx.theme().muted_foreground
        })
        .child(if enabled { "✓" } else { "–" })
        .child(label)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |root, _, window, cx| {
                cx.stop_propagation();
                action(root, window, cx);
            }),
        )
        .into_any_element()
}

fn agent_root_action(
    cx: &Context<'_, AppRoot>,
    id: String,
    label: &'static str,
    color: gpui::Hsla,
    action: impl Fn(&mut AppRoot, &mut Window, &mut Context<'_, AppRoot>) + 'static,
) -> AnyElement {
    div()
        .id(gpui::ElementId::Name(id.clone().into()))
        .debug_selector(move || id)
        .px_1()
        .text_xs()
        .cursor_pointer()
        .text_color(color)
        .child(label)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |root, _, window, cx| {
                cx.stop_propagation();
                action(root, window, cx);
            }),
        )
        .into_any_element()
}

fn agent_root_default_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().take(64).collect())
        .unwrap_or_else(|| "Custom root".to_owned())
}

fn agent_root_capabilities_changed(previous: &[AgentRoot], next: &[AgentRoot]) -> bool {
    previous.iter().any(|root| {
        next.iter()
            .find(|candidate| candidate.id() == root.id())
            .is_some_and(|candidate| candidate.capabilities() != root.capabilities())
    })
}
