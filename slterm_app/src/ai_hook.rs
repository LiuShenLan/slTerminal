//! AI 生命周期集成。依赖流向:
//! 传输 -> 协议/载荷 -> 类型化事件 -> 排序 -> pane 生命周期。
//!
//! UI 适配器渲染共享的生命周期并投递它的通知,绝不通过扫终端散文重新解释
//! 一条已完成的 hook。屏幕证据是对当前集成缺失能力的显式兜底。
//!
//! `SLTERM_HOOK_LOG` 诊断桥接投递而不记载荷;`GateVerdict` 在应用调试日志里
//! 解释每一条被拒事件。
//!
//! 落盘范围:协议/载荷/事件门/仲裁、安装权威与安装器、
//! 壳侧事件通道(命名管道线程 → 进程内 mpsc → GPUI foreground
//! 排水 → `AgentActivity` 的 pane 宿主)。
//!
//! 缝合点(不在本模块消费):设置页数据源消费 `integrations::inspect()` 归
//! 设置域;`config_guard` 随设置域启动链挂接;通知漏斗消费
//! TurnDone/NeedsAttention 两个状态边归 `notify`。

mod bridges;
/// Hooks 草稿 JSON 子树解析（重复键/坏 JSON/非法 UTF-8 拒绝）。
pub(crate) mod draft;
/// 按面板实例＋CLI＋层级隔离的 Hooks 草稿宿主。
pub(crate) mod draft_host;
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

// re-export 面是壳侧、工作区与通知域的缝合点(事件排水、pane 宿主、通知漏斗)。
pub(crate) use event::CodexHookMode;
pub use event::{
    AiBackgroundTasks, AiHookEvent, AiHookKind, AiPermissionMode, AiTurnOutcome, AttentionContext,
};
#[allow(unused_imports)] // 能力表消费点归通知漏斗 / runtime 快照。
pub use event::{AiHookCapabilities, capabilities_for};
pub(crate) use lifecycle::AgentActivity;
/// 设置域启动链持有该守卫；失败边只经封闭类别回到壳前台呈现。
pub(crate) use local::config_guard::{HookFailureKind, spawn_config_guard_with};
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

/// 我们订阅的 claude hook 事件与编辑器完整目录；白名单单源在 `slterm_hooks`。
pub(crate) use slterm_hooks::{CLAUDE_EVENT_CATALOG, CLAUDE_EVENTS};

/// 卸载器入口(`setup-ai --remove`):移除全部 AI CLI 的 hook 注入。
pub(crate) use local::claude_config_dir;
pub(crate) use local::settings::remove_all as remove_all_hooks;

/// 命名管道服务入口；生产由启动链持有，测试使用同一实现。
pub use windows::spawn_gpui_server;

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

/// 注入以用户级设置文件提交为边界的三种结果。
///
/// 提交后清理失败必须保留「已提交」事实并提供只清理的重试；提交前失败
/// 已回滚准备材料，只能重新安装，不得伪装为部分成功。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HookCommitState {
    Committed,
    CommittedCleanupFailed,
    RolledBack,
}

/// 结果对应的显式重试动作；`Committed` 没有可重试的副作用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HookRetryAction {
    /// 设置文件已提交，重试只清理材料，不重写配置。
    CleanupOnly,
    /// 提交前失败，重新走完整安装。
    Reinstall,
}

pub(crate) fn hook_retry_action(state: HookCommitState) -> Option<HookRetryAction> {
    match state {
        HookCommitState::Committed => None,
        HookCommitState::CommittedCleanupFailed => Some(HookRetryAction::CleanupOnly),
        HookCommitState::RolledBack => Some(HookRetryAction::Reinstall),
    }
}

#[cfg(test)]
mod hook_commit_tests {
    use super::{HookCommitState, HookRetryAction, hook_retry_action};

    /// 提交后清理失败显示为已提交并只重试清理；回滚后只能重装。
    #[test]
    fn hooks_post_commit_cleanup_failure_is_visible() {
        assert_eq!(hook_retry_action(HookCommitState::Committed), None);
        assert_eq!(
            hook_retry_action(HookCommitState::CommittedCleanupFailed),
            Some(HookRetryAction::CleanupOnly),
            "清理失败必须保留已提交事实并只重试清理"
        );
        assert_eq!(
            hook_retry_action(HookCommitState::RolledBack),
            Some(HookRetryAction::Reinstall),
            "提交前失败只能重新安装"
        );
    }
}

/// Hooks 草稿写前重读结论。
///
/// 只有基线（草稿进入编辑时观察到的文件指纹）与当前指纹一致才允许写入；
/// 外部修改、删除或外部新建都保留草稿并要求重新加载，绝不自动合并。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DraftWriteGuard {
    Write,
    ExternalModified,
}

/// 指纹为文件内容 SHA-256；`None` 表示文件不存在。
pub(crate) fn draft_write_guard(
    baseline: Option<[u8; 32]>,
    current: Option<[u8; 32]>,
) -> DraftWriteGuard {
    if baseline == current {
        DraftWriteGuard::Write
    } else {
        DraftWriteGuard::ExternalModified
    }
}

#[cfg(test)]
mod draft_guard_tests {
    use super::{DraftWriteGuard, draft_write_guard};

    /// 写前重读：基线缺失、外部新建、修改与删除都拒绝写入并保留草稿。
    #[test]
    fn hooks_external_modification_guard() {
        let a = [0x11_u8; 32];
        let b = [0x22_u8; 32];
        assert_eq!(
            draft_write_guard(None, None),
            DraftWriteGuard::Write,
            "文件不存在时按空配置创建"
        );
        assert_eq!(draft_write_guard(Some(a), Some(a)), DraftWriteGuard::Write);
        assert_eq!(
            draft_write_guard(Some(a), Some(b)),
            DraftWriteGuard::ExternalModified,
            "外部修改必须保留草稿"
        );
        assert_eq!(
            draft_write_guard(Some(a), None),
            DraftWriteGuard::ExternalModified,
            "外部删除必须保留草稿"
        );
        assert_eq!(
            draft_write_guard(None, Some(a)),
            DraftWriteGuard::ExternalModified,
            "外部新建不得被覆盖"
        );
    }
}
