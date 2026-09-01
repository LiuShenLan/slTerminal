//! claude 历史会话写操作 —— SEC-05 sessionId 校验 + 删除（BE-07，MC-301/304 下沉）
//!
//! 职责：
//! - `validate_session_id()`：SEC-05 严格校验（仅 UUID 形态，非法 → Validation）
//! - `delete_session()`：删除 `<id>.jsonl` + 同名 `<id>/` 附属目录（BE-07）
//!
//! 安全模型（SEC-05）：定位只接受 sessionId（不信托前端任何路径参数），
//! 文件路径全部由「扫描根 + 一级子目录名 + 校验过的 sessionId」拼接派生。
//! 命令写用户 home 目录文件、绕过 project_root 沙箱（照 hooks/config.rs user 层先例），
//! 入参即攻击面——sessionId 严格校验是唯一防线。
//! 命令 `agent_history_delete` 在聚合层 mod.rs 经 provider `validate_session_id`
//! 前置后分发到本模块（trait 契约强制，MC-304）。

use std::path::{Path, PathBuf};

use crate::agent_history::claude::jsonl;
use crate::agent_history::claude::scan::resolve_projects_root;
use crate::agent_history::{is_uuid_filename, AgentHistoryTitle};

/// SEC-05 sessionId 严格校验
///
/// 仅接受 UUID 形态（8-4-4-4-12 十六进制带连字符，大小写不敏感）。
/// 该形态天然拒绝含 `..`、路径分隔符（`/`、`\`）、空串、超长等一切非 UUID 输入
/// （复用聚合层 `is_uuid_filename`：36 长度 + 连字符位置 + ascii hex 全检）。
/// 非法 → `AppError::Validation`。provider impl 的 `validate_session_id` 委托本函数。
pub(crate) fn validate_session_id(session_id: &str) -> Result<(), crate::AppError> {
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

/// 在扫描根一级子目录中定位 `<session_id>.jsonl`（SEC-05 定位，不递归子目录）
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
        // AQ-3 符号链接拒跟随：一级子目录为 symlink（可能指向扫描根外）→ 跳过
        if !dir_path.is_dir() || dir_path.is_symlink() {
            continue;
        }
        let candidate = dir_path.join(&target);
        // AQ-3：命中文件为 symlink → 不命中（防外部文件被定位/删除）
        if candidate.is_file() && !candidate.is_symlink() {
            return Some(candidate);
        }
    }
    None
}

/// 删除会话纯逻辑（阻塞 I/O，供 provider impl 的 `delete` 委托 + 单元测试直测）
///
/// SEC-05 校验 → 定位 → 删除 `<id>.jsonl` + 同名 `<id>/` 目录（存在则 remove_dir_all，
/// 含 subagents 等附属数据）；jsonl 不存在 → Err。
/// 命令层已经 provider `validate_session_id` 前置，本函数内部自带校验兜底——
/// 零行为改动（SEC-05 保留）。
pub(crate) fn delete_session(session_id: &str) -> Result<(), crate::AppError> {
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
        // AQ-3：同名目录为 symlink（指向扫描根外）→ 拒绝删除，不跟随链接目标
        if session_dir.is_dir() && !session_dir.is_symlink() {
            std::fs::remove_dir_all(&session_dir)?;
        }
    }
    Ok(())
}

/// 读取单会话标题（运行中会话页签/会话行显示名通道，人工验证问题 3）
///
/// SEC-05 校验 → 定位 → `parse_head` + `parse_tail_title` → `resolve_title`
/// ——回退链与 `scan.rs` 的 `parse_session_file` **完全同源**（custom-title >
/// ai-title > summary > firstPrompt），运行中页签/会话行标题与历史 session
/// 标题一致。
/// **文件未定位 → `Ok(title: None, source: "none")`**：运行中会话的 jsonl 可能
/// 尚未创建（SessionStart 早于落盘）或已被删除——读是幂等查询，属正常条件，
/// 与 delete 的「会话不存在 → Err」语义区分（删是有副作用操作）。
pub(crate) fn read_session_title(session_id: &str) -> Result<AgentHistoryTitle, crate::AppError> {
    validate_session_id(session_id)?;
    let Some(root) = resolve_projects_root() else {
        return Ok(AgentHistoryTitle {
            title: None,
            title_source: "none".to_string(),
        });
    };
    let Some(jsonl_path) = locate_session_jsonl(&root, session_id) else {
        return Ok(AgentHistoryTitle {
            title: None,
            title_source: "none".to_string(),
        });
    };
    let head = jsonl::parse_head(&jsonl_path);
    let tail = jsonl::parse_tail_title(&jsonl_path);
    let (title, source) = jsonl::resolve_title(&head, tail);
    Ok(AgentHistoryTitle {
        title,
        title_source: source.as_str().to_string(),
    })
}

