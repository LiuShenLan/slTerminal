//! Hooks 模块 —— 宿主侧增强信号通道、注入管理与状态可视化
//!
//! 职责：
//! - 信号文件监听与解析（signal.rs）
//! - Hook 脚本注入/卸载/状态检测（inject.rs）
//! - 信号目录监听器（watcher.rs）

pub mod config;
pub mod inject;
pub mod signal;
pub mod usage;
pub mod watcher;

// 三命令由 inject 模块实现，经此 re-export 供外部引用
// （generate_handler! 使用完整路径 hooks::inject::xxx，此处保留供前端 IPC 类型导入）
#[allow(unused_imports)]
pub use inject::{hooks_inject, hooks_uninstall, hooks_injection_status};

// 配置读写命令由 config 模块实现，经此 re-export 供外部引用（风格同 inject）
#[allow(unused_imports)]
pub use config::{hooks_config_read, hooks_config_write};

use std::sync::Mutex;
use tauri::AppHandle;

use crate::hooks::watcher::HookSignalWatcher;

/// 注入状态枚举（C6 契约）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum InjectionStatus {
    /// 已注入且版本匹配
    Injected,
    /// 未注入
    NotInjected,
    /// 已注入但版本过旧
    Outdated,
}

/// Hook 注入状态 DTO（C6 契约）
///
/// PartialEq 供 serde 往返精确断言测试使用（HUK-09）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInjectionStatus {
    /// 注入状态
    pub status: InjectionStatus,
    /// 已注入脚本版本号（未注入时为 null）
    pub version: Option<u32>,
}

/// 重导出 signal 模块的 HookEventPayload DTO
#[allow(unused_imports)]
pub use signal::HookEventPayload;

/// 监听器句柄抽象（HUK-04 最小可测性重构：存储 trait object 化，测试可注入桩）
/// 仅用于 WATCHER 静态存储，生产实例为 `HookSignalWatcher`。
trait WatcherHandle: Send + 'static {}

impl WatcherHandle for HookSignalWatcher {}

/// 全局 Hook 信号监听器实例
/// 使用静态 Mutex 而非 AppState 字段，避免 state.rs 与 hooks 循环依赖
static WATCHER: Mutex<Option<Box<dyn WatcherHandle>>> = Mutex::new(None);

/// 启动 Hook 信号文件监听器
///
/// 监听 `~/.slterminal/hooks-events/` 目录，检测信号文件创建事件，
/// 解析后通过 Tauri Event `hook-event` 广播到前端。
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── InjectionStatus / HookInjectionStatus serde（HUK-09：roundtrip + 键集合精确匹配） ──

    /// serde 键集合精确匹配辅助（防多键/缺键）
    fn assert_status_key_set(json: &str) {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["status", "version"]);
    }

    #[test]
    fn injection_status_roundtrip_injected() {
        let s = HookInjectionStatus {
            status: InjectionStatus::Injected,
            version: Some(1),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        // 字段值精确断言（防字段值/类型错误）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "injected");
        assert_eq!(v["version"], 1);
        // 序列化 → 反序列化往返
        let back: HookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn injection_status_roundtrip_not_injected() {
        let s = HookInjectionStatus {
            status: InjectionStatus::NotInjected,
            version: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "notInjected");
        assert_eq!(v["version"], serde_json::Value::Null);
        let back: HookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn injection_status_roundtrip_outdated() {
        let s = HookInjectionStatus {
            status: InjectionStatus::Outdated,
            version: Some(2),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_status_key_set(&json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "outdated");
        assert_eq!(v["version"], 2);
        let back: HookInjectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    // ── HookEventPayload serde（HUK-09：roundtrip + 键集合精确匹配） ──

    #[test]
    fn hook_event_payload_roundtrip_and_key_set() {
        let p = signal::HookEventPayload {
            panel_id: "p1".into(),
            event: "SessionStart".into(),
            timestamp: 1700000000000,
            session_id: "s1".into(),
            transcript_path: "/t.jsonl".into(),
            cwd: "/cwd".into(),
            tool_name: Some("Bash".into()),
            notification_type: Some("idle".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        // 键集合精确匹配（8 字段全量，防多键/缺键）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "cwd",
                "event",
                "notificationType",
                "panelId",
                "sessionId",
                "timestamp",
                "toolName",
                "transcriptPath",
            ]
        );
        // 序列化 → 反序列化往返
        let back: signal::HookEventPayload = serde_json::from_str(&json).unwrap();
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
        let content = r#"{"panelId":"p1","event":"PreToolUse","timestamp":1000,"sessionId":"s1","transcriptPath":"/t.jsonl","cwd":"/cwd","toolName":null,"notificationType":null}"#;
        let r = signal::parse_signal_file(content);
        assert!(r.is_some());
        assert_eq!(r.unwrap().panel_id, "p1");
    }

    #[test]
    fn parse_signal_file_missing_panel_id() {
        let content = r#"{"event":"PreToolUse","timestamp":1000,"sessionId":"s1","transcriptPath":"/t.jsonl","cwd":"/cwd"}"#;
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
}
