//! 生产侧栏：活动栏与双槽只读取 SideViewHost 的组织状态和实体。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, DismissEvent, Focusable as _, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _,
    StatefulInteractiveElement as _, StyleRefinement, Styled as _, WeakEntity, Window, anchored,
    deferred, div, point, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName,
    menu::{PopupMenu, PopupMenuItem},
};
use slterm_sidebar::{ButtonMovement, SideSlot, SidebarState, SidebarTransition};
use slterm_workspace::SideViewId;

use crate::i18n::{Message, UiLanguage};

use super::command_registry::{CommandId, CommandParameters, CommandSource, CommandTarget};
use super::{
    ActivateSideButton, AdjustSidebarRatioDown, AdjustSidebarRatioDownLarge, AdjustSidebarRatioUp,
    AdjustSidebarRatioUpLarge, AdjustSidebarWidthLeft, AdjustSidebarWidthLeftLarge,
    AdjustSidebarWidthRight, AdjustSidebarWidthRightLarge, AppRoot, FocusFirstSideButton,
    FocusLastSideButton, FocusNextSideButton, FocusPreviousSideButton, OpenSideButtonMenu,
    ResetSidebarRatio, ResetSidebarWidth, SideButtonMenu, SidebarDivider, SidebarDrag,
};

pub(super) const ACTIVITY_BAR_WIDTH: f32 = 46.0;
pub(super) const ACTIVITY_BAR_KEY_CONTEXT: &str = "ActivityBar";
pub(super) const SIDEBAR_WIDTH_KEY_CONTEXT: &str = "SidebarWidthDivider";
pub(super) const SIDEBAR_RATIO_KEY_CONTEXT: &str = "SidebarRatioDivider";
const PROJECT_MIN_WIDTH: f32 = 200.0;
const SIDE_SLOT_MIN_HEIGHT: f32 = 120.0;
const DUAL_SLOT_MIN_HEIGHT: f32 = SIDE_SLOT_MIN_HEIGHT * 2.0 + slterm_split::DIVIDER_GAP;

fn effective_ratio(state: &SidebarState, viewport_height: f32) -> f32 {
    let container_height = viewport_height.max(DUAL_SLOT_MIN_HEIGHT);
    let max_upper = container_height - slterm_split::DIVIDER_GAP - SIDE_SLOT_MIN_HEIGHT;
    let upper = (container_height * state.ratio()).clamp(SIDE_SLOT_MIN_HEIGHT, max_upper);
    upper / container_height
}

pub(super) fn divider_is_visible(state: &SidebarState, divider: SidebarDivider) -> bool {
    match divider {
        SidebarDivider::Width => state.is_visible(),
        SidebarDivider::Ratio => {
            state.is_visible()
                && SideSlot::ALL
                    .into_iter()
                    .all(|slot| state.open_view(slot).is_some())
        }
    }
}

/// 活动栏键盘焦点使用主题强调色，与打开态底色区分。
pub(super) fn side_button_focus_style(accent: Hsla) -> StyleRefinement {
    StyleRefinement::default().border_1().border_color(accent)
}

pub(super) enum SideButtonNavigation {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy)]
pub(super) enum SideButtonCommand {
    Reorder(ButtonMovement),
    MoveToOtherSlot,
    Hide,
    ResetOrder,
}

fn side_button_command_parts(command: SideButtonCommand) -> (CommandId, Option<CommandParameters>) {
    match command {
        SideButtonCommand::Reorder(movement) => (
            CommandId::ReorderSideButton,
            Some(CommandParameters::SideButtonMovement(movement)),
        ),
        SideButtonCommand::MoveToOtherSlot => (CommandId::MoveSideButtonToOtherSlot, None),
        SideButtonCommand::Hide => (CommandId::HideSideButton, None),
        SideButtonCommand::ResetOrder => (CommandId::ResetSideButtonOrder, None),
    }
}

#[derive(Clone, Copy)]
pub(super) enum SidebarGeometryCommand {
    SetWidth(u32),
    ResetWidth,
    SetRatio(f32),
    ResetRatio,
}

