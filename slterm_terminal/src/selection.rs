//! State management for a selection in the grid.
//!
//! A selection should start when the mouse is clicked, and it should be
//! finalized when the button is released. The selection should be cleared
//! when text is added/removed/scrolled on the screen. The selection should
//! also be cleared if the user clicks off of the selection.

use std::cmp::min;
use std::mem;
use std::ops::{Bound, Range, RangeBounds};

use crate::event::EventListener;
use crate::grid::{Dimensions, GridCell, Indexed, Scroll};
use crate::index::{Boundary, Column, Line, Point, Side};
use crate::term::Term;
use crate::term::cell::{Cell, Flags, LineLength};
use vte::ansi::CursorShape;

/// A Point and side within that point.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Anchor {
    point: Point,
    side: Side,
}

impl Anchor {
    fn new(point: Point, side: Side) -> Anchor {
        Anchor { point, side }
    }
}

/// Represents a range of selected cells.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SelectionRange {
    /// Start point, top left of the selection.
    pub start: Point,
    /// End point, bottom right of the selection.
    pub end: Point,
    /// Whether this selection is a block selection.
    pub is_block: bool,
}

impl SelectionRange {
    pub fn new(start: Point, end: Point, is_block: bool) -> Self {
        assert!(start <= end);
        Self {
            start,
            end,
            is_block,
        }
    }
}

impl SelectionRange {
    /// Check if a point lies within the selection.
    pub fn contains(&self, point: Point) -> bool {
        self.start.line <= point.line
            && self.end.line >= point.line
            && (self.start.column <= point.column
                || (self.start.line != point.line && !self.is_block))
            && (self.end.column >= point.column || (self.end.line != point.line && !self.is_block))
    }

    /// Check if the cell at a point is part of the selection.
    pub fn contains_cell(
        &self,
        indexed: &Indexed<&Cell>,
        point: Point,
        shape: CursorShape,
    ) -> bool {
        // Do not invert block cursor at selection boundaries.
        if shape == CursorShape::Block
            && point == indexed.point
            && (self.start == indexed.point
                || self.end == indexed.point
                || (self.is_block
                    && ((self.start.line == indexed.point.line
                        && self.end.column == indexed.point.column)
                        || (self.end.line == indexed.point.line
                            && self.start.column == indexed.point.column))))
        {
            return false;
        }

        // Point itself is selected.
        if self.contains(indexed.point) {
            return true;
        }

        // Check if a wide char's trailing spacer is selected.
        indexed.cell.flags().contains(Flags::WIDE_CHAR)
            && self.contains(Point::new(indexed.point.line, indexed.point.column + 1))
    }
}

/// Different kinds of selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SelectionType {
    Simple,
    Block,
    Semantic,
    Lines,
}

/// 键盘扩选方向；语义按网格行列与可视页计算，不依赖前台布局。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SelectionMotion {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// 前端发往所属 worker 的选区命令；不共享或锁定实时网格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionCommand {
    Set(Option<Selection>),
    Update { point: Point, side: Side },
    Extend(SelectionMotion),
    SelectAll,
    Clear,
}

/// Describes a region of a 2-dimensional area.
///
/// Used to track a text selection. There are four supported modes, each with its own constructor:
/// [`simple`], [`block`], [`semantic`], and [`lines`]. The [`simple`] mode precisely tracks which
/// cells are selected without any expansion. [`block`] will select rectangular regions.
/// [`semantic`] mode expands the initial selection to the nearest semantic escape char in either
/// direction. [`lines`] will always select entire lines.
///
/// Calls to [`update`] operate different based on the selection kind. The [`simple`] and [`block`]
/// mode do nothing special, simply track points and sides. [`semantic`] will continue to expand
/// out to semantic boundaries as the selection point changes. Similarly, [`lines`] will always
/// expand the new point to encompass entire lines.
///
/// [`simple`]: enum.Selection.html#method.simple
/// [`block`]: enum.Selection.html#method.block
/// [`semantic`]: enum.Selection.html#method.semantic
/// [`lines`]: enum.Selection.html#method.lines
/// [`update`]: enum.Selection.html#method.update
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub ty: SelectionType,
    region: Range<Anchor>,
}

