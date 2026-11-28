//! Runtime 有界读取：统一页面预算、UTF-8 安全切分与代次绑定游标。
//!
//! 终端、Git diff 与 Agent 读取共用同一分页语义；截断必须显式返回
//! 续读游标，不得把半个字符或半行内容伪装成完整结果。

use std::collections::VecDeque;
use std::sync::Arc;

use slterm_workspace::{ProjectId, TerminalId};

/// Runtime 读取缓存：分页续页直接复用不可变全文，不重新物化。
pub(crate) const READ_CACHE_ENTRIES: usize = 16;
/// 读取缓存全文总字节上限；超限按 LRU 淘汰。
pub(crate) const READ_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// 一页链共享的不可变快照；`resource` 与游标绑定，禁止跨资源命中。
#[derive(Clone, Debug)]
pub(crate) struct CachedReadText {
    pub(crate) resource: u64,
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    /// Git 源正文指纹；续页前重算以证明快照仍然有效。终端读取为 0。
    pub(crate) source_fingerprint: u64,
    /// Git 分页解析后的项目身份；终端读取为 None。
    pub(crate) project_id: Option<ProjectId>,
    pub(crate) text: Arc<str>,
}

impl CachedReadText {
    fn len(&self) -> usize {
        self.text.len()
    }
}

/// 有界 LRU：条目数与全文总字节双上限；命中即刷新最近使用顺序。
pub(crate) struct ReadSnapshotCache {
    entries: VecDeque<(String, Arc<CachedReadText>)>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for ReadSnapshotCache {
    fn default() -> Self {
        Self::new(READ_CACHE_ENTRIES, READ_CACHE_BYTES)
    }
}

impl ReadSnapshotCache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            max_entries,
            max_bytes,
        }
    }

    pub(crate) fn get(&mut self, key: &str) -> Option<Arc<CachedReadText>> {
        let position = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        let entry = self.entries.remove(position)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    pub(crate) fn insert(&mut self, key: String, value: Arc<CachedReadText>) {
        if let Some(position) = self
            .entries
            .iter()
            .position(|(candidate, _)| *candidate == key)
        {
            let (_, previous) = self.entries.remove(position).expect("命中的缓存项必须存在");
            self.bytes = self.bytes.saturating_sub(previous.len());
        }
        self.bytes = self.bytes.saturating_add(value.len());
        self.entries.push_back((key, value));
        while self.entries.len() > self.max_entries
            || (self.bytes > self.max_bytes && self.entries.len() > 1)
        {
            if let Some((_, evicted)) = self.entries.pop_front() {
                self.bytes = self.bytes.saturating_sub(evicted.len());
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn bytes(&self) -> usize {
        self.bytes
    }
}

/// 游标中的快照身份；服务端在读取前解析，用于跳过续页重物化。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CursorSnapshot {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
}

/// 游标解析结果中的快照身份与资源身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CursorPart {
    pub(crate) resource: u64,
    pub(crate) snapshot: CursorSnapshot,
}

/// 解析游标；格式非法返回 None，后续由读取层映射 `StaleCursor`。
pub(crate) fn cursor_part(cursor: &str) -> Option<CursorPart> {
    let cursor = ReadCursor::parse(cursor).ok()?;
    Some(CursorPart {
        resource: cursor.resource,
        snapshot: CursorSnapshot {
            generation: cursor.generation,
            revision: cursor.revision,
        },
    })
}

/// 终端读取缓存的稳定键；与游标资源不同，允许在本进程内精确命中。
pub(crate) fn terminal_cache_key(
    terminal_id: TerminalId,
    agent_id: Option<&str>,
    snapshot: CursorSnapshot,
) -> String {
    format!(
        "terminal:{terminal_id}:{}:{}:{}",
        agent_id.unwrap_or(""),
        snapshot.generation,
        snapshot.revision
    )
}

/// Git 读取缓存的稳定键；项目使用请求值（None 表示活跃项目）。
pub(crate) fn git_cache_key(
    project_id: Option<ProjectId>,
    path: &str,
    snapshot: CursorSnapshot,
) -> String {
    format!(
        "git:{}:{path}:{}:{}",
        project_id.map_or_else(|| "-".to_owned(), |id| id.to_string()),
        snapshot.generation,
        snapshot.revision
    )
}

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
    pub(crate) text: Arc<str>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(revision: u64, text: &'static str) -> Arc<CachedReadText> {
        Arc::new(CachedReadText {
            resource: 7,
            generation: 1,
            revision,
            source_fingerprint: 0,
            project_id: None,
            text: Arc::from(text),
        })
    }

    /// 缓存同时有界于条目数与总字节；命中刷新最近使用顺序。
    #[test]
    fn snapshot_cache_is_bounded_and_lru() {
        let mut cache = ReadSnapshotCache::new(2, 8);
        cache.insert("a".to_owned(), cached(1, "1234"));
        cache.insert("b".to_owned(), cached(2, "5678"));
        assert!(cache.get("a").is_some(), "命中必须刷新顺序");

        cache.insert("c".to_owned(), cached(3, "9012"));
        assert!(cache.get("b").is_none(), "最久未使用项必须先淘汰");
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.bytes(), 8);
    }

    /// 单条超过字节预算时保留唯一项，保证分页仍可续读。
    #[test]
    fn oversized_single_snapshot_is_retained() {
        let mut cache = ReadSnapshotCache::new(4, 2);
        cache.insert("big".to_owned(), cached(1, "123456"));
        assert!(cache.get("big").is_some());
        assert_eq!(cache.bytes(), 6);
    }

    /// 游标解析暴露快照身份，供读取层在续页跳过重物化。
    #[test]
    fn cursor_part_exposes_snapshot_identity() {
        let generation = ReadGeneration {
            resource: 42,
            generation: 3,
            revision: 9,
        };
        let page = read_page(b"0123456789", generation, None, 4).unwrap();
        let part = cursor_part(page.next_cursor.as_deref().unwrap()).unwrap();
        assert_eq!(part.resource, 42);
        assert_eq!(
            part.snapshot,
            CursorSnapshot {
                generation: 3,
                revision: 9
            }
        );
        assert!(cursor_part("v2:bad").is_none());
    }
}