impl AppRoot {
    /// 动作布局校验与渲染共用实际侧栏占宽；窗口压缩只影响本次绘制。
    pub(super) fn visible_sidebar_width(&self, viewport_width: f32) -> f32 {
        let state = self.side_views.state();
        if !state.is_visible() {
            return 0.0;
        }
        let available = (viewport_width - ACTIVITY_BAR_WIDTH - PROJECT_MIN_WIDTH).max(0.0);
        f32::from(state.width()).min(available)
    }

    fn execute_sidebar_action(
        &mut self,
        id: CommandId,
        target: CommandTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Err(error) =
            self.execute_command(id, CommandSource::Keyboard, target, None, window, cx)
        {
            log::warn!("侧栏键盘命令失败: {error}");
        }
    }

    pub(super) fn adjust_sidebar_width(
        &mut self,
        delta: f32,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let visible = self.visible_sidebar_width(f32::from(window.viewport_size().width));
        self.change_sidebar_geometry(
            SidebarGeometryCommand::SetWidth((visible + delta).round().max(0.0) as u32),
            cx,
        )
    }

    pub(super) fn adjust_sidebar_ratio(
        &mut self,
        delta: f32,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let visible = effective_ratio(
            self.side_views.state(),
            f32::from(window.viewport_size().height),
        );
        self.change_sidebar_geometry(SidebarGeometryCommand::SetRatio(visible + delta), cx)
    }

    fn focused_side_view(&self, window: &Window, cx: &Context<'_, Self>) -> Option<SideViewId> {
        SideViewId::ALL.into_iter().find(|view| {
            self.side_views
                .instance(*view)
                .and_then(|instance| instance.read(cx).focus_handle.as_ref())
                .is_some_and(|focus| focus.contains_focused(window, cx))
        })
    }

    pub(super) fn focused_side_button(
        &self,
        window: &Window,
        cx: &Context<'_, Self>,
    ) -> Option<SideViewId> {
        let focused = window.focused(cx)?;
        SideViewId::ALL.into_iter().find(|view| {
            self.side_buttons
                .get(view)
                .is_some_and(|button| button.handle == focused)
        })
    }

    fn focused_sidebar_divider(
        &self,
        window: &Window,
        cx: &Context<'_, Self>,
    ) -> Option<SidebarDivider> {
        let focused = window.focused(cx)?;
        [SidebarDivider::Width, SidebarDivider::Ratio]
            .into_iter()
            .find(|divider| {
                self.side_dividers
                    .get(divider)
                    .is_some_and(|separator| separator.handle == focused)
            })
    }

