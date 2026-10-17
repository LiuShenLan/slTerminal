//! AI 生命周期集成。依赖流向:
//! 传输 -> 协议/载荷 -> 类型化事件 -> 排序 -> pane 生命周期。
//!
//! UI 适配器渲染共享的生命周期并投递它的通知,绝不通过扫终端散文重新解释
//! 一条已完成的 hook。屏幕证据是对当前集成缺失能力的显式兜底。
//!
//! `SLTERM_HOOK_LOG` 诊断桥接投递而不记载荷;`GateVerdict` 在应用调试日志里
//! 解释每一条被拒事件。
//!
//! 本步(M4.2)落盘范围:协议与仲裁层(信封解析/有界载荷/事件门/每 pane
//! 仲裁/命名管道传输)。安装权威(`installation`/`local`/`bridges`/
//! `integrations`)归 M4.3;壳侧接线(GPUI foreground executor 排水、
//! `AgentActivity` 挂 pane 宿主)归 M4.5/05 篇。

mod event;
pub(crate) mod lifecycle;
mod native_events;
mod ordering;
mod payload;
mod protocol;
mod windows;

// re-export 面是 M4.5/05/09 篇的缝合点(壳侧排水、pane 宿主、通知漏斗);
// 本步(crate 内尚无壳消费)静默 unused,测试经这些路径消费。
#[allow(unused_imports)]
pub(crate) use event::CodexHookMode;
#[allow(unused_imports)]
pub use event::{
    AiBackgroundTasks, AiHookCapabilities, AiHookEvent, AiHookKind, AiPermissionMode,
    AiTurnOutcome, AttentionContext, capabilities_for,
};
#[allow(unused_imports)]
pub use ordering::GateVerdict;
#[allow(unused_imports)]
pub(crate) use ordering::{accept_for_pane, reorder_batch};
pub(crate) use protocol::parse_envelope;

#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod tests;

/// 把本实例管道名带进子 shell 的环境变量(ConPTY 会合并当前进程环境,在首个
/// PTY spawn 之前进程级 set 即覆盖全部 pane)。
pub const PIPE_ENV: &str = "SLTERM_NOTIFY_PIPE";
/// 每 pane 身份,注入各 pane 的 PTY 环境。
pub const PANE_ENV: &str = "SLTERM_PANE_ID";
/// `slterm-hook.exe` 的绝对路径,导出给 opencode Bun 插件(它自己解析不出
/// slterm.exe 的安装目录,只能 shell out 到这座桥)。作用域同 [`PIPE_ENV`]。
pub const HOOK_EXE_ENV: &str = "SLTERM_HOOK_EXE";

/// 仅用于阻止未知 notify 包装器再次套娃;子串不能作为修改/删除的归属依据。
///
/// 消费点归 M4.3 安装器(codex notify 槽包装 #38 防线)。
#[allow(dead_code)]
fn contains_helper(value: &str) -> bool {
    value.contains("slterm-hook")
}

/// 消费点归 M4.3 安装器(幂等认领完整自有命令)。
#[allow(dead_code)]
fn is_helper_executable(value: &str) -> bool {
    let filename = value.rsplit(['/', '\\']).next().unwrap_or_default();
    let absolute = value.starts_with('/')
        || value.starts_with("\\\\")
        || (value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && value.as_bytes().get(1) == Some(&b':')
            && matches!(value.as_bytes().get(2), Some(b'/' | b'\\')));
    (value == filename || absolute)
        && !value.contains(['"', '\r', '\n', '\0'])
        && ["slterm-hook.exe", "slterm-hook"]
            .iter()
            .any(|name| filename.eq_ignore_ascii_case(name))
}

/// 只接受安装器产生的单条调用;echo、管道和追加命令仍属于用户。
///
/// POSIX sh 的单引号转义形态(上游经 remote::quote 校验)随 unix/remote 整支
/// 裁剪:Windows hook 命令只认双引号包裹,单引号路径直接判非自有。
///
/// 消费点归 M4.3 安装器。
#[allow(dead_code)]
fn is_helper_shell_command(command: &str, source: &str) -> bool {
    let Some(path) = command
        .trim()
        .strip_suffix(source)
        .and_then(|s| s.strip_suffix(' '))
    else {
        return false;
    };
    let path = path.trim_end();
    let executable = if let Some(quoted) = path.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
    {
        if quoted.contains('"') {
            return false;
        }
        quoted
    } else {
        if path.chars().any(char::is_whitespace)
            || path.contains(['"', '\'', ';', '&', '|', '<', '>'])
        {
            return false;
        }
        path
    };
    is_helper_executable(executable)
}

