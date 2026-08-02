//! 历史会话写操作 —— SEC-01 sessionId 校验 + 删除命令（BE-07）
//!
//! 职责：
//! - `validate_session_id()`：SEC-01 严格校验（仅 UUID 形态，非法 → Validation）
//! - `claude_history_delete`：删除 `<id>.jsonl` + 同名 `<id>/` 附属目录（BE-07）
//!
//! 安全模型（SEC-01）：定位只接受 sessionId（不信托前端任何路径参数），
//! 文件路径全部由「扫描根 + 一级子目录名 + 校验过的 sessionId」拼接派生。
//! 命令写用户 home 目录文件、绕过 project_root 沙箱（照 hooks/config.rs user 层先例），
//! 入参即攻击面——sessionId 严格校验是唯一防线。

use std::path::{Path, PathBuf};

use crate::claude_history::{is_uuid_filename, scan::resolve_projects_root};

/// SEC-01 sessionId 严格校验
///
/// 仅接受 UUID 形态（8-4-4-4-12 十六进制带连字符，大小写不敏感）。
/// 该形态天然拒绝含 `..`、路径分隔符（`/`、`\`）、空串、超长等一切非 UUID 输入
/// （复用 mod.rs `is_uuid_filename`：36 长度 + 连字符位置 + ascii hex 全检）。
/// 非法 → `AppError::Validation`。
fn validate_session_id(session_id: &str) -> Result<(), crate::AppError> {
    if !is_uuid_filename(session_id) {
        return Err(crate::AppError::Validation(format!(
            "非法 sessionId: {session_id}"
        )));
    }
    Ok(())
}

/// 「会话不存在」错误（BE-07：jsonl 找不到 → Err 且消息含「不存在」语义）
fn session_not_found(session_id: &str) -> crate::AppError {
    crate::AppError::Validation(format!("会话不存在: {session_id}"))
}

/// 在扫描根一级子目录中定位 `<session_id>.jsonl`（SEC-01 定位，不递归子目录）
///
/// 遍历扫描根的一级子目录（cwd 编码目录），精确匹配文件名。
/// 扫描根不存在 / 未命中 → None（调用方按「会话不存在」处理）。
fn locate_session_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    let target = format!("{session_id}.jsonl");
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let candidate = dir_path.join(&target);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// claude_history_delete 命令（BE-07）
///
/// SEC-01 校验 → spawn_blocking 内定位 → 删除 `<id>.jsonl` + 同名 `<id>/` 目录
/// （存在则 remove_dir_all，含 subagents 等附属数据）；jsonl 不存在 → Err。
#[tauri::command]
pub async fn claude_history_delete(session_id: String) -> Result<(), crate::AppError> {
    tokio::task::spawn_blocking(move || delete_session(&session_id))
        .await
        .map_err(crate::AppError::from)?
}

