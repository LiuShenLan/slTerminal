//! 生产侧栏：活动栏与双槽只读取 SideViewHost 的组织状态和实体。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, div, px, relative,
};
use gpui_component::{ActiveTheme as _, Icon, IconName};
use slterm_sidebar::{SideSlot, SidebarError};
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

    fn click_side_view(
        &mut self,
        view: SideViewId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), SidebarError> {
        let control = self.side_view_persistence.controller(cx);
        self.side_views.click_button(view, control)?;
        self.side_view_persistence
            .save_sidebar_state(self.side_views.state());
        cx.notify();
        Ok(())
    }

    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<'_, Self>) {
        let control = self.side_view_persistence.controller(cx);
        self.side_views.toggle_sidebar(control);
        self.side_view_persistence
            .save_sidebar_state(self.side_views.state());
        cx.notify();
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
        let icon = match view {
            SideViewId::Nav => IconName::FolderOpen,
            SideViewId::Explorer => IconName::File,
            SideViewId::Commit => IconName::Github,
            SideViewId::AgentFiles => IconName::Bot,
        };
        div()
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
                cx.listener(move |this, _, _, cx| {
                    if let Err(error) = this.click_side_view(view, cx) {
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
