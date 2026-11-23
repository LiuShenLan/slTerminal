//! 文件树根的可见性与复核调度；实际 OS watcher 由既有 FsWatch 持有。

use std::collections::BTreeMap;
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
}

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
        let Some(state) = self
            .roots
            .values_mut()
            .filter(|root| root.visible && path.as_path().starts_with(root.path.as_path()))
            .max_by_key(|root| root.path.as_path().components().count())
        else {
            return;
        };
        let pending = state.pending.get_or_insert(PendingReview {
            first_event: now,
            due: now + REVIEW_DEBOUNCE,
        });
        pending.due = (now + REVIEW_DEBOUNCE).min(pending.first_event + MAX_REVIEW_DELAY);
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
