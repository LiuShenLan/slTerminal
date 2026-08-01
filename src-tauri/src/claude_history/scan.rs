//! 历史会话扫描 —— 扫描根单点 + `claude_history_scan` 命令（BE-02/BE-05/BE-06）
//!
//! 职责：
//! - `resolve_projects_root()`：扫描根单点（SEC-02 约束面 / BE-06 实现面）
//! - `claude_history_scan` 命令：遍历扫描根一级子目录收集会话元数据
//!
//! 排除规则（规格 3.1）：`agent-*.jsonl` 平铺形态、文件名主干非 UUID 者；
//! 不递归子目录（`<id>/subagents/` 天然不命中）。
//! 容错：单文件解析失败 → 降级条目；扫描根不存在 → 空 Vec（新机无 claude 数据属正常）。

use std::path::{Path, PathBuf};

use crate::claude_history::{is_uuid_filename, jsonl, HistorySession};

/// 扫描根解析单点（SEC-02/BE-06）
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

/// claude_history_scan 命令：扫描全部历史会话元数据（BE-02）
///
/// 阻塞 I/O 全部在 spawn_blocking 内执行（硬约束 #3）。
/// 扫描根不存在 → 空 Vec（非 Err）；单文件解析失败 → 降级条目，不阻塞整体。
#[tauri::command]
pub async fn claude_history_scan() -> Result<Vec<HistorySession>, crate::AppError> {
    tokio::task::spawn_blocking(scan_sessions)
        .await
        .map_err(crate::AppError::from)
}

/// 遍历扫描根一级子目录，收集其中 UUID 形态的顶层 *.jsonl 会话（纯 I/O 逻辑）
fn scan_sessions() -> Vec<HistorySession> {
    let Some(root) = resolve_projects_root() else {
        return Vec::new(); // 无法解析扫描根（无 home 目录）
    };
    if !root.is_dir() {
        return Vec::new(); // 扫描根不存在（新机无 claude 数据）→ 空
    }
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(&root) else {
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

/// 解析单个会话文件 → HistorySession
///
/// 任何解析失败不返回 Err——降级为仅 sessionId + mtime_ms 的条目，
/// 其余字段 None / titleSource=none / cwdExists=false（BE-02 降级契约）。
fn parse_session_file(path: &Path) -> HistorySession {
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
    HistorySession {
        session_id,
        cwd,
        title,
        title_source,
        first_prompt: head.first_prompt,
        mtime_ms,
        cwd_exists,
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
    use crate::claude_history::TitleSource;
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

    /// 设置扫描根 env（依赖 --test-threads=1 门禁：env 全局可变，并行测试会互相污染）
    fn set_scan_root(path: &Path) {
        std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", path);
    }

    /// 恢复扫描根 env
    fn unset_scan_root() {
        std::env::remove_var("SLTERM_CLAUDE_PROJECTS_DIR");
    }

    // ── resolve_projects_root（SEC-02/BE-06） ──

    #[test]
    fn resolve_root_env_override() {
        // env 指向 tempdir → 用之；测毕恢复（依赖 --test-threads=1 门禁）
        let dir = tempfile::tempdir().unwrap();
        let canon = dunce::canonicalize(dir.path()).unwrap();
        set_scan_root(&canon);
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, canon);
        unset_scan_root();
    }

    #[test]
    fn resolve_root_empty_env_falls_back_to_home() {
        // env 为空串 → 回退 home/.claude/projects（依赖 --test-threads=1 门禁）
        std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", "");
        let expected = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects"))
            .unwrap();
        let root = resolve_projects_root().unwrap();
        assert_eq!(root, expected);
        unset_scan_root();
    }

    #[test]
    fn resolve_root_default_without_env() {
        // 未设 env → home/.claude/projects（依赖 --test-threads=1 门禁：先确保无残留）
        unset_scan_root();
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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        assert_eq!(
            sessions.len(),
            1,
            "应仅命中 1 条 UUID 会话，实际: {:?}",
            sessions.len()
        );
        assert_eq!(sessions[0].session_id, uuid);
    }

    #[test]
    fn scan_multiple_sessions_sorted_input_order() {
        // 多个编码目录 + 多个会话 → 全部收集（顺序无契约，只断言集合）
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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        let mut ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, [uuid1, uuid2, uuid3]);
    }

    #[test]
    fn scan_root_missing_returns_empty() {
        // 扫描根不存在 → 空 Vec（非 Err）
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("不存在");
        set_scan_root(&missing);
        let sessions = scan_sessions();
        unset_scan_root();
        assert!(sessions.is_empty());
    }

    // ── parse_session_file：降级条目 ──

    #[test]
    fn scan_corrupt_jsonl_produces_degraded_entry() {
        // 损坏 jsonl → 降级条目：仅 sessionId + mtime_ms，其余 None/none/false
        let (_dir, root, proj) = make_scan_root();
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        std::fs::write(proj.join(format!("{uuid}.jsonl")), "{broken json 没有换行").unwrap();

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        assert!(s.cwd.is_none());
        assert!(s.title.is_none());
        assert_eq!(s.title_source, TitleSource::None);
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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, uuid);
        assert!(sessions[0].title.is_none());
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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        let s = &sessions[0];
        assert_eq!(s.session_id, uuid);
        // 标题回退链：custom-title/ai-title 均无 → summary 赢
        assert_eq!(s.title.as_deref(), Some("修复登录 bug"));
        assert_eq!(s.title_source, TitleSource::Summary);
        assert_eq!(s.first_prompt.as_deref(), Some("帮我修 bug"));
        assert!(s.mtime_ms > 0);
        assert_eq!(s.cwd.as_deref(), Some(existing.to_str().unwrap()));
        assert!(s.cwd_exists, "cwd 指向存在的目录 → cwd_exists=true");
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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

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

        set_scan_root(&root);
        let sessions = scan_sessions();
        unset_scan_root();

        let s = &sessions[0];
        assert_eq!(s.title.as_deref(), Some("重命名后的标题"));
        assert_eq!(s.title_source, TitleSource::CustomTitle);
    }
}
