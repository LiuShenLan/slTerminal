//! 历史会话写操作 —— SEC-01 sessionId 校验 + 删除/重命名命令（BE-07/BE-08）
//!
//! 职责：
//! - `validate_session_id()`：SEC-01 严格校验（仅 UUID 形态，非法 → Validation）
//! - `claude_history_delete`：删除 `<id>.jsonl` + 同名 `<id>/` 附属目录（BE-07）
//! - `claude_history_rename`：追加 custom-title 行（BE-08，决策 20/22）
//!
//! 安全模型（SEC-01）：定位只接受 sessionId（不信托前端任何路径参数），
//! 文件路径全部由「扫描根 + 一级子目录名 + 校验过的 sessionId」拼接派生。
//! 两命令写用户 home 目录文件、绕过 project_root 沙箱（照 hooks/config.rs user 层先例），
//! 入参即攻击面——sessionId 严格校验是唯一防线。

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::claude_history::{is_uuid_filename, scan::resolve_projects_root};

/// 重命名标题最大字符数（BE-08：trim 后 ≤200，按字符计）
const TITLE_MAX_CHARS: usize = 200;

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

/// 「会话不存在」错误（BE-07/BE-08：jsonl 找不到 → Err 且消息含「不存在」语义）
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

/// claude_history_rename 命令（BE-08）
///
/// SEC-01 校验 + new_title trim 后非空且 ≤200 字符 → spawn_blocking 内定位 →
/// append 追加一行 custom-title（serde_json 序列化，禁手拼字符串防注入）。
#[tauri::command]
pub async fn claude_history_rename(
    session_id: String,
    new_title: String,
) -> Result<(), crate::AppError> {
    tokio::task::spawn_blocking(move || rename_session(&session_id, &new_title))
        .await
        .map_err(crate::AppError::from)?
}

/// 重命名会话纯逻辑（阻塞 I/O，供命令 spawn_blocking 包装 + 单元测试直测）
///
/// 追加写与运行中会话写入无冲突（决策 20/22），不做原子重写。
/// 追加行格式：`{"type":"custom-title","customTitle":<名>,"sessionId":<id>}`。
fn rename_session(session_id: &str, new_title: &str) -> Result<(), crate::AppError> {
    validate_session_id(session_id)?;
    let title = new_title.trim();
    if title.is_empty() {
        return Err(crate::AppError::Validation(
            "重命名标题不能为空".to_string(),
        ));
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(crate::AppError::Validation(format!(
            "重命名标题超过 {TITLE_MAX_CHARS} 字符上限"
        )));
    }
    let Some(root) = resolve_projects_root() else {
        return Err(session_not_found(session_id));
    };
    let Some(jsonl) = locate_session_jsonl(&root, session_id) else {
        return Err(session_not_found(session_id));
    };
    let mut f = std::fs::OpenOptions::new().append(true).open(&jsonl)?;
    ensure_trailing_newline(&mut f, &jsonl)?;
    // serde_json 序列化构造（禁手拼字符串防注入；Value Display 为紧凑 JSON）
    let line = serde_json::json!({
        "type": "custom-title",
        "customTitle": title,
        "sessionId": session_id,
    });
    writeln!(f, "{line}")?;
    Ok(())
}

