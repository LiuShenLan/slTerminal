//! tab 级拖拽停靠(拖拽停靠状态机)。
//!
//! 因果链:tab 是拖拽单位——拖动一个 tab 等于把它的**整棵分屏树**嫁接到
//! 活跃 tab 的某处,而不是拆出单个 pane。指针越过 4px 阈值才进入拖拽,未
//! 越过保留普通点击(激活 tab)语义。
//!
//! 落点两态:
//! - `pane = None` → 窗口内容区外圈(outer rim)根级停靠,整树与目标树
//!   50/50 并排;外圈只在多 pane 时存在,单 pane 的四分位已覆盖全部区域。
//! - `pane = Some(id)` → 悬停 pane 矩形的四分位最近边(`nearest_edge`)。
//!
//! 拖拽期间只画 [`DockTarget`] 预览覆盖层,分屏树与终端网格零重排;松手才
//! 走 [`SltermWorkspace::dock_tab_into_active`] 提交。目标 pane 在手势途中
//! 被关时,`SplitTree::dock_at_leaf` 以 `Err(source)` 原样退回源树——壳侧
//! 必须把源 tab 按原位置原状态放回,不得 panic、不得半提交。
//!
//! 树手术的唯一生产调用点:根级走 [`dock_tree`] 薄封装(`joined`),pane 级
//! 走 `dock_at_leaf`;壳层其余代码不直接摸 `joined`。

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, MouseButton, MouseMoveEvent,
    Pixels, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_split::{DIVIDER_GAP, PaneId, Rect, SplitDirection, SplitNav, SplitTree, TabId};

use super::render::to_split_rect;
use super::{SltermWorkspace, WorkspaceTab};

/// 拖拽启动阈值(逻辑像素):按住不动/轻微抖动是点击,越过才进入拖拽。
pub(super) const TAB_DRAG_THRESHOLD: f32 = 4.0;
/// 外圈停靠带最大宽度;实际取 `min(14px, 视口宽高各 5%)`。
const OUTER_RIM_MAX: f32 = 14.0;
/// 预览刷新节流,与分隔条拖拽同节奏(125Hz 上限)。
const PREVIEW_REFRESH_INTERVAL: Duration = Duration::from_millis(8);
/// 停靠预览圆角(旧壳卡片圆角同档)。
const PREVIEW_CORNER_RADIUS: f32 = 6.0;

/// 一次停靠的落点描述:目标 pane(`None` = 窗口内容区外圈根级)+ 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DockTarget {
    pub pane: Option<PaneId>,
    pub nav: SplitNav,
}

/// 进行中的 tab 拖拽(含未过阈值的待命态)。
pub(super) struct TabDrag {
    /// 手势开始时冻结的稳定身份:拖拽途中关 tab/重排都不漂移。
    pub source: TabId,
    pub press_x: f32,
    pub press_y: f32,
    /// 越过阈值才为真;未越过时松手仍是普通点击。
    pub active: bool,
    /// 最近一次指针位置算出的落点;`None` = 当前不在可停靠区域。
    pub dock: Option<DockTarget>,
    pub last_notified: Instant,
}

/// 指针位移是否越过拖拽阈值(单轴越界即可,照上游)。
pub(super) fn threshold_crossed(press_x: f32, press_y: f32, x: f32, y: f32) -> bool {
    (x - press_x).abs() >= TAB_DRAG_THRESHOLD || (y - press_y).abs() >= TAB_DRAG_THRESHOLD
}

/// 根级嫁接的唯一生产调用点(壳侧薄封装):目标树与源树 50/50 并排,
/// `nav` 决定方向与先后。
pub(super) fn dock_tree(
    target: SplitTree<PaneId>,
    source: SplitTree<PaneId>,
    nav: SplitNav,
) -> SplitTree<PaneId> {
    target.joined(source, nav)
}

/// 指针位置 → 落点。外圈保留整宽/整高插入;圈内落点只归悬停叶。
///
/// 外圈判定先于叶命中:多 pane 时贴内容区四边即根级停靠,不会因为边带
/// 同时压在某片叶子上而退化成叶内四分位。
pub(super) fn dock_target_at(
    area: Rect,
    panes: &[(PaneId, Rect)],
    x: f32,
    y: f32,
) -> Option<DockTarget> {
    if !area.contains(x, y) || panes.is_empty() {
        return None;
    }
    let rim = OUTER_RIM_MAX.min(area.w * 0.05).min(area.h * 0.05);
    if panes.len() > 1
        && (x - area.x < rim
            || area.x + area.w - x < rim
            || y - area.y < rim
            || area.y + area.h - y < rim)
    {
        return Some(DockTarget {
            pane: None,
            nav: nearest_edge(area, x, y),
        });
    }
    let (pane, area) = panes.iter().find(|(_, area)| area.contains(x, y))?;
    Some(DockTarget {
        pane: Some(*pane),
        nav: nearest_edge(*area, x, y),
    })
}

