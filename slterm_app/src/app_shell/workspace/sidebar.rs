//! 侧栏骨架:ActivityBar + 上下双槽内容区。
//!
//! `slterm_sidebar::SidebarState` 是按钮顺序、显隐、槽位、打开视图、显示意图、
//! 宽度与比例的唯一事实；`SideViewHost` 消费转换效果管理真实实体。本模块只
//! 负责展示与手势换算，不再推断第二套槽位或持久化 DTO。
//!
//! 双槽同时打开时按 core 比例分配高度,中间分隔条可拖拽。拖拽只改内存态并
//! 重绘,终端网格零重排、不触发即时写盘。

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    canvas, div, px,
};
use gpui_component::{ActiveTheme as _, Icon, IconName};
use slterm_sidebar::SideSlot;

pub(crate) use slterm_sidebar::SidebarState;
pub(crate) use slterm_workspace::SideViewId;

use super::SltermWorkspace;
use crate::app_shell::config::Settings;
use crate::i18n::Message;

/// ActivityBar 固定宽度(旧栈同值 46)。
pub(crate) const ACTIVITY_BAR_SIZE: f32 = 46.0;
/// 侧栏 tab 行高度(旧壳纵向列表同档 30)。
const TAB_ROW_HEIGHT: f32 = 30.0;
/// 相邻 tab 行间距。
const TAB_ROW_GAP: f32 = 2.0;
/// 宽度拖拽热区宽度(内容区右缘竖条)。
pub(crate) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 4.0;
/// 侧栏内容区最小宽度。
pub(super) const SIDEBAR_WIDTH_MIN: f32 = SidebarState::MIN_WIDTH as f32;
/// 侧栏内容区最大宽度。
pub(super) const SIDEBAR_WIDTH_MAX: f32 = SidebarState::MAX_WIDTH as f32;
/// 双槽分隔条命中带高度(视觉线居中 1px,整带可拖)。
const SIDEBAR_SPLIT_DIVIDER_HEIGHT: f32 = 5.0;

/// 双槽布局推导结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SidebarLayout {
    Hidden,
    SingleTop,
    SingleBottom,
    Split,
}

/// core 身份到本壳展示文案的扩展。
trait SideViewIdExt {
    fn title(self) -> Message;
}

impl SideViewIdExt for SideViewId {
    fn title(self) -> Message {
        match self {
            Self::Nav => Message::SidebarViewNav,
            Self::Explorer => Message::SidebarViewExplorer,
            Self::Commit => Message::SidebarViewCommit,
            Self::AgentFiles => Message::SidebarViewAgentFiles,
        }
    }
}

/// SideView 展示注册项：id + title。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SideViewDef {
    pub id: SideViewId,
    pub title: Message,
}

/// 封闭集定义列表(注册序 = 旧栈 sideViewDefs 注册序)。
#[cfg(test)]
pub(super) fn side_view_defs() -> [SideViewDef; 4] {
    SideViewId::ALL.map(|id| SideViewDef {
        id,
        title: id.title(),
    })
}

/// 启动几何用的侧栏总宽；实际不可见时只剩 ActivityBar。
pub(crate) fn chrome_width(state: &SidebarState) -> f32 {
    if state.is_visible() {
        ACTIVITY_BAR_SIZE + f32::from(state.width()) + SIDEBAR_RESIZE_HANDLE_WIDTH
    } else {
        ACTIVITY_BAR_SIZE
    }
}

/// 指针 y → 上槽比例；无效几何回默认值，有效输入交给 core 钳制。
pub(super) fn sidebar_split_ratio_at(pointer_y: f32, top: f32, height: f32) -> f32 {
    if !pointer_y.is_finite() || !top.is_finite() || !height.is_finite() || height <= 0.0 {
        return SidebarState::DEFAULT_RATIO;
    }
    ((pointer_y - top) / height).clamp(SidebarState::MIN_RATIO, SidebarState::MAX_RATIO)
}

/// 由双槽打开状态纯推导展示形态；显示意图在渲染根单独判定。
pub(super) fn derive_layout(state: &SidebarState) -> SidebarLayout {
    match (
        state.open_view(SideSlot::Upper).is_some(),
        state.open_view(SideSlot::Lower).is_some(),
    ) {
        (false, false) => SidebarLayout::Hidden,
        (true, false) => SidebarLayout::SingleTop,
        (false, true) => SidebarLayout::SingleBottom,
        (true, true) => SidebarLayout::Split,
    }
}

