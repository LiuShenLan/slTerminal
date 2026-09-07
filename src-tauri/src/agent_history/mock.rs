//! mockcli 历史 provider —— L4 专用测试 provider(CP-041)
//!
//! 仅当 env `SLTERM_MOCKCLI_PROJECTS_DIR` 存在时注册进注册表(run-wdio.cjs 注入;
//! 生产/日常二进制无该 env → 注册表与现状一致,`agent_history_scan("mockcli")` 仍
//! Validation)。扫描根/命名/解析自管(MC-305 先例:env 不上提聚合层)。会话文件
//! 形态与 claude jsonl 相同(fixture 由 e2e-tests/fixtures/mockcli-projects/ 复制,
//! 占位符替换同 claude 通道)。
//!
//! 实现无缓存层(BE-19 仅 claude 有):trait `scan()` 每次全量读盘,force 通道经
//! mod.rs `run_scan` 身份比对后恒走本实现(claude 缓存通道不达)。
//! 解析一律复用 `claude::jsonl` 纯函数助手(parse_head/parse_tail_title/resolve_title
//! ——禁止复刻第二份解析);定位/删除结构照 claude ops 同构(SEC-05:路径全部由
//! 扫描根 + 一级子目录名 + 校验过的 sessionId 拼接派生;AQ-3 符号链接拒跟随)。

use std::path::{Path, PathBuf};

use crate::agent_history::claude::jsonl;
use crate::agent_history::{is_uuid_filename, AgentHistorySession, AgentHistoryTitle};
use crate::error::AppError;

use super::CliHistoryProvider;

/// mockcli 测试 provider(无状态单元结构体,注册表静态引用)
#[derive(Debug)]
pub struct MockCliHistoryProvider;

/// 扫描根解析:env 优先;未设置 → None(生产形态,provider 注册亦被 env 门控)
fn scan_root() -> Option<PathBuf> {
    std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR").map(PathBuf::from)
}

impl CliHistoryProvider for MockCliHistoryProvider {
    fn scan(&self) -> Vec<AgentHistorySession> {
        // 形态照 claude scan:遍历扫描根一级编码目录,UUID 主干 .jsonl,解析
        // cwd/summary(复用 claude::jsonl 解析助手),条目打标 cli_id = "mockcli"。
        // env 缺失/目录不存在 → 空 Vec(无 Err 通道,照 trait 契约降级)。
        let Some(root) = scan_root() else {
            return Vec::new();
        };
        scan_sessions_uncached(&root)
    }

    fn delete(&self, session_id: &str) -> Result<(), AppError> {
        // 定位扫描根内 UUID jsonl 并删除(含同名附属目录);不存在 → Validation
        // 「会话不存在」语义照 claude ops(BE-07——删是有副作用操作,不存在是错误)。
        if !is_uuid_filename(session_id) {
            return Err(AppError::Validation(format!(
                "非法 sessionId: {session_id}"
            )));
        }
        let Some(root) = scan_root() else {
            return Err(session_not_found(session_id));
        };
        let Some(jsonl_path) = locate_session_jsonl(&root, session_id) else {
            return Err(session_not_found(session_id));
        };
        std::fs::remove_file(&jsonl_path)?;
        // 同名 <id>/ 目录(subagents 等附属数据)存在则一并删除(照 claude ops 范围)
        if let Some(dir) = jsonl_path.parent() {
            let session_dir = dir.join(session_id);
            // AQ-3:同名目录为 symlink(指向扫描根外)→ 拒绝删除,不跟随链接目标
            if session_dir.is_dir() && !session_dir.is_symlink() {
                std::fs::remove_dir_all(&session_dir)?;
            }
        }
        Ok(())
    }

    fn validate_session_id(&self, session_id: &str) -> Result<(), AppError> {
        // 照 trait 契约:UUID 形态(is_uuid_filename)——与 claude 校验同口径(SEC-05)
        if is_uuid_filename(session_id) {
            Ok(())
        } else {
            Err(AppError::Validation(format!(
                "非法 sessionId: {session_id}"
            )))
        }
    }

