//! Hooks 模块 —— 宿主侧增强信号通道、注入管理与状态可视化
//!
//! 职责：
//! - 信号文件监听与解析（signal.rs）
//! - Hook 脚本注入/卸载/状态检测（inject.rs）
//! - 信号目录监听器（watcher.rs）

pub mod inject;
pub mod signal;
pub mod usage;
pub mod watcher;

// 三命令由 inject 模块实现，经此 re-export 供外部引用
// （generate_handler! 使用完整路径 hooks::inject::xxx，此处保留供前端 IPC 类型导入）
#[allow(unused_imports)]
pub use inject::{hooks_inject, hooks_uninstall, hooks_injection_status};

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// 全局 Hook 信号监听器实例
/// 使用静态 Mutex 而非 AppState 字段，避免 state.rs 与 hooks 循环依赖
static WATCHER: Mutex<Option<HookSignalWatcher>> = Mutex::new(None);

/// 启动 Hook 信号文件监听器
///
/// 监听 `~/.slterminal/hooks-events/` 目录，检测信号文件创建事件，
/// 解析后通过 Tauri Event `hook-event` 广播到前端。
/// 幂等：已启动时跳过而不报错。
pub fn start_signal_watcher(app_handle: AppHandle) {
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
    match HookSignalWatcher::start(app_handle) {
        Ok(w) => {
            *guard = Some(w);
            tracing::info!("Hook 信号监听器已启动");
        }
        Err(e) => {
            tracing::error!("启动 Hook 信号监听器失败: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── InjectionStatus 序列化 ──

    #[test]
    fn injection_status_serialize_injected() {
        let s = HookInjectionStatus {
            status: InjectionStatus::Injected,
            version: Some(1),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"injected\""));
        assert!(json.contains("\"version\""));
        assert!(json.contains("1"));
    }

    #[test]
    fn injection_status_serialize_not_injected() {
        let s = HookInjectionStatus {
            status: InjectionStatus::NotInjected,
            version: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"notInjected\""));
        assert!(json.contains("\"version\":null"));
    }

    #[test]
    fn injection_status_serialize_outdated() {
        let s = HookInjectionStatus {
            status: InjectionStatus::Outdated,
            version: Some(2),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"outdated\""));
    }

    // ── HookEventPayload serde ──

    #[test]
    fn hook_event_payload_camelcase_keys() {
        let p = signal::HookEventPayload {
            panel_id: "p1".into(),
            event: "SessionStart".into(),
            timestamp: 1700000000000,
            session_id: "s1".into(),
            transcript_path: "/t.jsonl".into(),
            cwd: "/cwd".into(),
            tool_name: Some("Bash".into()),
            notification_type: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"panelId\""));
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"transcriptPath\""));
        assert!(json.contains("\"toolName\""));
        assert!(json.contains("\"notificationType\""));
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
