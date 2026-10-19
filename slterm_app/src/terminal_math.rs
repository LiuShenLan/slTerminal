//! 终端网格中公式定界符的原生 TeX 覆盖层。
//!
//! 终端仍是真值：本模块从不修改 cell、滚动、光标位置、选区或复制文本，
//! 只在最终 paint pass 替换可见定界符跨度。
//!
//! 本模块落有界扫描、[`LineProjection`]、[`CoverageMask`]
//! 与后端无关绘制计划数据面。

#[path = "terminal_math/scan.rs"]
mod scan;
#[allow(unused_imports)] // 消费点 = 终端快照同一锁窗口内调用。
pub(crate) use scan::scan_visible;
use scan::*;

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::size_of;
use std::sync::Arc;

use slterm_terminal::grid::Dimensions;
use slterm_terminal::index::{Column, Line, Point, Side};
use slterm_terminal::term::cell::Flags;
use slterm_terminal::term::{self, Term};
use vte::ansi::Rgb;

use crate::math::cache::{FormulaCacheKey, MathLayoutCache};
use crate::math::layout::{MathLayout, MathMetrics};
use crate::math::{DEFAULT_LIMITS, MIN_READABLE_MATH_PX, compile_formula};

const MAX_VISIBLE_FORMULAS: usize = 64;
const MAX_PERSISTED_FORMULAS: usize = 2_048;
const PERSISTED_FORMULA_BUDGET: usize = 1024 * 1024;
const MAX_HISTORY_FORMULA_ROWS: usize = 512;
/// **裸**定界符（Markdown 吃掉了反斜杠后残留的 `(…)` / `[…]`）闭合搜索的
/// 行预算。`MAX_VISIBLE_FORMULAS` 限制的是成功公式数，不是失败候选数；
/// 裸开头本身不携带任何意图——一个未闭合的 `(` 会扫到网格末尾，且每个 `(`
/// 在 paint 帧内各扫一遍。真实 display 块只占几行；更长的东西不是被折行的
/// 公式。
const BARE_PAREN_SEARCH_ROWS: usize = 8;
/// 多行 `[` 块采用同一预算。它合法容纳 `aligned` 环境，因此比行内括号宽松。
const BARE_BRACKET_SEARCH_ROWS: usize = 24;
/// 独立 display 块在跨过 TUI 段落空隙（见 [`TextGrid::find_closing`]）后，
/// 还能向下延伸多远闭合。跨空隙是猜测，因此不同于普通搜索，它不允许一直
/// 扫到网格底部。
const DISPLAY_BLOCK_GAP_SEARCH_ROWS: usize = BARE_BRACKET_SEARCH_ROWS;
/// 整帧可花在**裸**定界符闭合搜索上的 cell 数。
///
/// 仅靠上面的每候选行预算还不够：一个屏幕可以放几千个未闭合 `(`，
/// 8 行 × 200 列累计仍是每帧数百万次 cell 访问。这道闸让帧的最坏情况与
/// 网格成正比而不是平方级。预算耗尽只让剩余裸候选保留原文——每个自带
/// 意图的定界符（`$$`、`\[`、`\(`、`$`）都有 O(1) 前置过滤，从不在此计费。
const BARE_SEARCH_CELL_BUDGET: usize = 4 * 1024;
const FORMULA_INSET: f32 = 2.0;
/// 公式每一侧可借用的连续空行数。2 是 AI 回答里 `$$` 块常见的上下留白；
/// 再多会把公式明显推离它所属的文本。
const MAX_ABSORBED_BLANK_ROWS: usize = 2;
/// 借来的空行保留的细边（单位：行），保证墨迹永远不碰到再下一行。
const BLANK_ROW_MARGIN: f32 = 0.1;
/// [`apply_layout_hints`] 查看网格前的邻行状态：整行都视为占用，因此跳过
/// 扫描的 overlay 只拿到行间隙。
const NEIGHBOUR_UNKNOWN: Option<(usize, usize)> = Some((0, usize::MAX));
/// 邻行在公式正下方有文字时的外溢预算：自然行间隙加上等宽字形在 cell 内
/// 留下的细缝。取值保证 [`crate::math::MIN_SCRIPT_SCALE`] 下的行内分式——
/// 最常见的与正文共享一行的高件——仍完整渲染。再大就会压到邻行字形；
/// 这里曾经无条件放宽，结果公式盖到相邻文字上。
const BLEED_INTO_PROSE: f32 = 0.2;
/// display 数学应读作独立块。它可少用一点占用正文行的内部行距，留下可见
/// 边界，而无需 emitter 额外插入空终端行。
const DISPLAY_BLEED_INTO_PROSE: f32 = 0.18;
/// 公式高度超出预算的比例；超过后缩小。取值保证行内分式——最常出现在
/// 单行正文里的高件——保留终端字号：它的溢出落在行间隙里，剩余由 clip 裁掉。
const HEIGHT_OVERRUN_TOLERANCE: f32 = 0.12;

/// 覆盖层所需的视口几何（后端无关；从 GPUI 布局构造）。
///
/// 坐标系与终端元素一致：原点为内容矩形左上角，padding 为内容矩形相对
/// 元素边界的偏移。列数/行数由内容矩形与 cell 尺寸向下取整得出。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MathViewport {
    width: f32,
    height: f32,
    cell_width: f32,
    cell_height: f32,
    padding_x: f32,
    padding_y: f32,
}

impl MathViewport {
    pub(crate) fn new(
        width: f32,
        height: f32,
        cell_width: f32,
        cell_height: f32,
        padding_x: f32,
        padding_y: f32,
    ) -> Self {
        Self {
            width,
            height,
            cell_width,
            cell_height,
            padding_x,
            padding_y,
        }
    }

    pub(crate) fn cell_width(&self) -> f32 {
        self.cell_width
    }

    pub(crate) fn cell_height(&self) -> f32 {
        self.cell_height
    }

    pub(crate) fn padding_x(&self) -> f32 {
        self.padding_x
    }

    pub(crate) fn padding_y(&self) -> f32 {
        self.padding_y
    }

    pub(crate) fn columns(&self) -> usize {
        ((self.width - self.padding_x * 2.0) / self.cell_width.max(1.0))
            .floor()
            .max(1.0) as usize
    }

    pub(crate) fn screen_lines(&self) -> usize {
        ((self.height - self.padding_y * 2.0) / self.cell_height.max(1.0))
            .floor()
            .max(1.0) as usize
    }
}