/// 删除会话纯逻辑（阻塞 I/O，供命令 spawn_blocking 包装 + 单元测试直测）
fn delete_session(session_id: &str) -> Result<(), crate::AppError> {
    validate_session_id(session_id)?;
    let Some(root) = resolve_projects_root() else {
        return Err(session_not_found(session_id));
    };
    let Some(jsonl) = locate_session_jsonl(&root, session_id) else {
        return Err(session_not_found(session_id));
    };
    std::fs::remove_file(&jsonl)?;
    // 同名 <id>/ 目录（subagents 等附属数据）存在则一并删除（规格 4.4 删除范围）
    if let Some(dir) = jsonl.parent() {
        let session_dir = dir.join(session_id);
        if session_dir.is_dir() {
            std::fs::remove_dir_all(&session_dir)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppError;

    // ── 测试辅助 ──
    //
    // env 操作（SLTERM_CLAUDE_PROJECTS_DIR）依赖 --test-threads=1 门禁：
    // env 全局可变，并行测试会互相污染（同 scan.rs 测试约束）。
    // 路径经 dunce::canonicalize 统一长名（8.3 短名坑，照 git/CLAUDE.md 先例）。

    /// 创建扫描根 + 一个编码目录（cwd 编码形态），返回 (TempDir 守卫, 规范化根, 编码目录)
    fn make_scan_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let proj = root.join("C--Users-test-app");
        std::fs::create_dir_all(&proj).unwrap();
        (dir, root, proj)
    }

    fn set_scan_root(path: &Path) {
        std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", path);
    }

    fn unset_scan_root() {
        std::env::remove_var("SLTERM_CLAUDE_PROJECTS_DIR");
    }

    /// 断言错误为 Validation 变体（并返回消息供「不存在」语义断言）
    fn assert_validation(err: AppError) -> String {
        match err {
            AppError::Validation(msg) => msg,
            other => panic!("应为 AppError::Validation，实际: {other:?}"),
        }
    }

    const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";

    // ── SEC-01：sessionId 校验 ──

    #[test]
    fn validate_session_id_accepts_uuid_forms() {
        // 小写 / 大写 hex 均合法（大小写不敏感）
        assert!(validate_session_id(UUID).is_ok());
        assert!(validate_session_id("123E4567-E89B-12D3-A456-426614174000").is_ok());
    }

    #[test]
    fn validate_session_id_rejects_non_uuid_inputs() {
        // SEC-01 五类非法输入全拒：含 `..` / 含 `/` / 含 `\` / 空串 / 非 UUID 形态
        for bad in [
            "..",
            "abc/def",
            "abc\\def",
            "",
            "123e4567-e89b-12d3-a456", // 非 UUID（长度不足）
        ] {
            let msg = assert_validation(validate_session_id(bad).unwrap_err());
            assert!(msg.contains(bad), "错误消息应含非法输入，实际: {msg}");
        }
    }

    // ── BE-07：delete ──

    #[test]
    fn delete_removes_jsonl_and_same_name_dir() {
        // 删除范围：jsonl + 同名 <id>/ 目录（含 subagents 附属数据）一并删除
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "{}").unwrap();
        let session_dir = proj.join(UUID);
        let sub = session_dir.join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("agent-x.jsonl"), "{}").unwrap();

        set_scan_root(&root);
        delete_session(UUID).unwrap();
        unset_scan_root();

        assert!(
            !proj.join(format!("{UUID}.jsonl")).exists(),
            "jsonl 应被删除"
        );
        assert!(!session_dir.exists(), "同名目录（附属数据）应一并删除");
    }

    #[test]
    fn delete_without_same_name_dir_only_removes_jsonl() {
        // 无同名目录 → 仅删 jsonl，不报错；同目录其他会话不受影响
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "{}").unwrap();
        let other = "123e4567-e89b-12d3-a456-426614174001";
        std::fs::write(proj.join(format!("{other}.jsonl")), "{}").unwrap();

        set_scan_root(&root);
        delete_session(UUID).unwrap();
        unset_scan_root();

        assert!(!proj.join(format!("{UUID}.jsonl")).exists());
        assert!(
            proj.join(format!("{other}.jsonl")).exists(),
            "其他会话不受影响"
        );
    }

    #[test]
    fn delete_missing_session_returns_not_found() {
        // 文件不存在 → Err 且消息含「不存在」语义
        let (_dir, root, _proj) = make_scan_root();
        set_scan_root(&root);
        let msg = assert_validation(delete_session(UUID).unwrap_err());
        unset_scan_root();
        assert!(
            msg.contains("不存在"),
            "消息应含「不存在」语义，实际: {msg}"
        );
    }

    #[test]
    fn delete_rejects_invalid_session_id() {
        // 非法 sessionId 在触碰文件系统前被拒（端到端 1 条，全表见 validate 测试）
        let (_dir, root, proj) = make_scan_root();
        set_scan_root(&root);
        let msg = assert_validation(delete_session("../evil").unwrap_err());
        unset_scan_root();
        assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
        // 越界文件未被触碰
        assert!(!proj.join("..").join("evil.jsonl").exists());
    }

    // ── 越界防护（BE-10：扫描根外无写入） ──

    #[test]
    fn delete_stays_within_scan_root() {
        // 扫描根外放哨兵文件：delete 全程不触碰（路径均由扫描根+校验 id 派生）
        let outer = tempfile::tempdir().unwrap();
        let outer_canon = dunce::canonicalize(outer.path()).unwrap();
        let root = outer_canon.join("projects");
        std::fs::create_dir_all(&root).unwrap();
        let proj = root.join("C--Users-test-app");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "line1\n").unwrap();
        let canary = outer_canon.join("canary.txt");
        std::fs::write(&canary, "keep").unwrap();

        set_scan_root(&root);
        delete_session(UUID).unwrap();
        unset_scan_root();

        assert_eq!(
            std::fs::read_to_string(&canary).unwrap(),
            "keep",
            "扫描根外哨兵文件不应被写入/删除"
        );
        assert!(!proj.join(format!("{UUID}.jsonl")).exists());
    }
}
