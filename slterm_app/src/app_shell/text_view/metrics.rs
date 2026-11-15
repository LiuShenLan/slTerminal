use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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
