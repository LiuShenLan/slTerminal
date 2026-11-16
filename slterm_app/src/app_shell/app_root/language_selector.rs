//! Editor 语言选择器：使用 AppRoot 唯一的模态焦点与语法共享链。

use gpui::{
    AnyElement, Context, DismissEvent, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Point, Subscription, Window, anchored, deferred, div,
    px,
};
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use slterm_document::LanguageId;
use slterm_workspace::{ProjectId, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::{FocusError, FocusRegistration};
use crate::app_shell::text_view::DocumentTextView;
use crate::i18n::Message;

/// 当前语言菜单只持展示与焦点资源；覆盖事实仍归对应 Editor ViewState。
pub(super) struct LanguageSelectorMenu {
    pub(super) view_key: ViewKey,
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    registration: FocusRegistration,
    _dismiss: Subscription,
}

impl AppRoot {
    pub(super) fn open_language_selector(
        &mut self,
        project_id: ProjectId,
        view_key: ViewKey,
        view: Entity<DocumentTextView>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        if self.language_selector_menu.is_some() {
            self.close_language_selector_menu(window, cx)?;
        }
        self.sync_focus(window, cx);
        let panel_id = self.panel_for_view(project_id, &view_key, cx)?;
        let trigger = self
            .panel_focus
            .get(&panel_id)
            .map(FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)?;
        let selected = view.read(cx).language_override();
        let automatic_label = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(Message::EditorLanguageAuto)
            .to_owned();
        let root = cx.weak_entity();
        let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
            menu = menu.scrollable(true);
            for language_override in
                std::iter::once(None).chain(LanguageId::ALL.into_iter().map(Some))
            {
                let label = language_override
                    .map(|language| language.as_str().to_owned())
                    .unwrap_or_else(|| automatic_label.clone());
                let root = root.clone();
                let view = view.clone();
                let view_key = view_key.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .checked(selected == language_override)
                        .on_click(move |_, window, cx| {
                            let _ = root.update(cx, |root, cx| {
                                if let Err(error) = root.select_language_override(
                                    project_id,
                                    view_key.clone(),
                                    view.clone(),
                                    language_override,
                                    window,
                                    cx,
                                ) {
                                    log::warn!("选择 Editor 语言失败: {error}");
                                }
                            });
                        }),
                );
            }
            menu
        });
        let registration =
            self.focus_manager
                .register(&menu.focus_handle(cx), trigger, Some(project_id), true)?;
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let root = cx.weak_entity();
        let dismiss = window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let _ = root.update(cx, |root, cx| {
                if let Err(error) = root.close_language_selector_menu(window, cx) {
                    log::warn!("语言选择器收合失败: {error:?}");
                }
            });
        });
        self.language_selector_menu = Some(LanguageSelectorMenu {
            view_key,
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    fn select_language_override(
        &mut self,
        project_id: ProjectId,
        view_key: ViewKey,
        view: Entity<DocumentTextView>,
        language_override: Option<LanguageId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.close_language_selector_menu(window, cx)?;
        self.panel_for_view(project_id, &view_key, cx)?;
        let ViewKey::Editor { document_id, .. } = view_key else {
            return Err(AppRootError::MissingPanel);
        };
        let language = view.read(cx).effective_language(language_override);
        let syntax = self.document_syntax(document_id, language)?;
        view.update(cx, |view, cx| {
            view.apply_language_override(language_override, syntax, cx);
        });
        Ok(())
    }

    pub(super) fn close_language_selector_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), FocusError> {
        let Some(state) = self.language_selector_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn render_language_selector_menu(&self) -> Option<AnyElement> {
        let state = self.language_selector_menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.0))
                    .position(state.position)
                    .child(
                        div()
                            .debug_selector(|| "app-root-language-selector-menu".to_owned())
                            .child(state.menu.clone()),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