/// hook 条目的 argv 尾。`claude` 是 `slterm-hook` 从 `args[0]` 读的 source
/// 判别符,必须作为真实参数传递:若改成拼进命令字符串,就得由某个 shell
/// 重解析整行——那正是上游 #80 炸掉的形态。
///
/// 消费点归 M4.3 安装器。
#[allow(dead_code)]
const HELPER_ARGS: [&str; 1] = ["claude"];

/// 我们订阅的 claude hook 事件。会话边界携带 resume/fork 需要的 id;
/// PostToolUse 让过期的权限状态在整个回合完成之前就能回到 working。
///
/// 消费点归 M4.3 安装器。
#[allow(dead_code)]
const CLAUDE_EVENTS: [&str; 10] = [
    "SessionStart",
    "UserPromptSubmit",
    "Notification",
    "PermissionRequest",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

#[allow(unused_imports)] // 消费点归 M4.5 壳侧接线。
pub use windows::spawn_gpui_server;

// ─── helper 定位(上游 local.rs 件,随 HOOK_EXE_ENV 导出需求提前落地)─────
// M4.3 安装器落地时与本件合并归位 local.rs;此处保持单一名单 slterm-hook。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// 配置监视器繁忙时避免每帧刷同一条警告。状态跃迁保持一次性嘈杂,重复探测
/// 静默,直到 helper 重新出现(或之后被移除)。
static HELPER_MISSING_ANNOUNCED: AtomicBool = AtomicBool::new(false);

fn helper_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let helper = helper_path_from_exe(&exe);
    match helper {
        Some(path) => {
            // 允许下一次状态跃迁重新报告一次安装/移除,而不是缓存一条过期路径。
            HELPER_MISSING_ANNOUNCED.store(false, Ordering::Relaxed);
            Some(path)
        }
        None => {
            if !HELPER_MISSING_ANNOUNCED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "ai_hook: hook helper missing from runtime/ and executable directory; AI integrations not installed"
                );
            }
            None
        }
    }
}

fn helper_path_from_exe(exe: &Path) -> Option<PathBuf> {
    let exe_dir = exe.parent()?;
    // 分类目录优先,同目录位置留给开发构建。
    let name = format!("slterm-hook{}", std::env::consts::EXE_SUFFIX);
    [exe_dir.join("runtime").join(&name), exe_dir.join(&name)]
        .into_iter()
        .find(|path| {
            path.metadata()
                .map(|metadata| metadata.is_file())
                .unwrap_or(false)
        })
}

#[cfg(test)]
mod helper_path_tests {
    use super::*;

    #[test]
    fn helper_path_prefers_the_runtime_subdirectory() {
        let dir = std::env::temp_dir().join(format!("slterm-helper-path-{}", std::process::id()));
        let runtime = dir.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let exe = dir.join("slterm.exe");
        let runtime_helper = runtime.join("slterm-hook.exe");
        let sibling_helper = dir.join("slterm-hook.exe");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(&runtime_helper, b"").unwrap();
        assert_eq!(helper_path_from_exe(&exe), Some(runtime_helper.clone()));
        std::fs::remove_file(&runtime_helper).unwrap();
        std::fs::write(&sibling_helper, b"").unwrap();
        assert_eq!(helper_path_from_exe(&exe), Some(sibling_helper.clone()));
        std::fs::remove_file(&sibling_helper).unwrap();
        assert_eq!(helper_path_from_exe(&exe), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn helper_ownership_judges_only_complete_own_commands() {
        // 带空格路径的双引号包裹(上游 #80 教训)。
        assert!(is_helper_shell_command(
            r#""C:\Program Files\slTerminal\slterm-hook.exe" claude"#,
            "claude"
        ));
        assert!(is_helper_shell_command("slterm-hook claude", "claude"));
        // echo/管道/追加命令仍属于用户;单引号形态在 Windows 不是合法引用。
        for command in [
            r#"C:\slTerminal\slterm-hook.exe claude && echo done"#,
            r#"echo C:\slTerminal\slterm-hook.exe claude"#,
            "'C:\\slTerminal\\slterm-hook.exe' claude",
            r#"C:\slTerminal\slterm-hook.exe claude | more"#,
            r#"C:\slTerminal\other-hook.exe claude"#,
        ] {
            assert!(!is_helper_shell_command(command, "claude"), "{command}");
        }
        assert!(is_helper_executable(r"C:\slTerminal\SLTERM-HOOK.EXE"));
        assert!(!is_helper_executable("slterm-hook.exe; calc"));
        assert!(contains_helper(
            "--previous-notify C:/x/slterm-hook.exe codex"
        ));
        assert!(!contains_helper("--previous-notify C:/x/notify.exe codex"));
    }
}