/// 进行中的侧栏宽度拖拽:指针原始位移与起始宽度解耦,只改内容区宽度,
/// 不重排终端网格。
pub(super) struct SidebarResizeDrag {
    pub start_x: f32,
    pub start_width: f32,
}

/// 指针位置 → 新宽度，按 core 区间钳制。
pub(super) fn dragged_width(drag: &SidebarResizeDrag, pointer_x: f32) -> f32 {
    let width = drag.start_width + pointer_x - drag.start_x;
    if width.is_finite() {
        width.clamp(SIDEBAR_WIDTH_MIN, SIDEBAR_WIDTH_MAX)
    } else {
        SIDEBAR_WIDTH_MIN
    }
}

/// 进行中的双槽分隔条拖拽。比例由指针在 `sidebar-area` 内的相对 y 直接
/// 求得(见 [`sidebar_split_ratio_at`]),故除手势标记外无需起始量。
pub(super) struct SidebarSplitDrag;

impl SltermWorkspace {
    /// ActivityBar:封闭集按钮区；active 态 = 选中底色 + 左侧指示条。
    pub(super) fn render_activity_bar(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = self.side_views.state();
        let language = cx.global::<Settings>().language;
        div()
            .id("sidebar-activity-bar")
            .debug_selector(|| "sidebar-activity-bar".to_owned())
            .w(px(ACTIVITY_BAR_SIZE))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .py(px(6.0))
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .children(state.visible_buttons().into_iter().map(|id| {
                let def = SideViewDef {
                    id,
                    title: id.title(),
                };
                let active = state.open_slot(id).is_some();
                let hover_bg = cx.theme().list_hover;
                let active_bg = cx.theme().sidebar_accent;
                let idle_fg = cx.theme().muted_foreground;
                let active_fg = cx.theme().sidebar_accent_foreground;
                let accent = cx.theme().primary;
                div()
                    .id(SharedString::from(format!(
                        "sidebar-view-btn-{}",
                        def.id.id_str()
                    )))
                    .debug_selector(move || format!("sidebar-view-btn-{}", def.id.id_str()))
                    .w(px(34.0))
                    .h(px(34.0))
                    .mb(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .when(active, |button| {
                        button
                            .bg(active_bg)
                            .text_color(active_fg)
                            .border_l_2()
                            .border_color(accent)
                    })
                    .when(!active, |button| button.text_color(idle_fg))
                    .hover(move |style| style.bg(hover_bg))
                    .child(
                        language
                            .text(def.title)
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_string(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_sidebar_view(def.id, cx);
                    }))
            }))
            // 底部配置钮:设置 tab 的唯一常驻入口。
            .child(div().flex_1())
            .child(
                div()
                    .id("sidebar-settings-btn")
                    .debug_selector(|| "sidebar-settings-btn".to_owned())
                    .w(px(34.0))
                    .h(px(34.0))
                    .mb(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .hover({
                        let hover_bg = cx.theme().list_hover;
                        move |button| button.bg(hover_bg)
                    })
                    .child(Icon::new(IconName::Settings).size(px(16.0)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings_tab(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// 侧栏列:顶部常驻 tab 列表 + 其下双槽内容区。
    ///
    /// tab 列表不随双槽全空而消失——它是切换 tab 与拖拽停靠的入口;
    /// 整列是否隐藏由 workspace 根按 core 实际可见性判定。
    pub(super) fn render_sidebar_content(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = self.side_views.state();
        let layout = derive_layout(state);
        let views_open = layout != SidebarLayout::Hidden && state.visible_intent();
        let top_open = state.open_view(SideSlot::Upper).is_some();
        let bottom_open = state.open_view(SideSlot::Lower).is_some();
        let split = top_open && bottom_open;
        // 双槽同时打开时按 core 比例分配高度;单槽独占整个内容区。
        let ratio = state.ratio();
        let (top_grow, bottom_grow) = if split {
            (ratio, 1.0 - ratio)
        } else {
            (1.0, 1.0)
        };
        // 视图内容需要 `&mut self`(文件树轮询/路径栏);先求值再进链式
        // 闭包,避免多个闭包同时借 self。
        let top_view = top_open.then(|| self.render_view_slot(SideSlot::Upper, top_grow, cx));
        let bottom_view =
            bottom_open.then(|| self.render_view_slot(SideSlot::Lower, bottom_grow, cx));
        let tabs = self.render_tab_list(cx);
        // 只观测的内容区矩形:分隔条拖拽把指针 y 换算成比例时读取上一帧
        // 区域,不参与布局(absolute + inset_0)。
        let split_bounds = self.sidebar_split_bounds.clone();
        div()
            .id("sidebar-column")
            .debug_selector(|| "sidebar-column".to_owned())
            .size_full()
            .flex()
            .flex_col()
            // 待命阶段的指针跟踪(激活后由根部罩层独占)。
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_tab_drag(event, window, cx);
            }))
            .child(tabs)
            .when(views_open, |column| {
                column.child(
                    div()
                        .id("sidebar-area")
                        .debug_selector(|| "sidebar-area".to_owned())
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .flex()
                        .flex_col()
                        .bg(cx.theme().sidebar)
                        .child(
                            canvas(
                                move |bounds, _, _| {
                                    *split_bounds.borrow_mut() = Some(bounds);
                                },
                                |_, _, _, _| (),
                            )
                            .absolute()
                            .inset_0(),
                        )
                        .children(top_view)
                        .when(split, |area| {
                            let divider_hover = cx.theme().primary.opacity(0.15);
                            area.child(
                                div()
                                    .id("sidebar-split-divider")
                                    .debug_selector(|| "sidebar-split-divider".to_owned())
                                    .h(px(SIDEBAR_SPLIT_DIVIDER_HEIGHT))
                                    .w_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .cursor_row_resize()
                                    // 拖拽热区 5px,视觉线保持 1px。
                                    .hover(move |style| style.bg(divider_hover))
                                    .child(div().h(px(1.0)).w_full().bg(cx.theme().sidebar_border))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                            // 与宽度拖拽互斥:同一时刻只允许一个手势。
                                            if this.sidebar_resize.is_some() {
                                                return;
                                            }
                                            this.sidebar_split_drag = Some(SidebarSplitDrag);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                        })
                        .children(bottom_view),
                )
            })
            .into_any_element()
    }

    /// 侧栏 tab 列表(左侧垂直 tab 侧边栏)。
    ///
    /// 行是 tab 拖拽的源:按下进入待命态,越过阈值后拖进终端区停靠。
    /// 徽章/新建钮/折叠槽与右键菜单归后续步骤。
    pub(super) fn render_tab_list(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let active = self.active;
        let active_bg = cx.theme().sidebar_accent;
        let active_fg = cx.theme().sidebar_accent_foreground;
        let muted = cx.theme().muted_foreground;
        let hover_bg = cx.theme().list_hover;
        let rows = self.tabs.iter().enumerate().map(|(ix, entry)| {
            let title = self.tab_title(ix, cx);
            let is_active = ix == active;
            let tab_id = entry.id;
            div()
                .id(SharedString::from(format!("sidebar-tab-{ix}")))
                .debug_selector(move || format!("sidebar-tab-{ix}"))
                .h(px(TAB_ROW_HEIGHT))
                .w_full()
                .flex_shrink_0()
                .px(px(8.0))
                .flex()
                .items_center()
                .rounded(px(6.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .when(is_active, |row| row.bg(active_bg).text_color(active_fg))
                .when(!is_active, |row| {
                    row.text_color(muted).hover(move |style| style.bg(hover_bg))
                })
                .child(div().min_w_0().truncate().child(title))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_tab(ix, window, cx);
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, _| {
                        this.begin_tab_drag(tab_id, event.position);
                    }),
                )
        });
        div()
            .id("sidebar-tabs")
            .debug_selector(|| "sidebar-tabs".to_owned())
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(TAB_ROW_GAP))
            .py(px(6.0))
            .children(rows)
            .into_any_element()
    }

    /// 单槽内容挂载点:按封闭集分派到各视图渲染函数。
    /// `grow` = flex 增长因子(双槽时按 core 比例,单槽 1.0);basis 归零保证
    /// 两槽严格按比例分高,不被内容固有高度带偏。
    fn render_view_slot(
        &mut self,
        zone: SideSlot,
        grow: f32,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let Some(id) = self.side_views.state().open_view(zone) else {
            return div().into_any_element();
        };
        let zone_name = match zone {
            SideSlot::Upper => "top",
            SideSlot::Lower => "bottom",
        };
        let selector = format!("sidebar-slot-{zone_name}-{}", id.id_str());
        let body = match id {
            SideViewId::Explorer => self.render_file_tree(cx),
            SideViewId::Nav => self.render_nav_view(cx),
            SideViewId::Commit => self.render_commit_view(cx),
            SideViewId::AgentFiles => self.render_agent_files_view(cx),
        };
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector)
            .flex_basis(px(0.0))
            .flex_grow(grow)
            .flex_shrink_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(body)
            .into_any_element()
    }

    /// explorer 槽当前可见(内容渲染/跟随 cwd 的单一判据)。
    pub(super) fn explorer_open(&self) -> bool {
        let state = self.side_views.state();
        state.is_visible() && state.open_slot(SideViewId::Explorer).is_some()
    }

    /// Commit 当前实际可见；文件树后台快照刷新判据与 Explorer 同形。
    pub(super) fn commit_open(&self) -> bool {
        let state = self.side_views.state();
        state.is_visible() && state.open_slot(SideViewId::Commit).is_some()
    }

    /// 宽度拖拽热区:内容区右缘 4px 竖条。
    pub(super) fn render_sidebar_resize_handle(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        div()
            .id("sidebar-resize-handle")
            .debug_selector(|| "sidebar-resize-handle".to_owned())
            .w(px(SIDEBAR_RESIZE_HANDLE_WIDTH))
            .h_full()
            .flex_shrink_0()
            .cursor_col_resize()
            .bg(cx.theme().sidebar_border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    // 与双槽分隔条拖拽互斥:同一时刻只允许一个手势。
                    if this.sidebar_split_drag.is_some() {
                        return;
                    }
                    this.sidebar_resize = Some(SidebarResizeDrag {
                        start_x: f32::from(event.position.x),
                        start_width: f32::from(this.side_views.state().width()),
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// 拖拽期间的全窗指针罩层:松手/失配即收尾。
    pub(super) fn render_sidebar_resize_overlay(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        self.sidebar_resize.as_ref().map(|_| {
            div()
                .absolute()
                .inset_0()
                .occlude()
                .cursor_col_resize()
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.update_sidebar_resize(event, cx);
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.finish_sidebar_resize(cx);
                    }),
                )
                .into_any_element()
        })
    }

    /// 双槽分隔条拖拽期间的全窗指针罩层(与宽度拖拽同套路):移动实时
    /// 换算比例;松手/按键失配即收尾。
    pub(super) fn render_sidebar_split_overlay(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        self.sidebar_split_drag.as_ref().map(|_| {
            div()
                .absolute()
                .inset_0()
                .occlude()
                .cursor_row_resize()
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.update_sidebar_split_drag(event, cx);
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.finish_sidebar_split_drag(cx);
                    }),
                )
                .into_any_element()
        })
    }

