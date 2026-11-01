//! 生产侧栏：活动栏与双槽只读取 SideViewHost 的组织状态和实体。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, Window, div, px, relative,
};
use gpui_component::{ActiveTheme as _, Icon, IconName};
use slterm_sidebar::{SideSlot, SidebarError, SidebarState, SidebarTransition};
use slterm_workspace::SideViewId;

use super::AppRoot;

pub(super) const ACTIVITY_BAR_WIDTH: f32 = 46.0;

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

    /// Host 已提交实体转换；这里统一协调弱焦点节点、还焦和组织状态落盘。
    fn apply_sidebar_effects(
        &mut self,
        transition: SidebarTransition,
        focused_before: Option<SideViewId>,
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
        self.apply_sidebar_effects(transition, focused_before, &previous_state, window, cx);
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
        self.apply_sidebar_effects(transition, focused_before, &previous_state, window, cx);
        Ok(())
    }

    pub(super) fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let focused_before = self.focused_side_view(window, cx);
        self.sync_focus(window, cx);
        let previous_state = self.side_views.state().clone();
        let control = self.side_view_persistence.controller(cx);
        let transition = self.side_views.toggle_sidebar(control);
        self.apply_sidebar_effects(transition, focused_before, &previous_state, window, cx);
    }

    pub(super) fn render_activity_bar(&self, cx: &Context<'_, Self>) -> AnyElement {
        let buttons = self
            .side_views
            .state()
            .visible_buttons()
            .into_iter()
            .map(|view| self.render_side_view_button(view, cx));
        div()
            .w(px(ACTIVITY_BAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .pt_2()
            .gap_1()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
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
            .child(Icon::new(icon).size(px(15.0)).text_color(if active {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            }))
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
