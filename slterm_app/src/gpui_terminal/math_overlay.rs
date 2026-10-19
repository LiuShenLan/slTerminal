//! GPUI 壳终端的公式覆盖层(管线 A 呈现端)。
//!
//! 探测/持久化/fit/几何全部复用 `terminal_math`(单一权威);本文件只做
//! 三件事:在 term 锁内组织一帧的扫描与绘制计划、把覆盖掩码交给格子绘制
//! 循环跳过源格、按计划用 [`super::math_render`] 的位图管线上屏。
//!
//! 普通拖选保留公式与投影,选区始终指向底层源码。Vi、矩形和语义选择
//! 继续显示源码,以保留它们按源列编辑的语义。备用屏幕程序可能把光标留在
//! 静态内容末行,因此不能把那一行当作正在编辑而排除。
//!
//! `OverlayCell` 必须在与 `RenderSnapshot::capture` 相同的 `Term` 锁窗口内
//! 从 Term cell 构造:快照的 `SnapCell` 不含 DIM,而推理样式过滤需要它
//! (裁定:同锁窗口构造,不给快照类型加字段)。

use std::ops::Range;
use std::sync::Arc;

use gpui::{App, Bounds, ContentMask, Pixels, Rgba, SharedString, Window, point, px, size};
use slterm_terminal::Term;
use slterm_terminal::event::EventListener;
use slterm_terminal::grid::Dimensions as _;
use slterm_terminal::index::{Column, Line, Point, Side};
use slterm_terminal::selection::{SelectionRange, SelectionType};
use slterm_terminal::term::TermMode;
use slterm_terminal::term::cell::Flags;
use vte::ansi::{Color, NamedColor, Rgb};

use super::colors::Palette;
use super::math_render;
use crate::terminal_math::{
    self, CoverageMask, FormulaOverlay, LineProjection, MathViewport, OverlayCell, OverlayDrawPlan,
    PreparedFormula, TerminalMathState,
};

/// 每 pane 一份的覆盖层状态(`TerminalView` 持有)。内部的持久锚点/缓存
/// 语义(滚动、reflow、清屏失效)全部由共享的 [`TerminalMathState`] 决定。
#[derive(Default)]
pub(crate) struct MathOverlay {
    state: TerminalMathState,
}

/// 一条通过几何计划的公式:源码 + 绘制计划(坐标系 = 网格局部逻辑 px)。
struct PlannedFormula {
    source: Arc<str>,
    plan: OverlayDrawPlan,
    /// 本条公式盖住的源格跨度 `(行, 起, 止)`;位图预检剔除公式之后要按
    /// 存活的公式重建掩码。
    spans: Vec<(usize, usize, usize)>,
    selected: bool,
}

/// 扫描/fit 已通过、等待位图预检的公式。预检只依赖公式内容、字号和颜色,
/// 不依赖最终屏幕原点;因此先在空投影下生成候选,过滤完成后再统一重算位置。
struct FormulaCandidate {
    overlay: FormulaOverlay,
    prepared: PreparedFormula,
    plan: OverlayDrawPlan,
}

/// term 锁内完成的扫描产物:全部为 owned 数据,不借用网格。
///
/// 拆成两阶段的目的是把首次公式编译(`prepare_overlays` → `state.layout`)
/// 移出 `Term` 锁:第一阶段只在锁内读网格;第二阶段在锁外做 CPU 密集的
/// 编译与几何计划,不再接触 `Term`。
pub(crate) struct ScannedMathFrame {
    size: MathViewport,
    font_pixel_size: f32,
    pixels_per_point: f32,
    reflow_inline: bool,
    selection: Option<SelectionRange>,
    viewport_origin: Line,
    overlays: Vec<FormulaOverlay>,
    /// Vi/矩形/语义选择保留源码:扫描阶段已清空投影,计划阶段直接产出空帧。
    source_only: bool,
}

/// term 锁内产生、锁外消费的候选帧。位图合成不能在 term 锁内访问,
/// 否则字体/位图缓存的工作会把 PTY 网格锁拖进渲染慢路径。
pub(crate) struct PendingMathFrame {
    candidates: Vec<FormulaCandidate>,
    size: MathViewport,
    pixels_per_point: f32,
    reflow_inline: bool,
    selection: Option<SelectionRange>,
    viewport_origin: Line,
}

impl PendingMathFrame {
    fn empty(size: MathViewport, pixels_per_point: f32) -> Self {
        Self {
            candidates: Vec::new(),
            size,
            pixels_per_point,
            reflow_inline: false,
            selection: None,
            viewport_origin: Line(0),
        }
    }
}

/// 一帧的覆盖产物:绘制清单 + 源格跳过掩码。
#[derive(Default)]
pub(crate) struct MathFrame {
    formulas: Vec<PlannedFormula>,
    coverage: CoverageMask,
    projection: LineProjection,
}

