//! 文件树根的可见性与复核调度；实际 OS watcher 由既有 FsWatch 持有。

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use slterm_path::CanonicalPath;
use slterm_sidebar::FileTreeRoot;

const REVIEW_DEBOUNCE: Duration = Duration::from_millis(200);
const MAX_REVIEW_DELAY: Duration = Duration::from_millis(1_000);

#[derive(Clone, Copy)]
struct PendingReview {
    first_event: Instant,
    due: Instant,
}

struct WatchRoot {
    path: CanonicalPath,
    visible: bool,
    pending: Option<PendingReview>,
    /// 本轮待复核路径；超过上限退回整根复核，避免不可见事件无界累积。
    dirty: BTreeSet<CanonicalPath>,
}

const MAX_PENDING_PATHS: usize = 512;

/// 只维护根作用域与复核时间；调用方把可见根集合交给同一 FsWatch 端口。
pub(crate) struct WatcherService {
    roots: BTreeMap<FileTreeRoot, WatchRoot>,
}

impl WatcherService {
    pub(crate) fn new() -> Self {
        Self {
            roots: BTreeMap::new(),
        }
    }

    pub(crate) fn register(&mut self, root: FileTreeRoot, path: CanonicalPath, visible: bool) {
        self.roots.insert(
            root,
            WatchRoot {
                path,
                visible,
                pending: None,
                dirty: BTreeSet::new(),
            },
        );
    }

    pub(crate) fn unregister(&mut self, root: &FileTreeRoot) {
        self.roots.remove(root);
    }

    pub(crate) fn set_visible(&mut self, root: &FileTreeRoot, visible: bool, now: Instant) {
        let Some(state) = self.roots.get_mut(root) else {
            return;
        };
        if state.visible == visible {
            return;
        }
        state.visible = visible;
        // 隐藏期间的事件不累积；重新可见只做一次整根复核。
        state.dirty.clear();
        state.pending = visible.then_some(PendingReview {
            first_event: now,
            due: now,
        });
    }

    pub(crate) fn visible_roots(&self) -> Vec<CanonicalPath> {
        self.roots
            .values()
            .filter(|root| root.visible)
            .map(|root| root.path.clone())
            .collect()
    }

    pub(crate) fn on_path_event(&mut self, path: &CanonicalPath, now: Instant) {
        let Some((state, root_path)) = self
            .roots
            .iter_mut()
            .filter(|(_, root)| root.visible && path.as_path().starts_with(root.path.as_path()))
            .max_by_key(|(_, root)| root.path.as_path().components().count())
            .map(|(_, state)| {
                let path = state.path.clone();
                (state, path)
            })
        else {
            return;
        };
        if state.dirty.len() < MAX_PENDING_PATHS {
            state.dirty.insert(path.clone());
        } else {
            state.dirty.clear();
            state.dirty.insert(root_path);
        }
        let pending = state.pending.get_or_insert(PendingReview {
            first_event: now,
            due: now + REVIEW_DEBOUNCE,
        });
        pending.due = (now + REVIEW_DEBOUNCE).min(pending.first_event + MAX_REVIEW_DELAY);
    }

    /// 已登记根的当前路径；根替换必须重建待复核状态。
    pub(crate) fn registered_path(&self, root: &FileTreeRoot) -> Option<&CanonicalPath> {
        self.roots.get(root).map(|state| &state.path)
    }

    pub(crate) fn keys(&self) -> Vec<FileTreeRoot> {
        self.roots.keys().cloned().collect()
    }

    /// 复核就绪后取走本轮脏路径；空集表示刷新整根。
    pub(crate) fn take_dirty(&mut self, root: &FileTreeRoot) -> Vec<CanonicalPath> {
        self.roots
            .get_mut(root)
            .map(|state| std::mem::take(&mut state.dirty).into_iter().collect())
            .unwrap_or_default()
    }

    pub(crate) fn poll(&mut self, now: Instant) -> Vec<FileTreeRoot> {
        let mut ready = Vec::new();
        for (id, root) in &mut self.roots {
            if root.visible && root.pending.is_some_and(|pending| pending.due <= now) {
                root.pending = None;
                ready.push(id.clone());
            }
        }
        ready
    }
}
