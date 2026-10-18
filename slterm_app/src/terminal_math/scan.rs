//! 终端网格识别与有界定界符搜索。

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TextGrid {
    pub(super) rows: Vec<Vec<Option<char>>>,
    pub(super) wrapped: Vec<bool>,
    pub(super) columns: usize,
    pub(super) absolute_top: usize,
    pub(super) scrolled_out: usize,
}

impl TextGrid {
    pub(super) fn from_term<T>(terminal: &Term<T>, size: &MathViewport) -> Self {
        Self::from_term_with_lookback(terminal, size, 0)
    }

    pub(super) fn from_term_with_lookback<T>(
        terminal: &Term<T>,
        size: &MathViewport,
        requested_lookback: usize,
    ) -> Self {
        let grid = terminal.grid();
        // Grid 与 PTY resize 在提交点同步，而显示几何每个拖拽 tick 都更新，
        // 于是视觉视口可能不同于网格。扫描锚定渲染器视口原点，只覆盖二者
        // 交集，保证 overlay 坐标与被匹配的渲染 cell 处于同一空间。
        let origin = terminal.viewport_origin_for(size.screen_lines());
        let columns = size.columns().min(grid.columns());
        let screen_lines = size
            .screen_lines()
            .min(((grid.bottommost_line() - origin).0.max(0) as usize).saturating_add(1));
        let scrolled_out = grid.scrolled_out();
        let history_size = grid.history_size();
        let available_lookback = (history_size as i64 + origin.0 as i64).max(0) as usize;
        let lookback = requested_lookback.min(available_lookback);
        let absolute_top = (scrolled_out as i64 + history_size as i64 + origin.0 as i64
            - lookback as i64)
            .max(0) as usize;
        let row_count = screen_lines.saturating_add(lookback);
        let mut rows = Vec::with_capacity(row_count);
        let mut wrapped = Vec::with_capacity(row_count);

        for row in 0..row_count {
            let line = origin + (row as i32 - lookback as i32);
            let mut cells = Vec::with_capacity(columns);
            for column in 0..columns {
                let cell = &grid[line][Column(column)];
                let spacer = cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);
                cells.push((!spacer).then_some(cell.c));
            }
            let is_wrapped = columns > 0
                && grid[line][Column(columns - 1)]
                    .flags
                    .contains(Flags::WRAPLINE);
            rows.push(cells);
            wrapped.push(is_wrapped);
        }