/// 指针到矩形四边归一化距离的最近边(相对悬停对象自身)。
pub(super) fn nearest_edge(area: Rect, x: f32, y: f32) -> SplitNav {
    let nx = (x - area.x) / area.w.max(1.0);
    let ny = (y - area.y) / area.h.max(1.0);
    [
        (nx, SplitNav::Left),
        (1.0 - nx, SplitNav::Right),
        (ny, SplitNav::Up),
        (1.0 - ny, SplitNav::Down),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .unwrap()
    .1
}

/// 停靠预览矩形:把一棵 50/50 哑树铺进目标区域,取源树将占据的那一半。
///
/// 不调 `joined` 构造哑树是刻意的:`joined` 的唯一生产调用点是 [`dock_tree`];
/// 这里只复刻其方向/先后映射,几何由 `layout` 裁定,等值性由单测钉住。
pub(super) fn preview_rect(area: Rect, nav: SplitNav) -> Rect {
    const SOURCE: PaneId = PaneId(1);
    const TARGET: PaneId = PaneId(0);
    let (direction, before) = match nav {
        SplitNav::Left => (SplitDirection::LeftRight, true),
        SplitNav::Right => (SplitDirection::LeftRight, false),
        SplitNav::Up => (SplitDirection::TopBottom, true),
        SplitNav::Down => (SplitDirection::TopBottom, false),
    };
    let (first, second) = if before {
        (SplitTree::leaf(SOURCE), SplitTree::leaf(TARGET))
    } else {
        (SplitTree::leaf(TARGET), SplitTree::leaf(SOURCE))
    };
    let tree = SplitTree::Split {
        direction,
        ratio: 0.5,
        preview_ratio: None,
        dragging: false,
        first: Box::new(first),
        second: Box::new(second),
    };
    tree.layout(area, 1.0, 1.0, DIVIDER_GAP, false)
        .panes
        .into_iter()
        .find(|(pane, _)| *pane == SOURCE)
        .map(|(_, rect)| rect)
        .unwrap_or(area)
}

impl SltermWorkspace {
    /// tab 行按下:进入待命态(冻结稳定身份 + 按点),越过阈值后转拖拽。
    pub(super) fn begin_tab_drag(&mut self, source: TabId, position: gpui::Point<Pixels>) {
        self.tab_drag = Some(TabDrag {
            source,
            press_x: f32::from(position.x),
            press_y: f32::from(position.y),
            active: false,
            dock: None,
            last_notified: Instant::now(),
        });
    }

    /// 指针移动:阈值判定 + 落点换算。未激活只跟踪;激活后仅刷新预览。
    pub(super) fn update_tab_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((press_x, press_y, active)) = self
            .tab_drag
            .as_ref()
            .map(|drag| (drag.press_x, drag.press_y, drag.active))
        else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.finish_tab_drag(window, cx);
            return;
        }
        let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
        let crossed = threshold_crossed(press_x, press_y, x, y);
        // 落点换算要在可变借用 drag 之前完成(它读 tabs/矩形缓存)。
        let dock = if active || crossed {
            self.dock_target_at_pointer(x, y)
        } else {
            None
        };
        let Some(drag) = self.tab_drag.as_mut() else {
            return;
        };
        let became_active = !drag.active && crossed;
        if became_active {
            drag.active = true;
        }
        if !drag.active {
            return;
        }
        let changed = drag.dock != dock;
        drag.dock = dock;
        if became_active || changed || drag.last_notified.elapsed() >= PREVIEW_REFRESH_INTERVAL {
            drag.last_notified = Instant::now();
            cx.notify();
        }
    }

    /// 松手:命中落点时提交停靠;否则丢弃手势(tab 原样留在列表)。
    ///
    /// 刻意用**缓存落点**而不是按松手坐标重算:手势期间目标 pane 被关时,
    /// 提交端必须收到那个已消失的目标并走 `Err(source)` 原样退回路径。
    pub(super) fn finish_tab_drag(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(drag) = self.tab_drag.take() else {
            return;
        };
        if drag.active
            && let Some(target) = drag.dock
        {
            self.dock_tab_into_active(drag.source, target, window, cx);
        }
        cx.notify();
    }

    /// 根节点兜底:未过阈值的松手只清待命态,保留行点击语义;已激活的
    /// 松手(overlay 来不及接管)直接提交。
    pub(super) fn release_tab_drag(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.tab_drag.as_ref().map(|drag| drag.active) {
            Some(true) => self.finish_tab_drag(window, cx),
            Some(false) => {
                self.tab_drag = None;
                cx.notify();
            }
            None => {}
        }
    }

    /// 停靠预览覆盖层。只画落点矩形,不触碰任何 pane 布局。
    pub(super) fn tab_drag_preview(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let drag = self.tab_drag.as_ref().filter(|drag| drag.active)?;
        let target = drag.dock?;
        let area = self.dock_preview_rect(target)?;
        Some(
            div()
                .absolute()
                .left(px(area.x))
                .top(px(area.y))
                .w(px(area.w))
                .h(px(area.h))
                .debug_selector(|| "tab-dock-preview".to_owned())
                .rounded(px(PREVIEW_CORNER_RADIUS))
                // 低透明水洗 + 细描边:提示落区边界,不盖过终端内容。
                .border_1()
                .border_color(cx.theme().primary.opacity(0.28))
                .bg(cx.theme().primary.opacity(0.08))
                .into_any_element(),
        )
    }

    /// 指针 → 落点(仅 Terminal↔Terminal、且源不是活跃 tab)。
    fn dock_target_at_pointer(&self, x: f32, y: f32) -> Option<DockTarget> {
        let source = self.tab_drag.as_ref()?.source;
        let source_ix = self.tabs.iter().position(|entry| entry.id == source)?;
        if source_ix == self.active {
            return None;
        }
        if !matches!(self.tabs[source_ix].tab, WorkspaceTab::Terminal { .. }) {
            return None;
        }
        if !matches!(
            self.tabs.get(self.active).map(|entry| &entry.tab),
            Some(WorkspaceTab::Terminal { .. })
        ) {
            return None;
        }
        let area = self.active_terminal_area()?;
        let panes = self.dock_pane_areas();
        dock_target_at(area, &panes, x, y)
    }

    /// 活跃 tab 的终端内容区(窗口坐标)。分屏取树根节点上一帧矩形;
    /// zoom 或单叶取聚焦 pane;两者都缺时退回全部 pane 矩形的并集。
    fn active_terminal_area(&self) -> Option<Rect> {
        let entry = self.tabs.get(self.active)?;
        let WorkspaceTab::Terminal {
            panes,
            tree,
            focused,
            zoomed,
        } = &entry.tab
        else {
            return None;
        };
        if !*zoomed
            && !tree.is_leaf()
            && let Some(bounds) = self
                .split_bounds
                .borrow()
                .get(&(entry.id, Vec::new()))
                .copied()
        {
            let rect = to_split_rect(&bounds);
            if rect.w > 0.0 && rect.h > 0.0 {
                return Some(rect);
            }
        }
        let bounds = self.pane_bounds.borrow();
        if *zoomed || tree.is_leaf() {
            // 缩放态只认屏幕上实际可见的 pane;其余 pane 的上一帧矩形仍在
            // 缓存里,不能把命中区扩到当前终端之外。
            let pane = panes
                .iter()
                .find(|pane| pane.id == *focused)
                .or_else(|| panes.first())?;
            let b = bounds.get(&pane.id)?;
            let rect = to_split_rect(b);
            return (rect.w > 0.0 && rect.h > 0.0).then_some(rect);
        }
        let mut acc: Option<(f32, f32, f32, f32)> = None;
        for pane in panes {
            let Some(b) = bounds.get(&pane.id) else {
                continue;
            };
            let (x0, y0) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let (x1, y1) = (x0 + f32::from(b.size.width), y0 + f32::from(b.size.height));
            acc = Some(match acc {
                Some((ax0, ay0, ax1, ay1)) => (ax0.min(x0), ay0.min(y0), ax1.max(x1), ay1.max(y1)),
                None => (x0, y0, x1, y1),
            });
        }
        let (x0, y0, x1, y1) = acc?;
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// 活跃 tab 内每个 pane 的命中矩形(树按提交比例铺陈,cell=1px)。
    ///
    /// 缩放态的多 pane 树不提供落点:落点几何必须与屏幕可见的满卡预览
    /// 一致,而满卡里其余 pane 并不在屏上;先还原缩放才能继续并入。
    fn dock_pane_areas(&self) -> Vec<(PaneId, Rect)> {
        let Some(area) = self.active_terminal_area() else {
            return vec![];
        };
        let Some(entry) = self.tabs.get(self.active) else {
            return vec![];
        };
        let WorkspaceTab::Terminal { tree, zoomed, .. } = &entry.tab else {
            return vec![];
        };
        if *zoomed && !tree.is_leaf() {
            return vec![];
        }
        tree.layout(area, 1.0, 1.0, DIVIDER_GAP, false).panes
    }

    /// 预览矩形:目标 pane 的四分位一半,或外圈根级停靠时内容区的一半。
    fn dock_preview_rect(&self, target: DockTarget) -> Option<Rect> {
        let area = match target.pane {
            Some(pane) => {
                self.dock_pane_areas()
                    .into_iter()
                    .find(|(id, _)| *id == pane)?
                    .1
            }
            None => self.active_terminal_area()?,
        };
        Some(preview_rect(area, target.nav))
    }

    /// 落点提交:整棵源 tab 树嫁接到活跃 tab。
    ///
    /// 顺序 = 先摘源、再挂树、失败原样放回:目标 pane 在手势途中被关时,
    /// `dock_at_leaf` 返回 `Err(source)`;此刻源 tab 的 panes 实体、树、
    /// 聚焦与 zoom 状态一样未动,按原下标插回即完整退回。成功路径才推进
    /// 节点矩形缓存(摘源时已清除其陈旧键)并做一次结构性 resize 下发。
    fn dock_tab_into_active(
        &mut self,
        source: TabId,
        target: DockTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(source_ix) = self.tabs.iter().position(|entry| entry.id == source) else {
            return false;
        };
        if source_ix == self.active {
            return false;
        }
        if !matches!(self.tabs[source_ix].tab, WorkspaceTab::Terminal { .. }) {
            return false;
        }
        if !matches!(
            self.tabs.get(self.active).map(|entry| &entry.tab),
            Some(WorkspaceTab::Terminal { .. })
        ) {
            return false;
        }
        let original_active = self.active;
        let Some(entry) = self.remove_tab_entry(source_ix) else {
            return false;
        };
        let super::TabEntry {
            id: source_id,
            tab: source_tab,
            meta: source_meta,
        } = entry;
        // 上面两道 matches! 已锁定源/目标均为 Terminal;不可达分支保留
        // 防御,避免新增变体时静默丢 tab。
        let WorkspaceTab::Terminal {
            panes: source_panes,
            tree: source_tree,
            focused: source_focused,
            zoomed: source_zoomed,
        } = source_tab
        else {
            unreachable!("dock source must be a Terminal tab");
        };

        // 摘掉源 tab 后活跃下标左移一格;源在活跃之后则不变。
        let active_ix = if source_ix < original_active {
            original_active - 1
        } else {
            original_active
        };
        let merge = {
            let Some(active_entry) = self.tabs.get_mut(active_ix) else {
                self.restore_docked_source(
                    source_ix,
                    source_id,
                    source_panes,
                    source_tree,
                    source_focused,
                    source_zoomed,
                    source_meta,
                );
                return false;
            };
            let WorkspaceTab::Terminal { tree, .. } = &mut active_entry.tab else {
                // 上述 matches! 已锁定目标为 Terminal;不可达分支保留防御。
                unreachable!("dock target must be a Terminal tab");
            };
            match target.pane {
                Some(pane) => tree.dock_at_leaf(pane, source_tree, target.nav),
                None => {
                    let previous = std::mem::replace(tree, SplitTree::leaf(source_focused));
                    *tree = dock_tree(previous, source_tree, target.nav);
                    Ok(())
                }
            }
        };
        match merge {
            Ok(()) => {
                if let Some(entry) = self.tabs.get_mut(active_ix)
                    && let WorkspaceTab::Terminal {
                        panes,
                        focused,
                        zoomed,
                        ..
                    } = &mut entry.tab
                {
                    panes.extend(source_panes);
                    *focused = source_focused;
                    *zoomed = false;
                }
                self.active = active_ix;
                self.mark_structural_resize(active_ix, cx);
                self.focus_active(window, cx);
                self.sync_window_title(window, cx);
                cx.notify();
                true
            }
            Err(source_tree) => {
                // 目标消失:源树、源 panes、焦点、zoom 一样未动,原样放回。
                self.restore_docked_source(
                    source_ix,
                    source_id,
                    source_panes,
                    source_tree,
                    source_focused,
                    source_zoomed,
                    source_meta,
                );
                self.active = original_active;
                cx.notify();
                false
            }
        }
    }

    /// 停靠失败/异常路径的源 tab 原样复位(同下标、同 panes、同树、同焦点)。
    #[allow(clippy::too_many_arguments)] // 源 tab 字段逐项显式还原,不做中间结构体
    fn restore_docked_source(
        &mut self,
        source_ix: usize,
        source_id: TabId,
        source_panes: Vec<super::TerminalPane>,
        source_tree: SplitTree<PaneId>,
        source_focused: PaneId,
        source_zoomed: bool,
        source_meta: super::TabMeta,
    ) {
        self.tabs.insert(
            source_ix,
            super::TabEntry {
                id: source_id,
                tab: WorkspaceTab::Terminal {
                    panes: source_panes,
                    tree: source_tree,
                    focused: source_focused,
                    zoomed: source_zoomed,
                },
                meta: source_meta,
            },
        );
    }
}