impl Selection {
    pub fn new(ty: SelectionType, location: Point, side: Side) -> Selection {
        Self {
            region: Range {
                start: Anchor::new(location, side),
                end: Anchor::new(location, side),
            },
            ty,
        }
    }

    /// Update the end of the selection.
    pub fn update(&mut self, point: Point, side: Side) {
        self.region.end = Anchor::new(point, side);
    }

    pub fn rotate<D: Dimensions>(
        mut self,
        dimensions: &D,
        range: &Range<Line>,
        delta: i32,
    ) -> Option<Selection> {
        let bottommost_line = dimensions.bottommost_line();
        let range_bottom = range.end;
        let range_top = range.start;

        let (mut start, mut end) = (&mut self.region.start, &mut self.region.end);
        if start.point > end.point {
            mem::swap(&mut start, &mut end);
        }

        // Rotate start of selection.
        if (start.point.line >= range_top || range_top == 0) && start.point.line < range_bottom {
            start.point.line = min(start.point.line - delta, bottommost_line);

            // If end is within the same region, delete selection once start rotates out.
            if start.point.line >= range_bottom && end.point.line < range_bottom {
                return None;
            }

            // Clamp selection to start of region.
            if start.point.line < range_top && range_top != 0 {
                if self.ty != SelectionType::Block {
                    start.point.column = Column(0);
                    start.side = Side::Left;
                }
                start.point.line = range_top;
            }
        }

        // Rotate end of selection.
        if (end.point.line >= range_top || range_top == 0) && end.point.line < range_bottom {
            end.point.line = min(end.point.line - delta, bottommost_line);

            // Delete selection if end has overtaken the start.
            if end.point.line < start.point.line {
                return None;
            }

            // Clamp selection to end of region.
            if end.point.line >= range_bottom {
                if self.ty != SelectionType::Block {
                    end.point.column = dimensions.last_column();
                    end.side = Side::Right;
                }
                end.point.line = range_bottom - 1;
            }
        }

        Some(self)
    }

    pub fn is_empty(&self) -> bool {
        match self.ty {
            SelectionType::Simple => {
                let (mut start, mut end) = (self.region.start, self.region.end);
                if start.point > end.point {
                    mem::swap(&mut start, &mut end);
                }

                // Simple selection is empty when the points are identical
                // or two adjacent cells have the sides right -> left.
                start == end
                    || (start.side == Side::Right
                        && end.side == Side::Left
                        && (start.point.line == end.point.line)
                        && start.point.column + 1 == end.point.column)
            }
            SelectionType::Block => {
                let (start, end) = (self.region.start, self.region.end);

                // Block selection is empty when the points' columns and sides are identical
                // or two cells with adjacent columns have the sides right -> left,
                // regardless of their lines
                (start.point.column == end.point.column && start.side == end.side)
                    || (start.point.column + 1 == end.point.column
                        && start.side == Side::Right
                        && end.side == Side::Left)
                    || (end.point.column + 1 == start.point.column
                        && start.side == Side::Left
                        && end.side == Side::Right)
            }
            SelectionType::Semantic | SelectionType::Lines => false,
        }
    }

    /// Check whether selection contains any point in a given range.
    pub fn intersects_range<R: RangeBounds<Line>>(&self, range: R) -> bool {
        let mut start = self.region.start.point.line;
        let mut end = self.region.end.point.line;

        if start > end {
            mem::swap(&mut start, &mut end);
        }

        let range_top = match range.start_bound() {
            Bound::Included(&range_start) => range_start,
            Bound::Excluded(&range_start) => range_start + 1,
            Bound::Unbounded => Line(i32::MIN),
        };

        let range_bottom = match range.end_bound() {
            Bound::Included(&range_end) => range_end,
            Bound::Excluded(&range_end) => range_end - 1,
            Bound::Unbounded => Line(i32::MAX),
        };

        range_bottom >= start && range_top <= end
    }

