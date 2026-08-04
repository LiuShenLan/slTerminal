//! 信号文件解析 —— 纯函数 + 文件处理流程
//!
//! 职责：
//! - 定义 HookEventPayload DTO（C1 契约 8 字段，camelCase）
//! - parse_signal_file 纯函数：JSON 字符串 → Option<HookEventPayload>
//! - process_signal_file：读文件 → 解析 → emit("hook-event") → 删除

// P1-BE-01/02 阶段钩子：以下公有 API 待后续模块消费后移除本行
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::Emitter;

/// Hook 事件载荷（C1 契约 8 字段，camelCase 序列化）
///
/// PartialEq 供 serde 往返精确断言测试使用（HUK-09）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEventPayload {
    /// 页签路由标识（环境变量 SLTERM_PANEL_ID）
    pub panel_id: String,
    /// 事件名（C9 10 事件之一）
    pub event: String,
    /// 时间戳（毫秒）
    pub timestamp: u64,
    /// 会话标识
    pub session_id: String,
    /// transcript 文件路径
    pub transcript_path: String,
    /// 当前工作目录
    pub cwd: String,
    /// 工具名（仅工具事件，可缺省）
    pub tool_name: Option<String>,
    /// 通知类型（仅 Notification 事件，可缺省）
    pub notification_type: Option<String>,
}

/// 解析信号文件 JSON 内容为 HookEventPayload
///
/// panelId 缺失（空串）或 JSON 解析失败返回 None。
pub fn parse_signal_file(content: &str) -> Option<HookEventPayload> {
    let payload: HookEventPayload = serde_json::from_str(content).ok()?;
    if payload.panel_id.is_empty() {
        return None;
    }
    Some(payload)
}

/// 处理单个信号文件：读取 → 解析 → emit("hook-event") → 删除
///
/// 生产路径委托 `process_signal_file_with`，emit 经 tauri::AppHandle 实现
/// （D6 最小可测性重构：emit 抽为注入参数，零行为变更）。
pub fn process_signal_file(app_handle: &tauri::AppHandle, path: &Path) {
    process_signal_file_with(path, |payload| app_handle.emit("hook-event", payload));
}