#[cfg(test)]
mod tab_drag_tests {
    use super::*;

    /// 外圈与叶内四分位产生不同落点;内容区外返回 None。
    #[test]
    fn outer_rim_and_quadrant_drops_have_distinct_targets() {
        let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
        let panes = [
            (PaneId(1), Rect::new(0.0, 0.0, 599.0, 399.0)),
            (PaneId(2), Rect::new(601.0, 0.0, 599.0, 399.0)),
            (PaneId(3), Rect::new(0.0, 401.0, 599.0, 399.0)),
            (PaneId(4), Rect::new(601.0, 401.0, 599.0, 399.0)),
        ];
        assert_eq!(
            dock_target_at(area, &panes, 610.0, 600.0),
            Some(DockTarget {
                pane: Some(PaneId(4)),
                nav: SplitNav::Left
            })
        );
        assert_eq!(
            dock_target_at(area, &panes, 1195.0, 600.0),
            Some(DockTarget {
                pane: None,
                nav: SplitNav::Right
            })
        );
        assert_eq!(dock_target_at(area, &panes, 1250.0, 600.0), None);
    }

    /// 单 pane 时没有外圈:整片区域都归属该叶的四分位。
    #[test]
    fn a_single_pane_uses_quadrants_without_an_outer_rim() {
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        let panes = [(PaneId(1), area)];
        assert_eq!(
            dock_target_at(area, &panes, 2.0, 300.0),
            Some(DockTarget {
                pane: Some(PaneId(1)),
                nav: SplitNav::Left
            })
        );
    }

