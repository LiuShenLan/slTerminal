//! Hooks 模块 —— 宿主侧增强信号通道、注入管理与状态可视化（CLI 泛化命令层）
//!
//! 职责：
//! - 信号文件监听与解析（signal.rs）
//! - 信号目录监听器（watcher.rs）
//! - CliHooksProvider trait + cliId 键静态注册表（provider.rs）
//! - claude hooks provider（claude/：注入/卸载/状态/statusline 桥接/配置实现下沉）
//! - 6 条泛化 Tauri 命令（本文件命令层，按 cliId 分发到 provider）
//! - 共享 DTO：AgentInjectionStatus / AgentHookInjectionStatus

pub mod claude;
pub mod provider;
pub mod signal;
pub mod watcher;

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::hooks::provider::resolve_provider;
use crate::hooks::watcher::HookSignalWatcher;
use crate::state::AppState;

/// 重导出 signal 模块的 AgentEventPayload DTO
#[allow(unused_imports)]
pub use signal::AgentEventPayload;

/// 注入状态枚举（C6 契约；决策 3 更名 AgentInjectionStatus）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AgentInjectionStatus {
    /// 已注入且版本匹配
    Injected,
    /// 未注入
    NotInjected,
    /// 已注入但版本过旧
    Outdated,
}

/// Agent 注入状态 DTO（C6 契约；决策 3 更名 AgentHookInjectionStatus）
///
/// PartialEq 供 serde 往返精确断言测试使用（HUK-09）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHookInjectionStatus {
    /// 注入状态
    pub status: AgentInjectionStatus,
    /// 已注入脚本版本号（未注入时为 null）
    pub version: Option<u32>,
}

/// 监听器句柄抽象（HUK-04 最小可测性重构：存储 trait object 化，测试可注入桩）
/// 仅用于 WATCHER 静态存储，生产实例为 `HookSignalWatcher`。
trait WatcherHandle: Send + 'static {}

impl WatcherHandle for HookSignalWatcher {}

/// 全局 Hook 信号监听器实例
/// 使用静态 Mutex 而非 AppState 字段，避免 state.rs 与 hooks 循环依赖
static WATCHER: Mutex<Option<Box<dyn WatcherHandle>>> = Mutex::new(None);

/// 启动 Hook 信号文件监听器
///
/// 监听 `~/.slterminal/hooks-events/` 目录（单目录全 CLI 共用，路由靠
/// payload.panelId + cliId），检测信号文件创建事件，解析后通过
/// Tauri Event `agent-event` 广播到前端。
/// 幂等：已启动时跳过而不报错。
pub fn start_signal_watcher(app_handle: AppHandle) {
    start_signal_watcher_impl(|| {
        HookSignalWatcher::start(app_handle).map(|w| Box::new(w) as Box<dyn WatcherHandle>)
    });
}

/// 可测试核心：锁 → 幂等判断 → 存储/失败（D6 最小可测性重构，零行为变更）
fn start_signal_watcher_impl(
    start: impl FnOnce() -> Result<Box<dyn WatcherHandle>, Box<dyn std::error::Error>>,
) {
    let mut guard = match WATCHER.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("WATCHER 锁中毒: {e}");
            return;
        }
    };
    if guard.is_some() {
        tracing::warn!("Hook 信号监听器已启动，跳过重复启动");
        return;
    }
    match start() {
        Ok(w) => {
            *guard = Some(w);
            tracing::info!("Hook 信号监听器已启动");
        }
        Err(e) => {
            tracing::error!("启动 Hook 信号监听器失败: {e}");
        }
    }
}

/// HUK-04 测试重置钩子：清空全局 WATCHER（仅测试用，drop 旧实例触发其清理逻辑）
#[cfg(test)]
fn reset_watcher_for_test() {
    let _ = WATCHER.lock().unwrap().take();
}

// ── 命令核心（L1 可测：block_on 直测 cliId 透传；provider 解析无 IO） ──
//
// 错误语义（MC-211）：未知 cliId → Validation("未知 cliId: ...")；
// 已注册但无 hooks 能力 → Validation（含「不支持 hooks 能力」语义），
// 两者均由 resolve_provider 统一产出（见 provider.rs）。
// 阻塞 I/O 经 spawn_blocking 串行化（硬约束 #3）。

pub(crate) async fn run_agent_hooks_inject(
    cli_id: String,
) -> Result<AgentHookInjectionStatus, AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || provider.inject())
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

pub(crate) async fn run_agent_hooks_uninstall(cli_id: String) -> Result<(), AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || provider.uninstall())
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

