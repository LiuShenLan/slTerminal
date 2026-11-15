//! 真实文件的只读源范围分页与逐页严格解码。

use std::io::{self, Read as _, Seek as _};
use std::sync::Arc;

use sha2::{Digest as _, Sha256};
use slterm_document::{
    DocumentContent, DocumentEncoding, DocumentTextRequest, LineEnding, PAGE_SOURCE_BYTES,
    PageIndex, PagedLineIndex, PagedSource, SearchError, SourceByteRange, TextChunk, TextEncoding,
};
use slterm_path::ResolvedPath;

/// 单页后台读取端口；实现只能返回该页真实完整解码文本。
pub(crate) type PageReader = Arc<dyn Fn(usize) -> io::Result<String> + Send + Sync>;

/// 分页只读源；索引不保存完整正文，解码页由全应用 LRU 另行持有。
pub(crate) struct PagedDocument {
    location: ResolvedPath,
    encoding: TextEncoding,
    index: PageIndex,
    line_index: PagedLineIndex,
    fingerprint: [u8; 32],
}

impl PagedDocument {
    /// 顺序建立真实源范围；只允许在后台执行体调用。
    pub(crate) fn open(
        location: &ResolvedPath,
        requested: Option<DocumentEncoding>,
    ) -> io::Result<Self> {
        let mut file = std::fs::File::open(location.canonical_path().as_path())?;
        let total_bytes = file.metadata()?.len();
        verify_identity(&file, location)?;
        let (encoding, bom, bom_bytes) = detect_encoding(&mut file, requested)?;
        let mut ranges = Vec::new();
        let mut line_stats = Vec::new();
        let mut hasher = Sha256::new();
        let mut start = 0;
        let mut line_ending = None;
        let mut trailing_newline = false;
        while start < total_bytes {
            file.seek(std::io::SeekFrom::Start(start))?;
            let length = PAGE_SOURCE_BYTES.min(total_bytes - start) as usize;
            let mut bytes = vec![0; length];
            file.read_exact(&mut bytes)?;
            let body_start = if start == 0 { bom_bytes } else { 0 };
            let (_, complete) = encoding
                .decode_prefix(&bytes[body_start..])
                .map_err(|_| invalid_encoding())?;
            bytes.truncate(body_start + complete);
            let mut end = start + bytes.len() as u64;
            if end < total_bytes
                && let Some(line_end) = last_line_end(&bytes, encoding, body_start)
            {
                bytes.truncate(line_end);
                end = start + line_end as u64;
            }
            if end < total_bytes {
                let trailing = trailing_carriage_return_len(&bytes, encoding, body_start);
                bytes.truncate(bytes.len() - trailing);
                end -= trailing as u64;
            }
            if end == start {
                return Err(invalid_encoding());
            }
            let decoded = if start == 0 {
                match DocumentContent::decode(bytes.clone(), total_bytes, Some(encoding)) {
                    DocumentContent::Editable(text) | DocumentContent::Paged(text) => text.text,
                    DocumentContent::Unavailable(_) => return Err(invalid_encoding()),
                }
            } else {
                encoding
                    .decode(&bytes[body_start..])
                    .map(|text| text.replace("\r\n", "\n"))
                    .map_err(|_| invalid_encoding())?
            };
            line_stats.push((
                decoded.bytes().filter(|byte| *byte == b'\n').count(),
                decoded.len(),
                decoded.ends_with('\n'),
            ));
            if line_ending.is_none() {
                line_ending = first_line_ending(&bytes, encoding, body_start);
            }
            trailing_newline = decoded.ends_with('\n');
            hasher.update(&bytes);
            ranges.push(SourceByteRange::new(start, end).map_err(|_| invalid_encoding())?);
            start = end;
        }
        if file.metadata()?.len() != total_bytes {
            return Err(changed_source());
        }
        verify_identity(&file, location)?;
        let index = PageIndex::new(total_bytes, ranges).map_err(|_| invalid_encoding())?;
        let line_index = PagedLineIndex::from_page_stats(&line_stats);
        Ok(Self {
            location: location.clone(),
            encoding: TextEncoding {
                encoding,
                bom,
                line_ending: line_ending.unwrap_or(LineEnding::Crlf),
                trailing_newline,
            },
            index,
            line_index,
            fingerprint: hasher.finalize().into(),
        })
    }

