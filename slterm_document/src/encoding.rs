//! Document 编码、行尾、磁盘字节基线与内容尺寸路由。

use std::fmt;
use std::sync::Arc;

pub const NORMAL_EDIT_LIMIT_BYTES: u64 = 10_000_000;
pub const EDITABLE_LIMIT_BYTES: u64 = 50_000_000;
pub const SYNTAX_LIMIT_BYTES: u64 = 200_000_000;
pub const BINARY_PROBE_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Gbk,
    Gb18030,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEncoding {
    pub encoding: DocumentEncoding,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub trailing_newline: bool,
}

impl Default for TextEncoding {
    fn default() -> Self {
        Self {
            encoding: DocumentEncoding::Utf8,
            bom: false,
            line_ending: LineEnding::Crlf,
            trailing_newline: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentUnavailable {
    Binary,
    UnknownEncoding,
    InvalidEncoding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    Unrepresentable,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unrepresentable => "当前编码无法表示文本",
        })
    }
}

impl std::error::Error for EncodeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: TextEncoding,
    pub disk_baseline: Arc<[u8]>,
    pub completion_enabled: bool,
    pub syntax_enabled: bool,
}

impl DecodedText {
    pub fn encode(&self, text: &str) -> Result<Vec<u8>, EncodeError> {
        self.encoding.encode(text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentContent {
    Editable(DecodedText),
    Paged(DecodedText),
    Unavailable(ContentUnavailable),
}

impl DocumentContent {
    /// 自动识别只接受 BOM 或合法 UTF-8；其他编码必须由用户显式指定。
    pub fn decode(bytes: Vec<u8>, total_bytes: u64, requested: Option<DocumentEncoding>) -> Self {
        let (encoding, bom, body) = match requested {
            Some(DocumentEncoding::Utf8) => {
                let bom = bytes.starts_with(b"\xef\xbb\xbf");
                (
                    DocumentEncoding::Utf8,
                    bom,
                    &bytes[if bom { 3 } else { 0 }..],
                )
            }
            Some(DocumentEncoding::Utf16Le) => {
                let bom = bytes.starts_with(b"\xff\xfe");
                (
                    DocumentEncoding::Utf16Le,
                    bom,
                    &bytes[if bom { 2 } else { 0 }..],
                )
            }
            Some(DocumentEncoding::Utf16Be) => {
                let bom = bytes.starts_with(b"\xfe\xff");
                (
                    DocumentEncoding::Utf16Be,
                    bom,
                    &bytes[if bom { 2 } else { 0 }..],
                )
            }
            Some(encoding) => (encoding, false, bytes.as_slice()),
            None if bytes.starts_with(b"\xef\xbb\xbf") => {
                (DocumentEncoding::Utf8, true, &bytes[3..])
            }
            None if bytes.starts_with(b"\xff\xfe") => {
                (DocumentEncoding::Utf16Le, true, &bytes[2..])
            }
            None if bytes.starts_with(b"\xfe\xff") => {
                (DocumentEncoding::Utf16Be, true, &bytes[2..])
            }
            None => (DocumentEncoding::Utf8, false, bytes.as_slice()),
        };
        let binary = match encoding {
            DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => body
                .chunks_exact(2)
                .take(BINARY_PROBE_BYTES / 2)
                .any(|unit| unit == [0, 0]),
            _ => body[..body.len().min(BINARY_PROBE_BYTES)].contains(&0),
        };
        if binary {
            return Self::Unavailable(ContentUnavailable::Binary);
        }
        let decoded = encoding.decode(body).map_err(|cause| {
            if encoding == DocumentEncoding::Utf8 && requested.is_none() {
                ContentUnavailable::UnknownEncoding
            } else {
                cause
            }
        });
        let decoded = match decoded {
            Ok(decoded) => decoded,
            Err(cause) => return Self::Unavailable(cause),
        };
        let line_ending = decoded.find('\n').map_or(LineEnding::Crlf, |index| {
            if index > 0 && decoded.as_bytes()[index - 1] == b'\r' {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            }
        });
        let text = decoded.replace("\r\n", "\n");
        let trailing_newline = text.ends_with('\n');
        let decoded = DecodedText {
            text,
            encoding: TextEncoding {
                encoding,
                bom,
                line_ending,
                trailing_newline,
            },
            disk_baseline: Arc::from(bytes),
            completion_enabled: total_bytes <= NORMAL_EDIT_LIMIT_BYTES,
            syntax_enabled: total_bytes <= SYNTAX_LIMIT_BYTES,
        };
        if total_bytes > EDITABLE_LIMIT_BYTES {
            Self::Paged(decoded)
        } else {
            Self::Editable(decoded)
        }
    }
}

impl DocumentEncoding {
    fn encoding_rs(self) -> &'static encoding_rs::Encoding {
        match self {
            Self::Utf8 => encoding_rs::UTF_8,
            Self::Utf16Le => encoding_rs::UTF_16LE,
            Self::Utf16Be => encoding_rs::UTF_16BE,
            Self::Gbk => encoding_rs::GBK,
            Self::Gb18030 => encoding_rs::GB18030,
        }
    }

    /// 严格解码完整字符前缀，并返回可安全切分的源字节位置。
    pub fn decode_prefix(self, bytes: &[u8]) -> Result<(String, usize), ContentUnavailable> {
        let mut decoder = self.encoding_rs().new_decoder_without_bom_handling();
        let mut output = String::new();
        let mut read = 0;
        loop {
            output.reserve((bytes.len() - read).saturating_mul(3).max(16));
            let (result, consumed) =
                decoder.decode_to_string_without_replacement(&bytes[read..], &mut output, false);
            read += consumed;
            match result {
                encoding_rs::DecoderResult::InputEmpty => break,
                encoding_rs::DecoderResult::OutputFull => {}
                encoding_rs::DecoderResult::Malformed(_, _) => {
                    return Err(ContentUnavailable::InvalidEncoding);
                }
            }
        }
        if let Ok(text) = self.decode(bytes) {
            return Ok((text, bytes.len()));
        }
        // InputEmpty 的 read 包含解码器暂存的不完整尾部，不能用作页边界。
        let max_character_bytes = match self {
            Self::Gbk => 2,
            Self::Utf8 | Self::Utf16Le | Self::Utf16Be | Self::Gb18030 => 4,
        };
        for trailing in 1..max_character_bytes.min(bytes.len() + 1) {
            let end = bytes.len() - trailing;
            if let Ok(text) = self.decode(&bytes[..end]) {
                return Ok((text, end));
            }
        }
        Err(ContentUnavailable::InvalidEncoding)
    }

    /// 严格解码完整正文；不完整尾字符与非法序列均拒绝。
    pub fn decode(self, bytes: &[u8]) -> Result<String, ContentUnavailable> {
        self.encoding_rs()
            .decode_without_bom_handling_and_without_replacement(bytes)
            .map(|text| text.into_owned())
            .ok_or(ContentUnavailable::InvalidEncoding)
    }
}

impl TextEncoding {
    pub fn encode(&self, text: &str) -> Result<Vec<u8>, EncodeError> {
        let text = match self.line_ending {
            LineEnding::Lf => text.to_owned(),
            LineEnding::Crlf => text.replace('\n', "\r\n"),
        };
        let mut bytes = Vec::new();
        match self.encoding {
            DocumentEncoding::Utf8 => {
                if self.bom {
                    bytes.extend_from_slice(b"\xef\xbb\xbf");
                }
                bytes.extend_from_slice(text.as_bytes());
            }
            DocumentEncoding::Utf16Le | DocumentEncoding::Utf16Be => {
                if self.bom {
                    bytes.extend_from_slice(if self.encoding == DocumentEncoding::Utf16Le {
                        b"\xff\xfe"
                    } else {
                        b"\xfe\xff"
                    });
                }
                for unit in text.encode_utf16() {
                    let encoded = if self.encoding == DocumentEncoding::Utf16Le {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    };
                    bytes.extend_from_slice(&encoded);
                }
            }
            DocumentEncoding::Gbk | DocumentEncoding::Gb18030 => {
                let encoding = if self.encoding == DocumentEncoding::Gbk {
                    encoding_rs::GBK
                } else {
                    encoding_rs::GB18030
                };
                let (encoded, _, had_errors) = encoding.encode(&text);
                if had_errors {
                    return Err(EncodeError::Unrepresentable);
                }
                bytes.extend_from_slice(&encoded);
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ContentUnavailable, DocumentContent, DocumentEncoding, EncodeError, LineEnding,
        TextEncoding,
    };

    fn editable(content: DocumentContent) -> crate::DecodedText {
        match content {
            DocumentContent::Editable(text) => text,
            _ => panic!("应路由为可编辑文本"),
        }
    }

    #[test]
    fn utf8_bom_first_crlf_and_trailing_newline_round_trip() {
        let decoded = editable(DocumentContent::decode(
            "\u{feff}标题\r\n正文\r\n".as_bytes().to_vec(),
            19,
            None,
        ));
        assert_eq!(decoded.text, "标题\n正文\n");
        assert_eq!(decoded.encoding.encoding, DocumentEncoding::Utf8);
        assert!(decoded.encoding.bom);
        assert_eq!(decoded.encoding.line_ending, LineEnding::Crlf);
        assert!(decoded.encoding.trailing_newline);
        assert_eq!(
            decoded.encode(&decoded.text).unwrap(),
            "\u{feff}标题\r\n正文\r\n".as_bytes()
        );
    }

    #[test]
    fn utf16_boms_round_trip_unicode_without_loss() {
        for (encoding, expected) in [
            (DocumentEncoding::Utf16Le, b"\xff\xfe".as_slice()),
            (DocumentEncoding::Utf16Be, b"\xfe\xff".as_slice()),
        ] {
            let mut bytes = expected.to_vec();
            let units = "标题😀\n".encode_utf16().collect::<Vec<_>>();
            for unit in units {
                if encoding == DocumentEncoding::Utf16Le {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                } else {
                    bytes.extend_from_slice(&unit.to_be_bytes());
                }
            }
            let decoded = editable(DocumentContent::decode(
                bytes.clone(),
                bytes.len() as u64,
                None,
            ));
            assert_eq!(decoded.text, "标题😀\n");
            assert_eq!(decoded.encoding.encoding, encoding);
            assert!(decoded.encoding.bom);
            assert_eq!(decoded.encode(&decoded.text).unwrap(), bytes);
        }
    }

    #[test]
    fn gbk_and_gb18030_round_trip_and_reject_unrepresentable_text() {
        let gbk = editable(DocumentContent::decode(
            vec![0xd6, 0xd0, 0xce, 0xc4],
            4,
            Some(DocumentEncoding::Gbk),
        ));
        assert_eq!(gbk.text, "中文");
        assert_eq!(gbk.encode(&gbk.text).unwrap(), [0xd6, 0xd0, 0xce, 0xc4]);
        let error = TextEncoding {
            encoding: DocumentEncoding::Gbk,
            bom: false,
            line_ending: LineEnding::Lf,
            trailing_newline: false,
        }
        .encode("😀")
        .unwrap_err();
        assert_eq!(error, EncodeError::Unrepresentable);

        let bytes = encoding_rs::GB18030.encode("中文😀").0.into_owned();
        let gb18030 = editable(DocumentContent::decode(
            bytes.clone(),
            bytes.len() as u64,
            Some(DocumentEncoding::Gb18030),
        ));
        assert_eq!(gb18030.text, "中文😀");
        assert_eq!(gb18030.encode(&gb18030.text).unwrap(), bytes);
    }

    #[test]
    fn nul_unknown_and_invalid_encoding_have_distinct_unavailable_routes() {
        assert!(matches!(
            DocumentContent::decode(b"text\0binary".to_vec(), 11, None),
            DocumentContent::Unavailable(ContentUnavailable::Binary)
        ));
        assert!(matches!(
            DocumentContent::decode(vec![0xff], 1, None),
            DocumentContent::Unavailable(ContentUnavailable::UnknownEncoding)
        ));
        assert!(matches!(
            DocumentContent::decode(vec![0xff, 0xfe, 0x00], 3, Some(DocumentEncoding::Utf16Le),),
            DocumentContent::Unavailable(ContentUnavailable::InvalidEncoding)
        ));
    }

    #[test]
    fn decode_prefix_reports_exact_incomplete_character_boundary() {
        let utf8 = "a😀".as_bytes();
        let (text, read) = DocumentEncoding::Utf8.decode_prefix(&utf8[..3]).unwrap();
        assert_eq!(text, "a");
        assert_eq!(read, 1);
        assert!(DocumentEncoding::Utf8.decode(&utf8[..3]).is_err());

        let mut utf16 = 0x0061u16.to_le_bytes().to_vec();
        utf16.extend_from_slice(&0xd83du16.to_le_bytes());
        let (text, read) = DocumentEncoding::Utf16Le.decode_prefix(&utf16).unwrap();
        assert_eq!(text, "a");
        assert_eq!(read, 2);

        let (text, read) = DocumentEncoding::Gbk.decode_prefix(&[0xd6]).unwrap();
        assert_eq!(text, "");
        assert_eq!(read, 0);
        let gb18030 = encoding_rs::GB18030.encode("😀").0;
        let (text, read) = DocumentEncoding::Gb18030
            .decode_prefix(&gb18030[..3])
            .unwrap();
        assert_eq!(text, "");
        assert_eq!(read, 0);
    }

    #[test]
    fn byte_thresholds_control_editing_completion_paging_and_syntax() {
        for (size, paged, completion, syntax) in [
            (10_000_000, false, true, true),
            (10_000_001, false, false, true),
            (50_000_000, false, false, true),
            (50_000_001, true, false, true),
            (200_000_001, true, false, false),
        ] {
            let content = DocumentContent::decode(b"text".to_vec(), size, None);
            match content {
                DocumentContent::Editable(text) if !paged => {
                    assert_eq!(text.completion_enabled, completion);
                    assert_eq!(text.syntax_enabled, syntax);
                }
                DocumentContent::Paged(text) if paged => {
                    assert!(!text.completion_enabled);
                    assert_eq!(text.syntax_enabled, syntax);
                }
                _ => panic!("尺寸 {size} 的内容路由错误"),
            }
        }
    }

    #[test]
    fn mixed_newlines_normalize_and_use_the_first_disk_newline_on_save() {
        let decoded = editable(DocumentContent::decode(b"a\r\nb\nc\r\n".to_vec(), 8, None));
        assert_eq!(decoded.text, "a\nb\nc\n");
        assert_eq!(decoded.encoding.line_ending, LineEnding::Crlf);
        assert!(decoded.encoding.trailing_newline);
        assert_eq!(decoded.encode(&decoded.text).unwrap(), b"a\r\nb\r\nc\r\n");
    }

    #[test]
    fn new_files_default_to_utf8_crlf_without_bom() {
        let encoding = TextEncoding::default();
        assert_eq!(encoding.encoding, DocumentEncoding::Utf8);
        assert_eq!(encoding.line_ending, LineEnding::Crlf);
        assert!(!encoding.bom);
        assert!(!encoding.trailing_newline);
    }
}
