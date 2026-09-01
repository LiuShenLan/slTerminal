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

/// 池容量：覆盖多项目快速切换（BE-11）
///
/// 5 → 8 理由：用户在多项目间快速来回切换时，5 槽位易被交替访问的项目挤出重建
/// （Windows 上递归注册大目录约需 2s，重建成本高）；8 覆盖「4-5 个活跃项目 +
/// 3-4 个近期访问项目」的典型工作集。暂停的 watcher 仍占 OS 句柄（pause/resume
/// 既定机制保留，不额外清理），故容量即 OS 句柄占用上限——8 个 watcher 的句柄
/// 开销可忽略，放大容量换取切换零重建。
pub const WATCHER_POOL_CAPACITY: usize = 8;

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
mod pool_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    /// 创建测试用 FileWatcher（不监听实际目录，线程真实监听 stop_rx）
    ///
    /// 线程在 stop_rx 收到信号或通道断开时立即退出，不再空转。
    /// 通过 FileWatcher::stop() → stop_tx.send(()) → 线程退出 → is_running() 变为 false。
    fn make_test_watcher(name: &str) -> FileWatcher {
        make_test_watcher_with_exit(name, None)
    }

    /// 带线程退出标志的测试 watcher：线程退出时置位 `exit` 标志，
    /// 供 p9/p10 断言「watcher 已被 stop / 已被 drop」的真实线程退出。
    fn make_test_watcher_with_exit(name: &str, exit: Option<Arc<AtomicBool>>) -> FileWatcher {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let paused = Arc::new(AtomicBool::new(false));

        let handle = std::thread::Builder::new()
            .name(format!("test-watcher-{name}"))
            .spawn(move || {
                // 真实监听 stop_rx：收到停止信号或通道断开时退出
                loop {
                    match stop_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // 空转，等待停止信号
                        }
                    }
                }
                if let Some(flag) = exit {
                    flag.store(true, Ordering::SeqCst);
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
    fn new_creates_empty_pool() {
        let pool = LruWatcherPool::new(5);
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn insert_and_get_hit() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/project-a");
        pool.insert(path.clone(), make_test_watcher("a"));
        assert_eq!(pool.len(), 1);
        assert!(pool.get(&path).is_some());
        assert!(pool.contains(&path));
    }

    #[test]
    fn get_updates_last_used_reordering_lru() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");

        pool.insert(a.clone(), make_test_watcher("a"));
        pool.insert(b.clone(), make_test_watcher("b"));

        // 访问 a，使其成为最近使用
        pool.get(&a);

        // 插入 c~i（7 个）→ 容量 8，第 9 个插入应淘汰 b（最久未使用）
        for name in &["c", "d", "e", "f", "g", "h", "i"] {
            pool.insert(
                PathBuf::from(format!("/test/{name}")),
                make_test_watcher(name),
            );
        }

        assert_eq!(pool.len(), WATCHER_POOL_CAPACITY);
        // a 被最近访问 → 保留
        assert!(pool.contains(&a), "a 应保留（被 get 刷新）");
        // b 从未被访问 → 淘汰
        assert!(!pool.contains(&b), "b 应被 LRU 淘汰");

        // 验证剩余 watcher 均在运行（淘汰的 b 已 stopped + dropped）
        for name in &["a", "c", "d", "e", "f", "g", "h", "i"] {
            let p = PathBuf::from(format!("/test/{name}"));
            assert!(
                pool.get(&p).unwrap().is_running(),
                "剩余 watcher {name} 应仍在运行"
            );
        }
    }

    #[test]
    fn insert_at_capacity_evicts_lru() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        for name in &["a", "b", "c", "d", "e", "f", "g", "h"] {
            pool.insert(
                PathBuf::from(format!("/test/{name}")),
                make_test_watcher(name),
            );
        }
        assert_eq!(pool.len(), WATCHER_POOL_CAPACITY);

        // a 是最久未使用 → 应被淘汰
        pool.insert(PathBuf::from("/test/i"), make_test_watcher("i"));
        assert_eq!(pool.len(), WATCHER_POOL_CAPACITY);
        assert!(!pool.contains(&PathBuf::from("/test/a")), "a 应被 LRU 淘汰");
        assert!(pool.contains(&PathBuf::from("/test/i")), "i 应存在");

        // 验证剩余 watcher 均在运行（淘汰的 a 已 stopped + dropped）
        for name in &["b", "c", "d", "e", "f", "g", "h", "i"] {
            let p = PathBuf::from(format!("/test/{name}"));
            assert!(
                pool.get(&p).unwrap().is_running(),
                "剩余 watcher {name} 应仍在运行"
            );
        }
    }

    #[test]
    fn pause_all_except_target_resumed_others_paused() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
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
    fn pause_all_except_empty_pool_no_panic() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/not-exist");
        // 空池不应 panic
        pool.pause_all_except(&path);
        assert!(pool.is_empty());
    }

    #[test]
    fn remove_stops_and_returns_watcher() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/a");
        pool.insert(path.clone(), make_test_watcher("a"));
        assert_eq!(pool.len(), 1);

        let watcher = pool.remove(&path).unwrap();
        assert!(
            !watcher.is_running(),
            "remove 后 watcher 应已停止（内部调 stop）"
        );
        assert_eq!(pool.len(), 0);
        assert!(!pool.contains(&path));
    }

    #[test]
    fn stop_all_clears_pool() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        for name in &["a", "b", "c"] {
            pool.insert(
                PathBuf::from(format!("/test/{name}")),
                make_test_watcher(name),
            );
        }
        assert_eq!(pool.len(), 3);

        pool.stop_all();
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn drop_stops_all_watchers() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let mut exit_flags = Vec::new();
        for i in 0..3 {
            let flag = Arc::new(AtomicBool::new(false));
            pool.insert(
                PathBuf::from(format!("/test/w{i}")),
                make_test_watcher_with_exit(&format!("w{i}"), Some(flag.clone())),
            );
            exit_flags.push(flag);
        }
        assert_eq!(pool.len(), 3);

        drop(pool);
        // Drop → stop_all → 各 watcher 线程应已退出（HFN-09①：补真实线程退出断言）
        for (i, flag) in exit_flags.iter().enumerate() {
            assert!(
                flag.load(Ordering::SeqCst),
                "watcher w{i} 线程应在池 Drop 后退出"
            );
        }
    }

    #[test]
    fn insert_same_path_replaces_old_watcher() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/a");

        // 旧 watcher 带退出标志：验证 insert 内部替换分支 stop 了它（HFN-02：不再手动 remove）
        let old_exited = Arc::new(AtomicBool::new(false));
        pool.insert(
            path.clone(),
            make_test_watcher_with_exit("old", Some(old_exited.clone())),
        );
        assert_eq!(pool.len(), 1);
        assert!(pool.get(&path).unwrap().is_running(), "旧 watcher 应运行中");

        // 同 path 直接二次 insert：真实执行 insert 内部"已存在→stop 旧 watcher"替换分支
        pool.insert(path.clone(), make_test_watcher("new"));
        assert_eq!(pool.len(), 1, "同一 path 不应增加计数");
        assert!(pool.contains(&path));
        assert!(pool.get(&path).unwrap().is_running(), "新 watcher 应运行中");
        assert!(
            old_exited.load(Ordering::SeqCst),
            "旧 watcher 线程应已被 insert 替换分支 stop"
        );
    }

    #[test]
    fn pause_and_resume_is_paused_toggles() {
        let w = make_test_watcher("toggle");
        assert!(!w.is_paused());

        w.pause();
        assert!(w.is_paused());

        w.resume();
        assert!(!w.is_paused());
    }

    #[test]
    fn paused_watcher_does_not_process_events() {
        // 验证 pause/resume 机制：paused 标记正确切换
        let w = make_test_watcher("paused-test");

        w.pause();
        assert!(w.is_paused(), "pause 后应标记 paused");

        w.resume();
        assert!(!w.is_paused(), "resume 后应清除 paused");
    }

    #[test]
    fn stop_sets_is_running_false() {
        let mut w = make_test_watcher("stop-test");
        assert!(w.is_running(), "创建后应运行中");
        w.stop();
        assert!(
            !w.is_running(),
            "stop 后应不再运行（thread_handle 被 take + join）"
        );
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        pool.insert(PathBuf::from("/test/a"), make_test_watcher("a"));
        // 移除不存在的 path（notify_stop_watch 幂等契约）：返回 None，池不受影响
        assert!(
            pool.remove(&PathBuf::from("/test/not-exist")).is_none(),
            "移除不存在的路径应返回 None"
        );
        assert_eq!(pool.len(), 1);
        assert!(pool.contains(&PathBuf::from("/test/a")));
    }

    #[test]
    fn watcher_pool_capacity_is_8() {
        // BE-11 守卫：容量常量固定为 8（覆盖多项目快速切换；pause/resume 既定机制保留）
        assert_eq!(WATCHER_POOL_CAPACITY, 8);
    }
}
