//! 历史会话聚合层 —— provider 注册表 + cliId 分发命令（Stage 04，MC-301~305；BE-19 缓存）
//!
//! 职责：
//! - DTO 定义：`AgentHistorySession`（IPC 契约八字段，serde camelCase）
//! - `is_uuid_filename`：UUID 形态纯校验（可复用工具，provider 共用）
//! - `provider.rs`：`CliHistoryProvider` trait + cliId 键静态注册表
//! - `claude/`：claude history provider（scan/jsonl/ops 整体下沉，行为零改动；
//!   scan 带 BE-19 进程内缓存——键 (目录 mtime, 文件数)）
//! - 泛化命令：`agent_history_scan(cliId, force)`（按 cliId 分发 + force 绕过缓存）/
//!   `agent_history_delete(cliId, sessionId)` / `agent_history_read_title(cliId, sessionId)`
//!
//! 分发语义（MC-303 + BE-19）：`agent_history_scan(cliId, force)` 按 cliId 分发单
//! provider 扫描（非无参聚合）；`force=true` 绕过 provider 进程内缓存强制重扫
//! （前端显式刷新/恢复完成后传 true，FE-19 联动）。
//! 单 provider 失败不报错（`scan()` 无 Err 通道——失败语义 = provider 内部降级为
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

/// 单会话标题 DTO（IPC 契约两键，serde camelCase，硬约束 #4）
///
/// 供运行中会话页签/导航树行取与历史 session 同源的标题
/// （`agent_history_read_title`——人工验证问题 3）。`title` 为 None 表示回退链
/// 四路全无（文件缺失/无任何标题数据）——前端兜底 CLI 名（claude 等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHistoryTitle {
    /// 标题（回退链合成结果；全无时为 None）
    pub title: Option<String>,
    /// 标题来源开放字符串（claude 值集：customTitle/aiTitle/summary/firstPrompt/none）
    pub title_source: String,
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

/// agent_history_scan — 按 cliId 分发扫描单 provider 历史会话（BE-19 契约）
///
/// 签名契约：`agent_history_scan(cli_id: String, force: Option<bool>)`；
/// `force=true` 绕过进程内缓存强制重扫（前端显式刷新/恢复完成后传 true，FE-19 联动）。
/// 未知 cliId → `AppError::Validation`（resolve_provider 统一产出，与 delete/read_title 一致）。
/// 阻塞 I/O 全部在 spawn_blocking 内执行（硬约束 #3）。
/// 单 provider 失败不报错（`scan()` 无 Err 通道——失败语义 = provider 内部降级为
/// 空/部分结果）；全部空 → 空数组（Ok 非 Err）。
#[tauri::command]
pub async fn agent_history_scan(
    cli_id: String,
    force: Option<bool>,
) -> Result<Vec<AgentHistorySession>, AppError> {
    let provider = resolve_provider(&cli_id)?;
    let force = force.unwrap_or(false);
    tokio::task::spawn_blocking(move || run_scan(provider, force))
        .await
        .map_err(AppError::from)
}

/// 单 provider 扫描核心（provider 注入，测试可直测）
///
/// BE-19 缓存契约：`force=true` 绕过缓存强制重扫。trait `scan()` 无 force 通道——
/// claude provider 的缓存层在 claude/scan.rs（`scan_sessions_with_force`），经注册表
/// cliId 身份比对（数据指针相等，照 provider.rs 测试先例）确认后直达其 force 入口；
/// 其余 provider 无缓存层 → force 无含义，恒走 trait `scan()`。
pub(crate) fn run_scan(
    provider: &'static dyn CliHistoryProvider,
    force: bool,
) -> Vec<AgentHistorySession> {
    if force && is_claude_provider(provider) {
        return claude::scan::scan_sessions_with_force(true);
    }
    provider.scan()
}

