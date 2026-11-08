//! PTY 会话接线:`Term` + `EventLoop` + ConPTY,全部来自 `slterm_terminal`。
//!
//! 事件出口是 futures channel,GPUI 前台任务可 `await` 事件。SSH 变体消亡
//! (`TerminalLaunch::Ssh` 不再存在,上游 `spawn_ssh`/stages/
//! remote_reader 一并裁剪——按理想终态重设计时另立)。

use std::sync::Arc;

use slterm_terminal::Term;
use slterm_terminal::event::{Event, EventListener, WindowSize};
use slterm_terminal::event_loop::{EventLoop, EventLoopHandle, Msg, Notifier};
use slterm_terminal::grid::Dimensions;
use slterm_terminal::sync::FairMutex;
use slterm_terminal::term::Config;
use slterm_terminal::tty;
use slterm_terminal::tty::EventedPty as _;

/// 这两个环境键的测试写方共用一把锁；守卫持锁到 Drop，panic 路径也恢复原值。
#[cfg(test)]
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) struct ParentEnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous: [(&'static str, Option<std::ffi::OsString>); 2],
    expected: [(&'static str, Option<std::ffi::OsString>); 2],
}

#[cfg(test)]
impl ParentEnvGuard {
    /// 只观测当前值；旧实现若写入，Drop 仍会恢复。
    pub(crate) fn capture() -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = [
            ("TERM", std::env::var_os("TERM")),
            ("COLORTERM", std::env::var_os("COLORTERM")),
        ];
        let expected = previous.clone();
        Self {
            _lock: lock,
            previous,
            expected,
        }
    }

    /// 设置确定的前置值，覆盖 Some/None 两种不污染判据。
    pub(crate) fn set(term: Option<&str>, colorterm: Option<&str>) -> Self {
        let mut guard = Self::capture();
        unsafe {
            match term {
                Some(value) => std::env::set_var("TERM", value),
                None => std::env::remove_var("TERM"),
            }
            match colorterm {
                Some(value) => std::env::set_var("COLORTERM", value),
                None => std::env::remove_var("COLORTERM"),
            }
        }
        guard.expected = [
            ("TERM", term.map(std::ffi::OsString::from)),
            ("COLORTERM", colorterm.map(std::ffi::OsString::from)),
        ];
        guard
    }

    pub(crate) fn assert_unchanged(&self) {
        for (name, expected) in &self.expected {
            assert_eq!(
                std::env::var_os(name).as_deref(),
                expected.as_deref(),
                "spawn 不得改写父进程 {name}"
            );
        }
    }
}

#[cfg(test)]
impl Drop for ParentEnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.previous {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

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

impl TerminalSession {
    /// 消费会话并显式回收 PTY 线程；join 的 panic 原样交给调用方。
    ///
    /// 此方法会阻塞，只能由后台关闭协调器调用，禁止在 GPUI 前台执行。
    #[allow(dead_code)] // 后台 CloseCoordinator 在后续切片接入消费。
    pub fn shutdown(self) -> std::thread::Result<()> {
        match self._event_loop {
            Some(event_loop) => event_loop.join().map(|_| ()),
            None => {
                // 无 PTY 的测试会话没有 EventLoopHandle 代发关闭消息。
                let _ = self.notifier.0.send(Msg::Shutdown);
                Ok(())
            }
        }
    }
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
    pane_id: crate::gpui_terminal::PaneId,
    cwd: Option<std::path::PathBuf>,
) -> std::io::Result<tty::Options> {
    local_options_with_refresh(
        shell,
        explicit_command,
        pane_id,
        cwd,
        tty::refresh_environment,
    )
}

/// 环境刷新可注入，便于覆盖刷新失败的原生继承回退分支。
fn local_options_with_refresh(
    shell: Option<tty::Shell>,
    explicit_command: bool,
    pane_id: crate::gpui_terminal::PaneId,
    cwd: Option<std::path::PathBuf>,
    refresh: impl FnOnce(&mut tty::Options) -> std::io::Result<()>,
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
    // 注册表快照合并(MACHINE+USER 环境),成功时置 env_is_complete；失败则
    // 保留增量环境，由 ConPTY 合并原生父环境并按排除名过滤旧端点。
    if let Err(error) = refresh(&mut options) {
        log::warn!("环境快照合并失败,回退原生进程环境继承: {error}");
    }
    // conpty_sideload 硬编码 true(换设置键真读时此处是唯一换点;
    // core `Options::default()` 同值,显式写出是登记锚)。
    options.conpty_sideload = true;
    // 固定身份/能力注入:TERM/COLORTERM 写入 options.env，作为子进程
    // 环境块的自定义覆盖；身份组与 CLI 发现统一走 `agent_env::apply`。
    options.env.insert("TERM", "xterm-256color");
    options.env.insert("COLORTERM", "truecolor");
    crate::agent_env::apply(&mut options, pane_id.0);
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

    // 子进程环境由 options.env 与 core custom 环境块组装，不写父进程环境。
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

    fn pane() -> crate::gpui_terminal::PaneId {
        crate::gpui_terminal::PaneId(1)
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

    #[test]
    fn refresh_failure_keeps_native_inheritance_and_excludes_legacy_endpoint() {
        let options = local_options_with_refresh(None, false, pane(), None, |options| {
            options
                .env
                .insert("SLTERM_REFRESH_FALLBACK_TEST", "preserved");
            Err(std::io::Error::other("injected refresh failure"))
        })
        .expect("环境刷新失败必须回退到原生进程环境继承");

        assert!(
            !options.env_is_complete,
            "回退必须继续由 ConPTY 合并原生父环境"
        );
        assert!(
            options.inherited_env_exclusions.iter().any(|name| {
                name.eq_ignore_ascii_case(std::ffi::OsStr::new("SLTERM_RUNTIME_ENDPOINT"))
            }),
            "旧 Runtime 端点必须在继承合并处排除"
        );
        assert_eq!(
            options
                .env
                .get("SLTERM_REFRESH_FALLBACK_TEST")
                .and_then(std::ffi::OsStr::to_str),
            Some("preserved"),
            "刷新失败不得丢弃已组装的自定义环境"
        );
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    #[cfg(feature = "gpui-test-support")]
    use std::sync::Arc;

    fn real_window_size() -> WindowSize {
        WindowSize {
            num_lines: 24,
            num_cols: 80,
            cell_width: 8,
            cell_height: 16,
        }
    }

    fn real_options() -> tty::Options {
        tty::Options {
            shell: Some(tty::Shell::new(
                "cmd.exe".into(),
                vec!["/d".into(), "/q".into(), "/k".into()],
            )),
            // 测试只走系统 ConPTY，禁止 Win10 侧载提取写用户数据目录。
            conpty_sideload: false,
            ..tty::Options::default()
        }
    }

    /// 正常路径必须经真实 ConPTY/EventLoop 的 join 回收，不用返回值替身。
    #[test]
    fn shutdown_joins_real_pty_event_loop() {
        let parent_env = ParentEnvGuard::set(Some("slterm-test-parent-term"), None);
        let (session, _event_rx) =
            spawn(real_window_size(), Config::default(), real_options()).expect("真实 PTY 应启动");

        session.shutdown().expect("真实 EventLoop join 应成功");
        parent_env.assert_unchanged();
    }

    /// 无 PTY 的测试会话沿用 Shutdown 消息合同，且显式关闭成功。
    #[cfg(feature = "gpui-test-support")]
    #[test]
    fn shutdown_without_pty_notifies_and_returns_ok() {
        let (session, receiver) = test_session();

        session.shutdown().expect("无 PTY 会话应关闭成功");

        assert!(matches!(receiver.try_recv(), Ok(Msg::Shutdown)));
    }

    /// 故障来自真实 EventLoopHandle 持有的工作线程 JoinHandle，验证
    /// TerminalSession 不把 join 的 panic Err 吞掉或二次 unwrap。
    #[cfg(feature = "gpui-test-support")]
    #[test]
    fn shutdown_propagates_real_event_loop_worker_panic() {
        let (events, _event_rx) = super::super::event_mailbox::channel();
        let proxy = EventProxy { events };
        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &real_window_size(),
            proxy.clone(),
        )));
        let pty = tty::new(&real_options(), real_window_size()).expect("真实 PTY 应启动");
        let shell_pid = pty.child_pid().unwrap_or(0);
        let native_prompt = proxy.events.native_prompt.clone();
        let event_loop =
            EventLoop::new(Arc::clone(&term), proxy, pty, false).expect("真实 EventLoop 应创建");
        let notifier = Notifier(event_loop.channel());
        let event_loop = event_loop.spawn_panicking_for_test();
        let session = TerminalSession {
            term,
            notifier,
            native_prompt,
            shell_pid,
            _event_loop: Some(event_loop),
        };

        let panic = session
            .shutdown()
            .expect_err("工作线程 panic 必须经 shutdown 返回 Err")
            .downcast::<&str>()
            .expect("测试 panic 载荷应保持原类型");
        assert_eq!(*panic, "测试注入的 PTY reader panic");
    }
}