impl MathFrame {
    /// 该格是否被某条公式覆盖(覆盖格不画原文)。
    pub(crate) fn covers(&self, row: usize, column: usize) -> bool {
        !self.coverage.is_empty() && self.coverage.covers(Point::new(row, Column(column)))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.formulas.is_empty()
    }

    /// 普通格的源列映射到这一帧的视觉列;公式源格以及越过右边界的格返回
    /// `None`。调用方仍以源坐标决定颜色、链接和光标语义。
    pub(crate) fn project_cell(
        &self,
        row: usize,
        source_column: usize,
        columns: usize,
    ) -> Option<usize> {
        self.projection
            .project_cell(Point::new(row, Column(source_column)), columns)
            .map(|point| point.column.0)
    }

    /// 公式源码格的 ANSI 背景映射。视觉公式盒之外的多余源码格会被丢弃,
    /// 避免压缩后仍留下一条源宽度的背景色带。
    pub(crate) fn project_formula_background(
        &self,
        row: usize,
        source_column: usize,
        columns: usize,
    ) -> Option<usize> {
        self.projection
            .project_formula_background(Point::new(row, Column(source_column)), columns)
            .map(|point| point.column.0)
    }

    /// 光标/浮层锚点优先按普通格投影;极端情况下锚点落进公式源区,则退到
    /// 公式视觉背景能承载的位置。
    pub(crate) fn visual_column(&self, row: usize, source_column: usize, columns: usize) -> usize {
        self.project_cell(row, source_column, columns)
            .or_else(|| self.project_formula_background(row, source_column, columns))
            .unwrap_or_else(|| source_column.min(columns.saturating_sub(1)))
    }

    /// 把源 run 投影成若干连续视觉 run。背景跨过公式时必须逐源格映射,
    /// 但最终仍合并相邻视觉格,避免给每个背景 cell 单独提交一个 quad。
    pub(crate) fn projected_runs(
        &self,
        row: usize,
        source: Range<usize>,
        columns: usize,
        formula_background: bool,
    ) -> Vec<Range<usize>> {
        let mut runs = Vec::new();
        let mut current: Option<Range<usize>> = None;
        for source_column in source {
            let target = if formula_background && self.covers(row, source_column) {
                self.project_formula_background(row, source_column, columns)
            } else {
                self.project_cell(row, source_column, columns)
            };
            match target {
                Some(column) if current.as_ref().is_some_and(|run| run.end == column) => {
                    current.as_mut().expect("current visual run").end += 1;
                }
                Some(column) => {
                    if let Some(run) = current.take() {
                        runs.push(run);
                    }
                    current = Some(column..column + 1);
                }
                None => {
                    if let Some(run) = current.take() {
                        runs.push(run);
                    }
                }
            }
        }
        if let Some(run) = current {
            runs.push(run);
        }
        runs
    }

    /// 一帧公式数(测试与诊断)。
    #[cfg(test)]
    fn len(&self) -> usize {
        self.formulas.len()
    }

    #[cfg(test)]
    fn source(&self, index: usize) -> &str {
        &self.formulas[index].source
    }

    #[cfg(test)]
    fn plan(&self, index: usize) -> &OverlayDrawPlan {
        &self.formulas[index].plan
    }

    #[cfg(test)]
    fn selected(&self, index: usize) -> bool {
        self.formulas[index].selected
    }
}

fn plan_color(plan: &OverlayDrawPlan) -> Rgba {
    Rgba {
        r: plan.foreground.r as f32 / 255.0,
        g: plan.foreground.g as f32 / 255.0,
        b: plan.foreground.b as f32 / 255.0,
        a: 1.0,
    }
}

/// 可见网格的 `MathViewport`:padding = 0,行列与元素布局一致。
///
/// 半格余量是为了 `columns()` 的除法不会把 `8.399994 * 120 / 8.399994 =
/// 119.999` 截成少一列,从而丢掉行尾的 `\]` / `\)`。
pub(crate) fn grid_math_viewport(
    columns: usize,
    rows: usize,
    cell_width: f32,
    cell_height: f32,
) -> MathViewport {
    MathViewport::new(
        cell_width * columns as f32 + cell_width * 0.5,
        cell_height * rows as f32 + cell_height * 0.5,
        cell_width,
        cell_height,
        0.0,
        0.0,
    )
}