/// 覆盖层需要的最小源格信息。在终端快照的同一锁窗口内把
/// `RenderSnapshot` 映射成本类型；`fg` 已按调色板解析，`bg_alpha` 为背景
/// 不透明度（0 = 默认背景，>0 = ANSI/TUI 背景）。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OverlayCell {
    pub(crate) character: char,
    pub(crate) point: Point<usize>,
    pub(crate) fg: Rgb,
    pub(crate) bg_alpha: f32,
    pub(crate) flags: Flags,
}

/// 每个 pane 的状态。克隆时刻意丢弃布局数据：克隆出的 UI 元数据可能比
/// 渲染器/字体缩放活得更久，而布局重建便宜，之后仍受 `MathLayoutCache` 约束。
#[derive(Default)]
pub(crate) struct TerminalMathState {
    cache: MathLayoutCache,
    layout_resolver: Option<LayoutResolver>,
    last_scan: Option<(TextGrid, GridScanResult)>,
    projection: LineProjection,
    formulas: BTreeMap<FormulaAnchor, PersistedFormula>,
    persisted_bytes: usize,
    max_formula_rows: usize,
    columns: Option<usize>,
    last_scrolled_out: Option<usize>,
    pending_display: Option<PendingDisplayFormula>,
}

impl Clone for TerminalMathState {
    fn clone(&self) -> Self {
        // pane 克隆可能挂到另一个 PTY。绝对网格锚点必须从新 pane 重建，
        // 不能跨会话携带。
        Self::default()
    }
}

impl fmt::Debug for TerminalMathState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalMathState")
            .field("projected_spans", &self.projection.spans.len())
            .field("persisted_formulas", &self.formulas.len())
            .field("persisted_bytes", &self.persisted_bytes)
            .finish()
    }
}

pub(crate) type LayoutResolver =
    Arc<dyn Fn(Arc<str>, f32, f32, bool) -> Option<Arc<MathLayout>> + Send + Sync>;

impl TerminalMathState {
    pub(crate) fn set_layout_resolver(&mut self, resolver: LayoutResolver) {
        self.layout_resolver = Some(resolver);
    }

    pub(crate) fn update_projection(
        &mut self,
        overlays: &[FormulaOverlay],
        prepared: &[Option<PreparedFormula>],
        reflow_inline: bool,
    ) {
        let _ = self.update_projection_with_survivors(overlays, prepared, reflow_inline);
    }

    pub(crate) fn update_projection_with_survivors(
        &mut self,
        overlays: &[FormulaOverlay],
        prepared: &[Option<PreparedFormula>],
        reflow_inline: bool,
    ) -> Vec<bool> {
        if reflow_inline {
            self.projection.rebuild(overlays, prepared)
        } else {
            self.projection.spans.clear();
            vec![true; overlays.len()]
        }
    }

    pub(crate) fn project_cell(&self, point: Point<usize>, columns: usize) -> Option<Point<usize>> {
        self.projection.project_cell(point, columns)
    }

    pub(crate) fn project_formula_background(
        &self,
        point: Point<usize>,
        columns: usize,
    ) -> Option<Point<usize>> {
        self.projection.project_formula_background(point, columns)
    }

    /// 冻结当前帧的稀疏投影给锁外渲染使用。GPUI 的 term 快照与数学扫描在
    /// 同一次锁内完成，后续背景/字形绘制不能再回头读取可能已经变化的状态。
    pub(crate) fn projection_snapshot(&self) -> LineProjection {
        self.projection.clone()
    }

    /// 把视觉鼠标 cell 还原为不可变终端网格 cell。公式跨度是原子：左半/右半
    /// 选中的是相应源边界，而不是发明 TeX 语法内部的光标位置。
    ///
    /// `viewport_origin` 是渲染器视口顶行：投影跨度住在渲染视口坐标里，
    /// resize 提交待定时可能相对网格被裁剪。
    pub(crate) fn source_point(
        &self,
        point: Point,
        side: Side,
        viewport_origin: Line,
    ) -> (Point, Side) {
        let Some(viewport_point) = term::point_to_viewport_from(viewport_origin, point) else {
            return (point, side);
        };
        let (source, source_side) = self.projection.source_from_visual(viewport_point, side);
        (
            term::viewport_to_point_from(viewport_origin, source),
            source_side,
        )
    }

    fn synchronize_grid(&mut self, grid: &TextGrid) {
        let columns_changed = self.columns.is_some_and(|columns| columns != grid.columns);
        let absolute_epoch_changed = self
            .last_scrolled_out
            .is_some_and(|floor| grid.scrolled_out < floor);
        if columns_changed || absolute_epoch_changed {
            // 宽度变化会重排历史行，绝对行号回退则代表网格生命周期已重置；
            // 两种情况下沿用旧锚点都会把公式覆盖到无关文本上。
            self.clear_formulas();
        }
        self.columns = Some(grid.columns);

        if self.last_scrolled_out != Some(grid.scrolled_out) {
            self.prune_before(grid.scrolled_out);
            self.last_scrolled_out = Some(grid.scrolled_out);
        }
    }

    /// 记住当前视口里的完整公式，并跟踪一条流式 display 公式：它的开头
    /// 可能在闭合定界符到达前先滚出视口。
    fn scan_visible_grid(
        &mut self,
        grid: &TextGrid,
        active_edit_rows: Option<&std::ops::RangeInclusive<usize>>,
    ) -> Option<FormulaAnchor> {
        let mut completed_pending = false;
        let scan = match &self.last_scan {
            Some((previous, scan)) if previous == grid => scan.clone(),
            _ => {
                let scan = scan_grid_result(grid);
                self.last_scan = Some((grid.clone(), scan.clone()));
                scan
            }
        };
        for overlay in scan.overlays {
            // 光标所在的逻辑行仍由 CLI 编辑器拥有。必须在持久化之前排除
            // 整段换行链，否则公式会先进入缓存，下一帧又覆盖输入。
            if active_edit_rows.is_some_and(|rows| overlay.intersects_rows(rows)) {
                continue;
            }
            let anchor = overlay_anchor(grid, &overlay);
            completed_pending |= self
                .pending_display
                .is_some_and(|pending| Some(pending.anchor) == anchor && overlay.display);
            self.remember(grid, &overlay);
        }
        if completed_pending {
            self.pending_display = None;
        }

        let (position, kind) = scan.unmatched_display?;
        if active_edit_rows.is_some_and(|rows| rows.contains(&position.row)) {
            return None;
        }
        let current = FormulaAnchor {
            row: grid.absolute_top.saturating_add(position.row),
            column: position.column,
        };

        match self.pending_display {
            None => {
                self.pending_display = Some(PendingDisplayFormula {
                    anchor: current,
                    kind,
                    attempted_at: None,
                });
                None
            }
            // 同一位置的孤立定界符稳定存在：它既可能是仍在流式输出的开头，
            // 也可能是"公式被误删后只剩下的闭合"。对这个位置回看一次历史，
            // 尝试把完整公式重新组装出来。
            Some(pending) if pending.anchor == current && pending.kind == kind => {
                if pending.attempted_at == Some(current) {
                    None
                } else {
                    self.pending_display = Some(PendingDisplayFormula {
                        attempted_at: Some(current),
                        ..pending
                    });
                    Some(current)
                }
            }
            Some(pending) if pending.kind == kind && pending.anchor < current => {
                if pending.attempted_at == Some(current) {
                    None
                } else {
                    self.pending_display = Some(PendingDisplayFormula {
                        attempted_at: Some(current),
                        ..pending
                    });
                    Some(pending.anchor)
                }
            }
            Some(_) => {
                self.pending_display = Some(PendingDisplayFormula {
                    anchor: current,
                    kind,
                    attempted_at: None,
                });
                None
            }
        }
    }

