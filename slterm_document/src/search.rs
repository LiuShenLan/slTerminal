//! Document 后台文本读取、跨块字面量查找与完整复制。

use std::ops::Range;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::{DocumentBody, DocumentId, DocumentRegistry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchError {
    SourceReaderRequired,
    ReadFailed,
    Cancelled,
    ChunkOffsetMismatch,
    OffsetExhausted,
    InvalidTextRange,
}

/// 解码后 UTF-8 文本中的一个连续块；偏移用于跨块结果对账。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChunk {
    offset: usize,
    text: String,
}

impl TextChunk {
    pub fn new(offset: usize, text: String) -> Self {
        Self { offset, text }
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

pub struct TextChunks<'a> {
    chunks: ropey::iter::Chunks<'a>,
    offset: usize,
}

impl Iterator for TextChunks<'_> {
    type Item = Result<TextChunk, SearchError>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        let current = TextChunk::new(self.offset, chunk.to_owned());
        let Some(next_offset) = self.offset.checked_add(chunk.len()) else {
            return Some(Err(SearchError::OffsetExhausted));
        };
        self.offset = next_offset;
        Some(Ok(current))
    }
}

/// 后台查找或复制捕获的不可变 Document 文本事实。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentTextRequest {
    document_id: DocumentId,
    text_revision: u64,
    source_generation: Option<u64>,
    request_generation: u64,
    body: DocumentBody,
}

impl DocumentTextRequest {
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub const fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub const fn source_generation(&self) -> Option<u64> {
        self.source_generation
    }

    pub const fn request_generation(&self) -> u64 {
        self.request_generation
    }

    pub fn paged(&self) -> Option<&crate::PagedSource> {
        match &self.body {
            DocumentBody::Paged(source) => Some(source),
            DocumentBody::Editable(_) => None,
        }
    }

