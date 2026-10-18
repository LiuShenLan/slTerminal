//! 渲染无关的文本快照(07 篇):编码/行尾/快照比对的单点。
//!
//! 本地文件与 gitshow(HEAD 字节)共用同一快照通路;`read_only` 是**类型层**
//! 合同——encode 入口直接拒绝,不靠 UI 层自觉(07 篇边界条 6)。M8.1 只点亮
//! 只读臂;M8.2 的可编辑 Document 复用同一结构,保存链按 `verify` 拒绝覆盖。

use std::io;
use std::sync::Arc;

/// 可编辑域上限(契约值,07 篇 §目标形态:超限 = 截断 + 只读)。
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct TextSnapshot {
    pub text: String,
    /// 载入原文逐字节副本——保存前比对的权威。
    #[allow(dead_code)] // 消费点 = M8.2 保存链 verify(本步只读,无写回)。
    pub bytes: Arc<[u8]>,
    /// UTF-8 BOM 保留往返。
    #[allow(dead_code)] // 消费点 = M8.2 保存链 encode。
    pub bom: bool,
    /// CRLF 行尾保留往返。
    #[allow(dead_code)] // 消费点 = M8.2 保存链 encode。
    pub crlf: bool,
    /// 超限截断 → 强制只读。
    pub truncated: bool,
    /// 非法 UTF-8 / 含 NUL → 强制只读。
    pub invalid_encoding: bool,
    pub read_only: bool,
}

#[allow(dead_code)] // 消费点 = M8.2 保存链三态(本步只读,无写回)。
#[derive(Debug)]
pub(crate) enum SaveError {
    Changed,
    ReadOnly,
    Io(io::Error),
}

impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Changed => formatter.write_str("文件在打开后已被外部修改"),
            Self::ReadOnly => formatter.write_str("文档处于只读状态"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SaveError {}

impl TextSnapshot {
    /// 有界读取器已先移走溢出字节时,仍保持只读合同(不把超限内容
    /// 交给 decode 造成超大分配后再截断)。
    pub(crate) fn decode_prefix(bytes: Vec<u8>, read_only: bool, truncated: bool) -> Self {
        let mut snapshot = Self::decode(bytes, read_only);
        snapshot.truncated |= truncated;
        snapshot.read_only |= truncated;
        snapshot
    }

    pub(crate) fn decode(mut bytes: Vec<u8>, read_only: bool) -> Self {
        let truncated = bytes.len() > MAX_BYTES;
        bytes.truncate(MAX_BYTES);
        let bom = bytes.starts_with(b"\xef\xbb\xbf");
        let body = if bom { &bytes[3..] } else { &bytes };
        let invalid_encoding = std::str::from_utf8(body).is_err() || body.contains(&0);
        let text = String::from_utf8_lossy(body);
        // CRLF 判定 = 全文只含 CRLF 一种换行;混排按 LF 处理,避免往返改写。
        let crlf = text.contains("\r\n") && !text.replace("\r\n", "").contains('\n');
        let text = text.replace("\r\n", "\n");
        Self {
            text,
            bytes: bytes.into(),
            bom,
            crlf,
            truncated,
            invalid_encoding,
            read_only: read_only || truncated || invalid_encoding,
        }
    }

    #[allow(dead_code)] // 消费点 = M8.2 保存链(本步只读;只读拒绝合同已由用例锁定)。
    pub(crate) fn encode(&self, text: &str) -> Result<Vec<u8>, SaveError> {
        if self.read_only {
            return Err(SaveError::ReadOnly);
        }
        let mut bytes = if self.bom {
            b"\xef\xbb\xbf".to_vec()
        } else {
            Vec::new()
        };
        let encoded = if self.crlf {
            text.replace('\n', "\r\n")
        } else {
            text.to_owned()
        };
        bytes.extend_from_slice(encoded.as_bytes());
        if bytes.len() > MAX_BYTES {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "可编辑文本上限为 8 MiB").into(),
            );
        }
        Ok(bytes)
    }

    #[allow(dead_code)] // 消费点 = M8.2 保存链(保存前磁盘快照比对)。
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
