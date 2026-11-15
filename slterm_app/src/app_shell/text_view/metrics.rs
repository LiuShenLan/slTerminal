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
        text.graphemes(true).fold(0, |column, grapheme| {
            column + self.grapheme_width(grapheme, column)
        })
    }

    /// 返回覆盖可视列的最小完整字素前缀，不复制不可见的长行尾部。
    pub(crate) fn visible_prefix_len(&self, text: RopeSlice<'_>, columns: usize) -> usize {
        if columns == 0 || text.len() == 0 {
            return 0;
        }
        let mut chunks = text.chunks();
        let Some(mut chunk) = chunks.next() else {
            return 0;
        };
        let mut chunk_start = 0;
        let mut cursor = GraphemeCursor::new(0, text.len(), true);
        let mut column = 0;
        let mut start = 0;
        loop {
            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(end)) => {
                    let grapheme = text.slice(start..end).to_string();
                    column += self.grapheme_width(&grapheme, column);
                    start = end;
                    if column >= columns {
                        return end;
                    }
                }
                Ok(None) => return text.len(),
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_start += chunk.len();
                    let Some(next) = chunks.next() else {
                        return text.len();
                    };
                    chunk = next;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let Some(previous) = end.checked_sub(1) else {
                        return start;
                    };
                    let Some((context, context_start)) = text.get_chunk(previous) else {
                        return start;
                    };
                    cursor.provide_context(&context[..end - context_start], context_start);
                }
                Err(_) => return start,
            }
        }
    }

    pub(crate) fn column_at(&self, text: &str, byte: usize) -> Option<usize> {
        if byte > text.len() || !text.is_char_boundary(byte) {
            return None;
        }
        let mut column = 0;
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
        let mut column = 0;
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

    fn grapheme_width(&self, grapheme: &str, column: usize) -> usize {
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