    /// Expand selection sides to include all cells.
    pub fn include_all(&mut self) {
        let (start, end) = (self.region.start.point, self.region.end.point);
        let (start_side, end_side) = match self.ty {
            SelectionType::Block
                if start.column > end.column
                    || (start.column == end.column && start.line > end.line) =>
            {
                (Side::Right, Side::Left)
            }
            SelectionType::Block => (Side::Left, Side::Right),
            _ if start > end => (Side::Right, Side::Left),
            _ => (Side::Left, Side::Right),
        };

        self.region.start.side = start_side;
        self.region.end.side = end_side;
    }

    /// Convert selection to grid coordinates.
    pub fn to_range<T>(&self, term: &Term<T>) -> Option<SelectionRange> {
        let grid = term.grid();
        let columns = grid.columns();

        // Order start above the end.
        let mut start = self.region.start;
        let mut end = self.region.end;

        if start.point > end.point {
            mem::swap(&mut start, &mut end);
        }

        // Clamp selection to within grid boundaries.
        if end.point.line < term.topmost_line() {
            return None;
        }
        start.point = start.point.grid_clamp(term, Boundary::Grid);
        end.point = end.point.grid_clamp(term, Boundary::Grid);

        match self.ty {
            SelectionType::Simple => self.range_simple(start, end, columns),
            SelectionType::Block => self.range_block(start, end),
            SelectionType::Semantic => Some(Self::range_semantic(term, start.point, end.point)),
            SelectionType::Lines => Some(Self::range_lines(term, start.point, end.point)),
        }
    }

    fn range_semantic<T>(term: &Term<T>, mut start: Point, mut end: Point) -> SelectionRange {
        if start == end
            && let Some(matching) = term.bracket_search(start)
        {
            if (matching.line == start.line && matching.column < start.column)
                || (matching.line < start.line)
            {
                start = matching;
            } else {
                end = matching;
            }

            return SelectionRange {
                start,
                end,
                is_block: false,
            };
        }

        let start = term.semantic_search_left(start);
        let end = term.semantic_search_right(end);

        SelectionRange {
            start,
            end,
            is_block: false,
        }
    }

    fn range_lines<T>(term: &Term<T>, start: Point, end: Point) -> SelectionRange {
        let start = term.line_search_left(start);
        let end = term.line_search_right(end);

        SelectionRange {
            start,
            end,
            is_block: false,
        }
    }

    fn range_simple(
        &self,
        mut start: Anchor,
        mut end: Anchor,
        columns: usize,
    ) -> Option<SelectionRange> {
        if self.is_empty() {
            return None;
        }

        // Remove last cell if selection ends to the left of a cell.
        if end.side == Side::Left && start.point != end.point {
            // Special case when selection ends to left of first cell.
            if end.point.column == 0 {
                end.point.column = Column(columns - 1);
                end.point.line -= 1;
            } else {
                end.point.column -= 1;
            }
        }

        // Remove first cell if selection starts at the right of a cell.
        if start.side == Side::Right && start.point != end.point {
            start.point.column += 1;

            // Wrap to next line when selection starts to the right of last column.
            if start.point.column == columns {
                start.point.column = Column(0);
                start.point.line += 1;
            }
        }

        Some(SelectionRange {
            start: start.point,
            end: end.point,
            is_block: false,
        })
    }