    pub(crate) const fn index(&self) -> &PageIndex {
        &self.index
    }

    pub(crate) const fn line_index(&self) -> &PagedLineIndex {
        &self.line_index
    }

    pub(crate) fn source(&self) -> PagedSource {
        PagedSource::new(
            self.index.clone(),
            self.fingerprint,
            self.line_index.clone(),
        )
    }

    pub(crate) const fn encoding(&self) -> &TextEncoding {
        &self.encoding
    }

    #[allow(dead_code)] // 消费点 = AppRoot Paged 视图生产接线。
    pub(crate) fn page_reader(self: &Arc<Self>) -> PageReader {
        let document = self.clone();
        Arc::new(move |page| document.read_page(page))
    }

    /// 每次读取复核身份与源长度；旧索引不得读取已增长或已替换的文件。
    pub(crate) fn read_page(&self, page: usize) -> io::Result<String> {
        let range = self
            .index
            .page(page)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "页号越界"))?;
        let mut file = std::fs::File::open(self.location.canonical_path().as_path())?;
        if file.metadata()?.len() != self.index.total_bytes() {
            return Err(changed_source());
        }
        verify_identity(&file, &self.location)?;
        file.seek(std::io::SeekFrom::Start(range.start()))?;
        let mut bytes = vec![0; range.len() as usize];
        file.read_exact(&mut bytes)?;
        let body_start = if range.start() == 0 && self.encoding.bom {
            bom_len(self.encoding.encoding)
        } else {
            0
        };
        self.encoding
            .encoding
            .decode(&bytes[body_start..])
            .map(|text| text.replace("\r\n", "\n"))
            .map_err(|_| invalid_encoding())
    }

    /// 分页块只允许读取请求捕获的同一文件指纹；调用与读取均在后台执行。
    pub(crate) fn chunks_for(
        &self,
        request: &DocumentTextRequest,
    ) -> Result<PagedDocumentChunks<'_>, SearchError> {
        if !request.paged().is_some_and(|source| {
            source.index() == &self.index && source.fingerprint() == &self.fingerprint
        }) {
            return Err(SearchError::ReadFailed);
        }
        Ok(PagedDocumentChunks {
            document: self,
            next_page: 0,
            offset: 0,
        })
    }
}

pub(crate) struct PagedDocumentChunks<'a> {
    document: &'a PagedDocument,
    next_page: usize,
    offset: usize,
}

impl Iterator for PagedDocumentChunks<'_> {
    type Item = Result<TextChunk, SearchError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_page >= self.document.index.len() {
            return None;
        }
        let text = match self.document.read_page(self.next_page) {
            Ok(text) => text,
            Err(_) => return Some(Err(SearchError::ReadFailed)),
        };
        self.next_page += 1;
        let chunk = TextChunk::new(self.offset, text);
        let Some(next_offset) = self.offset.checked_add(chunk.text().len()) else {
            return Some(Err(SearchError::OffsetExhausted));
        };
        self.offset = next_offset;
        Some(Ok(chunk))
    }
}

fn detect_encoding(
    file: &mut std::fs::File,
    requested: Option<DocumentEncoding>,
) -> io::Result<(DocumentEncoding, bool, usize)> {
    let mut probe = Vec::new();
    file.take(4).read_to_end(&mut probe)?;
    file.seek(std::io::SeekFrom::Start(0))?;
    let (encoding, bom) = match requested {
        Some(DocumentEncoding::Utf8) => {
            (DocumentEncoding::Utf8, probe.starts_with(b"\xef\xbb\xbf"))
        }
        Some(DocumentEncoding::Utf16Le) => {
            (DocumentEncoding::Utf16Le, probe.starts_with(b"\xff\xfe"))
        }
        Some(DocumentEncoding::Utf16Be) => {
            (DocumentEncoding::Utf16Be, probe.starts_with(b"\xfe\xff"))
        }
        Some(encoding) => (encoding, false),
        None if probe.starts_with(b"\xef\xbb\xbf") => (DocumentEncoding::Utf8, true),
        None if probe.starts_with(b"\xff\xfe") => (DocumentEncoding::Utf16Le, true),
        None if probe.starts_with(b"\xfe\xff") => (DocumentEncoding::Utf16Be, true),
        None => (DocumentEncoding::Utf8, false),
    };
    Ok((encoding, bom, if bom { bom_len(encoding) } else { 0 }))
}

