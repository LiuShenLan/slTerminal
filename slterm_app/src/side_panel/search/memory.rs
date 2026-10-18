//! 进程级保留内存预算:缓存与已发布结果各一套。
//!
//! 租约跟随分配属主,包括已经交给视图的结果——替换 query 不会给退役
//! 任务再发一份预算。

use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) const CACHE_LIMIT: usize = 6 * 1024 * 1024;
pub(super) const MATCH_LIMIT: usize = 1024 * 1024;
pub(super) const RESULT_LIMIT: usize = 512 * 1024;
pub(super) const PATH_LIMIT: usize = 64 * 1024;

static CACHES: AtomicUsize = AtomicUsize::new(0);
static RESULTS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub(crate) struct FileSearchMemory {
    counter: &'static AtomicUsize,
    limit: usize,
    bytes: usize,
}

impl FileSearchMemory {
    pub(super) fn cache() -> Self {
        Self {
            counter: &CACHES,
            limit: 8 * 1024 * 1024,
            bytes: 0,
        }
    }

    pub(super) fn results() -> Self {
        Self {
            counter: &RESULTS,
            limit: 2 * 1024 * 1024,
            bytes: 0,
        }
    }

    pub(super) fn grow(&mut self, bytes: usize) -> bool {
        if self
            .counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|next| *next <= self.limit)
            })
            .is_err()
        {
            return false;
        }
        self.bytes += bytes;
        true
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn release(&mut self, bytes: usize) {
        assert!(bytes <= self.bytes);
        self.counter.fetch_sub(bytes, Ordering::AcqRel);
        self.bytes -= bytes;
    }
}

impl Drop for FileSearchMemory {
    fn drop(&mut self) {
        self.release(self.bytes);
    }
}