    fn range_block(&self, mut start: Anchor, mut end: Anchor) -> Option<SelectionRange> {
        if self.is_empty() {
            return None;
        }

        // Always go top-left -> bottom-right.
        if start.point.column > end.point.column {
            mem::swap(&mut start.side, &mut end.side);
            mem::swap(&mut start.point.column, &mut end.point.column);
        }

        // Remove last cell if selection ends to the left of a cell.
        if end.side == Side::Left && start.point != end.point && end.point.column.0 > 0 {
            end.point.column -= 1;
        }

        // Remove first cell if selection starts at the right of a cell.
        if start.side == Side::Right && start.point != end.point {
            start.point.column += 1;
        }

        Some(SelectionRange {
            start: start.point,
            end: end.point,
            is_block: true,
        })
    }
}

impl<T> Term<T> {
    /// 选中回滚与当前屏幕的全部文本；备用屏没有回滚，等价于选中整屏。
    ///
    /// 首尾空白行不计入选区，末列取该行最后一个占用格：全选后复制
    /// 不会凭空多出尾随换行，粘贴时也不会误触执行。
    pub fn select_all(&mut self) {
        let grid = self.grid();
        let mut start_line = grid.topmost_line();
        let mut end_line = grid.bottommost_line();
        while start_line < end_line && grid[start_line].line_length().0 == 0 {
            start_line += 1;
        }
        while end_line > start_line && grid[end_line].line_length().0 == 0 {
            end_line -= 1;
        }
        if grid[end_line].line_length().0 == 0 {
            self.selection = None;
            return;
        }

        let last_column = Column(grid[end_line].line_length().0 - 1);
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(start_line, Column(0)),
            Side::Left,
        );
        selection.update(Point::new(end_line, last_column), Side::Right);
        self.selection = Some(selection);
    }
}

