//! 渲染无关的文本快照：core 编码路由、磁盘基线与条件保存适配。

use std::io;
use std::sync::Arc;

use slterm_document::{
    DecodedText, DocumentContent, EDITABLE_LIMIT_BYTES, EncodeError, TextEncoding,
};

/// 可编辑域与读取上限使用同一契约；Paged 另行只读有限前缀。
pub(crate) const MAX_BYTES: usize = EDITABLE_LIMIT_BYTES as usize;

#[derive(Clone, Debug)]
pub(crate) struct TextSnapshot {
    pub text: String,
    /// 载入原文逐字节副本——保存前比对的权威。
    pub bytes: Arc<[u8]>,
    pub encoding: TextEncoding,
    pub completion_enabled: bool,
    pub syntax_enabled: bool,
    /// 有界前缀而非完整正文时强制只读。
    pub truncated: bool,
    /// 仅供 GitShow／Diff 的不可用预览；真实 Document 打开会直接拒绝。
    pub invalid_encoding: bool,
    pub read_only: bool,
}

#[derive(Debug)]
pub(crate) enum SaveError {
    Changed,
    ReadOnly,
    Unauthorized,
    TargetOpen,
    Unrepresentable,
    Io(io::Error),
}

impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<EncodeError> for SaveError {
    fn from(_: EncodeError) -> Self {
        Self::Unrepresentable
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Changed => formatter.write_str("文件在打开后已被外部修改"),
            Self::ReadOnly => formatter.write_str("文档处于只读状态"),
            Self::Unauthorized => formatter.write_str("文档写入未获授权或授权代次已变化"),
            Self::TargetOpen => formatter.write_str("另存为目标已关联其他打开的 Document"),
            Self::Unrepresentable => formatter.write_str("当前编码无法表示文本"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SaveError {}

impl TextSnapshot {
    pub(crate) fn decode_prefix(bytes: Vec<u8>, read_only: bool, truncated: bool) -> Self {
        let total = bytes.len() as u64;
        Self::from_content(
            DocumentContent::decode(bytes, total, None),
            read_only,
            truncated,
        )
    }

    pub(crate) fn decode(bytes: Vec<u8>, read_only: bool) -> Self {
        Self::decode_prefix(bytes, read_only, false)
    }

    pub(crate) fn from_content(content: DocumentContent, read_only: bool, truncated: bool) -> Self {
        match content {
            DocumentContent::Editable(decoded) => {
                Self::from_decoded(decoded, read_only || truncated, truncated)
            }
            DocumentContent::Paged(decoded) => Self::from_decoded(decoded, true, truncated),
            DocumentContent::Unavailable(_) => Self {
                text: String::new(),
                bytes: Arc::from(Vec::new()),
                encoding: TextEncoding::default(),
                completion_enabled: false,
                syntax_enabled: false,
                truncated,
                invalid_encoding: true,
                read_only: true,
            },
        }
    }

    fn from_decoded(decoded: DecodedText, read_only: bool, truncated: bool) -> Self {
        Self {
            text: decoded.text,
            bytes: decoded.disk_baseline,
            encoding: decoded.encoding,
            completion_enabled: decoded.completion_enabled,
            syntax_enabled: decoded.syntax_enabled,
            truncated,
            invalid_encoding: false,
            read_only,
        }
    }

    pub(crate) fn encode(&self, text: &str) -> Result<Vec<u8>, SaveError> {
        if self.read_only {
            return Err(SaveError::ReadOnly);
        }
        let bytes = self.encoding.encode(text)?;
        if bytes.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "可编辑文本上限为 50000000 字节",
            )
            .into());
        }
        Ok(bytes)
    }

    pub(crate) fn verify(&self, current: &[u8]) -> Result<(), SaveError> {
        if current == self.bytes.as_ref() {
            Ok(())
        } else {
            Err(SaveError::Changed)
        }
    }
}

#[cfg(test)]
mod text_document_tests {
    use super::*;

    /// 有界前缀已是只读:encode 必须拒绝,truncated 标志不得丢失。
    #[test]
    fn bounded_prefix_stays_read_only_and_rejects_encoding() {
        let snapshot = TextSnapshot::decode_prefix(b"valid prefix".to_vec(), false, true);
        assert!(snapshot.truncated && snapshot.read_only);
        assert!(matches!(
            snapshot.encode("replacement"),
            Err(SaveError::ReadOnly)
        ));
    }

    /// BOM/CRLF/Unicode 往返保留(旧栈编码语义资产)。
    #[test]
    fn bom_crlf_and_unicode_round_trip() {
        let snapshot = TextSnapshot::decode("\u{feff}标题\r\n内容\r\n".as_bytes().to_vec(), false);
        assert_eq!(snapshot.text, "标题\n内容\n");
        assert_eq!(
            snapshot.encode("新标题\n").unwrap(),
            "\u{feff}新标题\r\n".as_bytes()
        );
        assert_eq!(snapshot.encode("").unwrap(), b"\xef\xbb\xbf");
    }

    #[test]
    fn unrepresentable_text_is_a_save_failure_without_replacement() {
        let mut snapshot = TextSnapshot::decode("中文".as_bytes().to_vec(), false);
        snapshot.encoding.encoding = slterm_document::DocumentEncoding::Gbk;

        assert!(matches!(
            snapshot.encode("😀"),
            Err(SaveError::Unrepresentable)
        ));
        assert_eq!(snapshot.bytes.as_ref(), "中文".as_bytes());
    }

    /// 等长外部修改同样是冲突(verify 逐字节比对)。
    #[test]
    fn same_length_external_edits_are_conflicts() {
        let snapshot = TextSnapshot::decode(b"first".to_vec(), false);
        assert!(snapshot.verify(b"first").is_ok());
        assert!(matches!(snapshot.verify(b"other"), Err(SaveError::Changed)));
    }

    /// 二进制/非法编码/超限快照永不可写。
    #[test]
    fn binary_and_oversized_snapshots_never_become_writable() {
        for bytes in [vec![b'a', 0, b'b'], vec![0xff], vec![b'x'; MAX_BYTES + 1]] {
            let snapshot = TextSnapshot::decode(bytes, false);
            assert!(matches!(
                snapshot.encode("replacement"),
                Err(SaveError::ReadOnly)
            ));
        }
    }

    /// 显式只读语义(图片之外的 gitshow/html 只读臂)在 encode 层生效。
    #[test]
    fn explicit_read_only_snapshots_reject_encoding() {
        let snapshot = TextSnapshot::decode(b"read only".to_vec(), true);
        assert!(snapshot.read_only && !snapshot.truncated);
        assert!(matches!(
            snapshot.encode("read only"),
            Err(SaveError::ReadOnly)
        ));
    }
}
