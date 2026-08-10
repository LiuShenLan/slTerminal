//! Hook 信号目录监听器
//!
//! 监听 ~/.slterminal/hooks-events/ 目录，检测新增信号文件（.json），
//! 调用 process_signal_file 解析后广播 hook-event 并删除文件。
//!
//! 双通道架构（win10 实证修复——notify 事件丢失/目录重建句柄失效导致 33 个残留）：
//! - notify 实时通道：50ms debounce，初始化/监听失败仅降级 warn（不致命）
//! - 轮询补漏通道：每 3s 扫描目录处理残留 .json，幂等（处理后删除），
//!   目录被删除（卸载 hooks 的 remove_dir_all 等）后自动重建——彻底免疫
//!   notify 事件丢失/目录删除重建/启动失败，积压残留被补送恢复前端状态
//!
//! 两通道同线程串行执行，无并发竞态。
//!
//! 线程名 "hook-signal-watcher"。

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};
use tauri::AppHandle;

use super::signal::process_signal_file;

/// 轮询补漏间隔（notify 实时通道失效/事件丢失时兜底）
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// 线程循环每圈等待时长（降级路径无 recv 等待时的节奏控制）
const LOOP_TICK: Duration = Duration::from_millis(250);

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
    /// debounce 50ms + 3s 轮询补漏。目录不存在则自动创建。
    /// notify 初始化/监听失败仅 warn 降级（轮询仍工作），不再返回 Err。
    pub fn start(app_handle: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let signal_dir = get_signal_dir()?;
        // 目录不存在则自动创建
        if !signal_dir.exists() {
            std::fs::create_dir_all(&signal_dir)?;
        }

        let timeout = Duration::from_millis(50);
        let (event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        // notify 实时通道：初始化/监听失败降级为 None（轮询补漏兜底，不致命）
        let mut debouncer = match new_debouncer(timeout, None, event_tx) {
            Ok(d) => Some(d),
            Err(e) => {
                tracing::warn!("创建 Hook 信号 debouncer 失败（轮询补漏兜底）: {e}");
                None
            }
        };
        if let Some(d) = debouncer.as_mut() {
            if let Err(e) = d.watch(&signal_dir, RecursiveMode::NonRecursive) {
                tracing::warn!(
                    "监听信号目录失败 {e}（轮询补漏兜底）: {}",
                    signal_dir.display()
                );
                debouncer = None;
            }
        }
        let has_notify = debouncer.is_some();

        let handle = std::thread::Builder::new()
            .name("hook-signal-watcher".into())
            .spawn(move || {
                // debouncer 存活于本线程，退出时自动 drop → stop
                let _debouncer_guard = debouncer;
                let mut last_poll = Instant::now();

                loop {
                    // 1. notify 实时事件（降级时无发送者，跳过 recv 走 sleep 节奏）
                    if has_notify {
                        match event_rx.recv_timeout(LOOP_TICK) {
                            Ok(Ok(events)) => {
                                handle_notify_events(&events, |path| {
                                    process_signal_file(&app_handle, path);
                                });
                            }
                            Ok(Err(errors)) => {
                                for e in errors {
                                    tracing::warn!("Hook 信号监听器 notify 错误: {e}");
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                // 超时——继续循环检查轮询/停止信号
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                // debouncer 意外 drop（_debouncer_guard 持有，理论不可达）——退出
                                break;
                            }
                        }
                    } else {
                        std::thread::sleep(LOOP_TICK);
                    }

                    // 2. 轮询补漏（每 POLL_INTERVAL）+ 停止信号检查：
                    //    run_one_tick = 目录重建 → 消费残留 → 停止则退出（返回 true）
                    if last_poll.elapsed() >= POLL_INTERVAL {
                        if run_one_tick(&signal_dir, &stop_rx, |path| {
                            process_signal_file(&app_handle, path);
                        }) {
                            break;
                        }
                        last_poll = Instant::now();
                    } else if stop_rx.try_recv().is_ok() {
                        // 3. 检查停止信号（非阻塞）
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

/// 收集信号目录中的残留信号文件（.json，大小写不敏感）。
/// 目录不存在/不可读 → 空 Vec（不报错，轮询下次再试）。
pub fn collect_signal_files(dir: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_signal_file(p))
        .collect()
}

/// 单次轮询：处理目录中全部残留信号文件（处理函数注入便于测试）。
/// 幂等：process 处理后会删除文件，二次 poll 不再处理（process_signal_file 语义）。
pub fn poll_once(dir: &Path, process: impl Fn(&Path)) {
    for path in collect_signal_files(dir) {
        process(&path);
    }
}

/// 消费单批 notify 实时事件：仅 .json 信号文件交给 process。
/// 与事件循环的 notify 分支对齐（D6 抽取，供测试复用）。
fn handle_notify_events(events: &[DebouncedEvent], process: impl Fn(&Path)) {
    for event in events {
        for path in &event.paths {
            if is_signal_file(path) {
                process(path);
            }
        }
    }
}

/// 单次轮询补漏 tick（事件循环第 2 段，D6 抽取）：
/// 目录被删除（卸载 hooks 的 remove_dir_all 等）后自动重建 → poll_once 消费残留 →
/// 检查停止信号。返回 true 表示收到停止信号、应退出循环。
fn run_one_tick(signal_dir: &Path, stop_rx: &mpsc::Receiver<()>, process: impl Fn(&Path)) -> bool {
    // 目录被删除后自动重建——免疫 notify 事件丢失/目录删除重建（win10 实证兜底）
    if let Err(e) = std::fs::create_dir_all(signal_dir) {
        tracing::warn!("重建信号目录失败 {}: {e}", signal_dir.display());
    }
    poll_once(signal_dir, process);
    // 检查停止信号（非阻塞）
    stop_rx.try_recv().is_ok()
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

    // ── collect_signal_files ──

    #[test]
    fn collect_gets_all_json_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        std::fs::write(dir.path().join("b.json"), "{}").unwrap();
        let files = collect_signal_files(dir.path());
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|p| p.extension().unwrap() == "json"));
    }

    #[test]
    fn collect_excludes_tmp_and_no_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.tmp"), "").unwrap();
        std::fs::write(dir.path().join("b"), "").unwrap();
        std::fs::write(dir.path().join("c.JSON"), "{}").unwrap(); // 大写仍收集
        let files = collect_signal_files(dir.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap(), "c.JSON");
    }

    #[test]
    fn collect_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_signal_files(dir.path()).is_empty());
    }

    #[test]
    fn collect_missing_dir_returns_empty() {
        let base = tempfile::tempdir().unwrap();
        let missing = base.path().join("nonexistent-sub");
        assert!(collect_signal_files(&missing).is_empty());
    }

    // ── poll_once ──

    #[test]
    fn poll_once_processes_each_residual() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        std::fs::write(dir.path().join("b.json"), "{}").unwrap();
        let processed = std::sync::Mutex::new(Vec::new());
        poll_once(dir.path(), |p| {
            processed
                .lock()
                .unwrap()
                .push(p.file_name().unwrap().to_string_lossy().to_string());
        });
        let got = processed.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&"a.json".to_string()));
        assert!(got.contains(&"b.json".to_string()));
    }

    #[test]
    fn poll_once_idempotent_after_process_deletes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        let count = std::sync::atomic::AtomicUsize::new(0);
        // process 语义 = 处理后删除（process_signal_file 行为）
        let process = |p: &Path| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = std::fs::remove_file(p);
        };
        poll_once(dir.path(), &process);
        poll_once(dir.path(), &process);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn poll_once_rebuilds_and_processes_after_dir_deleted() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("signals");
        // 目录不存在 → collect 空，process 零调用（无 panic）
        poll_once(&dir, |_| panic!("目录不存在时不应处理任何文件"));
        // 重建目录（模拟线程循环 create_dir_all）→ 写入 → 处理
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), "{}").unwrap();
        let count = std::sync::atomic::AtomicUsize::new(0);
        poll_once(&dir, |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn poll_once_ignores_non_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.tmp"), "").unwrap();
        std::fs::write(dir.path().join("b"), "").unwrap();
        let count = std::sync::atomic::AtomicUsize::new(0);
        poll_once(dir.path(), |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn poll_once_no_files_no_process() {
        let dir = tempfile::tempdir().unwrap();
        let count = std::sync::atomic::AtomicUsize::new(0);
        poll_once(dir.path(), |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
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
        w.stop(); // 第一次停止：发送停止信号并 join 线程
        assert!(
            w.thread_handle.is_none(),
            "stop 后线程句柄应被取走（线程已 join）"
        );
        w.stop(); // 第二次应不 panic（幂等）
    }

    #[test]
    fn watcher_stop_signal_terminates_thread() {
        // 停止信号 → 线程应真实结束（thread.is_finished 断言，防 stop 假象）
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = stop_rx.recv();
        });
        assert!(!handle.is_finished(), "线程运行中不应已结束");
        let _ = stop_tx.send(());
        // 轮询等待线程结束（带上限，防 CI 挂死）
        for _ in 0..100 {
            if handle.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(handle.is_finished(), "发送停止信号后线程应结束");
        let _ = handle.join();
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

    // ── run_one_tick（轮询补漏 tick——HUK-03） ──

    #[test]
    fn run_one_tick_polls_residual_files() {
        // 轮询补漏消费残留文件：目录存在 + 残留 .json → 全部交给 process
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        std::fs::write(dir.path().join("b.json"), "{}").unwrap();
        let (_stop_tx, stop_rx) = mpsc::channel();
        let processed = std::sync::Mutex::new(Vec::new());
        let stopped = run_one_tick(dir.path(), &stop_rx, |p| {
            processed
                .lock()
                .unwrap()
                .push(p.file_name().unwrap().to_string_lossy().to_string());
        });
        assert!(!stopped, "无停止信号时不应退出");
        let got = processed.lock().unwrap();
        assert_eq!(got.len(), 2, "残留 .json 应全部被消费");
        assert!(got.contains(&"a.json".to_string()));
        assert!(got.contains(&"b.json".to_string()));
    }

    #[test]
    fn run_one_tick_rebuilds_deleted_dir() {
        // 目录被删除（卸载 hooks 的 remove_dir_all 等）后：tick 自动重建 + 消费残留
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("signals");
        let (_stop_tx, stop_rx) = mpsc::channel();
        // 目录不存在 → tick 重建（collect 空、零处理、不 panic）
        let stopped = run_one_tick(&dir, &stop_rx, |_| {});
        assert!(!stopped);
        assert!(dir.exists(), "tick 应自动重建被删除的目录");
        // 重建后写入残留 → 下一次 tick 消费
        std::fs::write(dir.join("residual.json"), "{}").unwrap();
        let count = std::sync::atomic::AtomicUsize::new(0);
        run_one_tick(&dir, &stop_rx, |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "重建后的残留应被消费"
        );
    }

    #[test]
    fn run_one_tick_stop_signal_returns_true() {
        // 收到停止信号 → tick 返回 true（事件循环据此退出）
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        let (stop_tx, stop_rx) = mpsc::channel();
        let _ = stop_tx.send(());
        let count = std::sync::atomic::AtomicUsize::new(0);
        let stopped = run_one_tick(dir.path(), &stop_rx, |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(stopped, "收到停止信号后 tick 应返回 true");
        // 与事件循环顺序一致：先消费残留再查停止信号
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    // ── handle_notify_events（notify 实时通道过滤——HUK-03） ──

    #[test]
    fn handle_notify_events_filters_json_only() {
        // notify 事件中仅 .json 信号文件交给 process（.tmp/无扩展名不处理）
        use notify::EventKind;
        let json_path = std::path::PathBuf::from("/tmp/hooks-events/evt.json");
        let tmp_path = std::path::PathBuf::from("/tmp/hooks-events/evt.tmp");
        let events = vec![
            DebouncedEvent::new(
                notify::Event::new(EventKind::Create(notify::event::CreateKind::File))
                    .add_path(json_path.clone()),
                Instant::now(),
            ),
            DebouncedEvent::new(
                notify::Event::new(EventKind::Create(notify::event::CreateKind::File))
                    .add_path(tmp_path.clone()),
                Instant::now(),
            ),
        ];
        let processed = std::sync::Mutex::new(Vec::new());
        handle_notify_events(&events, |p| {
            processed.lock().unwrap().push(p.to_path_buf());
        });
        let got = processed.lock().unwrap();
        assert_eq!(got.len(), 1, "仅 .json 信号文件应交给 process");
        assert_eq!(got[0], json_path);
    }
}
