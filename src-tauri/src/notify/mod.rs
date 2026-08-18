//! 文件系统监听模块 — 基于 notify-debouncer-full
//!
//! 职责：
//! - 递归监听项目目录的文件变更
//! - 去抖 300ms 后通过 Tauri Event 广播到前端
//! - 处理 need_rescan（事件队列溢出）通知前端全量刷新
//!
//! 技术栈：notify = "9.0.0-rc.4" + notify-debouncer-full = "0.8.0-rc.2"

use std::path::{Path, PathBuf};
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

/// BE-02：watcher 事件侧排除目录（D8 定稿七元素，仅事件侧过滤）
///
/// notify 不支持目录级排除（watcher 仍注册全树），排除在事件侧完成：
/// 事件路径任一分量命中以下目录即丢弃该事件，防大仓库
/// （node_modules/target 等）事件风暴。fs_read_dir 不动（懒加载既定决策）。
pub const WATCH_EXCLUDE_DIRS: [&str; 7] = [
    "node_modules",
    "target",
    ".venv",
    "venv",
    "dist",
    ".git",
    "__pycache__",
];

/// BE-07: fs-event 单事件路径合并上限——去抖批内单事件 paths 数**严格超限**时
/// 合并为 Rescan 变体下发（不再逐路径推送）。防巨型批次（大目录整体删除/
/// rename 等一次性产生数千路径）打爆前端事件循环与 IPC。
pub const FS_EVENT_PATH_BATCH_LIMIT: usize = 100;

// BE-07 评估结论：agent-event（hooks/signal.rs）低频不节流——hook 触发才 emit
// （常态每会话每事件一次，量级远小于 fs-event 的每秒级目录变更），节流会延误
// 前端状态机响应（F3 四态状态指示依赖实时事件）。维持不节流，不改 signal.rs；
// 评估结论登记在 S19（文档同步）。

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

/// 文件系统事件发射抽象（D6 抽离：隔离 AppHandle，使事件循环可 L1 测试）
pub trait EventEmitter: Send + Sync + 'static {
    /// 向前端广播文件系统事件载荷
    fn emit_fs_event(&self, payload: FsEventPayload);
}

/// 生产 EventEmitter：包装 Tauri AppHandle 的 emit（发送失败静默忽略）
pub struct AppHandleEmitter {
    app_handle: AppHandle,
}