        Self {
            rows,
            wrapped,
            columns,
            absolute_top,
            scrolled_out,
        }
    }

    #[cfg(test)]
    pub(super) fn from_rows(rows: &[&str]) -> Self {
        let columns = rows
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0);
        let rows = rows
            .iter()
            .map(|row| {
                let mut cells: Vec<_> = row.chars().map(Some).collect();
                cells.resize(columns, Some(' '));
                cells
            })
            .collect::<Vec<_>>();
        let wrapped = vec![false; rows.len()];
        Self {
            rows,
            wrapped,
            columns,
            absolute_top: 0,
            scrolled_out: 0,
        }
    }

    pub(super) fn character(&self, position: GridPosition) -> Option<char> {
        self.rows
            .get(position.row)?
            .get(position.column)
            .copied()
            .flatten()
    }

    /// 由终端换行标记连接的物理行构成一条可编辑逻辑行。只抑制光标格可以
    /// 让同一提示行上更早的公式继续渲染，即使用户正在其后输入。
    pub(super) fn logical_rows_containing(
        &self,
        row: usize,
    ) -> Option<std::ops::RangeInclusive<usize>> {
        if row >= self.rows.len() {
            return None;
        }
        let mut start = row;
        while start > 0 && self.wrapped.get(start - 1).copied().unwrap_or(false) {
            start -= 1;
        }
        let mut end = row;
        while end + 1 < self.rows.len() && self.wrapped.get(end).copied().unwrap_or(false) {
            end += 1;
        }
        Some(start..=end)
    }

    pub(super) fn starts_with(&self, position: GridPosition, delimiter: &[char]) -> bool {
        delimiter.iter().enumerate().all(|(offset, expected)| {
            self.character(GridPosition {
                row: position.row,
                column: position.column + offset,
            }) == Some(*expected)
        })
    }

    pub(super) fn is_escaped(&self, position: GridPosition) -> bool {
        let mut column = position.column;
        let mut slashes = 0usize;
        while column > 0
            && self.character(GridPosition {
                row: position.row,
                column: column - 1,
            }) == Some('\\')
        {
            slashes += 1;
            column -= 1;
        }
        slashes % 2 == 1
    }

    pub(super) fn after(&self, position: GridPosition, width: usize) -> GridPosition {
        GridPosition {
            row: position.row,
            column: position.column + width,
        }
    }

    pub(super) fn next(&self, position: GridPosition) -> Option<GridPosition> {
        if position.column + 1 < self.columns {
            Some(GridPosition {
                row: position.row,
                column: position.column + 1,
            })
        } else if position.row + 1 < self.rows.len() {
            Some(GridPosition {
                row: position.row + 1,
                column: 0,
            })
        } else {
            None
        }
    }

    pub(super) fn previous(&self, position: GridPosition) -> Option<GridPosition> {
        if position.column > 0 {
            Some(GridPosition {
                row: position.row,
                column: position.column - 1,
            })
        } else if position.row > 0 {
            Some(GridPosition {
                row: position.row - 1,
                column: self.columns.saturating_sub(1),
            })
        } else {
            None
        }
    }

    /// **display** 定界符的闭合搜索：它们拥有二者之间的行，因此真实换行
    /// 属于公式。Agent TUI 把块级公式硬折到多行时靠这条路径恢复。
    ///
    /// **空行终止搜索。** TeX 禁止数学模式内分段，Markdown 也在空行结束
    /// 数学块，所以真实公式不会跨空行——而 `$$` 本身就是闭合符，扫描器
    /// 否则无法区分开头与伙伴已滚出视口顶部的闭合。没有这道屏障，孤立
    /// 闭合会与**下一个**块的定界符配对，把中间所有段落和公式吞进一个巨大
    /// 候选；它还能编译成功（散文被排成数学标识符），于是整片区域被一张
    /// 图盖住。停在空行则让它保持 unmatched，交由
    /// [`TerminalMathState::scan_visible_grid`] 的历史重建处理。
    ///
    /// 但一行空行**不是**分段：那是 *TUI* 的。Agent CLI 把回答交给不认识
    /// `$$` 的 Markdown 渲染器，其中两条规则会用空行切开 display 块：
    ///
    /// * Claude Code：只有 `=` 的行是 setext 下划线，上一行变成标题，`=`
    ///   消失并留下空行。
    /// * Codex：以 `- ` / `+ ` / `* ` 开头的公式行打断段落成为列表项，
    ///   `start_list` 在前面插空行，`start_item` 把标记画成 `- `，后续行
    ///   （含闭合）缩进两格。
    ///
    /// `open` 是开头位置；当它独占一行时，搜索可跨过这种空隙，但必须同时
    /// 满足四个条件，才能把上述孤立闭合照旧留在 pending 路径：
    ///
    /// 1. 块在空隙之上已有内容——孤立闭合后面紧跟着它的空行；
    /// 2. 空隙只有一行——两行空行是真正的段落分隔；
    /// 3. 空隙后的行自带数学证据——散文没有，吞段落的形状到此为止；
    /// 4. 跨过空隙后闭合必须独占一行，且距开头不超过
    ///    [`DISPLAY_BLOCK_GAP_SEARCH_ROWS`] 行。
    pub(super) fn find_closing(
        &self,
        open: GridPosition,
        mut position: GridPosition,
        delimiter: &[char],
    ) -> Option<GridPosition> {
        let standalone_opener =
            self.span_is_blank(open.row, position.column, self.columns) && open.row == position.row;
        let last_row = open.row.saturating_add(DISPLAY_BLOCK_GAP_SEARCH_ROWS);
        let mut content_rows = 0usize;
        let mut bridged = false;
        // 上一行是刚跨过的空行：这一行要先自证是公式，搜索才继续。
        let mut gap_pending = false;
        loop {
            if position.row >= self.rows.len() {
                return None;
            }
            if self.starts_with(position, delimiter)
                && !self.is_escaped(position)
                && (!bridged || self.delimiter_owns_row(position, delimiter.len()))
            {
                return Some(position);
            }
            let previous_row = position.row;
            position = self.next(position)?;
            if position.row == previous_row {
                continue;
            }
            if self.span_is_blank(position.row, 0, self.columns) {
                if !standalone_opener || content_rows == 0 || gap_pending {
                    return None;
                }
                gap_pending = true;
                bridged = true;
                continue;
            }
            content_rows += 1;
            if bridged && position.row > last_row {
                return None;
            }
            if gap_pending {
                gap_pending = false;
                if !self.row_is_only(position.row, delimiter)
                    && !self.row_has_math_evidence(position.row)
                {
                    return None;
                }
            }
        }
    }

    /// `position` 处的定界符是否是它所在行唯一的墨迹。
    pub(super) fn delimiter_owns_row(&self, position: GridPosition, length: usize) -> bool {
        self.span_is_blank(position.row, 0, position.column)
            && self.span_is_blank(position.row, position.column + length, self.columns)
    }

    pub(super) fn row_text(&self, row: usize) -> String {
        self.rows
            .get(row)
            .into_iter()
            .flat_map(|cells| cells.iter().flatten())
            .collect()
    }

    /// `row` 是否除 `delimiter` 外别无他物。
    pub(super) fn row_is_only(&self, row: usize, delimiter: &[char]) -> bool {
        self.row_text(row)
            .trim()
            .chars()
            .eq(delimiter.iter().copied())
    }

    /// 行级版的源码证据测试，供跨空隙搜索为每一行单独担保。Codex 形状里
    /// 空隙后的行在渲染器眼中是列表项，因此其前导符号也被读作列表标记：
    /// `- 2xy` 整体不算证据，但标记后的紧凑操作数算，而
    /// `- 修复 SSH 重连`（真正的列表项）不算。
    pub(super) fn row_has_math_evidence(&self, row: usize) -> bool {
        let text = self.row_text(row);
        let text = text.trim();
        if standard_formula_source(text, true) {
            return true;
        }
        let body = text.trim_start_matches(['-', '+', '*']).trim_start();
        body.len() != text.len()
            && (implicit_product_operand(body) || standard_formula_source(body, true))
    }

    /// **行内**定界符的闭合搜索。行内 TeX 只有在该物理行由软折行产生时
    /// 才能跨行：真实换行仍终止跨度，因此两条独立终端行永远不会意外合并
    /// 成一条公式。
    pub(super) fn find_closing_soft_wrap(
        &self,
        mut position: GridPosition,
        delimiter: &[char],
    ) -> Option<GridPosition> {
        loop {
            if position.row >= self.rows.len() {
                return None;
            }
            if self.starts_with(position, delimiter) && !self.is_escaped(position) {
                return Some(position);
            }
            let previous_row = position.row;
            position = self.next(position)?;
            if position.row != previous_row && !self.wrapped[previous_row] {
                return None;
            }
        }
    }

    /// 对自身不携带意图的开头，闭合搜索带显式行预算**和**整帧 cell 预算。
    /// 裸 `(` 是普通终端输出里最密的字符（代码、日志、JSON），未闭合时
    /// 否则会扫到网格末尾——每个 `(` 每帧一次。50×200 未闭合括号网格实测
    /// 相对纯文本网格是 165 倍退化，因此预算不是可选项。`spent` 由帧内
    /// 所有裸候选共享。
    pub(super) fn find_closing_bounded(
        &self,
        mut position: GridPosition,
        rows: usize,
        spent: &mut usize,
        mut matcher: impl FnMut(&Self, GridPosition) -> Option<bool>,
    ) -> Option<GridPosition> {
        let last_row = position.row.saturating_add(rows);
        loop {
            if position.row >= self.rows.len() || position.row > last_row || *spent == 0 {
                return None;
            }
            *spent -= 1;
            if matcher(self, position)? {
                return Some(position);
            }
            position = self.next(position)?;
        }
    }

    pub(super) fn extract(&self, start: GridPosition, end: GridPosition) -> Option<Box<str>> {
        let mut output = String::new();
        let mut position = start;
        while position < end {
            if let Some(character) = self.character(position) {
                output.push(character);
                if output.len() > DEFAULT_LIMITS.max_source_bytes {
                    return None;
                }
            }

            let previous_row = position.row;
            position = self.next(position)?;
            if position.row != previous_row && !self.wrapped[previous_row] {
                output.push('\n');
            }
        }
        let trimmed = output.trim();
        (!trimmed.is_empty()).then(|| Box::<str>::from(trimmed))
    }

    pub(super) fn span_fingerprint(
        &self,
        row: usize,
        start: usize,
        end: usize,
        include_wrap: bool,
    ) -> Option<u64> {
        let cells = self.rows.get(row)?.get(start..end)?;
        let mut hasher = DefaultHasher::new();
        start.hash(&mut hasher);
        end.hash(&mut hasher);
        cells.hash(&mut hasher);
        if include_wrap {
            self.wrapped.get(row)?.hash(&mut hasher);
        }
        Some(hasher.finish())
    }

    /// 跨度当前是否只有空白。TUI 重绘会先清行再重画，因此空白跨度被视为
    /// 瞬态而非持久公式已消失的证据。
    pub(super) fn span_is_blank(&self, row: usize, start: usize, end: usize) -> bool {
        self.rows
            .get(row)
            .and_then(|cells| cells.get(start..end))
            .is_some_and(|cells| {
                cells
                    .iter()
                    .all(|cell| cell.is_none_or(|character| character == ' '))
            })
    }

    /// `row` 第一个与最后一个非空白 cell 之间的列；空白行返回 `None`。
    /// 内部空隙计为占用：把墨迹挤进上一行两个词之间的公式会被读成重叠，
    /// 即便那些 cell 技术上是空的。
    pub(super) fn row_ink_columns(&self, row: usize) -> Option<(usize, usize)> {
        let cells = self.rows.get(row)?;
        let occupied = |cell: &Option<char>| cell.is_some_and(|character| character != ' ');
        let start = cells.iter().position(occupied)?;
        let end = cells.iter().rposition(occupied)? + 1;
        Some((start, end))
    }

    pub(super) fn spans(&self, start: GridPosition, end: GridPosition) -> Vec<RowSpan> {
        if start.row == end.row {
            return vec![RowSpan {
                row: start.row as i32,
                start: start.column,
                end: end.column,
            }];
        }

        let mut spans = Vec::with_capacity(end.row - start.row + 1);
        spans.push(RowSpan {
            row: start.row as i32,
            start: start.column,
            end: self.columns,
        });
        for row in start.row + 1..end.row {
            spans.push(RowSpan {
                row: row as i32,
                start: 0,
                end: self.columns,
            });
        }
        spans.push(RowSpan {
            row: end.row as i32,
            start: 0,
            end: end.column,
        });
        spans
    }
}

