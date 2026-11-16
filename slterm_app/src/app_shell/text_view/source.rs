use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use ropey::{LineType, Rope};
use slterm_document::{DocumentId, DocumentPageCache, PageKey, PagedLineIndex, PagedSource};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use super::metrics::DisplayMap;

/// 视图瞬态选区；坐标始终是解码正文的全局 UTF-8 字素边界。
#[derive(Clone, Copy, Debug)]
pub(crate) struct TextSelection {
    pub(crate) anchor: usize,
    pub(crate) head: usize,
}

impl TextSelection {
    pub(crate) fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

/// 一行按需物化的展示文本；全局字节区间不含行尾换行。
pub(crate) struct VisibleLine {
    index: usize,
    byte_range: Range<usize>,
    display_start: usize,
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

    pub(crate) const fn display_start(&self) -> usize {
        self.display_start
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

    /// 读取行内容的全局字节范围，不物化行文本，也不含 LF／CRLF。
    pub(crate) fn line_byte_range(&self, index: usize) -> Option<Range<usize>> {
        if index >= self.text.len_lines(LineType::LF) {
            return None;
        }
        let line = self.text.line(index, LineType::LF);
        let line_start = self.text.line_to_byte_idx(index, LineType::LF);
        let has_newline = line.len() > 0 && line.byte(line.len() - 1) == b'\n';
        let mut content_len = line.len() - usize::from(has_newline);
        if has_newline && content_len > 0 && line.byte(content_len - 1) == b'\r' {
            content_len -= 1;
        }
        Some(line_start..line_start + content_len)
    }

    /// 仅物化与可视列相交的完整字素；长行其余部分不进入前台字符串。
    pub(crate) fn visible_line_windows(
        &self,
        lines: Range<usize>,
        columns: Range<usize>,
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
                let (visible, display_start) =
                    self.display.visible_window(content, columns.clone());
                let text = content.slice(visible.clone()).to_string();
                VisibleLine {
                    index,
                    byte_range: line_start + visible.start..line_start + visible.end,
                    display_start,
                    display_width: self.display.width_from(&text, display_start) - display_start,
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
                    display_start: 0,
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
    missing_pages: BTreeSet<usize>,
}

#[derive(Clone, Copy)]
struct PagedLineLocation {
    index: usize,
    byte_start: usize,
    page: usize,
}

#[derive(Clone, Copy)]
enum PagedTarget {
    Byte(usize),
    Column(usize),
}

/// 跨页窗口的增量结果；只保留可见文本与当前跨页字素。
#[derive(Default)]
struct PagedWindow {
    column: usize,
    display_start: Option<usize>,
    byte_start: Option<usize>,
    text: String,
    target: Option<PagedTarget>,
    target_column: Option<usize>,
    target_position: Option<(usize, usize)>,
    materialize: bool,
}

struct PagedScan {
    line: VisibleLine,
    target_column: Option<usize>,
    target_position: Option<(usize, usize)>,
}

impl PagedWindow {
    fn byte_navigation(target_byte: usize) -> Self {
        Self {
            target: Some(PagedTarget::Byte(target_byte)),
            ..Self::default()
        }
    }

    fn column_navigation(target_column: usize) -> Self {
        Self {
            target: Some(PagedTarget::Column(target_column)),
            ..Self::default()
        }
    }

    fn painted() -> Self {
        Self {
            materialize: true,
            ..Self::default()
        }
    }

    fn push(
        &mut self,
        display: &DisplayMap,
        grapheme: &str,
        line_start: usize,
        start: usize,
        columns: &Range<usize>,
    ) -> bool {
        let grapheme_column = self.column;
        let byte = line_start + start;
        let width = display.grapheme_width(grapheme, self.column);
        match self.target {
            Some(PagedTarget::Byte(target)) if target == byte => {
                self.target_column = Some(grapheme_column);
            }
            Some(PagedTarget::Column(target)) if self.target_position.is_none() => {
                if target == grapheme_column {
                    self.target_position = Some((byte, grapheme_column));
                } else if target > grapheme_column && target < grapheme_column + width {
                    let offset = target - grapheme_column;
                    self.target_position = Some(if offset * 2 < width {
                        (byte, grapheme_column)
                    } else {
                        (byte + grapheme.len(), grapheme_column + width)
                    });
                }
            }
            _ => {}
        }
        self.column += width;
        if self.display_start.is_none() && self.column > columns.start {
            self.display_start = Some(grapheme_column);
            self.byte_start = Some(byte);
        }
        if self.materialize && self.display_start.is_some() {
            self.text.push_str(grapheme);
        }
        self.target_column.is_some() || self.target_position.is_some() || self.column >= columns.end
    }

    fn finish(self, index: usize, line_start: usize, end: usize) -> PagedScan {
        let display_start = self.display_start.unwrap_or(self.column);
        let byte_start = self.byte_start.unwrap_or(line_start + end);
        let target_column = if matches!(
            self.target,
            Some(PagedTarget::Byte(target)) if target == line_start + end
        ) {
            Some(self.column)
        } else {
            self.target_column
        };
        let target_position = match self.target {
            Some(PagedTarget::Column(_)) => self
                .target_position
                .or(Some((line_start + end, self.column))),
            _ => self.target_position,
        };
        PagedScan {
            line: VisibleLine {
                index,
                byte_range: byte_start..line_start + end,
                display_start,
                display_width: self.column - display_start,
                text: self.text,
            },
            target_column,
            target_position,
        }
    }
}

impl<'a> PagedTextSource<'a> {
    pub(crate) fn new(
        document_id: DocumentId,
        source: &'a PagedSource,
        cache: &'a mut DocumentPageCache,
        display: DisplayMap,
    ) -> Self {
        let lines = source.line_index();
        Self {
            document_id,
            source,
            lines,
            cache,
            display,
            missing_pages: BTreeSet::new(),
        }
    }

    pub(crate) const fn missing_pages(&self) -> &BTreeSet<usize> {
        &self.missing_pages
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
            let Some(page_text) = self.cache.get(&key) else {
                self.missing_pages.insert(page);
                return None;
            };
            text.push_str(page_text.split('\n').next()?);
            if self.lines.line_ends_in_page(page, 0)? || page + 1 == self.source.index().len() {
                break;
            }
            page += 1;
        }
        Some(VisibleLine {
            index,
            byte_range: byte_start..byte_start + text.len(),
            display_start: 0,
            display_width: self.display.width(&text),
            text,
        })
    }

    /// 字素游标需要历史时，从缓存页按全局坐标回投，不保留已跳过的整行正文。
    fn provide_context(
        &mut self,
        cursor: &mut GraphemeCursor,
        end: usize,
        line_start: usize,
        chunk_start: usize,
        segment: &str,
    ) -> Option<()> {
        if end > chunk_start && end <= chunk_start + segment.len() {
            cursor.provide_context(&segment[..end - chunk_start], chunk_start);
            return Some(());
        }
        if end == 0 {
            return None;
        }
        let absolute_end = line_start + end;
        let (mut context_page, mut page_offset) = self.lines.utf8_page(absolute_end - 1)?;
        loop {
            let local_end = absolute_end - page_offset;
            let local_start = line_start.saturating_sub(page_offset);
            let key = PageKey::new(
                self.document_id,
                self.source.source_generation(),
                context_page,
            );
            let Some(page_text) = self.cache.get(&key) else {
                self.missing_pages.insert(context_page);
                return None;
            };
            if local_start <= local_end
                && local_end <= page_text.len()
                && page_text.is_char_boundary(local_start)
                && page_text.is_char_boundary(local_end)
            {
                cursor.provide_context(
                    &page_text[local_start..local_end],
                    page_offset.max(line_start) - line_start,
                );
                return Some(());
            }
            if context_page == 0 {
                return None;
            }
            context_page -= 1;
            page_offset = self.lines.page_utf8_offset(context_page)?;
        }
    }

    fn cached_chunk(&mut self, byte: usize) -> Option<(Arc<str>, usize)> {
        let (page, offset) = self.lines.utf8_page(byte)?;
        let key = PageKey::new(self.document_id, self.source.source_generation(), page);
        let text = self.cache.get(&key).or_else(|| {
            self.missing_pages.insert(page);
            None
        })?;
        Some((text, offset))
    }

    /// 全局 UTF-8 位置按完整字素移动；跨页上下文只读缓存，不物化整行。
    pub(crate) fn previous_grapheme_boundary(&mut self, byte: usize) -> Option<usize> {
        let total = self.lines.total_utf8_bytes();
        if byte == 0 || byte > total {
            return None;
        }
        let (mut chunk, mut chunk_start) = self.cached_chunk(byte)?;
        let mut cursor = GraphemeCursor::new(byte, total, true);
        loop {
            match cursor.prev_boundary(&chunk, chunk_start) {
                Ok(boundary) => return boundary.or(Some(0)),
                Err(GraphemeIncomplete::PrevChunk) => {
                    let previous = cursor.cur_cursor().checked_sub(1)?;
                    (chunk, chunk_start) = self.cached_chunk(previous)?;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let previous = end.checked_sub(1)?;
                    let (context, start) = self.cached_chunk(previous)?;
                    cursor.provide_context(&context[..end - start], start);
                }
                Err(_) => return None,
            }
        }
    }

    pub(crate) fn next_grapheme_boundary(&mut self, byte: usize) -> Option<usize> {
        let total = self.lines.total_utf8_bytes();
        if byte >= total {
            return None;
        }
        let (mut chunk, mut chunk_start) = self.cached_chunk(byte)?;
        let mut cursor = GraphemeCursor::new(byte, total, true);
        loop {
            match cursor.next_boundary(&chunk, chunk_start) {
                Ok(boundary) => return boundary.or(Some(total)),
                Err(GraphemeIncomplete::NextChunk) => {
                    let offset = cursor.cur_cursor();
                    if offset >= total {
                        return Some(total);
                    }
                    (chunk, chunk_start) = self.cached_chunk(offset)?;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    let previous = end.checked_sub(1)?;
                    let (context, start) = self.cached_chunk(previous)?;
                    cursor.provide_context(&context[..end - start], start);
                }
                Err(_) => return None,
            }
        }
    }

    fn cached_line(
        &mut self,
        index: usize,
    ) -> Option<(PagedLineLocation, Arc<str>, Range<usize>, bool)> {
        let (page, local_start, page_offset) = self.lines.location(index)?;
        let key = PageKey::new(self.document_id, self.source.source_generation(), page);
        let page_text = self.cache.get(&key).or_else(|| {
            self.missing_pages.insert(page);
            None
        })?;
        let mut local_byte = 0;
        for (local, segment) in page_text.split('\n').enumerate().take(local_start + 1) {
            if local == local_start {
                let ended = self.lines.line_ends_in_page(page, local)?;
                return Some((
                    PagedLineLocation {
                        index,
                        byte_start: page_offset + local_byte,
                        page,
                    },
                    page_text.clone(),
                    local_byte..local_byte + segment.len(),
                    ended || page + 1 == self.source.index().len(),
                ));
            }
            local_byte += segment.len() + 1;
        }
        None
    }

    fn scan_line(
        &mut self,
        index: usize,
        columns: Range<usize>,
        target: Option<PagedTarget>,
    ) -> Option<PagedScan> {
        let (location, page_text, segment, complete) = self.cached_line(index)?;
        self.read_window(location, page_text, segment, complete, &columns, target)
    }

    fn line_start(&mut self, index: usize) -> Option<usize> {
        self.cached_line(index).map(|line| line.0.byte_start)
    }

    pub(crate) fn line_byte_range(&mut self, index: usize) -> Option<Range<usize>> {
        if index >= self.lines.line_count() {
            return None;
        }
        if self.source.index().is_empty() {
            return (index == 0).then_some(0..0);
        }
        let start = self.line_start(index)?;
        let end = if index + 1 < self.lines.line_count() {
            self.line_start(index + 1)?.checked_sub(1)?
        } else {
            self.lines.total_utf8_bytes()
        };
        Some(start..end)
    }

    pub(crate) fn column_at(&mut self, index: usize, byte: usize) -> Option<usize> {
        self.scan_line(index, 0..usize::MAX, Some(PagedTarget::Byte(byte)))
            .and_then(|scan| scan.target_column)
    }

    pub(crate) fn byte_at_column(&mut self, index: usize, column: usize) -> Option<(usize, usize)> {
        self.scan_line(index, 0..usize::MAX, Some(PagedTarget::Column(column)))
            .and_then(|scan| scan.target_position)
    }

    /// 保留字素文本状态跨页扫描；页接缝重建游标并先判边界，避免 RI 预上下文重复计数。
    fn read_window(
        &mut self,
        location: PagedLineLocation,
        mut page_text: Arc<str>,
        mut segment_range: Range<usize>,
        complete_in_page: bool,
        columns: &Range<usize>,
        target: Option<PagedTarget>,
    ) -> Option<PagedScan> {
        let PagedLineLocation {
            index,
            byte_start: line_start,
            mut page,
        } = location;
        if columns.start >= columns.end {
            return Some(PagedWindow::default().finish(index, line_start, 0));
        }
        let total = if complete_in_page {
            segment_range.len()
        } else {
            usize::MAX
        };
        let mut cursor = GraphemeCursor::new(0, total, true);
        let initial_page = page;
        let mut chunk_start = 0;
        let mut grapheme_start = 0;
        let mut continued_grapheme = String::new();
        let mut window = match target {
            Some(PagedTarget::Byte(byte)) => PagedWindow::byte_navigation(byte),
            Some(PagedTarget::Column(column)) => PagedWindow::column_navigation(column),
            None => PagedWindow::painted(),
        };
        let mut check_seam = false;
        loop {
            let segment = &page_text[segment_range.clone()];
            if check_seam && !segment.is_empty() {
                match cursor.is_boundary(segment, chunk_start) {
                    Ok(boundary) => {
                        if boundary {
                            if window.push(
                                &self.display,
                                &continued_grapheme,
                                line_start,
                                grapheme_start,
                                columns,
                            ) {
                                return Some(window.finish(index, line_start, chunk_start));
                            }
                            continued_grapheme.clear();
                            grapheme_start = chunk_start;
                        }
                        check_seam = false;
                    }
                    Err(GraphemeIncomplete::PreContext(end)) => {
                        self.provide_context(&mut cursor, end, line_start, chunk_start, segment)?;
                        continue;
                    }
                    Err(_) => return None,
                }
            }
            match cursor.next_boundary(segment, chunk_start) {
                Ok(Some(end)) => {
                    let local_end = end - chunk_start;
                    let grapheme = if grapheme_start < chunk_start {
                        continued_grapheme.push_str(&segment[..local_end]);
                        continued_grapheme.as_str()
                    } else {
                        &segment[grapheme_start - chunk_start..local_end]
                    };
                    if window.push(&self.display, grapheme, line_start, grapheme_start, columns) {
                        return Some(window.finish(index, line_start, end));
                    }
                    continued_grapheme.clear();
                    grapheme_start = end;
                }
                Ok(None) => return Some(window.finish(index, line_start, segment.len())),
                Err(GraphemeIncomplete::NextChunk) => {
                    let end = cursor.cur_cursor();
                    let local_end = end - chunk_start;
                    if grapheme_start < chunk_start {
                        continued_grapheme.push_str(&segment[..local_end]);
                    } else {
                        continued_grapheme
                            .push_str(&segment[grapheme_start - chunk_start..local_end]);
                    }
                    let ended = page != initial_page && self.lines.line_ends_in_page(page, 0)?;
                    if complete_in_page || ended || page + 1 == self.source.index().len() {
                        let grapheme = if grapheme_start < chunk_start {
                            continued_grapheme.as_str()
                        } else {
                            &segment[grapheme_start - chunk_start..local_end]
                        };
                        window.push(&self.display, grapheme, line_start, grapheme_start, columns);
                        return Some(window.finish(index, line_start, end));
                    }
                    chunk_start = end;
                    page += 1;
                    let key = PageKey::new(self.document_id, self.source.source_generation(), page);
                    let Some(next_page) = self.cache.get(&key) else {
                        self.missing_pages.insert(page);
                        return None;
                    };
                    page_text = next_page;
                    let segment_len = page_text.find('\n').unwrap_or(page_text.len());
                    segment_range = 0..segment_len;
                    // 上游 RI 状态若直接续传，会在 PreContext 中重复计数；重建后先判接缝。
                    cursor = GraphemeCursor::new(end, total, true);
                    check_seam = true;
                }
                Err(GraphemeIncomplete::PreContext(end)) => {
                    self.provide_context(&mut cursor, end, line_start, chunk_start, segment)?;
                }
                Err(_) => return None,
            }
        }
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
                    display_start: 0,
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
                self.missing_pages.insert(page);
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
                        display_start: 0,
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

impl PagedTextSource<'_> {
    /// 分页行只物化可视列；窗口在页接缝前满足后不读取续页，接缝处保留字素状态。
    pub(crate) fn visible_line_windows(
        &mut self,
        lines: Range<usize>,
        columns: Range<usize>,
    ) -> Vec<VisibleRow> {
        self.missing_pages.clear();
        let start = lines.start.min(self.line_count());
        let end = lines.end.min(self.line_count()).max(start);
        let mut rows = Vec::with_capacity(end - start);
        if self.lines.location(start).is_none() {
            if start == 0 && end == 1 && self.source.index().is_empty() {
                rows.push(VisibleRow::Text(VisibleLine {
                    index: 0,
                    byte_range: 0..0,
                    text: String::new(),
                    display_start: 0,
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
                self.missing_pages.insert(page);
                rows.extend((index..row_end).map(|index| VisibleRow::Loading { index }));
                index = row_end;
                continue;
            };

            let mut local_byte = 0;
            for (local, segment) in page_text.split('\n').enumerate().take(local_end) {
                if local >= local_start {
                    let line_index = index + local - local_start;
                    let byte_start = page_offset + local_byte;
                    let ended = self.lines.line_ends_in_page(page, local).unwrap_or(false);
                    let line = self.read_window(
                        PagedLineLocation {
                            index: line_index,
                            byte_start,
                            page,
                        },
                        page_text.clone(),
                        local_byte..local_byte + segment.len(),
                        ended || page + 1 == self.source.index().len(),
                        &columns,
                        None,
                    );
                    let line = line.map(|scan| scan.line);
                    rows.push(
                        line.map_or(VisibleRow::Loading { index: line_index }, VisibleRow::Text),
                    );
                }
                local_byte += segment.len() + 1;
            }
            index = row_end;
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    use ropey::{LineType, Rope};
    use slterm_document::{
        DocumentPageCache, PageIndex, PageKey, PagedLineIndex, PagedSource, SourceByteRange,
    };

    use super::{PagedTextSource, RopeTextSource, TextSource};
    use crate::app_shell::text_view::metrics::DisplayMap;

    thread_local! {
        static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
        static ALLOCATED_BYTES: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    fn record_allocation(size: usize) {
        let _ = TRACK_ALLOCATIONS.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATED_BYTES.try_with(|bytes| bytes.set(bytes.get() + size));
            }
        });
    }

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record_allocation(layout.size());
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            record_allocation(size);
            unsafe { System.realloc(pointer, layout, size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

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
        let lines = source.visible_line_windows(0..1, 0..4);
        assert_eq!(lines[0].text(), "aaaa");
        assert_eq!(lines[0].byte_range(), 0..4);

        let rope = Rope::from_str(&format!("e\u{301}{}", "b".repeat(100_000)));
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_line_windows(0..1, 0..1);
        assert_eq!(lines[0].text(), "e\u{301}");
        assert_eq!(lines[0].byte_range(), 0..3);

        let rope = Rope::from_str("abcdef");
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_line_windows(0..1, 2..5);
        assert_eq!(lines[0].text(), "cde");
        assert_eq!(lines[0].byte_range(), 2..5);
        assert_eq!(lines[0].display_start(), 2);

        let rope = Rope::from_str("中a");
        let source = RopeTextSource::new(&rope, DisplayMap::new(4));
        let lines = source.visible_line_windows(0..1, 1..2);
        assert_eq!(lines[0].text(), "中");
        assert_eq!(lines[0].display_start(), 0);
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
        let lines = PagedLineIndex::from_page_stats(&[(1, 4, true), (1, 5, true)]);
        let source = PagedSource::new(pages, [7; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "中\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));

        let rows = text.visible_lines(0..3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].as_text().unwrap().text(), "中");
        assert_eq!(rows[0].as_text().unwrap().byte_range(), 0..3);
        assert!(rows[1].as_text().is_none());
        assert!(rows[2].as_text().is_none());
        assert_eq!(rows[1].index(), 1);

        let _ = text;
        cache.insert(PageKey::new(id, 0, 1), "😀\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
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
        let lines = PagedLineIndex::from_page_stats(&[(0, 3, false), (2, 10, true)]);
        let source = PagedSource::new(pages, [9; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "abc".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        assert!(text.visible_lines(0..1)[0].as_text().is_none());
        let _ = text;

        cache.insert(PageKey::new(id, 0, 1), "😀\nnext\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_lines(0..3);
        assert_eq!(rows[0].as_text().unwrap().text(), "abc😀");
        assert_eq!(rows[0].as_text().unwrap().byte_range(), 0..7);
        assert_eq!(rows[1].as_text().unwrap().text(), "next");
        assert_eq!(rows[1].as_text().unwrap().byte_range(), 8..12);
        assert_eq!(rows[2].as_text().unwrap().byte_range(), 13..13);
    }

    #[test]
    fn paged_windows_clip_tabs_and_do_not_wait_for_unseen_continuation() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(3, vec![SourceByteRange::new(0, 3).unwrap()]).unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(1, 4, true)]);
        let source = PagedSource::new(pages, [5; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "A\tB\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 2..5);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "\tB");
        assert_eq!(line.byte_range(), 1..3);
        assert_eq!(line.display_start(), 1);
    }

    #[test]
    fn paged_cross_page_window_does_not_require_unseen_tail() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            8,
            vec![
                SourceByteRange::new(0, 3).unwrap(),
                SourceByteRange::new(3, 8).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 3, false), (1, 5, true)]);
        let source = PagedSource::new(pages, [6; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "abc".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 0..2);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "ab");
        assert_eq!(line.byte_range(), 0..2);
        let rows = text.visible_line_windows(0..1, 0..4);
        assert!(rows[0].as_text().is_none());
        let _ = text;

        cache.insert(PageKey::new(id, 0, 1), "😀\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 2..5);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "c😀");
        assert_eq!(line.byte_range(), 2..7);
        assert_eq!(line.display_start(), 2);
    }

    #[test]
    fn paged_navigation_preserves_zero_width_line_prefixes() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let content = "\u{301}A\nx";
        let pages = PageIndex::new(
            content.len() as u64,
            vec![SourceByteRange::new(0, content.len() as u64).unwrap()],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(1, content.len(), false)]);
        let source = PagedSource::new(pages, [13; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), content.to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));

        assert_eq!(text.line_byte_range(0), Some(0..3));
        assert_eq!(text.byte_at_column(0, 0), Some((0, 0)));
    }

    #[test]
    fn paged_column_navigation_does_not_materialize_the_target_line() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let target_len = 256 * 1024;
        let content = format!("a\n{}", "b".repeat(target_len));
        let pages = PageIndex::new(
            content.len() as u64,
            vec![SourceByteRange::new(0, content.len() as u64).unwrap()],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(1, content.len(), false)]);
        let source = PagedSource::new(pages, [13; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), content);
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));

        ALLOCATED_BYTES.with(|bytes| bytes.set(0));
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
        let position = text.byte_at_column(1, target_len + 10);
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        let allocated = ALLOCATED_BYTES.with(Cell::get);

        assert_eq!(position, Some((target_len + 2, target_len)));
        assert!(
            allocated < 64 * 1024,
            "导航物化目标行分配了 {allocated} 字节"
        );
    }

    #[test]
    fn paged_window_does_not_allocate_skipped_page_segments() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let first_len = 256 * 1024;
        let pages = PageIndex::new(
            (first_len + 2) as u64,
            vec![
                SourceByteRange::new(0, first_len as u64).unwrap(),
                SourceByteRange::new(first_len as u64, (first_len + 2) as u64).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, first_len, false), (1, 2, true)]);
        let source = PagedSource::new(pages, [13; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "a".repeat(first_len));
        cache.insert(PageKey::new(id, 0, 1), "b\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));

        ALLOCATED_BYTES.with(|bytes| bytes.set(0));
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
        let rows = text.visible_line_windows(0..1, first_len - 1..first_len + 1);
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        let allocated = ALLOCATED_BYTES.with(Cell::get);

        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "ab");
        assert_eq!(line.byte_range(), first_len - 1..first_len + 1);
        assert!(allocated < 64 * 1024, "可视窗口分配了 {allocated} 字节");
    }

    #[test]
    fn paged_window_continues_a_nonfirst_line_at_the_page_end() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            13,
            vec![
                SourceByteRange::new(0, 8).unwrap(),
                SourceByteRange::new(8, 13).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(1, 8, false), (1, 5, true)]);
        let source = PagedSource::new(pages, [12; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "skip\nabc".to_owned());
        cache.insert(PageKey::new(id, 0, 1), "😀\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(1..2, 0..4);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "abc😀");
        assert_eq!(line.byte_range(), 5..12);
    }

    #[test]
    fn paged_cross_page_window_preserves_graphemes_at_the_seam() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            5,
            vec![
                SourceByteRange::new(0, 1).unwrap(),
                SourceByteRange::new(1, 5).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 1, false), (1, 4, true)]);
        let source = PagedSource::new(pages, [8; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "e".to_owned());
        cache.insert(PageKey::new(id, 0, 1), "\u{301}\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 0..1);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "e\u{301}");
        assert_eq!(line.byte_range(), 0..3);
    }

    #[test]
    fn paged_cross_page_window_preserves_zwj_and_regional_indicator_sequences() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            12,
            vec![
                SourceByteRange::new(0, 4).unwrap(),
                SourceByteRange::new(4, 7).unwrap(),
                SourceByteRange::new(7, 12).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 4, false), (0, 3, false), (1, 5, true)]);
        let source = PagedSource::new(pages, [10; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "👨".to_owned());
        cache.insert(PageKey::new(id, 0, 1), "\u{200d}".to_owned());
        cache.insert(PageKey::new(id, 0, 2), "👩\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 0..2);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "👨\u{200d}👩");
        assert_eq!(line.byte_range(), 0..11);
        assert_eq!(line.display_width(), 2);

        let indicators = "🇦🇧🇨🇩\n";
        let pages = PageIndex::new(
            17,
            vec![
                SourceByteRange::new(0, 4).unwrap(),
                SourceByteRange::new(4, 12).unwrap(),
                SourceByteRange::new(12, 17).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 4, false), (0, 8, false), (1, 5, true)]);
        let source = PagedSource::new(pages, [11; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "🇦".to_owned());
        cache.insert(PageKey::new(id, 0, 1), "🇧🇨".to_owned());
        cache.insert(PageKey::new(id, 0, 2), "🇩\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 0..4);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), indicators.trim_end_matches('\n'));
        assert_eq!(line.byte_range(), 0..16);
    }

    fn assert_same_line(
        actual: &super::VisibleLine,
        expected: &super::VisibleLine,
        split: usize,
        columns: &std::ops::Range<usize>,
    ) {
        assert_eq!(
            actual.index(),
            expected.index(),
            "切分 {split}, 窗口 {columns:?}"
        );
        assert_eq!(
            actual.text(),
            expected.text(),
            "切分 {split}, 窗口 {columns:?}"
        );
        assert_eq!(
            actual.byte_range(),
            expected.byte_range(),
            "切分 {split}, 窗口 {columns:?}"
        );
        assert_eq!(
            actual.display_start(),
            expected.display_start(),
            "切分 {split}, 窗口 {columns:?}"
        );
        assert_eq!(
            actual.display_width(),
            expected.display_width(),
            "切分 {split}, 窗口 {columns:?}"
        );
    }

    #[test]
    fn paged_windows_match_rope_at_every_character_seam() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let content = "head\n\u{301}A\u{301}👨\u{200d}👩🇦🇧🇨🇩\tZ\n";
        let rope = Rope::from_str(content);
        let map = DisplayMap::new(4);
        let windows = [0..0, 0..1, 1..2, 0..2, 2..3, 3..8, 5..200, 200..300];
        for split in content.char_indices().map(|(index, _)| index).skip(1) {
            let (first, second) = content.split_at(split);
            let pages = PageIndex::new(
                content.len() as u64,
                vec![
                    SourceByteRange::new(0, split as u64).unwrap(),
                    SourceByteRange::new(split as u64, content.len() as u64).unwrap(),
                ],
            )
            .unwrap();
            let lines = PagedLineIndex::from_page_stats(&[
                (
                    first.bytes().filter(|byte| *byte == b'\n').count(),
                    first.len(),
                    first.ends_with('\n'),
                ),
                (
                    second.bytes().filter(|byte| *byte == b'\n').count(),
                    second.len(),
                    second.ends_with('\n'),
                ),
            ]);
            let source = PagedSource::new(pages, [14; 32], lines);
            let mut cache = DocumentPageCache::new();
            cache.insert(PageKey::new(id, 0, 0), first.to_owned());
            cache.insert(PageKey::new(id, 0, 1), second.to_owned());
            let mut paged = PagedTextSource::new(id, &source, &mut cache, map);
            let rope_source = RopeTextSource::new(&rope, map);
            for columns in &windows {
                let expected = rope_source
                    .visible_line_windows(0..rope.len_lines(LineType::LF), columns.clone());
                let actual =
                    paged.visible_line_windows(0..rope.len_lines(LineType::LF), columns.clone());
                assert_eq!(actual.len(), expected.len(), "切分字节 {split}");
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_same_line(actual.as_text().unwrap(), expected, split, columns);
                }
            }
        }
    }

    #[test]
    fn paged_cross_page_grapheme_requires_each_continuation_page() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            12,
            vec![
                SourceByteRange::new(0, 4).unwrap(),
                SourceByteRange::new(4, 7).unwrap(),
                SourceByteRange::new(7, 12).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 4, false), (0, 3, false), (1, 5, true)]);
        let source = PagedSource::new(pages, [15; 32], lines);
        let segments = ["👨", "\u{200d}", "👩\n"];
        for missing in 0..segments.len() {
            let mut cache = DocumentPageCache::new();
            for (page, segment) in segments.iter().enumerate() {
                if page != missing {
                    cache.insert(PageKey::new(id, 0, page), (*segment).to_owned());
                }
            }
            let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
            assert!(text.visible_line_windows(0..1, 0..2)[0].as_text().is_none());
            assert_eq!(
                text.missing_pages().iter().copied().collect::<Vec<_>>(),
                vec![missing]
            );
        }
    }

    #[test]
    fn paged_window_skips_zero_length_decoded_pages() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(
            4,
            vec![
                SourceByteRange::new(0, 1).unwrap(),
                SourceByteRange::new(1, 2).unwrap(),
                SourceByteRange::new(2, 4).unwrap(),
            ],
        )
        .unwrap();
        let lines = PagedLineIndex::from_page_stats(&[(0, 1, false), (0, 0, false), (1, 2, true)]);
        let source = PagedSource::new(pages, [16; 32], lines);
        let mut cache = DocumentPageCache::new();
        cache.insert(PageKey::new(id, 0, 0), "a".to_owned());
        cache.insert(PageKey::new(id, 0, 1), String::new());
        cache.insert(PageKey::new(id, 0, 2), "b\n".to_owned());
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));
        let rows = text.visible_line_windows(0..1, 0..2);
        let line = rows[0].as_text().unwrap();
        assert_eq!(line.text(), "ab");
        assert_eq!(line.byte_range(), 0..2);
    }

    #[test]
    fn paged_source_does_not_fake_empty_text_for_a_nonempty_source() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let pages = PageIndex::new(1, vec![SourceByteRange::new(0, 1).unwrap()]).unwrap();
        let lines = PagedLineIndex::from_page_stats(&[]);
        let source = PagedSource::new(pages, [3; 32], lines);
        let mut cache = DocumentPageCache::new();
        let mut text = PagedTextSource::new(id, &source, &mut cache, DisplayMap::new(4));

        let rows = text.visible_lines(0..1);
        assert!(rows[0].as_text().is_none());
        assert_eq!(rows[0].index(), 0);
    }
}