#[cfg(test)]
mod ops_tests {
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

    // ── SEC-05：sessionId 校验 ──

    #[test]
    fn validate_session_id_accepts_uuid_forms() {
        // 小写 / 大写 hex 均合法（大小写不敏感）
        assert!(validate_session_id(UUID).is_ok());
        assert!(validate_session_id("123E4567-E89B-12D3-A456-426614174000").is_ok());
    }

    #[test]
    fn validate_session_id_rejects_non_uuid_inputs() {
        // SEC-05 五类非法输入全拒：含 `..` / 含 `/` / 含 `\` / 空串 / 非 UUID 形态
        for bad in [
            "..",
            "abc/def",
            "abc\\def",
            "123e4567-e89b-12d3-a456", // 非 UUID（长度不足）
        ] {
            let msg = assert_validation(validate_session_id(bad).unwrap_err());
            assert!(msg.contains(bad), "错误消息应含非法输入，实际: {msg}");
        }
        // 空串特判（HFN-09③）：contains("") 恒真、无法验证消息内容——改断言具体校验文案
        let msg = assert_validation(validate_session_id("").unwrap_err());
        assert!(
            msg.contains("非法 sessionId"),
            "错误消息应含校验文案，实际: {msg}"
        );
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

    // ── read_session_title：运行中会话标题通道（人工验证问题 3） ──

    #[test]
    fn read_title_resolves_custom_title_then_ai_title() {
        // 回退链与 scan 同源：custom-title 恒优先，其次 ai-title
        // 1. 尾部 custom-title 恒优先（决策 22）
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(
            proj.join(format!("{UUID}.jsonl")),
            concat!(
                "{\"type\":\"summary\",\"summary\":\"旧摘要\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"自动标题\"}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"用户重命名\",\"sessionId\":\"x\"}\n",
            ),
        )
        .unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert_eq!(t.title.as_deref(), Some("用户重命名"));
        assert_eq!(t.title_source, "customTitle");

        // 2. 无 custom-title → ai-title
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(
            proj.join(format!("{UUID}.jsonl")),
            "{\"type\":\"ai-title\",\"aiTitle\":\"自动标题\"}\n",
        )
        .unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert_eq!(t.title.as_deref(), Some("自动标题"));
        assert_eq!(t.title_source, "aiTitle");
    }

    #[test]
    fn read_title_falls_back_to_summary_then_first_prompt_then_none() {
        // 3. summary
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(
            proj.join(format!("{UUID}.jsonl")),
            "{\"type\":\"summary\",\"summary\":\"会话摘要\"}\n{\"type\":\"user\",\"message\":{\"content\":\"首条提问\"}}\n",
        )
        .unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert_eq!(t.title.as_deref(), Some("会话摘要"));
        assert_eq!(t.title_source, "summary");

        // 4. 仅 firstPrompt（运行中会话常见形态：首条消息已写入、无任何标题类字段）
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(
            proj.join(format!("{UUID}.jsonl")),
            "{\"type\":\"user\",\"message\":{\"content\":\"首条提问\"}}\n",
        )
        .unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert_eq!(t.title.as_deref(), Some("首条提问"));
        assert_eq!(t.title_source, "firstPrompt");

        // 5. 空文件 → 四路全无 → None（前端兜底 CLI 名）
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "").unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
    }

    #[test]
    fn read_title_missing_file_returns_none_ok() {
        // 会话文件不存在（运行中会话尚未落盘）→ Ok(title: None)——正常条件非错误
        // （与 delete 的「会话不存在 → Err」语义区分：读是幂等查询）
        let (_dir, root, _proj) = make_scan_root();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
    }

    #[test]
    fn read_title_rejects_invalid_session_id() {
        // 非法 sessionId 在触碰文件系统前被拒（SEC-05 校验前置，端到端 1 条）
        let (_dir, root, proj) = make_scan_root();
        set_scan_root(&root);
        let msg = assert_validation(read_session_title("../evil").unwrap_err());
        unset_scan_root();
        assert!(msg.contains("非法"), "消息应说明非法，实际: {msg}");
        // 越界文件未被触碰
        assert!(!proj.join("..").join("evil.jsonl").exists());
    }

    #[test]
    fn read_title_tail_custom_title_overrides_head_summary() {
        // 文件 >512KB：头部窗口内 summary、尾部 64KB 内 custom-title → 尾部赢（决策 22）
        let (_dir, root, proj) = make_scan_root();
        let path = proj.join(format!("{UUID}.jsonl"));
        let mut f = std::fs::File::create(&path).unwrap();
        use std::io::Write;
        writeln!(
            f,
            r#"{{"type":"summary","summary":"头部摘要","leafUuid":"x"}}"#
        )
        .unwrap();
        let line = b"{\"type\":\"pad\",\"payload\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}\n";
        while f.metadata().unwrap().len() < 600 * 1024 {
            f.write_all(line).unwrap();
        }
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"尾部重命名","sessionId":"{UUID}"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert_eq!(t.title.as_deref(), Some("尾部重命名"));
        assert_eq!(t.title_source, "customTitle");
    }