impl MathOverlay {
    /// 第一阶段:只在 term 锁内做依赖网格的扫描,返回 owned 数据。
    ///
    /// 本阶段不编译公式,因此 PTY 网格锁不会被首次 TeX 编译拖住。
    pub(crate) fn scan_frame<T: EventListener>(
        &mut self,
        term: &Term<T>,
        size: &MathViewport,
        palette: &Palette,
        font_pixel_size: f32,
        pixels_per_point: f32,
    ) -> ScannedMathFrame {
        let alt_screen = term.mode().contains(TermMode::ALT_SCREEN);
        let selection = term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(term));
        // 矩形/语义选择依赖源列展开;普通拖选沿用公式边界命中映射。
        if term.mode().intersects(TermMode::VI)
            || (selection.is_some()
                && term.selection.as_ref().is_some_and(|selection| {
                    matches!(selection.ty, SelectionType::Block | SelectionType::Semantic)
                }))
        {
            // 与旧路径一致:命中区域在 paint 之外,投影必须立即清掉。
            self.state.update_projection(&[], &[], false);
            return ScannedMathFrame {
                size: *size,
                font_pixel_size,
                pixels_per_point,
                reflow_inline: false,
                selection: None,
                viewport_origin: Line(0),
                overlays: Vec::new(),
                source_only: true,
            };
        }

        let origin = term.viewport_origin_for(size.screen_lines());
        // 光标所在行是活动输入,必须保留原文。备用屏幕里同样放过:编辑器
        // 的光标压在你要改的那一行上,换成渲染图就没法编辑源码了。
        let cursor =
            slterm_terminal::term::point_to_viewport_from(origin, term.grid().cursor.point).filter(
                |point| point.line < size.screen_lines() && point.column.0 < size.columns(),
            );
        let default_foreground = rgb_from_rgba(palette.foreground);
        let rendered_cells =
            scan_cells_from_term(term, size, palette, term.colors(), default_foreground);
        let overlays = terminal_math::scan_visible(
            &mut self.state,
            term,
            size,
            &rendered_cells,
            alt_screen,
            cursor,
            default_foreground,
        );
        ScannedMathFrame {
            size: *size,
            font_pixel_size,
            pixels_per_point,
            // TUI/备用屏继续拥有固定列坐标,但公式覆盖本身仍可用。
            reflow_inline: !alt_screen,
            selection,
            viewport_origin: origin,
            overlays,
            source_only: false,
        }
    }

    /// 第二阶段:预编 + 几何计划。不接收 `Term`,必须在 term 锁外调用。
    pub(crate) fn plan_frame(&mut self, scanned: ScannedMathFrame) -> PendingMathFrame {
        let ScannedMathFrame {
            size,
            font_pixel_size,
            pixels_per_point,
            reflow_inline,
            selection,
            viewport_origin,
            overlays,
            source_only,
        } = scanned;
        if source_only {
            return PendingMathFrame::empty(size, pixels_per_point);
        }

        let prepared = terminal_math::prepare_overlays(
            &mut self.state,
            &overlays,
            &size,
            font_pixel_size,
            pixels_per_point,
        );
        // 候选计划必须在空投影下生成。位图预检会淘汰无法合成的公式;若先把
        // 它们计入累计 shift,后续存活公式的原点就会沿用一个不存在的压缩量。
        self.state.update_projection(&[], &[], false);
        let candidates = overlays
            .into_iter()
            .zip(prepared)
            .filter_map(|(overlay, prepared)| {
                let prepared = prepared?;
                let plan = terminal_math::plan_overlay_draw(
                    &mut self.state,
                    &overlay,
                    &prepared,
                    &size,
                    pixels_per_point,
                )?;
                Some(FormulaCandidate {
                    overlay,
                    prepared,
                    plan,
                })
            })
            .collect();
        PendingMathFrame {
            candidates,
            size,
            pixels_per_point,
            reflow_inline,
            selection,
            viewport_origin,
        }
    }

    /// 位图预检并完成一帧。必须在 term 锁外调用。
    pub(crate) fn finalize_frame(
        &mut self,
        pending: PendingMathFrame,
        raster_scale: f32,
    ) -> MathFrame {
        let pixels_per_point = pending.pixels_per_point;
        self.finalize_frame_with(pending, |source, plan| {
            let Some(layout) = math_render::layout_for(
                source,
                plan.display_style,
                plan.fitted_pixel_size,
                pixels_per_point,
            ) else {
                return false;
            };
            // 前景色与 paint 用同一个值,否则缓存键不同,预检的那张图白合成。
            math_render::can_compose(
                source,
                plan.display_style,
                plan.fitted_pixel_size,
                pixels_per_point,
                raster_scale,
                plan_color(plan),
                &layout,
            )
        })
    }

    /// 预检策略与几何收敛拆开,测试可以用确定的存活集合验证 projection,
    /// 不需要启动 GPUI 字体后端。
    pub(crate) fn finalize_frame_with(
        &mut self,
        mut pending: PendingMathFrame,
        mut paintable: impl FnMut(&Arc<str>, &OverlayDrawPlan) -> bool,
    ) -> MathFrame {
        pending.candidates.retain(|candidate| {
            let source = candidate.overlay.source_arc();
            paintable(&source, &candidate.plan)
        });

        // plan 也可能在最终横向 shift 后退化。每淘汰一条就重新建立共享
        // LineProjection,直到 projection、coverage、draw plan 使用同一集合。
        loop {
            let overlays: Vec<FormulaOverlay> = pending
                .candidates
                .iter()
                .map(|candidate| candidate.overlay.clone())
                .collect();
            let prepared: Vec<Option<PreparedFormula>> = pending
                .candidates
                .iter()
                .map(|candidate| Some(candidate.prepared))
                .collect();
            let retained = self.state.update_projection_with_survivors(
                &overlays,
                &prepared,
                pending.reflow_inline,
            );

            if retained.iter().any(|keep| !keep) {
                let mut index = 0usize;
                pending.candidates.retain(|_| {
                    let keep = retained[index];
                    index += 1;
                    keep
                });
                continue;
            }

            let before = pending.candidates.len();
            pending.candidates.retain_mut(|candidate| {
                let Some(plan) = terminal_math::plan_overlay_draw(
                    &mut self.state,
                    &candidate.overlay,
                    &candidate.prepared,
                    &pending.size,
                    pending.pixels_per_point,
                ) else {
                    return false;
                };
                candidate.plan = plan;
                true
            });
            if pending.candidates.len() == before {
                break;
            }
        }

        let formulas: Vec<PlannedFormula> = pending
            .candidates
            .into_iter()
            .map(|candidate| PlannedFormula {
                source: candidate.overlay.source_arc(),
                plan: candidate.plan,
                spans: candidate.overlay.covered_spans().collect(),
                selected: pending.selection.is_some_and(|selection| {
                    candidate.overlay.covered_spans().any(|(row, start, end)| {
                        let line = pending.viewport_origin + row;
                        line >= selection.start.line
                            && line <= selection.end.line
                            && (start..end)
                                .any(|column| selection.contains(Point::new(line, Column(column))))
                    })
                }),
            })
            .collect();
        let spans: Vec<(usize, usize, usize)> = formulas
            .iter()
            .flat_map(|formula| formula.spans.iter().copied())
            .collect();
        MathFrame {
            coverage: CoverageMask::from_spans(spans),
            projection: self.state.projection_snapshot(),
            formulas,
        }
    }

    /// GPUI 所有 terminal-cell 输入的反投影入口。调用方传入与当前 term 锁
    /// 同源的 viewport origin,避免滚动或 resize commit 时把视觉行映错。
    pub(crate) fn source_point(
        &self,
        point: Point,
        side: Side,
        viewport_origin: Line,
    ) -> (Point, Side) {
        self.state.source_point(point, side, viewport_origin)
    }
}