    fn read_title(&self, session_id: &str) -> Result<AgentHistoryTitle, AppError> {
        // 回退链照 claude ops 同构:parse_head + parse_tail_title + resolve_title
        // (custom-title > ai-title > summary > firstPrompt,与历史扫描同源);
        // 文件缺失/扫描根缺失 → Ok(title: None)(读是幂等查询,正常条件非错误)。
        if !is_uuid_filename(session_id) {
            return Err(AppError::Validation(format!(
                "非法 sessionId: {session_id}"
            )));
        }
        let Some(root) = scan_root() else {
            return Ok(title_none());
        };
        let Some(jsonl_path) = locate_session_jsonl(&root, session_id) else {
            return Ok(title_none());
        };
        let head = jsonl::parse_head(&jsonl_path);
        let tail = jsonl::parse_tail_title(&jsonl_path);
        let (title, source) = jsonl::resolve_title(&head, tail);
        Ok(AgentHistoryTitle {
            title,
            title_source: source.as_str().to_string(),
        })
    }
}

/// 「会话不存在」错误(BE-07:jsonl 找不到 → Err 且消息含「不存在」语义,照 claude ops)
fn session_not_found(session_id: &str) -> AppError {
    AppError::Validation(format!("会话不存在: {session_id}"))
}

/// 标题四路全无的 DTO(title None + source "none")
fn title_none() -> AgentHistoryTitle {
    AgentHistoryTitle {
        title: None,
        title_source: "none".to_string(),
    }
}

/// 全量扫描(无缓存):遍历扫描根一级子目录,收集其中 UUID 形态的顶层 *.jsonl 会话
///
/// 无 Err 通道:任何失败(目录不可读/单文件解析失败)均降级为空或降级条目——
/// 聚合层「单 provider 失败不阻塞其他」由此保证(MC-303)。
fn scan_sessions_uncached(root: &Path) -> Vec<AgentHistorySession> {
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new(); // 扫描根不存在/不可读 → 空
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        // AQ-3 符号链接拒跟随:一级子目录为 symlink(可能指向扫描根外)→ 跳过
        if !dir_path.is_dir() || dir_path.is_symlink() {
            continue;
        }
        // 只扫一级子目录的直属文件,不递归(subagents/ 子目录天然不命中)
        let Ok(files) = std::fs::read_dir(&dir_path) else {
            continue;
        };
        for file in files.flatten() {
            let file_path = file.path();
            if !is_session_jsonl(&file_path) {
                continue;
            }
            sessions.push(parse_session_file(&file_path));
        }
    }
    sessions
}

/// 判定是否为会话 jsonl:扩展名 jsonl + 文件名主干 UUID 形态 + 非 agent- 平铺
fn is_session_jsonl(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    !stem.starts_with("agent-") && is_uuid_filename(stem)
}

/// 解析单个会话文件 → AgentHistorySession(cli_id 打标 "mockcli",MC-302)
///
/// 任何解析失败不返回 Err——降级为仅 sessionId + mtime_ms 的条目,
/// 其余字段 None / titleSource=none / cwdExists=false(BE-02 降级契约)。
fn parse_session_file(path: &Path) -> AgentHistorySession {
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let mtime_ms = file_mtime_ms(path);
    // 头部解析 + 尾部标题(均容错:任何失败返回空候选,不 panic)——复用 claude 解析助手
    let head = jsonl::parse_head(path);
    let tail = jsonl::parse_tail_title(path);
    let (title, title_source) = jsonl::resolve_title(&head, tail);
    // cwd 一律从 JSONL 内容解析(目录名只是 cwd 的有损编码,禁止反解码)
    let cwd = head.cwd;
    let cwd_exists = cwd.as_ref().map(|c| Path::new(c).is_dir()).unwrap_or(false);
    AgentHistorySession {
        session_id,
        cwd,
        title,
        // 内部枚举 → DTO 开放字符串(claude 值集;UI 不消费具体值,MC-302)
        title_source: title_source.as_str().to_string(),
        first_prompt: head.first_prompt,
        mtime_ms,
        cwd_exists,
        // provider 打标(provider 内部写字面量合法,MC-302)
        cli_id: "mockcli".to_string(),
    }
}