impl PartialOrd for GridPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GridPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.row, self.column).cmp(&(other.row, other.column))
    }
}

/// 扫描可见网格，并附加渲染器已解析的颜色/兜底 cell。
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_visible<T>(
    state: &mut TerminalMathState,
    terminal: &Term<T>,
    size: &MathViewport,
    source_cells: &[OverlayCell],
    filter_reasoning_style: bool,
    cursor: Option<Point<usize>>,
    default_foreground: Rgb,
) -> Vec<FormulaOverlay> {
    let grid = TextGrid::from_term(terminal, size);
    let active_edit_rows = cursor.and_then(|cursor| grid.logical_rows_containing(cursor.line));
    state.synchronize_grid(&grid);
    if let Some(anchor) = state.scan_visible_grid(&grid, active_edit_rows.as_ref()) {
        // `extract` 把每个终端 cell 计入 16 KiB 解析上限。按当前宽度推导行上限，
        // 让这块临时网格保持在约 64 KiB cell 数据，而不是扫描全部回滚历史。
        let source_rows = DEFAULT_LIMITS
            .max_source_bytes
            .div_ceil(grid.columns.max(1))
            .saturating_add(4)
            .min(MAX_HISTORY_FORMULA_ROWS);
        // 视口锚点意味着 pending 定界符是孤立闭合，其开头在上方某处；深度
        // 未知，因此按完整源码预算回看，而不是精确距离。
        let lookback = if anchor.row >= grid.absolute_top {
            source_rows
        } else {
            grid.absolute_top - anchor.row
        };
        if lookback <= source_rows {
            let history = TextGrid::from_term_with_lookback(terminal, size, lookback);
            state.complete_pending_from_history(&history);
        } else {
            state.pending_display = None;
        }
    }

    let mut overlays: Vec<_> = state
        .visible_overlays(&grid)
        .into_iter()
        .filter(|overlay| {
            active_edit_rows
                .as_ref()
                .is_none_or(|rows| !overlay.intersects_rows(rows))
        })
        .filter_map(|mut overlay| {
            overlay.fallback = source_cells
                .iter()
                .filter(|cell| overlay.contains(cell.point))
                .cloned()
                .collect();

            // 非默认背景通常属于代码块或 TUI 自绘表面。在它上面绘制公式
            // 需要渲染器理解那个应用的布局，因此普通终端路径刻意保持源码
            // 不动。
            let has_ansi_background = overlay.fallback.iter().any(|cell| cell.bg_alpha > 0.0);
            if has_ansi_background {
                return None;
            }

            overlay.foreground = overlay
                .fallback
                .iter()
                .find(|cell| !cell.character.is_whitespace())
                .map_or(default_foreground, |cell| cell.fg);

            // Agent TUI 刻意用 dim 或降低透明度的前景渲染思维链，而最终
            // 答案使用普通终端前景。推理公式保留原文：把它替换成精致覆盖层
            // 会让内部工作看起来像答案的一部分。该启发式仅限全屏 TUI 调用方，
            // 普通 shell 里用户着色的公式永远不会被归为推理。
            if filter_reasoning_style && formula_uses_reasoning_style(&overlay) {
                return None;
            }

            apply_layout_hints(&mut overlay, &grid);
            Some(overlay)
        })
        .collect();
    mark_formula_neighbours(&mut overlays);
    overlays
}

/// 源字形是否携带 agent 推理使用的呈现。
///
/// Codex 与 Claude Code 用 SGR DIM 标记弱化推理。要求四分之三源字形一致，
/// 避免某个 token 恰好变暗就把最终答案归为推理。
pub(super) fn formula_uses_reasoning_style(overlay: &FormulaOverlay) -> bool {
    let (source_cells, dim_cells) = overlay
        .fallback
        .iter()
        .filter(|cell| !cell.character.is_whitespace())
        .fold((0usize, 0usize), |(total, dim), cell| {
            (
                total + 1,
                dim + usize::from(cell.flags.contains(Flags::DIM)),
            )
        });
    source_cells > 0 && dim_cells * 4 >= source_cells * 3
}