impl EventEmitter for AppHandleEmitter {
    fn emit_fs_event(&self, payload: FsEventPayload) {
        let _ = self.app_handle.emit("fs-event", payload);
    }
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
        Self::start_with_emitter(
            Box::new(AppHandleEmitter { app_handle }),
            watch_paths,
            debounce_ms,
        )
    }

    /// 注入 EventEmitter 的启动入口（D2 参数注入：L1 用 mock emitter 驱动，无需 AppHandle）
    pub fn start_with_emitter(
        emitter: Box<dyn EventEmitter>,
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
                let wps = watch_paths_arc.clone();
                let paused_clone = paused.clone();

                move || {
                    // debouncer 存活于本线程，退出时自动 Drop
                    let _debouncer_guard = debouncer;
                    event_loop(&event_rx, &stop_rx, &paused_clone, &wps, emitter.as_ref());
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

/// BE-02：事件路径是否命中排除目录（任一分量匹配即排除）
///
/// 用 `components()` 按整分量比较，避免子串误伤（如 `mytarget` 不匹配 `target`）。
fn is_excluded_path(path: &Path) -> bool {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|seg| WATCH_EXCLUDE_DIRS.contains(&seg))
}

/// 下发 Rescan 载荷（need_rescan 溢出 / BE-07 批量合并共用）——携带监听根路径
fn emit_rescan_overflow(emitter: &dyn EventEmitter, wps: &Mutex<Vec<PathBuf>>) {
    match wps.lock() {
        Ok(guard) => emitter.emit_fs_event(FsEventPayload {
            paths: guard.iter().map(|p| p.display().to_string()).collect(),
            kind: "Rescan".to_string(),
            detail: "Overflow".to_string(),
        }),
        Err(e) => tracing::error!("fs-watcher 锁获取失败: {e}"),
    }
}

/// SEC-08：事件路径（或其任一祖先分量）是否为符号链接
///
/// 用 `symlink_metadata` 检查（不跟随末级符号链接）。事件路径可能位于符号链接
/// 目录内部（祖先为 symlink 时外部路径会经 fs-event 泄露），故逐祖先检查。
/// 元数据读取失败（如删除竞态——路径已不存在）视为非符号链接，不丢正常事件。
fn is_symlink_path(path: &Path) -> bool {
    path.ancestors()
        .filter_map(|ancestor| std::fs::symlink_metadata(ancestor).ok())
        .any(|m| m.file_type().is_symlink())
}

/// watcher 事件循环（D6 抽离为独立函数：事件/暂停/停止全部经参数驱动，emit 经 trait 注入，
/// 使 L1 可用 mock emitter + channel 直接驱动，无需 AppHandle）
fn event_loop(
    event_rx: &mpsc::Receiver<DebounceEventResult>,
    stop_rx: &mpsc::Receiver<()>,
    paused: &AtomicBool,
    wps: &Mutex<Vec<PathBuf>>,
    emitter: &dyn EventEmitter,
) {
    loop {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(events)) => {
                // 暂停状态：跳过事件上报（watcher 仍运行，OS 句柄保留）
                if paused.load(Ordering::Relaxed) {
                    continue;
                }
                for event in &events {
                    // need_rescan — 通知前端全量刷新
                    if event.need_rescan() {
                        emit_rescan_overflow(emitter, wps);
                        continue;
                    }

                    // BE-02：事件路径任一分量命中排除目录 → 丢弃（大目录事件风暴防护）
                    if event.paths.iter().any(|p| is_excluded_path(p)) {
                        continue;
                    }
                    // SEC-08：符号链接路径不 emit（防项目内 symlink 泄露外部路径）
                    if event.paths.iter().any(|p| is_symlink_path(p)) {
                        continue;
                    }

                    // BE-07: 单事件路径数严格超限 → 合并为 Rescan（全量刷新比
                    // 逐路径推送更省——前端 fs-event 消费侧本就整体刷新）
                    if event.paths.len() > FS_EVENT_PATH_BATCH_LIMIT {
                        emit_rescan_overflow(emitter, wps);
                        continue;
                    }

                    let payload = classify_event(event);
                    emitter.emit_fs_event(payload);
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
/// 前端在项目切换时调用。内部维护最多 WATCHER_POOL_CAPACITY（8）个 watcher 的 LRU 缓存池：
/// - 命中缓存：pause 其他 watcher，resume 目标，不重建
/// - 未命中：暂停现有 watcher，新建并插入池（超限时淘汰 LRU）
///
/// 此模式避免每次切换都 `stop()` + `start()`（Windows 上 `ReadDirectoryChangesW`
/// 递归注册 26K 文件的 target/ 目录需约 2 秒）。
/// notify_watch 路径前置校验（D2 抽离，供命令与 L1 共用）：存在性 + 路径沙箱（P1-28）
fn validate_watch_path(watch_path: &Path, project_root: &Option<PathBuf>) -> Result<(), AppError> {
    if !watch_path.exists() {
        return Err(AppError::Notify(format!(
            "路径不存在: {}",
            watch_path.display()
        )));
    }
    crate::state::validate_path_within_root(project_root, watch_path)
}

/// notify_watch 阶段 1（持池锁调用）：暂停其他 watcher、恢复/激活目标，返回是否命中缓存
fn notify_watch_phase1(pool: &mut pool::LruWatcherPool, watch_path: &Path) -> bool {
    // 暂停其他 watcher，恢复/激活目标
    pool.pause_all_except(watch_path);

    // 命中缓存：直接返回
    pool.get(watch_path).is_some()
}

/// notify_watch 阶段 3（持池锁调用）：竞态检查 + 插入。
/// 竞态命中（另一线程已在阶段 2 期间插入同路径 watcher）时丢弃传入 watcher
/// （drop → FileWatcher::drop 发送停止信号并 join 线程），返回 Ok。
fn notify_watch_phase3(
    pool: &mut pool::LruWatcherPool,
    watch_path: &Path,
    watcher: FileWatcher,
) -> Result<(), AppError> {
    // 竞态检查：若另一线程已在阶段 2 期间插入同路径 watcher，丢弃当前 watcher
    if pool.get(watch_path).is_some() {
        drop(watcher);
        return Ok(());
    }

    // 未命中：插入新 watcher（超限时自动淘汰 LRU）
    pool.insert(watch_path.to_path_buf(), watcher);
    Ok(())
}

#[tauri::command]
pub async fn notify_watch(
    path: String,
    state: tauri::State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let watch_path = dunce::simplified(std::path::Path::new(&path)).to_path_buf();

    // 路径前置校验（存在性 + 沙箱），短暂持有 project_root 锁
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::Notify(format!("获取 project_root 锁失败: {e}")))?;
        validate_watch_path(&watch_path, &root)?;
    }

    // 阶段 1：持池锁 → pause_all_except + 缓存检查
    {
        let mut pool = state
            .file_watchers
            .lock()
            .map_err(|e| AppError::Notify(format!("获取 file_watchers 锁失败: {e}")))?;
        if notify_watch_phase1(&mut pool, &watch_path) {
            return Ok(());
        }
    } // 池锁在此释放

    // 阶段 2：锁外创建 watcher（含 debouncer.watch 阻塞调用，避免持锁阻塞其他线程）
    // BE-04: 改 spawn_blocking——Windows 上递归注册大目录耗时约 2s，不阻塞 IPC worker
    let watch_path_for_spawn = watch_path.clone();
    let watcher = match tokio::task::spawn_blocking(move || -> Result<FileWatcher, AppError> {
        FileWatcher::start_with_emitter(
            Box::new(AppHandleEmitter { app_handle }),
            vec![watch_path_for_spawn],
            300,
        )
        .map_err(|e| AppError::Notify(format!("启动文件监听失败: {e}")))
    })
    .await
    {
        Ok(inner) => inner?,
        Err(e) => return Err(AppError::TaskJoin(e.to_string())),
    };

    // 阶段 3：短暂持锁插入池（处理可能的竞态——另一线程可能已为同一路径创建 watcher）
    {
        let mut pool = state
            .file_watchers
            .lock()
            .map_err(|e| AppError::Notify(format!("获取 file_watchers 锁失败: {e}")))?;
        notify_watch_phase3(&mut pool, &watch_path, watcher)?;
    }
    Ok(())
}

/// notify_stop_watch — 停止并移除指定路径的 watcher（BE-10）
///
/// 前端在项目移除/切换时调用，避免旧 watcher 占用 OS 句柄直至 LRU 淘汰。
/// 路径不在池中时静默返回 Ok（幂等，无副作用）。路径 key 与 notify_watch
/// 相同（dunce::simplified 规范化）。
#[tauri::command]
pub async fn notify_stop_watch(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    let watch_path = dunce::simplified(std::path::Path::new(&path)).to_path_buf();
    let mut pool = state
        .file_watchers
        .lock()
        .map_err(|e| AppError::Notify(format!("获取 file_watchers 锁失败: {e}")))?;
    pool.remove(&watch_path);
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
    use notify::event::{
        AccessKind, AccessMode, CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode,
    };
    use notify::EventKind;

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
        assert!(
            p.detail.contains("Name"),
            "Name 变体 detail 应包含 Name 前缀"
        );
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
        let p = classify_by_kind(&EventKind::Modify(ModifyKind::Other), paths());
        assert_eq!(p.kind, "Modify");
        assert_eq!(p.detail, "Other");
    }

    #[test]
    fn classify_access() {
        let p = classify_by_kind(
            &EventKind::Access(AccessKind::Close(AccessMode::Any)),
            paths(),
        );
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

    // ─── BE-02 排除目录过滤 ───

    #[test]
    fn is_excluded_path_matches_all_seven_dirs() {
        // 契约七元素逐一验证：任一分量命中即排除
        for dir in WATCH_EXCLUDE_DIRS {
            let p = PathBuf::from(format!("C:/project/{dir}/sub/file.txt"));
            assert!(is_excluded_path(&p), "分量 {dir} 应命中排除");
        }
        // 整分量比较：子串不误伤
        assert!(
            !is_excluded_path(&PathBuf::from("C:/project/mytarget/file.txt")),
            "mytarget 不应命中 target"
        );
        assert!(
            !is_excluded_path(&PathBuf::from("C:/project/target-backup/x.txt")),
            "target-backup 不应命中 target"
        );
        // 普通路径不排除
        assert!(!is_excluded_path(&PathBuf::from("C:/project/src/main.rs")));
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
        assert!(!watcher.is_running(), "stop 后 is_running 应返回 false");
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
        // Drop 内部已 join 线程；此处轮询线程退出标志（2s 超时）兜底断言，替代固定 sleep 消除慢 CI 抖动（HFN-07）
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if !*running.lock().unwrap() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Drop 后线程应在 2s 内退出"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 验证 FileWatcher 结构创建（不启动实际监听——单元测试无 AppHandle）
    #[test]
    fn file_watcher_struct_creation() {
        let (stop_tx, _stop_rx) = mpsc::channel::<()>();
        let (_event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();

        let handle = std::thread::spawn(move || loop {
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(_) => {}
                Err(_) => break,
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
    /// 对应 notify_watch 命令的核心逻辑（停止旧→启动新）
    #[test]
    fn file_watcher_replacement_stops_old() {
        // 创建旧 watcher
        let (old_stop_tx, _old_stop_rx) = mpsc::channel::<()>();
        let (_old_event_tx, old_event_rx) = mpsc::channel::<DebounceEventResult>();
        let old_handle = std::thread::spawn(move || loop {
            match old_event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(_) => {}
                Err(_) => break,
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
        let new_handle = std::thread::spawn(move || loop {
            match new_event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(_) => {}
                Err(_) => break,
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
        let handle = std::thread::spawn(move || loop {
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(_) => {}
                Err(_) => break,
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

    // ─── 事件循环测试（HFN-03：mock emitter 驱动，无需 AppHandle） ───

    /// 记录全部 emit 载荷的 mock emitter
    #[derive(Default)]
    struct MockEmitter {
        emitted: Mutex<Vec<FsEventPayload>>,
    }

    impl EventEmitter for MockEmitter {
        fn emit_fs_event(&self, payload: FsEventPayload) {
            self.emitted.lock().unwrap().push(payload);
        }
    }

    /// Arc 包装也实现 trait，便于 Box<Arc<MockEmitter>> 传入 start_with_emitter
    impl EventEmitter for Arc<MockEmitter> {
        fn emit_fs_event(&self, payload: FsEventPayload) {
            self.emitted.lock().unwrap().push(payload);
        }
    }

    impl MockEmitter {
        fn count(&self) -> usize {
            self.emitted.lock().unwrap().len()
        }

        fn last(&self) -> Option<FsEventPayload> {
            self.emitted.lock().unwrap().last().cloned()
        }
    }

    /// 构造 DebouncedEvent（notify::Event → DebouncedEvent::new）
    fn make_debounced(
        kind: EventKind,
        paths: Vec<PathBuf>,
    ) -> notify_debouncer_full::DebouncedEvent {
        notify_debouncer_full::DebouncedEvent::new(
            notify::Event {
                kind,
                paths,
                attrs: notify::event::EventAttributes::new(),
            },
            std::time::Instant::now(),
        )
    }

    /// 构造 need_rescan 事件（Flag::Rescan）
    fn make_rescan_debounced() -> notify_debouncer_full::DebouncedEvent {
        let mut attrs = notify::event::EventAttributes::new();
        attrs.set_flag(notify::event::Flag::Rescan);
        notify_debouncer_full::DebouncedEvent::new(
            notify::Event {
                kind: EventKind::Other,
                paths: vec![],
                attrs,
            },
            std::time::Instant::now(),
        )
    }

    /// 事件循环测试句柄：真实线程跑 event_loop，测试经 channel 驱动
    struct LoopHarness {
        event_tx: mpsc::Sender<DebounceEventResult>,
        stop_tx: mpsc::Sender<()>,
        paused: Arc<AtomicBool>,
        wps: Arc<Mutex<Vec<PathBuf>>>,
        handle: std::thread::JoinHandle<()>,
    }

    impl LoopHarness {
        fn start(emitter: Arc<MockEmitter>) -> Self {
            let (event_tx, event_rx) = mpsc::channel::<DebounceEventResult>();
            let (stop_tx, stop_rx) = mpsc::channel::<()>();
            let paused = Arc::new(AtomicBool::new(false));
            let wps = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
            let handle = std::thread::spawn({
                let paused_clone = paused.clone();
                let wps_clone = wps.clone();
                move || {
                    event_loop(
                        &event_rx,
                        &stop_rx,
                        &paused_clone,
                        &wps_clone,
                        emitter.as_ref(),
                    );
                }
            });
            Self {
                event_tx,
                stop_tx,
                paused,
                wps,
                handle,
            }
        }

        /// 关闭发送端并等待循环退出（循环因 Disconnected 分支退出）
        fn shutdown(self) {
            drop(self.event_tx);
            self.handle.join().unwrap();
        }
    }

    /// 轮询等待条件成立（2s 超时），消除固定 sleep 抖动
    fn wait_until(cond: impl Fn() -> bool, msg: &str) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if cond() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(cond(), "{msg}");
    }

    #[test]
    fn event_loop_emits_classified_payload() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Create(CreateKind::File),
                vec![PathBuf::from("/tmp/a.txt")],
            )]))
            .unwrap();

        wait_until(|| emitter.count() == 1, "事件循环应 emit 分类后的载荷");
        let p = emitter.last().unwrap();
        assert_eq!(p.kind, "Create");
        assert_eq!(p.detail, "File");
        assert_eq!(p.paths, vec!["/tmp/a.txt".to_string()]);

        harness.shutdown();
    }

    #[test]
    fn event_loop_rescan_emits_overflow_payload_with_watch_paths() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());
        harness
            .wps
            .lock()
            .unwrap()
            .push(PathBuf::from("/project/root"));

        harness
            .event_tx
            .send(Ok(vec![make_rescan_debounced()]))
            .unwrap();

        wait_until(
            || emitter.count() == 1,
            "need_rescan 事件应触发 Rescan 载荷",
        );
        let p = emitter.last().unwrap();
        assert_eq!(p.kind, "Rescan");
        assert_eq!(p.detail, "Overflow");
        assert_eq!(
            p.paths,
            vec!["/project/root".to_string()],
            "Rescan 载荷应携带监听根路径"
        );

        harness.shutdown();
    }

    #[test]
    fn event_loop_filters_excluded_paths_keeps_normal() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        // 含 node_modules 分量的路径 → 事件被过滤；正常路径 → 仍 emit
        harness
            .event_tx
            .send(Ok(vec![
                make_debounced(
                    EventKind::Create(CreateKind::File),
                    vec![PathBuf::from("/project/node_modules/pkg/x.js")],
                ),
                make_debounced(
                    EventKind::Create(CreateKind::File),
                    vec![PathBuf::from("/project/src/main.rs")],
                ),
            ]))
            .unwrap();

        wait_until(
            || emitter.count() == 1,
            "排除路径事件应被过滤，正常路径仍 emit",
        );
        assert_eq!(
            emitter.last().unwrap().paths,
            vec!["/project/src/main.rs".to_string()]
        );

        harness.shutdown();
    }

    // ─── BE-07: 单事件路径合并上限测试 ───

    /// 单事件 paths 数严格超限 → 合并为单条 Rescan 下发（携带监听根路径，不再逐路径）
    #[test]
    fn event_loop_merges_oversized_batch_to_rescan() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());
        harness
            .wps
            .lock()
            .unwrap()
            .push(PathBuf::from("/project/root"));

        // 超限批：LIMIT + 1 个路径的单事件
        let paths: Vec<PathBuf> = (0..=FS_EVENT_PATH_BATCH_LIMIT)
            .map(|i| PathBuf::from(format!("/project/src/file_{i}.rs")))
            .collect();
        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Create(CreateKind::File),
                paths,
            )]))
            .unwrap();

        wait_until(|| emitter.count() == 1, "超限批应合并为单条 Rescan");
        let p = emitter.last().unwrap();
        assert_eq!(p.kind, "Rescan");
        assert_eq!(p.detail, "Overflow");
        assert_eq!(
            p.paths,
            vec!["/project/root".to_string()],
            "合并 Rescan 应携带监听根路径"
        );

        harness.shutdown();
    }

    /// 恰好达到限制值（阈值语义：严格大于才合并）→ 正常分类下发
    #[test]
    fn event_loop_batch_at_limit_stays_classified() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        // 恰好 LIMIT 个路径 → 仍按原分类逐路径下发
        let paths: Vec<PathBuf> = (0..FS_EVENT_PATH_BATCH_LIMIT)
            .map(|i| PathBuf::from(format!("/project/src/file_{i}.rs")))
            .collect();
        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Create(CreateKind::File),
                paths,
            )]))
            .unwrap();

        wait_until(|| emitter.count() == 1, "限制值内批应正常分类下发");
        let p = emitter.last().unwrap();
        assert_eq!(p.kind, "Create", "限制值内不应合并为 Rescan");
        assert_eq!(p.detail, "File");
        assert_eq!(p.paths.len(), FS_EVENT_PATH_BATCH_LIMIT);

        harness.shutdown();
    }

    #[test]
    fn event_loop_rescan_bypasses_exclusion_filter() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());
        harness
            .wps
            .lock()
            .unwrap()
            .push(PathBuf::from("/project/root"));

        // 同一批：排除路径事件 + need_rescan——rescan 分支在排除过滤之前，不受影响
        harness
            .event_tx
            .send(Ok(vec![
                make_debounced(
                    EventKind::Create(CreateKind::File),
                    vec![PathBuf::from("/project/node_modules/x.js")],
                ),
                make_rescan_debounced(),
            ]))
            .unwrap();

        wait_until(
            || emitter.count() == 1,
            "need_rescan 事件应绕过排除过滤正常 emit",
        );
        let p = emitter.last().unwrap();
        assert_eq!(p.kind, "Rescan", "排除过滤不应拦截 need_rescan");

        harness.shutdown();
    }

    /// 创建目录符号链接（跨平台 helper）
    ///
    /// Windows 上创建符号链接需管理员权限或开发者模式——失败由调用方 skip 处理。
    fn try_create_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(target, link)
        }
        #[cfg(not(windows))]
        {
            std::os::unix::fs::symlink(target, link)
        }
    }

    #[test]
    fn is_symlink_path_detects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        if let Err(e) = try_create_dir_symlink(&target, &link) {
            // Windows 需管理员/developer mode——创建失败则跳过（BE-17 豁免约定）
            eprintln!("跳过 symlink 测试：创建符号链接失败（需管理员/开发者模式）: {e}");
            return;
        }

        assert!(is_symlink_path(&link), "符号链接路径应被检出");
        assert!(!is_symlink_path(&target), "真实目录不应被误判");
        // 符号链接目录内部的路径（祖先为 symlink）也应被检出——防外部路径经 fs-event 泄露
        assert!(
            is_symlink_path(&link.join("nested.txt")),
            "祖先为 symlink 的路径应被检出"
        );
        // 不存在路径：symlink_metadata 失败 → 视为非符号链接，不丢事件
        assert!(!is_symlink_path(&dir.path().join("not-exist.txt")));
    }

    #[test]
    fn event_loop_filters_symlink_paths_keeps_normal() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        if let Err(e) = try_create_dir_symlink(&target, &link) {
            // Windows 需管理员/developer mode——创建失败则跳过（BE-17 豁免约定）
            eprintln!("跳过 symlink 测试：创建符号链接失败（需管理员/开发者模式）: {e}");
            return;
        }

        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        // symlink 路径 → 不 emit；正常路径 → 仍 emit
        harness
            .event_tx
            .send(Ok(vec![
                make_debounced(EventKind::Create(CreateKind::File), vec![link]),
                make_debounced(
                    EventKind::Create(CreateKind::File),
                    vec![dir.path().join("normal.txt")],
                ),
            ]))
            .unwrap();

        wait_until(
            || emitter.count() == 1,
            "symlink 路径事件应被过滤，正常路径仍 emit",
        );
        assert_eq!(
            emitter.last().unwrap().paths,
            vec![dir.path().join("normal.txt").display().to_string()]
        );

        harness.shutdown();
    }

    #[test]
    fn event_loop_paused_skips_emit_resume_recovers() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        // 暂停期间：事件被丢弃，零 emit（负向断言窗口 300ms）
        harness.paused.store(true, Ordering::SeqCst);
        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Create(CreateKind::File),
                vec![PathBuf::from("/tmp/a.txt")],
            )]))
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_millis(300);
        while std::time::Instant::now() < deadline {
            assert_eq!(emitter.count(), 0, "暂停期间不应上报事件");
            std::thread::sleep(Duration::from_millis(10));
        }

        // 恢复后：后续事件正常上报
        harness.paused.store(false, Ordering::SeqCst);
        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Create(CreateKind::File),
                vec![PathBuf::from("/tmp/b.txt")],
            )]))
            .unwrap();
        wait_until(|| emitter.count() == 1, "恢复后应上报事件");
        assert_eq!(
            emitter.last().unwrap().paths,
            vec!["/tmp/b.txt".to_string()]
        );

        harness.shutdown();
    }

    #[test]
    fn event_loop_errors_logged_loop_continues() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        // 错误只记日志不中断循环：后续正常事件仍 emit
        harness
            .event_tx
            .send(Err(vec![notify::Error::generic("test error")]))
            .unwrap();
        harness
            .event_tx
            .send(Ok(vec![make_debounced(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                vec![PathBuf::from("/tmp/c.txt")],
            )]))
            .unwrap();
        wait_until(|| emitter.count() == 1, "错误事件后循环应继续处理正常事件");

        harness.shutdown();
    }

    #[test]
    fn event_loop_stop_signal_exits() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());

        harness.stop_tx.send(()).unwrap();
        wait_until(
            || harness.handle.is_finished(),
            "收到停止信号后循环线程应退出",
        );
        harness.shutdown();
    }

    #[test]
    fn event_loop_sender_disconnected_exits() {
        let emitter = Arc::new(MockEmitter::default());
        let harness = LoopHarness::start(emitter.clone());
        // 仅关闭发送端（不发送停止信号）：循环应因 Disconnected 分支退出
        harness.shutdown();
    }

    /// 真实 debouncer + 真实目录（仅 mock emitter）：覆盖 debouncer 创建、watch 注册、事件循环全链路
    #[test]
    fn start_with_emitter_real_dir_emits_on_change() {
        let dir = tempfile::tempdir().unwrap();
        let emitter = Arc::new(MockEmitter::default());

        let mut watcher = FileWatcher::start_with_emitter(
            Box::new(emitter.clone()),
            vec![dir.path().to_path_buf()],
            50,
        )
        .expect("start_with_emitter 应成功（真实 debouncer + 空目录）");

        // 写文件触发 notify → 去抖 50ms → 事件循环 emit
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while emitter.count() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "真实目录变更应在 5s 内触发 fs-event"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(watcher.is_running(), "watcher 应保持运行");

        watcher.stop();
        assert!(!watcher.is_running(), "stop 后 watcher 线程应退出");
    }

    // ─── notify_watch 路径校验与池交互（HFN-03 补测） ───

    /// 创建测试用 FileWatcher（线程真实监听 stop_rx，退出时置 exit 标志）
    fn make_test_watcher_with_exit(name: &str, exit: Option<Arc<AtomicBool>>) -> FileWatcher {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let handle = std::thread::Builder::new()
            .name(format!("test-watcher-{name}"))
            .spawn(move || {
                loop {
                    match stop_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
                if let Some(flag) = exit {
                    flag.store(true, Ordering::SeqCst);
                }
            })
            .unwrap();
        FileWatcher {
            stop_tx: Some(stop_tx),
            thread_handle: Some(handle),
            watch_paths: Arc::new(Mutex::new(vec![])),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn validate_watch_path_nonexistent_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not-exist");
        let err = validate_watch_path(&missing, &Some(dir.path().to_path_buf())).unwrap_err();
        assert!(
            format!("{err}").contains("路径不存在"),
            "不存在路径应报错，实际: {err}"
        );
    }

    #[test]
    fn validate_watch_path_outside_root_rejected() {
        let root_dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let err =
            validate_watch_path(outside.path(), &Some(root_dir.path().to_path_buf())).unwrap_err();
        assert!(
            format!("{err}").contains("超出项目范围"),
            "根外路径应被沙箱拒绝，实际: {err}"
        );
    }

    #[test]
    fn validate_watch_path_inside_root_ok() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        validate_watch_path(&sub, &Some(dir.path().to_path_buf())).unwrap();
    }

    #[test]
    fn notify_watch_phase1_hit_resumes_target_pauses_others() {
        let mut pool = pool::LruWatcherPool::new(pool::WATCHER_POOL_CAPACITY);
        let a = PathBuf::from("/test/a");
        let b = PathBuf::from("/test/b");
        pool.insert(a.clone(), make_test_watcher_with_exit("a", None));
        pool.insert(b.clone(), make_test_watcher_with_exit("b", None));
        pool.pause_all_except(&b);
        assert!(pool.get(&a).unwrap().is_paused(), "前置：a 应已暂停");

        assert!(notify_watch_phase1(&mut pool, &a), "命中缓存应返回 true");
        assert!(!pool.get(&a).unwrap().is_paused(), "命中目标应被 resume");
        assert!(pool.get(&b).unwrap().is_paused(), "其他 watcher 应被 pause");
    }

    #[test]
    fn notify_watch_phase1_miss_returns_false() {
        let mut pool = pool::LruWatcherPool::new(pool::WATCHER_POOL_CAPACITY);
        pool.insert(
            PathBuf::from("/test/a"),
            make_test_watcher_with_exit("a", None),
        );
        assert!(
            !notify_watch_phase1(&mut pool, &PathBuf::from("/test/other")),
            "未命中应返回 false"
        );
    }

    #[test]
    fn notify_watch_phase3_inserts_watcher() {
        let mut pool = pool::LruWatcherPool::new(pool::WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/a");
        notify_watch_phase3(&mut pool, &path, make_test_watcher_with_exit("a", None)).unwrap();
        assert!(pool.contains(&path), "新 watcher 应插入池");
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn notify_watch_phase3_race_drops_incoming_watcher() {
        let mut pool = pool::LruWatcherPool::new(pool::WATCHER_POOL_CAPACITY);
        let path = PathBuf::from("/test/a");
        pool.insert(path.clone(), make_test_watcher_with_exit("existing", None));

        // 竞态：池中已存在同路径 watcher，传入 watcher 应被丢弃（drop 自动 stop）
        let incoming_exited = Arc::new(AtomicBool::new(false));
        notify_watch_phase3(
            &mut pool,
            &path,
            make_test_watcher_with_exit("incoming", Some(incoming_exited.clone())),
        )
        .unwrap();
        assert_eq!(pool.len(), 1, "竞态时不应新增条目");
        assert!(
            pool.get(&path).unwrap().is_running(),
            "池中原 watcher 保持运行"
        );
        assert!(
            incoming_exited.load(Ordering::SeqCst),
            "竞态丢弃的 watcher 线程应已退出"
        );
    }
}