/// 文件修改时间转毫秒时间戳;metadata 失败 → 0(BE-05)
fn file_mtime_ms(path: &Path) -> u64 {
    let modified = match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 在扫描根一级子目录中定位 `<session_id>.jsonl`(SEC-05 定位,不递归子目录)
///
/// 遍历扫描根的一级子目录(cwd 编码目录),精确匹配文件名。
/// 扫描根不存在 / 未命中 → None(调用方按「会话不存在」处理)。
fn locate_session_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    let target = format!("{session_id}.jsonl");
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        // AQ-3 符号链接拒跟随:一级子目录为 symlink(可能指向扫描根外)→ 跳过
        if !dir_path.is_dir() || dir_path.is_symlink() {
            continue;
        }
        let candidate = dir_path.join(&target);
        // AQ-3:命中文件为 symlink → 不命中(防外部文件被定位/删除)
        if candidate.is_file() && !candidate.is_symlink() {
            return Some(candidate);
        }
    }
    None
}

/// `SLTERM_MOCKCLI_PROJECTS_DIR` env 守卫(CP-041 测试用,同 claude ScanRootGuard 形态)
///
/// set/unset 后无论测试成功或 panic,Drop 时均恢复原 env 值(原无 → 移除),
/// 不残留污染后续用例。依赖 --test-threads=1 门禁(env 全局可变,并行测试互污染)。
#[cfg(test)]
pub(crate) struct MockRootGuard(Option<std::ffi::OsString>);

#[cfg(test)]
impl MockRootGuard {
    /// 设置 env 为给定值(路径 / 空串均可),Drop 时恢复原值
    pub(crate) fn set(value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR");
        std::env::set_var("SLTERM_MOCKCLI_PROJECTS_DIR", value);
        MockRootGuard(prev)
    }

    /// 移除 env(等价未设),Drop 时恢复原值
    pub(crate) fn unset() -> Self {
        let prev = std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR");
        std::env::remove_var("SLTERM_MOCKCLI_PROJECTS_DIR");
        MockRootGuard(prev)
    }
}

#[cfg(test)]
impl Drop for MockRootGuard {
    fn drop(&mut self) {
        match &self.0 {
            Some(v) => std::env::set_var("SLTERM_MOCKCLI_PROJECTS_DIR", v),
            None => std::env::remove_var("SLTERM_MOCKCLI_PROJECTS_DIR"),
        }
    }
}

#[cfg(test)]
mod mock_provider_tests {
    use super::*;

    // ── 测试辅助 ──
    //
    // env 操作(SLTERM_MOCKCLI_PROJECTS_DIR)依赖 --test-threads=1 门禁:
    // env 全局可变,并行测试会互相污染(同 claude scan/ops 测试约束)。
    // 路径经 dunce::canonicalize 统一长名(8.3 短名坑,照 git/CLAUDE.md 先例)。

    const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";