/// 可测试核心：读取 → 解析 → emit（注入闭包）→ 删除
///
/// 读失败、解析失败、缺 panelId 均 tracing::warn! 并仍尝试删除文件，绝不 panic。
/// emit 闭包返回 Err 时仅 warn，文件同样继续删除。
pub(crate) fn process_signal_file_with(
    path: &Path,
    emit: impl Fn(&HookEventPayload) -> Result<(), tauri::Error>,
) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("读取信号文件失败 {}: {e}", path.display());
            let _ = fs::remove_file(path);
            return;
        }
    };

    match parse_signal_file(&content) {
        Some(payload) => {
            if let Err(e) = emit(&payload) {
                tracing::warn!("发送 hook-event 失败: {e}");
            }
        }
        None => {
            tracing::warn!(
                "解析信号文件失败（缺 panelId 或非法 JSON）: {}",
                path.display()
            );
        }
    }

    if let Err(e) = fs::remove_file(path) {
        tracing::warn!("删除信号文件失败 {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合法完整 JSON → 返回 Some
    #[test]
    fn parse_valid_full_payload() {
        let json = r#"{"panelId":"p1","event":"PreToolUse","timestamp":1700000000000,"sessionId":"s1","transcriptPath":"/t.jsonl","cwd":"/cwd","toolName":"Bash","notificationType":null}"#;
        let r = parse_signal_file(json);
        assert!(r.is_some());
        let p = r.unwrap();
        assert_eq!(p.panel_id, "p1");
        assert_eq!(p.event, "PreToolUse");
        assert_eq!(p.timestamp, 1700000000000);
        assert_eq!(p.session_id, "s1");
        assert_eq!(p.tool_name.unwrap(), "Bash");
        assert!(p.notification_type.is_none());
    }

    /// toolName 和 notificationType 均为 null → 正常解析
    #[test]
    fn parse_with_null_optionals() {
        let json = r#"{"panelId":"p2","event":"SessionStart","timestamp":1,"sessionId":"s2","transcriptPath":"/t.jsonl","cwd":"/","toolName":null,"notificationType":null}"#;
        let p = parse_signal_file(json).unwrap();
        assert!(p.tool_name.is_none());
        assert!(p.notification_type.is_none());
    }

    /// 缺 panelId 字段 → None
    #[test]
    fn parse_missing_panel_id() {
        let json = r#"{"event":"SessionStart","timestamp":1,"sessionId":"s2","transcriptPath":"/t.jsonl","cwd":"/"}"#;
        assert!(parse_signal_file(json).is_none());
    }

    /// panelId 为空串 → None
    #[test]
    fn parse_empty_panel_id() {
        let json = r#"{"panelId":"","event":"SessionStart","timestamp":1,"sessionId":"s2","transcriptPath":"/t.jsonl","cwd":"/"}"#;
        assert!(parse_signal_file(json).is_none());
    }

    /// 非法 JSON → None
    #[test]
    fn parse_invalid_json() {
        assert!(parse_signal_file("not json at all").is_none());
        assert!(parse_signal_file("{broken").is_none());
    }

    /// 空字符串 → None
    #[test]
    fn parse_empty_string() {
        assert!(parse_signal_file("").is_none());
    }

    /// 仅空白 → None
    #[test]
    fn parse_whitespace_only() {
        assert!(parse_signal_file("   ").is_none());
    }

    /// serde camelCase 键集合精确匹配（HUK-09：8 字段全量，防多键/缺键）
    fn assert_payload_key_set(json: &str) {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
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
    }

    /// serde camelCase 序列化 → 反序列化往返精确断言（HUK-09，替代 contains 弱断言）
    #[test]
    fn serialize_deserialize_roundtrip() {
        let p = HookEventPayload {
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
        assert_payload_key_set(&json);
        // 字段值精确断言（防字段值/类型错误）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["panelId"], "p1");
        assert_eq!(v["event"], "SessionStart");
        assert_eq!(v["timestamp"], 1700000000000u64);
        assert_eq!(v["toolName"], "Bash");
        assert_eq!(v["notificationType"], serde_json::Value::Null);
        // 序列化 → 反序列化往返
        let back: HookEventPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    /// camelCase JSON 反序列化 → 字段正确映射
    #[test]
    fn deserialize_camelcase() {
        let json = r#"{"panelId":"p3","event":"Stop","timestamp":999,"sessionId":"s3","transcriptPath":"/x.jsonl","cwd":"/app","toolName":null,"notificationType":"idle"}"#;
        let p: HookEventPayload = serde_json::from_str(json).unwrap();
        assert_eq!(p.panel_id, "p3");
        assert_eq!(p.event, "Stop");
        assert_eq!(p.notification_type.unwrap(), "idle");
    }

    // ── process_signal_file_with 全流程（HUK-01：D6 emit 注入）──

    const VALID_SIGNAL_JSON: &str = r#"{"panelId":"p1","event":"PreToolUse","timestamp":1700000000000,"sessionId":"s1","transcriptPath":"/t.jsonl","cwd":"/cwd","toolName":"Bash","notificationType":null}"#;

    /// 全流程：读文件 → parse → emit("hook-event") → 删文件
    #[test]
    fn process_full_flow_read_emit_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evt.json");
        std::fs::write(&path, VALID_SIGNAL_JSON).unwrap();

        let emitted = std::sync::Mutex::new(Vec::new());
        process_signal_file_with(&path, |payload| {
            emitted.lock().unwrap().push(payload.clone());
            Ok(())
        });

        // 读 → parse → emit 全流程
        let got = emitted.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].panel_id, "p1");
        assert_eq!(got[0].event, "PreToolUse");
        assert_eq!(got[0].timestamp, 1700000000000);
        assert_eq!(got[0].session_id, "s1");
        assert_eq!(got[0].tool_name.as_deref(), Some("Bash"));
        drop(got);
        // 删文件
        assert!(!path.exists());
    }

    /// emit 失败仍删除文件（emit 返回 Err 仅 warn，删除不受影响）
    #[test]
    fn process_emit_failure_still_deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evt.json");
        std::fs::write(&path, VALID_SIGNAL_JSON).unwrap();

        let emitted = std::sync::atomic::AtomicUsize::new(0);
        process_signal_file_with(&path, |_| {
            emitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(tauri::Error::Io(std::io::Error::other("模拟 emit 失败")))
        });

        assert_eq!(emitted.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!path.exists()); // emit 失败后文件仍被删除
    }

    /// 非法 JSON 降级：不 emit，文件仍删除
    #[test]
    fn process_invalid_json_degrades_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json at all").unwrap();

        let emitted = std::sync::atomic::AtomicUsize::new(0);
        process_signal_file_with(&path, |_| {
            emitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });

        assert_eq!(emitted.load(std::sync::atomic::Ordering::SeqCst), 0); // 降级不 emit
        assert!(!path.exists()); // 仍删除
    }

    /// 缺 panelId 降级：不 emit，文件仍删除
    #[test]
    fn process_missing_panel_id_degrades_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nopid.json");
        std::fs::write(
            &path,
            r#"{"event":"SessionStart","timestamp":1,"sessionId":"s","transcriptPath":"/t.jsonl","cwd":"/"}"#,
        )
        .unwrap();

        let emitted = std::sync::atomic::AtomicUsize::new(0);
        process_signal_file_with(&path, |_| {
            emitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });

        assert_eq!(emitted.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!path.exists());
    }

    /// 读失败分支：warn 后尝试删除，绝不 panic
    #[test]
    fn process_read_failure_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json"); // 文件不存在——读失败分支
        process_signal_file_with(&missing, |_| Ok(())); // 不 panic
        assert!(!missing.exists());
    }
}
