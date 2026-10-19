//! 外部文件系统事件监听:树根递归监听 + 打开文件的父目录监听。
//!
//! 设计约束:
//! - **事件驱动**:文件树不做定时重扫,外部新增/删除/改名经本监听器进入
//!   前台合并刷新;显式刷新按钮仍是兜底。
//! - **回调线程只做过滤与去重**:`notify` 自有线程上仅分类事件、写共享脏集,
//!   再用容量 1 的 `sync_channel` 唤醒前台(满即丢信号,脏集已记事件)。
//! - **引用计数**:同一父目录下打开多个文件只 watch 一次;全部关闭才
//!   unwatch。父目录已被树根递归覆盖时不重复 watch,树根切换/释放时重新
//!   对齐,不得把树根 watch 误删。
//! - watcher 创建失败返回 `None` 并降级(刷新按钮仍可用),不 panic。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// 一次前台收割的事件路径集:文件脏(精确命中)与目录脏(树根内)。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FsPending {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) dirs: Vec<PathBuf>,
}

impl FsPending {
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty() && self.dirs.is_empty()
    }
}

/// 回调线程与前台共享的分类状态(锁粒度 = 一次事件分类/一次收割)。
#[derive(Debug, Default)]
struct WatchState {
    /// 已登记文件(精确路径过滤;含已关闭 tab 路径时由 retain 清掉)。
    files: HashSet<PathBuf>,
    /// 当前树根;`tree_active` = 递归 watch 是否建立成功。
    tree_root: Option<PathBuf>,
    tree_active: bool,
    dirty_files: HashSet<PathBuf>,
    dirty_dirs: HashSet<PathBuf>,
}

fn lock_state(state: &Mutex<WatchState>) -> std::sync::MutexGuard<'_, WatchState> {
    // 回调线程 panic 不应让前台永久失效:毒锁按内部值继续。
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 单实例文件系统监听器(随 workspace 存活;Drop 释放 notify 线程)。
pub(crate) struct FsWatch {
    /// `None` 仅在构造成功后不会出现;保留 Option 便于 Drop 先行释放。
    watcher: Option<RecommendedWatcher>,
    state: Arc<Mutex<WatchState>>,
    /// 文件父目录引用计数:值 = 该目录下登记的文件数。
    parent_refs: HashMap<PathBuf, usize>,
    /// 已单独 watch 的父目录(不含树根递归覆盖的部分)。
    watched_parents: HashSet<PathBuf>,
    /// 上一次建立递归 watch 失败的根:同根不重复尝试,避免每帧告警。
    failed_tree_root: Option<PathBuf>,
    /// 有界唤醒信号:只用于提示前台"有事件了";容量 1,满即丢。
    wake_rx: Receiver<()>,
}

