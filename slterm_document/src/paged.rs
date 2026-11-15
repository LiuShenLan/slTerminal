//! 只读分页的源范围索引与全应用解码页缓存。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use ropey::Rope;

use crate::DocumentId;

/// Document 的唯一正文形态；Paged 不持伪造的空 Rope 或完整实时文本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentBody {
    Editable(Rope),
    Paged(PagedSource),
}

impl From<Rope> for DocumentBody {
    fn from(text: Rope) -> Self {
        Self::Editable(text)
    }
}

impl From<PagedSource> for DocumentBody {
    fn from(source: PagedSource) -> Self {
        Self::Paged(source)
    }
}

/// 只读分页源及其单调代次；代次用于淘汰旧外部版本的缓存页。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagedSource {
    index: PageIndex,
    line_index: PagedLineIndex,
    fingerprint: [u8; 32],
    source_generation: u64,
}

impl PagedSource {
    pub fn new(index: PageIndex, fingerprint: [u8; 32], line_index: PagedLineIndex) -> Self {
        Self {
            index,
            line_index,
            fingerprint,
            source_generation: 0,
        }
    }

    pub const fn index(&self) -> &PageIndex {
        &self.index
    }

    pub const fn line_index(&self) -> &PagedLineIndex {
        &self.line_index
    }

    pub const fn source_generation(&self) -> u64 {
        self.source_generation
    }

    pub const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    pub(crate) fn with_generation(mut self, source_generation: u64) -> Self {
        self.source_generation = source_generation;
        self
    }
}

/// 单页源字节上限；页边界可在编码与行边界处提前结束。
pub const PAGE_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
/// 全应用共享的解码页缓存上限。
pub const DECODED_PAGE_CACHE_LIMIT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageIndexError {
    EmptyRange,
    PageTooLarge,
    NonContiguous,
    LengthMismatch,
}

/// 源文件中的左闭右开字节范围。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceByteRange {
    start: u64,
    end: u64,
}

impl SourceByteRange {
    pub fn new(start: u64, end: u64) -> Result<Self, PageIndexError> {
        if start >= end {
            return Err(PageIndexError::EmptyRange);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }

    pub const fn len(self) -> u64 {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        false
    }
}

/// 只保存连续源范围，不复制源文件正文。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageIndex {
    total_bytes: u64,
    ranges: Vec<SourceByteRange>,
}

impl PageIndex {
    pub fn new(total_bytes: u64, ranges: Vec<SourceByteRange>) -> Result<Self, PageIndexError> {
        let mut expected_start = 0;
        for range in &ranges {
            if range.len() > PAGE_SOURCE_BYTES {
                return Err(PageIndexError::PageTooLarge);
            }
            if range.start != expected_start {
                return Err(PageIndexError::NonContiguous);
            }
            expected_start = range.end;
        }
        if expected_start != total_bytes {
            return Err(PageIndexError::LengthMismatch);
        }
        Ok(Self {
            total_bytes,
            ranges,
        })
    }

    pub fn page(&self, page: usize) -> Option<SourceByteRange> {
        self.ranges.get(page).copied()
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

/// 每页换行数、换行结尾与 UTF-8 长度形成的只读行索引；不保存正文。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagedLineIndex {
    first_lines: Vec<usize>,
    utf8_offsets: Vec<usize>,
    newlines: Vec<usize>,
    continuations: Vec<bool>,
    line_count: usize,
    total_utf8_bytes: usize,
}

impl PagedLineIndex {
    /// stats 每项为（页内换行数，页解码后的 UTF-8 字节数，是否以换行结束）。
    pub fn from_page_stats(stats: &[(usize, usize, bool)]) -> Self {
        let mut first_lines = Vec::with_capacity(stats.len());
        let mut utf8_offsets = Vec::with_capacity(stats.len());
        let mut newlines_in_pages = Vec::with_capacity(stats.len());
        let mut continuations = Vec::with_capacity(stats.len());
        let mut line_count = 0;
        let mut utf8_offset = 0;
        let mut previous_ended = true;
        for (index, (newlines, utf8_bytes, ended)) in stats.iter().copied().enumerate() {
            first_lines.push(line_count);
            utf8_offsets.push(utf8_offset);
            newlines_in_pages.push(newlines);
            continuations.push(index > 0 && !previous_ended);
            line_count += newlines + usize::from(index + 1 == stats.len());
            utf8_offset += utf8_bytes;
            previous_ended = ended;
        }
        if stats.is_empty() {
            line_count = 1;
        }
        Self {
            first_lines,
            utf8_offsets,
            newlines: newlines_in_pages,
            continuations,
            line_count,
            total_utf8_bytes: utf8_offset,
        }
    }

    pub const fn line_count(&self) -> usize {
        self.line_count
    }