    fn toggle_sidebar_view(&mut self, id: SideViewId, cx: &mut Context<'_, Self>) {
        self.side_views
            .click_button(
                id,
                super::side_view::SideViewController::new(
                    cx,
                    Rc::clone(&self.side_view_state_cache),
                    Arc::clone(&self.view_state_registry),
                    self.view_state_writer.clone(),
                    self.side_view_state_write_blocked.clone(),
                ),
            )
            .expect("活动栏点击必须对应可见按钮");
        cx.notify();
    }

    fn update_sidebar_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.finish_sidebar_resize(cx);
            return;
        }
        let Some(drag) = self.sidebar_resize.as_ref() else {
            return;
        };
        let width = dragged_width(drag, f32::from(event.position.x));
        if (f32::from(self.side_views.state().width()) - width).abs() > f32::EPSILON {
            self.side_views.set_width(width.round().max(0.0) as u32);
            cx.notify();
        }
    }

    fn finish_sidebar_resize(&mut self, cx: &mut Context<'_, Self>) {
        if self.sidebar_resize.take().is_some() {
            cx.notify();
        }
    }

    /// 分隔条拖拽的指针移动:按 `sidebar-area` 上一帧矩形把指针 y 换算成
    /// 上槽比例。只改 `SidebarState` 并重绘——不触碰 pane/pane 树,终端
    /// 网格零重排;持久化由 1 Hz 自动保存快照自然带走,不在此写盘。
    fn update_sidebar_split_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.finish_sidebar_split_drag(cx);
            return;
        }
        if self.sidebar_split_drag.is_none() {
            return;
        }
        let Some(bounds) = *self.sidebar_split_bounds.borrow() else {
            return;
        };
        let ratio = sidebar_split_ratio_at(
            f32::from(event.position.y),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.height),
        );
        if (self.side_views.state().ratio() - ratio).abs() > f32::EPSILON {
            self.side_views
                .set_ratio(ratio)
                .expect("指针换算必须产生有限侧栏比例");
            cx.notify();
        }
    }

    fn finish_sidebar_split_drag(&mut self, cx: &mut Context<'_, Self>) {
        if self.sidebar_split_drag.take().is_some() {
            cx.notify();
        }
    }

    /// 侧栏整区开合(策略动作 ctrl-shift-b)。
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<'_, Self>) {
        self.side_views
            .toggle_sidebar(super::side_view::SideViewController::new(
                cx,
                Rc::clone(&self.side_view_state_cache),
                Arc::clone(&self.view_state_registry),
                self.view_state_writer.clone(),
                self.side_view_state_write_blocked.clone(),
            ));
        cx.notify();
    }
}

