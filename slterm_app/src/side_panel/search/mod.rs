//! 有界文件名搜索:流式遍历 + 热缓存复用 + generation 防串台。
//!
//! 07 篇照抄点 21(search 子模块;WSL find 臂已砍):
//! - 一个进程内 worker 串行处理所有查询;视图侧只提交查询、收割
//!   generation 匹配的结果,从不等待;
//! - 热缓存按遍历前缀保留,重复查询不再走盘;清空 query 不触发遍历;
//! - 内存租约随分配属主走(缓存/已发布结果两套进程级预算)。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::EventKind;
use notify::event::ModifyKind;

mod cache;
mod matching;
mod memory;
mod walk;
mod worker;

#[cfg(test)]
#[path = "search_tests.rs"]
mod search_tests;

pub(crate) use memory::FileSearchMemory;
pub(crate) use worker::EmbeddedFileIndex;

use super::FileRow;

/// 单次查询最多返回的行数。
pub(crate) const MAX_SEARCH_RESULTS: usize = 1_000;
/// 输入防抖:连续键入只触发最后一趟遍历。
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);
/// 无工作时等待 fs 事件的窗口。
const WATCH_DEBOUNCE: Duration = Duration::from_millis(100);
const INDEX_IDLE: u8 = 0;
const INDEX_BUILDING: u8 = 1;
const INDEX_READY: u8 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FileSearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileIndexStatus {
    Idle,
    Building,
    Ready,
}

#[derive(Clone)]
struct SearchRequest {
    epoch: u64,
    generation: u64,
    query: String,
    options: FileSearchOptions,
}

#[derive(Debug)]
pub(crate) struct FileSearchResult {
    pub(crate) epoch: u64,
    pub(crate) generation: u64,
    pub(crate) query: String,
    pub(crate) options: FileSearchOptions,
    pub(crate) rows: Vec<FileRow>,
    pub(crate) total: usize,
    pub(crate) error: Option<String>,
    pub(crate) memory: FileSearchMemory,
    pub(crate) complete: bool,
}

#[derive(Debug, Clone)]
struct IndexedPath {
    path: PathBuf,
    name: String,
    name_folded: String,
    /// 仓库根相对路径,`/` 分隔(匹配与展示同源)。
    key: String,
    key_folded: String,
    is_dir: bool,
}

impl IndexedPath {
    fn heap_bytes(&self) -> usize {
        self.path.capacity()
            + self.name.capacity()
            + self.name_folded.capacity()
            + self.key.capacity()
            + self.key_folded.capacity()
    }
}

fn event_changes_names(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Name(_))
    )
}

fn indexed_local_path(root: &Path, path: &Path, is_dir: bool) -> Option<IndexedPath> {
    let relative = path.strip_prefix(root).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    let key = relative.to_string_lossy().replace('\\', "/");
    Some(IndexedPath {
        path: path.to_path_buf(),
        name_folded: name.to_lowercase(),
        key_folded: key.to_lowercase(),
        name,
        key,
        is_dir,
    })
}
