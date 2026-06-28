//! 文件系统监听模块 — 基于 notify-debouncer-full
//!
//! 职责：
//! - 递归监听项目目录的文件变更
//! - 去抖 300ms 后通过 Tauri Event 广播到前端
//! - 处理 need_rescan（事件队列溢出）通知前端全量刷新
//!
//! 技术栈：notify = "9.0.0-rc.4" + notify-debouncer-full = "0.8.0-rc.2"

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::state::AppState;

/// 发送到前端的文件系统事件载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEventPayload {
    /// 受影响的路径列表
    pub paths: Vec<String>,
    /// 事件类型：Create | Modify | Remove | Rescan | Other
    pub kind: String,
    /// 子类型：File | Folder | Content | Name(From/To/Both) | Metadata | Any
    pub detail: String,
}

/// 可运行时控制的文件系统监听器
///
/// 生命周期：由 AppState 持有，应用关闭时 Drop → debouncer 自动 stop。
pub struct FileWatcher {
    /// 停止事件处理线程的通道
    stop_tx: Option<mpsc::Sender<()>>,
    /// 事件处理线程的 JoinHandle
    thread_handle: Option<std::thread::JoinHandle<()>>,
    /// 当前监听的根路径（被 watcher 线程的 Arc clone 持有，struct 字段仅用于生命周期绑定）
    #[allow(dead_code)]
    watch_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl FileWatcher {
    /// 启动文件系统监听器
    ///
    /// # Arguments
    /// * `app_handle` — Tauri AppHandle，用于 emit 事件
    /// * `watch_paths` — 需要递归监听的路径列表
    /// * `debounce_ms` — 去抖窗口（毫秒），推荐 200-300
    pub fn start(
        app_handle: AppHandle,
        watch_paths: Vec<PathBuf>,
        debounce_ms: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let timeout = Duration::from_millis(debounce_ms);

        // debouncer 内置支持 mpsc::Sender 作为 event handler
        let (event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let mut debouncer = new_debouncer(timeout, None, event_tx)?;

        for path in &watch_paths {
            if path.exists() {
                debouncer
                    .watch(path, RecursiveMode::Recursive)
                    .map_err(|e| format!("无法监听路径 {}: {e}", path.display()))?;
            }
        }

        let watch_paths_arc = Arc::new(Mutex::new(watch_paths));

        let thread_handle = std::thread::Builder::new()
            .name("fs-watcher".into())
            .spawn({
                let app_handle = app_handle.clone();
                let wps = watch_paths_arc.clone();

                move || {
                    // debouncer 存活于本线程，退出时自动 Drop
                    let _debouncer_guard = debouncer;

                    loop {
                        match event_rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(Ok(events)) => {
                                for event in &events {
                                    // need_rescan — 通知前端全量刷新
                                    if event.need_rescan() {
                                        let wps_lock = match wps.lock() {
                                            Ok(g) => g,
                                            Err(e) => {
                                                tracing::error!("fs-watcher 锁获取失败: {e}");
                                                continue;
                                            }
                                        };
                                        let _ = app_handle.emit(
                                            "fs-event",
                                            FsEventPayload {
                                                paths: wps_lock
                                                    .iter()
                                                    .map(|p| p.display().to_string())
                                                    .collect(),
                                                kind: "Rescan".to_string(),
                                                detail: "Overflow".to_string(),
                                            },
                                        );
                                        continue;
                                    }

                                    let payload = classify_event(event);
                                    let _ = app_handle.emit("fs-event", payload);
                                }
                            }
                            Ok(Err(errors)) => {
                                for err in &errors {
                                    tracing::error!("文件系统监听错误: {err}");
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                // 正常超时，检查是否应退出
                                if stop_rx.try_recv().is_ok() {
                                    tracing::info!(
                                        "收到停止信号，文件系统监听器退出"
                                    );
                                    break;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                tracing::info!("事件通道断开，文件系统监听器退出");
                                break;
                            }
                        }
                    }
                }
            })?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            thread_handle: Some(thread_handle),
            watch_paths: watch_paths_arc,
        })
    }

    /// 停止监听器，等待线程退出
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        tracing::info!("文件系统监听器已完全停止");
    }

