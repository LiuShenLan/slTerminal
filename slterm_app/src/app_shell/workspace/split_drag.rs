//! 分隔条拖拽 overlay(05 篇 M5.3 数据流「分隔条拖拽状态机」)。
//!
//! 因果链:指针移动只更新 [`SplitDrag`] 的预览值与全窗 overlay;pane flex
//! 树继续用提交比例,终端网格与 PTY 都不追预览。松手才 `commit_ratio`
//! 写回树节点并走结构性 resize 同步下发——高频事件不重排网格,避免
//! Term 与 ConPTY 在拖拽期间反复 rewrap(02 篇 TTY 尺寸链交界)。
//!
//! 关闭区:原始比例越界(低于 `CLOSE_MARGIN` 或高于 `1 - CLOSE_MARGIN`)
//! 时预览线钉在边缘、显示关闭高亮;松手仅当被挤压侧是单叶才走
//! `close_pane`(子树不做整树误杀,按钳制带提交)。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, MouseButton, MouseMoveEvent,
    ParentElement as _, Pixels, Point, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_split::{
    DIVIDER_GAP, PaneId, SplitDirection, SplitTree, commit_ratio, drag_close_target, drag_ratio,
    preview_ratio as preview_curve_ratio,
};

use super::{SltermWorkspace, WorkspaceTab, to_split_rect};
use crate::gpui_terminal::view::TerminalView;

