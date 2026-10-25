//! 进程树:hook 事件路由的第二因子与 agent 进程身份解析。
//!
//! 本模块只承载祖先链解析(`nearest_agent_ancestor`,命名管道身份核验的
//! 另一半)及其支撑:带 TTL 的全机快照、PID 复用消解与 `display_name`。

use crate::platform::process_snapshot::{self, ProcessRow};

/// 上溯上限。真实的 pane 进程链是 shell → 解释器 → agent → 子进程,个位数
/// 深度;给足余量同时保证快照里的父指针成环时一定会停。
const MAX_ANCESTRY_DEPTH: u32 = 64;

/// 快照复用窗口。取值只需覆盖「同一回合内成批到达的 hook」,不需要更长:
/// 表越旧,查不到新 helper 的概率越高,祖先链解析就越容易退化成无证据放行。
const SNAPSHOT_TTL: std::time::Duration = std::time::Duration::from_millis(250);

/// 把裸 exe 名归一成可展示/可比对的形态:剥掉 `.exe` 后缀
/// (`node.exe` → `node`)。
///
/// 2026-07-27 上游用户反馈:关闭窗口时确认框写的是 `node.exe 仍在运行`——那
/// 其实是 Claude Code,只不过快照只看得见宿主解释器。本函数只负责把名字擦干
/// 净;身份推断归 `AgentKind::parse`。
pub(crate) fn display_name(exe: &str) -> String {
    let name = exe.rsplit(['/', '\\']).next().unwrap_or(exe);
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
        .to_owned()
}

fn snapshot() -> Result<std::collections::HashMap<u32, (u32, String)>, String> {
    Ok(verified_parentage(process_snapshot::snapshot()?))
}

/// Windows 会把已退出创建者的 PID 留给孩子。若这个号如今属于一个更年轻的
/// 进程,它就不可能是这孩子的父亲。创建时间缺失的行保留原边:不确定性不能
/// 掩盖一条真实的父子关系。
fn verified_parentage(rows: Vec<ProcessRow>) -> std::collections::HashMap<u32, (u32, String)> {
    let creation_times: std::collections::HashMap<_, _> =
        rows.iter().map(|row| (row.pid, row.created)).collect();
    rows.into_iter()
        .map(|row| {
            let reused = row.created != 0
                && creation_times
                    .get(&row.parent)
                    .is_some_and(|parent| *parent > row.created);
            let parent = if reused { 0 } else { row.parent };
            (row.pid, (parent, row.executable))
        })
        .collect()
}

/// 带 TTL 的全机进程快照,祖先链查询共用。
///
/// 用闭包而不是把表交出去:调用方只需要读,克隆整张进程表纯属浪费。
fn with_cached_snapshot<T>(
    read: impl FnOnce(&std::collections::HashMap<u32, (u32, String)>) -> T,
) -> Option<T> {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    use std::time::Instant;

    type Cached = Option<(Instant, HashMap<u32, (u32, String)>)>;
    static CACHE: LazyLock<Mutex<Cached>> = LazyLock::new(|| Mutex::new(None));

    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fresh = cache
        .as_ref()
        .is_some_and(|(taken, _)| taken.elapsed() < SNAPSHOT_TTL);
    if !fresh {
        *cache = Some((Instant::now(), snapshot().ok()?));
    }
    let (_, processes) = cache.as_ref()?;
    Some(read(processes))
}

/// 沿 `pid` 的祖先链找最近的 AI CLI 进程,返回它的 pid 与 slug。
///
/// hook helper 不一定是 agent 直接 spawn 的:Windows 上 claude 通过 shell 执行
/// hook 命令,helper 的父进程是 `cmd.exe`,agent 在更上一层。所以不能只看父
/// 进程,必须往上找到第一个认得的 CLI。
///
/// 这个 pid 是 agent 的**进程身份**,比 session id 稳:嵌套 `claude -p` 子代理
/// 有自己的 pid,但它的 session id 是短命的——一旦被当成 pane 的会话身份,就会
/// 把真正活着的那个顶掉(`claude --resume` 之后指向一个已经结束的会话)。
pub(crate) fn nearest_agent_ancestor(pid: u32) -> Option<(u32, String)> {
    with_cached_snapshot(|processes| resolve_agent_ancestor(processes, pid)).flatten()
}

