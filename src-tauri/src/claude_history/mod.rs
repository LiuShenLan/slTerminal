//! Claude Code 历史会话查询 —— 后端扫描模块（Stage 01）
//!
//! 职责：
//! - DTO 定义：`HistorySession`（IPC 契约七字段）+ `TitleSource`（五变体）
//! - `scan.rs`：扫描根单点（SEC-02/BE-06）+ `claude_history_scan` 命令（BE-02/BE-05）
//! - `jsonl.rs`：JSONL 轻量解析纯函数（BE-03/BE-04）
//! - `ops.rs`：写操作（SEC-01 sessionId 校验 + BE-07 删除 / BE-08 重命名）
//!
//! 数据源事实约束（规格 3.1）：存储根 `~/.claude/projects/`，一级目录名 = cwd 的
//! 有损编码（禁止反解码），会话文件 = `<uuidv4>.jsonl`（文件名主干即 sessionId）。

pub mod jsonl;
pub mod ops;
pub mod scan;

use serde::{Deserialize, Serialize};

/// 标题来源枚举（IPC 契约五变体，serde camelCase）
///
/// 序列化为 `"customTitle"` / `"aiTitle"` / `"summary"` / `"firstPrompt"` / `"none"`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TitleSource {
    /// 用户自定义标题（custom-title 行）
    CustomTitle,
    /// AI 自动标题（ai-title 行）
    AiTitle,
    /// 会话摘要（summary 首行）
    Summary,
    /// 回退到首条可见 user prompt
    FirstPrompt,
    /// 无标题
    None,
}

/// 历史会话元数据 DTO（IPC 契约七字段，serde camelCase，硬约束 #4）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySession {
    /// 会话 ID（文件名主干 = UUID）
    pub session_id: String,
    /// 会话启动时工作目录（从 JSONL 内容解析，不反解码目录名）
    pub cwd: Option<String>,
    /// 标题（回退链合成结果；全无时为 None）
    pub title: Option<String>,
    /// 标题来源
    pub title_source: TitleSource,
    /// 首条可见 user prompt（≤200 字符）
    pub first_prompt: Option<String>,
    /// 文件修改时间（毫秒时间戳，决策 26）
    pub mtime_ms: u64,
    /// cwd 目录当前是否存在（cwd 为 null 时恒 false）
    pub cwd_exists: bool,
}

