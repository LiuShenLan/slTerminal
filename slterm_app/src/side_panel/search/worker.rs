//! 搜索属主:最新请求信箱、取消与热缓存复用。
//!
//! 单 worker 串行处理全部窗口请求;排队窗口只保留最后一条 query,
//! 不为每次键入生成一个任务。

use std::path::PathBuf;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use super::cache::FileCache;
use super::matching::{BestMatches, QueryMatcher};
use super::*;

// 同一时刻只跑一趟 CPU/I/O 搜索;视图从不等待这把锁。
static SEARCH_WORK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default)]
struct DesiredSearch {
    root: Option<PathBuf>,
    query: Option<SearchRequest>,
    epoch: u64,
    refresh: u64,
}

struct SearchState {
    desired: Mutex<DesiredSearch>,
    wake: Condvar,
    revision: AtomicU64,
    processed: AtomicU64,
    stopped: AtomicBool,
    result: Mutex<Option<FileSearchResult>>,
    status: AtomicU8,
    truncated: AtomicBool,
    #[cfg(test)]
    scans: AtomicUsize,
    #[cfg(test)]
    work_deferrals: AtomicUsize,
    #[cfg(test)]
    retained_bytes: AtomicUsize,
}

pub(crate) struct EmbeddedFileIndex {
    state: Arc<SearchState>,
}

impl EmbeddedFileIndex {
    pub(crate) fn new() -> Self {
        let state = Arc::new(SearchState {
            desired: Mutex::new(DesiredSearch::default()),
            wake: Condvar::new(),
            revision: AtomicU64::new(1),
            processed: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            result: Mutex::new(None),
            status: AtomicU8::new(INDEX_IDLE),
            truncated: AtomicBool::new(false),
            #[cfg(test)]
            scans: AtomicUsize::new(0),
            #[cfg(test)]
            work_deferrals: AtomicUsize::new(0),
            #[cfg(test)]
            retained_bytes: AtomicUsize::new(0),
        });
        let worker = state.clone();
        let _ = std::thread::Builder::new()
            .name("slterm file search".to_owned())
            .spawn(move || run_search_worker(worker));
        Self { state }
    }

    fn update(&self, edit: impl FnOnce(&mut DesiredSearch)) -> u64 {
        let mut desired = self
            .state
            .desired
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        edit(&mut desired);
        *self
            .state
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        self.state.status.store(
            if desired.query.is_some() {
                INDEX_BUILDING
            } else {
                INDEX_IDLE
            },
            Ordering::Release,
        );
        self.state.truncated.store(false, Ordering::Release);
        let revision = self.state.revision.fetch_add(1, Ordering::AcqRel) + 1;
        self.state.wake.notify_all();
        revision
    }

    pub(crate) fn rebuild(
        &self,
        root: Option<PathBuf>,
        epoch: u64,
        query: Option<(u64, String, FileSearchOptions)>,
    ) {
        self.update(|desired| {
            desired.root = root;
            desired.epoch = epoch;
            desired.query = query.map(|(generation, query, options)| SearchRequest {
                epoch,
                generation,
                query,
                options,
            });
        });
    }

    pub(crate) fn query(
        &self,
        epoch: u64,
        generation: u64,
        query: String,
        options: FileSearchOptions,
    ) {
        self.update(|desired| {
            desired.query = Some(SearchRequest {
                epoch,
                generation,
                query,
                options,
            });
        });
    }

    pub(crate) fn clear_query(&self) {
        self.update(|desired| desired.query = None);
    }

    pub(crate) fn refresh(&self, epoch: u64) {
        self.update(|desired| {
            if desired.epoch == epoch {
                desired.refresh = desired.refresh.wrapping_add(1);
            }
        });
    }

    pub(crate) fn take_result(&self) -> Option<FileSearchResult> {
        self.state.result.lock().ok()?.take()
    }

    pub(crate) fn status(&self) -> FileIndexStatus {
        match self.state.status.load(Ordering::Acquire) {
            INDEX_BUILDING => FileIndexStatus::Building,
            INDEX_READY => FileIndexStatus::Ready,
            _ => FileIndexStatus::Idle,
        }
    }