/// [`nearest_agent_ancestor`] 的纯函数内核。
fn resolve_agent_ancestor(
    processes: &std::collections::HashMap<u32, (u32, String)>,
    pid: u32,
) -> Option<(u32, String)> {
    let mut current = pid;
    for _ in 0..MAX_ANCESTRY_DEPTH {
        let (parent, executable) = processes.get(&current)?;
        if let Some(kind) = crate::ai_agents::AgentKind::parse(&display_name(executable)) {
            return Some((current, kind.slug().to_owned()));
        }
        if *parent == 0 || *parent == current {
            return None;
        }
        current = *parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{ProcessRow, display_name, resolve_agent_ancestor, verified_parentage};

    /// 带自定义可执行名的进程表,用来构造「helper 上面隔着一层 shell」的链。
    fn named_table(rows: &[(u32, u32, &str)]) -> HashMap<u32, (u32, String)> {
        rows.iter()
            .map(|&(pid, parent, exe)| (pid, (parent, exe.to_owned())))
            .collect()
    }

    /// hook helper 的父进程往往不是 agent 本身:Windows 上 claude 通过
    /// `cmd.exe` 执行 hook 命令。只看父进程会把 agent 身份漏掉,必须往上找。
    #[test]
    fn agent_identity_comes_from_the_nearest_ancestor_not_the_parent() {
        // pwsh(100) → claude(200) → cmd(300) → slterm-hook(400)
        let processes = named_table(&[
            (100, 4, "pwsh.exe"),
            (200, 100, "claude.exe"),
            (300, 200, "cmd.exe"),
            (400, 300, "slterm-hook.exe"),
        ]);
        let (pid, slug) = resolve_agent_ancestor(&processes, 400).expect("claude 应被找到");
        assert_eq!(pid, 200);
        assert_eq!(slug, "claude");

        // 嵌套子代理:主 claude(200) 之下又起了一个 claude(500)。从子代理这条链
        // 上溯,拿到的是子代理自己的 pid——两条流因此能被区分开,短命的子会话
        // 不会顶掉活着的那个。
        let nested = named_table(&[
            (100, 4, "pwsh.exe"),
            (200, 100, "claude.exe"),
            (500, 200, "claude.exe"),
            (600, 500, "slterm-hook.exe"),
        ]);
        assert_eq!(
            resolve_agent_ancestor(&nested, 600).map(|(pid, _)| pid),
            Some(500)
        );
        assert_eq!(
            resolve_agent_ancestor(&nested, 200).map(|(pid, _)| pid),
            Some(200)
        );

        // 链上没有任何认得的 CLI:不许瞎猜。
        let plain = named_table(&[(100, 4, "pwsh.exe"), (200, 100, "slterm-hook.exe")]);
        assert_eq!(resolve_agent_ancestor(&plain, 200), None);
    }

    #[test]
    fn display_name_drops_exe_suffix() {
        assert_eq!(display_name("node.exe"), "node");
        assert_eq!(display_name("CARGO.EXE"), "CARGO");
        assert_eq!(display_name("/bin/bash"), "bash");
        // 非后缀的点号名保持原样。
        assert_eq!(display_name("cargo"), "cargo");
        assert_eq!(display_name("python3.11"), "python3.11");
    }

    /// PID 复用消解:父号已被更年轻的进程占用时,父子边必须断开——否则一个
    /// 无关进程会被错挂到别人的壳下,身份链解析随之说谎。
    #[test]
    fn reused_parent_pid_detaches_the_stale_edge() {
        let row = |pid: u32, parent: u32, created: u64| ProcessRow {
            pid,
            parent,
            executable: format!("p{pid}.exe"),
            created,
        };
        let processes = verified_parentage(vec![
            row(100, 4, 300),
            row(200, 100, 100),
            row(300, 200, 200),
        ]);
        // 100 的创建时间晚于 200:它不可能是 200 的父亲,边断到 0。
        assert_eq!(processes.get(&200).map(|(parent, _)| *parent), Some(0));
        // 创建时间缺失(0)保留原边:不确定性不掩盖真实关系。
        assert_eq!(processes.get(&300).map(|(parent, _)| *parent), Some(200));
    }
}
