//! claude history provider（MC-301/304/305 下沉）
//!
//! provider 内部是 claude 合法领地（照 hooks/claude/ 先例）：claude 命名与 claude
//! 知识全部保留（~/.claude/projects/、SLTERM_CLAUDE_PROJECTS_DIR env 覆盖、
//! transcript JSONL 解析窗口、custom-title/ai-title 语义、UUID sessionId 校验）。
//!
//! 对外暴露：
//! - `ClaudeHistoryProvider`：`CliHistoryProvider` trait 实现（注册表条目）
//! - `TitleSource`：标题来源五态（claude 值集，provider 内部类型；DTO 层为开放字符串）
//! - `ScanRootGuard`（cfg test）：`SLTERM_CLAUDE_PROJECTS_DIR` env 测试守卫（MC-305）

pub mod jsonl;
pub mod ops;
pub mod scan;

use crate::agent_history::provider::CliHistoryProvider;
use crate::agent_history::AgentHistorySession;
use crate::error::AppError;

/// 标题来源（claude 值集五态，provider 内部类型；DTO 层为开放字符串，MC-302）
///
/// 序列化值为 `"customTitle"` / `"aiTitle"` / `"summary"` / `"firstPrompt"` / `"none"`
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl TitleSource {
    /// claude 值集 → 开放字符串（DTO `title_source` 字段值；UI 不消费具体值，MC-302）
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            TitleSource::CustomTitle => "customTitle",
            TitleSource::AiTitle => "aiTitle",
            TitleSource::Summary => "summary",
            TitleSource::FirstPrompt => "firstPrompt",
            TitleSource::None => "none",
        }
    }
}

/// claude history provider（单元结构，静态注册表条目）
#[derive(Debug)]
pub struct ClaudeHistoryProvider;

impl CliHistoryProvider for ClaudeHistoryProvider {
    fn scan(&self) -> Vec<AgentHistorySession> {
        // scan_sessions 内部完成 cli_id: "claude" 打标（provider 内部写字面量合法，MC-302）
        scan::scan_sessions()
    }

    fn delete(&self, session_id: &str) -> Result<(), AppError> {
        // delete_session 内部自带 validate_session_id 兜底（零行为改动，SEC-05 保留）
        ops::delete_session(session_id)
    }

    fn validate_session_id(&self, session_id: &str) -> Result<(), AppError> {
        ops::validate_session_id(session_id)
    }

    fn read_title(
        &self,
        session_id: &str,
    ) -> Result<crate::agent_history::AgentHistoryTitle, AppError> {
        // 回退链与 scan 的 parse_session_file 同源（运行中会话标题通道）
        ops::read_session_title(session_id)
    }
}

/// `SLTERM_CLAUDE_PROJECTS_DIR` 环境变量守卫（HFN-06，MC-305：env 覆盖留 provider 内部）
///
/// set/unset 后无论测试成功或 panic，Drop 时均恢复原 env 值（原无 → 移除），
/// 不残留污染后续用例。依赖 --test-threads=1 门禁（env 全局可变，并行测试互污染）。
#[cfg(test)]
pub(crate) struct ScanRootGuard(Option<std::ffi::OsString>);

#[cfg(test)]
impl ScanRootGuard {
    /// 设置 env 为给定值（路径 / 空串均可），Drop 时恢复原值
    pub(crate) fn set(value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR");
        std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", value);
        ScanRootGuard(prev)
    }

    /// 移除 env（等价未设），Drop 时恢复原值
    pub(crate) fn unset() -> Self {
        let prev = std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR");
        std::env::remove_var("SLTERM_CLAUDE_PROJECTS_DIR");
        ScanRootGuard(prev)
    }
}

#[cfg(test)]
impl Drop for ScanRootGuard {
    fn drop(&mut self) {
        match &self.0 {
            Some(v) => std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", v),
            None => std::env::remove_var("SLTERM_CLAUDE_PROJECTS_DIR"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TitleSource serde camelCase 五变体（claude 值集） ──

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

    // ── as_str：值集 → 开放字符串（MC-302 DTO title_source 字段值来源） ──

    #[test]
    fn title_source_as_str_maps_value_set() {
        assert_eq!(TitleSource::CustomTitle.as_str(), "customTitle");
        assert_eq!(TitleSource::AiTitle.as_str(), "aiTitle");
        assert_eq!(TitleSource::Summary.as_str(), "summary");
        assert_eq!(TitleSource::FirstPrompt.as_str(), "firstPrompt");
        assert_eq!(TitleSource::None.as_str(), "none");
    }

    // ── ScanRootGuard RAII（HFN-06） ──

    #[test]
    fn scan_root_guard_restores_previous_env_on_drop() {
        // 守卫 Drop 恢复原 env（HFN-06：set 后 panic 不残留）；外层 guard 保护本测试自身
        let _outer = ScanRootGuard::set("C:\\guard-prev");
        {
            let _g = ScanRootGuard::set("C:\\guard-new");
            assert_eq!(
                std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR"),
                Some(std::ffi::OsString::from("C:\\guard-new"))
            );
        }
        assert_eq!(
            std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR"),
            Some(std::ffi::OsString::from("C:\\guard-prev")),
            "Drop 后应恢复原 env 值"
        );
    }
}