    fn complete_pending_from_history(&mut self, history: &TextGrid) -> bool {
        let Some(pending) = self.pending_display.take() else {
            return false;
        };

        for overlay in scan_grid_result(history).overlays {
            if !overlay.display {
                continue;
            }
            // 原场景：pending 记录的是已滚入历史的开头。孤立闭合场景：
            // pending 记录的是视口里那个落单的 `$$`，此时要求组装出的公式
            // 恰好以它收尾，才能证明它确实是被误删公式的闭合定界符。
            let opens_at_pending = overlay_anchor(history, &overlay) == Some(pending.anchor);
            let closes_at_pending = overlay.spans.last().is_some_and(|span| {
                usize::try_from(span.row)
                    .ok()
                    .and_then(|row| history.absolute_top.checked_add(row))
                    == Some(pending.anchor.row)
                    && span.end >= 2
                    && span.end - 2 == pending.anchor.column
            });
            if opens_at_pending || closes_at_pending {
                self.remember(history, &overlay);
                // 放回带 attempted 标记的 pending：孤立闭合在网格里仍会被
                // 扫成 unmatched（persisted 覆盖对扫描不可见），清空会让它
                // 每两帧重建并再次触发历史回看。
                self.pending_display = Some(pending);
                return true;
            }
        }
        // 失败时放回（保留单次尝试标记），否则同一个孤立定界符每帧都会
        // 触发一轮历史扫描。
        self.pending_display = Some(pending);
        false
    }

    fn remember(&mut self, grid: &TextGrid, overlay: &FormulaOverlay) {
        let Some(formula) = PersistedFormula::from_overlay(grid, overlay) else {
            return;
        };
        let anchor = formula.anchor;
        if self
            .formulas
            .get(&anchor)
            .is_some_and(|existing| existing.same_content(&formula))
        {
            return;
        }
        let replaced: Vec<_> = self
            .formulas
            .iter()
            .filter_map(|(&existing_anchor, existing)| {
                existing.overlaps(&formula).then_some(existing_anchor)
            })
            .collect();
        for existing_anchor in replaced {
            self.remove(existing_anchor);
        }
        if formula.charge > PERSISTED_FORMULA_BUDGET {
            return;
        }

        self.persisted_bytes = self.persisted_bytes.saturating_add(formula.charge);
        self.max_formula_rows = self.max_formula_rows.max(formula.row_count());
        self.formulas.insert(anchor, formula);
        while self.formulas.len() > MAX_PERSISTED_FORMULAS
            || self.persisted_bytes > PERSISTED_FORMULA_BUDGET
        {
            let Some(oldest) = self.formulas.first_key_value().map(|(&anchor, _)| anchor) else {
                break;
            };
            self.remove(oldest);
        }
    }

    fn visible_overlays(&mut self, grid: &TextGrid) -> Vec<FormulaOverlay> {
        if grid.rows.is_empty() || self.formulas.is_empty() {
            return Vec::new();
        }

        let viewport_bottom = grid.absolute_top.saturating_add(grid.rows.len() - 1);
        let search_top = grid
            .absolute_top
            .saturating_sub(self.max_formula_rows.saturating_sub(1));
        let lower = FormulaAnchor {
            row: search_top,
            column: 0,
        };
        let upper = FormulaAnchor {
            row: viewport_bottom,
            column: usize::MAX,
        };
        let candidates: Vec<_> = self
            .formulas
            .range(lower..=upper)
            .filter(|(_, formula)| formula.intersects(grid.absolute_top, viewport_bottom))
            .map(|(&anchor, formula)| (anchor, formula.matches_visible_rows(grid)))
            .collect();

        let mut overlays = Vec::with_capacity(candidates.len().min(MAX_VISIBLE_FORMULAS));
        for (anchor, valid) in candidates {
            if !valid {
                self.remove(anchor);
                continue;
            }
            if overlays.len() < MAX_VISIBLE_FORMULAS
                && let Some(formula) = self.formulas.get(&anchor)
            {
                overlays.push(formula.to_overlay(grid.absolute_top));
            }
        }
        overlays
    }

    fn prune_before(&mut self, absolute_floor: usize) {
        if self
            .pending_display
            .is_some_and(|pending| pending.anchor.row < absolute_floor)
        {
            self.pending_display = None;
        }
        loop {
            let stale = self
                .formulas
                .first_key_value()
                .filter(|(_, formula)| formula.last_row() < absolute_floor)
                .map(|(&anchor, _)| anchor);
            match stale {
                Some(anchor) => self.remove(anchor),
                None => break,
            }
        }
    }

    fn remove(&mut self, anchor: FormulaAnchor) {
        let Some(removed) = self.formulas.remove(&anchor) else {
            return;
        };
        self.persisted_bytes = self.persisted_bytes.saturating_sub(removed.charge);
        if removed.row_count() == self.max_formula_rows {
            self.max_formula_rows = self
                .formulas
                .values()
                .map(PersistedFormula::row_count)
                .max()
                .unwrap_or(0);
        }
    }

    fn clear_formulas(&mut self) {
        self.formulas.clear();
        self.persisted_bytes = 0;
        self.max_formula_rows = 0;
        self.pending_display = None;
    }