    pub(crate) fn truncated(&self) -> bool {
        self.state.truncated.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn scans(&self) -> usize {
        self.state.scans.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.state.retained_bytes.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn release_for_test(&self) {
        let revision = self.update(|desired| *desired = DesiredSearch::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.state.processed.load(Ordering::Acquire) < revision {
            assert!(
                Instant::now() < deadline,
                "search worker did not release its resources"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for EmbeddedFileIndex {
    fn drop(&mut self) {
        self.state.stopped.store(true, Ordering::Release);
        self.state.revision.fetch_add(1, Ordering::AcqRel);
        self.state.wake.notify_all();
    }
}

fn wait_for_work(state: &SearchState, duration: Duration) {
    let guard = state
        .desired
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _ = state.wake.wait_timeout(guard, duration);
}

fn run_search_worker(state: Arc<SearchState>) {
    let mut cache = FileCache::new();
    let mut root = None;
    let mut epoch = 0;
    let mut refresh = 0;
    loop {
        if state.stopped.load(Ordering::Acquire) {
            return;
        }
        let (desired, revision) = {
            let desired = state
                .desired
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (desired.clone(), state.revision.load(Ordering::Acquire))
        };
        let changed = root != desired.root || epoch != desired.epoch || refresh != desired.refresh;
        let dirty = cache.watches.dirty.swap(false, Ordering::AcqRel);
        if changed || dirty {
            // watch 事件可以让已经发布过的查询失效;若防抖/共享工作锁把
            // 本趟推迟,结果必须保持 pending。
            state.processed.store(0, Ordering::Release);
            cache = FileCache::new();
            root = desired.root.clone();
            epoch = desired.epoch;
            refresh = desired.refresh;
        } else if revision == state.processed.load(Ordering::Acquire) {
            wait_for_work(&state, WATCH_DEBOUNCE);
            continue;
        }
        let cancelled = || {
            state.stopped.load(Ordering::Acquire)
                || state.revision.load(Ordering::Acquire) != revision
        };
        let Some(request) = desired.query.as_ref().filter(|query| query.epoch == epoch) else {
            state.status.store(INDEX_IDLE, Ordering::Release);
            #[cfg(test)]
            state
                .retained_bytes
                .store(cache.memory.bytes(), Ordering::Release);
            state.processed.store(revision, Ordering::Release);
            state.wake.notify_all();
            continue;
        };
        wait_for_work(&state, SEARCH_DEBOUNCE);
        if cancelled() {
            continue;
        }
        let _work = match SEARCH_WORK.try_lock() {
            Ok(work) => work,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                #[cfg(test)]
                state.work_deferrals.fetch_add(1, Ordering::Release);
                wait_for_work(&state, WATCH_DEBOUNCE);
                continue;
            }
        };
        let matcher = match QueryMatcher::compile(&request.query, request.options) {
            Ok(matcher) => matcher,
            Err(error) => {
                publish(
                    &state,
                    revision,
                    request,
                    &BestMatches::new(request),
                    Some(error),
                    false,
                    true,
                );
                continue;
            }
        };
        let mut best = BestMatches::new(request);
        for entry in &cache.entries {
            if cancelled() {
                break;
            }
            if matcher.matches(entry) {
                best.add(entry.clone());
            }
        }
        let reusable = cache.complete
            && (!cache.watches.unwatched || cache.finished.elapsed() < Duration::from_secs(2));
        if reusable {
            publish(&state, revision, request, &best, None, false, true);
            continue;
        }
        if !cache.entries.is_empty() {
            publish(&state, revision, request, &best, None, true, false);
        }
        best = BestMatches::new(request);
        let mut position = 0;
        let mut last_publish = Instant::now();
        // 每个目录枚举前先挂非递归 watch,与条目存储解耦:两层递归回调
        // 都能改自己的属主,不必等扫描结束后再补 watch。
        let mut watches = std::mem::replace(&mut cache.watches, cache::DirectoryWatches::new());
        #[cfg(test)]
        state.scans.fetch_add(1, Ordering::AcqRel);
        let mut visit = |entry: IndexedPath| {
            cache.record(position, &entry);
            position += 1;
            if matcher.matches(&entry) {
                best.add(entry);
            }
            if last_publish.elapsed() >= Duration::from_millis(100) {
                publish(&state, revision, request, &best, None, true, false);
                last_publish = Instant::now();
            }
        };
        let outcome = match root.as_ref() {
            Some(root) => walk::local(
                root,
                cancelled,
                |directory| {
                    watches.watch(directory);
                },
                &mut visit,
            ),
            None => walk::WalkOutcome::default(),
        };
        cache.watches = watches;
        if !cancelled() && !outcome.limited && outcome.error.is_none() {
            cache.finish_prefix(position);
        }
        cache.complete = !cancelled() && !outcome.limited && outcome.error.is_none() && !cache.full;
        cache.finished = Instant::now();
        #[cfg(test)]
        state
            .retained_bytes
            .store(cache.memory.bytes(), Ordering::Release);
        publish(
            &state,
            revision,
            request,
            &best,
            outcome.error,
            outcome.limited,
            true,
        );
    }
}

fn publish(
    state: &SearchState,
    revision: u64,
    request: &SearchRequest,
    best: &BestMatches,
    error: Option<String>,
    limited: bool,
    complete: bool,
) {
    let (mut result, result_limited) = best.result(request);
    result.error = error;
    result.complete = complete;
    let desired = state
        .desired
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if state.revision.load(Ordering::Acquire) != revision || state.stopped.load(Ordering::Acquire) {
        return;
    }
    *state
        .result
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(result);
    state
        .truncated
        .store(limited || result_limited || best.limited, Ordering::Release);
    state.status.store(
        if complete {
            INDEX_READY
        } else {
            INDEX_BUILDING
        },
        Ordering::Release,
    );
    if complete {
        state.processed.store(revision, Ordering::Release);
    }
    drop(desired);
    state.wake.notify_all();
}