impl<T: EventListener> Term<T> {
    /// 键盘扩选：无有效选区时从光标起选，否则移动端点。
    ///
    /// 端点按网格行列移动并在真实边界内夹紧；移动后把端点滚入视口，
    /// 备用屏没有回滚，自然被限制在当前画面。
    pub fn extend_selection(&mut self, motion: SelectionMotion) {
        let (top_line, bottom_line, last_column, screen_lines, cursor) = {
            let grid = self.grid();
            (
                grid.topmost_line(),
                grid.bottommost_line(),
                grid.last_column(),
                grid.screen_lines() as i32,
                grid.cursor.point,
            )
        };

        let (anchor_point, mut point) = match self.selection.as_ref() {
            Some(selection) if !selection.is_empty() => {
                (selection.region.start.point, selection.region.end.point)
            }
            _ => (cursor, cursor),
        };

        match motion {
            SelectionMotion::Left => {
                if point.column.0 > 0 {
                    point.column -= 1;
                } else if point.line > top_line {
                    point.line -= 1;
                    point.column = last_column;
                }
            }
            SelectionMotion::Right => {
                if point.column < last_column {
                    point.column += 1;
                } else if point.line < bottom_line {
                    point.line += 1;
                    point.column = Column(0);
                }
            }
            SelectionMotion::Up => point.line -= 1,
            SelectionMotion::Down => point.line += 1,
            SelectionMotion::PageUp => point.line -= screen_lines,
            SelectionMotion::PageDown => point.line += screen_lines,
            SelectionMotion::Home => point.column = Column(0),
            SelectionMotion::End => point.column = last_column,
        }
        point.line = Line(point.line.0.clamp(top_line.0, bottom_line.0));
        point.column = min(point.column, last_column);

        if point == anchor_point {
            self.selection = None;
            return;
        }

        // 先滚动再落选区：scroll_display 在 vi 模式下会按 vi 光标重算旧选区，
        // 顺序反过来会覆盖本次键盘扩选。
        let display_offset = self.grid().display_offset() as i32;
        let visible_top = -display_offset;
        let visible_bottom = visible_top + screen_lines - 1;
        if point.line.0 < visible_top {
            self.scroll_display(Scroll::Delta(visible_top - point.line.0));
        } else if point.line.0 > visible_bottom {
            self.scroll_display(Scroll::Delta(-(point.line.0 - visible_bottom)));
        }

        // 锚点始终留在 region.start；端点跨越锚点时翻转两侧的排除语义，
        // 保证 [下界, 上界] 两端格子都计入。
        let (start_side, end_side) = if point >= anchor_point {
            (Side::Left, Side::Right)
        } else {
            (Side::Right, Side::Left)
        };
        self.selection = Some(Selection {
            ty: SelectionType::Simple,
            region: Range {
                start: Anchor::new(anchor_point, start_side),
                end: Anchor::new(point, end_side),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::index::{Column, Point, Side};
    use crate::term::test::TermSize;
    use crate::term::{Config, Term};

    fn term(height: usize, width: usize) -> Term<()> {
        let size = TermSize::new(width, height);
        Term::new(Config::default(), &size, ())
    }

    /// Test case of single cell selection.
    ///
    /// 1. [  ]
    /// 2. [B ]
    /// 3. [BE]
    #[test]
    fn single_cell_left_to_right() {
        let location = Point::new(Line(0), Column(0));
        let mut selection = Selection::new(SelectionType::Simple, location, Side::Left);
        selection.update(location, Side::Right);

        assert_eq!(
            selection.to_range(&term(1, 2)).unwrap(),
            SelectionRange {
                start: location,
                end: location,
                is_block: false
            }
        );
    }

    /// Test case of single cell selection.
    ///
    /// 1. [  ]
    /// 2. [ B]
    /// 3. [EB]
    #[test]
    fn single_cell_right_to_left() {
        let location = Point::new(Line(0), Column(0));
        let mut selection = Selection::new(SelectionType::Simple, location, Side::Right);
        selection.update(location, Side::Left);

        assert_eq!(
            selection.to_range(&term(1, 2)).unwrap(),
            SelectionRange {
                start: location,
                end: location,
                is_block: false
            }
        );
    }

    /// Test adjacent cell selection from left to right.
    ///
    /// 1. [  ][  ]
    /// 2. [ B][  ]
    /// 3. [ B][E ]
    #[test]
    fn between_adjacent_cells_left_to_right() {
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(0), Column(0)),
            Side::Right,
        );
        selection.update(Point::new(Line(0), Column(1)), Side::Left);

        assert_eq!(selection.to_range(&term(1, 2)), None);
    }

    /// 键盘扩选从光标起选，端点按格子移动并在回到锚点时清空。
    #[test]
    fn keyboard_extension_moves_endpoint_with_inclusive_cells() {
        use crate::event::VoidListener;
        use crate::event_loop::StreamProcessor;

        let mut term = Term::new(Config::default(), &TermSize::new(4, 2), VoidListener);
        let mut stream = StreamProcessor::default();
        stream.feed(&mut term, &VoidListener, b"abc");

        term.extend_selection(SelectionMotion::Left);
        assert_eq!(term.selection_to_string().as_deref(), Some("c"));
        term.extend_selection(SelectionMotion::Left);
        assert_eq!(term.selection_to_string().as_deref(), Some("bc"));
        term.extend_selection(SelectionMotion::Right);
        assert_eq!(term.selection_to_string().as_deref(), Some("c"));
        term.extend_selection(SelectionMotion::Right);
        assert!(term.selection.is_none(), "回到锚点必须收缩为空选区");

        // 末列右移换到下一行行首；End 只夹到当前行末列，不越出网格。
        term.extend_selection(SelectionMotion::Right);
        assert_eq!(
            term.selection.as_ref().unwrap().region.end.point,
            Point::new(Line(1), Column(0))
        );
        term.extend_selection(SelectionMotion::End);
        assert_eq!(
            term.selection.as_ref().unwrap().region.end.point,
            Point::new(Line(1), Column(3))
        );
    }

    /// 备用屏没有回滚：反复扩选夹在当前画面，不产生滚动偏移。
    #[test]
    fn keyboard_extension_in_alt_screen_stays_in_current_screen() {
        use crate::event::VoidListener;
        use crate::event_loop::StreamProcessor;

        let mut term = Term::new(Config::default(), &TermSize::new(20, 3), VoidListener);
        let mut stream = StreamProcessor::default();
        stream.feed(&mut term, &VoidListener, b"\x1b[?1049htop\r\nmid\r\nbot");

        for _ in 0..4 {
            term.extend_selection(SelectionMotion::Up);
        }
        term.extend_selection(SelectionMotion::Home);
        term.extend_selection(SelectionMotion::PageUp);
        assert_eq!(term.grid().display_offset(), 0, "备用屏不得进入回滚");
        assert_eq!(term.selection_to_string().as_deref(), Some("top\nmid\nbot"));
    }

    /// 翻页扩选把端点滚入视口，且不改变 PTY 状态。
    #[test]
    fn page_extension_scrolls_endpoint_into_view() {
        use crate::event::VoidListener;
        use crate::event_loop::StreamProcessor;

        let mut term = Term::new(Config::default(), &TermSize::new(20, 3), VoidListener);
        let mut stream = StreamProcessor::default();
        for i in 0..10 {
            stream.feed(&mut term, &VoidListener, format!("line {i}\r\n").as_bytes());
        }
        assert_eq!(term.grid().display_offset(), 0);

        term.extend_selection(SelectionMotion::PageUp);
        assert!(term.grid().display_offset() > 0, "端点必须滚入视口");
        let text = term.selection_to_string().expect("翻页扩选必须形成选区");
        assert!(text.contains("line 7"), "{text:?}");
    }

    /// Test adjacent cell selection from right to left.
    ///
    /// 1. [  ][  ]
    /// 2. [  ][B ]
    /// 3. [ E][B ]
    #[test]
    fn between_adjacent_cells_right_to_left() {
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(0), Column(1)),
            Side::Left,
        );
        selection.update(Point::new(Line(0), Column(0)), Side::Right);

        assert_eq!(selection.to_range(&term(1, 2)), None);
    }