impl FsWatch {
    /// 建立 watcher;平台 watcher 不可用时记录降级并返回 `None`。
    pub(crate) fn new() -> Option<Self> {
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let state = Arc::new(Mutex::new(WatchState::default()));
        let callback_state = Arc::clone(&state);
        let watcher =
            match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let event = match result {
                    Ok(event) => event,
                    Err(error) => {
                        log::warn!("fs watch: event delivery failed: {error}");
                        return;
                    }
                };
                // 访问事件(读/打开)不改变磁盘内容,直接丢弃。
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                let mut state = lock_state(&callback_state);
                for path in event.paths {
                    // 命中已登记文件 = 文件脏;树根内路径 = 目录脏。
                    if state.files.contains(&path) {
                        state.dirty_files.insert(path.clone());
                    }
                    let inside_tree = match state.tree_root.as_ref() {
                        Some(root) => state.tree_active && path.starts_with(root),
                        None => false,
                    };
                    if inside_tree {
                        state.dirty_dirs.insert(path);
                    }
                }
                drop(state);
                // 满 = 前台尚未收割,已有一次唤醒在途;脏集才是真值。
                let _ = wake_tx.try_send(());
            }) {
                Ok(watcher) => watcher,
                Err(error) => {
                    log::warn!("fs watch: unavailable ({error}); manual refresh still works");
                    return None;
                }
            };
        Some(Self {
            watcher: Some(watcher),
            state,
            parent_refs: HashMap::new(),
            watched_parents: HashSet::new(),
            failed_tree_root: None,
            wake_rx,
        })
    }

    /// 切换树根监听(None = 释放)。同一根且已生效时零成本。
    pub(crate) fn watch_tree_root(&mut self, root: Option<&Path>) {
        let next = root.map(Path::to_path_buf);
        let (current, active) = {
            let state = lock_state(&self.state);
            (state.tree_root.clone(), state.tree_active)
        };
        // 同根已生效,或同根此前建立失败(不重复尝试/告警)均零成本返回。
        if current == next && (next.is_none() || active || self.failed_tree_root == next) {
            return;
        }
        if current != next {
            self.failed_tree_root = None;
        }
        // 旧根释放:仅在其确实生效时 unwatch;新根建立失败走文件父目录兜底。
        if current != next
            && active
            && let (Some(previous), Some(watcher)) = (current.as_deref(), self.watcher.as_mut())
        {
            let _ = watcher.unwatch(previous);
        }
        // notify 的 unwatch 按路径生效:必须先撤掉将被新树根覆盖的单独
        // 父目录 watch,再安装递归根;反序会把刚装上的树根 watch 一起删掉。
        if let Some(path) = next.as_deref() {
            let covered: Vec<PathBuf> = self
                .watched_parents
                .iter()
                .filter(|parent| parent.starts_with(path))
                .cloned()
                .collect();
            for parent in covered {
                if let Some(watcher) = self.watcher.as_mut() {
                    let _ = watcher.unwatch(&parent);
                }
                self.watched_parents.remove(&parent);
            }
        }
        let mut active = false;
        if let (Some(path), Some(watcher)) = (next.as_deref(), self.watcher.as_mut()) {
            match watcher.watch(path, RecursiveMode::Recursive) {
                Ok(()) => active = true,
                Err(error) => {
                    self.failed_tree_root = Some(path.to_path_buf());
                    log::warn!(
                        "fs watch: cannot watch tree root {}: {error}",
                        path.display()
                    );
                }
            }
        }
        {
            let mut state = lock_state(&self.state);
            state.tree_root = next;
            state.tree_active = active;
        }
        // 树根覆盖范围变了:此前被树根覆盖的文件父目录要补 watch,
        // 新树根覆盖的父目录要撤销单独 watch。
        self.reconcile_file_watches();
    }

    /// 用当前打开的可编辑文件全集替换文件兴趣(全量对账,天然合并引用计数)。
    pub(crate) fn retain_files(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let next: HashSet<PathBuf> = paths.into_iter().collect();
        {
            let mut state = lock_state(&self.state);
            if state.files == next {
                return;
            }
            state.files = next.clone();
        }
        self.parent_refs = parent_counts(&next);
        self.reconcile_file_watches();
    }

    /// 收割脏集并清空;顺带排空唤醒信号(信号只是加速提示,不承载数据)。
    pub(crate) fn take_pending(&mut self) -> FsPending {
        while self.wake_rx.try_recv().is_ok() {}
        let mut state = lock_state(&self.state);
        FsPending {
            files: state.dirty_files.drain().collect(),
            dirs: state.dirty_dirs.drain().collect(),
        }
    }

    /// 是否仍有监听兴趣(文件或生效的树根)。无兴趣时前台泵应退出。
    pub(crate) fn has_interest(&self) -> bool {
        let state = lock_state(&self.state);
        !state.files.is_empty() || (state.tree_root.is_some() && state.tree_active)
    }

    /// 文件父目录 watch 对账:先补缺,再撤销不再需要且未被树根覆盖的。
    fn reconcile_file_watches(&mut self) {
        let tree_root = {
            let state = lock_state(&self.state);
            state.tree_root.clone().filter(|_| state.tree_active)
        };
        // 树根递归覆盖的父目录不单独 watch(含根目录自身)。
        let desired: HashSet<PathBuf> = self
            .parent_refs
            .keys()
            .filter(|parent| match tree_root.as_ref() {
                Some(root) => !parent.starts_with(root),
                None => true,
            })
            .cloned()
            .collect();

        let stale: Vec<PathBuf> = self.watched_parents.difference(&desired).cloned().collect();
        for parent in stale {
            if let Some(watcher) = self.watcher.as_mut()
                && let Err(error) = watcher.unwatch(&parent)
            {
                log::debug!("fs watch: cannot unwatch {}: {error}", parent.display());
            }
            self.watched_parents.remove(&parent);
        }

        let missing: Vec<PathBuf> = desired.difference(&self.watched_parents).cloned().collect();
        for parent in missing {
            let Some(watcher) = self.watcher.as_mut() else {
                break;
            };
            match watcher.watch(&parent, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.watched_parents.insert(parent);
                }
                Err(error) => {
                    log::warn!("fs watch: cannot watch {}: {error}", parent.display());
                }
            }
        }
    }
}

impl Drop for FsWatch {
    fn drop(&mut self) {
        // notify 的 watcher 自带后台线程,Drop 即终止,无需 join。
        self.watcher.take();
    }
}

