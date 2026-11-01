//! 生产侧栏：活动栏与双槽只读取 SideViewHost 的组织状态和实体。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, DismissEvent, Focusable as _, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, StyleRefinement, Styled as _, WeakEntity, Window, anchored,
    deferred, div, point, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName,
    menu::{PopupMenu, PopupMenuItem},
};
use slterm_sidebar::{ButtonMovement, SideSlot, SidebarError, SidebarState, SidebarTransition};
use slterm_workspace::SideViewId;

use crate::i18n::{Message, UiLanguage};

use super::{
    ActivateSideButton, AppRoot, FocusFirstSideButton, FocusLastSideButton, FocusNextSideButton,
    FocusPreviousSideButton, OpenSideButtonMenu, SideButtonMenu,
};

pub(super) const ACTIVITY_BAR_WIDTH: f32 = 46.0;
pub(super) const ACTIVITY_BAR_KEY_CONTEXT: &str = "ActivityBar";

/// 活动栏键盘焦点使用主题强调色，与打开态底色区分。
pub(super) fn side_button_focus_style(accent: Hsla) -> StyleRefinement {
    StyleRefinement::default().border_1().border_color(accent)
}

enum SideButtonNavigation {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy)]
enum SideButtonCommand {
    Reorder(ButtonMovement),
    MoveToOtherSlot,
    Hide,
    ResetOrder,
}

impl AppRoot {
    /// 动作布局校验与渲染共用实际侧栏占宽；隐藏按钮不等于隐藏整个侧栏。
    pub(super) fn visible_sidebar_width(&self) -> f32 {
        let state = self.side_views.state();
        let has_open_view = SideSlot::ALL
            .into_iter()
            .any(|slot| state.open_view(slot).is_some());
        if state.is_visible() && has_open_view {
            f32::from(state.width())
        } else {
            0.0
        }
    }

    fn focused_side_view(&self, window: &Window, cx: &Context<'_, Self>) -> Option<SideViewId> {
        SideViewId::ALL.into_iter().find(|view| {
            self.side_views
                .instance(*view)
                .and_then(|instance| instance.read(cx).focus_handle.as_ref())
                .is_some_and(|focus| focus.contains_focused(window, cx))
        })
    }

    fn focused_side_button(&self, window: &Window, cx: &Context<'_, Self>) -> Option<SideViewId> {
        let focused = window.focused(cx)?;
        SideViewId::ALL.into_iter().find(|view| {
            self.side_buttons
                .get(view)
                .is_some_and(|button| button.handle == focused)
        })
    }

    fn focus_side_button(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let token = self.side_buttons[&view].registration.token();
        if let Err(error) = self.focus_manager.request_focus(token, window, cx) {
            log::warn!("活动栏按钮送焦失败: {error:?}");
        }
    }

    pub(super) fn focus_activity_bar(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let focused_view = self.focused_side_view(window, cx);
        self.sync_focus(window, cx);
        let state = self.side_views.state();
        let target = focused_view
            .filter(|view| state.button_is_visible(*view))
            .or_else(|| {
                state
                    .visible_buttons()
                    .into_iter()
                    .find(|view| state.open_slot(*view).is_some())
            })
            .or_else(|| state.visible_buttons().first().copied());
        if let Some(view) = target {
            self.focus_side_button(view, window, cx);
        }
    }

    fn navigate_side_buttons(
        &mut self,
        navigation: SideButtonNavigation,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let current = self.focused_side_button(window, cx);
        let buttons = self.side_views.state().visible_buttons();
        if buttons.is_empty() {
            return;
        }
        let current =
            current.and_then(|view| buttons.iter().position(|candidate| *candidate == view));
        let last = buttons.len() - 1;
        let index = match navigation {
            SideButtonNavigation::Next => current.map_or(0, |index| (index + 1).min(last)),
            SideButtonNavigation::Previous => current.map_or(0, |index| index.saturating_sub(1)),
            SideButtonNavigation::First => 0,
            SideButtonNavigation::Last => last,
        };
        self.focus_side_button(buttons[index], window, cx);
    }

    fn activate_focused_side_button(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(view) = self.focused_side_button(window, cx) else {
            return;
        };
        if let Err(error) = self.click_side_view(view, window, cx) {
            log::warn!("活动栏按钮键盘激活失败: {error}");
        }
    }

    fn open_focused_side_button_menu(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(view) = self.focused_side_button(window, cx) else {
            return;
        };
        self.open_side_button_menu(view, None, window, cx);
    }

    fn side_button_menu_item(
        root: &WeakEntity<Self>,
        language: UiLanguage,
        message: Message,
        view: SideViewId,
        command: SideButtonCommand,
    ) -> PopupMenuItem {
        let root = root.clone();
        PopupMenuItem::new(language.text(message)).on_click(move |_, window, cx| {
            let _ = root.update(cx, |this, cx| {
                this.organize_side_button(view, command, window, cx);
            });
        })
    }