/// 周围网格可借给 overlay 的空间，纯粹由邻行内容决定。公式渲染后的尺寸
/// 不得取决于其源码恰好占了几行几列，因此一行里的 `$$x$$` 与同一源码铺成
/// 三行时，只要周围空白允许，预算相同。`scan_visible` 与测试共享这一个
/// 裁决，fit 与 clip 背后的数字始终一致。
pub(super) fn apply_layout_hints(overlay: &mut FormulaOverlay, grid: &TextGrid) {
    // 记录邻行内容；`prepare_overlays` 知道渲染宽度后再决定其中多少真正挡路。
    let (Some(first), Some(last)) = (overlay.spans.first(), overlay.spans.last()) else {
        return;
    };
    overlay.neighbours_above = neighbour_rows(grid, first.row, -1, overlay.display);
    overlay.neighbours_below = neighbour_rows(grid, last.row, 1, overlay.display);

    // 源跨度右侧整行为空的显示公式可以铺满行内剩余宽度，其渲染尺寸不再
    // 取决于源码占了几列。滚出视口的行无法检查；视为空白是安全的，因为
    // 只有可见行才可能显示水平碰撞。
    let source_right = overlay.spans.iter().map(|span| span.end).max().unwrap_or(0);
    overlay.widen_right_to = (overlay.display
        && overlay
            .spans
            .iter()
            .all(|span| match usize::try_from(span.row) {
                Ok(row) if row < grid.rows.len() => grid.span_is_blank(row, span.end, grid.columns),
                _ => true,
            }))
    .then_some(grid.columns.max(source_right));
}

/// 收集完所有可见 overlay 后，标记被**另一条**公式占用的行。原始终端行
/// 只说明有文字；这第二遍保留垂直裁剪所需的区分。
pub(super) fn mark_formula_neighbours(overlays: &mut [FormulaOverlay]) {
    let spans: Vec<Vec<(i32, usize, usize)>> = overlays
        .iter()
        .map(|overlay| {
            overlay
                .spans
                .iter()
                .map(|span| (span.row, span.start, span.end))
                .collect()
        })
        .collect();
    for (index, overlay) in overlays.iter_mut().enumerate() {
        let Some(first) = overlay.spans.first() else {
            continue;
        };
        let Some(last) = overlay.spans.last() else {
            continue;
        };
        let source_start = overlay
            .spans
            .iter()
            .map(|span| span.start)
            .min()
            .unwrap_or(first.start);
        let source_end = overlay
            .spans
            .iter()
            .map(|span| span.end)
            .max()
            .unwrap_or(last.end);

        for distance in 0..MAX_ABSORBED_BLANK_ROWS {
            let distance = (distance + 1) as i32;
            let above = first.row - distance;
            let below = last.row + distance;
            overlay.formula_neighbours_above[distance as usize - 1] =
                spans.iter().enumerate().any(|(other_index, other_spans)| {
                    other_index != index
                        && other_spans.iter().any(|&(row, start, end)| {
                            row == above && start < source_end && end > source_start
                        })
                });
            overlay.formula_neighbours_below[distance as usize - 1] =
                spans.iter().enumerate().any(|(other_index, other_spans)| {
                    other_index != index
                        && other_spans.iter().any(|&(row, start, end)| {
                            row == below && start < source_end && end > source_start
                        })
                });
        }
    }
}

/// 从 `from` 的下一步开始，[`MAX_ABSORBED_BLANK_ROWS`] 行的墨迹列，最近
/// 优先。视口之外的行对 display 数学视为空闲——clip 停在视口边缘，额外
/// 预算够不到任何东西——对行内数学视为占用，否则它会抬离共享一行的正文基线。
pub(super) fn neighbour_rows(
    grid: &TextGrid,
    from: i32,
    step: i32,
    display: bool,
) -> [Option<(usize, usize)>; MAX_ABSORBED_BLANK_ROWS] {
    std::array::from_fn(|index| {
        let row = from + step * (index as i32 + 1);
        match usize::try_from(row) {
            Ok(row) if row < grid.rows.len() => grid.row_ink_columns(row),
            _ => (!display).then_some((0, grid.columns)),
        }
    })
}

#[cfg(test)]
pub(super) fn scan_grid(grid: &TextGrid) -> Vec<FormulaOverlay> {
    scan_grid_result(grid).overlays
}

/// `scan_visible` 尾段的测试镜像：先扫描，再让邻行发放真实管线会发放的
/// 同一布局预算。
#[cfg(test)]
pub(super) fn scan_grid_with_hints(grid: &TextGrid) -> Vec<FormulaOverlay> {
    let mut overlays = scan_grid(grid);
    for overlay in &mut overlays {
        apply_layout_hints(overlay, grid);
    }
    mark_formula_neighbours(&mut overlays);
    overlays
}

#[derive(Clone)]
pub(super) struct GridScanResult {
    pub(super) overlays: Vec<FormulaOverlay>,
    pub(super) unmatched_display: Option<(GridPosition, DisplayDelimiterKind)>,
}

pub(super) fn scan_grid_result(grid: &TextGrid) -> GridScanResult {
    let mut overlays = Vec::new();
    let mut unmatched_display = None;
    let mut position = GridPosition { row: 0, column: 0 };
    // 只有裸定界符消费它：其余四类各自有 O(1) 前置闸。
    let mut bare_budget = BARE_SEARCH_CELL_BUDGET;

    while position.row < grid.rows.len() && overlays.len() < MAX_VISIBLE_FORMULAS {
        let (candidate, incomplete_display) =
            if grid.starts_with(position, &['$', '$']) && !grid.is_escaped(position) {
                let candidate = find_formula(
                    grid,
                    position,
                    &['$', '$'],
                    &['$', '$'],
                    DelimiterKind::DollarDisplay,
                );
                (candidate, Some(DisplayDelimiterKind::Dollars))
            } else if grid.starts_with(position, &['\\', '[']) && !grid.is_escaped(position) {
                let candidate = find_formula(
                    grid,
                    position,
                    &['\\', '['],
                    &['\\', ']'],
                    DelimiterKind::BracketDisplay,
                );
                (candidate, Some(DisplayDelimiterKind::Brackets))
            } else if grid.starts_with(position, &['\\', ']']) && !grid.is_escaped(position) {
                (None, Some(DisplayDelimiterKind::Brackets))
            } else if grid.starts_with(position, &['\\', '(']) && !grid.is_escaped(position) {
                (
                    find_formula(
                        grid,
                        position,
                        &['\\', '('],
                        &['\\', ')'],
                        DelimiterKind::Parenthesized,
                    ),
                    None,
                )
            } else if grid.character(position) == Some('[') && !grid.is_escaped(position) {
                (
                    find_bare_bracket_formula(grid, position, &mut bare_budget),
                    None,
                )
            } else if grid.character(position) == Some('(') && !grid.is_escaped(position) {
                (
                    find_bare_paren_formula(grid, position, &mut bare_budget),
                    None,
                )
            } else if grid.character(position) == Some('$') && !grid.is_escaped(position) {
                (find_dollar_formula(grid, position), None)
            } else {
                (None, None)
            };

        if let Some((overlay, after)) = candidate {
            overlays.push(overlay);
            position = after;
        } else if let Some(next) = grid.next(position) {
            if let Some(kind) = incomplete_display {
                unmatched_display = Some((position, kind));
            }
            position = next;
        } else {
            if let Some(kind) = incomplete_display {
                unmatched_display = Some((position, kind));
            }
            break;
        }
    }

    GridScanResult {
        overlays,
        unmatched_display,
    }
}