    #[rustfmt::skip]
    /// Test selection across adjacent lines.
    ///
    /// 1.  [  ][  ][  ][  ][  ]
    ///     [  ][  ][  ][  ][  ]
    /// 2.  [  ][ B][  ][  ][  ]
    ///     [  ][  ][  ][  ][  ]
    /// 3.  [  ][ B][XX][XX][XX]
    ///     [XX][XE][  ][  ][  ]
    #[test]
    fn across_adjacent_lines_upward_final_cell_exclusive() {
        let mut selection =
            Selection::new(SelectionType::Simple, Point::new(Line(0), Column(1)), Side::Right);
        selection.update(Point::new(Line(1), Column(1)), Side::Right);

        assert_eq!(selection.to_range(&term(2, 5)).unwrap(), SelectionRange {
            start: Point::new(Line(0), Column(2)),
            end: Point::new(Line(1), Column(1)),
            is_block: false,
        });
    }

    #[rustfmt::skip]
    /// Test selection across adjacent lines.
    ///
    /// 1.  [  ][  ][  ][  ][  ]
    ///     [  ][  ][  ][  ][  ]
    /// 2.  [  ][  ][  ][  ][  ]
    ///     [  ][ B][  ][  ][  ]
    /// 3.  [  ][ E][XX][XX][XX]
    ///     [XX][XB][  ][  ][  ]
    /// 4.  [ E][XX][XX][XX][XX]
    ///     [XX][XB][  ][  ][  ]
    #[test]
    fn selection_bigger_then_smaller() {
        let mut selection =
            Selection::new(SelectionType::Simple, Point::new(Line(1), Column(1)), Side::Right);
        selection.update(Point::new(Line(0), Column(1)), Side::Right);
        selection.update(Point::new(Line(0), Column(0)), Side::Right);

        assert_eq!(selection.to_range(&term(2, 5)).unwrap(), SelectionRange {
            start: Point::new(Line(0), Column(1)),
            end: Point::new(Line(1), Column(1)),
            is_block: false,
        });
    }