/// 进行中的分隔条拖拽:目标 Split 节点以树路径寻址(`Divider` 同语义)。
/// 指针 → 比例的换算依赖 `SplitBoundsStore` 记录的该节点上一帧视口矩形。
pub(super) struct SplitDrag {
    pub tab: usize,
    pub path: Vec<bool>,
    pub direction: SplitDirection,
    /// 轻量视觉预览。pane flex 树在松手前始终用提交比例,拖拽不重排网格。
    pub preview_ratio: f32,
    /// `Some(false)` 关第一个孩子,`Some(true)` 关第二个。
    pub close_target: Option<bool>,
    pub last_notified: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SplitDragVisual {
    divider: slterm_split::Rect,
    close_area: Option<slterm_split::Rect>,
}

/// 无副作用的 overlay 几何:预览线跟手,关闭区用被挤压侧的矩形高亮。
/// 不触发任何 pane 布局。
fn split_drag_visual_geometry(
    direction: SplitDirection,
    viewport: slterm_split::Rect,
    preview_ratio: f32,
    close_target: Option<bool>,
) -> SplitDragVisual {
    let usable = match direction {
        SplitDirection::LeftRight => (viewport.w - DIVIDER_GAP).max(0.0),
        SplitDirection::TopBottom => (viewport.h - DIVIDER_GAP).max(0.0),
    };
    let split = usable * preview_ratio.clamp(0.0, 1.0);
    let divider = match direction {
        SplitDirection::LeftRight => {
            slterm_split::Rect::new(viewport.x + split, viewport.y, DIVIDER_GAP, viewport.h)
        }
        SplitDirection::TopBottom => {
            slterm_split::Rect::new(viewport.x, viewport.y + split, viewport.w, DIVIDER_GAP)
        }
    };
    let close_area = close_target.map(|second| match (direction, second) {
        (SplitDirection::LeftRight, false) => {
            slterm_split::Rect::new(viewport.x, viewport.y, split, viewport.h)
        }
        (SplitDirection::LeftRight, true) => slterm_split::Rect::new(
            divider.x + divider.w,
            viewport.y,
            (viewport.w - split - divider.w).max(0.0),
            viewport.h,
        ),
        (SplitDirection::TopBottom, false) => {
            slterm_split::Rect::new(viewport.x, viewport.y, viewport.w, split)
        }
        (SplitDirection::TopBottom, true) => slterm_split::Rect::new(
            viewport.x,
            divider.y + divider.h,
            viewport.w,
            (viewport.h - split - divider.h).max(0.0),
        ),
    });
    SplitDragVisual {
        divider,
        close_area,
    }
}

impl SltermWorkspace {
    /// 分隔条拖拽的指针移动:指针位置 → 原始比例 → 轻量预览线(常规带
    /// 跟手、关闭区钉边示意「松手即关」)。pane flex、终端网格与 PTY 都不
    /// 追预览,松手后才做一次结构提交。高频鼠标事件最多按 125Hz 通知绘制;
    /// 关闭目标变化即时刷新。
    pub(super) fn update_split_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.split_drag.is_none() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            let position = event.position;
            self.finish_split_drag(position, window, cx);
            return;
        }
        let Some(raw) = self.split_drag_raw_ratio(event.position, window, cx) else {
            return;
        };
        let preview = preview_curve_ratio(raw);
        let close = drag_close_target(raw);
        let Some(drag) = self.split_drag.as_mut() else {
            return;
        };
        let changed =
            (drag.preview_ratio - preview).abs() > f32::EPSILON || drag.close_target != close;
        if !changed {
            return;
        }
        let close_changed = drag.close_target != close;
        drag.preview_ratio = preview;
        drag.close_target = close;
        if close_changed || drag.last_notified.elapsed() >= std::time::Duration::from_millis(8) {
            drag.last_notified = std::time::Instant::now();
            cx.notify();
        }
    }

    /// 只属于 overlay 的视觉:预览线与关闭区高亮。它活在稳定的 pane 树
    /// 之上,移动它不会用新布局调用 `TerminalElement::prepaint`。
    pub(super) fn split_drag_visual(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let drag = self.split_drag.as_ref()?;
        let viewport = self
            .split_bounds
            .borrow()
            .get(&(drag.tab, drag.path.clone()))
            .copied()?;
        let visual = split_drag_visual_geometry(
            drag.direction,
            to_split_rect(&viewport),
            drag.preview_ratio,
            drag.close_target,
        );
        let line = visual.divider;
        let close = visual.close_area.map(|area| {
            div()
                .absolute()
                .left(px(area.x))
                .top(px(area.y))
                .w(px(area.w))
                .h(px(area.h))
                .bg(cx.theme().warning.opacity(0.12))
        });
        Some(
            div()
                .absolute()
                .inset_0()
                .children(close)
                .child(
                    div()
                        .absolute()
                        .left(px(line.x))
                        .top(px(line.y))
                        .w(px(line.w))
                        .h(px(line.h))
                        .debug_selector(|| "split-drag-line".to_owned())
                        .bg(cx.theme().primary.opacity(0.88)),
                )
                .into_any_element(),
        )
    }

    /// 当前拖拽的指针位置 → 未钳制原始比例(依赖上一帧节点视口记录)。
    fn split_drag_raw_ratio(
        &self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<f32> {
        let drag = self.split_drag.as_ref()?;
        let viewport = self
            .split_bounds
            .borrow()
            .get(&(drag.tab, drag.path.clone()))
            .copied()?;
        let (cell_w, line_h) = TerminalView::cell_metrics(window, cx);
        let cell = match drag.direction {
            SplitDirection::LeftRight => f32::from(cell_w),
            SplitDirection::TopBottom => f32::from(line_h),
        };
        Some(drag_ratio(
            drag.direction,
            to_split_rect(&viewport),
            DIVIDER_GAP,
            cell,
            f32::from(position.x),
            f32::from(position.y),
        ))
    }

    /// 松手:关闭区按被挤压侧收尾(仅当那一侧是单叶——子树不做整树误杀,
    /// 按钳制带提交);常规带比例吸附整数单元格后写回提交值,并标记该 tab
    /// 为结构性 resize(一次性同步下发)。
    pub(super) fn finish_split_drag(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let raw = self.split_drag_raw_ratio(position, window, cx);
        let Some(drag) = self.split_drag.take() else {
            return;
        };
        let viewport = self
            .split_bounds
            .borrow()
            .get(&(drag.tab, drag.path.clone()))
            .copied();
        let (cell_w, line_h) = TerminalView::cell_metrics(window, cx);
        let cell = match drag.direction {
            SplitDirection::LeftRight => f32::from(cell_w),
            SplitDirection::TopBottom => f32::from(line_h),
        };

        // 先裁定拖拽关闭(close_pane 需要 &mut self,借用两段式)。
        let mut close_target: Option<PaneId> = None;
        if let Some(raw) = raw
            && let Some(WorkspaceTab::Terminal { tree, .. }) =
                self.tabs.get_mut(drag.tab).map(|entry| &mut entry.tab)
            && let Some(SplitTree::Split { first, second, .. }) = tree.node_mut(&drag.path)
            && let Some(squeezed_second) = drag_close_target(raw)
        {
            let child = if squeezed_second {
                second.as_ref()
            } else {
                first.as_ref()
            };
            if let SplitTree::Leaf(id) = child {
                close_target = Some(*id);
            }
        }
        if let Some(pane_id) = close_target {
            // 先清拖拽态再关 pane,避免确认/收编路径背后残留高亮分隔线。
            if let Some(WorkspaceTab::Terminal { tree, .. }) =
                self.tabs.get_mut(drag.tab).map(|entry| &mut entry.tab)
                && let Some(SplitTree::Split {
                    preview_ratio,
                    dragging,
                    ..
                }) = tree.node_mut(&drag.path)
            {
                *preview_ratio = None;
                *dragging = false;
            }
            self.close_pane(pane_id, window, cx);
            cx.notify();
            return;
        }

        if let Some(WorkspaceTab::Terminal { tree, .. }) =
            self.tabs.get_mut(drag.tab).map(|entry| &mut entry.tab)
            && let Some(SplitTree::Split {
                ratio,
                preview_ratio,
                dragging,
                ..
            }) = tree.node_mut(&drag.path)
        {
            if let (Some(raw), Some(viewport)) = (raw, viewport) {
                let viewport = to_split_rect(&viewport);
                let extent = match drag.direction {
                    SplitDirection::LeftRight => viewport.w,
                    SplitDirection::TopBottom => viewport.h,
                };
                *ratio = commit_ratio(preview_curve_ratio(raw), extent, DIVIDER_GAP, cell);
            }
            *preview_ratio = None;
            *dragging = false;
        }
        // 松手提交 = 几何一次性落定,PTY 同步下发,不进尾沿去抖。
        self.mark_structural_resize(drag.tab, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod split_drag_tests {
    use super::*;

    /// overlay 几何:分隔线落在预览比例处,关闭区精确覆盖被挤压侧。
    #[test]
    fn visual_geometry_tracks_preview_and_marks_squeezed_side() {
        let viewport = slterm_split::Rect::new(0.0, 0.0, 1002.0, 600.0);

        let regular = split_drag_visual_geometry(SplitDirection::LeftRight, viewport, 0.5, None);
        // usable = 1000,split = 500;分隔线 2px。
        assert_eq!((regular.divider.x, regular.divider.w), (500.0, DIVIDER_GAP));
        assert_eq!(regular.close_area, None);

        // 挤左:关闭区 = 左半区。
        let left =
            split_drag_visual_geometry(SplitDirection::LeftRight, viewport, 0.02, Some(false));
        assert_eq!(left.close_area.unwrap().w, 20.0);

        // 挤右:关闭区从分隔线右缘到视口右缘。
        let right =
            split_drag_visual_geometry(SplitDirection::LeftRight, viewport, 0.98, Some(true));
        let area = right.close_area.unwrap();
        assert!((area.x - 982.0).abs() < 0.001);
        assert!((area.w - 20.0).abs() < 0.001);
    }

    /// 上下方向同构:分隔线 y 跟预览比例,关闭区高度覆盖被挤压侧。
    #[test]
    fn visual_geometry_is_top_bottom_symmetric() {
        let viewport = slterm_split::Rect::new(0.0, 0.0, 800.0, 602.0);
        let preview =
            split_drag_visual_geometry(SplitDirection::TopBottom, viewport, 0.25, Some(true));
        // usable = 600,split = 150。
        assert_eq!((preview.divider.y, preview.divider.h), (150.0, DIVIDER_GAP));
        let area = preview.close_area.unwrap();
        assert_eq!(area.y, 152.0);
        assert_eq!(area.h, 450.0);
    }
}
