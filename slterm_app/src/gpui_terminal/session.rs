//! PTY 会话接线:`Term` + `EventLoop` + ConPTY,全部来自 `slterm_terminal`。
//!
//! 事件出口是 futures channel,GPUI 前台任务可 `await` 事件。SSH 变体消亡
//! (`TerminalLaunch::Ssh` 不再存在,上游 `spawn_ssh`/stages/
//! remote_reader 一并裁剪——按理想终态重设计时另立)。

use std::sync::Arc;

use slterm_terminal::Term;
use slterm_terminal::event::{Event, EventListener, WindowSize};
use slterm_terminal::event_loop::{EventLoop, EventLoopHandle, Notifier};
use slterm_terminal::grid::Dimensions;
use slterm_terminal::sync::FairMutex;
use slterm_terminal::term::Config;
use slterm_terminal::tty;
use slterm_terminal::tty::EventedPty as _;

/// 把 PTY 线程发出的终端事件转投到 GPUI 前台的异步通道。
#[derive(Clone)]
pub struct EventProxy {
    pub(super) events: super::event_mailbox::EventSender,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        self.events.send(event);
    }
}

/// 初始网格尺寸;`Term::new`/`Term::resize` 只消费行列数。
#[derive(Clone, Copy)]
pub struct GridSize {
    pub columns: usize,
    pub screen_lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

pub struct TerminalSession {
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    pub notifier: Notifier,
    pub(super) native_prompt: super::event_mailbox::NativePromptState,
    /// PTY 直系 shell PID;关闭确认的 `busy_child(shell_pid)` 判据归 PTY 域。
    #[allow(dead_code)] // 仅观测;关闭确认消费点归工作区。
    pub shell_pid: u32,
    /// EventLoop 句柄(裁决:`spawn` 返回 must_use 句柄,Drop 兜底发
    /// Shutdown + join)。会话必须持证:句柄随会话 Drop,后台线程才跟 pane
    /// 生命周期走。`None` 仅存在于 test_session(无真 PTY)。
    _event_loop: Option<EventLoopHandle<tty::Pty, EventProxy>>,
}

/// 一次会话 spawn 的完整出口:会话句柄 + 终端事件流。
pub type SpawnedSession = (TerminalSession, super::event_mailbox::EventReceiver);

#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) fn test_session() -> (
    TerminalSession,
    std::sync::mpsc::Receiver<slterm_terminal::event_loop::Msg>,
) {
    let (events, _event_rx) = super::event_mailbox::channel();
    let proxy = EventProxy { events };
    let term = Term::new(
        Config::default(),
        &GridSize {
            columns: 80,
            screen_lines: 24,
        },
        proxy.clone(),
    );
    let (sender, receiver) = slterm_terminal::event_loop::EventLoopSender::standalone().unwrap();
    (
        TerminalSession {
            term: Arc::new(FairMutex::new(term)),
            notifier: Notifier(sender),
            native_prompt: proxy.events.native_prompt.clone(),
            shell_pid: 0,
            _event_loop: None,
        },
        receiver,
    )
}

/// 组装本地 shell 的 PTY 启动参数。
///
/// `shell`:`None` 走 core 探测链(pwsh → powershell → cmd,白名单深检 +
/// pwsh EncodedCommand 集成)。环境身份契约(`SLTERM_PANE_ID`)最后写,
/// 以 `refresh_environment` 的快照为基准。
///
/// `explicit_command`:`-e` 显式命令通道——裸名经 core
/// `resolve_explicit_command` PATH 路径化(含 PATHEXT 补全),并在
/// `Options` 置位跳过三家白名单(豁免论证见该字段);settings 键
/// 来源(设置键)必须传 `false`。解析失败直接 Err(「PATH 中找不到
/// 命令」),由调用方并入 spawn 错误通道呈现。
pub(super) fn local_options(
    shell: Option<tty::Shell>,
    explicit_command: bool,
    pane_id: slterm_split::PaneId,
    cwd: Option<std::path::PathBuf>,
) -> std::io::Result<tty::Options> {
    let shell = match (shell, explicit_command) {
        (Some(shell), true) => {
            let program = tty::resolve_explicit_command(shell.program())?;
            Some(tty::Shell::new(program, shell.args().to_vec()))
        }
        (shell, _) => shell,
    };
    let mut options = tty::Options {
        shell,
        working_directory: cwd,
        ..Default::default()
    };
    options.explicit_command = explicit_command;
    // 注册表快照合并(MACHINE+USER 环境),置 env_is_complete;失败仅降级
    // 为进程环境继承,不拦启动(与上游 prepare_local_pty 同语义)。
    if let Err(error) = tty::refresh_environment(&mut options) {
        log::warn!("环境快照合并失败,回退进程环境继承: {error}");
    }
    // conpty_sideload 硬编码 true(换设置键真读时此处是唯一换点;
    // core `Options::default()` 同值,显式写出是登记锚)。
    options.conpty_sideload = true;
    // 固定身份/能力注入:core `setup_env` 只写 TERM/COLORTERM;身份组、
    // CLI 发现与端点注入统一走 `agent_env::apply` 末位(runtime_api
    // CHILD_ENDPOINT 在 apply 末尾注入,早于首个 PTY spawn 即已登记)。
    options.env.insert("TERM".into(), "xterm-256color".into());
    options.env.insert("COLORTERM".into(), "truecolor".into());
    crate::agent_env::apply(&mut options.env, pane_id.0);
    Ok(options)
}

