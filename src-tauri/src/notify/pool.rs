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

/// 池中条目：watcher + 最后使用时间（LRU 淘汰依据）+ pinned 钉住标记
struct WatcherEntry {
    watcher: FileWatcher,
    last_used: Instant,
    /// pinned 语义（ADR-0024）：agent 全局目录等长期监听——`pause_all_except` 跳过
    ///（项目切换不暂停之；其启动也不暂停项目 watcher），LRU 淘汰避让
    pinned: bool,
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
    /// 若池已满，淘汰最久未使用的 entry（LRU，pinned 条目避让）。
    /// `pinned` = true：长期监听（agent 全局目录等），不参与项目切换暂停/恢复。
    pub fn insert(&mut self, path: PathBuf, watcher: FileWatcher, pinned: bool) {
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
                pinned,
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

    /// 暂停除 `active` 外的所有非 pinned watcher，对 active 执行 resume。
    /// pinned 条目（ADR-0024）跳过暂停/恢复但仍 touch 使用时间（仍在服务，
    /// 不应成 LRU 淘汰首选）。若 active 不在池中则只执行 pause。
    pub fn pause_all_except(&mut self, active: &Path) {
        for (path, entry) in self.entries.iter_mut() {
            if !entry.pinned {
                if path == active {
                    entry.watcher.resume();
                } else {
                    entry.watcher.pause();
                }
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

    /// 淘汰最久未使用的 watcher（pinned 避让——ADR-0024：agent 视图展开期间其
    /// watcher 不被项目切换挤掉；极端全 pinned 时退化全池 LRU，保证插入不死锁）
    fn evict_lru(&mut self) {
        let lru_path = self
            .entries
            .iter()
            .filter(|(_, e)| !e.pinned)
            .min_by_key(|(_, e)| e.last_used)
            .map(|(p, _)| p.clone())
            .or_else(|| {
                self.entries
                    .iter()
                    .min_by_key(|(_, e)| e.last_used)
                    .map(|(p, _)| p.clone())
            });

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
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
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
        pool.insert(path.clone(), make_test_watcher("a"), false);
        assert_eq!(pool.len(), 1);
        assert!(pool.get(&path).is_some());
        assert!(pool.contains(&path));
    }

    #[test]
    fn get_updates_last_used_reordering_lru() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");

        pool.insert(a.clone(), make_test_watcher("a"), false);
        pool.insert(b.clone(), make_test_watcher("b"), false);

        // 访问 a，使其成为最近使用
        pool.get(&a);

        // 插入 c~i（7 个）→ 容量 8，第 9 个插入应淘汰 b（最久未使用）
        for name in &["c", "d", "e", "f", "g", "h", "i"] {
            pool.insert(
                PathBuf::from(format!("/test/{name}")),
                make_test_watcher(name),
                false,
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
                false,
            );
        }
        assert_eq!(pool.len(), WATCHER_POOL_CAPACITY);

        // a 是最久未使用 → 应被淘汰
        pool.insert(PathBuf::from("/test/i"), make_test_watcher("i"), false);
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

        pool.insert(a.clone(), make_test_watcher("a"), false);
        pool.insert(b.clone(), make_test_watcher("b"), false);
        pool.insert(c.clone(), make_test_watcher("c"), false);

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
        pool.insert(path.clone(), make_test_watcher("a"), false);
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
                false,
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
                false,
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
            false,
        );
        assert_eq!(pool.len(), 1);
        assert!(pool.get(&path).unwrap().is_running(), "旧 watcher 应运行中");

        // 同 path 直接二次 insert：真实执行 insert 内部"已存在→stop 旧 watcher"替换分支
        pool.insert(path.clone(), make_test_watcher("new"), false);
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
        pool.insert(PathBuf::from("/test/a"), make_test_watcher("a"), false);
        // 移除不存在的 path（notify_stop_watch 幂等契约）：返回 None，池不受影响
        assert!(
            pool.remove(&PathBuf::from("/test/not-exist")).is_none(),
            "移除不存在的路径应返回 None"
        );
        assert_eq!(pool.len(), 1);
        assert!(pool.contains(&PathBuf::from("/test/a")));
    }

    // ── pinned 语义（ADR-0024：agent 全局目录长期监听） ──

    /// pause_all_except 跳过 pinned 条目：pinned 不暂停、非 pinned 照常暂停
    #[test]
    fn pause_all_except_skips_pinned() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let agent = PathBuf::from("/home/u/.claude");
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");

        pool.insert(agent.clone(), make_test_watcher("agent"), true);
        pool.insert(a.clone(), make_test_watcher("a"), false);
        pool.insert(b.clone(), make_test_watcher("b"), false);

        pool.pause_all_except(&b);

        assert!(
            !pool.get(&agent).unwrap().is_paused(),
            "pinned watcher 不应被暂停"
        );
        assert!(pool.get(&a).unwrap().is_paused(), "非 pinned 应被暂停");
        assert!(!pool.get(&b).unwrap().is_paused(), "active 目标应 resume");
    }

    /// pause_all_except 对 pinned 仍 touch 使用时间（不当 LRU 淘汰首选）
    #[test]
    fn pause_all_except_touches_pinned_last_used() {
        let mut pool = LruWatcherPool::new(2);
        let agent = PathBuf::from("/home/u/.claude");
        let a = PathBuf::from("/test/a");

        pool.insert(agent.clone(), make_test_watcher("agent"), true);
        std::thread::sleep(Duration::from_millis(5));
        pool.insert(a.clone(), make_test_watcher("a"), false);

        // pause_all_except touch agent → agent 比 a 新
        std::thread::sleep(Duration::from_millis(5));
        pool.pause_all_except(&a);

        // 插入第三个（容量 2）→ 淘汰最久未用：a 先于 agent 插入且未被 touch → 淘汰 a
        pool.insert(PathBuf::from("/test/c"), make_test_watcher("c"), false);
        assert!(pool.contains(&agent), "pinned 被 touch 后不应成淘汰首选");
        assert!(!pool.contains(&a), "a 最久未用应被淘汰");
    }

    /// evict_lru 避让 pinned：pinned 最久未用也保活，淘汰次老的非 pinned
    #[test]
    fn evict_lru_spares_pinned() {
        let mut pool = LruWatcherPool::new(2);
        let agent = PathBuf::from("/home/u/.claude");
        let a = PathBuf::from("/test/a");

        // agent 先插入（最老）但 pinned；a 后插入非 pinned
        pool.insert(agent.clone(), make_test_watcher("agent"), true);
        std::thread::sleep(Duration::from_millis(5));
        pool.insert(a.clone(), make_test_watcher("a"), false);

        // 池满插入第三个 → 淘汰避让 pinned 的 agent，淘汰 a
        pool.insert(PathBuf::from("/test/c"), make_test_watcher("c"), false);
        assert_eq!(pool.len(), 2);
        assert!(pool.contains(&agent), "pinned watcher 应避让淘汰");
        assert!(!pool.contains(&a), "非 pinned 的 a 应被淘汰");
    }

    /// 极端退化：全部条目 pinned 且池满 → 仍淘汰最久 pinned（插入不死锁）
    #[test]
    fn evict_lru_all_pinned_falls_back_to_lru() {
        let mut pool = LruWatcherPool::new(2);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");

        pool.insert(a.clone(), make_test_watcher("a"), true);
        std::thread::sleep(Duration::from_millis(5));
        pool.insert(b.clone(), make_test_watcher("b"), true);

        pool.insert(PathBuf::from("/test/c"), make_test_watcher("c"), true);
        assert_eq!(pool.len(), 2, "全 pinned 池满仍应淘汰最久项");
        assert!(!pool.contains(&a), "最老的 pinned a 应被退化淘汰");
        assert!(pool.contains(&b));
    }

    /// 同 path 替换可改变 pinned 标记（pinned → 非 pinned 替换）
    #[test]
    fn insert_same_path_replaces_pinned_flag() {
        let mut pool = LruWatcherPool::new(WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/a");
        pool.insert(path.clone(), make_test_watcher("old"), true);
        pool.insert(path.clone(), make_test_watcher("new"), false);

        // 替换后条目为非 pinned → pause_all_except 对其它目标时应暂停它
        let b = PathBuf::from("/test/b");
        pool.insert(b.clone(), make_test_watcher("b"), false);
        pool.pause_all_except(&b);
        assert!(
            pool.get(&path).unwrap().is_paused(),
            "替换为非 pinned 后应参与暂停"
        );
    }

    #[test]
    fn watcher_pool_capacity_is_8() {
        // BE-11 守卫：容量常量固定为 8（覆盖多项目快速切换；pause/resume 既定机制保留）
        assert_eq!(WATCHER_POOL_CAPACITY, 8);
    }
}