    fn layout(
        &mut self,
        formula_id: u64,
        source: &Arc<str>,
        pixel_size: f32,
        pixels_per_point: f32,
        display: bool,
    ) -> Result<Arc<MathLayout>, crate::math::MathError> {
        if let Some(resolve) = &self.layout_resolver {
            return resolve(source.clone(), pixel_size, pixels_per_point, display)
                .ok_or_else(|| crate::math::MathError::new(crate::math::MathErrorKind::Parse, 0));
        }
        let key = FormulaCacheKey::new(formula_id, pixel_size, pixels_per_point, display);
        self.cache
            .get_or_insert_with(key, || {
                compile_formula(
                    source,
                    display,
                    pixel_size,
                    pixels_per_point,
                    DEFAULT_LIMITS,
                )
            })
            .map(|layout| Arc::new(layout.clone()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FormulaAnchor {
    row: usize,
    column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayDelimiterKind {
    Dollars,
    Brackets,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingDisplayFormula {
    anchor: FormulaAnchor,
    kind: DisplayDelimiterKind,
    /// 已经触发过一次历史重建的 unmatched 定界符位置。每个位置只尝试一次：
    /// 否则一个始终 unmatched 的定界符（例如它的公式早已持久化）会每帧重扫历史。
    attempted_at: Option<FormulaAnchor>,
}

fn overlay_anchor(grid: &TextGrid, overlay: &FormulaOverlay) -> Option<FormulaAnchor> {
    let first = overlay.spans.first()?;
    let row = usize::try_from(first.row).ok()?;
    Some(FormulaAnchor {
        row: grid.absolute_top.checked_add(row)?,
        column: first.start,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PersistedRowSpan {
    row: usize,
    start: usize,
    end: usize,
    fingerprint: u64,
    include_wrap: bool,
}

#[derive(Clone, Debug)]
struct PersistedFormula {
    anchor: FormulaAnchor,
    source: Arc<str>,
    display: bool,
    formula_id: u64,
    spans: Box<[PersistedRowSpan]>,
    charge: usize,
}

impl PersistedFormula {
    fn from_overlay(grid: &TextGrid, overlay: &FormulaOverlay) -> Option<Self> {
        let mut spans = Vec::with_capacity(overlay.spans.len());
        for (index, span) in overlay.spans.iter().enumerate() {
            let row = usize::try_from(span.row).ok()?;
            let include_wrap = index + 1 < overlay.spans.len();
            let fingerprint = grid.span_fingerprint(row, span.start, span.end, include_wrap)?;
            spans.push(PersistedRowSpan {
                row: grid.absolute_top.checked_add(row)?,
                start: span.start,
                end: span.end,
                fingerprint,
                include_wrap,
            });
        }
        let first = spans.first()?;
        let anchor = FormulaAnchor {
            row: first.row,
            column: first.start,
        };
        let charge = size_of::<Self>()
            .saturating_add(overlay.source.len())
            .saturating_add(
                spans
                    .capacity()
                    .saturating_mul(size_of::<PersistedRowSpan>()),
            );
        Some(Self {
            anchor,
            source: Arc::clone(&overlay.source),
            display: overlay.display,
            formula_id: overlay.formula_id,
            spans: spans.into_boxed_slice(),
            charge,
        })
    }

    fn row_count(&self) -> usize {
        self.last_row()
            .saturating_sub(self.anchor.row)
            .saturating_add(1)
    }

    fn same_content(&self, other: &Self) -> bool {
        self.display == other.display
            && self.formula_id == other.formula_id
            && self.source == other.source
            && self.spans == other.spans
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.spans.iter().any(|left| {
            other.spans.iter().any(|right| {
                left.row == right.row && left.start < right.end && right.start < left.end
            })
        })
    }

    fn last_row(&self) -> usize {
        self.spans.last().map_or(self.anchor.row, |span| span.row)
    }

    fn intersects(&self, top: usize, bottom: usize) -> bool {
        self.anchor.row <= bottom && self.last_row() >= top
    }

    fn matches_visible_rows(&self, grid: &TextGrid) -> bool {
        let mut compared = false;
        for span in &self.spans {
            let Some(row) = span.row.checked_sub(grid.absolute_top) else {
                continue;
            };
            if row >= grid.rows.len() {
                continue;
            }
            if grid.span_fingerprint(row, span.start, span.end, span.include_wrap)
                == Some(span.fingerprint)
            {
                compared = true;
                continue;
            }
            // 整段被清空是 TUI 重绘的中间帧（先清行再重画）：跳过比较，
            // 让公式在这一两帧里继续渲染，避免输入期间不停闪回原文。
            // 只要所有可比行都空白（真清屏），compared 保持 false 仍会淘汰。
            if grid.span_is_blank(row, span.start, span.end) {
                continue;
            }
            return false;
        }
        compared
    }

    fn to_overlay(&self, absolute_top: usize) -> FormulaOverlay {
        let spans = self
            .spans
            .iter()
            .map(|span| RowSpan {
                row: relative_row(span.row, absolute_top),
                start: span.start,
                end: span.end,
            })
            .collect();
        FormulaOverlay {
            source: Arc::clone(&self.source),
            display: self.display,
            formula_id: self.formula_id,
            spans,
            foreground: Rgb::default(),
            fallback: Vec::new(),
            neighbours_above: [NEIGHBOUR_UNKNOWN; MAX_ABSORBED_BLANK_ROWS],
            neighbours_below: [NEIGHBOUR_UNKNOWN; MAX_ABSORBED_BLANK_ROWS],
            formula_neighbours_above: [false; MAX_ABSORBED_BLANK_ROWS],
            formula_neighbours_below: [false; MAX_ABSORBED_BLANK_ROWS],
            widen_right_to: None,
        }
    }
}

fn relative_row(row: usize, absolute_top: usize) -> i32 {
    if row >= absolute_top {
        i32::try_from(row - absolute_top).unwrap_or(i32::MAX)
    } else {
        -i32::try_from(absolute_top - row).unwrap_or(i32::MAX)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum DelimiterKind {
    DollarInline,
    Parenthesized,
    DollarDisplay,
    BracketDisplay,
    /// Markdown 去掉反斜杠后的 `\[ … \]`：AI CLI 的 markdown 渲染器吃掉
    /// 反斜杠后留下的裸 `[` / `]`。
    BareBracketDisplay,
    /// Markdown 去掉反斜杠后的 `\( … \)`：TeX 内容外的裸括号。
    BareParenInline,
}

impl DelimiterKind {
    fn is_display(self) -> bool {
        matches!(
            self,
            Self::DollarDisplay | Self::BracketDisplay | Self::BareBracketDisplay
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GridPosition {
    row: usize,
    column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowSpan {
    row: i32,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaOverlay {
    source: Arc<str>,
    display: bool,
    formula_id: u64,
    spans: Vec<RowSpan>,
    foreground: Rgb,
    fallback: Vec<OverlayCell>,
    /// 每个邻行的占用列区间，最近优先，每侧上限 [`MAX_ABSORBED_BLANK_ROWS`]。
    /// `None` 表示该行永远不会挡路（空白，或视口之外——clip 已拦住墨迹）。
    /// 占用行是否真的阻塞取决于公式墨迹落在哪些列——居中公式上方的正文
    /// 通常远未延伸过来——因此裁决属于 [`prepare_overlays`]（它知道渲染宽度）。
    neighbours_above: [Option<(usize, usize)>; MAX_ABSORBED_BLANK_ROWS],
    neighbours_below: [Option<(usize, usize)>; MAX_ABSORBED_BLANK_ROWS],
    /// 对应占用邻行是否由另一条公式绘制。公式行连正文抗锯齿的细缝都不能
    /// 借出：相邻两块数学 clip 必须在行边界相接，不能重叠。
    formula_neighbours_above: [bool; MAX_ABSORBED_BLANK_ROWS],
    formula_neighbours_below: [bool; MAX_ABSORBED_BLANK_ROWS],
    /// display 公式的源码之后整行无内容时，可用的右边界（网格列）。
    widen_right_to: Option<usize>,
}

impl FormulaOverlay {
    /// 归一化后的 TeX 源（绘制/缓存键用）。
    pub(crate) fn source_arc(&self) -> Arc<str> {
        Arc::clone(&self.source)
    }

    /// 源格跨度 `(行, 起, 止)`。在位图预检之后要按存活的公式重建
    /// 覆盖掩码，因此这里把跨度交出去，而不是让它去猜 `spans` 的内部表示。
    pub(crate) fn covered_spans(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.spans.iter().filter_map(|span| {
            usize::try_from(span.row)
                .ok()
                .map(|row| (row, span.start, span.end))
        })
    }

    fn contains(&self, point: Point<usize>) -> bool {
        self.spans.iter().any(|span| {
            usize::try_from(span.row) == Ok(point.line)
                && (span.start..span.end).contains(&point.column.0)
        })
    }

    fn intersects_rows(&self, rows: &std::ops::RangeInclusive<usize>) -> bool {
        self.spans
            .iter()
            .filter_map(|span| usize::try_from(span.row).ok())
            .any(|row| rows.contains(&row))
    }

    fn bounds(&self, size: &MathViewport) -> Option<FormulaBounds> {
        let first = self.spans.first()?;
        let last = self.spans.last()?;
        let left_col = self.spans.iter().map(|span| span.start).min()?;
        let right_col = self.spans.iter().map(|span| span.end).max()?;
        let left = size.padding_x() + left_col as f32 * size.cell_width();
        let right = size.padding_x() + right_col as f32 * size.cell_width();
        let top = size.padding_y() + first.row as f32 * size.cell_height();
        let bottom = size.padding_y() + (last.row + 1) as f32 * size.cell_height();
        (right > left && bottom > top).then_some(FormulaBounds {
            left,
            top,
            right,
            bottom,
        })
    }

    /// 给定公式墨迹允许伸入的邻行数，`bounds()` 之外每侧的垂直余量（像素）。
    /// 借出的行保留细边，抗锯齿永不触碰再下一行；占用行只借自然行间隙——
    /// 终端字形墨迹并不填满整个 cell——因为再大就会压到邻行字形。fit 与
    /// clip 消费同一组数字，这是"公式墨迹不落到相邻文字上"的不变量：
    /// fit 吸收不掉的部分由 clip 裁掉。
    fn vertical_bleed(&self, size: &MathViewport, absorbed: (usize, usize)) -> (f32, f32) {
        let budget = |rows: usize, formula_neighbour: bool| {
            size.cell_height()
                * if rows == 0 {
                    if formula_neighbour {
                        0.0
                    } else if self.display {
                        DISPLAY_BLEED_INTO_PROSE
                    } else {
                        BLEED_INTO_PROSE
                    }
                } else {
                    rows as f32 - BLANK_ROW_MARGIN
                }
        };
        (
            budget(absorbed.0, self.formula_neighbours_above[0]),
            budget(absorbed.1, self.formula_neighbours_below[0]),
        )
    }

    /// 每侧墨迹可伸入多少个邻行：最近若干行自身墨迹不与 `columns` 重叠的
    /// 连续段。行内数学对称扩展——它与正文共享一行，单侧扩展会把基线抬离
    /// 那段文字。
    fn absorbable_rows(&self, columns: (usize, usize)) -> (usize, usize) {
        let free = |rows: &[Option<(usize, usize)>; MAX_ABSORBED_BLANK_ROWS]| {
            rows.iter()
                .take_while(|row| {
                    row.is_none_or(|(start, end)| end <= columns.0 || start >= columns.1)
                })
                .count()
        };
        let (above, below) = (free(&self.neighbours_above), free(&self.neighbours_below));
        if self.display {
            (above, below)
        } else {
            (above.min(below), above.min(below))
        }
    }
}

#[derive(Clone, Copy)]
struct FormulaBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl FormulaBounds {
    fn height(self) -> f32 {
        self.bottom - self.top
    }
}

/// 通过布局预检的公式：网格 pass 省略其源格，[`plan_overlay_draw`] 的消费者
/// 在它们的位置绘制编译后的布局。所有几何决策都在这里完成，因此 fit 与
/// clip 永远不会对公式拿到多少空间产生分歧。
#[derive(Clone, Copy)]
pub(crate) struct PreparedFormula {
    fitted_pixel_size: f32,
    /// 实际使用的排版样式。无法塞进一行的块级公式先退回 text 样式，之后
    /// 才放弃字号：读者判断公式大小看的是它相对正文的字母大小，而不是
    /// `\sum` 把上下限放在上面还是旁边。
    display_style: bool,
    /// `bounds()` 之外每侧的空间，以及布局盒右边缘：`plan_overlay_draw`
    /// 直接消费这些值而不是重算。
    bleed_top: f32,
    bleed_bottom: f32,
    box_right: f32,
    centered: bool,
    /// 单行行内公式量化后的视觉宽度。display 公式保留源网格盒，不参与。
    compact_cells: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct ProjectionSpan {
    source_index: usize,
    row: usize,
    source_start: usize,
    source_end: usize,
    visual_cells: usize,
    shift_before: isize,
    shift_after: isize,
}

/// 当前视口内紧凑行内公式的稀疏源→屏映射。一个条目代表一条公式；
/// 不构建逐 cell 对象。
#[derive(Clone, Debug, Default)]
pub(crate) struct LineProjection {
    spans: Vec<ProjectionSpan>,
}

impl LineProjection {
    /// 返回与 `overlays` 对齐的存活表。流式输出可能短暂产生重叠公式，调用方
    /// 必须和投影采用同一裁决，否则位图、coverage 与后续文字会落在三套坐标上。
    fn rebuild(
        &mut self,
        overlays: &[FormulaOverlay],
        prepared: &[Option<PreparedFormula>],
    ) -> Vec<bool> {
        self.spans.clear();
        let mut retained = vec![true; overlays.len()];
        // `reserve` 在 `clear` 之后于稳定帧复用既有分配，增长上限为可见公式数。
        self.spans.reserve(overlays.len());
        for (source_index, (overlay, prepared)) in overlays.iter().zip(prepared).enumerate() {
            let Some(visual_cells) = prepared.and_then(|prepared| prepared.compact_cells) else {
                continue;
            };
            let [span] = overlay.spans.as_slice() else {
                continue;
            };
            let Ok(row) = usize::try_from(span.row) else {
                continue;
            };
            if span.end <= span.start {
                continue;
            }
            self.spans.push(ProjectionSpan {
                source_index,
                row,
                source_start: span.start,
                source_end: span.end,
                visual_cells,
                shift_before: 0,
                shift_after: 0,
            });
        }
        self.spans.sort_unstable_by(|left, right| {
            (left.row, left.source_start)
                .cmp(&(right.row, right.source_start))
                .then_with(|| right.source_end.cmp(&left.source_end))
        });

        // 流式 TUI 可能在外层定界符到达前短暂暴露内层公式。持久化通常会替换
        // 这个陈旧条目，但投影仍保持防御：优先保留更宽的较早跨度，丢弃任何
        // 重叠尾部，而不是在帧内 panic。
        let mut retained_row = None;
        let mut retained_end = 0usize;
        self.spans.retain(|span| {
            if retained_row != Some(span.row) {
                retained_row = Some(span.row);
                retained_end = 0;
            }
            if span.source_start < retained_end {
                retained[span.source_index] = false;
                return false;
            }
            retained_end = span.source_end;
            true
        });

        let mut row = None;
        let mut shift = 0isize;
        for span in &mut self.spans {
            if row != Some(span.row) {
                row = Some(span.row);
                shift = 0;
            }
            span.shift_before = shift;
            let source_cells = span.source_end - span.source_start;
            shift = shift.saturating_add(span.visual_cells as isize - source_cells as isize);
            span.shift_after = shift;
        }
        retained
    }

    #[cfg(test)]
    fn build(overlays: &[FormulaOverlay], prepared: &[Option<PreparedFormula>]) -> Self {
        let mut projection = Self::default();
        let _ = projection.rebuild(overlays, prepared);
        projection
    }

    fn row_spans(&self, row: usize) -> &[ProjectionSpan] {
        let start = self.spans.partition_point(|span| span.row < row);
        let end = start + self.spans[start..].partition_point(|span| span.row == row);
        &self.spans[start..end]
    }

    fn shift_before(&self, row: usize, source_column: usize) -> isize {
        self.row_spans(row)
            .iter()
            .take_while(|span| span.source_end <= source_column)
            .last()
            .map_or(0, |span| span.shift_after)
    }

    pub(crate) fn project_cell(&self, point: Point<usize>, columns: usize) -> Option<Point<usize>> {
        let spans = self.row_spans(point.line);
        let preceding = spans.partition_point(|span| span.source_start <= point.column.0);
        let shift = match preceding.checked_sub(1).and_then(|index| spans.get(index)) {
            Some(span) if point.column.0 < span.source_end => return None,
            Some(span) => span.shift_after,
            None => 0,
        };
        let column = apply_shift(point.column.0, shift);
        (column < columns).then(|| Point::new(point.line, Column(column)))
    }

    /// 将已被清空的源格映射到紧凑视觉盒，让解析后的 ANSI 背景仍留在行内
    /// 公式之后。TeX 比渲染公式宽时丢弃多余源格。
    pub(crate) fn project_formula_background(
        &self,
        point: Point<usize>,
        columns: usize,
    ) -> Option<Point<usize>> {
        let span = self
            .row_spans(point.line)
            .iter()
            .find(|span| (span.source_start..span.source_end).contains(&point.column.0));
        let Some(span) = span else {
            return (point.column.0 < columns).then_some(point);
        };
        let relative = point.column.0 - span.source_start;
        if relative >= span.visual_cells {
            return None;
        }
        let visual_start = apply_shift(span.source_start, span.shift_before);
        let column = visual_start.saturating_add(relative);
        (column < columns).then(|| Point::new(point.line, Column(column)))
    }

    fn source_from_visual(&self, point: Point<usize>, side: Side) -> (Point<usize>, Side) {
        let spans = self.row_spans(point.line);
        let visual_column = point.column.0;
        let mut shift = 0isize;

        for span in spans {
            let visual_start = apply_shift(span.source_start, span.shift_before);
            let visual_end = visual_start.saturating_add(span.visual_cells);
            if visual_column < visual_start {
                break;
            }
            if visual_column < visual_end {
                let relative_twice = (visual_column - visual_start)
                    .saturating_mul(2)
                    .saturating_add(usize::from(side == Side::Right));
                return if relative_twice < span.visual_cells {
                    (
                        Point::new(point.line, Column(span.source_start)),
                        Side::Left,
                    )
                } else {
                    (
                        Point::new(point.line, Column(span.source_end.saturating_sub(1))),
                        Side::Right,
                    )
                };
            }
            shift = span.shift_after;
        }

        let source_column = apply_shift(visual_column, shift.saturating_neg());
        (Point::new(point.line, Column(source_column)), side)
    }
}

fn apply_shift(column: usize, shift: isize) -> usize {
    if shift >= 0 {
        column.saturating_add(shift as usize)
    } else {
        column.saturating_sub(shift.unsigned_abs())
    }
}

fn quantized_visual_cells(width: f32, cell_width: f32, remaining_columns: usize) -> usize {
    let cells = ((width + FORMULA_INSET * 2.0) / cell_width.max(1.0))
        .ceil()
        .max(1.0) as usize;
    cells.min(remaining_columns.max(1))
}

/// `metrics` 在给定空间内可用的最大统一缩放，上限 1。
///
/// 高度允许先超出 [`HEIGHT_OVERRUN_TOLERANCE`] 再让字号让步。同类公式以
/// 同一字号阅读才是用户真正注意到的；几个百分点高度只损失最外圈抗锯齿，
/// 而 clip 在预算处会裁掉。宽度没有这种容差——那里溢出是右边缘的真实
/// 符号，裁掉就丢内容。公式与公式的边界是硬裁剪：不同于正文间隙，它不能
/// 吸收名义溢出而不切掉相邻上标。
fn fit_ratio(
    metrics: &MathMetrics,
    available_width: f32,
    available_height: f32,
    height_overrun_tolerance: f32,
) -> f32 {
    let total_height = (metrics.height + metrics.depth).max(1.0);
    let height_fit = available_height / total_height;
    let height_fit = if height_fit >= 1.0 - height_overrun_tolerance {
        1.0
    } else {
        height_fit
    };
    (available_width / metrics.width.max(1.0))
        .min(height_fit)
        .min(1.0)
}

/// 宽度为 `width` 的布局墨迹会占用的网格列，遵循 [`plan_overlay_draw`] 的
/// 同一放置规则。按终端字号测量，这是公式可能达到的最宽状态：缩小只会
/// 把墨迹进一步收进该范围，因此判为无冲突的邻行仍然无冲突。
fn ink_columns(
    overlay: &FormulaOverlay,
    size: &MathViewport,
    bounds: FormulaBounds,
    box_right: f32,
    width: f32,
) -> (usize, usize) {
    let box_width = box_right - bounds.left;
    let left = if overlay.display || width >= box_width * 0.75 {
        bounds.left + (box_width - width) / 2.0
    } else {
        bounds.left + FORMULA_INSET
    }
    .max(bounds.left);
    let column = |x: f32| ((x - size.padding_x()) / size.cell_width().max(1.0)).max(0.0);
    (
        column(left).floor() as usize,
        column(left + width).ceil() as usize,
    )
}

/// 在 cell pass 之前预编译所有 overlay，让渲染器知道跳过哪些源格。返回与
/// overlay 一一对应的条目；`None` 保留原始源码可见（布局失败，或拟合字号
/// 已不可读）。
pub(crate) fn prepare_overlays(
    state: &mut TerminalMathState,
    overlays: &[FormulaOverlay],
    size: &MathViewport,
    font_pixel_size: f32,
    pixels_per_point: f32,
) -> Vec<Option<PreparedFormula>> {
    overlays
        .iter()
        .map(|overlay| {
            let bounds = overlay.bounds(size)?;
            let viewport_right = size.padding_x() + size.columns() as f32 * size.cell_width();
            // 两种候选样式都先按终端字号测量：其中最宽者决定墨迹可能落进
            // 哪些列，只有那些列里有文字的邻行才算挡路。居中公式上方的正文
            // 通常远未延伸到那里，于是夹在两条列表项之间的 `$$` 块仍可使用
            // 它们的垂直空间。
            let styles: &[bool] = if overlay.display {
                &[true, false]
            } else {
                &[false]
            };
            let mut widest = 0.0f32;
            for &display_style in styles {
                let metrics = state
                    .layout(
                        overlay.formula_id,
                        &overlay.source,
                        font_pixel_size,
                        pixels_per_point,
                        display_style,
                    )
                    .ok()?
                    .metrics;
                widest = widest.max(metrics.width);
            }
            let compact_inline = !overlay.display && overlay.spans.len() == 1;
            let source_start = overlay.spans.first()?.start;
            let remaining_columns = size.columns().saturating_sub(source_start);
            let natural_cells =
                quantized_visual_cells(widest, size.cell_width(), remaining_columns);
            let box_right = if compact_inline {
                (bounds.left + natural_cells as f32 * size.cell_width()).min(viewport_right)
            } else if let Some(right_column) = overlay.widen_right_to {
                (size.padding_x() + right_column as f32 * size.cell_width()).max(bounds.right)
            } else {
                bounds.right
            };
            let available_width = (box_right - bounds.left - FORMULA_INSET * 2.0).max(1.0);
            let absorbed =
                overlay.absorbable_rows(ink_columns(overlay, size, bounds, box_right, widest));
            // 垂直空间是 bounds 加两侧预算。墨迹可伸入的行实际上把整行高度
            // 交给公式，于是 `$$` 块像周围正文一样以终端字号渲染；真正被
            // 文字夹住的公式只拿到行间隙。
            let (bleed_top, bleed_bottom) = overlay.vertical_bleed(size, absorbed);
            // 相邻公式共享硬边界，不能沿用正文行的 12% 容差，否则上标会先被裁掉。
            let formula_boundary = (absorbed.0 == 0 && overlay.formula_neighbours_above[0])
                || (absorbed.1 == 0 && overlay.formula_neighbours_below[0]);
            let height_overrun_tolerance = if formula_boundary {
                0.0
            } else {
                HEIGHT_OVERRUN_TOLERANCE
            };
            // 2026-08-05 裁定（用户两次实测后）：字号一致优先于呼吸感。这里
            // 曾经先扣一条"留白边距"再去 fit，结果被长正文夹住的 `$$` 掉到
            // 74–76%，而前后有空行的同款仍是 100%——同组两个尺寸，正是要消
            // 除的现象。固定行高里造不出高度，留白只能靠 emitter 在 `$$` 前
            // 后留空行。别再加回来。
            let available_height = (bounds.height() + bleed_top + bleed_bottom).max(1.0);

            // 样式阶梯先于字号阶梯。夹在两条正文行之间的 `$$` 块只有大约
            // 一行高度，而 display 样式想要两行：`\sum` 上下堆叠、分式保持
            // 全尺寸。把同一份源码排成 text 样式可让字母保持终端字号，并把
            // 上下限移到运算符旁边——这正是 TeX 对段落内数学的做法。统一
            // 缩小是最后手段，因为那才让公式读起来"比正文小"。
            let mut best: Option<(bool, f32)> = None;
            for &display_style in styles {
                let metrics = state
                    .layout(
                        overlay.formula_id,
                        &overlay.source,
                        font_pixel_size,
                        pixels_per_point,
                        display_style,
                    )
                    .ok()?
                    .metrics;
                let fit = fit_ratio(
                    &metrics,
                    available_width,
                    available_height,
                    height_overrun_tolerance,
                );
                if fit >= 1.0 {
                    best = Some((display_style, font_pixel_size));
                    break;
                }
                // 数学布局对像素字号只是近似线性；与 markdown 阅读器同一取整
                // 余量，保证重排后的墨迹仍落在 clip 会执行的预算内。
                let fitted = font_pixel_size * fit * 0.98;
                if best.is_none_or(|(_, previous)| fitted > previous) {
                    best = Some((display_style, fitted));
                }
            }

            let (display_style, fitted_pixel_size) = best?;
            if !fitted_pixel_size.is_finite() || fitted_pixel_size < MIN_READABLE_MATH_PX {
                return None;
            }
            let metrics = state
                .layout(
                    overlay.formula_id,
                    &overlay.source,
                    fitted_pixel_size,
                    pixels_per_point,
                    display_style,
                )
                .ok()?
                .metrics;
            let compact_cells = compact_inline.then(|| {
                quantized_visual_cells(metrics.width, size.cell_width(), remaining_columns)
            });
            let box_right = compact_cells.map_or(box_right, |cells| {
                bounds.left + cells as f32 * size.cell_width()
            });
            // display 数学像块级排版一样居中。源码被紧凑 cell 盒替代的行内
            // 数学在该量化盒内居中；绝不在旧源跨度内居中。
            let centered = overlay.display
                || compact_inline
                || metrics.width >= (box_right - bounds.left) * 0.75;
            Some(PreparedFormula {
                fitted_pixel_size,
                display_style,
                bleed_top,
                bleed_bottom,
                box_right,
                centered,
                compact_cells,
            })
        })
        .collect()
}

/// 行→被公式覆盖的源格跨度查找表；仅包含**会**渲染的公式。网格 pass 保留
/// 每个源格已解析的背景，但抑制其字形与装饰，全屏 TUI 因而保有连续表面。
#[derive(Default)]
pub(crate) struct CoverageMask {
    rows: BTreeMap<usize, Vec<(usize, usize)>>,
}

impl CoverageMask {
    /// `gates` 与 overlays 一一对应；`None` 的公式保留原文（其源格不进掩码）。
    pub(crate) fn build<T>(overlays: &[FormulaOverlay], gates: &[Option<T>]) -> Self {
        Self::from_spans(
            overlays
                .iter()
                .zip(gates)
                .filter(|(_, gate)| gate.is_some())
                .flat_map(|(overlay, _)| overlay.covered_spans()),
        )
    }

    /// 由调用方自己挑出的源格跨度重建掩码。在位图预检之后用它：那一步
    /// 之前拿不到位图的公式还留在 `build` 的结果里，必须整条剔除。
    pub(crate) fn from_spans(spans: impl IntoIterator<Item = (usize, usize, usize)>) -> Self {
        let mut rows: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for (row, start, end) in spans {
            rows.entry(row).or_default().push((start, end));
        }
        Self { rows }
    }

    pub(crate) fn covers(&self, point: Point<usize>) -> bool {
        self.rows.get(&point.line).is_some_and(|spans| {
            spans
                .iter()
                .any(|&(start, end)| (start..end).contains(&point.column.0))
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// 一个 overlay 的后端无关绘制计划：几何决策（fit/居中/bleed/裁剪）的
/// 单一出口，的 GPUI paint pass 消费，保证公式落点与裁剪像素级同源。
/// 坐标系与 [`MathViewport`] 一致。
#[derive(Clone, Copy, Debug)]
pub(crate) struct OverlayDrawPlan {
    pub(crate) fitted_pixel_size: f32,
    pub(crate) display_style: bool,
    pub(crate) origin_x: f32,
    pub(crate) baseline_y: f32,
    pub(crate) clip_left: f32,
    pub(crate) clip_top: f32,
    pub(crate) clip_right: f32,
    pub(crate) clip_bottom: f32,
    pub(crate) foreground: Rgb,
}

/// 计算一个已通过预检的 overlay 的绘制几何。`None` = 本帧回退原文
/// （布局失败或裁剪退化），调用方自行决定回退方式（保留源格不跳过）。
pub(crate) fn plan_overlay_draw(
    state: &mut TerminalMathState,
    overlay: &FormulaOverlay,
    prepared: &PreparedFormula,
    size: &MathViewport,
    pixels_per_point: f32,
) -> Option<OverlayDrawPlan> {
    let mut bounds = overlay.bounds(size)?;
    let shift_columns = match overlay.spans.as_slice() {
        [span] => usize::try_from(span.row)
            .ok()
            .map_or(0, |row| state.projection.shift_before(row, span.start)),
        _ => 0,
    };
    let shift_pixels = shift_columns as f32 * size.cell_width();
    bounds.left += shift_pixels;
    bounds.right += shift_pixels;
    let projected_box_right = prepared.box_right + shift_pixels;

    // 预检已通过，实践中这里不会失败。
    let metrics = state
        .layout(
            overlay.formula_id,
            &overlay.source,
            prepared.fitted_pixel_size,
            pixels_per_point,
            prepared.display_style,
        )
        .ok()?
        .metrics;
    let total_height = metrics.height + metrics.depth;
    let viewport_right = size.padding_x() + size.columns() as f32 * size.cell_width();
    let viewport_bottom = size.padding_y() + size.screen_lines() as f32 * size.cell_height();
    let box_right = projected_box_right;
    let box_width = box_right - bounds.left;
    let origin_x = if prepared.centered {
        bounds.left + (box_width - metrics.width) / 2.0
    } else {
        bounds.left + FORMULA_INSET
    };
    // 墨迹放得下时在源行内居中。放不下的部分按两侧实际可借空间比例分配：
    // 上方空行、下方正文时，溢出向上走，公式与下方文字之间保留完整行间隙，
    // 而不是把拥挤平均摊开。
    let slack = bounds.height() - total_height;
    let room = prepared.bleed_top + prepared.bleed_bottom;
    let top = if slack >= 0.0 {
        bounds.top + slack / 2.0
    } else if room > 0.0 {
        bounds.top + slack * (prepared.bleed_top / room)
    } else {
        bounds.top + slack / 2.0
    };
    let baseline_y = (top + metrics.height)
        .max(bounds.top - prepared.bleed_top + metrics.height)
        .min(bounds.bottom + prepared.bleed_bottom - metrics.depth);

    // clip 跟随 bleed 预算，绝不跟随墨迹：fit 吸收不掉的部分被裁掉，而不是
    // 落到相邻正文上。（跟随墨迹正是公式盖到相邻行的旧病根。）
    let clip_left = bounds.left.max(size.padding_x());
    let clip_top = (bounds.top - prepared.bleed_top).max(size.padding_y());
    let clip_right = box_right.min(viewport_right);
    let clip_bottom = (bounds.bottom + prepared.bleed_bottom).min(viewport_bottom);
    if clip_right <= clip_left || clip_bottom <= clip_top {
        return None;
    }
    Some(OverlayDrawPlan {
        fitted_pixel_size: prepared.fitted_pixel_size,
        display_style: prepared.display_style,
        origin_x,
        baseline_y,
        clip_left,
        clip_top,
        clip_right,
        clip_bottom,
        foreground: overlay.foreground,
    })
}

#[cfg(test)]
mod tests;