pub(super) fn find_formula(
    grid: &TextGrid,
    open: GridPosition,
    opening: &[char],
    closing: &[char],
    kind: DelimiterKind,
) -> Option<(FormulaOverlay, GridPosition)> {
    let source_start = grid.after(open, opening.len());
    // display 定界符拥有它与闭合之间的行，因此可跨真实换行——但只有当它
    // 像块级定界符那样**开启**本行。一行正文中间遇到的 `$$` 是被引用，
    // 不是被开启：agent 在复述问题，或句子提到了这个定界符。让它跨真实
    // 换行会与下一个真实块的**开头**配对，吞掉中间所有公式——扫描在匹配
    // 之后继续（见 `scan_grid_result`），那些行根本不会被考虑。
    // 行内定界符同理：跨过硬折行会把两条无关终端行合并成一条公式。
    let close = if kind.is_display() && opens_a_block(grid, open) {
        grid.find_closing(open, source_start, closing)?
    } else {
        grid.find_closing_soft_wrap(source_start, closing)?
    };
    let after = grid.after(close, closing.len());
    let source = grid.extract(source_start, close)?;
    let display = kind.is_display();
    if !standard_formula_source(&source, display) {
        return None;
    }
    Some((make_overlay(grid, open, after, source, kind), after))
}

/// display 定界符是否开启本行：它前面只有空白——或 Markdown 列表标记。
/// Agent 即使把后续公式硬折在同一行，也这样发出 `$$` / `\[`；而只是
/// 提到定界符的句子前面总是有正文。前缀就是全部判据：再要求空白尾会
/// 拒绝 `\[\displaystyle …` 跨行形态，而那是真实形状。镜像
/// [`find_bare_bracket_formula`] 对裸 `[` 块施加的独立规则的前半。
pub(super) fn opens_a_block(grid: &TextGrid, open: GridPosition) -> bool {
    span_is_blank_or_markdown_list_marker(grid, open.row, open.column)
}

pub(super) fn find_dollar_formula(
    grid: &TextGrid,
    open: GridPosition,
) -> Option<(FormulaOverlay, GridPosition)> {
    let source_start = grid.after(open, 1);
    if grid.character(source_start)? == '$' {
        return None;
    }
    // 第一个闭合候选失败就交回逐格扫描，不能越过后续公式把正文吞进同一源码。
    let close = find_inline_dollar_closing(grid, source_start)?;
    let next = grid
        .next(close)
        .and_then(|position| grid.character(position));
    if next.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_') {
        return None;
    }
    if close.row > source_start.row
        && !grid.wrapped[close.row - 1]
        && !inline_math_fragment(grid, close.row, 0, close.column)
    {
        return None;
    }
    let after = grid.after(close, 1);
    let source = grid.extract(source_start, close)?;
    standard_formula_source(&source, false).then(|| {
        (
            make_overlay(grid, open, after, source, DelimiterKind::DollarInline),
            after,
        )
    })
}

/// 寻找单美元闭合，不借用 `$$` display 定界符的任一字符。硬折行需要独立
/// 数学证据；空行与散文仍终止搜索。
pub(super) fn find_inline_dollar_closing(
    grid: &TextGrid,
    mut position: GridPosition,
) -> Option<GridPosition> {
    let start = position;
    let mut bytes = DEFAULT_LIMITS.max_source_bytes;
    let mut hard_rows = 0;
    loop {
        if position.row >= grid.rows.len() {
            return None;
        }
        bytes = bytes.checked_sub(grid.character(position).map_or(0, char::len_utf8))?;
        if grid.character(position) == Some('$') && !grid.is_escaped(position) {
            let previous_is_dollar = grid
                .previous(position)
                .and_then(|previous| grid.character(previous))
                == Some('$');
            let next_is_dollar =
                grid.next(position).and_then(|next| grid.character(next)) == Some('$');
            if !previous_is_dollar && !next_is_dollar {
                return Some(position);
            }
        }
        let previous_row = position.row;
        position = grid.next(position)?;
        if position.row != previous_row && !grid.wrapped[previous_row] {
            hard_rows += 1;
            let column = if previous_row == start.row {
                start.column
            } else {
                0
            };
            if hard_rows > 8 || !inline_math_fragment(grid, previous_row, column, grid.columns) {
                return None;
            }
        }
    }
}

fn inline_math_fragment(grid: &TextGrid, row: usize, start: usize, end: usize) -> bool {
    let source: String = grid.rows[row][start.min(grid.columns)..end.min(grid.columns)]
        .iter()
        .flatten()
        .collect();
    let source = source.trim();
    if source.is_empty() || obviously_non_math(source) {
        return false;
    }
    // TUI 的硬折行不带 WRAPLINE；只接数学片段，绝不跨空段或普通说明文字。
    let operand = source.trim_start_matches(['=', '+', '-', '*']).trim();
    standard_formula_source(source, true)
        || explicit_operand(operand)
        || implicit_product_operand(operand)
        || source
            .chars()
            .all(|character| matches!(character, '{' | '}'))
}

