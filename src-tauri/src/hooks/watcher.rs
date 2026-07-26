//! Hook 信号目录监听器
//!
//! 基于 notify + notify-debouncer-full，监听 ~/.slterminal/hooks-events/ 目录，
//! 检测新增信号文件（.json），调用 process_signal_file 解析后广播 hook-event 并删除文件。
//! debounce 50ms，线程名 "hook-signal-watcher"。

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tauri::AppHandle;

use super::signal::process_signal_file;

/// Hook 信号目录监听器
pub struct HookSignalWatcher {
    /// 停止信号发送端
    stop_tx: Option<mpsc::Sender<()>>,
    /// 监听线程句柄
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl HookSignalWatcher {
    /// 启动监听器
    ///
    /// 监听 `~/.slterminal/hooks-events/` 目录（NonRecursive），
    /// debounce 50ms。目录不存在则自动创建。
    pub fn start(app_handle: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let signal_dir = get_signal_dir()?;
        // 目录不存在则自动创建
        if !signal_dir.exists() {
            std::fs::create_dir_all(&signal_dir)?;
        }

        let timeout = Duration::from_millis(50);
        let (event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let mut debouncer = new_debouncer(timeout, None, event_tx)?;
        debouncer
            .watch(&signal_dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("无法监听信号目录 {}: {e}", signal_dir.display()))?;

        let handle = std::thread::Builder::new()
            .name("hook-signal-watcher".into())
            .spawn(move || {
                // debouncer 存活于本线程，退出时自动 drop → stop
                let _debouncer_guard = debouncer;

                loop {
                    match event_rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(Ok(events)) => {
                            for event in &events {
                                for path in &event.paths {
                                    if is_signal_file(path) {
                                        process_signal_file(&app_handle, path);
                                    }
                                }
                            }
                        }
                        Ok(Err(errors)) => {
                            for e in errors {
                                tracing::warn!("Hook 信号监听器 notify 错误: {e}");
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // 超时——继续循环检查停止信号
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }

                    // 检查停止信号（非阻塞）
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                }
            })?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
        })
    }

    /// 停止监听器（幂等）
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for HookSignalWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 获取信号目录路径（~/.slterminal/hooks-events/）
fn get_signal_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    dirs::home_dir()
        .ok_or_else(|| "无法获取用户 home 目录".into())
        .map(|h| h.join(".slterminal").join("hooks-events"))
}

/// 判断路径是否为 .json 信号文件
fn is_signal_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_signal_file ──

    #[test]
    fn is_signal_file_json() {
        assert!(is_signal_file(std::path::Path::new(
            "/tmp/hooks-events/evt_001.json"
        )));
    }

    #[test]
    fn is_signal_file_json_uppercase() {
        assert!(is_signal_file(std::path::Path::new(
            "/tmp/hooks-events/evt_001.JSON"
        )));
    }

    #[test]
    fn is_signal_file_tmp_rejected() {
        assert!(!is_signal_file(std::path::Path::new(
            "/tmp/hooks-events/evt_001.tmp"
        )));
    }

    #[test]
    fn is_signal_file_no_extension_rejected() {
        assert!(!is_signal_file(std::path::Path::new(
            "/tmp/hooks-events/evt_001"
        )));
    }

    // ── 生命周期 ──

    #[test]
    fn watcher_stop_is_idempotent() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = stop_rx.recv();
        });
        let mut w = HookSignalWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
        };
        w.stop(); // 第一次停止
        w.stop(); // 第二次应不 panic
    }

    #[test]
    fn watcher_drop_stops_thread() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = stop_rx.recv();
        });
        let w = HookSignalWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
        };
        drop(w); // Drop 应 join 线程
    }
}