pub(crate) async fn run_agent_hooks_injection_status(
    cli_id: String,
) -> Result<AgentHookInjectionStatus, AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || provider.injection_status())
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// 客户端关闭清理：恢复 statusline 桥接（还原备份原配置，备份保留供重开重注入）
pub(crate) async fn run_agent_hooks_restore_statusline(cli_id: String) -> Result<(), AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || provider.restore_statusline())
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

pub(crate) async fn run_agent_hooks_config_read(
    cli_id: String,
    layer: String,
    project_path: Option<String>,
    project_root: Option<PathBuf>,
) -> Result<Value, AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || {
        provider.config_read(&layer, project_path.as_deref(), &project_root)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

pub(crate) async fn run_agent_hooks_config_write(
    cli_id: String,
    layer: String,
    hooks: Value,
    project_path: Option<String>,
    project_root: Option<PathBuf>,
) -> Result<(), AppError> {
    let provider = resolve_provider(&cli_id)?;
    tokio::task::spawn_blocking(move || {
        provider.config_write(&layer, hooks, project_path.as_deref(), &project_root)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

// ── Tauri 命令（cliId 分发到 provider；config 两命令读取 AppState.project_root） ──

/// agent_hooks_inject — 按 cliId 分发注入（claude：落盘脚本 + merge 注入 user 层 settings.json）
#[tauri::command]
pub async fn agent_hooks_inject(cli_id: String) -> Result<AgentHookInjectionStatus, AppError> {
    run_agent_hooks_inject(cli_id).await
}

/// agent_hooks_uninstall — 按 cliId 分发卸载（移除配置段 + 删脚本目录 + 清信号目录）
#[tauri::command]
pub async fn agent_hooks_uninstall(cli_id: String) -> Result<(), AppError> {
    run_agent_hooks_uninstall(cli_id).await
}

/// agent_hooks_injection_status — 按 cliId 分发注入状态查询（三态）
#[tauri::command]
pub async fn agent_hooks_injection_status(
    cli_id: String,
) -> Result<AgentHookInjectionStatus, AppError> {
    run_agent_hooks_injection_status(cli_id).await
}

/// agent_hooks_restore_statusline — 客户端关闭清理：还原 statusline 桥接（备份保留）
#[tauri::command]
pub async fn agent_hooks_restore_statusline(cli_id: String) -> Result<(), AppError> {
    run_agent_hooks_restore_statusline(cli_id).await
}

/// 启动自动重注入：对注册表全部已注册 provider 调 reinject_statusline（失败仅 warn 不阻断启动）
pub fn reinject_statusline_on_startup() {
    for (cli_id, entry) in provider::REGISTRY {
        let Some(p) = entry else { continue };
        if let Err(e) = p.reinject_statusline() {
            tracing::warn!("启动重注入 statusline 失败（cliId {cli_id}）: {e}");
        }
    }
}

/// agent_hooks_config_read — 按 cliId 分发 hooks 配置子树读取（P3-BE-02）
#[tauri::command]
pub async fn agent_hooks_config_read(
    cli_id: String,
    layer: String,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<Value, AppError> {
    // 锁内读取 project_root 并 clone 出（作用域块：块结束即 drop 锁守卫，
    // 避免非 Send 的 RwLockReadGuard 跨 await 存活）
    let project_root = {
        let root_guard = state.project_root.read().map_err(|e| AppError::IoKind {
            kind: "lock".into(),
            message: format!("获取 project_root 锁失败: {e}"),
        })?;
        root_guard.clone()
    };
    run_agent_hooks_config_read(cli_id, layer, project_path, project_root).await
}

/// agent_hooks_config_write — 按 cliId 分发 hooks 配置子树写回（read-modify-write merge，P3-BE-03）
#[tauri::command]
pub async fn agent_hooks_config_write(
    cli_id: String,
    layer: String,
    hooks: Value,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // 锁内读取 project_root 并 clone 出（作用域块：块结束即 drop 锁守卫，
    // 避免非 Send 的 RwLockReadGuard 跨 await 存活）
    let project_root = {
        let root_guard = state.project_root.read().map_err(|e| AppError::IoKind {
            kind: "lock".into(),
            message: format!("获取 project_root 锁失败: {e}"),
        })?;
        root_guard.clone()
    };
    run_agent_hooks_config_write(cli_id, layer, hooks, project_path, project_root).await
}

#[cfg(test)]
mod hooks_tests {
    use super::*;
    use crate::hooks::claude::HomeDirGuard;

    // ── AgentInjectionStatus / AgentHookInjectionStatus serde（HUK-09：roundtrip + 键集合精确匹配） ──

    /// serde 键集合精确匹配辅助（防多键/缺键）
    fn assert_status_key_set(json: &str) {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["status", "version"]);
    }

    #[test]
    fn injection_status_roundtrip_injected() {
        let s = AgentHookInjectionStatus {
            status: AgentInjectionStatus::Injected,
            version: Some(1),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        // 字段值精确断言（防字段值/类型错误）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "injected");
        assert_eq!(v["version"], 1);
        // 序列化 → 反序列化往返
        let back: AgentHookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn injection_status_roundtrip_not_injected() {
        let s = AgentHookInjectionStatus {
            status: AgentInjectionStatus::NotInjected,
            version: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "notInjected");
        assert_eq!(v["version"], serde_json::Value::Null);
        let back: AgentHookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn injection_status_roundtrip_outdated() {
        let s = AgentHookInjectionStatus {
            status: AgentInjectionStatus::Outdated,
            version: Some(2),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "outdated");
        assert_eq!(v["version"], 2);
        let back: AgentHookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    // ── AgentEventPayload serde（HUK-09：roundtrip + 键集合精确匹配，10 键含 cliId/usedPercentage） ──

    #[test]
    fn agent_event_payload_roundtrip_and_key_set() {
        let p = signal::AgentEventPayload {
            panel_id: "p1".into(),
            event: "SessionStart".into(),
            timestamp: 1700000000000,
            session_id: "s1".into(),
            usage_source_path: Some("/t.jsonl".into()),
            cwd: "/cwd".into(),
            tool_name: Some("Bash".into()),
            notification_type: Some("idle".into()),
            cli_id: Some("claude".into()),
            used_percentage: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        // 键集合精确匹配（10 字段全量含 cliId/usedPercentage，防多键/缺键）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "cliId",
                "cwd",
                "event",
                "notificationType",
                "panelId",
                "sessionId",
                "timestamp",
                "toolName",
                "usageSourcePath",
                "usedPercentage",
            ]
        );
        // 序列化 → 反序列化往返
        let back: signal::AgentEventPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    // ── start_signal_watcher 幂等启动（HUK-04） ──

    /// 测试桩监听器（无资源，drop 无副作用）
    struct StubWatcher;

    impl WatcherHandle for StubWatcher {}

    #[test]
    fn start_signal_watcher_first_start_stores_instance() {
        reset_watcher_for_test();
        let start_calls = std::sync::atomic::AtomicUsize::new(0);
        start_signal_watcher_impl(|| {
            start_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(StubWatcher) as Box<dyn WatcherHandle>)
        });
        // 首次启动：start 被调一次，WATCHER 已存实例
        assert!(WATCHER.lock().unwrap().is_some());
        assert_eq!(start_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        reset_watcher_for_test();
    }

    #[test]
    fn start_signal_watcher_second_start_skipped() {
        reset_watcher_for_test();
        let start_calls = std::sync::atomic::AtomicUsize::new(0);
        let start = || {
            start_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(StubWatcher) as Box<dyn WatcherHandle>)
        };
        start_signal_watcher_impl(start);
        start_signal_watcher_impl(start); // 已启动 → 跳过，不再次调用 start
        assert_eq!(start_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        reset_watcher_for_test();
    }

    #[test]
    fn start_signal_watcher_failure_not_stored() {
        reset_watcher_for_test();
        start_signal_watcher_impl(|| Err("模拟启动失败".into()));
        // 启动失败不存入 WATCHER，后续可重试
        assert!(WATCHER.lock().unwrap().is_none());
    }

    #[test]
    fn start_signal_watcher_reset_reenables_restart() {
        reset_watcher_for_test();
        start_signal_watcher_impl(|| Ok(Box::new(StubWatcher) as Box<dyn WatcherHandle>));
        assert!(WATCHER.lock().unwrap().is_some());
        reset_watcher_for_test(); // 重置钩子生效：清空后可再次启动
        assert!(WATCHER.lock().unwrap().is_none());
        start_signal_watcher_impl(|| Ok(Box::new(StubWatcher) as Box<dyn WatcherHandle>));
        assert!(WATCHER.lock().unwrap().is_some());
        reset_watcher_for_test();
    }

    // ── parse_signal_file 纯函数（委托 signal 模块测试，此处仅快速冒烟） ──

    #[test]
    fn parse_signal_file_valid_smoke() {
        let content = r#"{"panelId":"p1","event":"PreToolUse","timestamp":1000,"sessionId":"s1","usageSourcePath":"/t.jsonl","cwd":"/cwd","toolName":null,"notificationType":null}"#;
        let r = signal::parse_signal_file(content);
        assert!(r.is_some());
        assert_eq!(r.unwrap().panel_id, "p1");
    }

    #[test]
    fn parse_signal_file_missing_panel_id() {
        let content = r#"{"event":"PreToolUse","timestamp":1000,"sessionId":"s1","usageSourcePath":"/t.jsonl","cwd":"/cwd"}"#;
        assert!(signal::parse_signal_file(content).is_none());
    }

    #[test]
    fn parse_signal_file_invalid_json() {
        assert!(signal::parse_signal_file("not json").is_none());
    }

    #[test]
    fn parse_signal_file_empty() {
        assert!(signal::parse_signal_file("").is_none());
    }

    // ── 命令层 cliId 透传（L1 新增：block_on 直测；HomeDirGuard 注入 tempdir 隔离） ──

    /// 手动 current_thread runtime 驱动 async 命令核心（tokio 未启用 #[tokio::test]）
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn agent_hooks_inject_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        let status = block_on(run_agent_hooks_inject("claude".into())).unwrap();
        assert_eq!(status.status, AgentInjectionStatus::Injected);
        // 注入全部落在覆盖 home 的 tempdir 内（L1 隔离纪律）
        assert!(
            dir.path()
                .join(".slterminal")
                .join("hooks")
                .join("slterm-hook-reporter.js")
                .exists(),
            "脚本应写入覆盖 home 的脚本目录"
        );
        assert!(dir.path().join(".claude").join("settings.json").exists());
    }

    #[test]
    fn agent_hooks_uninstall_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        // 先注入再卸载：settings matcher 移除 + 目录删除
        block_on(run_agent_hooks_inject("claude".into())).unwrap();
        block_on(run_agent_hooks_uninstall("claude".into())).unwrap();
        assert!(
            !dir.path().join(".slterminal").join("hooks").exists(),
            "卸载后脚本目录应删除"
        );
        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".claude").join("settings.json")).unwrap(),
        )
        .unwrap();
        assert!(settings.get("hooks").is_none(), "卸载后 hooks 键应整体移除");
    }

    #[test]
    fn agent_hooks_injection_status_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        // 未注入 → NotInjected（三态经命令层返回）
        let status = block_on(run_agent_hooks_injection_status("claude".into())).unwrap();
        assert_eq!(status.status, AgentInjectionStatus::NotInjected);
    }

    #[test]
    fn agent_hooks_restore_statusline_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        // 先注入（statusLine 桥接 + 备份）→ 恢复 → statusLine 还原为原配置
        block_on(run_agent_hooks_inject("claude".into())).unwrap();
        block_on(run_agent_hooks_restore_statusline("claude".into())).unwrap();
        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".claude").join("settings.json")).unwrap(),
        )
        .unwrap();
        assert!(
            settings.get("statusLine").is_none(),
            "无原配置时恢复后 statusLine 键应移除"
        );
    }

    #[test]
    fn agent_hooks_config_read_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        let settings = dir.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let hooks = serde_json::json!({"PreToolUse": [1]});
        std::fs::write(
            &settings,
            serde_json::to_string(&serde_json::json!({"hooks": hooks})).unwrap(),
        )
        .unwrap();
        let v = block_on(run_agent_hooks_config_read(
            "claude".into(),
            "user".into(),
            None,
            None,
        ))
        .unwrap();
        assert_eq!(v, hooks, "layer/project_path 应透传到 provider");
    }

    #[test]
    fn agent_hooks_config_write_cli_id_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        let hooks = serde_json::json!({"SessionStart": []});
        block_on(run_agent_hooks_config_write(
            "claude".into(),
            "user".into(),
            hooks.clone(),
            None,
            None,
        ))
        .unwrap();
        let path = dir.path().join(".claude").join("settings.json");
        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reloaded, serde_json::json!({"hooks": hooks}));
    }

    #[test]
    fn unknown_cli_id_validation_on_all_commands() {
        // 6 命令未知 cliId → Validation（消息含「未知 cliId」语义，resolve_provider 统一产出）
        let errs = vec![
            block_on(run_agent_hooks_inject("nope".into())).unwrap_err(),
            block_on(run_agent_hooks_uninstall("nope".into())).unwrap_err(),
            block_on(run_agent_hooks_injection_status("nope".into())).unwrap_err(),
            block_on(run_agent_hooks_restore_statusline("nope".into())).unwrap_err(),
            block_on(run_agent_hooks_config_read(
                "nope".into(),
                "user".into(),
                None,
                None,
            ))
            .unwrap_err(),
            block_on(run_agent_hooks_config_write(
                "nope".into(),
                "user".into(),
                serde_json::json!({}),
                None,
                None,
            ))
            .unwrap_err(),
        ];
        for err in errs {
            match err {
                AppError::Validation(msg) => {
                    assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}");
                }
                other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
            }
        }
    }
}
