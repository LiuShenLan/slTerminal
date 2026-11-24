//! Runtime 有界读取：统一页面预算、UTF-8 安全切分与代次绑定游标。
//!
//! 终端、Git diff 与 Agent 读取共用同一分页语义；截断必须显式返回
//! 续读游标，不得把半个字符或半行内容伪装成完整结果。

/// 单页响应预算与协议响应上限一致。
pub(crate) const PAGE_BUDGET_BYTES: usize = 1024 * 1024;

/// 资源代次与输出 revision；任一变化都使旧游标失效。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReadGeneration {
    pub generation: u64,
    pub revision: u64,
}

/// 游标只由服务端生成与解释，客户端不得构造或跨代次复用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReadCursor {
    generation: u64,
    revision: u64,
    offset: usize,
}

impl ReadCursor {
    fn encode(self) -> String {
        format!("v1:{}:{}:{}", self.generation, self.revision, self.offset)
    }

    fn parse(text: &str) -> Result<Self, ReadError> {
        let mut parts = text.split(':');
        let (Some("v1"), Some(generation), Some(revision), Some(offset), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(ReadError::StaleCursor);
        };
        Ok(Self {
            generation: generation.parse().map_err(|_| ReadError::StaleCursor)?,
            revision: revision.parse().map_err(|_| ReadError::StaleCursor)?,
            offset: offset.parse().map_err(|_| ReadError::StaleCursor)?,
        })
    }
}

/// 单页读取结果；`next_cursor` 存在即表示尾部仍有内容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Page {
    pub bytes: Vec<u8>,
    pub next_cursor: Option<String>,
}

/// 游标不可用：非法、越界或资源代次/revision 已变化。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    StaleCursor,
}

/// 从字节快照读取一页；切分点保持在完整 UTF-8 字符边界，并优先收束在行尾。
pub(crate) fn read_page(
    bytes: &[u8],
    current: ReadGeneration,
    cursor: Option<&str>,
    budget: usize,
) -> Result<Page, ReadError> {
    let offset = match cursor {
        None => 0,
        Some(text) => {
            let cursor = ReadCursor::parse(text)?;
            if cursor.generation != current.generation
                || cursor.revision != current.revision
                || cursor.offset > bytes.len()
            {
                return Err(ReadError::StaleCursor);
            }
            cursor.offset
        }
    };
    let budget = budget.max(1);
    let remaining = bytes.len() - offset;
    if remaining <= budget {
        return Ok(Page {
            bytes: bytes[offset..].to_vec(),
            next_cursor: None,
        });
    }

    let limit = offset + budget;
    let mut end = limit;
    // 预算落在多字节字符中间时回退到字符起始。
    while end > offset && (bytes[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    if end == offset {
        // 单字符长于预算：前进到该字符末尾，保证页面持续进展。
        end = limit;
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end += 1;
        }
    }
    // 行尾优先：只在本页范围内回退，不跨过 offset。
    if let Some(position) = bytes[offset..end].iter().rposition(|byte| *byte == b'\n') {
        end = offset + position + 1;
    }
    Ok(Page {
        bytes: bytes[offset..end].to_vec(),
        next_cursor: Some(
            ReadCursor {
                generation: current.generation,
                revision: current.revision,
                offset: end,
            }
            .encode(),
        ),
    })
}