    /// 方向相对悬停对象自身,与窗口方向无关。
    #[test]
    fn direction_is_relative_to_the_hovered_pane() {
        let pane = Rect::new(600.0, 400.0, 600.0, 400.0);
        assert_eq!(nearest_edge(pane, 610.0, 600.0), SplitNav::Left);
        assert_eq!(nearest_edge(pane, 900.0, 410.0), SplitNav::Up);
        assert_eq!(nearest_edge(pane, 1190.0, 600.0), SplitNav::Right);
        assert_eq!(nearest_edge(pane, 900.0, 790.0), SplitNav::Down);
    }

    /// 阈值:单轴 4px 才算拖拽,3px 抖动仍是点击。
    #[test]
    fn threshold_crossing_is_single_axis_and_keeps_click_semantics() {
        assert!(!threshold_crossed(0.0, 0.0, 3.0, 3.0));
        assert!(threshold_crossed(0.0, 0.0, 4.0, 0.0));
        assert!(threshold_crossed(0.0, 0.0, 0.0, -4.0));
    }

    /// 预览几何必须与真实 `joined` 嫁接后的源侧矩形逐值一致(防两套
    /// 方向/先后映射漂移)。
    #[test]
    fn preview_geometry_matches_the_real_joined_layout() {
        let area = Rect::new(10.0, 20.0, 1201.0, 801.0);
        for nav in [
            SplitNav::Left,
            SplitNav::Right,
            SplitNav::Up,
            SplitNav::Down,
        ] {
            let expected = SplitTree::leaf(PaneId(0))
                .joined(SplitTree::leaf(PaneId(1)), nav)
                .layout(area, 1.0, 1.0, DIVIDER_GAP, false)
                .panes
                .into_iter()
                .find(|(pane, _)| *pane == PaneId(1))
                .map(|(_, rect)| rect)
                .expect("源侧必须出现在布局里");
            assert_eq!(preview_rect(area, nav), expected, "nav = {nav:?}");
        }
    }
}