    /// 创建扫描根 + 一个编码目录,返回 (TempDir 守卫, 规范化根, 编码目录)
    fn make_scan_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let proj = root.join("C--Users-e2e-fixture-mock");
        std::fs::create_dir_all(&proj).unwrap();
        (dir, root, proj)
    }

    /// 在编码目录下写一个有效会话文件(UUID 文件名 + summary 首行 + user prompt 行)
    /// 内容经 serde_json 序列化保证 JSON 转义正确(Windows 路径含反斜杠)
    fn write_valid_session(proj: &Path, uuid: &str) {
        let content = serde_json::json!({
            "type": "summary",
            "summary": "mockcli 测试会话",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": "C:\\mock\\app",
                "message": { "content": "帮我修 mock 问题" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();
    }

    /// 断言错误为 Validation 变体(并返回消息供「不存在」语义断言)
    fn assert_validation(err: AppError) -> String {
        match err {
            AppError::Validation(msg) => msg,
            other => panic!("应为 AppError::Validation，实际: {other:?}"),
        }
    }

    // ── scan:env 缺失 → 空 Vec(无 Err 通道降级) ──

    #[test]
    fn scan_env_missing_returns_empty() {
        // env 未设(生产形态双保险——provider 注册已被门控,此测试锁死降级语义)
        let _guard = MockRootGuard::unset();
        let sessions = MockCliHistoryProvider.scan();
        assert!(sessions.is_empty());
    }

    #[test]
    fn scan_root_missing_returns_empty() {
        // env 指向不存在的目录 → 空 Vec(非 Err)
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在");
        let _guard = MockRootGuard::set(&missing);
        let sessions = MockCliHistoryProvider.scan();
        assert!(sessions.is_empty());
    }

    // ── scan:fixture 单条目打标 + 字段解析 ──

    #[test]
    fn scan_tempdir_fixture_tags_cli_id_mockcli() {
        // fixture tempdir 单条目扫描:打标 cli_id == "mockcli";标题回退落 summary
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);

        let _guard = MockRootGuard::set(&root);
        let sessions = MockCliHistoryProvider.scan();

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.session_id, UUID);
        assert_eq!(s.cli_id, "mockcli", "provider 打标 cliId");
        assert_eq!(s.title.as_deref(), Some("mockcli 测试会话"));
        assert_eq!(s.title_source, "summary");
        assert_eq!(s.cwd.as_deref(), Some("C:\\mock\\app"));
        assert!(s.mtime_ms > 0);
        assert!(!s.cwd_exists, "C:\\mock\\app 不存在 → cwd_exists=false");
    }

    #[test]
    fn scan_excludes_agent_and_non_uuid_files() {
        // 排除规则与 claude 同构:agent-*.jsonl 平铺 + 非 UUID 文件名不入条目
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);
        std::fs::write(proj.join("agent-abc.jsonl"), "{}").unwrap();
        std::fs::write(proj.join("not-a-uuid.jsonl"), "{}").unwrap();

        let _guard = MockRootGuard::set(&root);
        let sessions = MockCliHistoryProvider.scan();

        assert_eq!(sessions.len(), 1, "应仅命中 UUID 会话文件");
        assert_eq!(sessions[0].session_id, UUID);
    }

    // ── validate_session_id:UUID 两态 ──

    #[test]
    fn validate_session_id_accepts_uuid_forms() {
        // 小写 / 大写 hex 均合法(大小写不敏感,与 claude 同口径)
        assert!(MockCliHistoryProvider.validate_session_id(UUID).is_ok());
        assert!(MockCliHistoryProvider
            .validate_session_id("123E4567-E89B-12D3-A456-426614174000")
            .is_ok());
    }

    #[test]
    fn validate_session_id_rejects_non_uuid_inputs() {
        // 路径穿越/非 UUID 形态全拒(SEC-05:含 ../、分隔符、长度不足)
        for bad in [
            "..",
            "abc/def",
            "abc\\def",
            "123e4567-e89b-12d3-a456", // 非 UUID(长度不足)
        ] {
            let msg =
                assert_validation(MockCliHistoryProvider.validate_session_id(bad).unwrap_err());
            assert!(msg.contains(bad), "错误消息应含非法输入，实际: {msg}");
        }
        let msg = assert_validation(MockCliHistoryProvider.validate_session_id("").unwrap_err());
        assert!(
            msg.contains("非法 sessionId"),
            "错误消息应含校验文案，实际: {msg}"
        );
    }

    // ── delete:落盘真删 ──

    #[test]
    fn delete_removes_jsonl_on_disk() {
        // 落盘真删(mk tempdir 隔离):jsonl 删除 + 同目录其他会话不受影响
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "line1\n").unwrap();
        let other = "123e4567-e89b-12d3-a456-426614174001";
        std::fs::write(proj.join(format!("{other}.jsonl")), "line1\n").unwrap();

        let _guard = MockRootGuard::set(&root);
        MockCliHistoryProvider.delete(UUID).unwrap();

        assert!(
            !proj.join(format!("{UUID}.jsonl")).exists(),
            "jsonl 应被删除"
        );
        assert!(
            proj.join(format!("{other}.jsonl")).exists(),
            "其他会话不受影响"
        );
    }

    #[test]
    fn delete_missing_session_returns_not_found() {
        // 文件不存在 → Err 且消息含「不存在」语义(照 claude ops BE-07)
        let (_dir, root, _proj) = make_scan_root();
        let _guard = MockRootGuard::set(&root);
        let msg = assert_validation(MockCliHistoryProvider.delete(UUID).unwrap_err());
        assert!(
            msg.contains("不存在"),
            "消息应含「不存在」语义，实际: {msg}"
        );
    }

    #[test]
    fn delete_rejects_invalid_session_id_before_fs() {
        // 非法 sessionId 在触碰文件系统前被拒(SEC-05 校验兜底)
        let (_dir, root, proj) = make_scan_root();
        let _guard = MockRootGuard::set(&root);
        let msg = assert_validation(MockCliHistoryProvider.delete("../evil").unwrap_err());
        assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
        assert!(
            !proj.join("..").join("evil.jsonl").exists(),
            "越界文件不应被触碰"
        );
    }

    // ── read_title:回退链 + 文件缺失 → Ok(None) ──

    #[test]
    fn read_title_resolves_summary_title() {
        // summary 首行 → 标题落位 summary(与 scan 回退链同源)
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);

        let _guard = MockRootGuard::set(&root);
        let t = MockCliHistoryProvider.read_title(UUID).unwrap();

        assert_eq!(t.title.as_deref(), Some("mockcli 测试会话"));
        assert_eq!(t.title_source, "summary");
    }

    #[test]
    fn read_title_missing_file_returns_none_ok() {
        // 会话文件不存在(运行中会话尚未落盘)→ Ok(title: None)——正常条件非 Err
        let (_dir, root, _proj) = make_scan_root();
        let _guard = MockRootGuard::set(&root);
        let t = MockCliHistoryProvider.read_title(UUID).unwrap();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
    }

    #[test]
    fn read_title_env_missing_returns_none_ok() {
        // env 未设 → Ok(title: None)(与 claude ops「根解析失败 → None」同语义)
        let _guard = MockRootGuard::unset();
        let t = MockCliHistoryProvider.read_title(UUID).unwrap();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
    }

    #[test]
    fn read_title_rejects_invalid_session_id() {
        // 非法 sessionId 在触碰文件系统前被拒(SEC-05 校验前置)
        let (_dir, root, _proj) = make_scan_root();
        let _guard = MockRootGuard::set(&root);
        let msg = assert_validation(MockCliHistoryProvider.read_title("../evil").unwrap_err());
        assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
    }

    #[test]
    fn scan_force_true_dispatch_returns_mock_rows_not_claude_rows() {
        // CP-041 防 ZST 地址共享回归(端到端锁死):agent_history_scan 的 force
        // 通道经 mod.rs run_scan 的 claude 身份比对(数据指针相等)分流——注册表
        // 两 provider 实例地址若共享(裸单元结构体 static 曾实测合并),mockcli 的
        // force 扫描会被误判为 claude 而返回 claude 条目。此处直接走命令核心
        // run_scan(mock provider, force=true),断言条目全部为 mockcli 打标。
        use crate::agent_history::claude::ScanRootGuard;
        use crate::agent_history::provider::resolve_provider;
        use crate::agent_history::run_scan;
        let (_dir, root, proj) = make_scan_root();
        write_valid_session(&proj, UUID);
        // 双守卫:mock env → 含会话 tempdir;claude env → 空 tempdir(回归时
        // claude 通道读空目录,防误读真实用户 ~/.claude/projects)
        let _mock_guard = MockRootGuard::set(&root);
        let claude_dir = tempfile::tempdir().unwrap();
        let _claude_guard = ScanRootGuard::set(&claude_dir.path());

        let provider = resolve_provider("mockcli").unwrap();
        let sessions = run_scan(provider, true);

        assert_eq!(sessions.len(), 1, "force 分发应走 mock provider 扫描");
        assert_eq!(sessions[0].session_id, UUID);
        assert_eq!(
            sessions[0].cli_id, "mockcli",
            "mockcli force 扫描不得产出 claude 打标条目(身份盒地址共享回归)"
        );
    }

    // ── MockRootGuard RAII 自身(防 env 残留污染) ──

    #[test]
    fn mock_root_guard_restores_previous_env_on_drop() {
        // 守卫 Drop 恢复原 env(panic 也不残留);外层 guard 保护本测试自身
        let _outer = MockRootGuard::set("C:\\guard-prev");
        {
            let _g = MockRootGuard::set("C:\\guard-new");
            assert_eq!(
                std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR"),
                Some(std::ffi::OsString::from("C:\\guard-new"))
            );
        }
        assert_eq!(
            std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR"),
            Some(std::ffi::OsString::from("C:\\guard-prev")),
            "Drop 后应恢复原 env 值"
        );
    }
}