    pub const fn total_utf8_bytes(&self) -> usize {
        self.total_utf8_bytes
    }

    /// 返回（页号，页内行号，页 UTF-8 起点）；空文档没有真实页。
    pub fn location(&self, line: usize) -> Option<(usize, usize, usize)> {
        if line >= self.line_count || self.first_lines.is_empty() {
            return None;
        }
        let next = self.first_lines.partition_point(|first| *first <= line);
        let mut page = next.checked_sub(1)?;
        while page > 0 && self.continuations[page] && self.first_lines[page] == line {
            page -= 1;
        }
        Some((page, line - self.first_lines[page], self.utf8_offsets[page]))
    }

    /// 返回包含目标 UTF-8 偏移的页候选与页起点；调用方按真实页长复核边界。
    pub fn utf8_page(&self, byte: usize) -> Option<(usize, usize)> {
        let next = self.utf8_offsets.partition_point(|offset| *offset <= byte);
        let page = next.checked_sub(1)?;
        Some((page, self.utf8_offsets[page]))
    }

    pub fn page_utf8_offset(&self, page: usize) -> Option<usize> {
        self.utf8_offsets.get(page).copied()
    }

    pub fn line_ends_in_page(&self, page: usize, line_in_page: usize) -> Option<bool> {
        self.newlines
            .get(page)
            .map(|newlines| line_in_page < *newlines)
    }
}

/// 缓存键同时绑定 Document、源代次与页号，旧外部版本不得命中新内容。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageKey {
    document_id: DocumentId,
    source_generation: u64,
    page: usize,
}

impl PageKey {
    pub const fn new(document_id: DocumentId, source_generation: u64, page: usize) -> Self {
        Self {
            document_id,
            source_generation,
            page,
        }
    }

    pub const fn document_id(self) -> DocumentId {
        self.document_id
    }
}

/// 全应用共享的解码页 LRU；不保存可编辑或脏正文。
pub struct DocumentPageCache {
    capacity: usize,
    used_bytes: usize,
    pages: BTreeMap<PageKey, Arc<str>>,
    lru: VecDeque<PageKey>,
}

impl DocumentPageCache {
    pub fn new() -> Self {
        Self::with_capacity(DECODED_PAGE_CACHE_LIMIT_BYTES)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            used_bytes: 0,
            pages: BTreeMap::new(),
            lru: VecDeque::new(),
        }
    }

    /// 返回共享页句柄；调用方可在后续缓存操作时保留文本切片，无需复制页内容。
    pub fn get(&mut self, key: &PageKey) -> Option<Arc<str>> {
        if !self.pages.contains_key(key) {
            return None;
        }
        self.touch(*key);
        self.pages.get(key).cloned()
    }

    /// 返回是否进入缓存；超过总容量的单页只读返回，不驱逐其他页。
    pub fn insert(&mut self, key: PageKey, text: String) -> bool {
        self.remove(key);
        if text.len() > self.capacity {
            return false;
        }
        let text: Arc<str> = text.into();
        while self.used_bytes + text.len() > self.capacity {
            let Some(oldest) = self.lru.pop_front() else {
                break;
            };
            if let Some(removed) = self.pages.remove(&oldest) {
                self.used_bytes -= removed.len();
            }
        }
        self.used_bytes += text.len();
        self.lru.push_back(key);
        self.pages.insert(key, text);
        true
    }

    pub fn remove_document(&mut self, document_id: DocumentId) {
        self.pages.retain(|key, _| key.document_id != document_id);
        self.lru.retain(|key| key.document_id != document_id);
        self.used_bytes = self.pages.values().map(|text| text.len()).sum();
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub const fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn remove(&mut self, key: PageKey) {
        if let Some(removed) = self.pages.remove(&key) {
            self.used_bytes -= removed.len();
            self.lru.retain(|candidate| *candidate != key);
        }
    }

    fn touch(&mut self, key: PageKey) {
        self.lru.retain(|candidate| *candidate != key);
        self.lru.push_back(key);
    }
}

