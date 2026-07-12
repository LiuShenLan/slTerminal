//! 文件系统监听模块 — 基于 notify-debouncer-full
//!
//! 职责：
//! - 递归监听项目目录的文件变更
//! - 去抖 300ms 后通过 Tauri Event 广播到前端
//! - 处理 need_rescan（事件队列溢出）通知前端全量刷新
//!
//! 技术栈：notify = "9.0.0-rc.4" + notify-debouncer-full = "0.8.0-rc.2"

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// 暂停标记：true 时 watcher 线程收到事件不上报前端
    paused: Arc<AtomicBool>,
}

// 池模块（定义在 pool.rs，此处引用）
pub mod pool;

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
        let paused = Arc::new(AtomicBool::new(false));

        let thread_handle = std::thread::Builder::new()
            .name("fs-watcher".into())
            .spawn({
                let app_handle = app_handle.clone();
                let wps = watch_paths_arc.clone();
                let paused_clone = paused.clone();

                move || {
                    // debouncer 存活于本线程，退出时自动 Drop
                    let _debouncer_guard = debouncer;

                    loop {
                        match event_rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(Ok(events)) => {
                                // 暂停状态：跳过事件上报（watcher 仍运行，OS 句柄保留）
                                if paused_clone.load(Ordering::Relaxed) {
                                    continue;
                                }
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
                                    break;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
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
            paused,
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
    }

    /// 暂停事件上报（watcher 线程继续运行，保留 OS 句柄）
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    /// 恢复事件上报
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    /// 查询是否处于暂停状态
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
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
        // 发送停止信号并等待线程退出（确保 OS 句柄释放）
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// 启动/切换文件系统监听（watcher 池模式）
///
/// 前端在项目切换时调用。内部维护最多 5 个 watcher 的 LRU 缓存池：
/// - 命中缓存：pause 其他 watcher，resume 目标，不重建
/// - 未命中：暂停现有 watcher，新建并插入池（超限时淘汰 LRU）
///
/// 此模式避免每次切换都 `stop()` + `start()`（Windows 上 `ReadDirectoryChangesW`
/// 递归注册 26K 文件的 target/ 目录需约 2 秒）。
#[tauri::command]
pub fn fs_watch(
    path: String,
    state: tauri::State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let watch_path = dunce::simplified(std::path::Path::new(&path)).to_path_buf();
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

    let mut pool = state
        .file_watchers
        .lock()
        .map_err(|e| AppError::Notify(format!("获取 file_watchers 锁失败: {e}")))?;

    // 暂停其他 watcher，恢复/激活目标
    pool.pause_all_except(&watch_path);

    // 命中缓存：直接返回
    if pool.get(&watch_path).is_some() {
        return Ok(());
    }

    // 未命中：创建新 watcher 并插入池（超限时自动淘汰 LRU）
    let watcher = FileWatcher::start(app_handle, vec![watch_path.clone()], 300)
        .map_err(|e| AppError::Notify(format!("启动文件监听失败: {e}")))?;

    pool.insert(watch_path, watcher);
    Ok(())
}

/// 将 DebouncedEvent 分类为前端友好的载荷
fn classify_event(event: &notify_debouncer_full::DebouncedEvent) -> FsEventPayload {
    let paths: Vec<String> = event
        .paths
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    classify_by_kind(&event.kind, paths)
}

/// 根据 EventKind 和路径生成前端事件载荷（纯函数，可单测）
fn classify_by_kind(kind: &notify::EventKind, paths: Vec<String>) -> FsEventPayload {
    let (kind_str, detail) = match kind {
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
        kind: kind_str.to_string(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;
    use notify::event::{CreateKind, ModifyKind, RemoveKind, AccessKind, RenameMode, DataChange, AccessMode};

    // ─── classify_by_kind 测试（覆盖全部 7 种 EventKind 变体） ───

    fn paths() -> Vec<String> {
        vec!["/test/file.txt".to_string()]
    }

    #[test]
    fn classify_create_file() {
        let p = classify_by_kind(&EventKind::Create(CreateKind::File), paths());
        assert_eq!(p.kind, "Create");
        assert_eq!(p.detail, "File");
    }

    #[test]
    fn classify_create_folder() {
        let p = classify_by_kind(&EventKind::Create(CreateKind::Folder), paths());
        assert_eq!(p.kind, "Create");
        assert_eq!(p.detail, "Folder");
    }

    #[test]
    fn classify_create_any() {
        let p = classify_by_kind(&EventKind::Create(CreateKind::Any), paths());
        assert_eq!(p.kind, "Create");
        assert_eq!(p.detail, "Any");
    }

    #[test]
    fn classify_create_other() {
        let p = classify_by_kind(&EventKind::Create(CreateKind::Other), paths());
        assert_eq!(p.kind, "Create");
        assert_eq!(p.detail, "Other");
    }

    #[test]
    fn classify_remove_file() {
        let p = classify_by_kind(&EventKind::Remove(RemoveKind::File), paths());
        assert_eq!(p.kind, "Remove");
        assert_eq!(p.detail, "File");
    }

    #[test]
    fn classify_remove_folder() {
        let p = classify_by_kind(&EventKind::Remove(RemoveKind::Folder), paths());
        assert_eq!(p.kind, "Remove");
        assert_eq!(p.detail, "Folder");
    }

    #[test]
    fn classify_remove_any() {
        let p = classify_by_kind(&EventKind::Remove(RemoveKind::Any), paths());
        assert_eq!(p.kind, "Remove");
        assert_eq!(p.detail, "Any");
    }

    #[test]
    fn classify_remove_other() {
        let p = classify_by_kind(&EventKind::Remove(RemoveKind::Other), paths());
        assert_eq!(p.kind, "Remove");
        assert_eq!(p.detail, "Other");
    }

    #[test]
    fn classify_modify_data() {
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert_eq!(p.detail, "Content");
    }

    #[test]
    fn classify_modify_metadata() {
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any)),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert_eq!(p.detail, "Metadata");
    }

    #[test]
    fn classify_modify_name() {
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert!(p.detail.contains("Name"), "Name 变体 detail 应包含 Name 前缀");
        assert!(p.detail.contains("From"), "应包含 RenameMode::From");
    }

    #[test]
    fn classify_modify_name_to() {
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert!(p.detail.contains("To"));
    }

    #[test]
    fn classify_modify_name_both() {
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert!(p.detail.contains("Both"));
    }

    #[test]
    fn classify_modify_other() {
        // ModifyKind::Other 变体 hit 通配 _ 分支
        let p = classify_by_kind(
            &EventKind::Modify(ModifyKind::Other),
            paths(),
        );
        assert_eq!(p.kind, "Modify");
        assert_eq!(p.detail, "Other");
    }

    #[test]
    fn classify_access() {
        let p = classify_by_kind(&EventKind::Access(AccessKind::Close(AccessMode::Any)), paths());
        assert_eq!(p.kind, "Access");
        assert_eq!(p.detail, "Any");
    }

    #[test]
    fn classify_other() {
        let p = classify_by_kind(&EventKind::Other, paths());
        assert_eq!(p.kind, "Other");
        assert_eq!(p.detail, "Meta");
    }

    #[test]
    fn classify_any() {
        // EventKind::Any hit 通配 _ 分支
        let p = classify_by_kind(&EventKind::Any, paths());
        assert_eq!(p.kind, "Unknown");
        assert_eq!(p.detail, "Unknown");
    }

    #[test]
    fn classify_preserves_paths() {
        let paths = vec!["/a/b.txt".to_string(), "/c/d.txt".to_string()];
        let p = classify_by_kind(&EventKind::Create(CreateKind::File), paths.clone());
        assert_eq!(p.paths, paths);
    }

    // ─── FileWatcher 生命周期测试 ───

    /// 验证 FsEventPayload 序列化为 camelCase
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
            paused: Arc::new(AtomicBool::new(false)),
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
            paused: Arc::new(AtomicBool::new(false)),
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
            paused: Arc::new(AtomicBool::new(false)),
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
            paused: Arc::new(AtomicBool::new(false)),
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
            paused: Arc::new(AtomicBool::new(false)),
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
            paused: Arc::new(AtomicBool::new(false)),
        };

        watcher.stop();
        // 第二次 stop 不应 panic（stop_tx 已被 take）
        watcher.stop();
        assert!(!watcher.is_running());
    }
}
