//! 流式文件名遍历(本地;WSL find 臂已砍)。
//!
//! 全应用同一时刻只允许一趟搜索遍历(worker 侧的 `SEARCH_WORK` 锁);
//! 这里只负责有界、可取消的深度优先读取。栈里保留目录迭代器而不是所有
//! 待处理后代路径,宽目录(node_modules/build)因此不能把队列撑成无界。

use std::path::Path;

use super::*;

pub(super) const VISIT_LIMIT: usize = 500_000;
const DEPTH_LIMIT: usize = 64;

#[derive(Default)]
pub(super) struct WalkOutcome {
    pub visited: usize,
    pub limited: bool,
    pub error: Option<String>,
}

pub(super) fn local(
    root: &Path,
    cancelled: impl Fn() -> bool,
    mut directory: impl FnMut(&Path),
    mut visit: impl FnMut(IndexedPath),
) -> WalkOutcome {
    let mut outcome = WalkOutcome::default();
    directory(root);
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(error) => {
            outcome.error = Some(error.to_string());
            return outcome;
        }
    };
    let mut stack = vec![read];
    while let Some(read) = stack.last_mut() {
        if cancelled() {
            break;
        }
        let Some(next) = read.next() else {
            stack.pop();
            continue;
        };
        let child = match next {
            Ok(child) => child,
            Err(_) => {
                outcome.limited = true;
                continue;
            }
        };
        if outcome.visited >= VISIT_LIMIT {
            outcome.limited = true;
            break;
        }
        let Ok(kind) = child.file_type() else {
            outcome.limited = true;
            continue;
        };
        if kind.is_symlink() || child.file_name() == ".git" {
            continue;
        }
        let path = child.path();
        if path.as_os_str().as_encoded_bytes().len() > memory::PATH_LIMIT {
            outcome.limited = true;
            continue;
        }
        if let Some(entry) = indexed_local_path(root, &path, kind.is_dir()) {
            outcome.visited += 1;
            visit(entry);
        }
        if kind.is_dir() {
            if stack.len() >= DEPTH_LIMIT {
                outcome.limited = true;
                continue;
            }
            directory(&path);
            match std::fs::read_dir(path) {
                Ok(read) => stack.push(read),
                Err(_) => outcome.limited = true,
            }
        }
    }
    outcome
}
