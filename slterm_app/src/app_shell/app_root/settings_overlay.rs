//! 无项目设置浮层：AppRoot 独立持有实体、应用级 ViewKey 与焦点注册。

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, Styled as _, Subscription,
    Window, div, relative,
};
use gpui_component::ActiveTheme as _;
use slterm_workspace::{PanelGroupId, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::{FocusRegistration, FocusToken};
use super::panel_host::PanelContent;

pub(super) struct SettingsView {
    pub(super) focus_handle: gpui::FocusHandle,
}

impl SettingsView {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) const fn view_key(&self) -> ViewKey {
        ViewKey::AppSettingsOverlay
    }

    fn request_close(cx: &mut Context<'_, Self>) {
        cx.emit(SettingsViewEvent::Close);
    }
}

enum SettingsViewEvent {
    Close,
}

impl EventEmitter<SettingsViewEvent> for SettingsView {}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .key_context("AppSettingsOverlay")
            .on_key_down(cx.listener(|_view, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "escape" {
                    Self::request_close(cx);
                }
            }))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(language.text(crate::i18n::Message::SettingsTitle)),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().accent)
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_view, _event, _window, cx| Self::request_close(cx)),
                            )
                            .child(language.text(crate::i18n::Message::SettingsNavBack)),
                    ),
            )
    }
}

pub(super) struct SettingsOverlayHost {
    pub(super) view: Entity<SettingsView>,
    registration: FocusRegistration,
    return_focus: FocusToken,
    _close: Subscription,
}

impl AppRoot {
    /// 设置入口：有活跃项目时在聚焦组打开项目设置面板（同项目单例），
    /// 无项目时打开应用级浮层。重复入口只还焦既有视图。
    pub(crate) fn open_settings_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.snapshot.registry().active().is_none() {
            return self.open_settings_overlay_from_request(window, cx);
        }
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let view_key = ViewKey::Settings { project_id };
        if self.panel_for_view(project_id, &view_key, cx).is_ok() {
            return self.activate_view(view_key, window, cx);
        }
        let target_group = match self.focused_group(project_id, cx)? {
            Some(group_id) => group_id,
            None => {
                PanelGroupId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7")
            }
        };
        let candidate =
            self.snapshot
                .with_opened_view(project_id, target_group, view_key.clone())?;
        candidate.validate()?;
        self.install_panel(
            super::documents::DocumentPanelTarget {
                project_id,
                view: view_key,
                focus: true,
                activate_project: false,
            },
            window,
            cx,
            |_, cx| {
                let view = cx.new(SettingsView::new);
                let focus = view.read(cx).focus_handle.clone();
                (PanelContent::Settings(view), focus)
            },
        )?;
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn open_settings_overlay_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(host) = &self.settings_overlay {
            self.focus_manager
                .request_focus(host.registration.token(), window, cx)?;
            cx.notify();
            return Ok(());
        }
        // 项目 Settings 面板由设置阶段接入；此处不得把浮层伪装成项目面板。
        if self.snapshot.registry().active().is_some() {
            return Err(AppRootError::MissingPanel);
        }
        self.sync_focus(window, cx);
        let return_focus = self
            .focus_manager
            .observed_focus_token()
            .unwrap_or_else(|| self.focus_manager.workspace_token());
        let view = cx.new(SettingsView::new);
        let registration = self.focus_manager.register(
            &view.read(cx).focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self
            .focus_manager
            .request_focus(registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let close = cx.subscribe_in(&view, window, |root, _, event, window, cx| match event {
            SettingsViewEvent::Close => {
                if let Err(error) = root.close_settings_overlay(window, cx) {
                    log::warn!("设置浮层关闭失败: {error:?}");
                }
            }
        });
        self.settings_overlay = Some(SettingsOverlayHost {
            view,
            registration,
            return_focus,
            _close: close,
        });
        cx.notify();
        Ok(())
    }

    fn close_settings_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.settings_overlay.take() else {
            return Ok(());
        };
        if let Err(error) = self.focus_manager.restore_focus(
            &[host.return_focus, self.focus_manager.workspace_token()],
            window,
            cx,
        ) {
            self.settings_overlay = Some(host);
            return Err(error.into());
        }
        self.focus_manager.unregister(host.registration.token())?;
        cx.notify();
        Ok(())
    }

    pub(super) fn render_settings_overlay(&self, cx: &App) -> Option<AnyElement> {
        let host = self.settings_overlay.as_ref()?;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().background.opacity(0.85))
                .debug_selector(|| "app-settings-overlay-backdrop".to_owned())
                .child(
                    div()
                        .w(relative(0.8))
                        .h(relative(0.8))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .p_4()
                        .debug_selector(|| "app-settings-overlay".to_owned())
                        .child(host.view.clone()),
                )
                .into_any_element(),
        )
    }
}
