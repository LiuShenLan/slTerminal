//! 历史会话聚合层 —— provider 注册表 + 无参聚合命令（Stage 04，MC-301~305）
//!
//! 职责：
//! - DTO 定义：`AgentHistorySession`（IPC 契约八字段，serde camelCase）
//! - `is_uuid_filename`：UUID 形态纯校验（可复用工具，provider 共用）
//! - `provider.rs`：`CliHistoryProvider` trait + cliId 键静态注册表
//! - `claude/`：claude history provider（scan/jsonl/ops 整体下沉，行为零改动）
//! - 两条泛化命令：`agent_history_scan`（无参聚合）/ `agent_history_delete(cliId, sessionId)`
//!
//! 聚合语义（MC-303）：`agent_history_scan` 遍历全部已注册 provider 串行聚合；
//! 单 provider 失败不阻塞其他（`scan()` 无 Err 通道——失败语义 = provider 内部降级为
//! 空/部分结果，照单文件降级条目契约的语义层级提升）；全部空 → 空数组（Ok 非 Err）。
//! 聚合层不假设任何 provider 的 env 命名（MC-305：`SLTERM_<CLI>_PROJECTS_DIR` 类
//! env 由各 provider 内部自管）。

pub mod claude;
pub mod provider;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use provider::{resolve_provider, CliHistoryProvider, REGISTRY};

/// 历史会话元数据 DTO（IPC 契约八字段，serde camelCase，硬约束 #4）
///
/// `title_source` 为开放字符串（MC-302）：claude 值集
/// `customTitle`/`aiTitle`/`summary`/`firstPrompt`/`none`，UI 不消费具体值。
/// `cli_id` 由 provider 打标（claude provider 产出 `"claude"`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHistorySession {
    /// 会话 ID（claude：文件名主干 = UUID）
    pub session_id: String,
    /// 会话启动时工作目录（从 JSONL 内容解析，不反解码目录名）
    pub cwd: Option<String>,
    /// 标题（回退链合成结果；全无时为 None）
    pub title: Option<String>,
    /// 标题来源开放字符串（claude 值集：customTitle/aiTitle/summary/firstPrompt/none）
    pub title_source: String,
    /// 首条可见 user prompt（≤200 字符）
    pub first_prompt: Option<String>,
    /// 文件修改时间（毫秒时间戳）
    pub mtime_ms: u64,
    /// cwd 目录当前是否存在（cwd 为 null 时恒 false）
    pub cwd_exists: bool,
    /// CLI 标识（provider 打标，聚合后前端按 cliId 区分来源）
    pub cli_id: String,
}

/// 校验字符串是否为 UUID 形态（`^[0-9a-fA-F]{8}-...-{12}$`）
///
/// 可复用工具（MC-301）：scan 排除非会话文件、provider validate_session_id 校验复用。
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

/// agent_history_scan — 无参聚合扫描全部已注册 provider 的历史会话（MC-303）
///
/// 阻塞 I/O 全部在 spawn_blocking 内执行（硬约束 #3）。
/// 单 provider 失败不阻塞其他（`scan()` 无 Err 通道）；全部空 → 空数组（Ok 非 Err）。
#[tauri::command]
pub async fn agent_history_scan() -> Result<Vec<AgentHistorySession>, AppError> {
    tokio::task::spawn_blocking(scan_all)
        .await
        .map_err(AppError::from)
}

