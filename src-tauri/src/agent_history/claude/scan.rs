//! claude 历史会话扫描 —— 扫描根单点 + 会话收集（BE-02/BE-05/BE-06/BE-19，MC-301/305 下沉）
//!
//! 职责：
//! - `resolve_projects_root()`：扫描根单点（SEC-02 约束面 / BE-06 实现面，MC-305）
//! - `scan_sessions()`：遍历扫描根一级子目录收集会话元数据（provider impl 调用，
//!   命令 `agent_history_scan` 在聚合层 mod.rs 按 cliId 分发）
//! - BE-19 缓存：扫描结果按 `(目录 mtime, 文件数)` 进程内缓存（键不变命中则复用，
//!   不重复读盘）；`scan_sessions_with_force(force)` 供命令层 force 通道强制重扫
//!
//! 排除规则（规格 3.1）：`agent-*.jsonl` 平铺形态、文件名主干非 UUID 者；
//! 不递归子目录（`<id>/subagents/` 天然不命中）。
//! 容错：单文件解析失败 → 降级条目；扫描根不存在 → 空 Vec（新机无 claude 数据属正常）。
//! env 覆盖 `SLTERM_CLAUDE_PROJECTS_DIR` 留 provider 内部（MC-305：聚合层不假设
//! env 命名——未来 `SLTERM_<CLI>_PROJECTS_DIR` 同款模式自管）。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::agent_history::claude::jsonl;
use crate::agent_history::{is_uuid_filename, AgentHistorySession};

/// 扫描根解析单点（SEC-02/BE-06，MC-305：env 覆盖留 provider 内部）
///
/// 解析顺序：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空 → 用之；
/// 否则 `dirs::home_dir()/.claude/projects`。
/// 每次调用时读取 env（不缓存）——E2E 进程继承 env 即可生效。
/// **生产不设置此 env，仅测试用途**（E2E fixture 隔离，防止测试触碰真实用户数据）。
pub fn resolve_projects_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SLTERM_CLAUDE_PROJECTS_DIR").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|home| home.join(".claude").join("projects"))
}

/// 遍历扫描根一级子目录，收集其中 UUID 形态的顶层 *.jsonl 会话（trait 路径入口）
///
/// BE-19 缓存：扫描结果按 `(目录 mtime, 文件数)` 进程内缓存——键不变命中则复用，
/// 不重复读盘；键变化（新增/删除/改名一级编码目录）自动失效重扫。
/// 目录内会话文件的增删改不改变根键——由前端显式刷新（force=true）兜底（FE-19 联动）。
pub(crate) fn scan_sessions() -> Vec<AgentHistorySession> {
    cached_scan(false)
}

/// 扫描入口（BE-19 契约 force 通道）：`force=true` 绕过缓存强制重扫
///
/// 命令层 `agent_history_scan(cliId, force)` 的 force 经 mod.rs `run_scan` 分发至此；
/// trait `scan()` 无 force 参数（注册表路径恒走 `scan_sessions()`）。
pub(crate) fn scan_sessions_with_force(force: bool) -> Vec<AgentHistorySession> {
    cached_scan(force)
}

// ── BE-19 进程内扫描缓存 ──

/// 缓存键 = (目录 mtime, 文件数)（BE-19 契约键）
///
/// 目录级粗粒度失效：新增/删除/改名一级编码目录 → mtime 或条目数变化 → 缓存失效；
/// 目录内会话文件的增删改不影响根键——由前端显式刷新（force=true）兜底。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanCacheKey {
    /// 扫描根目录修改时间（毫秒时间戳）
    pub(crate) dir_mtime_ms: u64,
    /// 扫描根一级条目数（编码目录数）
    pub(crate) file_count: u64,
}

/// 缓存条目（单槽：键含 root，不同扫描根互不冲突，换根即重扫回填）
struct ScanCacheEntry {
    /// 扫描根（条目归属，防不同根串扰）
    root: PathBuf,
    /// 契约键（目录 mtime + 文件数）
    key: ScanCacheKey,
    /// 缓存扫描结果
    sessions: Vec<AgentHistorySession>,
}

/// 进程内扫描缓存（BE-19；单槽，键含 root——跨根命中恒为 miss，仅成本为重扫）
static SCAN_CACHE: OnceLock<Mutex<Option<ScanCacheEntry>>> = OnceLock::new();

