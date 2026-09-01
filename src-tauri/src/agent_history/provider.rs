//! 历史会话 provider 注册表 —— trait + 以 cliId 为键的静态注册表（MC-301）
//!
//! 跨边界契约（PREAMBLE 契约段 3，签名写死）：
//! - `CliHistoryProvider` trait 四方法：`scan() -> Vec<AgentHistorySession>` /
//!   `delete(session_id) -> Result<()>` / `validate_session_id(id) -> Result<()>` /
//!   `read_title(session_id) -> Result<AgentHistoryTitle>`（运行中会话标题通道）
//! - 契约注释写明「validate_session_id 是 delete / read_title 的强制前置」
//!   （SEC-05 等价强制，MC-304）——未来 provider 的等价校验强制
//! - 注册表 = cliId 键静态映射；claude 为首个实现（行为零改动）
//!
//! 错误语义（MC-303）：未知 cliId → `AppError::Validation("未知 cliId: ...")`

use crate::agent_history::claude::ClaudeHistoryProvider;
use crate::agent_history::{AgentHistorySession, AgentHistoryTitle};
use crate::error::AppError;

/// CLI 历史会话能力 trait（四方法，跨边界契约签名写死）
///
/// 实现均为同步阻塞（含 IO）——命令层经 `spawn_blocking` 串行化（硬约束 #3）。
pub trait CliHistoryProvider: Send + Sync + std::fmt::Debug {
    /// 扫描该 CLI 的全部历史会话元数据。
    ///
    /// 无 Err 通道：provider 内部失败降级为空/部分结果（照单文件降级条目契约），
    /// 聚合层「单 provider 失败不阻塞其他」由签名天然保证（MC-303）。
    fn scan(&self) -> Vec<AgentHistorySession>;

    /// 删除会话。
    ///
    /// **validate_session_id 是 delete 的强制前置**（SEC-05 等价，MC-304）：
    /// 命令层必须先调 `validate_session_id` 通过，才可调本方法；未来 provider
    /// 的等价校验在此强制。
    fn delete(&self, session_id: &str) -> Result<(), AppError>;

    /// 会话 ID 校验（delete / read_title 的强制前置校验；非法 → `AppError::Validation`）
    fn validate_session_id(&self, session_id: &str) -> Result<(), AppError>;

    /// 读取单会话标题（回退链合成——运行中会话页签/会话行显示名与历史扫描同源）。
    ///
    /// 会话文件不存在/无任何标题数据 → `Ok(AgentHistoryTitle { title: None, .. })`
    /// （运行中会话 jsonl 可能尚未创建，正常条件非错误）；仅 sessionId 非法 →
    /// `Err(Validation)`。**validate_session_id 是 read_title 的强制前置**。
    fn read_title(&self, session_id: &str) -> Result<AgentHistoryTitle, AppError>;
}

/// claude provider 静态实例（注册表条目引用）
static CLAUDE_PROVIDER: ClaudeHistoryProvider = ClaudeHistoryProvider;

/// 注册表条目：cliId → history provider（lifetime 泛型——静态注册表用 'static，
/// 测试注入桩用局部生命周期）
pub(crate) type ProviderEntry<'a> = (&'static str, &'a dyn CliHistoryProvider);

/// 静态注册表（cliId 键，跨边界契约「静态映射」形态；claude 为首个实现）
pub(crate) static REGISTRY: &[ProviderEntry<'static>] = &[("claude", &CLAUDE_PROVIDER)];

/// 解析 cliId → history provider（纯函数，无 IO；命令层分发唯一入口）
pub(crate) fn resolve_provider(cli_id: &str) -> Result<&'static dyn CliHistoryProvider, AppError> {
    REGISTRY
        .iter()
        .find(|(id, _)| *id == cli_id)
        .map(|(_, p)| *p)
        .ok_or_else(|| AppError::Validation(format!("未知 cliId: {cli_id}")))
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    // ── 注册表 resolve（L1 新增：命中/未命中） ──

    /// 已知 cliId → 返回注册表内静态实例（身份断言）
    #[test]
    fn resolve_provider_known_cli_id_returns_registry_instance() {
        let p = resolve_provider("claude").unwrap();
        let expected: *const () = &CLAUDE_PROVIDER as *const ClaudeHistoryProvider as *const ();
        let got: *const () = p as *const dyn CliHistoryProvider as *const ();
        assert_eq!(got, expected, "应返回注册表中 claude 的静态实例");
    }

    /// 未知 cliId → Validation（消息含「未知 cliId」语义）
    #[test]
    fn resolve_provider_unknown_cli_id_validation() {
        let err = resolve_provider("unknown-cli").unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}");
            }
            other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
        }
    }
}