/// 追加前保证文件以换行结尾（JSONL 行完整）；文件为空则无需处理
///
/// append 句柄的 seek 无效（恒写末尾），故用独立只读句柄探测末字节。
fn ensure_trailing_newline(f: &mut std::fs::File, path: &Path) -> Result<(), crate::AppError> {
    use std::io::{Read, Seek, SeekFrom};
    let len = f.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    let mut probe = std::fs::File::open(path)?;
    probe.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    probe.read_exact(&mut last)?;
    if last[0] != b'\n' {
        f.write_all(b"\n")?;
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

    // ── BE-08：rename ──

    #[test]
    fn rename_appends_custom_title_line_preserving_content() {
        // 追加行三字段逐字断言 + 原文件内容不变
        let (_dir, root, proj) = make_scan_root();
        let path = proj.join(format!("{UUID}.jsonl"));
        let orig1 = serde_json::json!({"type": "summary", "summary": "旧标题"}).to_string();
        let orig2 = serde_json::json!({
            "type": "user",
            "message": { "content": "你好" },
        })
        .to_string();
        std::fs::write(&path, format!("{orig1}\n{orig2}\n")).unwrap();

        set_scan_root(&root);
        rename_session(UUID, "重命名后的标题").unwrap();
        unset_scan_root();

        let content = std::fs::read_to_string(&path).unwrap();
        let mut lines = content.lines();
        // 原两行内容不变
        assert_eq!(lines.next(), Some(orig1.as_str()));
        assert_eq!(lines.next(), Some(orig2.as_str()));
        // 追加行：JSON 反序列化后三字段逐字断言
        let last = lines.next().expect("应追加一行 custom-title");
        assert!(lines.next().is_none(), "不应有多余行");
        let v: serde_json::Value = serde_json::from_str(last).unwrap();
        assert_eq!(v["type"], "custom-title");
        assert_eq!(v["customTitle"], "重命名后的标题");
        assert_eq!(v["sessionId"], UUID);
    }

    #[test]
    fn rename_repairs_missing_trailing_newline() {
        // 文件非空且末字节非 \n → 先补 \n 再写行（保证 JSONL 行完整）
        let (_dir, root, proj) = make_scan_root();
        let path = proj.join(format!("{UUID}.jsonl"));
        std::fs::write(&path, r#"{"type":"summary","summary":"s"}"#).unwrap(); // 无尾 \n

        set_scan_root(&root);
        rename_session(UUID, "标题").unwrap();
        unset_scan_root();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "补 \n 后应恰好两行");
        // 两行均为合法 JSON（原行未被续写破坏）
        for l in &lines {
            assert!(
                serde_json::from_str::<serde_json::Value>(l).is_ok(),
                "行应为完整 JSON，实际: {l}"
            );
        }
    }

    #[test]
    fn rename_rejects_empty_or_oversized_title() {
        // 空串 / 纯空白 / >200 字符 → Validation 拒绝，且文件未被改动（校验先于写）
        let (_dir, root, proj) = make_scan_root();
        let path = proj.join(format!("{UUID}.jsonl"));
        std::fs::write(&path, "line1\n").unwrap();

        let long_title = "长".repeat(TITLE_MAX_CHARS + 1);
        set_scan_root(&root);
        for bad in ["", "   ", long_title.as_str()] {
            let msg = assert_validation(rename_session(UUID, bad).unwrap_err());
            assert!(!msg.is_empty());
        }
        unset_scan_root();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "line1\n",
            "非法标题不应改动文件"
        );
    }

    #[test]
    fn rename_accepts_200_char_title() {
        // 边界：trim 后恰 200 字符合法
        let (_dir, root, proj) = make_scan_root();
        let path = proj.join(format!("{UUID}.jsonl"));
        std::fs::write(&path, "line1\n").unwrap();
        let title_200 = "题".repeat(TITLE_MAX_CHARS);

        set_scan_root(&root);
        rename_session(UUID, &title_200).unwrap();
        unset_scan_root();

        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(content.lines().last().unwrap()).unwrap();
        assert_eq!(v["customTitle"], title_200);
    }

    #[test]
    fn rename_missing_session_returns_not_found() {
        let (_dir, root, _proj) = make_scan_root();
        set_scan_root(&root);
        let msg = assert_validation(rename_session(UUID, "标题").unwrap_err());
        unset_scan_root();
        assert!(
            msg.contains("不存在"),
            "消息应含「不存在」语义，实际: {msg}"
        );
    }

    #[test]
    fn rename_rejects_invalid_session_id() {
        let (_dir, root, _proj) = make_scan_root();
        set_scan_root(&root);
        let msg = assert_validation(rename_session("a/b", "标题").unwrap_err());
        unset_scan_root();
        assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
    }

    // ── 越界防护（BE-10：扫描根外无写入） ──

    #[test]
    fn delete_and_rename_stay_within_scan_root() {
        // 扫描根外放哨兵文件：rename/delete 全程不触碰（路径均由扫描根+校验 id 派生）
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
        rename_session(UUID, "新标题").unwrap();
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