/// 带缓存的扫描核心（BE-19）
///
/// 键不变命中则复用缓存结果（不重复读盘）；键变化或 `force=true` → 全量重扫并回填。
/// 扫描根缺失/不可读不写缓存，保持既有降级语义（空 Vec）。
fn cached_scan(force: bool) -> Vec<AgentHistorySession> {
    let Some(root) = resolve_projects_root() else {
        return Vec::new(); // 无法解析扫描根（无 home 目录）
    };
    if !root.is_dir() {
        return Vec::new(); // 扫描根不存在（新机无 claude 数据）→ 空
    }
    let Some(key) = cache_key_of(&root) else {
        return Vec::new(); // 根目录不可读 → 空（降级语义与既有 scan 一致）
    };
    let cache = SCAN_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap();
    if !force {
        if let Some(entry) = guard.as_ref() {
            if entry.root == root && entry.key == key {
                return entry.sessions.clone(); // 缓存命中：不重复读盘
            }
        }
    }
    let sessions = scan_sessions_uncached(&root);
    *guard = Some(ScanCacheEntry {
        root,
        key,
        sessions: sessions.clone(), // 回填缓存后返回（命中路径复用同一 Vec）
    });
    sessions
}

/// 计算契约缓存键 = (目录 mtime, 文件数)（BE-19）
fn cache_key_of(root: &Path) -> Option<ScanCacheKey> {
    let meta = std::fs::metadata(root).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let file_count = std::fs::read_dir(root).ok()?.count() as u64;
    Some(ScanCacheKey {
        dir_mtime_ms: mtime_ms,
        file_count,
    })
}