#[cfg(test)]
mod sidebar_state_tests {
    use super::*;
    use crate::i18n::UiLanguage;

    /// 四种标题在中英两语下由同一 typed 消息查表。
    #[test]
    fn side_view_titles_localize_in_both_languages() {
        let expected = [
            (SideViewId::Nav, "导航树", "Navigation"),
            (SideViewId::Explorer, "文件浏览器", "File Explorer"),
            (SideViewId::Commit, "Commit", "Commit"),
            (SideViewId::AgentFiles, "Agent 全局文件", "Agent Files"),
        ];
        for (id, zh, en) in expected {
            assert_eq!(UiLanguage::ZhCn.text(id.title()), zh);
            assert_eq!(UiLanguage::EnUs.text(id.title()), en);
        }
        for def in side_view_defs() {
            assert_eq!(def.title, def.id.title());
        }
    }

    /// 展示形态只从 core 双槽状态推导，显示意图不改变槽位。
    #[test]
    fn layout_is_derived_from_core_slots() {
        let mut state = SidebarState::default();
        assert_eq!(derive_layout(&state), SidebarLayout::SingleTop);
        state.close_view(SideViewId::Nav);
        assert_eq!(derive_layout(&state), SidebarLayout::Hidden);

        state
            .move_button_to_slot(SideViewId::AgentFiles, SideSlot::Lower, 0)
            .unwrap();
        state.click_button(SideViewId::AgentFiles).unwrap();
        assert_eq!(derive_layout(&state), SidebarLayout::SingleBottom);
        state.click_button(SideViewId::Nav).unwrap();
        assert_eq!(derive_layout(&state), SidebarLayout::Split);
        state.toggle_sidebar();
        assert!(!state.visible_intent());
        assert_eq!(derive_layout(&state), SidebarLayout::Split);
    }