/// Markdown 未转义的 display 块：部分 AI CLI 让回答经过 markdown 渲染器，
/// 后者吃掉 `\[` / `\]` / `\,` 的反斜杠（它们是 markdown 标点转义），而
/// `\int`、`\frac` ……幸存，屏幕上只剩裸 `[` 块。裸括号本身没有数学意图——
/// JSON、数组、`[INFO]` 日志都用它——因此形态要求比 `\[` 更严格：`[` 必须
/// 起行，`]` 必须收行。多行块还携带足够呈现意图，可接受 `E = mc^2` 这类
/// 普通数学结构；紧凑单行形态仍要求已知 TeX 命令，避免 `[x^2]` 变成覆盖层。
pub(super) fn find_bare_bracket_formula(
    grid: &TextGrid,
    open: GridPosition,
    budget: &mut usize,
) -> Option<(FormulaOverlay, GridPosition)> {
    if !span_is_blank_or_markdown_list_marker(grid, open.row, open.column) {
        return None;
    }
    let source_start = grid.after(open, 1);
    let multiline = grid.span_is_blank(open.row, open.column + 1, grid.columns);
    let close = if multiline {
        // 多行形态：`[` 独占一行，闭合 `]` 也必须独占一行。数学源码里
        // `[0, 1]` 区间的 `]` 不具备闭合资格，跳过继续找。搜索同样带行
        // 预算：屏幕底部一个没闭合的 `[` 不该每帧扫到网格末尾。空行按
        // 与 `find_closing` 同一条理由收尾（数学模式里没有分段），否则一个
        // 没闭合的 `[` 会把下面一整段正文连同它自己的公式吞成一条候选——
        // 也按同一套条件放行 TUI 塞进来的那一行空行：吃掉 `\[` 的正是
        // 这个 Markdown 渲染器，它切开 `$$` 块的手法在这里一模一样。
        let mut content_rows = 0usize;
        let mut gap_pending = false;
        grid.find_closing_bounded(
            source_start,
            BARE_BRACKET_SEARCH_ROWS,
            budget,
            |grid, position| {
                if position.column == 0 && position.row > open.row {
                    if grid.span_is_blank(position.row, 0, grid.columns) {
                        if content_rows == 0 || gap_pending {
                            return None;
                        }
                        gap_pending = true;
                        return Some(false);
                    }
                    content_rows += 1;
                    if std::mem::take(&mut gap_pending)
                        && !grid.row_is_only(position.row, &[']'])
                        && !grid.row_has_math_evidence(position.row)
                    {
                        return None;
                    }
                }
                if grid.character(position) != Some(']') || grid.is_escaped(position) {
                    return Some(false);
                }
                Some(grid.delimiter_owns_row(position, 1))
            },
        )?
    } else {
        // 单行形态 `[ … ]`：闭合必须是本行最后一个非空白字符。
        let cells = grid.rows.get(open.row)?;
        let last = cells
            .iter()
            .rposition(|cell| cell.is_some_and(|c| c != ' '))?;
        let close = GridPosition {
            row: open.row,
            column: last,
        };
        if last <= open.column || grid.character(close) != Some(']') {
            return None;
        }
        close
    };
    let after = grid.after(close, 1);
    let source = grid.extract(source_start, close)?;
    if !bare_formula_source(&source, true) {
        return None;
    }
    Some((
        make_overlay(grid, open, after, source, DelimiterKind::BareBracketDisplay),
        after,
    ))
}

/// Codex 风格的 Markdown 渲染器保留列表圆点，却去掉了 `\[` / `\]` 的
/// 反斜杠，得到 `• [` 后跟 TeX 与缩进的 `]`。列表标记纯属呈现，因此可以
/// 位于本来独立的开括号之前。其他前缀仍拒绝候选（JSON、日志、shell 提示
/// 符与散文保持原文）。
pub(super) fn span_is_blank_or_markdown_list_marker(
    grid: &TextGrid,
    row: usize,
    end: usize,
) -> bool {
    if grid.span_is_blank(row, 0, end) {
        return true;
    }
    let marker: String = grid
        .rows
        .get(row)
        .into_iter()
        .flat_map(|cells| cells.iter().take(end).flatten())
        .collect();
    matches!(marker.trim(), "•" | "-" | "*" | "+")
}

/// Markdown 未转义的行内公式：`\( … \)` 被剥成裸括号。裸括号是散文标点，
/// 因此通常要求已知 TeX 命令（`(\sqrt{…})`）。Codex 也可能剥掉紧凑方程
/// `(E=mc^2)` 的定界符；这种狭窄数学形态合格，正则与普通散文保持原文。
/// 闭合按括号深度配对，`(\sin(x))` 保留内层 `)`。
pub(super) fn find_bare_paren_formula(
    grid: &TextGrid,
    open: GridPosition,
    budget: &mut usize,
) -> Option<(FormulaOverlay, GridPosition)> {
    let source_start = grid.after(open, 1);
    let mut depth = 1usize;
    let close = grid.find_closing_bounded(
        source_start,
        BARE_PAREN_SEARCH_ROWS,
        budget,
        |grid, position| {
            match grid.character(position) {
                Some('(') if !grid.is_escaped(position) => depth += 1,
                Some(')') if !grid.is_escaped(position) => {
                    depth -= 1;
                    return Some(depth == 0);
                }
                _ => {}
            }
            Some(false)
        },
    )?;
    let after = grid.after(close, 1);
    let source = grid.extract(source_start, close)?;
    if !bare_formula_source(&source, false) {
        return None;
    }
    Some((
        make_overlay(grid, open, after, source, DelimiterKind::BareParenInline),
        after,
    ))
}

/// 对周围上下文尚不能识别 AI 客户端的定界符，采用严格的紧凑方程兜底。
/// 要求等号加算术/指数标记或短隐式乘积，避免把 `key=value` 这类配置散文
/// 渲染出来。
pub(super) fn looks_like_compact_equation(source: &str) -> bool {
    let source = compact_equation_source(source);
    let compact_syntax = source.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character.is_ascii_whitespace()
            || matches!(
                character,
                '=' | '+' | '-' | '*' | '/' | '^' | '_' | '{' | '}' | '(' | ')' | '.' | ','
            )
    });
    if !compact_syntax {
        return false;
    }

    let Some((left, right)) = source.split_once('=') else {
        return false;
    };
    if right.contains('=') {
        return false;
    }

    source.chars().any(|character| {
        character.is_ascii_digit() || matches!(character, '+' | '-' | '*' | '/' | '^' | '_')
    }) || looks_like_implicit_product_equation(left, right)
}

pub(super) fn compact_equation_source(source: &str) -> &str {
    let source = source.trim();
    // 呈现命令不改变后续源码是否是紧凑方程。Markdown 渲染器可能在消费原始
    // `\(` / `\)` 定界符后把这个命令留在裸括号里。
    source
        .strip_prefix(r"\displaystyle")
        .map(str::trim_start)
        .unwrap_or(source)
}

/// 识别习惯省略乘号的紧凑方程，如 `F=ma` 与 `PV=nRT`。要求短标识符加
/// 单符号或大写变量，把 `key=value` 这类配置散文挡在公式路径之外。
pub(super) fn looks_like_implicit_product_equation(left: &str, right: &str) -> bool {
    fn compact_identifier(source: &str) -> Option<(usize, bool)> {
        let source = source.trim();
        let count = source.chars().count();
        if !(1..=3).contains(&count)
            || !source
                .chars()
                .all(|character| character.is_ascii_alphabetic())
        {
            return None;
        }
        Some((
            count,
            source
                .chars()
                .any(|character| character.is_ascii_uppercase()),
        ))
    }

    let Some((left_count, left_uppercase)) = compact_identifier(left) else {
        return false;
    };
    let Some((right_count, right_uppercase)) = compact_identifier(right) else {
        return false;
    };

    left_count == 1 || right_count == 1 || left_uppercase || right_uppercase
}