const fn bom_len(encoding: DocumentEncoding) -> usize {
    match encoding {
        DocumentEncoding::Utf8 => 3,
        DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => 2,
        DocumentEncoding::Gbk | DocumentEncoding::Gb18030 => 0,
    }
}

fn trailing_carriage_return_len(
    bytes: &[u8],
    encoding: DocumentEncoding,
    body_start: usize,
) -> usize {
    match encoding {
        DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be
            if bytes.len().saturating_sub(body_start) >= 2 =>
        {
            let pair = &bytes[bytes.len() - 2..];
            let unit = if encoding == DocumentEncoding::Utf16Le {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            };
            usize::from(unit == 0x000d) * 2
        }
        DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => 0,
        _ => usize::from(bytes.last() == Some(&b'\r')),
    }
}

fn first_line_ending(
    bytes: &[u8],
    encoding: DocumentEncoding,
    body_start: usize,
) -> Option<LineEnding> {
    match encoding {
        DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => {
            let mut previous = None;
            for pair in bytes[body_start..].chunks_exact(2) {
                let unit = if encoding == DocumentEncoding::Utf16Le {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                };
                if unit == 0x000a {
                    return Some(if previous == Some(0x000d) {
                        LineEnding::Crlf
                    } else {
                        LineEnding::Lf
                    });
                }
                previous = Some(unit);
            }
            None
        }
        _ => {
            let index = bytes.iter().position(|byte| *byte == b'\n')?;
            Some(if index > body_start && bytes[index - 1] == b'\r' {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            })
        }
    }
}

fn last_line_end(bytes: &[u8], encoding: DocumentEncoding, body_start: usize) -> Option<usize> {
    match encoding {
        DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => bytes[body_start..]
            .chunks_exact(2)
            .enumerate()
            .filter_map(|(index, pair)| {
                let unit = if encoding == DocumentEncoding::Utf16Le {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                };
                (unit == 0x000a).then_some(body_start + (index + 1) * 2)
            })
            .next_back(),
        _ => bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|index| index + 1),
    }
}

fn verify_identity(file: &std::fs::File, expected: &ResolvedPath) -> io::Result<()> {
    if expected.file_identity().is_some()
        && Some(crate::atomic_file::file_identity(file)?) != expected.file_identity()
    {
        return Err(changed_source());
    }
    Ok(())
}

fn invalid_encoding() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "分页源不是完整合法的文本编码")
}

