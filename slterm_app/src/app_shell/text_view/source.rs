use std::ops::Range;

use ropey::{LineType, Rope};
use slterm_document::{DocumentId, DocumentPageCache, PageKey, PagedLineIndex, PagedSource};

use super::metrics::DisplayMap;

/// 一行按需物化的展示文本；全局字节区间不含行尾换行。
pub(crate) struct VisibleLine {
    index: usize,
    byte_range: Range<usize>,
    text: String,
    display_width: usize,
}

impl VisibleLine {
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn display_width(&self) -> usize {
        self.display_width
    }
}

pub(crate) enum VisibleRow {
    Text(VisibleLine),
    Loading { index: usize },
}

impl VisibleRow {
    pub(crate) const fn index(&self) -> usize {
        match self {
            Self::Text(line) => line.index(),
            Self::Loading { index } => *index,
        }
    }

    pub(crate) const fn as_text(&self) -> Option<&VisibleLine> {
        match self {
            Self::Text(line) => Some(line),
            Self::Loading { .. } => None,
        }
    }
}

pub(crate) trait TextSource {
    fn line_count(&self) -> usize;
    fn visible_lines(&mut self, lines: Range<usize>) -> Vec<VisibleRow>;
}

pub(crate) struct RopeTextSource<'a> {
    text: &'a Rope,
    display: DisplayMap,
}

impl<'a> RopeTextSource<'a> {
    pub(crate) const fn new(text: &'a Rope, display: DisplayMap) -> Self {
        Self { text, display }
    }

    /// 仅物化覆盖可视列的完整字素前缀；长行不可见尾部不进入前台字符串。
    pub(crate) fn visible_line_windows(
        &self,
        lines: Range<usize>,
        columns: usize,
    ) -> Vec<VisibleLine> {
        let line_count = self.text.len_lines(LineType::LF);
        let start = lines.start.min(line_count);
        let end = lines.end.min(line_count).max(start);
        (start..end)
            .map(|index| {
                let line = self.text.line(index, LineType::LF);
                let line_start = self.text.line_to_byte_idx(index, LineType::LF);
                let has_newline = line.len() > 0 && line.byte(line.len() - 1) == b'\n';
                let mut content_len = line.len() - usize::from(has_newline);
                if has_newline && content_len > 0 && line.byte(content_len - 1) == b'\r' {
                    content_len -= 1;
                }
                let content = line.slice(..content_len);
                let visible_len = self.display.visible_prefix_len(content, columns);
                let text = content.slice(..visible_len).to_string();
                VisibleLine {
                    index,
                    byte_range: line_start..line_start + visible_len,
                    display_width: self.display.width(&text),
                    text,
                }
            })
            .collect()
    }

    /// 仅物化请求范围内的行；范围越界只截到真实行数。
    pub(crate) fn visible_lines(&self, lines: Range<usize>) -> Vec<VisibleLine> {
        let line_count = self.text.len_lines(LineType::LF);
        let start = lines.start.min(line_count);
        let end = lines.end.min(line_count).max(start);
        (start..end)
            .map(|index| {
                let line = self.text.line(index, LineType::LF);
                let line_start = self.text.line_to_byte_idx(index, LineType::LF);
                let has_newline = line.len() > 0 && line.byte(line.len() - 1) == b'\n';
                let mut content_len = line.len() - usize::from(has_newline);
                if has_newline && content_len > 0 && line.byte(content_len - 1) == b'\r' {
                    content_len -= 1;
                }
                let text = line.slice(..content_len).to_string();
                VisibleLine {
                    index,
                    byte_range: line_start..line_start + content_len,
                    display_width: self.display.width(&text),
                    text,
                }
            })
            .collect()
    }
}

impl TextSource for RopeTextSource<'_> {
    fn line_count(&self) -> usize {
        self.text.len_lines(LineType::LF)
    }

    fn visible_lines(&mut self, lines: Range<usize>) -> Vec<VisibleRow> {
        RopeTextSource::visible_lines(self, lines)
            .into_iter()
            .map(VisibleRow::Text)
            .collect()
    }
}

/// 只读分页视图端口；缓存缺失只形成 Loading，不在前台读盘。
pub(crate) struct PagedTextSource<'a> {
    document_id: DocumentId,
    source: &'a PagedSource,
    lines: &'a PagedLineIndex,
    cache: &'a mut DocumentPageCache,
    display: DisplayMap,
}