/// 文件 → 父目录引用计数;无父目录的裸文件名不参与监听。
fn parent_counts(files: &HashSet<PathBuf>) -> HashMap<PathBuf, usize> {
    let mut counts = HashMap::new();
    for file in files {
        if let Some(parent) = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            *counts.entry(parent.to_path_buf()).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod fs_watch_tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// 轮询等待事件落地(notify 线程异步;固定 sleep 易抖动)。
    fn wait_for_pending(watch: &mut FsWatch, matches: impl Fn(&FsPending) -> bool) -> FsPending {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let pending = watch.take_pending();
            if matches(&pending) {
                return pending;
            }
            assert!(
                Instant::now() < deadline,
                "fs watch event did not arrive within 5s"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 树根递归:根内新建文件 → 目录脏。
    #[test]
    fn tree_root_creation_marks_the_directory_dirty() {
        let directory = tempfile::tempdir().unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.watch_tree_root(Some(directory.path()));
        assert!(watch.has_interest());

        let created = directory.path().join("created.txt");
        std::fs::write(&created, b"new\n").unwrap();

        let pending = wait_for_pending(&mut watch, |pending| !pending.dirs.is_empty());
        assert!(
            pending
                .dirs
                .iter()
                .any(|path| path.ends_with("created.txt")),
            "目录脏集必须包含新文件路径: {:?}",
            pending.dirs
        );
        assert!(pending.files.is_empty(), "未登记文件不得进文件脏集");
    }

    /// 登记文件被外部改写 → 文件脏;无树根时目录脏集不得混入。
    #[test]
    fn registered_file_change_is_file_dirty_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tracked.txt");
        std::fs::write(&path, b"one\n").unwrap();

        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([path.clone()]);

        std::fs::write(&path, b"changed\n").unwrap();
        let pending = wait_for_pending(&mut watch, |pending| !pending.files.is_empty());
        assert_eq!(pending.files, vec![path]);
        assert!(
            pending.dirs.is_empty(),
            "无树根时目录脏集必须为空: {:?}",
            pending.dirs
        );
    }

    /// 同一父目录多个文件只 watch 一次;关闭一个文件不撤销仍在用的父目录。
    #[test]
    fn shared_parent_watch_survives_one_file_closing() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        std::fs::write(&first, b"1\n").unwrap();
        std::fs::write(&second, b"2\n").unwrap();

        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([first.clone(), second.clone()]);
        assert_eq!(
            watch.parent_refs.get(directory.path()).copied(),
            Some(2),
            "父目录引用计数 = 登记文件数"
        );
        watch.retain_files([first.clone()]);
        assert_eq!(watch.parent_refs.get(directory.path()).copied(), Some(1));

        // 仍登记的文件继续上报。
        std::fs::write(&first, b"changed\n").unwrap();
        let pending = wait_for_pending(&mut watch, |pending| !pending.files.is_empty());
        assert_eq!(pending.files, vec![first]);
    }

    /// 关闭兴趣后不再上报(父目录 watch 已撤销)。
    #[test]
    fn closing_interest_stops_reporting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("closed.txt");
        std::fs::write(&path, b"one\n").unwrap();

        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([path.clone()]);
        watch.retain_files(Vec::new());
        assert!(!watch.has_interest());
        assert!(watch.watched_parents.is_empty());

        std::fs::write(&path, b"changed\n").unwrap();
        // 反向断言:给事件线程一个窗口,期间不得报告任何脏路径。
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            let pending = watch.take_pending();
            assert!(pending.is_empty(), "关闭兴趣后不得上报: {pending:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 文件父目录 watch 升级为树根递归 watch:递归根不得被随后的父目录
    /// 对账误删。
    #[test]
    fn tree_root_replaces_covered_file_parent_watch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("upgraded.txt");
        std::fs::write(&path, b"one\n").unwrap();

        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([path.clone()]);
        assert!(watch.watched_parents.contains(directory.path()));
        watch.watch_tree_root(Some(directory.path()));
        assert!(
            !watch.watched_parents.contains(directory.path()),
            "已被树根覆盖的父目录必须撤销单独 watch"
        );

        std::fs::write(&path, b"changed\n").unwrap();
        let pending = wait_for_pending(&mut watch, |pending| !pending.dirs.is_empty());
        assert!(
            pending
                .dirs
                .iter()
                .any(|dirty| dirty.ends_with("upgraded.txt")),
            "升级后递归根必须仍在工作: {:?}",
            pending.dirs
        );
    }

    /// 根切换:旧根事件不再上报,新根事件正常上报。
    #[test]
    fn switching_tree_root_moves_the_recursive_watch() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.watch_tree_root(Some(first.path()));
        watch.watch_tree_root(Some(second.path()));

        std::fs::write(first.path().join("old-root.txt"), b"old\n").unwrap();
        std::fs::write(second.path().join("new-root.txt"), b"new\n").unwrap();

        let pending = wait_for_pending(&mut watch, |pending| {
            pending
                .dirs
                .iter()
                .any(|path| path.ends_with("new-root.txt"))
        });
        assert!(
            pending
                .dirs
                .iter()
                .all(|path| !path.ends_with("old-root.txt")),
            "旧根不得继续上报: {:?}",
            pending.dirs
        );
    }
}
