//! 线程 join 超时共享件（BE-01，自 pty/reader.rs 上提，app_dir/home 顶层共享件同形态）
//! ——全部线程退出点统一带超时 join，禁止裸 join 无界阻塞（CP-011 口径扩展：
//! 生产+测试全域，守卫见 src-tauri/src/CLAUDE.md）。

use std::time::Duration;

/// 线程退出 join 超时——3s 后判定超时，调用方自行决策后续（detach / 移交监督线程）
/// BE-01: 自 pty/reader.rs 上提（随上提去 KILL_ 前缀，语义不变），pty/notify/hooks 共用
pub(crate) const JOIN_TIMEOUT: Duration = Duration::from_secs(3);

/// join 超时轮询间隔（10ms，轻量轮询，避免忙等）
pub(crate) const JOIN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// 带超时的线程 join——轮询 `is_finished` 至 deadline，避免无限期阻塞
///
/// 返回 false = 超时未完成（调用方按自身清理计划决策：
/// reader detach + session 移交监督线程，CP-011）。
/// 轮询到 is_finished 后调用 join() 回收线程资源（立即返回）。
/// 纯逻辑 + 标准库线程，可 L1 单测（不依赖 PTY）。
pub(crate) fn join_with_timeout(handle: std::thread::JoinHandle<()>, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if handle.is_finished() {
            let _ = handle.join();
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(JOIN_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod join_tests {
    use super::*;

    #[test]
    fn join_with_timeout_finished_handle_returns_true() {
        // 立即结束的线程：超时前完成 join，返回 true 且快速（<1s）
        let start = std::time::Instant::now();
        let handle = std::thread::spawn(|| {});
        assert!(join_with_timeout(handle, Duration::from_millis(200)));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "已结束线程的 join 应立即返回，实际耗时 {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn join_with_timeout_blocked_thread_returns_false() {
        // park 的线程 + 短超时（50ms）→ 返回 false（调用方按 CP-011 决策:
        // detach reader + 监督线程 drop）
        let handle = std::thread::spawn(|| std::thread::park());
        assert!(!join_with_timeout(handle, Duration::from_millis(50)));
    }

    #[test]
    fn join_with_timeout_abandoned_thread_finishes_later_no_panic() {
        // 超时放弃 join 后，线程自行结束不 panic（JoinHandle drop 时 detach）
        let handle = std::thread::spawn(|| std::thread::sleep(Duration::from_millis(100)));
        assert!(!join_with_timeout(handle, Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(150));
    }
}