impl<'a> PagedTextSource<'a> {
    pub(crate) fn new(
        document_id: DocumentId,
        source: &'a PagedSource,
        lines: &'a PagedLineIndex,
        cache: &'a mut DocumentPageCache,
        display: DisplayMap,
    ) -> Self {
        Self {
            document_id,
            source,
            lines,
            cache,
            display,
        }
    }

    fn read_continuation(
        &mut self,
        index: usize,
        byte_start: usize,
        mut text: String,
        mut page: usize,
    ) -> Option<VisibleLine> {
        loop {
            let key = PageKey::new(self.document_id, self.source.source_generation(), page);
            let page_text = self.cache.get(&key)?;
            text.push_str(page_text.split('\n').next()?);
            if self.lines.line_ends_in_page(page, 0)? || page + 1 == self.source.index().len() {
                break;
            }
            page += 1;
        }
        Some(VisibleLine {
            index,
            byte_range: byte_start..byte_start + text.len(),
            display_width: self.display.width(&text),
            text,
        })
    }
}

impl TextSource for PagedTextSource<'_> {
    fn line_count(&self) -> usize {
        self.lines.line_count()
    }

    fn visible_lines(&mut self, lines: Range<usize>) -> Vec<VisibleRow> {
        let start = lines.start.min(self.line_count());
        let end = lines.end.min(self.line_count()).max(start);
        let mut rows = Vec::with_capacity(end - start);
        if self.lines.location(start).is_none() {
            if start == 0 && end == 1 && self.source.index().is_empty() {
                rows.push(VisibleRow::Text(VisibleLine {
                    index: 0,
                    byte_range: 0..0,
                    text: String::new(),
                    display_width: 0,
                }));
            } else {
                rows.extend((start..end).map(|index| VisibleRow::Loading { index }));
            }
            return rows;
        }

        let mut index = start;
        while index < end {
            let (page, local_start, page_offset) = self.lines.location(index).unwrap();
            let mut row_end = index + 1;
            while row_end < end
                && self
                    .lines
                    .location(row_end)
                    .is_some_and(|(next, _, _)| next == page)
            {
                row_end += 1;
            }
            let local_end = local_start + row_end - index;
            let key = PageKey::new(self.document_id, self.source.source_generation(), page);
            let Some(page_text) = self.cache.get(&key) else {
                rows.extend((index..row_end).map(|index| VisibleRow::Loading { index }));
                index = row_end;
                continue;
            };

            // 每页只顺序扫描一次；续行在该借用结束后才读取后续页。
            let mut pending = Vec::with_capacity(row_end - index);
            let mut local_byte = 0;
            for (local, segment) in page_text.split('\n').enumerate().take(local_end) {
                if local >= local_start {
                    let line_index = index + local - local_start;
                    let byte_start = page_offset + local_byte;
                    let ended = self.lines.line_ends_in_page(page, local).unwrap_or(false);
                    pending.push((line_index, byte_start, segment.to_owned(), ended));
                }
                local_byte += segment.len() + 1;
            }
            for (line_index, byte_start, text, ended) in pending {
                let line = if ended || page + 1 == self.source.index().len() {
                    Some(VisibleLine {
                        index: line_index,
                        display_width: self.display.width(&text),
                        byte_range: byte_start..byte_start + text.len(),
                        text,
                    })
                } else {
                    self.read_continuation(line_index, byte_start, text, page + 1)
                };
                rows.push(line.map_or(VisibleRow::Loading { index: line_index }, VisibleRow::Text));
            }
            index = row_end;
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;
    use slterm_document::{
        DocumentPageCache, PageIndex, PageKey, PagedLineIndex, PagedSource, SourceByteRange,
    };

    use super::{PagedTextSource, RopeTextSource, TextSource};
    use crate::app_shell::text_view::metrics::DisplayMap;

    #[test]
    fn rope_source_materializes_only_requested_lines_with_document_offsets() {
        let rope = Rope::from_str("zero\n中😀\nlast\n");
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_lines(1..3);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].index(), 1);
        assert_eq!(lines[0].text(), "中😀");
        assert_eq!(lines[0].display_width(), 4);
        assert_eq!(lines[0].byte_range(), 5..12);
        assert_eq!(lines[1].text(), "last");
        assert_eq!(lines[1].byte_range(), 13..17);

        let trailing = source.visible_lines(3..5);
        assert_eq!(trailing.len(), 1);
        assert_eq!(trailing[0].index(), 3);
        assert_eq!(trailing[0].text(), "");
        assert_eq!(trailing[0].byte_range(), 18..18);
        assert!(source.visible_lines(99..100).is_empty());

        let rope = Rope::from_str("a\r\nb");
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_lines(0..1);
        assert_eq!(lines[0].text(), "a");
        assert_eq!(lines[0].byte_range(), 0..1);
    }

    #[test]
    fn rope_source_bounds_long_line_materialization_to_visible_columns() {
        let rope = Rope::from_str(&"a".repeat(100_000));
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_line_windows(0..1, 4);
        assert_eq!(lines[0].text(), "aaaa");
        assert_eq!(lines[0].byte_range(), 0..4);

        let rope = Rope::from_str(&format!("e\u{301}{}", "b".repeat(100_000)));
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_line_windows(0..1, 1);
        assert_eq!(lines[0].text(), "e\u{301}");
        assert_eq!(lines[0].byte_range(), 0..3);
    }

    #[test]
    fn paged_source_reads_only_cached_lines_and_marks_missing_pages() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            9,
            vec![
                SourceByteRange::new(0, 4).unwrap(),
                SourceByteRange::new(4, 9).unwrap(),
            ],
        )
        .unwrap();
        let source = PagedSource::new(pages, [7; 32]);
        let lines = PagedLineIndex::from_page_stats(&[(1, 4), (1, 5)]);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "中\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &lines, &mut cache, DisplayMap::new(4));

        let rows = text.visible_lines(0..3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].as_text().unwrap().text(), "中");
        assert_eq!(rows[0].as_text().unwrap().byte_range(), 0..3);
        assert!(rows[1].as_text().is_none());
        assert!(rows[2].as_text().is_none());
        assert_eq!(rows[1].index(), 1);

        let _ = text;
        cache.insert(PageKey::new(id, 0, 1), "😀\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &lines, &mut cache, DisplayMap::new(4));
        let rows = text.visible_lines(0..3);
        assert_eq!(rows[1].as_text().unwrap().text(), "😀");
        assert_eq!(rows[1].as_text().unwrap().byte_range(), 4..8);
        assert_eq!(rows[2].as_text().unwrap().text(), "");
        assert_eq!(rows[2].as_text().unwrap().byte_range(), 9..9);
        assert_eq!(text.line_count(), 3);
    }

    #[test]
    fn paged_source_joins_a_line_split_across_pages() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            13,
            vec![
                SourceByteRange::new(0, 3).unwrap(),
                SourceByteRange::new(3, 13).unwrap(),
            ],
        )
        .unwrap();
        let source = PagedSource::new(pages, [9; 32]);
        let lines = PagedLineIndex::from_page_stats(&[(0, 3), (2, 10)]);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "abc".to_owned());
        let mut text = PagedTextSource::new(id, &source, &lines, &mut cache, DisplayMap::new(4));
        assert!(text.visible_lines(0..1)[0].as_text().is_none());
        let _ = text;

        cache.insert(PageKey::new(id, 0, 1), "😀\nnext\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &lines, &mut cache, DisplayMap::new(4));
        let rows = text.visible_lines(0..3);
        assert_eq!(rows[0].as_text().unwrap().text(), "abc😀");
        assert_eq!(rows[0].as_text().unwrap().byte_range(), 0..7);
        assert_eq!(rows[1].as_text().unwrap().text(), "next");
        assert_eq!(rows[1].as_text().unwrap().byte_range(), 8..12);
        assert_eq!(rows[2].as_text().unwrap().byte_range(), 13..13);
    }

    #[test]
    fn paged_source_does_not_fake_empty_text_for_a_nonempty_source() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(1, vec![SourceByteRange::new(0, 1).unwrap()]).unwrap();
        let source = PagedSource::new(pages, [3; 32]);
        let lines = PagedLineIndex::from_page_stats(&[]);
        let mut cache = DocumentPageCache::new();
        let mut text = PagedTextSource::new(id, &source, &lines, &mut cache, DisplayMap::new(4));

        let rows = text.visible_lines(0..1);
        assert!(rows[0].as_text().is_none());
        assert_eq!(rows[0].index(), 0);
    }
}