/// 足以把裸定界符视为数学的命令白名单。刻意用白名单而不是"任意 `\` + 字母"：
/// 正则转义（`\d`、`\w`）、C 转义与 Windows 路径都符合宽松形状。单字母
/// 命令由这里的长度检查整体排除（每个条目 ≥2 字符）。
const KNOWN_TEX_COMMANDS: &[&str] = &[
    "alpha",
    "approx",
    "arccos",
    "arcsin",
    "arctan",
    "bar",
    "begin",
    "beta",
    "big",
    "bigg",
    "binom",
    "boldsymbol",
    "bullet",
    "cap",
    "cdot",
    "cdots",
    "chi",
    "circ",
    "cos",
    "cosh",
    "cot",
    "csc",
    "cup",
    "ddot",
    "ddots",
    "delta",
    "det",
    "dfrac",
    "div",
    "dot",
    "dots",
    "emptyset",
    "end",
    "epsilon",
    "equiv",
    "eta",
    "exists",
    "exp",
    "forall",
    "frac",
    "gamma",
    "gcd",
    "ge",
    "geq",
    "gg",
    "hat",
    "iff",
    "iiint",
    "iint",
    "implies",
    "in",
    "inf",
    "infty",
    "int",
    "iota",
    "kappa",
    "lambda",
    "land",
    "langle",
    "lceil",
    "ldots",
    "le",
    "left",
    "leftarrow",
    "leftrightarrow",
    "leq",
    "lfloor",
    "lg",
    "lim",
    "liminf",
    "limsup",
    "ll",
    "ln",
    "log",
    "lor",
    "mapsto",
    "mathbb",
    "mathbf",
    "mathcal",
    "mathfrak",
    "mathit",
    "mathrm",
    "mathsf",
    "max",
    "min",
    "mp",
    "mu",
    "nabla",
    "ne",
    "neg",
    "neq",
    "notin",
    "nu",
    "odot",
    "oint",
    "omega",
    "ominus",
    "operatorname",
    "oplus",
    "otimes",
    "overbrace",
    "overline",
    "partial",
    "phi",
    "pi",
    "pm",
    "prod",
    "propto",
    "psi",
    "qquad",
    "quad",
    "rangle",
    "rceil",
    "rfloor",
    "rho",
    "right",
    "rightarrow",
    "sec",
    "sigma",
    "sim",
    "simeq",
    "sin",
    "sinh",
    "sqrt",
    "subset",
    "subseteq",
    "sum",
    "sup",
    "supset",
    "supseteq",
    "tan",
    "tanh",
    "tau",
    "text",
    "textbf",
    "textit",
    "textrm",
    "theta",
    "tilde",
    "times",
    "to",
    "underbrace",
    "underline",
    "upsilon",
    "varepsilon",
    "varnothing",
    "varphi",
    "varpi",
    "varrho",
    "varsigma",
    "vartheta",
    "vec",
    "vert",
    "widehat",
    "widetilde",
    "xi",
    "zeta",
    "Big",
    "Bigg",
    "Delta",
    "Gamma",
    "Lambda",
    "Leftarrow",
    "Leftrightarrow",
    "Omega",
    "Phi",
    "Pi",
    "Psi",
    "Rightarrow",
    "Sigma",
    "Theta",
    "Upsilon",
    "Vert",
    "Xi",
];

/// `source` 是否至少含一个白名单 TeX 命令。裸定界符没有 `\[` / `\(` 的
/// 意图声明背书，因此必须出现真实数学命令作为证据才可渲染。
pub(super) fn has_known_tex_command(source: &str) -> bool {
    let mut rest = source;
    while let Some(index) = rest.find('\\') {
        rest = &rest[index + 1..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        if end > 1 && KNOWN_TEX_COMMANDS.contains(&&rest[..end]) {
            return true;
        }
        rest = &rest[end..];
    }
    false
}

/// 定界符声明意图，内容提供证据。
///
/// 只有意图声明不够：终端里满是作为 shell sigil、提示符和价格的美元符号，
/// 任何非空跨度都接受的话，`echo $HOME $USER` 就变成公式。Display 块
/// （`$$…$$`、`\[…\]`）很少出现在真实数学之外，因此宽松证据足够；行内
/// `$…$` 与 `\(…\)` 会撞上货币、shell 变量和 BRE 捕获组，所以证据必须
/// 结构紧凑。
pub(super) fn standard_formula_source(source: &str, display: bool) -> bool {
    let source = source.trim();
    // 成对定界符中的数字与单字母都是公式；不能被货币字母白名单误伤。
    if source.parse::<f64>().is_ok_and(f64::is_finite)
        || source.chars().count() == 1 && source.chars().all(char::is_alphabetic)
    {
        return true;
    }
    !obviously_non_math(source) && has_math_evidence(source, display)
}

/// 无论定界符强弱都绝不能渲染成数学的终端噪声：注释/URL（`//`）、
/// Windows 路径（`:\`）、控制字节、货币金额与全大写 shell 变量。
pub(super) fn obviously_non_math(source: &str) -> bool {
    if source.is_empty()
        || source.contains("//")
        || source.contains(":\\")
        || source
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return true;
    }

    // 货币与 shell 变量是美元符号在终端里的主要用法。
    let currency = source.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_ascii_whitespace()
            || ".,EURUSDCNYGBPJPY".contains(character)
    });
    let shell_identifier = source.chars().all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    });
    currency || (shell_identifier && source.chars().count() > 1)
}