/// 校验字符串是否为 UUID 形态（`^[0-9a-fA-F]{8}-...-{12}$`）
///
/// 独立纯函数供 scan 排除非会话文件、Stage 02 写操作（SEC-01）复用。
pub fn is_uuid_filename(stem: &str) -> bool {
    let bytes = stem.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HistorySession serde camelCase（七键契约，防字段漂移） ──

    #[test]
    fn history_session_serialize_camelcase_seven_keys() {
        let s = HistorySession {
            session_id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            cwd: Some("C:\\proj".to_string()),
            title: Some("修复登录 bug".to_string()),
            title_source: TitleSource::CustomTitle,
            first_prompt: Some("你好".to_string()),
            mtime_ms: 1_752_500_000_000,
            cwd_exists: true,
        };
        let json = serde_json::to_value(&s).unwrap();
        let obj = json.as_object().unwrap();
        // 键集合恰为七键（无多余键）
        let expected: std::collections::BTreeSet<&str> = [
            "sessionId",
            "cwd",
            "title",
            "titleSource",
            "firstPrompt",
            "mtimeMs",
            "cwdExists",
        ]
        .into_iter()
        .collect();
        let actual: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
        assert_eq!(actual, expected, "HistorySession 键集合应与契约一致");
    }

    #[test]
    fn history_session_deserialize_camelcase() {
        let json = r#"{
            "sessionId":"123e4567-e89b-12d3-a456-426614174000",
            "cwd":null,
            "title":null,
            "titleSource":"none",
            "firstPrompt":null,
            "mtimeMs":0,
            "cwdExists":false
        }"#;
        let s: HistorySession = serde_json::from_str(json).unwrap();
        assert_eq!(s.session_id, "123e4567-e89b-12d3-a456-426614174000");
        assert!(s.cwd.is_none());
        assert!(s.title.is_none());
        assert_eq!(s.title_source, TitleSource::None);
        assert!(s.first_prompt.is_none());
        assert_eq!(s.mtime_ms, 0);
        assert!(!s.cwd_exists);
    }

    #[test]
    fn history_session_roundtrip_all_fields() {
        let s = HistorySession {
            session_id: "abc".to_string(),
            cwd: Some("D:\\a\\b".to_string()),
            title: Some("标题".to_string()),
            title_source: TitleSource::AiTitle,
            first_prompt: Some("prompt".to_string()),
            mtime_ms: 42,
            cwd_exists: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: HistorySession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, s.session_id);
        assert_eq!(back.cwd, s.cwd);
        assert_eq!(back.title, s.title);
        assert_eq!(back.title_source, s.title_source);
        assert_eq!(back.first_prompt, s.first_prompt);
        assert_eq!(back.mtime_ms, s.mtime_ms);
        assert_eq!(back.cwd_exists, s.cwd_exists);
    }

    // ── TitleSource serde camelCase 五变体 ──

    #[test]
    fn title_source_serialize_camelcase() {
        assert_eq!(
            serde_json::to_string(&TitleSource::CustomTitle).unwrap(),
            "\"customTitle\""
        );
        assert_eq!(
            serde_json::to_string(&TitleSource::AiTitle).unwrap(),
            "\"aiTitle\""
        );
        assert_eq!(
            serde_json::to_string(&TitleSource::Summary).unwrap(),
            "\"summary\""
        );
        assert_eq!(
            serde_json::to_string(&TitleSource::FirstPrompt).unwrap(),
            "\"firstPrompt\""
        );
        assert_eq!(
            serde_json::to_string(&TitleSource::None).unwrap(),
            "\"none\""
        );
    }

    #[test]
    fn title_source_deserialize_camelcase() {
        let v: TitleSource = serde_json::from_str("\"customTitle\"").unwrap();
        assert_eq!(v, TitleSource::CustomTitle);
        let v: TitleSource = serde_json::from_str("\"aiTitle\"").unwrap();
        assert_eq!(v, TitleSource::AiTitle);
        let v: TitleSource = serde_json::from_str("\"summary\"").unwrap();
        assert_eq!(v, TitleSource::Summary);
        let v: TitleSource = serde_json::from_str("\"firstPrompt\"").unwrap();
        assert_eq!(v, TitleSource::FirstPrompt);
        let v: TitleSource = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(v, TitleSource::None);
    }

    // ── is_uuid_filename（Stage 02 复用校验） ──

    #[test]
    fn is_uuid_filename_valid() {
        assert!(is_uuid_filename("123e4567-e89b-12d3-a456-426614174000"));
        // 大写 hex 也合法
        assert!(is_uuid_filename("123E4567-E89B-12D3-A456-426614174000"));
    }

    #[test]
    fn is_uuid_filename_invalid() {
        // 长度错误
        assert!(!is_uuid_filename(""));
        assert!(!is_uuid_filename("123e4567-e89b-12d3-a456-42661417400"));
        assert!(!is_uuid_filename("123e4567-e89b-12d3-a456-4266141740000"));
        // 连字符位置错误
        assert!(!is_uuid_filename("123e4567e89b-12d3-a456-426614174000"));
        assert!(!is_uuid_filename("123e4567-e89b12d3-a456-426614174000"));
        // 非 hex 字符
        assert!(!is_uuid_filename("123e4567-g89b-12d3-a456-426614174000"));
        assert!(!is_uuid_filename("123e4567-e89b-12d3-a456-42661417400z"));
        // 含下划线（agent 形态）
        assert!(!is_uuid_filename(
            "agent-123e4567-e89b-12d3-a456-426614174000"
        ));
    }
}