    #[test]
    fn read_title_corrupt_jsonl_returns_none_ok() {
        // 损坏 jsonl → 解析容错降级 → 四路全无（None，非 Err——照单文件降级契约）
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "{broken json 没有换行").unwrap();
        set_scan_root(&root);
        let t = read_session_title(UUID).unwrap();
        unset_scan_root();
        assert!(t.title.is_none());
        assert_eq!(t.title_source, "none");
    }

    // ── AQ-3：符号链接拒跟随 ──
    //
    // Windows 符号链接创建需管理员权限/开发者模式（CI runner 权限差异）——
    // 创建失败时测试内直接 return 跳过（照 state.rs validate_symlink_* 先例）。
    //
    // 豁免（BE-17/D5）：下方三条测试保留 #[cfg(windows)]——测试调用
    // std::os::windows::fs::symlink_dir/symlink_file，该 API 仅 Windows target
    // 编译期存在，非 Windows 无法编译，故不能改 cfg!(windows) 运行时分支；
    // 且创建 symlink 需 Windows 特权（管理员/开发者模式），无权限时运行时跳过。

    /// 一级子目录为 symlink（指向扫描根外）→ 定位不命中
    // 豁免（BE-17/D5）：依赖 std::os::windows::fs::symlink_dir（仅 Windows target 编译期存在，
    // 非 Windows 无法编译）+ symlink 创建需 Windows 特权，失败时运行时跳过
    #[cfg(windows)]
    #[test]
    fn locate_skips_symlinked_subdir() {
        let (_dir, root, _proj) = make_scan_root();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join(format!("{UUID}.jsonl")), "{}").unwrap();

        let link_dir = root.join("C--Users-test-link");
        if std::os::windows::fs::symlink_dir(external.path(), &link_dir).is_err() {
            return; // 无权限创建符号链接，跳过
        }

        assert!(
            locate_session_jsonl(&root, UUID).is_none(),
            "symlink 子目录不应被跟随定位"
        );
    }

    /// 命中文件为 symlink（指向扫描根外文件）→ 定位不命中 + 删除按「会话不存在」拒绝
    // 豁免（BE-17/D5）：依赖 std::os::windows::fs::symlink_file（仅 Windows target 编译期存在，
    // 非 Windows 无法编译）+ symlink 创建需 Windows 特权，失败时运行时跳过
    #[cfg(windows)]
    #[test]
    fn locate_and_delete_reject_symlinked_jsonl() {
        let (_dir, root, proj) = make_scan_root();
        let external = tempfile::tempdir().unwrap();
        let ext_file = external.path().join(format!("{UUID}.jsonl"));
        std::fs::write(&ext_file, "{}").unwrap();

        let link = proj.join(format!("{UUID}.jsonl"));
        if std::os::windows::fs::symlink_file(&ext_file, &link).is_err() {
            return; // 无权限创建符号链接，跳过
        }

        set_scan_root(&root);
        assert!(
            locate_session_jsonl(&root, UUID).is_none(),
            "symlink 文件不应命中定位"
        );
        let msg = assert_validation(delete_session(UUID).unwrap_err());
        unset_scan_root();
        assert!(
            msg.contains("不存在"),
            "symlink 文件删除应按「会话不存在」拒绝，实际: {msg}"
        );
        // 扫描根外真实文件未被触碰
        assert!(ext_file.exists());
    }

    /// 同名 <id>/ 目录为 symlink → 删除不跟随，外部目录内容保留
    // 豁免（BE-17/D5）：依赖 std::os::windows::fs::symlink_dir（仅 Windows target 编译期存在，
    // 非 Windows 无法编译）+ symlink 创建需 Windows 特权，失败时运行时跳过
    #[cfg(windows)]
    #[test]
    fn delete_ignores_symlinked_session_dir() {
        let (_dir, root, proj) = make_scan_root();
        std::fs::write(proj.join(format!("{UUID}.jsonl")), "{}").unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("keep.txt"), "keep").unwrap();

        let session_link = proj.join(UUID);
        if std::os::windows::fs::symlink_dir(external.path(), &session_link).is_err() {
            return; // 无权限创建符号链接，跳过
        }

        set_scan_root(&root);
        delete_session(UUID).unwrap();
        unset_scan_root();

        assert!(
            !proj.join(format!("{UUID}.jsonl")).exists(),
            "jsonl 应被删除"
        );
        assert_eq!(
            std::fs::read_to_string(external.path().join("keep.txt")).unwrap(),
            "keep",
            "symlink 指向的外部目录内容不应被删除"
        );
    }
}
