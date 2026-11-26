//! Runtime 有界读取：统一页面预算、UTF-8 安全切分与代次绑定游标。
//!
//! 终端、Git diff 与 Agent 读取共用同一分页语义；截断必须显式返回
//! 续读游标，不得把半个字符或半行内容伪装成完整结果。

use slterm_workspace::TerminalId;

/// 单页正文预算：低于 1 MiB 响应上限，为 JSON 封装与转义留出裕度。
///
/// `serde_json` 对控制字符输出 `\u00XX`，最坏每字节膨胀六倍；160 KiB 正文
/// 编码后仍低于 1 MiB，减去响应信封后不会因极限页面断开连接。
pub(crate) const PAGE_BUDGET_BYTES: usize = 160 * 1024;

/// 终端读取目标：稳定身份、PTY 代次与可选 Agent 身份；都由前台复核。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalReadTarget {
    pub(crate) terminal_id: TerminalId,
    pub(crate) generation: u64,
    /// `Some` 表示 agent.read，必须再匹配该终端当前对外身份。
    pub(crate) agent_id: Option<String>,
}

/// service 从 worker 文本快照包装出的读取结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalTextSnapshot {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) text: String,
}

/// 资源代次与输出 revision；任一变化都使旧游标失效。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReadGeneration {
    /// 稳定资源身份散列；不同终端／路径不得互换游标。
    pub resource: u64,
    pub generation: u64,
    pub revision: u64,
}

/// 稳定资源身份散列；只用于游标绑定，不承载路径或正文。
pub(crate) fn resource_token(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 游标只由服务端生成与解释，客户端不得构造或跨代次复用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReadCursor {
    resource: u64,
    generation: u64,
    revision: u64,
    offset: usize,
}

impl ReadCursor {
    fn encode(self) -> String {
        format!(
            "v2:{}:{}:{}:{}",
            self.resource, self.generation, self.revision, self.offset
        )
    }

    fn parse(text: &str) -> Result<Self, ReadError> {
        let mut parts = text.split(':');
        let (Some("v2"), Some(resource), Some(generation), Some(revision), Some(offset), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(ReadError::StaleCursor);
        };
        Ok(Self {
            resource: resource.parse().map_err(|_| ReadError::StaleCursor)?,
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
            if cursor.resource != current.resource
                || cursor.generation != current.generation
                || cursor.revision != current.revision
                || cursor.offset > bytes.len()
                || (cursor.offset < bytes.len() && (bytes[cursor.offset] & 0xC0) == 0x80)
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
                resource: current.resource,
                generation: current.generation,
                revision: current.revision,
                offset: end,
            }
            .encode(),
        ),
    })
}