    /// Editable 直接流式读取 Rope；Paged 必须由应用的分页端口提供相同源代次的块。
    pub fn chunks(&self) -> Result<TextChunks<'_>, SearchError> {
        match &self.body {
            DocumentBody::Editable(text) => Ok(TextChunks {
                chunks: text.chunks(),
                offset: 0,
            }),
            DocumentBody::Paged(_) => Err(SearchError::SourceReaderRequired),
        }
    }

    pub fn search(
        &self,
        pattern: &str,
        chunks: impl IntoIterator<Item = Result<TextChunk, SearchError>>,
        cancellation: &Receiver<()>,
    ) -> Result<DocumentSearchResult, SearchError> {
        let mut matches = Vec::new();
        if pattern.is_empty() {
            return Ok(self.result(matches));
        }
        let mut pending = String::new();
        let mut pending_offset = 0;
        let mut nonoverlap_before: usize = 0;
        for chunk in chunks {
            check_cancelled(cancellation)?;
            let chunk = chunk?;
            if chunk.text.is_empty() {
                continue;
            }
            if pending.is_empty() {
                pending_offset = chunk.offset;
            } else {
                let expected = pending_offset
                    .checked_add(pending.len())
                    .ok_or(SearchError::OffsetExhausted)?;
                if chunk.offset != expected {
                    return Err(SearchError::ChunkOffsetMismatch);
                }
            }
            pending.push_str(&chunk.text);
            let search_start = nonoverlap_before
                .saturating_sub(pending_offset)
                .min(pending.len());
            for matched in pending[search_start..].match_indices(pattern) {
                let start = pending_offset + search_start + matched.0;
                if start < nonoverlap_before {
                    continue;
                }
                let end = start + pattern.len();
                matches.push(SearchMatch { range: start..end });
                nonoverlap_before = end;
            }
            let keep = pattern.len().saturating_sub(1).min(pending.len());
            let mut keep_start = pending.len() - keep;
            while !pending.is_char_boundary(keep_start) {
                keep_start -= 1;
            }
            pending_offset += keep_start;
            pending = pending[keep_start..].to_owned();
            check_cancelled(cancellation)?;
        }
        Ok(self.result(matches))
    }

    /// 完整复制所有块，不应用视图分页或前缀上限。
    pub fn copy(
        &self,
        chunks: impl IntoIterator<Item = Result<TextChunk, SearchError>>,
        cancellation: &Receiver<()>,
    ) -> Result<String, SearchError> {
        let mut copied = String::new();
        for chunk in chunks {
            check_cancelled(cancellation)?;
            let chunk = chunk?;
            if chunk.offset != copied.len() {
                return Err(SearchError::ChunkOffsetMismatch);
            }
            copied.push_str(&chunk.text);
            check_cancelled(cancellation)?;
        }
        Ok(copied)
    }

    /// 流式复制指定 UTF-8 范围；到达右边界后不再读取后续块。
    pub fn copy_range(
        &self,
        range: std::ops::Range<usize>,
        chunks: impl IntoIterator<Item = Result<TextChunk, SearchError>>,
        cancellation: &Receiver<()>,
    ) -> Result<String, SearchError> {
        if range.start > range.end {
            return Err(SearchError::InvalidTextRange);
        }
        let mut copied = String::new();
        let mut covered = 0;
        let mut chunks = chunks.into_iter();
        loop {
            if covered >= range.end {
                return Ok(copied);
            }
            check_cancelled(cancellation)?;
            let Some(chunk) = chunks.next() else {
                return Err(SearchError::OffsetExhausted);
            };
            let chunk = chunk?;
            if chunk.offset != covered {
                return Err(SearchError::ChunkOffsetMismatch);
            }
            let end = chunk
                .offset
                .checked_add(chunk.text.len())
                .ok_or(SearchError::OffsetExhausted)?;
            if end <= range.start {
                covered = end;
                continue;
            }
            let start = range.start.max(chunk.offset) - chunk.offset;
            let local_end = range.end.min(end) - chunk.offset;
            if !chunk.text.is_char_boundary(start) || !chunk.text.is_char_boundary(local_end) {
                return Err(SearchError::InvalidTextRange);
            }
            copied.push_str(&chunk.text[start..local_end]);
            covered = end;
        }
    }

    fn result(&self, matches: Vec<SearchMatch>) -> DocumentSearchResult {
        DocumentSearchResult {
            document_id: self.document_id,
            text_revision: self.text_revision,
            source_generation: self.source_generation,
            request_generation: self.request_generation,
            matches,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    range: Range<usize>,
}

impl SearchMatch {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSearchResult {
    document_id: DocumentId,
    text_revision: u64,
    source_generation: Option<u64>,
    request_generation: u64,
    matches: Vec<SearchMatch>,
}

impl DocumentSearchResult {
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub const fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub const fn request_generation(&self) -> u64 {
        self.request_generation
    }

    pub const fn source_generation(&self) -> Option<u64> {
        self.source_generation
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }
}

impl DocumentRegistry {
    pub fn text_request(
        &self,
        id: DocumentId,
        request_generation: u64,
    ) -> Result<DocumentTextRequest, SearchError> {
        let Some(session) = self.session(id) else {
            return Err(SearchError::ReadFailed);
        };
        Ok(DocumentTextRequest {
            document_id: id,
            text_revision: session.text_revision(),
            source_generation: session.paged().map(|source| source.source_generation()),
            request_generation,
            body: session.body.clone(),
        })
    }
}

pub(crate) fn check_cancelled(cancellation: &Receiver<()>) -> Result<(), SearchError> {
    match cancellation.try_recv() {
        Ok(()) | Err(TryRecvError::Disconnected) => Err(SearchError::Cancelled),
        Err(TryRecvError::Empty) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::mpsc::sync_channel;

    use ropey::Rope;
    use slterm_path::{CanonicalPath, ResolvedPath};

    use crate::{
        DocumentId, DocumentMetadata, DocumentRegistry, SearchError, TextChunk, TextStateId,
    };

    fn request() -> crate::DocumentTextRequest {
        let id = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/search.txt").unwrap(), None),
                Rope::from_str("abcdab"),
                TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a").unwrap(),
                std::sync::Arc::from(b"abcdab".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        registry.text_request(id, 41).unwrap()
    }

    #[test]
    fn literal_search_streams_across_chunk_boundaries() {
        let request = request();
        let (_cancel, cancellation) = sync_channel(1);
        let chunks = vec![
            Ok(TextChunk::new(0, "ab".to_owned())),
            Ok(TextChunk::new(2, "cd".to_owned())),
            Ok(TextChunk::new(4, "ab".to_owned())),
        ];

        let result = request.search("cda", chunks, &cancellation).unwrap();

        assert_eq!(result.document_id(), request.document_id());
        assert_eq!(result.text_revision(), 0);
        assert_eq!(result.request_generation(), 41);
        assert_eq!(result.matches()[0].range(), 2..5);
        assert!(
            request
                .search("", request.chunks().unwrap(), &cancellation)
                .unwrap()
                .matches()
                .is_empty()
        );
    }

    #[test]
    fn streaming_literal_search_matches_contiguous_reference() {
        let request = request();
        let (_cancel, cancellation) = sync_channel(1);
        for (text, pattern) in [
            ("ababa", "aba"),
            ("aaaa", "aa"),
            ("中文abc中文", "中文"),
            ("a\nb\na\n", "a\n"),
            ("nothing", "missing"),
        ] {
            let mut boundaries = text
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(text.len()))
                .collect::<Vec<_>>();
            boundaries.sort_unstable();
            boundaries.dedup();
            let expected = text
                .match_indices(pattern)
                .map(|(start, matched)| start..start + matched.len())
                .collect::<Vec<_>>();
            for split in boundaries.iter().copied() {
                let chunks = [(0, &text[..split]), (split, &text[split..])]
                    .into_iter()
                    .filter(|(_, chunk)| !chunk.is_empty())
                    .map(|(offset, chunk)| Ok(TextChunk::new(offset, chunk.to_owned())))
                    .collect::<Vec<_>>();
                let actual = request
                    .search(pattern, chunks, &cancellation)
                    .unwrap()
                    .matches()
                    .iter()
                    .map(|matched| matched.range())
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual, expected,
                    "文本 {text:?}，模式 {pattern:?}，切点 {split}"
                );
            }
            let chunks = boundaries
                .windows(2)
                .map(|range| {
                    let start = range[0];
                    Ok(TextChunk::new(start, text[range[0]..range[1]].to_owned()))
                })
                .collect::<Vec<_>>();
            let actual = request
                .search(pattern, chunks, &cancellation)
                .unwrap()
                .matches()
                .iter()
                .map(|matched| matched.range())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "逐字符切块: {text:?}，模式 {pattern:?}");
        }
    }

    #[test]
    fn copy_is_complete_and_cancellation_is_observed() {
        let request = request();
        let (cancel, cancellation) = sync_channel(1);
        let copied = request
            .copy(request.chunks().unwrap(), &cancellation)
            .unwrap();
        assert_eq!(copied, "abcdab");

        cancel.send(()).unwrap();
        assert_eq!(
            request.copy(request.chunks().unwrap(), &cancellation),
            Err(SearchError::Cancelled)
        );
        cancel.send(()).unwrap();
        assert_eq!(
            request.search("a", request.chunks().unwrap(), &cancellation),
            Err(SearchError::Cancelled)
        );
    }

    #[test]
    fn disconnected_cancellation_stops_before_the_next_chunk() {
        let request = request();
        let (cancel, cancellation) = sync_channel(1);
        let mut cancel = Some(cancel);
        let mut reads = 0;
        let chunks = std::iter::from_fn(|| {
            reads += 1;
            if reads == 1 {
                cancel.take();
                Some(Ok(TextChunk::new(0, "ab".to_owned())))
            } else {
                Some(Ok(TextChunk::new(2, "cd".to_owned())))
            }
        });
        assert_eq!(
            request.copy_range(0..4, chunks, &cancellation),
            Err(SearchError::Cancelled)
        );
        assert_eq!(reads, 1);
    }

    #[test]
    fn copy_range_streams_only_the_selected_extent() {
        let request = request();
        let (_cancel, cancellation) = sync_channel(1);
        let chunks = vec![
            Ok(TextChunk::new(0, "ab".to_owned())),
            Ok(TextChunk::new(2, "cd".to_owned())),
            Ok(TextChunk::new(4, "ab".to_owned())),
            Err(SearchError::ReadFailed),
        ];
        assert_eq!(
            request.copy_range(1..5, chunks, &cancellation),
            Ok("bcda".to_owned())
        );
        assert_eq!(
            request.copy_range(2..2, request.chunks().unwrap(), &cancellation),
            Ok(String::new())
        );
        assert_eq!(
            request.copy_range(0..7, request.chunks().unwrap(), &cancellation),
            Err(SearchError::OffsetExhausted)
        );
        assert_eq!(
            request.copy_range(
                std::hint::black_box(4)..std::hint::black_box(2),
                request.chunks().unwrap(),
                &cancellation,
            ),
            Err(SearchError::InvalidTextRange)
        );

        let unicode = vec![Ok(TextChunk::new(0, "a😀b".to_owned()))];
        assert_eq!(
            request.copy_range(1..3, unicode, &cancellation),
            Err(SearchError::InvalidTextRange)
        );
    }
}