fn changed_source() -> io::Error {
    io::Error::other("分页源在读取期间已变化")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use slterm_document::{DocumentEncoding, LineEnding, PAGE_SOURCE_BYTES, TextEncoding};
    use slterm_path::{CanonicalPath, ResolvedPath};

    use super::PagedDocument;

    fn location(path: &std::path::Path) -> ResolvedPath {
        let canonical = CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
        let file = std::fs::File::open(path).unwrap();
        let identity = crate::atomic_file::file_identity(&file).unwrap();
        ResolvedPath::new(canonical, Some(identity))
    }

    fn read_all(document: &PagedDocument) -> String {
        (0..document.index().len())
            .map(|page| document.read_page(page).unwrap())
            .collect()
    }

    #[test]
    fn paged_reads_utf8_utf16_and_gbk_without_splitting_characters() {
        let temp = tempfile::tempdir().unwrap();
        let utf8_text = format!(
            "{}😀\r\nnext\r\n",
            "a".repeat(PAGE_SOURCE_BYTES as usize - 2)
        );
        let utf8 = temp.path().join("utf8.txt");
        std::fs::write(&utf8, &utf8_text).unwrap();
        let document = PagedDocument::open(&location(&utf8), None).unwrap();
        assert_eq!(document.index().len(), 2);
        assert_eq!(document.line_index().line_count(), 3);
        assert_eq!(document.encoding().line_ending, LineEnding::Crlf);
        let first_page_len = document.read_page(0).unwrap().len();
        assert_eq!(
            document.line_index().location(1),
            Some((1, 1, first_page_len))
        );
        assert_eq!(read_all(&document), utf8_text.replace("\r\n", "\n"));

        let utf16_text = format!(
            "{}😀\nnext\n",
            "a".repeat(PAGE_SOURCE_BYTES as usize / 2 - 2)
        );
        let utf16 = temp.path().join("utf16.txt");
        let bytes = TextEncoding {
            encoding: DocumentEncoding::Utf16Le,
            bom: true,
            line_ending: LineEnding::Lf,
            trailing_newline: true,
        }
        .encode(&utf16_text)
        .unwrap();
        std::fs::write(&utf16, bytes).unwrap();
        let document = PagedDocument::open(&location(&utf16), None).unwrap();
        assert_eq!(document.index().len(), 2);
        assert_eq!(document.line_index().line_count(), 3);
        assert_eq!(read_all(&document), utf16_text);

        let gbk = temp.path().join("gbk.txt");
        let mut bytes = vec![b'a'; PAGE_SOURCE_BYTES as usize - 1];
        bytes.extend_from_slice(
            &TextEncoding {
                encoding: DocumentEncoding::Gbk,
                bom: false,
                line_ending: LineEnding::Lf,
                trailing_newline: true,
            }
            .encode("中\nnext\n")
            .unwrap(),
        );
        std::fs::write(&gbk, bytes).unwrap();
        let document = PagedDocument::open(&location(&gbk), Some(DocumentEncoding::Gbk)).unwrap();
        assert_eq!(document.index().len(), 2);
        assert_eq!(
            read_all(&document),
            format!("{}中\nnext\n", "a".repeat(PAGE_SOURCE_BYTES as usize - 1))
        );
    }

    #[test]
    fn paged_search_and_copy_cross_source_pages() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("search.txt");
        let text = format!("{}END\nstart\n", "x".repeat(PAGE_SOURCE_BYTES as usize - 3));
        std::fs::write(&path, &text).unwrap();
        let location = location(&path);
        let document = PagedDocument::open(&location, None).unwrap();
        let id =
            slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut registry = slterm_document::DocumentRegistry::new();
        registry
            .open(
                id,
                location,
                document.source(),
                slterm_document::TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a")
                    .unwrap(),
                std::sync::Arc::default(),
                slterm_document::DocumentMetadata {
                    read_only: true,
                    ..slterm_document::DocumentMetadata::default()
                },
            )
            .unwrap();
        let request = registry.text_request(id, 9).unwrap();
        let (_cancel, cancellation) = std::sync::mpsc::sync_channel(1);

        let result = request
            .search(
                "END\nstart",
                document.chunks_for(&request).unwrap(),
                &cancellation,
            )
            .unwrap();
        let expected = PAGE_SOURCE_BYTES as usize - 3;
        assert_eq!(result.matches().len(), 1);
        assert_eq!(result.matches()[0].range(), expected..expected + 9);
        let copied = request
            .copy(document.chunks_for(&request).unwrap(), &cancellation)
            .unwrap();
        assert_eq!(copied, text);
    }

    #[test]
    fn paged_crlf_is_never_split_between_source_pages() {
        let temp = tempfile::tempdir().unwrap();
        let text = format!("{}\r\nnext", "a".repeat(PAGE_SOURCE_BYTES as usize - 1));
        let path = temp.path().join("split-crlf.txt");
        std::fs::write(&path, &text).unwrap();
        let document = PagedDocument::open(&location(&path), None).unwrap();

        assert_eq!(document.index().len(), 2);
        assert_eq!(document.encoding().line_ending, LineEnding::Crlf);
        assert_eq!(read_all(&document), text.replace("\r\n", "\n"));
    }

    #[test]
    fn paged_rejects_invalid_input_and_external_growth() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.txt");
        std::fs::write(&path, [0xff]).unwrap();
        assert!(PagedDocument::open(&location(&path), Some(DocumentEncoding::Utf8)).is_err());

        std::fs::write(&path, b"line\n").unwrap();
        let document = PagedDocument::open(&location(&path), None).unwrap();
        std::fs::write(&path, b"grown\n").unwrap();
        assert!(document.read_page(0).is_err());
        assert_eq!(document.index().total_bytes(), 5);
    }
}
