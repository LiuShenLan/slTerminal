//! CLI hooks provider 注册表 —— trait + 以 cliId 为键的静态注册表
//!
//! 跨边界契约（PREAMBLE 契约段 4）：
//! - `CliHooksProvider` trait 六方法，签名写死
//! - 注册表 = cliId 键静态映射；claude 为首个实现（行为零改动）
//!
//! 错误语义（MC-211）：
//! - 未知 cliId → `AppError::Validation("未知 cliId: ...")`
//! - 已注册但无 hooks 能力 → `AppError::Validation`（消息含「不支持 hooks 能力」语义，
//!   本期注册表仅 claude，走不到第二分支，但分支与测试建好）

use serde_json::Value;
use std::path::PathBuf;

use crate::error::AppError;

use super::claude::ClaudeHooksProvider;
use super::{AgentHookInjectionStatus, ContextUsage};

/// CLI hooks 能力 trait（六方法，跨边界契约签名写死）
///
/// 实现均为同步阻塞（含 IO）——命令层经 `spawn_blocking` 串行化（硬约束 #3）。
/// 仅 hooks 能力的 CLI 在此实现；无 hooks 能力的 CLI 不实现本 trait
/// （resolve 时走「不支持 hooks 能力」Validation 分支）。
pub trait CliHooksProvider: Send + Sync + std::fmt::Debug {
    /// 注入 hooks（脚本落盘 + settings.json merge 注入）
    fn inject(&self) -> Result<AgentHookInjectionStatus, AppError>;
    /// 卸载 hooks（移除配置段 + 删脚本目录 + 清信号目录）
    fn uninstall(&self) -> Result<(), AppError>;
    /// 查询注入状态（Injected/NotInjected/Outdated 三态）
    fn injection_status(&self) -> Result<AgentHookInjectionStatus, AppError>;
    /// 查询 transcript token 用量（无 usage 或文件异常 → Ok(None)）
    fn context_usage(&self, transcript_path: &str) -> Result<Option<ContextUsage>, AppError>;
    /// 读取指定层 hooks 配置子树（project/local 层经 project_root 路径沙箱校验）
    fn config_read(
        &self,
        layer: &str,
        project_path: Option<&str>,
        project_root: &Option<PathBuf>,
    ) -> Result<Value, AppError>;
    /// 写回指定层 hooks 配置子树（read-modify-write merge）
    fn config_write(
        &self,
        layer: &str,
        hooks: Value,
        project_path: Option<&str>,
        project_root: &Option<PathBuf>,
    ) -> Result<(), AppError>;
}

/// claude provider 静态实例（注册表条目引用）
static CLAUDE_PROVIDER: ClaudeHooksProvider = ClaudeHooksProvider;

/// 注册表条目：cliId → hooks provider。
/// Some = 支持 hooks 能力；None = 已注册但无 hooks 能力（预留分支，本期无此类条目）。
type ProviderEntry = (&'static str, Option<&'static dyn CliHooksProvider>);

/// 静态注册表（cliId 键，跨边界契约「静态映射」形态）
static REGISTRY: &[ProviderEntry] = &[("claude", Some(&CLAUDE_PROVIDER))];

/// 解析 cliId → hooks provider（纯函数，无 IO；命令层分发唯一入口）
pub(crate) fn resolve_provider(cli_id: &str) -> Result<&'static dyn CliHooksProvider, AppError> {
    lookup_provider(cli_id, REGISTRY)
}

/// 注册表查找核心（注册表注入供测试构造「无 hooks 能力」条目）
pub(crate) fn lookup_provider<'a>(
    cli_id: &str,
    registry: &'a [ProviderEntry],
) -> Result<&'a dyn CliHooksProvider, AppError> {
    match registry.iter().find(|(id, _)| *id == cli_id) {
        Some((_, Some(p))) => Ok(*p),
        Some((_, None)) => Err(AppError::Validation(format!(
            "cliId {cli_id} 已注册但不支持 hooks 能力"
        ))),
        None => Err(AppError::Validation(format!("未知 cliId: {cli_id}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 注册表 get（L1 新增：resolve_provider 命中/未命中） ──

    /// 已知 cliId → 返回注册表内静态实例（身份断言）
    #[test]
    fn resolve_provider_known_cli_id_returns_registry_instance() {
        let p = resolve_provider("claude").unwrap();
        let expected: *const () = &CLAUDE_PROVIDER as *const ClaudeHooksProvider as *const ();
        let got: *const () = p as *const dyn CliHooksProvider as *const ();
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

    /// 已注册但无 hooks 能力 → Validation（含「不支持 hooks 能力」语义）；
    /// 未注册 → 未知 cliId（两分支经注册表注入直测）
    #[test]
    fn lookup_registered_without_hooks_capability_validation() {
        let registry: &[ProviderEntry] = &[("nohooks", None)];
        let err = lookup_provider("nohooks", registry).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("不支持 hooks 能力"),
                    "消息应含「不支持 hooks 能力」: {msg}"
                );
            }
            other => panic!("无 hooks 能力 cliId 应返回 Validation，实际: {other:?}"),
        }
        let err = lookup_provider("ghost", registry).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "未注册应走未知分支: {msg}");
            }
            other => panic!("未注册 cliId 应返回 Validation，实际: {other:?}"),
        }
    }
}