pub fn spawn(
    window_size: WindowSize,
    term_config: Config,
    options: tty::Options,
) -> std::io::Result<SpawnedSession> {
    let (tx, rx) = super::event_mailbox::channel();
    let proxy = EventProxy { events: tx };

    let grid = GridSize {
        columns: window_size.num_cols as usize,
        screen_lines: window_size.num_lines as usize,
    };
    let term = Arc::new(FairMutex::new(Term::new(term_config, &grid, proxy.clone())));

    tty::setup_env();
    let pty = tty::new(&options, window_size)?;
    let shell_pid = pty.child_pid().unwrap_or(0);

    let native_prompt = proxy.events.native_prompt.clone();
    let event_loop = EventLoop::new(Arc::clone(&term), proxy, pty, options.drain_on_exit)?;
    let notifier = Notifier(event_loop.channel());
    // 句柄随会话持有:Drop 兜底(Shutdown+join)由 EventLoopHandle 保证。
    let event_loop = event_loop.spawn();

    Ok((
        TerminalSession {
            term,
            notifier,
            native_prompt,
            shell_pid,
            _event_loop: Some(event_loop),
        },
        rx,
    ))
}

#[cfg(test)]
mod local_options_tests {
    use super::*;

    fn pane() -> slterm_split::PaneId {
        slterm_split::PaneId(1)
    }

    /// 显式通道:裸名经 core `resolve_explicit_command` 路径化为完整
    /// 路径,`Options::explicit_command` 置位(tty::new 跳过白名单的
    /// 凭据)。读真实 PATH(`cmd` 恒可解析,本 crate 测试进程无 PATH
    /// 写方,无需锁)。
    #[test]
    fn explicit_command_resolves_bare_name_and_marks_options() {
        let options = local_options(
            Some(tty::Shell::new("cmd".into(), vec!["/q".into()])),
            true,
            pane(),
            None,
        )
        .expect("cmd 裸名应可解析");
        let shell = options.shell.expect("shell 应保留");
        // 扩展名大小写跟随系统 PATHEXT 书写(拼接形态),按不敏感断言。
        assert!(
            shell.program().contains('\\') && shell.program().to_lowercase().ends_with("cmd.exe"),
            "应路径化为完整路径,实际: {}",
            shell.program()
        );
        assert_eq!(shell.args(), &["/q".to_string()], "参数应原样保留");
        assert!(options.explicit_command, "显式通道标记应置位");
    }

    /// 显式通道解析失败:Err 经 spawn 同通道呈现(「PATH 中找不到命令」)。
    #[test]
    fn explicit_command_not_found_errs() {
        let err = local_options(
            Some(tty::Shell::new("slterm-no-such-command-xyz".into(), vec![])),
            true,
            pane(),
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("PATH 中找不到命令"), "实际: {err}");
    }

    /// 非显式通道(settings/探测链来源):program 原样保留、不路径化,
    /// 标记不置位——白名单深检留给 core `tty::new`。
    #[test]
    fn non_explicit_keeps_program_verbatim() {
        let options = local_options(
            Some(tty::Shell::new("cmd".into(), vec![])),
            false,
            pane(),
            None,
        )
        .expect("非显式通道不解析不应失败");
        let shell = options.shell.expect("shell 应保留");
        assert_eq!(shell.program(), "cmd", "非显式通道应原样保留裸名");
        assert!(!options.explicit_command, "标记不得置位");
    }
}