/// 聚合扫描核心（注册表可注入，测试构造多 provider 桩直测）
///
/// 串行按注册表顺序聚合；`scan()` 无 Err 通道——provider 内部失败降级为空/部分
/// 结果，聚合循环天然不阻塞其他 provider（MC-303 语义层级提升）。
fn scan_all_with(registry: &[provider::ProviderEntry<'_>]) -> Vec<AgentHistorySession> {
    let mut all = Vec::new();
    for (_, p) in registry {
        all.extend(p.scan());
    }
    all
}

/// 遍历静态注册表全部 provider 聚合（命令层 spawn_blocking 入口）
fn scan_all() -> Vec<AgentHistorySession> {
    scan_all_with(REGISTRY)
}

/// agent_history_delete — 按 cliId 分发删除会话（MC-303）
///
/// 未知 cliId → `AppError::Validation`（resolve_provider 统一产出）。
/// 删除前经该 provider `validate_session_id` 前置（SEC-05 等价强制，MC-304）。
#[tauri::command]
pub async fn agent_history_delete(cli_id: String, session_id: String) -> Result<(), AppError> {
    let provider = resolve_provider(&cli_id)?;
    run_delete(provider, session_id).await
}

/// 删除核心（provider 注入，测试可直测 validate 前置）
///
/// **validate_session_id 是 delete 的强制前置**（trait 契约，SEC-05 等价）：
/// 先经 provider 校验 sessionId，通过后才在 spawn_blocking 内执行删除。
pub(crate) async fn run_delete(
    provider: &'static dyn CliHistoryProvider,
    session_id: String,
) -> Result<(), AppError> {
    provider.validate_session_id(&session_id)?;
    tokio::task::spawn_blocking(move || provider.delete(&session_id))
        .await
        .map_err(AppError::from)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_history::claude::ScanRootGuard;
    use std::sync::atomic::{AtomicBool, Ordering};

    // ── AgentHistorySession serde camelCase（八键契约，防字段漂移） ──

    #[test]
    fn history_session_serialize_camelcase_eight_keys() {
        let s = AgentHistorySession {
            session_id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            cwd: Some("C:\\proj".to_string()),
            title: Some("修复登录 bug".to_string()),
            title_source: "customTitle".to_string(),
            first_prompt: Some("你好".to_string()),
            mtime_ms: 1_752_500_000_000,
            cwd_exists: true,
            cli_id: "claude".to_string(),
        };
        let json = serde_json::to_value(&s).unwrap();
        let obj = json.as_object().unwrap();
        // 键集合恰为八键（无多余键）
        let expected: std::collections::BTreeSet<&str> = [
            "sessionId",
            "cwd",
            "title",
            "titleSource",
            "firstPrompt",
            "mtimeMs",
            "cwdExists",
            "cliId",
        ]
        .into_iter()
        .collect();
        let actual: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
        assert_eq!(actual, expected, "AgentHistorySession 键集合应与契约一致");
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
            "cwdExists":false,
            "cliId":"claude"
        }"#;
        let s: AgentHistorySession = serde_json::from_str(json).unwrap();
        assert_eq!(s.session_id, "123e4567-e89b-12d3-a456-426614174000");
        assert!(s.cwd.is_none());
        assert!(s.title.is_none());
        assert_eq!(s.title_source, "none");
        assert!(s.first_prompt.is_none());
        assert_eq!(s.mtime_ms, 0);
        assert!(!s.cwd_exists);
        assert_eq!(s.cli_id, "claude");
    }

    #[test]
    fn history_session_roundtrip_all_fields() {
        let s = AgentHistorySession {
            session_id: "abc".to_string(),
            cwd: Some("D:\\a\\b".to_string()),
            title: Some("标题".to_string()),
            title_source: "aiTitle".to_string(),
            first_prompt: Some("prompt".to_string()),
            mtime_ms: 42,
            cwd_exists: true,
            cli_id: "claude".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: AgentHistorySession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, s.session_id);
        assert_eq!(back.cwd, s.cwd);
        assert_eq!(back.title, s.title);
        assert_eq!(back.title_source, s.title_source);
        assert_eq!(back.first_prompt, s.first_prompt);
        assert_eq!(back.mtime_ms, s.mtime_ms);
        assert_eq!(back.cwd_exists, s.cwd_exists);
        assert_eq!(back.cli_id, s.cli_id);
    }

    // ── is_uuid_filename（可复用工具，MC-301） ──

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

    // ── 聚合 scan（L1 新增：多 provider 桩 / 单 provider 失败不阻塞） ──

    /// 测试桩 provider（注册表注入直测聚合语义）
    #[derive(Debug)]
    struct StubProvider {
        sessions: Vec<AgentHistorySession>,
    }

    impl CliHistoryProvider for StubProvider {
        fn scan(&self) -> Vec<AgentHistorySession> {
            self.sessions.clone()
        }
        fn delete(&self, _session_id: &str) -> Result<(), AppError> {
            Ok(())
        }
        fn validate_session_id(&self, _session_id: &str) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// 构造最小会话条目（sessionId/cliId 打标）
    fn stub_session(id: &str, cli_id: &str) -> AgentHistorySession {
        AgentHistorySession {
            session_id: id.to_string(),
            cwd: None,
            title: None,
            title_source: "none".to_string(),
            first_prompt: None,
            mtime_ms: 0,
            cwd_exists: false,
            cli_id: cli_id.to_string(),
        }
    }

    #[test]
    fn scan_aggregates_all_registered_providers() {
        // 多 provider 桩 → 串行按注册表顺序聚合全部条目
        let p1 = StubProvider {
            sessions: vec![stub_session("a", "cli1")],
        };
        let p2 = StubProvider {
            sessions: vec![stub_session("b", "cli2"), stub_session("c", "cli2")],
        };
        let registry: &[provider::ProviderEntry<'_>] = &[("cli1", &p1), ("cli2", &p2)];
        let all = scan_all_with(registry);
        assert_eq!(all.len(), 3, "应聚合全部 provider 的条目");
        let ids: Vec<&str> = all.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"], "串行按注册表顺序聚合");
        // cliId 打标随 provider 条目保留
        assert_eq!(all[2].cli_id, "cli2");
    }

    #[test]
    fn scan_single_provider_failure_does_not_block_others() {
        // 单 provider「失败」（scan 无 Err 通道——失败语义 = 内部降级为空）不阻塞其他
        let failed = StubProvider { sessions: vec![] };
        let ok = StubProvider {
            sessions: vec![stub_session("b", "cli2")],
        };
        let registry: &[provider::ProviderEntry<'_>] = &[("failed", &failed), ("cli2", &ok)];
        let all = scan_all_with(registry);
        assert_eq!(all.len(), 1, "失败 provider 不阻塞后续 provider 聚合");
        assert_eq!(all[0].session_id, "b");
    }

    // ── delete 命令层（L1 新增：未知 cliId Validation / validate_session_id 前置） ──

    /// 手动 current_thread runtime 驱动 async 命令核心（tokio 未启用 #[tokio::test]，
    /// 照 hooks/usage.rs block_on 先例）
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// 记录调用序的桩 provider（validate 前置测试用；static 须 Sync，用 AtomicBool）
    #[derive(Debug)]
    struct RecordingStub {
        delete_called: AtomicBool,
    }

    impl CliHistoryProvider for RecordingStub {
        fn scan(&self) -> Vec<AgentHistorySession> {
            Vec::new()
        }
        fn delete(&self, _session_id: &str) -> Result<(), AppError> {
            self.delete_called.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn validate_session_id(&self, session_id: &str) -> Result<(), AppError> {
            // 照 claude 语义：仅 UUID 形态合法（SEC-05 等价）
            if is_uuid_filename(session_id) {
                Ok(())
            } else {
                Err(AppError::Validation(format!(
                    "非法 sessionId: {session_id}"
                )))
            }
        }
    }

    #[test]
    fn delete_validates_session_id_before_delete() {
        // validate_session_id 是 delete 的强制前置：validate 拒绝 → delete 不应被调用
        static STUB: RecordingStub = RecordingStub {
            delete_called: AtomicBool::new(false),
        };
        let err = block_on(run_delete(&STUB, "../evil".to_string())).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("非法 sessionId"), "消息应含校验文案: {msg}");
            }
            other => panic!("非法 sessionId 应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !STUB.delete_called.load(Ordering::SeqCst),
            "validate 拒绝时 delete 不应被调用"
        );
    }

    #[test]
    fn delete_unknown_cli_id_returns_validation() {
        // 未知 cliId → Validation（resolve_provider 统一产出，消息含「未知 cliId」语义）
        let err = block_on(agent_history_delete(
            "nope".to_string(),
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
        ))
        .unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}");
            }
            other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
        }
    }

    // ── 命令包装层（迁移自 scan.rs/ops.rs：直接 await #[tauri::command] fn） ──

    const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";

    /// 创建扫描根 + 一个编码目录（cwd 编码形态，如 C--Users-test-app）
    /// 返回 (TempDir 守卫, 规范化扫描根路径, 编码目录路径)
    ///
    /// 路径经 dunce::canonicalize 统一长名（8.3 短名坑，照 git/CLAUDE.md 先例）。
    fn make_scan_root() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let proj = root.join("C--Users-test-app");
        std::fs::create_dir_all(&proj).unwrap();
        (dir, root, proj)
    }

    /// 在编码目录下写一个有效会话文件（UUID 文件名 + summary 首行 + user prompt 行）
    /// 内容经 serde_json 序列化保证 JSON 转义正确（Windows 路径含反斜杠）
    fn write_valid_session(proj: &std::path::Path, uuid: &str) {
        let content = serde_json::json!({
            "type": "summary",
            "summary": "修复登录 bug",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": "C:\\test\\app",
                "message": { "content": "帮我修 bug" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();
    }

    #[test]
    fn command_scan_wraps_spawn_blocking_and_returns_sessions() {
        // 包装层最小用例（HFN-05/D6）：spawn_blocking + await + map_err 全链路，内容透传
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);

        let _guard = ScanRootGuard::set(&root);
        let sessions = block_on(agent_history_scan()).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
        assert_eq!(
            sessions[0].cli_id, "claude",
            "claude provider 产出条目应打标 cliId"
        );
    }

    #[test]
    fn command_scan_degraded_root_returns_empty_ok() {
        // 包装层 + IO 降级（HFN-05）：扫描根不存在 → 命令仍 Ok(空)，不把降级变 Err
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在");
        let _guard = ScanRootGuard::set(&missing);
        let sessions = block_on(agent_history_scan()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn command_delete_wraps_spawn_blocking_and_passes_params() {
        // 包装层最小用例（HFN-05/D6）：cliId/sessionId 透传 spawn_blocking → 文件真实删除
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "{}").unwrap();

        let _guard = ScanRootGuard::set(&root);
        block_on(agent_history_delete("claude".to_string(), UUID.to_string())).unwrap();

        assert!(!proj.join(format!("{UUID}.jsonl")).exists());
    }

    #[test]
    fn command_delete_invalid_id_returns_validation() {
        // 包装层 + 错误映射（HFN-05/D6）：非法 id 经 validate 前置校验失败 → Err(Validation)
        // 透传；env 指向 tempdir——即使校验回归（越界）也只触碰隔离目录
        let (_dir, root, _proj) = make_scan_root();
        let _guard = ScanRootGuard::set(&root);
        let err = block_on(agent_history_delete(
            "claude".to_string(),
            "../evil".to_string(),
        ))
        .unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
            }
            other => panic!("非法 sessionId 应返回 Validation，实际: {other:?}"),
        }
    }
}
