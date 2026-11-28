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
//! - **前台不等待 OS**：`FsWatchHandle` 只更新最新配置并发送有界唤醒；
//!   专用工作线程执行 notify 创建与 watch/unwatch 的同步 ACK。
//! - 工作线程创建失败返回 `None`；notify 创建失败记录降级，不 panic。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// 一次前台收割的事件路径集：文件脏、目录脏与已配对的文件改名。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FsPending {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) dirs: Vec<PathBuf>,
    pub(crate) renames: Vec<(PathBuf, PathBuf)>,
}

impl FsPending {
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty() && self.dirs.is_empty() && self.renames.is_empty()
    }
}

/// 回调线程与前台共享的分类状态(锁粒度 = 一次事件分类/一次收割)。
#[derive(Debug, Default)]
struct WatchState {
    /// 已登记文件（精确路径过滤；含已关闭 tab 路径时由 retain 清掉）。
    files: HashSet<PathBuf>,
    /// 当前全部项目树根；实际 watch 可由某个祖先根覆盖。
    tree_roots: HashSet<PathBuf>,
    dirty_files: HashSet<PathBuf>,
    dirty_dirs: HashSet<PathBuf>,
    dirty_renames: Vec<(PathBuf, PathBuf)>,
    /// 已产出但尚未完成前台对账的改名目标，允许连续 A→B→C 的 B 作为源。
    rename_targets: HashSet<PathBuf>,
    /// Windows 将一次改名拆成相邻 From/To；只保留已登记文件或改名目标的 From。
    pending_rename_from: Option<PathBuf>,
}

fn lock_state(state: &Mutex<WatchState>) -> std::sync::MutexGuard<'_, WatchState> {
    // 回调线程 panic 不应让前台永久失效:毒锁按内部值继续。
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Clone, Default)]
struct WatchConfig {
    roots: HashSet<PathBuf>,
    files: HashSet<PathBuf>,
}