    fn build_side_button_menu(
        mut menu: PopupMenu,
        view: SideViewId,
        target_slot: SideSlot,
        root: WeakEntity<Self>,
        language: UiLanguage,
    ) -> PopupMenu {
        let move_message = match target_slot {
            SideSlot::Upper => Message::SidebarButtonMoveUpper,
            SideSlot::Lower => Message::SidebarButtonMoveLower,
        };
        for (message, command) in [
            (
                Message::SidebarButtonMoveUp,
                SideButtonCommand::Reorder(ButtonMovement::Up),
            ),
            (
                Message::SidebarButtonMoveDown,
                SideButtonCommand::Reorder(ButtonMovement::Down),
            ),
            (
                Message::SidebarButtonMoveTop,
                SideButtonCommand::Reorder(ButtonMovement::Top),
            ),
            (
                Message::SidebarButtonMoveBottom,
                SideButtonCommand::Reorder(ButtonMovement::Bottom),
            ),
            (move_message, SideButtonCommand::MoveToOtherSlot),
            (Message::SidebarButtonHide, SideButtonCommand::Hide),
        ] {
            menu = menu.item(Self::side_button_menu_item(
                &root, language, message, view, command,
            ));
        }
        menu.separator().item(Self::side_button_menu_item(
            &root,
            language,
            Message::SidebarButtonResetOrder,
            view,
            SideButtonCommand::ResetOrder,
        ))
    }