impl Default for DocumentPageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{
        DocumentPageCache, PAGE_SOURCE_BYTES, PageIndex, PageKey, PagedLineIndex, PagedSource,
        SourceByteRange,
    };
    use crate::DocumentId;

    fn document() -> DocumentId {
        DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap()
    }

    #[test]
    fn paged_source_preserves_its_line_index_across_clones() {
        let source = PagedSource::new(
            PageIndex::new(0, Vec::new()).unwrap(),
            [4; 32],
            PagedLineIndex::from_page_stats(&[(2, 6, true)]),
        );
        assert_eq!(source.line_index(), source.clone().line_index());
        assert_eq!(source.line_index().line_count(), 3);
    }

    #[test]
    fn page_index_requires_contiguous_bounded_source_ranges() {
        let empty = PageIndex::new(0, Vec::new()).unwrap();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
        let index = PageIndex::new(
            8,
            vec![
                SourceByteRange::new(0, 4).unwrap(),
                SourceByteRange::new(4, 8).unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(index.len(), 2);
        assert!(!index.is_empty());
        assert_eq!(index.page(1), Some(SourceByteRange::new(4, 8).unwrap()));
        assert_eq!(index.page(2), None);

        assert!(SourceByteRange::new(4, 4).is_err());
        assert!(
            PageIndex::new(
                8,
                vec![SourceByteRange::new(0, PAGE_SOURCE_BYTES + 1).unwrap()],
            )
            .is_err()
        );
        assert!(
            PageIndex::new(
                8,
                vec![
                    SourceByteRange::new(0, 3).unwrap(),
                    SourceByteRange::new(4, 8).unwrap(),
                ],
            )
            .is_err()
        );
        assert!(PageIndex::new(8, vec![SourceByteRange::new(0, 4).unwrap()]).is_err());
    }

    #[test]
    fn paged_line_index_maps_global_lines_and_utf8_offsets() {
        let index =
            PagedLineIndex::from_page_stats(&[(1, "中\n".len(), true), (1, "😀\n".len(), true)]);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.total_utf8_bytes(), 9);
        assert_eq!(index.location(0), Some((0, 0, 0)));
        assert_eq!(index.location(1), Some((1, 0, 4)));
        assert_eq!(index.location(2), Some((1, 1, 4)));
        assert_eq!(index.location(3), None);
        assert_eq!(index.utf8_page(0), Some((0, 0)));
        assert_eq!(index.utf8_page(3), Some((0, 0)));
        assert_eq!(index.utf8_page(4), Some((1, 4)));
        assert_eq!(index.page_utf8_offset(1), Some(4));
        assert_eq!(index.page_utf8_offset(2), None);

        let empty = PagedLineIndex::from_page_stats(&[]);
        assert_eq!(empty.line_count(), 1);
        assert_eq!(empty.location(0), None);

        let split_line = PagedLineIndex::from_page_stats(&[(0, 3, false), (2, 10, true)]);
        assert_eq!(split_line.line_count(), 3);
        assert_eq!(split_line.location(0), Some((0, 0, 0)));
        assert_eq!(split_line.location(1), Some((1, 1, 3)));
        assert_eq!(split_line.location(2), Some((1, 2, 3)));

        let continued =
            PagedLineIndex::from_page_stats(&[(1, 8, false), (0, 1, false), (1, 5, true)]);
        assert_eq!(continued.location(1), Some((0, 1, 0)));
        assert_eq!(continued.location(2), Some((2, 1, 9)));

        let new_line = PagedLineIndex::from_page_stats(&[(1, 8, true), (1, 5, true)]);
        assert_eq!(new_line.location(1), Some((1, 0, 8)));
    }

    #[test]
    fn decoded_page_cache_evicts_the_global_least_recently_used_page() {
        let mut cache = DocumentPageCache::with_capacity(10);
        assert!(cache.is_empty());
        let first = PageKey::new(document(), 0, 0);
        let second = PageKey::new(document(), 0, 1);
        let third = PageKey::new(document(), 0, 2);
        assert!(cache.insert(first, "aaaa".to_owned()));
        assert!(cache.insert(second, "bbbb".to_owned()));
        assert_eq!(
            cache.get(&first).map(|text| text.to_string()),
            Some("aaaa".to_owned())
        );

        assert!(cache.insert(third, "cccc".to_owned()));

        assert_eq!(
            cache.get(&first).map(|text| text.to_string()),
            Some("aaaa".to_owned())
        );
        assert_eq!(cache.get(&second), None);
        assert_eq!(
            cache.get(&third).map(|text| text.to_string()),
            Some("cccc".to_owned())
        );
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 8);
        assert!(!cache.insert(PageKey::new(document(), 0, 3), "01234567890".to_owned()));
        assert_eq!(cache.used_bytes(), 8);
    }

    #[test]
    fn page_cache_separates_source_generations_and_document_lifetime() {
        let mut cache = DocumentPageCache::with_capacity(32);
        let old = PageKey::new(document(), 1, 0);
        let current = PageKey::new(document(), 2, 0);
        cache.insert(old, "old".to_owned());
        cache.insert(current, "current".to_owned());

        assert_eq!(
            cache.get(&old).map(|text| text.to_string()),
            Some("old".to_owned())
        );
        assert_eq!(
            cache.get(&current).map(|text| text.to_string()),
            Some("current".to_owned())
        );
        cache.remove_document(document());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.used_bytes(), 0);
    }
}
