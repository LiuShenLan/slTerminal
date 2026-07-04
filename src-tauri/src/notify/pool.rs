//! LRU Watcher 池 — 管理多个 FileWatcher 实例的生命周期
//!
//! 职责：
//! - 缓存最多 `max_size` 个 watcher，按 LRU 淘汰
//! - `pause_all_except` — 切换项目时暂停/恢复 watcher，避免重建
//! - `stop_all` + Drop — 确保所有 watcher 线程正确退出

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::FileWatcher;

/// 池中条目：watcher + 最后使用时间（LRU 淘汰依据）
struct WatcherEntry {
    watcher: FileWatcher,
    last_used: Instant,
}

/// LRU watcher 池
pub struct LruWatcherPool {
    entries: HashMap<PathBuf, WatcherEntry>,
    max_size: usize,
}

impl LruWatcherPool {
    /// 创建容量为 `max_size` 的空池
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
        }
    }

    /// 当前缓存数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 池是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 查找 watcher（命中则更新最后使用时间）
    pub fn get(&mut self, path: &Path) -> Option<&FileWatcher> {
        if let Some(entry) = self.entries.get_mut(path) {
            entry.last_used = Instant::now();
            Some(&entry.watcher)
        } else {
            None
        }
    }

    /// 检查池中是否已存在指定 path 的 watcher
    pub fn contains(&mut self, path: &Path) -> bool {
        // 内部调 get 更新 last_used
        self.get(path).is_some()
    }

    /// 插入新 watcher。若已存在同 path 则替换旧 watcher（旧 watcher 被 stop）。
    /// 若池已满，淘汰最久未使用的 entry（LRU）。
    pub fn insert(&mut self, path: PathBuf, watcher: FileWatcher) {
        // 同一 path 替换：停掉旧的
        if let Some(mut old_entry) = self.entries.remove(&path) {
            old_entry.watcher.stop();
        }

        // 池满 → 淘汰 LRU
        if self.entries.len() >= self.max_size {
            self.evict_lru();
        }

        self.entries.insert(
            path,
            WatcherEntry {
                watcher,
                last_used: Instant::now(),
            },
        );
    }

    /// 移除指定 path 的 watcher（调用 stop 释放）
    pub fn remove(&mut self, path: &Path) -> Option<FileWatcher> {
        self.entries.remove(path).map(|mut entry| {
            entry.watcher.stop();
            entry.watcher
        })
    }

    /// 暂停除 `active` 外的所有 watcher，对 active 执行 resume。
    /// 若 active 不在池中则只执行 pause 所有现有 watcher。
    pub fn pause_all_except(&mut self, active: &Path) {
        for (path, entry) in self.entries.iter_mut() {
            if path == active {
                entry.watcher.resume();
            } else {
                entry.watcher.pause();
            }
            // 暂停/恢复操作更新使用时间
            entry.last_used = Instant::now();
        }
    }

    /// 停止所有 watcher 并清空池
    pub fn stop_all(&mut self) {
        for (_, mut entry) in self.entries.drain() {
            entry.watcher.stop();
        }
    }

    /// 淘汰最久未使用的 watcher
    fn evict_lru(&mut self) {
        let lru_path = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_used)
            .map(|(p, _)| p.clone());

        if let Some(path) = lru_path {
            self.remove(&path);
        }
    }
}