    /// 键盘与右键共用同一个菜单实体和收合订阅。
    fn open_side_button_menu(
        &mut self,
        view: SideViewId,
        position: Option<gpui::Point<gpui::Pixels>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.side_button_menu.is_some() {
            self.close_side_button_menu(window, cx);
        }
        self.focus_side_button(view, window, cx);
        let button = &self.side_buttons[&view];
        let trigger = button.registration.token();
        let target_slot = match self.side_views.state().button_slot(view) {
            SideSlot::Upper => SideSlot::Lower,
            SideSlot::Lower => SideSlot::Upper,
        };
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let root = cx.weak_entity();
        let menu = PopupMenu::build(window, cx, |menu, _, _| {
            Self::build_side_button_menu(menu, view, target_slot, root, language)
        });
        let registration = self
            .focus_manager
            .register(&menu.focus_handle(cx), trigger, None, true)
            .expect("按钮菜单焦点注册必须唯一");
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            log::warn!("按钮菜单打开送焦失败: {error:?}");
            let _ = self.focus_manager.unregister(registration.token());
            return;
        }
        let root = cx.weak_entity();
        let dismiss = window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let _ = root.update(cx, |this, cx| this.close_side_button_menu(window, cx));
        });
        self.side_button_menu = Some(SideButtonMenu {
            view,
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
    }

    fn close_side_button_menu(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(state) = self.side_button_menu.take() else {
            return;
        };
        if let Err(error) = self.focus_manager.close_modal(window, cx) {
            log::warn!("按钮菜单关闭还焦失败: {error:?}");
        }
        if let Err(error) = self.focus_manager.unregister(state.registration.token()) {
            log::warn!("注销按钮菜单焦点失败: {error:?}");
        }
        cx.notify();
    }

    fn organize_side_button(
        &mut self,
        view: SideViewId,
        command: SideButtonCommand,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.close_side_button_menu(window, cx);
        self.sync_focus(window, cx);
        let previous_state = self.side_views.state().clone();
        let control = self.side_view_persistence.controller(cx);
        let transition = match command {
            SideButtonCommand::Reorder(movement) => self.side_views.reorder_button(view, movement),
            SideButtonCommand::MoveToOtherSlot => {
                let target = match self.side_views.state().button_slot(view) {
                    SideSlot::Upper => SideSlot::Lower,
                    SideSlot::Lower => SideSlot::Upper,
                };
                let target_index = self.side_views.state().buttons(target).len();
                match self
                    .side_views
                    .move_button_to_slot(view, target, target_index, control)
                {
                    Ok(transition) => transition,
                    Err(error) => {
                        log::warn!("侧栏按钮换区失败: {error}");
                        return;
                    }
                }
            }
            SideButtonCommand::Hide => self.side_views.set_button_hidden(view, true),
            SideButtonCommand::ResetOrder => self.side_views.reset_button_order(),
        };
        let button_return = matches!(command, SideButtonCommand::Hide).then_some(view);
        self.apply_sidebar_effects(transition, None, button_return, &previous_state, window, cx);
    }

    fn render_side_button_menu(&self, view: SideViewId) -> Option<AnyElement> {
        let state = self
            .side_button_menu
            .as_ref()
            .filter(|state| state.view == view)?;
        let mut anchor = anchored().snap_to_window_with_margin(px(8.0));
        anchor = match state.position {
            Some(position) => anchor.position(position),
            None => anchor.offset(point(px(34.0), px(0.0))),
        };
        Some(
            deferred(
                anchor.child(
                    div()
                        .debug_selector(|| "app-root-side-button-menu".to_owned())
                        .child(state.menu.clone()),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// Host 已提交实体转换；这里统一协调弱焦点节点、还焦和组织状态落盘。
    fn apply_sidebar_effects(
        &mut self,
        transition: SidebarTransition,
        focused_before: Option<SideViewId>,
        button_return: Option<SideViewId>,
        previous_state: &SidebarState,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let return_from = focused_before
            .filter(|view| transition.closed.contains(view) || !transition.is_visible);
        for view in &transition.closed {
            let registration = self
                .side_view_focus
                .remove(view)
                .expect("关闭的 SideView 必须持有焦点注册");
            if let Err(error) = self.focus_manager.unregister(registration.token()) {
                log::warn!("注销 SideView 焦点失败: {error:?}");
            }
        }
        for view in transition.opened.iter().copied() {
            let handle = self
                .side_views
                .instance(view)
                .and_then(|instance| instance.read(cx).focus_handle.clone())
                .expect("生产 SideView 必须在注册前安装焦点句柄");
            let parent = self.focus_manager.workspace_token();
            let registration = self
                .focus_manager
                .register(&handle, parent, None, transition.is_visible)
                .expect("新 SideView 焦点注册必须唯一");
            let previous = self.side_view_focus.insert(view, registration);
            debug_assert!(previous.is_none());
        }
        self.sync_sidebar_focus_visibility();
        if self.side_views.state() != previous_state {
            self.side_view_persistence
                .save_sidebar_state(self.side_views.state());
        }

        if let Some(view) = transition.focus {
            let token = self.side_view_focus[&view].token();
            if let Err(error) = self.focus_manager.request_focus(token, window, cx) {
                log::warn!("SideView 送焦失败: {error:?}");
            }
        } else if let Some(view) = return_from {
            let token = self
                .side_views
                .state()
                .button_is_visible(view)
                .then_some(view)
                .or_else(|| self.side_views.state().visible_buttons().first().copied())
                .and_then(|view| self.side_buttons.get(&view))
                .map(|button| button.registration.token())
                .unwrap_or_else(|| self.focus_manager.workspace_token());
            if let Err(error) = self.focus_manager.restore_focus(&[token], window, cx) {
                log::warn!("SideView 关闭还焦失败: {error:?}");
            }
        } else if let Some(view) =
            button_return.filter(|view| !self.side_views.state().button_is_visible(*view))
        {
            let previous = previous_state.visible_buttons();
            let index = previous
                .iter()
                .position(|candidate| *candidate == view)
                .unwrap_or(0);
            let visible = self.side_views.state().visible_buttons();
            let target = visible.get(index).copied().or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(|previous| visible.get(previous).copied())
            });
            let token = target
                .and_then(|view| self.side_buttons.get(&view))
                .map(|button| button.registration.token())
                .unwrap_or_else(|| self.focus_manager.workspace_token());
            if let Err(error) = self.focus_manager.request_focus(token, window, cx) {
                log::warn!("隐藏活动栏按钮还焦失败: {error:?}");
            }
        }
        cx.notify();
    }

    fn sync_sidebar_focus_visibility(&mut self) {
        let state = self.side_views.state();
        let visible = state.is_visible();
        let mut updates = Vec::new();
        for view in SideViewId::ALL {
            if let Some(registration) = self.side_view_focus.get(&view) {
                updates.push((registration.token(), visible));
            }
            if let Some(button) = self.side_buttons.get(&view) {
                updates.push((button.registration.token(), state.button_is_visible(view)));
            }
        }
        for (token, visible) in updates {
            let result = if visible {
                self.focus_manager.present_nonmodal(token)
            } else {
                self.focus_manager.set_visible(token, false)
            };
            if let Err(error) = result {
                log::warn!("侧栏焦点可见性同步失败: {error:?}");
            }
        }
    }

    pub(super) fn activate_side_view(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let focused_before = self.focused_side_view(window, cx);
        self.sync_focus(window, cx);
        let previous_state = self.side_views.state().clone();
        let control = self.side_view_persistence.controller(cx);
        let transition =
            self.side_views
                .activate_shortcut(view, focused_before == Some(view), control);
        self.apply_sidebar_effects(
            transition,
            focused_before,
            None,
            &previous_state,
            window,
            cx,
        );
    }

    fn click_side_view(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), SidebarError> {
        let focused_before = self.focused_side_view(window, cx);
        self.sync_focus(window, cx);
        let previous_state = self.side_views.state().clone();
        let control = self.side_view_persistence.controller(cx);
        let transition = self.side_views.click_button(view, control)?;
        self.apply_sidebar_effects(
            transition,
            focused_before,
            None,
            &previous_state,
            window,
            cx,
        );
        Ok(())
    }

    pub(super) fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let focused_before = self.focused_side_view(window, cx);
        self.sync_focus(window, cx);
        let previous_state = self.side_views.state().clone();
        let control = self.side_view_persistence.controller(cx);
        let transition = self.side_views.toggle_sidebar(control);
        self.apply_sidebar_effects(
            transition,
            focused_before,
            None,
            &previous_state,
            window,
            cx,
        );
    }

    pub(super) fn render_activity_bar(&self, cx: &Context<'_, Self>) -> AnyElement {
        div()
            .key_context(ACTIVITY_BAR_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &FocusNextSideButton, window, cx| {
                this.navigate_side_buttons(SideButtonNavigation::Next, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &FocusPreviousSideButton, window, cx| {
                    this.navigate_side_buttons(SideButtonNavigation::Previous, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &FocusFirstSideButton, window, cx| {
                this.navigate_side_buttons(SideButtonNavigation::First, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusLastSideButton, window, cx| {
                this.navigate_side_buttons(SideButtonNavigation::Last, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ActivateSideButton, window, cx| {
                this.activate_focused_side_button(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSideButtonMenu, window, cx| {
                this.open_focused_side_button_menu(window, cx);
            }))
            .w(px(ACTIVITY_BAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .pt_2()
            .pb_2()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(self.render_side_button_group(SideSlot::Upper, cx))
            .child(div().flex_1().min_h_0())
            .child(self.render_side_button_group(SideSlot::Lower, cx))
            .into_any_element()
    }

    fn render_side_button_group(&self, slot: SideSlot, cx: &Context<'_, Self>) -> AnyElement {
        let buttons = self
            .side_views
            .state()
            .buttons(slot)
            .iter()
            .copied()
            .filter(|view| self.side_views.state().button_is_visible(*view))
            .map(|view| self.render_side_view_button(view, cx));
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .children(buttons)
            .into_any_element()
    }

    fn render_side_view_button(&self, view: SideViewId, cx: &Context<'_, Self>) -> AnyElement {
        let active = self.side_views.state().open_slot(view).is_some();
        let focus = self.side_buttons[&view].handle.clone();
        let icon = match view {
            SideViewId::Nav => IconName::FolderOpen,
            SideViewId::Explorer => IconName::File,
            SideViewId::Commit => IconName::Github,
            SideViewId::AgentFiles => IconName::Bot,
        };
        div()
            .track_focus(&focus)
            .focus_visible(|_| side_button_focus_style(cx.theme().accent))
            .relative()
            .size(px(34.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .debug_selector(move || format!("app-root-side-button-{}", view.id_str()))
            .when(active, |button| {
                button.bg(cx.theme().list_active).child(
                    div()
                        .absolute()
                        .left(px(-6.0))
                        .top(px(8.0))
                        .w(px(2.0))
                        .h(px(18.0))
                        .bg(cx.theme().accent),
                )
            })
            .hover(|button| button.bg(cx.theme().list_hover))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    if let Err(error) = this.click_side_view(view, window, cx) {
                        log::warn!("侧栏按钮操作失败: {error}");
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.open_side_button_menu(view, Some(event.position), window, cx);
                }),
            )
            .child(Icon::new(icon).size(px(15.0)).text_color(if active {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            }))
            .children(self.render_side_button_menu(view))
            .into_any_element()
    }

    pub(super) fn render_sidebar(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let state = self.side_views.state();
        if !state.is_visible() {
            return None;
        }
        let open = SideSlot::ALL
            .into_iter()
            .filter_map(|slot| state.open_view(slot))
            .collect::<Vec<_>>();
        if open.is_empty() {
            return None;
        }
        let mut column = div()
            .w(px(state.width().into()))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .bg(cx.theme().sidebar);
        let count = open.len();
        for (index, view) in open.into_iter().enumerate() {
            let instance = self
                .side_views
                .instance(view)
                .expect("已打开槽位必须持有对应 SideView 实体")
                .clone();
            let slot = if count == 2 && index == 0 {
                div()
                    .w_full()
                    .h(relative(state.ratio()))
                    .min_h_0()
                    .flex_shrink_0()
                    .child(instance)
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .child(instance)
                    .into_any_element()
            };
            column = column.child(slot);
            if index + 1 < count {
                column = column.child(
                    div()
                        .h(px(slterm_split::DIVIDER_GAP))
                        .w_full()
                        .flex_shrink_0()
                        .bg(cx.theme().border),
                );
            }
        }
        Some(column.into_any_element())
    }
}