    /// 检查监听线程是否仍在运行
    #[cfg(test)]
    pub fn is_running(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // 确保线程退出
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// 启动/切换文件系统监听
///
/// 前端在项目打开时调用此命令，传入项目根路径。
/// 若已有 watcher 在运行则先停止旧的，再为新路径启动。
#[tauri::command]
pub fn fs_watch(
    path: String,
    state: tauri::State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let watch_path = PathBuf::from(&path);
    if !watch_path.exists() {
        return Err(AppError::Notify(format!("路径不存在: {path}")));
    }

    // 路径 sandbox 校验（P1-28）
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::Notify(format!("获取 project_root 锁失败: {e}")))?;
        crate::fs::validate_path_within_root(&root, &watch_path)?;
    }

    // P1-14: 先提取旧 watcher（持有锁），释放锁后 stop，再重新获取锁写入新 watcher。
    // 避免 stop() 阻塞期间持有锁导致后续命令级联卡死。
    let old_watcher = {
        let mut watcher_guard = state
            .file_watcher
            .lock()
            .map_err(|e| AppError::Notify(format!("获取 file_watcher 锁失败: {e}")))?;
        watcher_guard.take()
    };
    // 锁已释放

    // 在锁外 stop 旧 watcher
    if let Some(mut old) = old_watcher {
        old.stop();
    }

    let watcher = FileWatcher::start(app_handle, vec![watch_path], 300)
        .map_err(|e| AppError::Notify(format!("启动文件监听失败: {e}")))?;

    // 重新获取锁写入新 watcher
    {
        let mut watcher_guard = state
            .file_watcher
            .lock()
            .map_err(|e| AppError::Notify(format!("获取 file_watcher 锁失败: {e}")))?;
        *watcher_guard = Some(watcher);
    }
    tracing::info!("文件监听已启动: {path}");
    Ok(())
}