/// GPUI 前台只持配置与脏集句柄；notify 的同步 ACK 等待全部归工作线程。
pub(crate) struct FsWatchHandle {
    state: Arc<Mutex<WatchState>>,
    wake_rx: Receiver<()>,
    config: Arc<Mutex<WatchConfig>>,
    reconcile_tx: Option<mpsc::SyncSender<()>>,
    #[cfg(test)]
    worker_probe: std::sync::Weak<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl FsWatchHandle {
    pub(crate) fn new() -> Option<Self> {
        let state = Arc::new(Mutex::new(WatchState::default()));
        let config = Arc::new(Mutex::new(WatchConfig::default()));
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let (reconcile_tx, reconcile_rx) = mpsc::sync_channel(1);
        let worker_state = Arc::clone(&state);
        let worker_config = Arc::clone(&config);
        let worker_probe = Arc::new(());
        #[cfg(test)]
        let probe = Arc::downgrade(&worker_probe);
        let worker = std::thread::Builder::new()
            .name("slterm-fs-watch".to_owned())
            .spawn(move || {
                let _probe = worker_probe;
                #[cfg(test)]
                let mut watch = {
                    let (_unused_wake_tx, worker_wake_rx) = mpsc::sync_channel(1);
                    FsWatch::with_state(worker_state, wake_tx, worker_wake_rx)
                };
                #[cfg(not(test))]
                let mut watch = FsWatch::with_state(worker_state, wake_tx);
                while reconcile_rx.recv().is_ok() {
                    let next = worker_config
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    watch.retain_tree_roots(next.roots);
                    watch.retain_files(next.files);
                }
            })
            .ok()?;
        Some(Self {
            state,
            wake_rx,
            config,
            reconcile_tx: Some(reconcile_tx),
            #[cfg(test)]
            worker_probe: probe,
            worker: Some(worker),
        })
    }

    /// 关闭配置通道并 join 工作线程；返回即证明线程已终止。
    ///
    /// 线程只阻塞在 `reconcile_rx.recv()` 或 notify 的有界同步调用上，
    /// 通道关闭是唯一等待来源，不会反向等待前台。
    pub(crate) fn shutdown(&mut self) {
        self.reconcile_tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    /// 配置满量覆盖且有界合并；队列满时既有信号仍会读取到最新值。
    pub(crate) fn retain(&self, roots: Vec<PathBuf>, files: Vec<PathBuf>) {
        let next = WatchConfig {
            roots: roots.into_iter().collect(),
            files: files.into_iter().collect(),
        };
        *self
            .config
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        if let Some(reconcile) = &self.reconcile_tx {
            let _ = reconcile.try_send(());
        }
    }

    pub(crate) fn take_pending(&mut self) -> FsPending {
        while self.wake_rx.try_recv().is_ok() {}
        let mut state = lock_state(&self.state);
        FsPending {
            files: state.dirty_files.drain().collect(),
            dirs: state.dirty_dirs.drain().collect(),
            renames: std::mem::take(&mut state.dirty_renames)
                .into_iter()
                .collect(),
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn has_interest(&self) -> bool {
        let state = lock_state(&self.state);
        !state.files.is_empty() || !state.tree_roots.is_empty()
    }

    /// 测试注入目录事件；真实分类与树根过滤由 FsWatch 单元测试覆盖。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn inject_dir_event_for_test(&self, path: PathBuf) {
        lock_state(&self.state).dirty_dirs.insert(path);
    }

    /// 测试注入文件事件；只进入既有 pending 通道，不伪造 OS watcher。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn inject_file_event_for_test(&self, path: PathBuf) {
        lock_state(&self.state).dirty_files.insert(path);
    }

    #[cfg(test)]
    pub(crate) fn tracks_file(&self, path: &std::path::Path) -> bool {
        lock_state(&self.state).files.contains(path)
    }

    #[cfg(test)]
    pub(crate) fn worker_thread_id(&self) -> std::thread::ThreadId {
        self.worker
            .as_ref()
            .expect("shutdown 后的句柄不再提供线程身份")
            .thread()
            .id()
    }
}

impl Drop for FsWatchHandle {
    fn drop(&mut self) {
        // 所有者销毁是最终 teardown：关闭通道并 join，保证线程与 notify
        // 句柄先于后续前台状态释放。
        self.shutdown();
    }
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
    /// 实际建立的递归树根；嵌套根由祖先覆盖，不重复 watch。
    watched_trees: HashSet<PathBuf>,
    /// 本轮根集合中建立失败的根；根集合变化后才重试。
    failed_tree_roots: HashSet<PathBuf>,
    /// 有界唤醒信号:只用于提示前台"有事件了";容量 1,满即丢。
    #[cfg(test)]
    wake_rx: Receiver<()>,
}

impl FsWatch {
    /// 建立仅供测试的同步 worker；生产统一经 `FsWatchHandle`。
    #[cfg(test)]
    pub(crate) fn new() -> Option<Self> {
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let state = Arc::new(Mutex::new(WatchState::default()));
        let watch = Self::with_state(state, wake_tx, wake_rx);
        watch.watcher.is_some().then_some(watch)
    }

    fn with_state(
        state: Arc<Mutex<WatchState>>,
        wake_tx: mpsc::SyncSender<()>,
        #[cfg(test)] wake_rx: Receiver<()>,
    ) -> Self {
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
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                let mut state = lock_state(&callback_state);
                classify_event(&mut state, event);
                drop(state);
                let _ = wake_tx.try_send(());
            }) {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    log::warn!("fs watch: unavailable ({error}); manual refresh still works");
                    None
                }
            };
        Self {
            watcher,
            state,
            parent_refs: HashMap::new(),
            watched_parents: HashSet::new(),
            watched_trees: HashSet::new(),
            failed_tree_roots: HashSet::new(),
            #[cfg(test)]
            wake_rx,
        }
    }

    /// 用全部项目根替换递归监听集合；嵌套根复用祖先 watch。
    pub(crate) fn retain_tree_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let next: HashSet<PathBuf> = roots.into_iter().collect();
        {
            let mut state = lock_state(&self.state);
            if state.tree_roots == next {
                return;
            }
            // 先切换分类范围，迟到旧根事件不得污染新项目。
            state.tree_roots = next.clone();
        }
        self.failed_tree_roots.clear();

        let stale: Vec<PathBuf> = self
            .watched_trees
            .iter()
            .filter(|watched| !next.iter().any(|root| root.starts_with(watched)))
            .cloned()
            .collect();
        for root in stale {
            if let Some(watcher) = self.watcher.as_mut() {
                let _ = watcher.unwatch(&root);
            }
            self.watched_trees.remove(&root);
        }

        // notify 的 unwatch 按路径生效；安装递归根前先撤掉将被覆盖的父目录。
        let covered: Vec<PathBuf> = self
            .watched_parents
            .iter()
            .filter(|parent| next.iter().any(|root| parent.starts_with(root)))
            .cloned()
            .collect();
        for parent in covered {
            if let Some(watcher) = self.watcher.as_mut() {
                let _ = watcher.unwatch(&parent);
            }
            self.watched_parents.remove(&parent);
        }

        let mut missing: Vec<PathBuf> = next
            .iter()
            .filter(|root| {
                !self
                    .watched_trees
                    .iter()
                    .any(|watched| root.starts_with(watched))
            })
            .cloned()
            .collect();
        missing.sort_by_key(|root| root.components().count());
        for root in missing {
            let Some(watcher) = self.watcher.as_mut() else {
                break;
            };
            match watcher.watch(&root, RecursiveMode::Recursive) {
                Ok(()) => {
                    self.watched_trees.insert(root);
                }
                Err(error) => {
                    self.failed_tree_roots.insert(root.clone());
                    log::warn!(
                        "fs watch: cannot watch tree root {}: {error}",
                        root.display()
                    );
                }
            }
        }

        // 新祖先生效后，既有嵌套 watch 已成为重复监听。
        let redundant: Vec<PathBuf> = self
            .watched_trees
            .iter()
            .filter(|root| {
                self.watched_trees
                    .iter()
                    .any(|other| other != *root && root.starts_with(other))
            })
            .cloned()
            .collect();
        for root in redundant {
            if let Some(watcher) = self.watcher.as_mut() {
                let _ = watcher.unwatch(&root);
            }
            self.watched_trees.remove(&root);
        }
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
            let pending_aliases = state
                .dirty_renames
                .iter()
                .flat_map(|(source, target)| [source.clone(), target.clone()])
                .collect::<HashSet<_>>();
            state
                .rename_targets
                .retain(|path| next.contains(path) || pending_aliases.contains(path));
        }
        self.parent_refs = parent_counts(&next);
        self.reconcile_file_watches();
    }

    /// 收割脏集并清空;顺带排空唤醒信号(信号只是加速提示,不承载数据)。
    #[cfg(test)]
    pub(crate) fn take_pending(&mut self) -> FsPending {
        while self.wake_rx.try_recv().is_ok() {}
        let mut state = lock_state(&self.state);
        FsPending {
            files: state.dirty_files.drain().collect(),
            dirs: state.dirty_dirs.drain().collect(),
            renames: std::mem::take(&mut state.dirty_renames)
                .into_iter()
                .collect(),
        }
    }

    /// 是否仍有监听兴趣（文件或生效的树根）。无兴趣时前台泵应退出。
    #[cfg(test)]
    pub(crate) fn has_interest(&self) -> bool {
        let state = lock_state(&self.state);
        !state.files.is_empty() || !self.watched_trees.is_empty()
    }

    /// 文件父目录 watch 对账：先补缺，再撤销不再需要且未被树根覆盖的。
    fn reconcile_file_watches(&mut self) {
        // 任一实际递归树根覆盖的父目录不单独 watch（含根目录自身）。
        let desired: HashSet<PathBuf> = self
            .parent_refs
            .keys()
            .filter(|parent| {
                !self
                    .watched_trees
                    .iter()
                    .any(|root| parent.starts_with(root))
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

fn record_rename(state: &mut WatchState, source: PathBuf, target: PathBuf) {
    let pair = (source, target.clone());
    if !state.dirty_renames.contains(&pair) {
        state.dirty_renames.push(pair);
    }
    state.rename_targets.insert(target);
}

/// 回调线程只分类当前事件；改名配对也在这里完成，前台不猜测相邻关系。
fn classify_event(state: &mut WatchState, event: notify::Event) {
    let rename_mode = match event.kind {
        EventKind::Modify(ModifyKind::Name(mode)) => Some(mode),
        _ => None,
    };
    let mut paired_source = None;
    match rename_mode {
        Some(RenameMode::Both) => {
            state.pending_rename_from = None;
            if let [source, target] = event.paths.as_slice()
                && (state.files.contains(source) || state.rename_targets.contains(source))
            {
                record_rename(state, source.clone(), target.clone());
                paired_source = Some(source.clone());
            }
        }
        Some(RenameMode::From) => {
            state.pending_rename_from = event
                .paths
                .first()
                .filter(|path| state.files.contains(*path) || state.rename_targets.contains(*path))
                .cloned();
        }
        Some(RenameMode::To) => {
            if let (Some(source), Some(target)) =
                (state.pending_rename_from.take(), event.paths.first())
            {
                record_rename(state, source.clone(), target.clone());
                paired_source = Some(source);
            }
        }
        _ => state.pending_rename_from = None,
    }
    for path in event.paths {
        if state.files.contains(&path) {
            state.dirty_files.insert(path.clone());
        }
        if state.tree_roots.iter().any(|root| path.starts_with(root)) {
            state.dirty_dirs.insert(path);
        }
    }
    // From 的普通文件脏先保留，以覆盖移出监听范围的删除语义；配对后撤销。
    if let Some(source) = paired_source {
        state.dirty_files.remove(&source);
    }
}

/// 文件 → 父目录引用计数；无父目录的裸文件名不参与监听。
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
        watch.retain_tree_roots([directory.path().to_path_buf()]);
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
        watch.retain_tree_roots([directory.path().to_path_buf()]);
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
    fn consecutive_rename_reports_the_intermediate_path_as_a_source() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.txt");
        let intermediate = directory.path().join("intermediate.txt");
        let final_path = directory.path().join("final.md");
        std::fs::write(&first, b"same\n").unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([first.clone()]);

        std::fs::rename(&first, &intermediate).unwrap();
        std::fs::rename(&intermediate, &final_path).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut renames = Vec::new();
        loop {
            renames.extend(watch.take_pending().renames);
            if renames.contains(&(intermediate.clone(), final_path.clone())) {
                break;
            }
            assert!(Instant::now() < deadline, "连续改名第二段不得丢失");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(renames.contains(&(first, intermediate.clone())));
        assert!(renames.contains(&(intermediate, final_path)));
    }

    #[test]
    fn switching_tree_root_moves_the_recursive_watch() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_tree_roots([first.path().to_path_buf()]);
        watch.retain_tree_roots([second.path().to_path_buf()]);

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

    #[test]
    fn registered_file_rename_reports_the_source_target_pair() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.txt");
        let target = directory.path().join("target.md");
        std::fs::write(&source, b"same\n").unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_files([source.clone()]);

        std::fs::rename(&source, &target).unwrap();

        let pending = wait_for_pending(&mut watch, |pending| !pending.renames.is_empty());
        assert_eq!(pending.renames, vec![(source.clone(), target.clone())]);
        assert!(
            !pending.files.contains(&source),
            "已配对改名不得再把旧位置当普通修改上报"
        );
    }

    #[test]
    fn multiple_tree_roots_report_events_without_replacing_each_other() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut watch = FsWatch::new().expect("watcher 应可用");
        watch.retain_tree_roots([first.path().to_path_buf(), second.path().to_path_buf()]);

        std::fs::write(first.path().join("first.txt"), b"first\n").unwrap();
        std::fs::write(second.path().join("second.txt"), b"second\n").unwrap();

        let pending = wait_for_pending(&mut watch, |pending| {
            ["first.txt", "second.txt"]
                .iter()
                .all(|name| pending.dirs.iter().any(|path| path.ends_with(name)))
        });
        assert!(pending.dirs.iter().any(|path| path.ends_with("first.txt")));
        assert!(pending.dirs.iter().any(|path| path.ends_with("second.txt")));
    }

    #[test]
    fn handle_applies_watch_registration_on_its_worker_thread() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tracked.txt");
        std::fs::write(&path, b"one\n").unwrap();
        let mut watch = FsWatchHandle::new().expect("watcher 工作线程应可用");
        assert_ne!(watch.worker_thread_id(), std::thread::current().id());

        watch.retain(vec![directory.path().to_path_buf()], vec![path.clone()]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !watch.tracks_file(&path) {
            assert!(Instant::now() < deadline, "工作线程未应用文件兴趣");
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(&path, b"changed\n").unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let pending = watch.take_pending();
            if pending.files.contains(&path) {
                break;
            }
            assert!(Instant::now() < deadline, "工作线程 watcher 未上报事件");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn shutdown_joins_the_worker_before_returning() {
        let mut watch = FsWatchHandle::new().expect("watcher 工作线程应可用");
        let probe = watch.worker_probe.clone();
        let deadline = Instant::now() + Duration::from_secs(5);
        while probe.strong_count() == 0 {
            assert!(Instant::now() < deadline, "工作线程未启动");
            std::thread::sleep(Duration::from_millis(10));
        }

        watch.shutdown();

        assert_eq!(probe.strong_count(), 0, "shutdown 返回时工作线程必须已终止");
    }

    #[test]
    fn dropping_handle_joins_the_worker_before_returning() {
        let watch = FsWatchHandle::new().expect("watcher 工作线程应可用");
        let probe = watch.worker_probe.clone();
        let deadline = Instant::now() + Duration::from_secs(5);
        while probe.strong_count() == 0 {
            assert!(Instant::now() < deadline, "工作线程未启动");
            std::thread::sleep(Duration::from_millis(10));
        }

        drop(watch);

        assert_eq!(probe.strong_count(), 0, "Drop 返回时工作线程必须已终止");
    }
}