    #[test]
    fn line_selection() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Lines,
            Point::new(Line(9), Column(1)),
            Side::Left,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(0), Column(0)),
                end: Point::new(Line(5), Column(4)),
                is_block: false,
            }
        );
    }

    #[test]
    fn semantic_selection() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Semantic,
            Point::new(Line(9), Column(3)),
            Side::Left,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(0), Column(1)),
                end: Point::new(Line(5), Column(3)),
                is_block: false,
            }
        );
    }

    #[test]
    fn simple_selection() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(9), Column(3)),
            Side::Right,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(0), Column(2)),
                end: Point::new(Line(5), Column(3)),
                is_block: false,
            }
        );
    }

    #[test]
    fn block_selection() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Block,
            Point::new(Line(9), Column(3)),
            Side::Right,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(0), Column(2)),
                end: Point::new(Line(5), Column(3)),
                is_block: true
            }
        );
    }

    #[test]
    fn simple_is_empty() {
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(1), Column(0)),
            Side::Right,
        );
        assert!(selection.is_empty());
        selection.update(Point::new(Line(1), Column(1)), Side::Left);
        assert!(selection.is_empty());
        selection.update(Point::new(Line(0), Column(0)), Side::Right);
        assert!(!selection.is_empty());
    }

    #[test]
    fn block_is_empty() {
        let mut selection = Selection::new(
            SelectionType::Block,
            Point::new(Line(1), Column(0)),
            Side::Right,
        );
        assert!(selection.is_empty());
        selection.update(Point::new(Line(1), Column(1)), Side::Left);
        assert!(selection.is_empty());
        selection.update(Point::new(Line(1), Column(1)), Side::Right);
        assert!(!selection.is_empty());
        selection.update(Point::new(Line(0), Column(0)), Side::Right);
        assert!(selection.is_empty());
        selection.update(Point::new(Line(0), Column(1)), Side::Left);
        assert!(selection.is_empty());
        selection.update(Point::new(Line(0), Column(1)), Side::Right);
        assert!(!selection.is_empty());
    }

    #[test]
    fn rotate_in_region_up() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(7), Column(3)),
            Side::Right,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(1), Column(0)),
                end: Point::new(Line(3), Column(3)),
                is_block: false,
            }
        );
    }

    #[test]
    fn rotate_in_region_down() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(4), Column(3)),
            Side::Right,
        );
        selection.update(Point::new(Line(1), Column(1)), Side::Left);
        selection = selection
            .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), -5)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(6), Column(1)),
                end: Point::new(Line(8), size.last_column()),
                is_block: false,
            }
        );
    }

    #[test]
    fn rotate_in_region_up_block() {
        let size = (10, 5);
        let mut selection = Selection::new(
            SelectionType::Block,
            Point::new(Line(7), Column(3)),
            Side::Right,
        );
        selection.update(Point::new(Line(4), Column(1)), Side::Right);
        selection = selection
            .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), 4)
            .unwrap();

        assert_eq!(
            selection.to_range(&term(size.0, size.1)).unwrap(),
            SelectionRange {
                start: Point::new(Line(1), Column(2)),
                end: Point::new(Line(3), Column(3)),
                is_block: true,
            }
        );
    }

    #[test]
    fn range_intersection() {
        let mut selection = Selection::new(
            SelectionType::Lines,
            Point::new(Line(3), Column(1)),
            Side::Left,
        );
        selection.update(Point::new(Line(6), Column(1)), Side::Right);

        assert!(selection.intersects_range(..));
        assert!(selection.intersects_range(Line(2)..));
        assert!(selection.intersects_range(Line(2)..=Line(4)));
        assert!(selection.intersects_range(Line(2)..=Line(7)));
        assert!(selection.intersects_range(Line(4)..=Line(5)));
        assert!(selection.intersects_range(Line(5)..Line(8)));

        assert!(!selection.intersects_range(..=Line(2)));
        assert!(!selection.intersects_range(Line(7)..=Line(8)));
    }
}