    fn focus_side_button(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::focus_manager::FocusError> {
        let token = self.side_buttons[&view].registration.token();
        self.focus_manager.request_focus(token, window, cx)
    }

    pub(super) fn focus_activity_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
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
            self.focus_side_button(view, window, cx)?;
        }
        Ok(())
    }

    pub(super) fn navigate_side_buttons(
        &mut self,
        navigation: SideButtonNavigation,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let current = self.focused_side_button(window, cx);
        let buttons = self.side_views.state().visible_buttons();
        if buttons.is_empty() {
            return Ok(());
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
        self.focus_side_button(buttons[index], window, cx)?;
        Ok(())
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
                let (id, parameters) = side_button_command_parts(command);
                if let Err(error) = this.execute_command(
                    id,
                    CommandSource::Menu,
                    CommandTarget::SideButton(view),
                    parameters,
                    window,
                    cx,
                ) {
                    log::warn!("侧栏菜单命令失败: {error}");
                }
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
    pub(super) fn open_side_button_menu(
        &mut self,
        view: SideViewId,
        position: Option<gpui::Point<gpui::Pixels>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.side_button_menu.is_some() {
            self.close_side_button_menu(window, cx)?;
        }
        self.focus_side_button(view, window, cx)?;
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
        let registration =
            self.focus_manager
                .register(&menu.focus_handle(cx), trigger, None, true)?;
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let root = cx.weak_entity();
        let dismiss = window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let _ = root.update(cx, |this, cx| {
                if let Err(error) = this.close_side_button_menu(window, cx) {
                    log::warn!("按钮菜单关闭失败: {error:?}");
                }
            });
        });
        self.side_button_menu = Some(SideButtonMenu {
            view,
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    fn close_side_button_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::focus_manager::FocusError> {
        let Some(state) = self.side_button_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn organize_side_button(
        &mut self,
        view: SideViewId,
        command: SideButtonCommand,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        self.close_side_button_menu(window, cx)?;
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
                        return Err(error.into());
                    }
                }
            }
            SideButtonCommand::Hide => self.side_views.set_button_hidden(view, true),
            SideButtonCommand::ResetOrder => self.side_views.reset_button_order(),
        };
        let button_return = matches!(command, SideButtonCommand::Hide).then_some(view);
        self.apply_sidebar_effects(transition, None, button_return, &previous_state, window, cx)?;
        Ok(())
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
    ) -> Result<(), super::commands::AppRootError> {
        let mut first_error = None;
        self.sidebar_drag = None;
        let divider_return = self
            .focused_sidebar_divider(window, cx)
            .filter(|divider| !divider_is_visible(self.side_views.state(), *divider))
            .and_then(|_| {
                transition.closed.first().copied().or_else(|| {
                    SideSlot::ALL
                        .into_iter()
                        .find_map(|slot| previous_state.open_view(slot))
                })
            });
        let return_from = focused_before
            .filter(|view| transition.closed.contains(view) || !transition.is_visible)
            .or(divider_return);
        for view in &transition.closed {
            let registration = self
                .side_view_focus
                .remove(view)
                .expect("关闭的 SideView 必须持有焦点注册");
            if let Err(error) = self.focus_manager.unregister(registration.token())
                && first_error.is_none()
            {
                first_error = Some(error.into());
            }
        }
        for view in transition.opened.iter().copied() {
            let handle = self
                .side_views
                .instance(view)
                .and_then(|instance| instance.read(cx).focus_handle.clone())
                .expect("生产 SideView 必须在注册前安装焦点句柄");
            let parent = self.focus_manager.workspace_token();
            let registration =
                match self
                    .focus_manager
                    .register(&handle, parent, None, transition.is_visible)
                {
                    Ok(registration) => registration,
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error.into());
                        }
                        continue;
                    }
                };
            let previous = self.side_view_focus.insert(view, registration);
            debug_assert!(previous.is_none());
        }
        if let Err(error) = self.sync_sidebar_focus_visibility()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if self.side_views.state() != previous_state {
            self.side_view_persistence
                .save_sidebar_state(self.side_views.state());
        }

        if let Some(view) = transition.focus {
            if let Some(registration) = self.side_view_focus.get(&view) {
                if let Err(error) =
                    self.focus_manager
                        .request_focus(registration.token(), window, cx)
                    && first_error.is_none()
                {
                    first_error = Some(error.into());
                }
            } else if first_error.is_none() {
                first_error = Some(super::focus_manager::FocusError::Unavailable.into());
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
            if let Err(error) = self.focus_manager.restore_focus(&[token], window, cx)
                && first_error.is_none()
            {
                first_error = Some(error.into());
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
            if let Err(error) = self.focus_manager.request_focus(token, window, cx)
                && first_error.is_none()
            {
                first_error = Some(error.into());
            }
        }
        cx.notify();
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn sync_sidebar_focus_visibility(&mut self) -> Result<(), super::commands::AppRootError> {
        let mut first_error = None;
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
        for divider in [SidebarDivider::Width, SidebarDivider::Ratio] {
            let registration = &self.side_dividers[&divider].registration;
            updates.push((registration.token(), divider_is_visible(state, divider)));
        }
        for (token, visible) in updates {
            let result = if visible {
                self.focus_manager.present_nonmodal(token)
            } else {
                self.focus_manager.set_visible(token, false)
            };
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            Err(error.into())
        } else {
            Ok(())
        }
    }

    pub(super) fn activate_side_view(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
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
        )
    }

    pub(super) fn click_side_view(
        &mut self,
        view: SideViewId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
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
        )
    }

    pub(super) fn toggle_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
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
        )
    }

    pub(super) fn render_activity_bar(&self, cx: &Context<'_, Self>) -> AnyElement {
        div()
            .key_context(ACTIVITY_BAR_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &FocusNextSideButton, window, cx| {
                this.execute_sidebar_action(
                    CommandId::FocusNextSideButton,
                    CommandTarget::Focused,
                    window,
                    cx,
                );
            }))
            .on_action(
                cx.listener(|this, _: &FocusPreviousSideButton, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::FocusPreviousSideButton,
                        CommandTarget::Focused,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(cx.listener(|this, _: &FocusFirstSideButton, window, cx| {
                this.execute_sidebar_action(
                    CommandId::FocusFirstSideButton,
                    CommandTarget::Focused,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &FocusLastSideButton, window, cx| {
                this.execute_sidebar_action(
                    CommandId::FocusLastSideButton,
                    CommandTarget::Focused,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ActivateSideButton, window, cx| {
                this.execute_sidebar_action(
                    CommandId::ActivateSideButton,
                    CommandTarget::Focused,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &OpenSideButtonMenu, window, cx| {
                this.execute_sidebar_action(
                    CommandId::OpenSideButtonMenu,
                    CommandTarget::Focused,
                    window,
                    cx,
                );
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
                    if let Err(error) = this.execute_command(
                        CommandId::ActivateSideButton,
                        CommandSource::Pointer,
                        CommandTarget::SideButton(view),
                        None,
                        window,
                        cx,
                    ) {
                        log::warn!("侧栏按钮操作失败: {error}");
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    if let Err(error) = this.execute_command(
                        CommandId::OpenSideButtonMenu,
                        CommandSource::Pointer,
                        CommandTarget::SideButton(view),
                        Some(CommandParameters::SideButtonMenuPosition(event.position)),
                        window,
                        cx,
                    ) {
                        log::warn!("侧栏按钮菜单打开失败: {error}");
                    }
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

    fn begin_sidebar_drag(
        &mut self,
        divider: SidebarDivider,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let token = self.side_dividers[&divider].registration.token();
        if let Err(error) = self.focus_manager.request_focus(token, window, cx) {
            log::warn!("侧栏分隔线送焦失败: {error:?}");
        }
        if event.click_count >= 2 {
            self.sidebar_drag = None;
            let (id, target) = match divider {
                SidebarDivider::Width => (
                    CommandId::ResetSidebarWidth,
                    CommandTarget::SidebarWidthDivider,
                ),
                SidebarDivider::Ratio => (
                    CommandId::ResetSidebarRatio,
                    CommandTarget::SidebarRatioDivider,
                ),
            };
            if let Err(error) =
                self.execute_command(id, CommandSource::Pointer, target, None, window, cx)
            {
                log::warn!("分隔线双击复位失败: {error}");
            }
            return;
        }
        self.sidebar_drag = Some(match divider {
            SidebarDivider::Width => SidebarDrag::Width {
                start_x: event.position.x.into(),
                start_width: self.visible_sidebar_width(f32::from(window.viewport_size().width)),
            },
            SidebarDivider::Ratio => SidebarDrag::Ratio {
                start_y: event.position.y.into(),
                start_ratio: effective_ratio(
                    self.side_views.state(),
                    f32::from(window.viewport_size().height),
                ),
            },
        });
        cx.notify();
    }

    fn update_sidebar_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(drag) = self.sidebar_drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.finish_sidebar_drag(cx);
            return;
        }
        match drag {
            SidebarDrag::Width {
                start_x,
                start_width,
            } => {
                let width = start_width + f32::from(event.position.x) - start_x;
                if let Err(error) = self.change_sidebar_geometry(
                    SidebarGeometryCommand::SetWidth(width.round().max(0.0) as u32),
                    cx,
                ) {
                    log::warn!("侧栏宽度拖动失败: {error}");
                }
            }
            SidebarDrag::Ratio {
                start_y,
                start_ratio,
            } => {
                let height = f32::from(window.viewport_size().height).max(DUAL_SLOT_MIN_HEIGHT);
                let ratio = start_ratio + (f32::from(event.position.y) - start_y) / height;
                if let Err(error) =
                    self.change_sidebar_geometry(SidebarGeometryCommand::SetRatio(ratio), cx)
                {
                    log::warn!("侧栏比例拖动失败: {error}");
                }
            }
        }
    }

    fn finish_sidebar_drag(&mut self, cx: &mut Context<'_, Self>) {
        if self.sidebar_drag.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn render_sidebar_drag_overlay(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let drag = self.sidebar_drag?;
        let overlay = div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_sidebar_drag(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            );
        Some(
            match drag {
                SidebarDrag::Width { .. } => overlay.cursor_col_resize(),
                SidebarDrag::Ratio { .. } => overlay.cursor_row_resize(),
            }
            .into_any_element(),
        )
    }

    pub(super) fn change_sidebar_geometry(
        &mut self,
        command: SidebarGeometryCommand,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let previous_state = self.side_views.state().clone();
        let result = match command {
            SidebarGeometryCommand::SetWidth(width) => {
                self.side_views.set_width(width);
                Ok(())
            }
            SidebarGeometryCommand::ResetWidth => {
                self.side_views.reset_width();
                Ok(())
            }
            SidebarGeometryCommand::SetRatio(ratio) => self.side_views.set_ratio(ratio),
            SidebarGeometryCommand::ResetRatio => {
                self.side_views.reset_ratio();
                Ok(())
            }
        };
        result?;
        if self.side_views.state() != &previous_state {
            self.side_view_persistence
                .save_sidebar_state(self.side_views.state());
            cx.notify();
        }
        Ok(())
    }

    fn render_sidebar_width_divider(&self, window: &Window, cx: &Context<'_, Self>) -> AnyElement {
        let focus = self.side_dividers[&SidebarDivider::Width].handle.clone();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let visible_width = self.visible_sidebar_width(f32::from(window.viewport_size().width));
        div()
            .id("sidebar-width-divider")
            .role(gpui::Role::Slider)
            .accessibility_id("sidebar-width-divider")
            .aria_label(language.text(Message::SidebarWidthDivider))
            .aria_numeric_value(f64::from(visible_width))
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(f64::from(SidebarState::MAX_WIDTH))
            .aria_numeric_value_step(16.0)
            .key_context(SIDEBAR_WIDTH_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &AdjustSidebarWidthLeft, window, cx| {
                this.execute_sidebar_action(
                    CommandId::AdjustSidebarWidthLeft,
                    CommandTarget::SidebarWidthDivider,
                    window,
                    cx,
                );
            }))
            .on_action(
                cx.listener(|this, _: &AdjustSidebarWidthRight, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::AdjustSidebarWidthRight,
                        CommandTarget::SidebarWidthDivider,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this, _: &AdjustSidebarWidthLeftLarge, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::AdjustSidebarWidthLeftLarge,
                        CommandTarget::SidebarWidthDivider,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this, _: &AdjustSidebarWidthRightLarge, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::AdjustSidebarWidthRightLarge,
                        CommandTarget::SidebarWidthDivider,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(cx.listener(|this, _: &ResetSidebarWidth, window, cx| {
                this.execute_sidebar_action(
                    CommandId::ResetSidebarWidth,
                    CommandTarget::SidebarWidthDivider,
                    window,
                    cx,
                );
            }))
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(slterm_split::DIVIDER_GAP))
            .track_focus(&focus)
            .focus_visible(|_| side_button_focus_style(cx.theme().accent))
            .debug_selector(|| "app-root-sidebar-width-divider".to_owned())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.begin_sidebar_drag(SidebarDivider::Width, event, window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            )
            .into_any_element()
    }

    fn render_sidebar_ratio_divider(&self, window: &Window, cx: &Context<'_, Self>) -> AnyElement {
        let focus = self.side_dividers[&SidebarDivider::Ratio].handle.clone();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let visible_ratio = effective_ratio(
            self.side_views.state(),
            f32::from(window.viewport_size().height),
        );
        div()
            .id("sidebar-ratio-divider")
            .role(gpui::Role::Slider)
            .accessibility_id("sidebar-ratio-divider")
            .aria_label(language.text(Message::SidebarRatioDivider))
            .aria_numeric_value(f64::from(visible_ratio * 100.0))
            .aria_min_numeric_value(f64::from(SidebarState::MIN_RATIO * 100.0))
            .aria_max_numeric_value(f64::from(SidebarState::MAX_RATIO * 100.0))
            .aria_numeric_value_step(2.0)
            .key_context(SIDEBAR_RATIO_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &AdjustSidebarRatioUp, window, cx| {
                this.execute_sidebar_action(
                    CommandId::AdjustSidebarRatioUp,
                    CommandTarget::SidebarRatioDivider,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &AdjustSidebarRatioDown, window, cx| {
                this.execute_sidebar_action(
                    CommandId::AdjustSidebarRatioDown,
                    CommandTarget::SidebarRatioDivider,
                    window,
                    cx,
                );
            }))
            .on_action(
                cx.listener(|this, _: &AdjustSidebarRatioUpLarge, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::AdjustSidebarRatioUpLarge,
                        CommandTarget::SidebarRatioDivider,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this, _: &AdjustSidebarRatioDownLarge, window, cx| {
                    this.execute_sidebar_action(
                        CommandId::AdjustSidebarRatioDownLarge,
                        CommandTarget::SidebarRatioDivider,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(cx.listener(|this, _: &ResetSidebarRatio, window, cx| {
                this.execute_sidebar_action(
                    CommandId::ResetSidebarRatio,
                    CommandTarget::SidebarRatioDivider,
                    window,
                    cx,
                );
            }))
            .h(px(slterm_split::DIVIDER_GAP))
            .w_full()
            .flex_shrink_0()
            .track_focus(&focus)
            .focus_visible(|_| side_button_focus_style(cx.theme().accent))
            .debug_selector(|| "app-root-sidebar-ratio-divider".to_owned())
            .bg(cx.theme().border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.begin_sidebar_drag(SidebarDivider::Ratio, event, window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_sidebar_drag(cx)),
            )
            .into_any_element()
    }

    pub(super) fn render_sidebar(
        &self,
        window: &Window,
        cx: &Context<'_, Self>,
    ) -> Option<AnyElement> {
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
        let viewport = window.viewport_size();
        let viewport_height = f32::from(viewport.height);
        let dual_slots = open.len() == 2;
        let container_height = if dual_slots {
            viewport_height.max(DUAL_SLOT_MIN_HEIGHT)
        } else {
            viewport_height
        };
        let upper_height = container_height * effective_ratio(state, viewport_height);
        let mut column = div()
            .relative()
            .w_full()
            .h(px(container_height))
            .flex_shrink_0()
            .flex()
            .flex_col();
        let count = open.len();
        for (index, view) in open.into_iter().enumerate() {
            let instance = self
                .side_views
                .instance(view)
                .expect("已打开槽位必须持有对应 SideView 实体")
                .clone();
            let slot = if dual_slots && index == 0 {
                div()
                    .w_full()
                    .h(px(upper_height))
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
                column = column.child(self.render_sidebar_ratio_divider(window, cx));
            }
        }
        Some(
            div()
                .id("sidebar")
                .w(px(self.visible_sidebar_width(f32::from(viewport.width))))
                .h_full()
                .flex_shrink_0()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(cx.theme().sidebar_border)
                .bg(cx.theme().sidebar)
                .child(column.child(self.render_sidebar_width_divider(window, cx)))
                .into_any_element(),
        )
    }
}
