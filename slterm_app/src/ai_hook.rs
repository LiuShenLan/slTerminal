//! AI 生命周期集成。依赖流向:
//! 传输 -> 协议/载荷 -> 类型化事件 -> 排序 -> pane 生命周期。
//!
//! UI 适配器渲染共享的生命周期并投递它的通知,绝不通过扫终端散文重新解释
//! 一条已完成的 hook。屏幕证据是对当前集成缺失能力的显式兜底。
//!
//! `SLTERM_HOOK_LOG` 诊断桥接投递而不记载荷;`GateVerdict` 在应用调试日志里
//! 解释每一条被拒事件。
//!
//! 落盘范围:协议/载荷/事件门/仲裁(M4.2)、安装权威与 9 家安装器(M4.3)、
//! 壳侧事件通道(M4.5:命名管道线程 → 进程内 mpsc → GPUI foreground
//! 排水 → `AgentActivity`/`AnswerInbox` 的 pane 宿主)。
//!
//! 缝合点(不在本模块消费):设置页数据源消费 `integrations::inspect()` 归
//! 06 篇;`config_guard` 随设置域启动链挂接归 06 篇;通知漏斗消费
//! TurnDone/NeedsAttention 两个状态边归 09 篇 M10.1。

mod bridges;
/// 设置页 hooks 编辑域(轨二):用户/第三方 handler 可编辑,marker 认领组只读。
pub(crate) mod editor;
mod event;
pub(crate) mod installation;
pub(crate) mod integrations;
mod lifecycle;
mod local;
mod native_events;
mod ordering;
mod payload;
mod protocol;
mod windows;

// re-export 面是壳侧/05/09 篇的缝合点(事件排水、pane 宿主、通知漏斗)。
pub(crate) use event::CodexHookMode;
pub use event::{
    AiBackgroundTasks, AiHookEvent, AiHookKind, AiPermissionMode, AiTurnOutcome, AttentionContext,
};
#[allow(unused_imports)] // 能力表消费点归 09 篇通知漏斗 / 04 篇 runtime 快照。
pub use event::{AiHookCapabilities, capabilities_for};
pub(crate) use lifecycle::AgentActivity;
pub use ordering::GateVerdict;
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
fn contains_helper(value: &str) -> bool {
    value.contains("slterm-hook")
}

/// 幂等认领完整自有命令(安装器用)。
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
/// POSIX sh 的单引号转义形态随 unix/remote 整支裁剪:Windows hook 命令
/// 只认双引号包裹,单引号路径直接判非自有。
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
const HELPER_ARGS: [&str; 1] = ["claude"];

/// 我们订阅的 claude hook 事件。会话边界携带 resume/fork 需要的 id;
/// PostToolUse 让过期的权限状态在整个回合完成之前就能回到 working。
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

/// 卸载器入口(`setup-ai --remove`):移除全部 AI CLI 的 hook 注入。
pub(crate) use local::settings::remove_all as remove_all_hooks;

// 壳侧启动链的唯一进程级入口(M4.5);返回句柄随 `run_shell` 作用域存活。
pub use windows::spawn_gpui_server;

/// config_guard 落盘本体在 local;消费点 = 壳启动链 `run_shell`(C4)。
pub use local::spawn_config_guard;

#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod ownership_tests {
    use super::*;

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