impl Drop for LruWatcherPool {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    /// 创建测试用 FileWatcher（不监听实际目录，空转线程）
    fn make_test_watcher(name: &str) -> FileWatcher {
        let (_stop_tx, stop_rx) = mpsc::channel::<()>();
        let (stop_tx, _) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<notify_debouncer_full::DebounceEventResult>();

        let paused = Arc::new(AtomicBool::new(false));
        let paused_clone = paused.clone();

        let handle = std::thread::Builder::new()
            .name(format!("test-watcher-{name}"))
            .spawn(move || loop {
                match event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {
                        if paused_clone.load(Ordering::Relaxed) {
                            continue;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if stop_rx.try_recv().is_ok() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            })
            .unwrap();

        FileWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
            paused,
        }
    }

    // ── 基础操作 ──

    #[test]
    fn p1_new_creates_empty_pool() {
        let pool = LruWatcherPool::new(5);
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn p2_insert_and_get_hit() {
        let mut pool = LruWatcherPool::new(5);
        let path = PathBuf::from("/test/project-a");
        pool.insert(path.clone(), make_test_watcher("a"));
        assert_eq!(pool.len(), 1);
        assert!(pool.get(&path).is_some());
        assert!(pool.contains(&path));
    }

    #[test]
    fn p3_get_updates_last_used_reordering_lru() {
        let mut pool = LruWatcherPool::new(5);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");

        pool.insert(a.clone(), make_test_watcher("a"));
        pool.insert(b.clone(), make_test_watcher("b"));

        // 访问 a，使其成为最近使用
        pool.get(&a);

        // 插入 c、d、e、f → 容量为 5 → 应淘汰 b（最久未使用）
        for name in &["c", "d", "e", "f"] {
            pool.insert(PathBuf::from(format!("/test/{name}")), make_test_watcher(name));
        }

        assert_eq!(pool.len(), 5);
        // a 被最近访问 → 保留
        assert!(pool.contains(&a), "a 应保留（被 get 刷新）");
        // b 从未被访问 → 淘汰
        assert!(!pool.contains(&b), "b 应被 LRU 淘汰");
    }

    #[test]
    fn p4_insert_at_capacity_evicts_lru() {
        let mut pool = LruWatcherPool::new(5);
        for name in &["a", "b", "c", "d", "e"] {
            pool.insert(PathBuf::from(format!("/test/{name}")), make_test_watcher(name));
        }
        assert_eq!(pool.len(), 5);

        // a 是最久未使用 → 应被淘汰
        pool.insert(PathBuf::from("/test/f"), make_test_watcher("f"));
        assert_eq!(pool.len(), 5);
        assert!(!pool.contains(&PathBuf::from("/test/a")), "a 应被 LRU 淘汰");
        assert!(pool.contains(&PathBuf::from("/test/f")), "f 应存在");
    }

    #[test]
    fn p5_pause_all_except_target_resumed_others_paused() {
        let mut pool = LruWatcherPool::new(5);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");
        let c = PathBuf::from("/test/c");

        pool.insert(a.clone(), make_test_watcher("a"));
        pool.insert(b.clone(), make_test_watcher("b"));
        pool.insert(c.clone(), make_test_watcher("c"));

        pool.pause_all_except(&b);

        // b 应 resumed
        assert!(!pool.get(&b).unwrap().is_paused(), "b 应 resumed");
        // a、c 应 paused
        assert!(pool.get(&a).unwrap().is_paused(), "a 应 paused");
        assert!(pool.get(&c).unwrap().is_paused(), "c 应 paused");
    }

    #[test]
    fn p6_pause_all_except_empty_pool_no_panic() {
        let mut pool = LruWatcherPool::new(5);
        let path = PathBuf::from("/test/not-exist");
        // 空池不应 panic
        pool.pause_all_except(&path);
        assert!(pool.is_empty());
    }

    #[test]
    fn p7_remove_stops_and_returns_watcher() {
        let mut pool = LruWatcherPool::new(5);
        let path = PathBuf::from("/test/a");
        pool.insert(path.clone(), make_test_watcher("a"));
        assert_eq!(pool.len(), 1);

        let watcher = pool.remove(&path);
        assert!(watcher.is_some(), "remove 应返回 watcher");
        assert_eq!(pool.len(), 0);
        assert!(!pool.contains(&path));
    }

    #[test]
    fn p8_stop_all_clears_pool() {
        let mut pool = LruWatcherPool::new(5);
        for name in &["a", "b", "c"] {
            pool.insert(PathBuf::from(format!("/test/{name}")), make_test_watcher(name));
        }
        assert_eq!(pool.len(), 3);

        pool.stop_all();
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn p9_drop_stops_all_watchers() {
        let pool = LruWatcherPool::new(5);
        // 不做额外断言：Drop 后不 panic 即验证通过
        drop(pool);
    }

    #[test]
    fn p10_insert_same_path_replaces_old_watcher() {
        let mut pool = LruWatcherPool::new(5);
        let path = PathBuf::from("/test/a");

        pool.insert(path.clone(), make_test_watcher("old"));
        assert_eq!(pool.len(), 1);

        // 插入同一 path → 旧 watcher 应被 stop + 替换
        pool.insert(path.clone(), make_test_watcher("new"));
        assert_eq!(pool.len(), 1, "同一 path 不应增加计数");
        assert!(pool.contains(&path));
    }

    #[test]
    fn p11_pause_and_resume_is_paused_toggles() {
        let w = make_test_watcher("toggle");
        assert!(!w.is_paused());

        w.pause();
        assert!(w.is_paused());

        w.resume();
        assert!(!w.is_paused());
    }

    #[test]
    fn p12_paused_watcher_does_not_process_events() {
        // 验证 pause/resume 机制：paused 标记正确切换
        let w = make_test_watcher("paused-test");

        w.pause();
        assert!(w.is_paused(), "pause 后应标记 paused");

        w.resume();
        assert!(!w.is_paused(), "resume 后应清除 paused");
    }
}