/// provider 是否注册表 claude 实例（数据指针身份比对——REGISTRY 静态实例）
fn is_claude_provider(provider: &'static dyn CliHistoryProvider) -> bool {
    let claude = REGISTRY
        .iter()
        .find(|(id, _)| *id == "claude")
        .map(|(_, p)| *p)
        .expect("注册表恒含 claude 条目");
    provider as *const dyn CliHistoryProvider as *const ()
        == claude as *const dyn CliHistoryProvider as *const ()
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

/// agent_history_read_title — 读取单会话标题（运行中会话页签/会话行显示名）
///
/// 回退链与历史扫描同源（custom-title > ai-title > summary > firstPrompt）。
/// 未知 cliId → `AppError::Validation`（resolve_provider 统一产出）。
/// 阻塞 I/O 全部在 spawn_blocking 内执行（硬约束 #3）。
#[tauri::command]
pub async fn agent_history_read_title(
    cli_id: String,
    session_id: String,
) -> Result<AgentHistoryTitle, AppError> {
    let provider = resolve_provider(&cli_id)?;
    run_read_title(provider, session_id).await
}

/// 读标题核心（provider 注入，测试可直测 validate 前置）
///
/// **validate_session_id 是 read_title 的强制前置**（trait 契约，SEC-05 等价）：
/// 先经 provider 校验 sessionId，通过后才在 spawn_blocking 内执行读取。
/// 会话文件不存在 → provider 返回 `Ok(title: None)`（正常条件非错误）。
pub(crate) async fn run_read_title(
    provider: &'static dyn CliHistoryProvider,
    session_id: String,
) -> Result<AgentHistoryTitle, AppError> {
    provider.validate_session_id(&session_id)?;
    tokio::task::spawn_blocking(move || provider.read_title(&session_id))
        .await
        .map_err(AppError::from)?
}

#[cfg(test)]
mod agent_history_tests {
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
        scan_called: AtomicBool,
        delete_called: AtomicBool,
        read_called: AtomicBool,
    }

    impl CliHistoryProvider for RecordingStub {
        fn scan(&self) -> Vec<AgentHistorySession> {
            self.scan_called.store(true, Ordering::SeqCst);
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
        fn read_title(&self, session_id: &str) -> Result<AgentHistoryTitle, AppError> {
            self.read_called.store(true, Ordering::SeqCst);
            Ok(AgentHistoryTitle {
                title: Some(format!("标题 {session_id}")),
                title_source: "customTitle".to_string(),
            })
        }
    }

    #[test]
    fn delete_validates_session_id_before_delete() {
        // validate_session_id 是 delete 的强制前置：validate 拒绝 → delete 不应被调用
        static STUB: RecordingStub = RecordingStub {
            scan_called: AtomicBool::new(false),
            delete_called: AtomicBool::new(false),
            read_called: AtomicBool::new(false),
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

    // ── AgentHistoryTitle serde camelCase（两键契约，防字段漂移） ──

    #[test]
    fn history_title_serialize_camelcase_two_keys() {
        let t = AgentHistoryTitle {
            title: Some("修复登录 bug".to_string()),
            title_source: "customTitle".to_string(),
        };
        let json = serde_json::to_value(&t).unwrap();
        let obj = json.as_object().unwrap();
        let expected: std::collections::BTreeSet<&str> =
            ["title", "titleSource"].into_iter().collect();
        let actual: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
        assert_eq!(actual, expected, "AgentHistoryTitle 键集合应与契约一致");
    }

    #[test]
    fn history_title_deserialize_none_roundtrip() {
        let json = r#"{"title":null,"titleSource":"none"}"#;
        let t: AgentHistoryTitle = serde_json::from_str(json).unwrap();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
        let back: AgentHistoryTitle =
            serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert!(back.title.is_none());
        assert_eq!(back.title_source, "none");
    }

    // ── read_title 命令层（运行中会话标题通道） ──

    #[test]
    fn read_title_dispatches_to_provider_and_returns_title() {
        // 命令核心经 provider 分发：validate 通过 → read_title 调用 → 结果透传
        static STUB: RecordingStub = RecordingStub {
            scan_called: AtomicBool::new(false),
            delete_called: AtomicBool::new(false),
            read_called: AtomicBool::new(false),
        };
        let t = block_on(run_read_title(&STUB, UUID.to_string())).unwrap();
        assert!(
            STUB.read_called.load(Ordering::SeqCst),
            "read_title 应被调用"
        );
        assert_eq!(t.title.as_deref(), Some(format!("标题 {UUID}").as_str()));
        assert_eq!(t.title_source, "customTitle");
    }

    #[test]
    fn read_title_validates_session_id_before_read() {
        // validate_session_id 是 read_title 的强制前置：validate 拒绝 → read_title 不应被调用
        static STUB: RecordingStub = RecordingStub {
            scan_called: AtomicBool::new(false),
            delete_called: AtomicBool::new(false),
            read_called: AtomicBool::new(false),
        };
        let err = block_on(run_read_title(&STUB, "../evil".to_string())).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("非法 sessionId"), "消息应含校验文案: {msg}");
            }
            other => panic!("非法 sessionId 应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !STUB.read_called.load(Ordering::SeqCst),
            "validate 拒绝时 read_title 不应被调用"
        );
    }

    #[test]
    fn read_title_unknown_cli_id_returns_validation() {
        // 未知 cliId → Validation（resolve_provider 统一产出，消息含「未知 cliId」语义）
        let err = block_on(agent_history_read_title(
            "nope".to_string(),
            UUID.to_string(),
        ))
        .unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}");
            }
            other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
        }
    }

    #[test]
    fn command_read_title_wraps_spawn_blocking_and_returns_resolved_title() {
        // 包装层最小用例：spawn_blocking + await + map_err 全链路——summary 首行
        // → 回退链落位 summary（真实 claude provider + ScanRootGuard tempdir）
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);

        let _guard = ScanRootGuard::set(&root);
        let t = block_on(agent_history_read_title(
            "claude".to_string(),
            UUID.to_string(),
        ))
        .unwrap();

        assert_eq!(t.title.as_deref(), Some("修复登录 bug"));
        assert_eq!(t.title_source, "summary");
    }

    #[test]
    fn command_read_title_missing_file_returns_none_ok() {
        // 会话文件不存在（运行中会话尚未落盘）→ Ok(title: None)——正常条件非 Err
        let (_dir, root, _proj) = make_scan_root();
        let _guard = ScanRootGuard::set(&root);
        let t = block_on(agent_history_read_title(
            "claude".to_string(),
            UUID.to_string(),
        ))
        .unwrap();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
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
        let sessions = block_on(agent_history_scan("claude".to_string(), None)).unwrap();

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
        let sessions = block_on(agent_history_scan("claude".to_string(), None)).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn command_scan_unknown_cli_id_returns_validation() {
        // 未知 cliId → Validation（resolve_provider 统一产出，与 delete/read_title 一致）
        let err = block_on(agent_history_scan("nope".to_string(), None)).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知 cliId"), "消息应含「未知 cliId」: {msg}");
            }
            other => panic!("未知 cliId 应返回 Validation，实际: {other:?}"),
        }
    }

    #[test]
    fn command_scan_force_true_bypasses_cache() {
        // BE-19 契约：默认走进程内缓存（键不变命中复用，不重复读盘）；force=true 绕过强制重扫
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);
        let _guard = ScanRootGuard::set(&root);

        // 首次扫描 → 缓存回填 1 条
        let first = block_on(agent_history_scan("claude".to_string(), None)).unwrap();
        assert_eq!(first.len(), 1);

        // 删除会话文件（根键不变：目录 mtime/一级条目数均未变）→ 默认走缓存 → 仍返回旧结果
        std::fs::remove_file(proj.join(format!("{UUID}.jsonl"))).unwrap();
        let cached = block_on(agent_history_scan("claude".to_string(), None)).unwrap();
        assert_eq!(cached.len(), 1, "键不变缓存命中——不重复读盘");

        // force=true → 绕过缓存强制重扫 → 文件已删 → 空
        let forced = block_on(agent_history_scan("claude".to_string(), Some(true))).unwrap();
        assert!(forced.is_empty(), "force=true 应绕过缓存强制重扫");
    }

    #[test]
    fn run_scan_force_true_on_non_claude_falls_back_to_trait_scan() {
        // force 通道身份比对：非 claude provider 无缓存层 → force 无含义，仍走 trait scan()
        static STUB: RecordingStub = RecordingStub {
            scan_called: AtomicBool::new(false),
            delete_called: AtomicBool::new(false),
            read_called: AtomicBool::new(false),
        };
        let sessions = run_scan(&STUB, true);
        assert!(sessions.is_empty());
        assert!(
            STUB.scan_called.load(Ordering::SeqCst),
            "force=true 的非 claude provider 应回落 trait scan()，而非 claude 缓存通道"
        );
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