/// 将 DebouncedEvent 分类为前端友好的载荷
fn classify_event(event: &notify_debouncer_full::DebouncedEvent) -> FsEventPayload {
    let paths: Vec<String> = event
        .paths
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    let (kind, detail) = match &event.kind {
        EventKind::Create(create_kind) => {
            let d = match create_kind {
                CreateKind::File => "File",
                CreateKind::Folder => "Folder",
                CreateKind::Any => "Any",
                CreateKind::Other => "Other",
            };
            ("Create", d.to_string())
        }
        EventKind::Remove(remove_kind) => {
            let d = match remove_kind {
                RemoveKind::File => "File",
                RemoveKind::Folder => "Folder",
                RemoveKind::Any => "Any",
                RemoveKind::Other => "Other",
            };
            ("Remove", d.to_string())
        }
        EventKind::Modify(modify_kind) => match modify_kind {
            ModifyKind::Data(_) => ("Modify", "Content".to_string()),
            ModifyKind::Metadata(_) => ("Modify", "Metadata".to_string()),
            ModifyKind::Name(rename_mode) => {
                let d = format!("Name({rename_mode:?})");
                ("Modify", d)
            }
            _ => ("Modify", "Other".to_string()),
        },
        EventKind::Access(_) => ("Access", "Any".to_string()),
        EventKind::Other => ("Other", "Meta".to_string()),
        _ => ("Unknown", "Unknown".to_string()),
    };

    FsEventPayload {
        paths,
        kind: kind.to_string(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 classify_event 对不同事件类型的分类正确性
    ///
    /// 注意：无法在单元测试中直接构造 DebouncedEvent（字段不公开），
    /// 因此测试 limited 到 FsEventPayload 序列化和 FileWatcher 生命周期。

    #[test]
    fn fs_event_payload_serializes_camel_case() {
        let payload = FsEventPayload {
            paths: vec!["C:\\test\\file.txt".to_string()],
            kind: "Create".to_string(),
            detail: "File".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("file.txt"));
        assert!(json.contains("Create"));
        // camelCase 验证：detail → 不变，kind → 不变（无下划线字段）
        // rename_all=camelCase 将 snake_case 转为 camelCase，paths/kind/detail 不变
        assert!(json.contains("\"paths\""));
    }

    #[test]
    fn file_watcher_stop_sets_stop_flag() {
        let (stop_tx, _stop_rx) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();

        // 启动一个空转线程模拟 watcher loop
        let handle = std::thread::spawn(move || {
            loop {
                match event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {} // 消费事件
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // 正常超时
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        let mut watcher = FileWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };

        watcher.stop();
        assert!(watcher.stop_tx.is_none(), "stop 后 stop_tx 应被 take");
        assert!(
            !watcher.is_running(),
            "stop 后 is_running 应返回 false"
        );
    }

    #[test]
    fn file_watcher_drop_stops_thread() {
        let (_stop_tx, stop_rx) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();

        let running = Arc::new(Mutex::new(true));
        let running_clone = running.clone();

        let handle = std::thread::spawn(move || {
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match event_rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(_) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let mut r = running_clone.lock().unwrap();
            *r = false;
        });

        let watcher = FileWatcher {
            stop_tx: Some(_stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };

        drop(watcher);
        // 给线程一些时间退出
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            !*running.lock().unwrap(),
            "Drop 后线程应已退出"
        );
    }

    /// 验证 FileWatcher 结构创建（不启动实际监听——单元测试无 AppHandle）
    #[test]
    fn file_watcher_struct_creation() {
        let (stop_tx, _stop_rx) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();

        let handle = std::thread::spawn(move || {
            loop {
                match event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let watcher = FileWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };

        assert!(watcher.is_running());
        drop(watcher);
    }

    /// 验证 watcher 替换模式：停止旧 watcher → 创建新 watcher
    /// 对应 fs_watch 命令的核心逻辑（停止旧→启动新）
    #[test]
    fn file_watcher_replacement_stops_old() {
        // 创建旧 watcher
        let (old_stop_tx, _old_stop_rx) = mpsc::channel::<()>();
        let (_old_event_tx, old_event_rx) = mpsc::channel::<DebounceEventResult>();
        let old_handle = std::thread::spawn(move || {
            loop {
                match old_event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let mut old_watcher = FileWatcher {
            stop_tx: Some(old_stop_tx),
            thread_handle: Some(old_handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };
        assert!(old_watcher.is_running(), "旧 watcher 应运行中");

        // 停止旧 watcher
        old_watcher.stop();
        assert!(!old_watcher.is_running(), "stop 后旧 watcher 应停止");

        // 创建新 watcher（模拟切换项目根路径）
        let (new_stop_tx, _new_stop_rx) = mpsc::channel::<()>();
        let (_new_event_tx, new_event_rx) = mpsc::channel::<DebounceEventResult>();
        let new_handle = std::thread::spawn(move || {
            loop {
                match new_event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let new_watcher = FileWatcher {
            stop_tx: Some(new_stop_tx),
            thread_handle: Some(new_handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };
        assert!(new_watcher.is_running(), "新 watcher 应运行中");
    }

    /// 验证 stop 后再次 stop 不 panic（幂等性）
    #[test]
    fn file_watcher_stop_is_idempotent() {
        let (stop_tx, _stop_rx) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();
        let handle = std::thread::spawn(move || {
            loop {
                match event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let mut watcher = FileWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
        };

        watcher.stop();
        // 第二次 stop 不应 panic（stop_tx 已被 take）
        watcher.stop();
        assert!(!watcher.is_running());
    }
}
