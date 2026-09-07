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
use crate::agent_history::mock::MockCliHistoryProvider;
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

/// provider 身份盒（CP-041 防 ZST 地址共享）
///
/// 两个 provider 均为单元结构体（零尺寸）——Rust 不保证不同 ZST static 的地址
/// 互异（实测可共享同一地址），而 mod.rs `run_scan` 的 claude 身份比对依赖
/// 「provider 数据指针 == 注册表 claude 条目指针」：若 claude/mockcli 两静态
/// 实例地址合并，mockcli 的 force 扫描会被误判为 claude 而走进 claude 缓存
/// 通道（返回 claude 条目、打标 claude——E2E 实证）。经非 ZST 元组盒装载后
/// `.0` 字段地址必然唯一（盒自身占存储），身份比对恢复可靠。
static CLAUDE_BOX: (ClaudeHistoryProvider, u8) = (ClaudeHistoryProvider, 0);
static MOCK_BOX: (MockCliHistoryProvider, u8) = (MockCliHistoryProvider, 0);

/// 注册表条目：cliId → history provider（lifetime 泛型——静态注册表用 'static，
/// 测试注入桩用局部生命周期）
pub(crate) type ProviderEntry<'a> = (&'static str, &'a dyn CliHistoryProvider);

/// 基础注册表（生产形态，与 CP-041 落地前逐字一致——仅 claude）
static BASE_REGISTRY: &[ProviderEntry<'static>] = &[("claude", &CLAUDE_BOX.0)];

/// L4 形态注册表（env SLTERM_MOCKCLI_PROJECTS_DIR 存在时启用——run-wdio.cjs 注入）
static E2E_REGISTRY: &[ProviderEntry<'static>] =
    &[("claude", &CLAUDE_BOX.0), ("mockcli", &MOCK_BOX.0)];

/// 当前生效注册表（env 门控，CP-041）
///
/// `SLTERM_MOCKCLI_PROJECTS_DIR` 存在 → E2E 形态（含 mockcli 测试 provider）；
/// 否则基础形态（生产/日常二进制，mockcli 恒 Validation「未知 cliId」）。
/// 每次调用时读取 env（不缓存）——E2E 进程继承 env 即可生效。
pub(crate) fn registry() -> &'static [ProviderEntry<'static>] {
    if std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR").is_some() {
        E2E_REGISTRY
    } else {
        BASE_REGISTRY
    }
}

/// 解析 cliId → history provider（纯函数，无 IO；命令层分发唯一入口）
pub(crate) fn resolve_provider(cli_id: &str) -> Result<&'static dyn CliHistoryProvider, AppError> {
    registry()
        .iter()
        .find(|(id, _)| *id == cli_id)
        .map(|(_, p)| *p)
        .ok_or_else(|| AppError::Validation(format!("未知 cliId: {cli_id}")))
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    // ── 注册表 resolve（L1 新增：命中/未命中） ──

    /// 已知 cliId → 返回注册表内静态实例（身份断言——身份盒字段，非裸 ZST static）
    #[test]
    fn resolve_provider_known_cli_id_returns_registry_instance() {
        let p = resolve_provider("claude").unwrap();
        let expected: *const () = &CLAUDE_BOX.0 as *const ClaudeHistoryProvider as *const ();
        let got: *const () = p as *const dyn CliHistoryProvider as *const ();
        assert_eq!(got, expected, "应返回注册表中 claude 的静态实例");
    }

    /// provider 身份盒地址互异（CP-041 防 ZST 地址共享回归）：mod.rs run_scan
    /// 的 claude 身份比对（数据指针相等）依赖两 provider 实例地址唯一——若改回
    /// 裸单元结构体 static，mockcli force 扫描会被误判为 claude 缓存通道
    /// （E2E 实证），本用例锁死身份盒装载形态
    #[test]
    fn provider_identity_boxes_have_distinct_addresses() {
        let claude_addr = &CLAUDE_BOX.0 as *const ClaudeHistoryProvider as usize;
        let mock_addr = &MOCK_BOX.0 as *const MockCliHistoryProvider as usize;
        assert_ne!(
            claude_addr, mock_addr,
            "claude/mockcli provider 身份盒地址必须互异（非 ZST 槽位）"
        );
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

    // ── 注册表 env 门控（CP-041；env 修改经 MockRootGuard set/remove 成对，
    //    Drop 恢复原值——依赖 --test-threads=1 串行门禁） ──

    /// env 未设 → 基础注册表（仅 claude 条目）——生产形态守卫
    #[test]
    fn registry_env_absent_returns_base_only() {
        use crate::agent_history::mock::MockRootGuard;
        let _guard = MockRootGuard::unset();
        let r = registry();
        assert_eq!(r.len(), 1, "基础注册表应仅含 claude");
        assert_eq!(r[0].0, "claude");
        assert!(
            std::ptr::eq(r, BASE_REGISTRY),
            "env 未设应返回 BASE_REGISTRY"
        );
        // 生产形态守卫:mockcli 恒 Validation(由本用例锁死,env 无关路径不可达)
        match resolve_provider("mockcli").unwrap_err() {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}")
            }
            other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
        }
    }

    /// env 设置 → E2E 形态注册表（claude + mockcli 两条目）
    #[test]
    fn registry_present_e2e_env_includes_mockcli() {
        use crate::agent_history::mock::MockRootGuard;
        let dir = tempfile::tempdir().unwrap();
        let _guard = MockRootGuard::set(&dir.path());
        let r = registry();
        assert_eq!(r.len(), 2, "E2E 注册表应含 claude + mockcli");
        let ids: Vec<&str> = r.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, ["claude", "mockcli"]);
        assert!(std::ptr::eq(r, E2E_REGISTRY), "env 设置应返回 E2E_REGISTRY");
        // 门控形态:env 存在时 mockcli 可解析(E2E 历史链路前置)
        let p = resolve_provider("mockcli").unwrap();
        let expected: *const () = &MOCK_BOX.0 as *const MockCliHistoryProvider as *const ();
        let got: *const () = p as *const dyn CliHistoryProvider as *const ();
        assert_eq!(got, expected, "env 门控下应返回 mockcli 静态实例");
    }
}