/// 旧壳 `RenderableContent` 迭代器的 GPUI 等价物:给 `scan_visible` 填
/// fallback / `bg_alpha` / 前景。`bg_alpha` 对齐旧壳规则:默认 Named 背景
/// 为 0,反色或其它 ANSI 背景为 1。空格+默认背景跳过。
fn scan_cells_from_term<T: EventListener>(
    term: &Term<T>,
    size: &MathViewport,
    palette: &Palette,
    overrides: &slterm_terminal::term::color::Colors,
    default_foreground: Rgb,
) -> Vec<OverlayCell> {
    let grid = term.grid();
    let origin = term.viewport_origin_for(size.screen_lines());
    let columns = size.columns().min(grid.columns());
    let screen_lines = size
        .screen_lines()
        .min(((grid.bottommost_line() - origin).0.max(0) as usize).saturating_add(1));
    let mut cells = Vec::new();
    for row in 0..screen_lines {
        let line = origin + row as i32;
        for column in 0..columns {
            let cell = &grid[line][Column(column)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let bg_alpha = if cell.flags.contains(Flags::INVERSE) {
                1.0
            } else if cell.bg == Color::Named(NamedColor::Background) {
                0.0
            } else {
                1.0
            };
            if bg_alpha == 0.0
                && cell.c == ' '
                && !cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
            {
                continue;
            }
            let color = if cell.flags.contains(Flags::INVERSE) {
                cell.bg
            } else {
                cell.fg
            };
            let fg = match color {
                Color::Named(_) | Color::Indexed(_) | Color::Spec(_) => rgb_from_rgba(
                    palette.resolve(color, overrides, cell.flags.contains(Flags::BOLD)),
                ),
            };
            cells.push(OverlayCell {
                character: cell.c,
                point: Point::new(row, Column(column)),
                fg: if color == Color::Named(NamedColor::Foreground)
                    || color == Color::Named(NamedColor::Background)
                {
                    default_foreground
                } else {
                    fg
                },
                bg_alpha,
                // 同一锁窗口从 Term cell Flags 取 DIM,SnapCell 不含该位。
                flags: cell.flags,
            });
        }
    }
    cells
}

fn rgb_from_rgba(color: Rgba) -> Rgb {
    Rgb {
        r: (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        g: (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        b: (color.b.clamp(0.0, 1.0) * 255.0).round() as u8,
    }
}

/// 按计划绘制一帧的公式(在格子文本之后调用)。`origin` 是网格左上角的
/// 窗口坐标;`cell` 是 (宽, 高),text ops 的"整字符可见才画"合同用它。
pub(crate) fn paint_frame(
    frame: &MathFrame,
    origin: gpui::Point<Pixels>,
    cell: (f32, f32),
    pixels_per_point: f32,
    selection_color: Rgba,
    window: &mut Window,
    cx: &mut App,
) {
    for formula in &frame.formulas {
        let plan = &formula.plan;
        let color = plan_color(plan);
        let source: SharedString = formula.source.to_string().into();

        // 裁剪跟随 bleed 预算(与 plan_overlay_draw 同一矩形),交给 GPUI
        // 的 content mask 执行像素级裁剪。
        let clip = Bounds::new(
            point(origin.x + px(plan.clip_left), origin.y + px(plan.clip_top)),
            size(
                px((plan.clip_right - plan.clip_left).max(0.0)),
                px((plan.clip_bottom - plan.clip_top).max(0.0)),
            ),
        );
        let left = origin.x + px(plan.origin_x);
        let baseline = origin.y + px(plan.baseline_y);
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            let Some(layout) = math_render::layout_for(
                &source,
                plan.display_style,
                plan.fitted_pixel_size,
                pixels_per_point,
            ) else {
                return;
            };
            math_render::paint_formula_image(
                &source,
                plan.display_style,
                plan.fitted_pixel_size,
                pixels_per_point,
                &layout,
                left,
                baseline,
                color,
                window,
            );
            // 数学字体缺字的字符(公式里的中文等):整字符落在裁剪窗内
            // 才画,避免半个汉字。
            math_render::paint_text_ops(
                &layout,
                plan.fitted_pixel_size,
                plan.origin_x,
                plan.baseline_y,
                plan.clip_left,
                plan.clip_top,
                plan.clip_right,
                plan.clip_bottom,
                cell,
                color,
                window,
                cx,
            );
        });
        if formula.selected {
            // 选区把公式当对象勾边;位图与前景不变,不产生新的栅格/颜色键。
            window.paint_quad(gpui::outline(
                clip,
                selection_color,
                gpui::BorderStyle::Solid,
            ));
        }
    }
}

#[cfg(test)]
mod math_overlay_tests {
    use super::*;
    use slterm_terminal::event::VoidListener;
    use slterm_terminal::grid::Dimensions;
    use slterm_terminal::index::{Column, Line, Point};
    use slterm_terminal::selection::{Selection, SelectionType};
    use slterm_terminal::term::Config;

    struct TestSize {
        cols: usize,
        rows: usize,
    }

    impl Dimensions for TestSize {
        fn total_lines(&self) -> usize {
            self.rows
        }

        fn screen_lines(&self) -> usize {
            self.rows
        }

        fn columns(&self) -> usize {
            self.cols
        }
    }

    fn term_with(cols: usize, rows: usize, lines: &[&str]) -> Term<VoidListener> {
        let mut term = Term::new(Config::default(), &TestSize { cols, rows }, VoidListener);
        for (row, text) in lines.iter().enumerate() {
            for (col, ch) in text.chars().enumerate() {
                if col >= cols {
                    break;
                }
                let cell = &mut term.grid_mut()[Line(row as i32)][Column(col)];
                cell.c = ch;
            }
        }
        // 公式行不能与光标逻辑行重合,否则 scan_visible 会整段排除。
        term.grid_mut().cursor.point = Point::new(Line(rows as i32 - 1), Column(0));
        term
    }

    fn plan(
        overlay: &mut MathOverlay,
        term: &Term<VoidListener>,
        cols: usize,
        rows: usize,
    ) -> MathFrame {
        let size = grid_math_viewport(cols, rows, 8.0, 16.0);
        let scanned = overlay.scan_frame(term, &size, &Palette::default(), 16.0, 1.0);
        let pending = overlay.plan_frame(scanned);
        overlay.finalize_frame_with(pending, |_, _| true)
    }

    #[test]
    fn grid_viewport_does_not_drop_a_column_to_float_truncation() {
        let viewport = grid_math_viewport(120, 30, 8.399994, 16.8);
        assert_eq!(viewport.columns(), 120);
        assert_eq!(viewport.screen_lines(), 30);
        assert_eq!(viewport.padding_x(), 0.0);
        assert_eq!(viewport.padding_y(), 0.0);
    }

    #[test]
    fn bracket_display_scans_without_inline_dollar() {
        let mut overlay = MathOverlay::default();
        let term = term_with(16, 4, &[r"\[x\]", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 16, 4);
        assert!(!frame.is_empty(), "\\[x\\] 是 display 数学,必须出覆盖层");
        assert!(frame.covers(0, 0));
        assert!(frame.covers(0, 4), "闭合 ] 必须被覆盖");
        assert_eq!(frame.source(0), "x");
        assert!(frame.plan(0).display_style, "\\[ 必须按 display 样式排版");
    }

    #[test]
    fn parenthesized_inline_scans_without_ai_cli() {
        let mut overlay = MathOverlay::default();
        let term = term_with(16, 4, &[r"\(x\)", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 16, 4);
        assert!(!frame.is_empty(), "\\(x\\) 是标准行内数学");
        assert!(frame.covers(0, 0));
        assert_eq!(frame.source(0), "x");
        assert!(!frame.plan(0).display_style);
    }

    #[test]
    fn inline_dollar_uses_the_standard_formula_rule() {
        let mut overlay = MathOverlay::default();
        let term = term_with(16, 4, &["$x$", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 16, 4);
        assert!(!frame.is_empty(), "$x$ 是标准行内数学");
        assert_eq!(frame.source(0), "x");
        assert!(!frame.plan(0).display_style);
    }

    #[test]
    fn main_screen_compacts_inline_suffix_without_changing_source_columns() {
        let mut overlay = MathOverlay::default();
        let term = term_with(20, 4, &["$x$ suffix", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 20, 4);
        let source_suffix = 3;
        let visual_suffix = frame
            .project_cell(0, source_suffix, 20)
            .expect("suffix remains inside the viewport");
        assert!(
            visual_suffix < source_suffix,
            "inline suffix must move into the compact gap"
        );
        assert!(frame.covers(0, 0));
        assert!(frame.covers(0, 2));
        // 终端 grid 本身不被覆盖层修改(终端仍是真值)。
        assert_eq!(term.grid()[Line(0)][Column(source_suffix)].c, ' ');
    }

    #[test]
    fn failed_formula_does_not_shift_later_survivors() {
        let line = "a $x$ b $y^2$ c";
        let term = term_with(32, 4, &[line, "", "prompt"]);
        let size = grid_math_viewport(32, 4, 8.0, 16.0);

        let mut all_overlay = MathOverlay::default();
        let all_scanned = all_overlay.scan_frame(&term, &size, &Palette::default(), 16.0, 1.0);
        let all_pending = all_overlay.plan_frame(all_scanned);
        let all = all_overlay.finalize_frame_with(all_pending, |_, _| true);

        let mut filtered_overlay = MathOverlay::default();
        let filtered_scanned =
            filtered_overlay.scan_frame(&term, &size, &Palette::default(), 16.0, 1.0);
        let filtered_pending = filtered_overlay.plan_frame(filtered_scanned);
        let filtered = filtered_overlay
            .finalize_frame_with(filtered_pending, |source, _| source.as_ref() != "x");

        assert!(
            !filtered.covers(0, 2),
            "failed first formula must leave its source visible"
        );
        assert!(
            filtered.covers(0, 8),
            "later paintable formula still overlays"
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered.project_formula_background(0, 8, 32),
            Some(8),
            "surviving formula must not inherit the failed formula's shift"
        );
        let source_suffix = 13;
        let filtered_suffix = filtered
            .project_cell(0, source_suffix, 32)
            .expect("filtered suffix remains visible");
        let all_suffix = all
            .project_cell(0, source_suffix, 32)
            .expect("unfiltered suffix remains visible");
        assert!(
            filtered_suffix > all_suffix,
            "the failed formula must not contribute a stale cumulative shift"
        );
    }

    #[test]
    fn ansi_background_keeps_formula_source_literal() {
        let mut overlay = MathOverlay::default();
        let mut term = term_with(16, 4, &["$x$", "", "prompt"]);
        for col in 0..3 {
            term.grid_mut()[Line(0)][Column(col)].bg = Color::Indexed(4);
        }
        let frame = plan(&mut overlay, &term, 16, 4);
        assert!(frame.is_empty(), "TUI/code 自有背景保持源码不动");
    }

    #[test]
    fn empty_selection_does_not_disable_overlays() {
        let mut overlay = MathOverlay::default();
        let mut term = term_with(16, 4, &[r"\[x\]", "", "prompt"]);
        term.selection = Some(Selection::new(
            SelectionType::Simple,
            Point::new(Line(2), Column(0)),
            Side::Left,
        ));
        let frame = plan(&mut overlay, &term, 16, 4);
        assert!(!frame.is_empty(), "零宽残留选区不得挡住整帧");
    }

    #[test]
    fn ordinary_selection_preserves_projection_and_copies_the_formula_source() {
        let mut overlay = MathOverlay::default();
        let mut term = term_with(20, 4, &["$x^2$ suffix", "", "prompt"]);
        let source_suffix = 5;
        let projected = plan(&mut overlay, &term, 20, 4);
        assert!(projected.project_cell(0, source_suffix, 20).unwrap() < source_suffix);

        let visual_suffix = projected.project_cell(0, source_suffix, 20).unwrap();
        let (start, start_side) =
            overlay.source_point(Point::new(Line(0), Column(0)), Side::Left, Line(0));
        let (end, end_side) = overlay.source_point(
            Point::new(Line(0), Column(visual_suffix - 1)),
            Side::Right,
            Line(0),
        );
        let mut selection = Selection::new(SelectionType::Simple, start, start_side);
        selection.update(end, end_side);
        term.selection = Some(selection);
        let selected = plan(&mut overlay, &term, 20, 4);

        // 选区仍按源列复制:终端 cell 未被覆盖层重写。
        assert_eq!(term.selection_to_string().as_deref(), Some("$x^2$"));
        assert!(!selected.is_empty());
        assert_eq!(
            selected.project_cell(0, source_suffix, 20),
            Some(visual_suffix)
        );
        assert!(selected.selected(0));
        assert_eq!(selected.source(0), projected.source(0));
        assert_eq!(selected.plan(0).foreground, projected.plan(0).foreground);
        assert_eq!(
            selected.plan(0).fitted_pixel_size,
            projected.plan(0).fitted_pixel_size
        );
    }

    #[test]
    fn source_column_selection_modes_keep_their_existing_source_view() {
        for kind in [SelectionType::Block, SelectionType::Semantic] {
            let mut overlay = MathOverlay::default();
            let mut term = term_with(20, 4, &["$x^2$ suffix", "", "prompt"]);
            assert!(!plan(&mut overlay, &term, 20, 4).is_empty());
            let mut selection = Selection::new(kind, Point::new(Line(0), Column(0)), Side::Left);
            selection.update(Point::new(Line(0), Column(2)), Side::Right);
            term.selection = Some(selection);
            let frame = plan(&mut overlay, &term, 20, 4);
            assert!(frame.is_empty());
            assert_eq!(frame.project_cell(0, 5, 20), Some(5));
        }
    }

    #[test]
    fn selecting_another_row_does_not_move_or_mark_a_formula() {
        let mut overlay = MathOverlay::default();
        let mut term = term_with(32, 4, &["$x^2$ suffix", "other text", "prompt"]);
        let before = plan(&mut overlay, &term, 32, 4);
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(1), Column(0)),
            Side::Left,
        );
        selection.update(Point::new(Line(1), Column(4)), Side::Right);
        term.selection = Some(selection);
        let after = plan(&mut overlay, &term, 32, 4);
        assert!(!after.is_empty());
        assert_eq!(after.project_cell(0, 5, 32), before.project_cell(0, 5, 32));
        assert!(!after.selected(0));
    }

    #[test]
    fn explicit_empty_frame_clears_existing_projection() {
        let mut overlay = MathOverlay::default();
        let term = term_with(20, 4, &["$x^2$ suffix", "", "prompt"]);
        let source_suffix = 5;
        let projected = plan(&mut overlay, &term, 20, 4);
        assert!(projected.project_cell(0, source_suffix, 20).unwrap() < source_suffix);

        let size = grid_math_viewport(20, 4, 8.0, 16.0);
        overlay.state.update_projection(&[], &[], false);
        let pending = PendingMathFrame::empty(size, 1.0);
        let cleared = overlay.finalize_frame_with(pending, |_, _| true);

        assert!(cleared.is_empty());
        assert_eq!(
            cleared.project_cell(0, source_suffix, 20),
            Some(source_suffix)
        );
    }

    #[test]
    fn alt_screen_renders_agent_math_with_fixed_columns() {
        let mut overlay = MathOverlay::default();
        let mut term = term_with(
            96,
            4,
            &[
                r"$n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n$",
                "",
                "prompt",
            ],
        );
        let main_frame = plan(&mut overlay, &term, 96, 4);
        let source_end = r"$n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n$"
            .chars()
            .count();
        assert!(main_frame.project_cell(0, source_end, 96).unwrap() < source_end);
        term.swap_alt();
        // `swap_alt` 按终端语义先清空备用网格;测试内容必须写进交换后的
        // 活动网格,才能验证 ALT_SCREEN 的覆盖与固定列合同。
        for (column, character) in r"$n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n$"
            .chars()
            .enumerate()
        {
            term.grid_mut()[Line(0)][Column(column)].c = character;
        }
        let frame = plan(&mut overlay, &term, 96, 4);
        assert!(!frame.is_empty(), "备用屏输出同样走 TeX 覆盖路径");
        assert_eq!(
            frame.source(0),
            r"n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n"
        );
        assert_eq!(
            frame.project_cell(0, source_end, 96),
            Some(source_end),
            "备用屏公式出覆盖层但必须保持固定 TUI 列坐标"
        );
    }

    #[test]
    fn bracket_display_can_cross_hard_rows() {
        let mut overlay = MathOverlay::default();
        let term = term_with(16, 6, &[r"\[", "x", r"\]", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 16, 6);
        assert!(!frame.is_empty(), "\\[ 跨硬换行必须出覆盖层");
        assert_eq!(frame.source(0), "x");
        assert!(frame.plan(0).display_style);
        assert!(frame.covers(0, 0));
        assert!(frame.covers(2, 0));
    }

    #[test]
    fn markdown_unescaped_bare_bracket_block_overlays() {
        // AI CLI 的 markdown 渲染吃掉 `\[` `\]` `\,` 的反斜杠后屏幕上只剩
        // 裸 `[` 块(`\int`/`\frac` 幸存);全链路必须照常出覆盖层。
        let mut overlay = MathOverlay::default();
        let term = term_with(
            40,
            6,
            &["[", r"\int_0^1 x^2,dx = \frac{1}{3}", "]", "", "prompt"],
        );
        let frame = plan(&mut overlay, &term, 40, 6);
        assert!(
            !frame.is_empty(),
            "markdown 去反斜杠的裸方括号块必须出覆盖层"
        );
        assert_eq!(frame.source(0), r"\int_0^1 x^2,dx = \frac{1}{3}");
        assert!(frame.plan(0).display_style);
        assert!(frame.covers(0, 0));
        assert!(frame.covers(2, 0));
    }

    #[test]
    fn markdown_unescaped_bare_paren_inline_overlays() {
        let mut overlay = MathOverlay::default();
        let term = term_with(40, 4, &[r"also (\sqrt{x^2+y^2}) works", "", "prompt"]);
        let frame = plan(&mut overlay, &term, 40, 4);
        assert!(!frame.is_empty(), "带 TeX 白名单命令的裸括号必须出覆盖层");
        assert_eq!(frame.source(0), r"\sqrt{x^2+y^2}");
        assert!(!frame.plan(0).display_style);
    }

    /// 结构断言「编译阶段不持锁」:扫描完成后由另一线程独占 `Term` 锁
    /// 200ms,主线程序列执行第二阶段 `plan_frame`,不得被该锁阻塞。
    ///
    /// 首次 TeX 编译被刻意排除在计时外(先预热一次):本用例守的是锁边界,
    /// 不是编译速度。
    #[test]
    fn plan_phase_does_not_wait_for_the_term_lock() {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        let term = Arc::new(Mutex::new(term_with(16, 4, &[r"\[x\]", "", "prompt"])));
        let size = grid_math_viewport(16, 4, 8.0, 16.0);
        let mut overlay = MathOverlay::default();

        // 预热编译缓存,让第二阶段只测锁边界。
        {
            let guard = term.lock().unwrap();
            let scanned = overlay.scan_frame(&guard, &size, &Palette::default(), 16.0, 1.0);
            drop(guard);
            let pending = overlay.plan_frame(scanned);
            let warm = overlay.finalize_frame_with(pending, |_, _| true);
            assert!(!warm.is_empty(), "预热帧应含公式");
        }

        let scanned = {
            let guard = term.lock().unwrap();
            overlay.scan_frame(&guard, &size, &Palette::default(), 16.0, 1.0)
        };

        // 150ms < 200ms 持锁时间:既留出调度裕度,又能区分「等锁」与
        // 「锁外计划」。这是结构判据的时间预算,不是语义放宽。
        let hold_time = Duration::from_millis(200);
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let holder = {
            let term = Arc::clone(&term);
            std::thread::spawn(move || {
                let _guard = term.lock().unwrap();
                locked_tx.send(()).unwrap();
                std::thread::sleep(hold_time);
            })
        };
        locked_rx.recv().unwrap();

        let started = Instant::now();
        let pending = overlay.plan_frame(scanned);
        let elapsed = started.elapsed();
        holder.join().unwrap();

        assert!(
            elapsed < Duration::from_millis(150),
            "plan_frame 仍在等待 Term 锁:耗时 {elapsed:?},持锁 {hold_time:?}"
        );
        let frame = overlay.finalize_frame_with(pending, |_, _| true);
        assert!(!frame.is_empty(), "锁外计划仍应产出公式帧");
    }
}
