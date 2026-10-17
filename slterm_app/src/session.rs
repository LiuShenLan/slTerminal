//! 会话文件身份(03 篇协议解析的支撑件)。
//!
//! 本步(M4.2)落地范围:hook 信封解析(`ai_hook::protocol`)需要的两个
//! 纯校验函数。布局会话树(`AgentSession`/恢复注入)归 05 篇 M5 续写;会话
//! 档案扫描(`ai_sessions`)归 M4.4,落地时复用本文件的 rollout id 提取。

/// 同时校验 Windows 与来宾绝对路径,不查宿主文件系统——真实存在性与原生
/// 身份归宿主侧校验。
pub(crate) fn valid_native_session_file(path: &str) -> bool {
    path.len() <= 4096
        && path.ends_with(".jsonl")
        && !path.chars().any(char::is_control)
        && (path.starts_with('/')
            || path.starts_with(r"\\")
            || (path.as_bytes().get(1) == Some(&b':')
                && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && matches!(path.as_bytes().get(2), Some(b'/' | b'\\'))))
}

/// codex rollout 文件名尾部的 thread uuid 是 `codex resume` 认的会话身份。
/// 形状钉死:`rollout-<日期时间>-<uuid>.jsonl`,uuid 36 字符、连字符位固定。
pub(crate) fn codex_rollout_id(path: &str) -> Option<&str> {
    if !valid_native_session_file(path) {
        return None;
    }
    let filename = path.rsplit(['/', '\\']).next()?;
    let stem = filename.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let start = stem.len().checked_sub(36)?;
    let id = stem.get(start..)?;
    if start == 0 || stem.as_bytes().get(start - 1) != Some(&b'-') {
        return None;
    }
    id.bytes()
        .enumerate()
        .all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        .then_some(id)
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn native_session_file_accepts_absolute_jsonl_paths_only() {
        for path in [
            r"C:\Users\user\.codex\sessions\2026\09\21\rollout-x.jsonl",
            "C:/Users/user/.claude/projects/p/x.jsonl",
            "/home/user/.codex/sessions/rollout-x.jsonl",
            r"\\wsl$\distro\home\user\x.jsonl",
        ] {
            assert!(valid_native_session_file(path), "{path}");
        }
        for path in [
            "relative/rollout-x.jsonl",
            "x.jsonl.txt",
            r"C:\Users\user\x.json",
            "",
        ] {
            assert!(!valid_native_session_file(path), "{path}");
        }
        assert!(
            !valid_native_session_file("C:/x/evil\n.jsonl"),
            "控制字符拒收"
        );
    }

    #[test]
    fn rollout_id_comes_only_from_a_well_shaped_filename_tail() {
        const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";
        for root in [
            r"C:\Users\user\.codex\sessions",
            "/home/user/.codex/sessions",
        ] {
            let path = format!("{root}/2026/09/21/rollout-2026-09-21T10-00-00-{THREAD}.jsonl");
            assert_eq!(codex_rollout_id(&path), Some(THREAD));
        }
        for path in [
            format!("/sessions/rollout-{THREAD}.jsonl"),
            format!("/sessions/rollout-2026-09-21-{THREAD}extra.jsonl"),
            "/sessions/x.jsonl".to_owned(),
        ] {
            assert_eq!(codex_rollout_id(&path), None, "{path}");
        }
    }
}
