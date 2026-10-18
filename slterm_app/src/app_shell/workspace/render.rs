//! 工作区渲染下沉面(壳层职责):终端 tab 内容区与分屏树递归。
//!
//! 从 `mod.rs` 拆出只为文件行数预算;职责与不变量不变——渲染永远读
//! **提交比例**,拖拽预览只走 overlay,终端网格零重排。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Bounds, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, Pixels, SharedString, Styled as _, canvas, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use slterm_split::{DIVIDER_GAP, HIT_SLOP, PaneId, SplitDirection, SplitTree};

use super::super::file_drop::FileTreeDrag;
use super::{SltermWorkspace, WorkspaceTab};

/// 分屏递归的只读渲染上下文:tab 身份(下标供调试选择器、TabId 供矩形
/// 缓存键)+ pane 属主与聚焦叶。递归调用只传本结构,不逐参展开。
struct SplitRenderContext<'a> {
    tab_ix: usize,
    tab_id: slterm_split::TabId,
    panes: &'a [super::TerminalPane],
    focused: PaneId,
}

/// GPUI `Bounds` → `slterm_split::Rect`(同为窗口逻辑像素坐标系)。
pub(super) fn to_split_rect(bounds: &Bounds<Pixels>) -> slterm_split::Rect {
    slterm_split::Rect::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

impl SltermWorkspace {
    /// Terminal tab 的内容区:zoom 或单叶 → 聚焦 pane 满卡直渲(M3 路径
    /// 保留为特例);否则按分屏树递归铺陈。
    pub(super) fn render_terminal_tab(
        &self,
        tab_ix: usize,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let Some(entry) = self.tabs.get(tab_ix) else {
            return div().into_any_element();
        };
        let tab_id = entry.id;
        let WorkspaceTab::Terminal {
            tree,
            panes,
            focused,
            zoomed,
        } = &entry.tab
        else {
            return div().into_any_element();
        };
        if *zoomed || tree.is_leaf() {
            let pane = panes
                .iter()
                .find(|pane| pane.id == *focused)
                .or_else(|| panes.first());
            let Some(pane) = pane else {
                return div().into_any_element();
            };
            let pane_id = pane.id;
            let store = self.pane_bounds.clone();
            let drop_view = pane.view.clone();
            // 满卡路径也记录窗口坐标:方向导航与拖拽命中依赖上一帧矩形,
            // 不能只在进入分屏递归后才得到边界。
            return div()
                .size_full()
                .min_w_0()
                .min_h_0()
                .relative()
                .debug_selector(move || format!("slterm-pane-{}", pane_id.0))
                // 文件树拖放落点:路径原文进 PTY。
                .on_drop(move |drag: &FileTreeDrag, _, cx| {
                    if let Some(bytes) = crate::side_panel::drop_text_for_path(&drag.path_text) {
                        drop_view.update(cx, |view, cx| view.drop_path_text(bytes, cx));
                    }
                })
                .child(pane.view.clone())
                .child(
                    canvas(
                        move |bounds, _, _| {
                            store.borrow_mut().insert(pane_id, bounds);
                        },
                        |_, _, _, _| (),
                    )
                    .absolute()
                    .inset_0(),
                )
                .into_any_element();
        }
        let mut path = Vec::new();
        let ctx = SplitRenderContext {
            tab_ix,
            tab_id,
            panes,
            focused: *focused,
        };
        self.render_split_node(&ctx, tree, &mut path, cx)
    }

    /// 递归渲染分屏树。Split 节点 = flex 容器:first 占 `relative(ratio)`、
    /// 分隔条固定 `DIVIDER_GAP`、second 吃剩余(与 `slterm_split::layout`
    /// 的切割数学等价到 ±divider×ratio ≤ 2px;比例在提交时按真实视口吸附
    /// 整格,不累积漂移)。
    ///
    /// 渲染永远读**提交比例**:拖拽预览只走 overlay,终端网格零重排。
    /// 节点视口与 pane 矩形经 canvas prepaint 回写帧记录,供拖拽换算与
    /// 方向导航消费。
    fn render_split_node(
        &self,
        ctx: &SplitRenderContext<'_>,
        node: &SplitTree<PaneId>,
        path: &mut Vec<bool>,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        match node {
            SplitTree::Leaf(id) => {
                let id = *id;
                let Some(pane) = ctx.panes.iter().find(|pane| pane.id == id) else {
                    return div().into_any_element();
                };
                let store = self.pane_bounds.clone();
                let is_focused = id == ctx.focused;
                let tab_ix = ctx.tab_ix;
                // 非聚焦 pane 压暗:开关来自 Settings.veil,
                // 自定义主题 effects.veil 显式值权威。veil 只盖终端区,
                // 不压暗分屏分隔条;关闭时完全不绘制压暗层。
                let veil_enabled = cx
                    .try_global::<crate::app_shell::config::Settings>()
                    .is_none_or(|settings| settings.veil);
                let veil = (!is_focused && veil_enabled).then(|| {
                    div()
                        .absolute()
                        .inset_0()
                        .debug_selector(move || format!("pane-veil-{}", id.0))
                        .bg(cx.theme().background.opacity(0.35))
                });
                let drop_view = pane.view.clone();
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .debug_selector(move || format!("slterm-pane-{}", id.0))
                    // 文件树拖放落点:路径原文进 PTY。
                    .on_drop(move |drag: &FileTreeDrag, _, cx| {
                        if let Some(bytes) = crate::side_panel::drop_text_for_path(&drag.path_text)
                        {
                            drop_view.update(cx, |view, cx| view.drop_path_text(bytes, cx));
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.focus_pane(tab_ix, id, window, cx);
                        }),
                    )
                    // canvas 探针量整个叶子:方向导航与 dock 命中读的是 pane
                    // 矩形。absolute + inset_0 不参与布局,纯观测。
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                store.borrow_mut().insert(id, bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        div()
                            .size_full()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            .child(pane.view.clone())
                            .children(veil),
                    )
                    .into_any_element()
            }
            SplitTree::Split {
                direction,
                ratio,
                dragging,
                first,
                second,
                ..
            } => {
                let direction = *direction;
                // 交互拖拽走轻量 overlay;这里保持提交比例,避免每次高频
                // 指针事件都重建/重排两侧终端网格。
                let show_ratio = *ratio;
                let dragging = *dragging;
                let key = (ctx.tab_id, path.clone());
                let store = self.split_bounds.clone();
                let probe = canvas(
                    move |bounds, _, _| {
                        store.borrow_mut().insert(key.clone(), bounds);
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0();

                path.push(false);
                let first_el = self.render_split_node(ctx, first, path, cx);
                path.pop();
                path.push(true);
                let second_el = self.render_split_node(ctx, second, path, cx);
                path.pop();

                let tab_ix = ctx.tab_ix;
                let tab_id = ctx.tab_id;
                let drag_path = path.clone();
                let drag_start_ratio = *ratio;
                let selector_path = drag_path.clone();
                let divider_id =
                    SharedString::from(format!("split-divider-{tab_ix}-{drag_path:?}"));
                let bar_color = if dragging {
                    cx.theme().primary
                } else {
                    cx.theme().border.opacity(0.35)
                };
                // 视觉线 DIVIDER_GAP,命中区向两侧各扩 HIT_SLOP(旧壳同参)。
                let hit = div()
                    .id(divider_id)
                    .debug_selector(move || format!("split-divider-{tab_ix}-{selector_path:?}"))
                    .absolute()
                    .map(|hit| match direction {
                        SplitDirection::LeftRight => hit
                            .top_0()
                            .bottom_0()
                            .left(px(-HIT_SLOP))
                            .w(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_col_resize(),
                        SplitDirection::TopBottom => hit
                            .left_0()
                            .right_0()
                            .top(px(-HIT_SLOP))
                            .h(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_row_resize(),
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            this.split_drag = Some(super::split_drag::SplitDrag {
                                tab: tab_id,
                                path: drag_path.clone(),
                                direction,
                                preview_ratio: drag_start_ratio,
                                close_target: None,
                                last_notified: std::time::Instant::now(),
                            });
                            if let Some(WorkspaceTab::Terminal { tree, .. }) =
                                this.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab)
                                && let Some(SplitTree::Split {
                                    preview_ratio,
                                    dragging,
                                    ..
                                }) = tree.node_mut(&drag_path)
                            {
                                *preview_ratio = None;
                                *dragging = true;
                            }
                            cx.notify();
                        }),
                    );
                let divider = div()
                    .relative()
                    .flex_shrink_0()
                    .map(|bar| match direction {
                        SplitDirection::LeftRight => bar.w(px(DIVIDER_GAP)).h_full(),
                        SplitDirection::TopBottom => bar.h(px(DIVIDER_GAP)).w_full(),
                    })
                    .bg(bar_color)
                    .child(hit);

                div()
                    .size_full()
                    .relative()
                    .flex()
                    .map(|el| match direction {
                        SplitDirection::LeftRight => el.flex_row(),
                        SplitDirection::TopBottom => el.flex_col(),
                    })
                    .child(probe)
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .map(|el| match direction {
                                SplitDirection::LeftRight => el.h_full().w(relative(show_ratio)),
                                SplitDirection::TopBottom => el.w_full().h(relative(show_ratio)),
                            })
                            .child(first_el),
                    )
                    .child(divider)
                    .child(div().flex_1().min_w_0().min_h_0().child(second_el))
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    /// 满卡路径与分屏递归共用同一坐标换算:GPUI Bounds → slterm_split Rect
    /// 保持左上原点与宽高逐字段一致。
    #[test]
    fn bounds_to_split_rect_keeps_origin_and_size() {
        let bounds = Bounds::new(
            gpui::point(px(10.0), px(20.0)),
            gpui::size(px(300.0), px(200.0)),
        );
        let rect = to_split_rect(&bounds);
        assert_eq!((rect.x, rect.y, rect.w, rect.h), (10.0, 20.0, 300.0, 200.0));
    }
}
