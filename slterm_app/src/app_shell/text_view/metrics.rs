use std::ops::Range;

use ropey::{Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete, UnicodeSegmentation};
use unicode_width::UnicodeWidthStr;

/// 不物化整行地判断 Rope 字节位置是否为扩展字素簇边界。
pub(crate) fn rope_is_grapheme_boundary(text: &Rope, byte: usize) -> bool {
    if byte > text.len() || !text.is_char_boundary(byte) {
        return false;
    }
    if byte == 0 || byte == text.len() {
        return true;
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    loop {
        let Some((chunk, chunk_start)) = text.get_chunk(byte) else {
            return false;
        };
        match cursor.is_boundary(chunk, chunk_start) {
            Ok(boundary) => return boundary,
            Err(GraphemeIncomplete::PreContext(end)) => {
                let Some(previous) = end.checked_sub(1) else {
                    return false;
                };
                let Some((chunk, start)) = text.get_chunk(previous) else {
                    return false;
                };
                cursor.provide_context(&chunk[..end - start], start);
            }
            Err(_) => return false,
        }
    }
}

/// 将字素内部位置向前吸附到所属字素起点。
pub(crate) fn rope_floor_grapheme_boundary(text: &Rope, byte: usize) -> Option<usize> {
    if byte > text.len() || !text.is_char_boundary(byte) {
        return None;
    }
    if rope_is_grapheme_boundary(text, byte) {
        return Some(byte);
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    let (mut chunk, mut chunk_start) = text.get_chunk(byte)?;
    loop {
        match cursor.prev_boundary(chunk, chunk_start) {
            Ok(boundary) => return boundary.or(Some(0)),
            Err(GraphemeIncomplete::PrevChunk) => {
                let previous = cursor.cur_cursor().checked_sub(1)?;
                (chunk, chunk_start) = text.get_chunk(previous)?;
            }
            Err(GraphemeIncomplete::PreContext(end)) => {
                let previous = end.checked_sub(1)?;
                let (context, start) = text.get_chunk(previous)?;
                cursor.provide_context(&context[..end - start], start);
            }
            Err(_) => return None,
        }
    }
}

/// 将字素内部位置向后吸附到所属字素终点。
pub(crate) fn rope_ceil_grapheme_boundary(text: &Rope, byte: usize) -> Option<usize> {
    if byte > text.len() || !text.is_char_boundary(byte) {
        return None;
    }
    if rope_is_grapheme_boundary(text, byte) {
        return Some(byte);
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    let (mut chunk, mut chunk_start) = text.get_chunk(byte)?;
    loop {
        match cursor.next_boundary(chunk, chunk_start) {
            Ok(boundary) => return boundary.or(Some(text.len())),
            Err(GraphemeIncomplete::NextChunk) => {
                let offset = cursor.cur_cursor();
                if offset >= text.len() {
                    return Some(text.len());
                }
                let old_end = chunk_start + chunk.len();
                let (next, start) = text.get_chunk(offset)?;
                if start == chunk_start && old_end == start + next.len() {
                    (chunk, chunk_start) = text.get_chunk(offset + 1)?;
                } else {
                    (chunk, chunk_start) = (next, start);
                }
            }
            Err(GraphemeIncomplete::PreContext(end)) => {
                let previous = end.checked_sub(1)?;
                let (context, start) = text.get_chunk(previous)?;
                cursor.provide_context(&context[..end - start], start);
            }
            Err(_) => return None,
        }
    }
}

/// 返回前一个完整字素边界，支持跨行与组合字符。
pub(crate) fn rope_previous_grapheme_boundary(text: &Rope, byte: usize) -> Option<usize> {
    if byte == 0 || byte > text.len() || !text.is_char_boundary(byte) {
        return None;
    }
    let mut probe = byte - 1;
    while !text.is_char_boundary(probe) {
        probe -= 1;
    }
    rope_floor_grapheme_boundary(text, probe)
}

/// 返回后一个完整字素边界，支持跨行与组合字符。
pub(crate) fn rope_next_grapheme_boundary(text: &Rope, byte: usize) -> Option<usize> {
    if byte == text.len() || byte > text.len() || !text.is_char_boundary(byte) {
        return None;
    }
    let mut probe = byte + 1;
    while probe < text.len() && !text.is_char_boundary(probe) {
        probe += 1;
    }
    rope_ceil_grapheme_boundary(text, probe)
}

/// 单行文本的字素簇与显示列映射；输入不包含内部换行。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DisplayMap {
    tab_width: usize,
}

impl DisplayMap {
    pub(crate) const fn new(tab_width: usize) -> Self {
        assert!(tab_width > 0, "Tab 宽度必须大于零");
        Self { tab_width }
    }

    pub(crate) fn width(&self, text: &str) -> usize {
        self.width_from(text, 0)
    }

    pub(crate) fn width_from(&self, text: &str, start_column: usize) -> usize {
        text.graphemes(true).fold(start_column, |column, grapheme| {
            column + self.grapheme_width(grapheme, column)
        })
    }

    /// 不物化整行地计算字素边界对应的显示列。
    pub(crate) fn column_at_rope(&self, text: RopeSlice<'_>, byte: usize) -> Option<usize> {
        self.column_at_rope_from(text, byte, 0)
    }

    pub(crate) fn column_at_rope_from(
        &self,
        text: RopeSlice<'_>,
        byte: usize,
        start_column: usize,
    ) -> Option<usize> {
        if byte > text.len() || !text.is_char_boundary(byte) {
            return None;
        }
        if byte == 0 {
            return Some(start_column);
        }
        let mut chunks = text.chunks();
        let mut chunk = chunks.next()?;
        let mut chunk_start = 0;
        let mut cursor = GraphemeCursor::new(0, text.len(), true);
        let mut column = start_column;
        let mut start = 0;
        loop {
            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(end)) => {
                    if byte > start && byte < end {
                        return None;
                    }
                    column += self.grapheme_width(&text.slice(start..end).to_string(), column);
                    start = end;
                    if byte == end {
                        return Some(column);
                    }
                }
                Ok(None) => return (byte == text.len()).then_some(column),
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_start += chunk.len();
                    chunk = chunks.next()?;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let previous = end.checked_sub(1)?;
                    let (context, context_start) = text.get_chunk(previous)?;
                    cursor.provide_context(&context[..end - context_start], context_start);
                }
                Err(_) => return None,
            }
        }
    }

    /// 返回与可视列相交的完整字素字节窗口及其实际起始显示列。
    pub(crate) fn visible_window(
        &self,
        text: RopeSlice<'_>,
        columns: Range<usize>,
    ) -> (Range<usize>, usize) {
        if columns.start >= columns.end || text.len() == 0 {
            return (0..0, 0);
        }
        let mut chunks = text.chunks();
        let Some(mut chunk) = chunks.next() else {
            return (0..0, 0);
        };
        let mut chunk_start = 0;
        let mut cursor = GraphemeCursor::new(0, text.len(), true);
        let mut column = 0;
        let mut start = 0;
        let mut window = None;
        loop {
            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(end)) => {
                    let grapheme_column = column;
                    column += self.grapheme_width(&text.slice(start..end).to_string(), column);
                    if window.is_none() && column > columns.start {
                        window = Some((start, grapheme_column));
                    }
                    if column >= columns.end
                        && let Some((window_start, display_start)) = window
                    {
                        return (window_start..end, display_start);
                    }
                    start = end;
                }
                Ok(None) => {
                    return match window {
                        Some((start, display_start)) => (start..text.len(), display_start),
                        None => (text.len()..text.len(), column),
                    };
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_start += chunk.len();
                    let Some(next) = chunks.next() else {
                        return match window {
                            Some((start, display_start)) => (start..text.len(), display_start),
                            None => (text.len()..text.len(), column),
                        };
                    };
                    chunk = next;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let Some(previous) = end.checked_sub(1) else {
                        return (0..0, 0);
                    };
                    let Some((context, context_start)) = text.get_chunk(previous) else {
                        return (0..0, 0);
                    };
                    cursor.provide_context(&context[..end - context_start], context_start);
                }
                Err(_) => {
                    return match window {
                        Some((start, display_start)) => (start..start, display_start),
                        None => (0..0, 0),
                    };
                }
            }
        }
    }

    /// 按显示列切分完整字素；范围不含换行，起点保留全局逻辑列以维持 Tab 停靠。
    pub(crate) fn wrap_segments(
        &self,
        text: RopeSlice<'_>,
        max_columns: usize,
    ) -> Vec<(Range<usize>, usize)> {
        let mut segments = Vec::new();
        self.visit_wrap_segments_from(text, max_columns, 0, 0, |range, display_start| {
            segments.push((range, display_start));
            true
        });
        segments
    }

    /// 从已知字素边界续扫；visitor 返回 false 时立即停止，不读取后续文本。
    pub(crate) fn visit_wrap_segments_from(
        &self,
        text: RopeSlice<'_>,
        max_columns: usize,
        start_byte: usize,
        start_column: usize,
        mut visit: impl FnMut(Range<usize>, usize) -> bool,
    ) {
        let max_columns = max_columns.max(1);
        if text.len() == 0 {
            visit(0..0, 0);
            return;
        }
        if start_byte >= text.len() {
            return;
        }
        let Some((mut chunk, mut chunk_start)) = text.get_chunk(start_byte) else {
            return;
        };
        let mut cursor = GraphemeCursor::new(start_byte, text.len(), true);
        let mut column = start_column;
        let mut start = start_byte;
        let mut segment_start = start_byte;
        let mut segment_display_start = start_column;
        loop {
            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(end)) => {
                    let grapheme = text.slice(start..end).to_string();
                    let width = self.grapheme_width(&grapheme, column);
                    let segment_columns = column - segment_display_start;
                    if start > segment_start && width > max_columns.saturating_sub(segment_columns)
                    {
                        if !visit(segment_start..start, segment_display_start) {
                            return;
                        }
                        segment_start = start;
                        segment_display_start = column;
                    }
                    column += width;
                    start = end;
                }
                Ok(None) => {
                    visit(segment_start..text.len(), segment_display_start);
                    return;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    let next = chunk_start + chunk.len();
                    if next >= text.len() {
                        visit(segment_start..text.len(), segment_display_start);
                        return;
                    }
                    let Some((next_chunk, next_start)) = text.get_chunk(next) else {
                        return;
                    };
                    chunk = next_chunk;
                    chunk_start = next_start;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let Some(previous) = end.checked_sub(1) else {
                        return;
                    };
                    let Some((context, context_start)) = text.get_chunk(previous) else {
                        return;
                    };
                    cursor.provide_context(&context[..end - context_start], context_start);
                }
                Err(_) => return,
            }
        }
    }

    pub(crate) fn column_at(&self, text: &str, byte: usize) -> Option<usize> {
        self.column_at_from(text, byte, 0)
    }

    pub(crate) fn column_at_from(
        &self,
        text: &str,
        byte: usize,
        start_column: usize,
    ) -> Option<usize> {
        if byte > text.len() || !text.is_char_boundary(byte) {
            return None;
        }
        let mut column = start_column;
        for (start, grapheme) in text.grapheme_indices(true) {
            if byte == start {
                return Some(column);
            }
            if byte < start + grapheme.len() {
                return None;
            }
            column += self.grapheme_width(grapheme, column);
        }
        (byte == text.len()).then_some(column)
    }

    pub(crate) fn byte_at_column(&self, text: &str, target: usize) -> Option<usize> {
        self.byte_at_column_from(text, target, 0)
    }

    pub(crate) fn byte_at_column_from(
        &self,
        text: &str,
        target: usize,
        start_column: usize,
    ) -> Option<usize> {
        if target <= start_column {
            return Some(0);
        }
        let mut column = start_column;
        for (start, grapheme) in text.grapheme_indices(true) {
            if target == column {
                return Some(start);
            }
            let width = self.grapheme_width(grapheme, column);
            if target < column + width {
                let offset = target - column;
                return Some(if offset * 2 < width {
                    start
                } else {
                    start + grapheme.len()
                });
            }
            column += width;
        }
        Some(text.len())
    }

    pub(crate) fn grapheme_width(&self, grapheme: &str, column: usize) -> usize {
        if grapheme == "\t" {
            self.tab_width - column % self.tab_width
        } else {
            UnicodeWidthStr::width(grapheme)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayMap;

    #[test]
    fn display_map_uses_graphemes_and_utf8_boundaries() {
        let map = DisplayMap::new(4);
        let text = "A中😀e\u{301}";
        assert_eq!(map.width(text), 6);
        assert_eq!(map.column_at(text, 0), Some(0));
        assert_eq!(map.column_at(text, 1), Some(1));
        assert_eq!(map.column_at(text, 2), None);
        assert_eq!(map.column_at(text, 4), Some(3));
        assert_eq!(map.column_at(text, 8), Some(5));
        assert_eq!(map.column_at(text, text.len()), Some(6));

        assert_eq!(map.byte_at_column(text, 0), Some(0));
        assert_eq!(map.byte_at_column(text, 1), Some(1));
        assert_eq!(map.byte_at_column(text, 2), Some(4));
        assert_eq!(map.byte_at_column(text, 5), Some(8));
        assert_eq!(map.byte_at_column(text, 6), Some(text.len()));
        assert_eq!(map.byte_at_column(text, 99), Some(text.len()));
    }

    #[test]
    fn display_map_wrap_segments_preserve_graphemes_and_display_columns() {
        let map = DisplayMap::new(4);
        let text = ropey::Rope::from_str("A中B😀C");
        assert_eq!(
            map.wrap_segments(text.slice(..), 3),
            vec![(0..4, 0), (4..9, 3), (9..10, 6)]
        );
        let tab = ropey::Rope::from_str("A\tB");
        assert_eq!(
            map.wrap_segments(tab.slice(..), 4),
            vec![(0..2, 0), (2..3, 4)]
        );
        let empty = ropey::Rope::from_str("");
        assert_eq!(map.wrap_segments(empty.slice(..), 3), vec![(0..0, 0)]);
        let wide = ropey::Rope::from_str("中😀");
        assert_eq!(
            map.wrap_segments(wide.slice(..), 1),
            vec![(0..3, 0), (3..7, 2)]
        );
    }

    #[test]
    fn display_map_honors_tab_stops_and_zero_width_continuation() {
        let map = DisplayMap::new(4);
        let text = "A\tB";
        assert_eq!(map.width(text), 5);
        assert_eq!(map.column_at(text, 2), Some(4));
        assert_eq!(map.byte_at_column(text, 4), Some(2));

        let combining = "e\u{301}";
        assert_eq!(map.width(combining), 1);
        assert_eq!(map.column_at(combining, 1), None);
        assert_eq!(map.column_at(combining, combining.len()), Some(1));
        assert_eq!(map.byte_at_column(combining, 0), Some(0));
        assert_eq!(map.byte_at_column(combining, 1), Some(combining.len()));
    }
}