/// 至少一个结构性数学迹象。`lax`（display 块）放宽脚本基底的紧凑操作数
/// 要求，让 `mc^2` 这类隐式乘积合格；行内保持该要求，使 `$foo^bar$` 仍是
/// 普通文本。
pub(super) fn has_math_evidence(source: &str, lax: bool) -> bool {
    let chars: Vec<_> = source.chars().collect();
    let single_variable = chars.len() == 1 && chars[0].is_alphabetic();
    let tex_command = chars
        .windows(2)
        .any(|pair| pair[0] == '\\' && pair[1].is_alphabetic());
    let script = source.find(['^', '_']).is_some_and(|index| {
        let (base, suffix) = source.split_at(index);
        let suffix = &suffix[1..];
        let base_qualifies = if lax {
            !base.trim().is_empty()
        } else {
            explicit_operand(base)
        };
        base_qualifies && !suffix.trim().is_empty()
    });
    let relation = ["<=", ">=", "!=", "==", "=", "<", ">"]
        .into_iter()
        .any(|operator| {
            source.find(operator).is_some_and(|index| {
                relation_operand(&source[..index])
                    && relation_operand(&source[index + operator.len()..])
            })
        });
    // `n!` / `\binom` 的裸写法：阶乘是后缀运算符，本身就是数学证据。
    let factorial = source
        .trim_end()
        .strip_suffix('!')
        .is_some_and(|base| explicit_operand(base) || implicit_product_operand(base));
    let structural = script
        || relation
        || factorial
        || source.chars().any(|character| {
            matches!(
                character,
                '±' | '×'
                    | '÷'
                    | '√'
                    | '∑'
                    | '∏'
                    | '∫'
                    | '∞'
                    | '≈'
                    | '≠'
                    | '≤'
                    | '≥'
                    | '∂'
                    | '∇'
                    | '∈'
                    | '∉'
                    | '⊂'
                    | '⊆'
                    | '∪'
                    | '∩'
                    | '→'
                    | '↦'
            )
        });
    let known_function = source
        .split(|character: char| !character.is_alphabetic())
        .any(|word| {
            matches!(
                word.to_ascii_lowercase().as_str(),
                "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "det" | "max" | "min"
            )
        });
    let function_application = source.find('(').is_some_and(|open| {
        let name = source[..open].trim();
        let arguments = source[open + 1..].strip_suffix(')').unwrap_or("").trim();
        name.chars().count() == 1 && name.chars().all(char::is_alphabetic) && !arguments.is_empty()
    });
    let parenthesized_variable = source
        .strip_prefix('(')
        .and_then(|source| source.strip_suffix(')'))
        .or_else(|| {
            source
                .strip_prefix('[')
                .and_then(|source| source.strip_suffix(']'))
        })
        .is_some_and(|body| body.split(',').all(explicit_operand));
    let compact_operator = ['+', '-', '*', '/'].into_iter().any(|operator| {
        source.find(operator).is_some_and(|index| {
            let (left, right) = source.split_at(index);
            let right = &right[operator.len_utf8()..];
            explicit_operand(left) && explicit_operand(right)
        })
    });

    single_variable
        || tex_command
        || structural
        || known_function
        || function_application
        || parenthesized_variable
        || compact_operator
}

/// 单个数学操作数：至多一个变量，可选带脚本。多字母串被拒绝，因此
/// `$foo^bar$` 与 `key=value` 保持文本。
pub(super) fn explicit_operand(operand: &str) -> bool {
    let operand = operand.trim().trim_matches(['(', ')', '[', ']', '{', '}']);
    if operand.is_empty() || operand.contains(char::is_whitespace) {
        return false;
    }
    let alphabetic = operand
        .chars()
        .filter(|character| character.is_alphabetic())
        .count();
    alphabetic <= 1
        && operand.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, '.' | ',' | '\\' | '^' | '_')
        })
}

/// 物理写法省略乘号：`mc^2`、`nRT`、`ma`。[`explicit_operand`] 把标识符
/// 卡在一个字母，因此会拒绝它们全部——这就是 `$E=mc^2$` 过去保持原文的
/// 原因。这里允许最多三个字母——仍不允许空白，也不允许脚本之外的标点，
/// 于是 `PATH=/tmp`（斜杠）、`key=value`（五个字母）与 `npm install`
/// （空白）都留在公式路径之外。
pub(super) fn implicit_product_operand(operand: &str) -> bool {
    let operand = operand.trim().trim_matches(['(', ')', '[', ']', '{', '}']);
    if operand.is_empty() || operand.contains(char::is_whitespace) {
        return false;
    }
    let alphabetic = operand
        .chars()
        .filter(|character| character.is_alphabetic())
        .count();
    (1..=3).contains(&alphabetic)
        && operand
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '^' | '_' | '.'))
}

/// 关系符两侧也可以是函数调用（`f(x)=0`）或隐式乘积（`E=mc^2`），
/// 二者都不是单个操作数。
pub(super) fn relation_operand(operand: &str) -> bool {
    let operand = operand.trim();
    explicit_operand(operand)
        || implicit_product_operand(operand)
        || operand.contains(['+', '-'])
            && operand
                .trim_start_matches(['+', '-'])
                .split(['+', '-'])
                .all(|term| explicit_operand(term) || implicit_product_operand(term))
        || operand.find('(').is_some_and(|open| {
            let name = operand[..open].trim();
            let arguments = operand[open + 1..].strip_suffix(')').unwrap_or("").trim();
            name.chars().count() == 1
                && name.chars().all(char::is_alphabetic)
                && !arguments.is_empty()
        })
}

/// Markdown 消费定界符反斜杠后，裸方括号或圆括号没有显式 TeX 意图。
/// 只有剩余源码仍携带 TeX/方程证据时才恢复，且绝不因为包含白名单命令名
/// 就把 Windows 路径重新解释成公式。
///
/// 裸定界符同时服从两层：上面的标准证据，以及自身更严格的要求——已知命令
/// 或紧凑方程。
pub(super) fn bare_formula_source(source: &str, display: bool) -> bool {
    standard_formula_source(source, display)
        && !source.contains(":\\")
        && (has_known_tex_command(source) || looks_like_compact_equation(source))
}

pub(super) fn make_overlay(
    grid: &TextGrid,
    open: GridPosition,
    after: GridPosition,
    source: Box<str>,
    kind: DelimiterKind,
) -> FormulaOverlay {
    let display = kind.is_display();
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    kind.hash(&mut hasher);
    grid.absolute_top.saturating_add(open.row).hash(&mut hasher);
    open.column.hash(&mut hasher);
    let formula_id = hasher.finish();
    FormulaOverlay {
        source: Arc::from(source),
        display,
        formula_id,
        spans: grid.spans(open, after),
        foreground: Rgb::default(),
        fallback: Vec::new(),
        neighbours_above: [NEIGHBOUR_UNKNOWN; MAX_ABSORBED_BLANK_ROWS],
        neighbours_below: [NEIGHBOUR_UNKNOWN; MAX_ABSORBED_BLANK_ROWS],
        formula_neighbours_above: [false; MAX_ABSORBED_BLANK_ROWS],
        formula_neighbours_below: [false; MAX_ABSORBED_BLANK_ROWS],
        widen_right_to: None,
    }
}