    /// 指针 y → 上槽比例；越界钳制，退化几何回默认。
    #[test]
    fn pointer_y_maps_to_a_clamped_split_ratio() {
        assert_eq!(sidebar_split_ratio_at(400.0, 100.0, 600.0), 0.5);
        assert_eq!(sidebar_split_ratio_at(250.0, 100.0, 600.0), 0.25);
        assert_eq!(
            sidebar_split_ratio_at(-5_000.0, 100.0, 600.0),
            SidebarState::MIN_RATIO
        );
        assert_eq!(
            sidebar_split_ratio_at(5_000.0, 100.0, 600.0),
            SidebarState::MAX_RATIO
        );
        assert_eq!(
            sidebar_split_ratio_at(400.0, 100.0, 0.0),
            SidebarState::DEFAULT_RATIO
        );
        assert_eq!(
            sidebar_split_ratio_at(f32::NAN, 100.0, 600.0),
            SidebarState::DEFAULT_RATIO
        );
    }

    /// 宽度拖拽纯净换算，并采用 core 的完整宽度区间。
    #[test]
    fn dragged_width_tracks_pointer_and_clamps_to_core_bounds() {
        let drag = SidebarResizeDrag {
            start_x: 300.0,
            start_width: 250.0,
        };
        assert_eq!(dragged_width(&drag, 340.0), 290.0);
        assert_eq!(dragged_width(&drag, -10_000.0), SIDEBAR_WIDTH_MIN);
        assert_eq!(dragged_width(&drag, 10_000.0), SIDEBAR_WIDTH_MAX);
        assert_eq!(dragged_width(&drag, f32::NAN), SIDEBAR_WIDTH_MIN);
    }
}
