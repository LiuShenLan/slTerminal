//! 有界热缓存:遍历前缀保留一次,后续查询直接复用。
//!
//! 目录 watch(非递归)让新建/重命名/删除在下一轮唤醒缓存;watch 数量
//! 与路径长度都有上限,超限只降级为「不再复用」,不影响查询正确性。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use super::*;

pub(super) struct FileCache {
    pub entries: Vec<IndexedPath>,
    pub memory: FileSearchMemory,
    pub full: bool,
    pub complete: bool,
    pub watches: DirectoryWatches,
    pub finished: Instant,
}

pub(super) struct DirectoryWatches {
    pub dirty: Arc<AtomicBool>,
    watcher: Option<RecommendedWatcher>,
    watched: HashSet<PathBuf>,
    watch_bytes: usize,
    pub unwatched: bool,
}

impl FileCache {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
            memory: FileSearchMemory::cache(),
            full: false,
            complete: false,
            watches: DirectoryWatches::new(),
            finished: Instant::now(),
        }
    }

    pub(super) fn record(&mut self, position: usize, entry: &IndexedPath) {
        if position < self.entries.len() {
            if self.entries[position].path != entry.path
                || self.entries[position].is_dir != entry.is_dir
            {
                // 未被观察到的编辑改变了遍历前缀:不追加重复项,也不再宣称
                // 这份缓存完整(下一次查询重扫)。
                self.entries = Vec::new();
                self.memory = FileSearchMemory::cache();
                self.full = true;
            }
            return;
        }
        if self.full || position != self.entries.len() {
            return;
        }
        let growth = if self.entries.len() == self.entries.capacity() {
            128
        } else {
            0
        };
        let bytes = entry.heap_bytes() + growth * std::mem::size_of::<IndexedPath>();
        if self.memory.bytes() + bytes > memory::CACHE_LIMIT || !self.memory.grow(bytes) {
            self.full = true;
            return;
        }
        if growth > 0 {
            self.entries.reserve_exact(growth);
        }
        self.entries.push(entry.clone());
    }

    pub(super) fn finish_prefix(&mut self, visited: usize) {
        if visited < self.entries.len() {
            let bytes = self.entries[visited..]
                .iter()
                .map(IndexedPath::heap_bytes)
                .sum();
            self.entries.truncate(visited);
            self.memory.release(bytes);
        }
    }
}

impl DirectoryWatches {
    pub(super) fn new() -> Self {
        Self {
            dirty: Arc::new(AtomicBool::new(false)),
            watcher: None,
            watched: HashSet::new(),
            watch_bytes: 0,
            unwatched: false,
        }
    }

    pub(super) fn watch(&mut self, directory: &Path) {
        if self.watched.contains(directory) {
            return;
        }
        if self.watched.len() >= 128 || self.watch_bytes + directory.as_os_str().len() > 64 * 1024 {
            self.unwatched = true;
            return;
        }
        if self.watcher.is_none() {
            let dirty = self.dirty.clone();
            self.watcher =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    if event.as_ref().map_or(true, |event| {
                        event.need_rescan() || event_changes_names(&event.kind)
                    }) {
                        dirty.store(true, Ordering::Release);
                    }
                })
                .ok();
        }
        let Some(watcher) = self.watcher.as_mut() else {
            self.unwatched = true;
            return;
        };
        if watcher
            .watch(directory, RecursiveMode::NonRecursive)
            .is_ok()
        {
            self.watch_bytes += directory.as_os_str().len();
            self.watched.insert(directory.to_owned());
        } else {
            self.unwatched = true;
        }
    }
}