/// 全量扫描（无缓存）：遍历扫描根一级子目录，收集其中 UUID 形态的顶层 *.jsonl 会话
///
/// 无 Err 通道：任何失败（扫描根缺失/目录不可读/单文件解析失败）均降级为空或
/// 降级条目——聚合层「单 provider 失败不阻塞其他」由此保证（MC-303）。
fn scan_sessions_uncached(root: &Path) -> Vec<AgentHistorySession> {
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        // 只扫一级子目录的直属文件，不递归（subagents/ 子目录天然不命中）
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

/// 判定是否为会话 jsonl：扩展名 jsonl + 文件名主干 UUID 形态 + 非 agent- 平铺
fn is_session_jsonl(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    !stem.starts_with("agent-") && is_uuid_filename(stem)
}

/// 解析单个会话文件 → AgentHistorySession（cli_id 打标 "claude"，MC-302）
///
/// 任何解析失败不返回 Err——降级为仅 sessionId + mtime_ms 的条目，
/// 其余字段 None / titleSource=none / cwdExists=false（BE-02 降级契约）。
fn parse_session_file(path: &Path) -> AgentHistorySession {
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let mtime_ms = file_mtime_ms(path);
    // 头部解析 + 尾部标题（均容错：任何失败返回空候选，不 panic）
    let head = jsonl::parse_head(path);
    let tail = jsonl::parse_tail_title(path);
    let (title, title_source) = jsonl::resolve_title(&head, tail);
    // cwd 一律从 JSONL 内容解析（目录名只是 cwd 的有损编码，禁止反解码）
    let cwd = head.cwd;
    let cwd_exists = cwd.as_ref().map(|c| Path::new(c).is_dir()).unwrap_or(false);
    AgentHistorySession {
        session_id,
        cwd,
        title,
        // 内部枚举 → DTO 开放字符串（claude 值集；UI 不消费具体值，MC-302）
        title_source: title_source.as_str().to_string(),
        first_prompt: head.first_prompt,
        mtime_ms,
        cwd_exists,
        // provider 打标（provider 内部写字面量合法，MC-302）
        cli_id: "claude".to_string(),
    }
}

/// 文件修改时间转毫秒时间戳（决策 26）；metadata 失败 → 0（BE-05）
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_history::claude::{ScanRootGuard, TitleSource};
    use std::io::Write;

    // ── 测试辅助 ──

    /// 创建扫描根 + 一个编码目录（cwd 编码形态，如 C--Users-test-app）
    /// 返回 (TempDir 守卫, 规范化扫描根路径, 编码目录路径)
    ///
    /// 路径经 dunce::canonicalize 统一长名（8.3 短名坑，照 git/CLAUDE.md 先例）。
    fn make_scan_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let proj = root.join("C--Users-test-app");
        std::fs::create_dir_all(&proj).unwrap();
        (dir, root, proj)
    }

    /// 在编码目录下写一个有效会话文件（UUID 文件名 + summary 首行 + user prompt 行）
    /// 内容经 serde_json 序列化保证 JSON 转义正确（Windows 路径含反斜杠）
    fn write_valid_session(proj: &Path, uuid: &str) {
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

    // ── resolve_projects_root（SEC-02/BE-06，MC-305：env 覆盖留 provider 内部） ──

    #[test]
    fn resolve_root_env_override() {
        // env 指向 tempdir → 用之（依赖 --test-threads=1 门禁；guard 测毕自动恢复）
        let dir = tempfile::tempdir().unwrap();
        let canon = dunce::canonicalize(dir.path()).unwrap();
        let _guard = ScanRootGuard::set(&canon);
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, canon);
    }

    #[test]
    fn resolve_root_empty_env_falls_back_to_home() {
        // env 为空串 → 回退 home/.claude/projects（依赖 --test-threads=1 门禁）
        let _guard = ScanRootGuard::set("");
        let expected = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects"))
            .unwrap();
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, expected);
    }

    #[test]
    fn resolve_root_default_without_env() {
        // 未设 env → home/.claude/projects（guard 移除 env 并保证测毕恢复）
        let _guard = ScanRootGuard::unset();
        let expected = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects"))
            .unwrap();
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, expected);
    }

    // ── scan_sessions：排除规则 ──

    #[test]
    fn scan_excludes_agent_non_uuid_subagents() {
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        // 排除 1：agent-*.jsonl 平铺形态
        std::fs::write(proj.join("agent-abc123.jsonl"), "{}").unwrap();
        // 排除 2：非 UUID 文件名主干
        std::fs::write(proj.join("not-a-uuid.jsonl"), "{}").unwrap();
        // 排除 3：subagents 子目录（不递归，天然不命中）
        let sub = proj.join(format!("{uuid}")).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("agent-def456.jsonl"), "{}").unwrap();
        // 非 jsonl 扩展名
        std::fs::write(proj.join(format!("{uuid}.txt")), "{}").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(
            sessions.len(),
            1,
            "应仅命中 1 条 UUID 会话，实际: {:?}",
            sessions.len()
        );
        assert_eq!(sessions[0].session_id, uuid);
    }

    #[test]
    fn scan_collects_all_sessions_across_dirs() {
        // 多个编码目录 + 多个会话 → 全部收集（HFN-09②：原名暗示测顺序——扫描顺序无契约，
        // 排序是前端职责，仅断言集合）
        let (_dir, root, proj) = make_scan_root();
        let uuid1 = "123e4567-e89b-12d3-a456-426614174001";
        let uuid2 = "123e4567-e89b-12d3-a456-426614174002";
        write_valid_session(&proj, uuid1);
        write_valid_session(&proj, uuid2);
        // 第二个编码目录
        let proj2 = root.join("D--other-app");
        std::fs::create_dir_all(&proj2).unwrap();
        let uuid3 = "123e4567-e89b-12d3-a456-426614174003";
        write_valid_session(&proj2, uuid3);

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let mut ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, [uuid1, uuid2, uuid3]);
    }

    #[test]
    fn scan_root_missing_returns_empty() {
        // 扫描根不存在 → 空 Vec（非 Err）
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在");
        let _guard = ScanRootGuard::set(&missing);
        let sessions = scan_sessions();
        assert!(sessions.is_empty());
    }

    // ── parse_session_file：降级条目 ──

    #[test]
    fn scan_corrupt_jsonl_produces_degraded_entry() {
        // 损坏 jsonl → 降级条目：仅 sessionId + mtime_ms，其余 None/none/false
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::write(proj.join(format!("{uuid}.jsonl")), "{broken json 没有换行").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        assert!(s.cwd.is_none());
        assert!(s.title.is_none());
        assert_eq!(s.title_source, TitleSource::None.as_str());
        assert!(s.first_prompt.is_none());
        assert!(s.mtime_ms > 0, "降级条目应保留文件 mtime");
        assert!(!s.cwd_exists);
    }

    #[test]
    fn scan_empty_jsonl_produces_degraded_entry() {
        // 空文件 → 同样降级（不 panic）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::write(proj.join(format!("{uuid}.jsonl")), "").unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
        assert!(sessions[0].title.is_none());
    }

    #[test]
    fn scan_unreadable_session_file_produces_degraded_entry() {
        // IO 降级（HFN-05）：<uuid>.jsonl 路径是目录——条目存在但无法按文件读取
        // （File::open 失败）→ 降级条目，不 panic、不跳过
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::create_dir_all(proj.join(format!("{uuid}.jsonl"))).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1, "不可读条目应降级而非崩溃/被跳过");
        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        assert!(s.title.is_none());
        assert!(s.cwd.is_none());
        assert_eq!(s.title_source, TitleSource::None.as_str());
        assert!(!s.cwd_exists);
    }

    #[test]
    fn parse_session_file_missing_file_degraded_zero_mtime() {
        // IO 降级（HFN-05）：metadata 失败（文件不存在）→ mtime_ms=0 + 全字段降级
        let dir = tempfile::tempdir().unwrap();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let missing = dir.path().join(format!("{uuid}.jsonl"));
        let s = parse_session_file(&missing);
        assert_eq!(s.session_id, uuid);
        assert_eq!(s.mtime_ms, 0, "metadata 失败 → mtime_ms=0");
        assert!(s.title.is_none());
        assert!(s.cwd.is_none());
        assert!(!s.cwd_exists);
    }

    // ── parse_session_file：完整字段 ──

    #[test]
    fn scan_full_session_fields() {
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        // cwd 指向存在的临时目录（验证 cwd_exists=true）
        let existing = dunce::canonicalize(proj.clone()).unwrap();
        let content = serde_json::json!({
            "type": "summary",
            "summary": "修复登录 bug",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": existing.to_string_lossy(),
                "message": { "content": "帮我修 bug" },
            })
            .to_string();
        let path = proj.join(format!("{uuid}.jsonl"));
        std::fs::write(&path, content).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        // 标题回退链：custom-title/ai-title 均无 → summary 赢
        assert_eq!(s.title.as_deref(), Some("修复登录 bug"));
        assert_eq!(s.title_source, TitleSource::Summary.as_str());
        assert_eq!(s.first_prompt.as_deref(), Some("帮我修 bug"));
        assert!(s.mtime_ms > 0);
        assert_eq!(s.cwd.as_deref(), Some(existing.to_str().unwrap()));
        assert!(s.cwd_exists, "cwd 指向存在的目录 → cwd_exists=true");
        assert_eq!(s.cli_id, "claude", "provider 打标 cliId");
    }

    #[test]
    fn scan_cwd_exists_false_for_missing_dir() {
        // cwd 指向不存在的目录 → cwd_exists=false（孤儿会话）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let ghost = proj.join("已被删除");
        let content = serde_json::json!({
            "type": "summary",
            "summary": "孤儿会话",
            "leafUuid": "x",
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "cwd": ghost.to_string_lossy(),
                "message": { "content": "q" },
            })
            .to_string();
        std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert!(!sessions[0].cwd_exists, "cwd 目录不存在 → cwd_exists=false");
        assert!(sessions[0].cwd.is_some());
    }

    // ── env 覆盖端到端（scan 每次调用读 env，进程继承即可生效） ──

    #[test]
    fn scan_respects_env_override_during_scan() {
        // 先设 env 再 scan → 命中 tempdir 会话（依赖 --test-threads=1 门禁）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
    }

    // ── file_mtime_ms ──

    #[test]
    fn mtime_ms_positive_for_existing_file() {
        let (_dir, _root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let path = proj.join(format!("{uuid}.jsonl"));
        std::fs::write(&path, "{}").unwrap();
        let m = file_mtime_ms(&path);
        assert!(m > 0, "现有文件 mtime 应为正毫秒时间戳");
    }

    #[test]
    fn mtime_ms_zero_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在.jsonl");
        assert_eq!(file_mtime_ms(&missing), 0);
    }

    // ── 尾部标题端到端（大文件场景经 scan 落地） ──

    #[test]
    fn scan_tail_custom_title_overrides_head_summary() {
        // 尾部 64KB 内 custom-title → 覆盖头部 summary（决策 22）
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let path = proj.join(format!("{uuid}.jsonl"));
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"summary","summary":"旧摘要","leafUuid":"x"}}"#
        )
        .unwrap();
        // 填充 >512KB，使尾部窗口与头部窗口分离
        let line = b"{\"type\":\"pad\",\"payload\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}\n";
        while f.metadata().unwrap().len() < 600 * 1024 {
            f.write_all(line).unwrap();
        }
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"重命名后的标题","sessionId":"{uuid}"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let _guard = ScanRootGuard::set(&root);
        let sessions = scan_sessions();

        let s = &sessions[0];
        assert_eq!(s.title.as_deref(), Some("重命名后的标题"));
        assert_eq!(s.title_source, TitleSource::CustomTitle.as_str());
    }

    // ── BE-19 进程内缓存（键 = (目录 mtime, 文件数)；force=true 绕过） ──

    #[test]
    fn scan_cache_hit_returns_stale_without_reread() {
        // 缓存命中不重复读盘：删除会话文件（根键不变——目录 mtime/一级条目数均未变）
        // → 默认扫描仍返回缓存旧结果，证明未重读磁盘
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].session_id, uuid);

        // 目录内文件删除不影响根键 → 命中缓存，返回删除前的旧结果
        std::fs::remove_file(proj.join(format!("{uuid}.jsonl"))).unwrap();
        let cached = scan_sessions();
        assert_eq!(cached.len(), 1, "键不变应缓存命中（未重复读盘）");
        assert_eq!(cached[0].session_id, uuid);
    }

    #[test]
    fn scan_cache_invalidated_when_file_count_changes() {
        // 新增编码目录（根条目数与 mtime 均变化）→ 键变化 → 缓存失效 → 重扫全量
        let (_dir, root, proj) = make_scan_root();
        let uuid1 = "123e4567-e89b-12d3-a456-426614174001";
        write_valid_session(&proj, uuid1);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);

        // 新增第二个编码目录 + 会话
        let proj2 = root.join("D--other-app");
        std::fs::create_dir_all(&proj2).unwrap();
        let uuid2 = "123e4567-e89b-12d3-a456-426614174002";
        write_valid_session(&proj2, uuid2);

        let second = scan_sessions();
        let mut ids: Vec<&str> = second.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, [uuid1, uuid2], "键变化应失效缓存并重扫全量");
    }

    #[test]
    fn scan_force_true_bypasses_cache() {
        // force=true 绕过缓存强制重扫：键未变也重读磁盘
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        write_valid_session(&proj, uuid);
        let _guard = ScanRootGuard::set(&root);

        let first = scan_sessions();
        assert_eq!(first.len(), 1);

        // 删除文件后 force 重扫 → 应直接读盘得空（不命中缓存旧结果）
        std::fs::remove_file(proj.join(format!("{uuid}.jsonl"))).unwrap();
        let forced = scan_sessions_with_force(true);
        assert!(forced.is_empty(), "force=true 应绕过缓存直接重扫磁盘");
    }

    #[test]
    fn scan_cache_key_tracks_dir_mtime_and_file_count() {
        // 契约键 = (目录 mtime, 文件数)：改名一级目录 → mtime 变、条目数不变；
        // 新增目录 → 条目数变。两半均为失效依据。
        let (_dir, root, proj) = make_scan_root();
        let key1 = cache_key_of(&root).unwrap();

        // 改名：根条目数不变，根 mtime 变化（目录条目增删）。
        // Windows NTFS 目录 mtime 精度 100ns，但 as_millis() 截断到毫秒——若 rename
        // 与上次目录修改（create_dir_all）落在同一毫秒，两键 mtime 同值导致 flaky。
        // sleep 须在 rename 之前：键值记录的是修改发生时刻的墙钟，睡眠后再 rename
        // 保证修改时刻跨入新毫秒（rename 后 sleep 对键值无影响）。
        std::thread::sleep(std::time::Duration::from_millis(5));
        let proj2 = root.join("D--renamed");
        std::fs::rename(&proj, &proj2).unwrap();
        let key2 = cache_key_of(&root).unwrap();
        assert_eq!(key1.file_count, key2.file_count, "改名不改变一级条目数");
        assert_ne!(
            key1.dir_mtime_ms, key2.dir_mtime_ms,
            "改名应改变根 mtime → 失效"
        );

        // 新增目录：条目数 +1（mtime 亦变）
        std::fs::create_dir_all(root.join("E--new-app")).unwrap();
        let key3 = cache_key_of(&root).unwrap();
        assert_eq!(key2.file_count + 1, key3.file_count, "新增目录应 +1 条目数");
    }

    #[test]
    fn scan_cache_isolated_per_root() {
        // 不同扫描根互不污染（缓存键含 root）：根 B 不命中根 A 的缓存，换根即重扫
        let (_dir_a, root_a, proj_a) = make_scan_root();
        let uuid_a = "123e4567-e89b-12d3-a456-42661417400a";
        write_valid_session(&proj_a, uuid_a);
        let (_dir_b, root_b, proj_b) = make_scan_root();
        let uuid_b = "123e4567-e89b-12d3-a456-42661417400b";
        write_valid_session(&proj_b, uuid_b);

        let g1 = ScanRootGuard::set(&root_a);
        let a1 = scan_sessions();
        assert_eq!(a1[0].session_id, uuid_a);
        drop(g1);

        let g2 = ScanRootGuard::set(&root_b);
        let b1 = scan_sessions();
        assert_eq!(
            b1[0].session_id, uuid_b,
            "根 B 应扫自己的根而非命中根 A 缓存"
        );
        drop(g2);

        // 切回根 A：单槽缓存已被根 B 顶替 → 重扫（结果仍正确，不串扰）
        let g3 = ScanRootGuard::set(&root_a);
        let a2 = scan_sessions();
        assert_eq!(a2[0].session_id, uuid_a);
        drop(g3);
    }
}
