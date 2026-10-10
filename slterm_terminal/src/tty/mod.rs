//! TTY related functionality.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::Arc;
use std::{env, io};

use polling::{Event, PollMode, Poller};

pub mod windows;
pub use self::windows::*;

/// Configuration for the `Pty` interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Shell options.
    ///
    /// [`None`] will use the default shell.
    pub shell: Option<Shell>,

    /// Shell startup directory.
    pub working_directory: Option<PathBuf>,

    /// Drain the child process output before exiting the terminal.
    pub drain_on_exit: bool,

    /// Extra environment variables.
    pub env: HashMap<String, String>,

    /// 此环境表是完整子进程环境，不是需要与当前进程环境合并的增量覆盖。
    pub env_is_complete: bool,

    /// Specifies whether the Windows shell arguments should be escaped.
    ///
    /// - When `true`: Arguments will be escaped according to the standard C runtime rules.
    /// - When `false`: Arguments will be passed raw without additional escaping.
    pub escape_args: bool,

    /// ConPTY 侧载开关：由壳读 settings 键注入（键域归 06 篇），core 不内嵌
    /// 路径推导。置位时优先 exe 旁完整文件对 /（Win10）NuGet 提取的
    /// conpty.dll + OpenConsole.exe，缺失回退系统 API。默认 true——侧载
    /// host 避免 in-box resize 视口重发怪癖与老 Win10 鼠标转发缺失；单文件
    /// exe 发布形态下 Win10 提取链必须默认可达。
    pub conpty_sideload: bool,

    /// ConPTY 输入模式能力矩阵（DTO 字段语义归 02 篇锚定）：壳读 settings
    /// 键注入；默认矩阵 = 现状三态零漂移（0x1/0x2/0x4 开、0x8 关）。
    pub conpty_input_modes: windows::ConptyInputModes,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            shell: None,
            working_directory: None,
            drain_on_exit: false,
            env: HashMap::new(),
            env_is_complete: false,
            escape_args: false,
            conpty_sideload: true,
            conpty_input_modes: windows::ConptyInputModes::default(),
        }
    }
}

/// Shell options.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Shell {
    /// Path to a shell program to run on startup.
    pub(crate) program: String,
    /// Arguments passed to shell.
    pub(crate) args: Vec<String>,
}

impl Shell {
    pub fn new(program: String, args: Vec<String>) -> Self {
        Self { program, args }
    }

    /// Path to the shell program (session persistence reads it back).
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Arguments passed to the shell.
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Stream read and/or write behavior.
///
/// This defines an abstraction over polling's interface in order to allow either
/// one read/write object or a separate read and write object.
pub trait EventedReadWrite {
    type Reader: io::Read;
    type Writer: io::Write;

    /// # Safety
    ///
    /// The underlying sources must outlive their registration in the `Poller`.
    unsafe fn register(&mut self, _: &Arc<Poller>, _: Event, _: PollMode) -> io::Result<()>;
    fn reregister(&mut self, _: &Arc<Poller>, _: Event, _: PollMode) -> io::Result<()>;
    fn deregister(&mut self, _: &Arc<Poller>) -> io::Result<()>;

    fn reader(&mut self) -> &mut Self::Reader;
    fn writer(&mut self) -> &mut Self::Writer;
}

/// Events concerning TTY child processes.
#[derive(Debug, PartialEq, Eq)]
pub enum ChildEvent {
    /// Indicates the child has exited.
    Exited(Option<ExitStatus>),
}

/// A pseudoterminal (or PTY).
///
/// This is a refinement of EventedReadWrite that also provides a channel through which we can be
/// notified if the PTY child process does something we care about (other than writing to the TTY).
pub trait EventedPty: EventedReadWrite {
    /// Tries to retrieve an event.
    ///
    /// Returns `Some(event)` on success, or `None` if there are no events to retrieve.
    fn next_child_event(&mut self) -> Option<ChildEvent>;

    /// PID of the direct child attached to this PTY, when the backend knows
    /// it. Windows uses it for the ConPTY cursor-realign probe
    /// (`AttachConsole` + `GetConsoleScreenBufferInfo`).
    fn child_pid(&self) -> Option<u32> {
        None
    }

    /// 窗口外 DSR 查询剥离门控（DA1/DSR 接管族，event_loop 读侧消费）。
    /// Windows ConPTY 后端按 OS build 预计算覆写：Win10 家族 = true（键
    /// 事件输入下 CPR 应答即 F3 毒键，剥离不答）；其他后端默认 false
    /// （DSR 透传，交 Term 以真实光标自答）。默认方法承载平台分叉，core
    /// 读侧不出现 cfg。
    fn strip_dsr_queries(&self) -> bool {
        false
    }
}

/// Setup environment variables.
///
/// Windows-only 定位下不做 terminfo 探测（spec 分片 02 不采纳点 11）：
/// 定死注入。`TERM_PROGRAM` 等固定注入叠加在 `convert_custom_env` 末尾
/// （tty/windows/conpty.rs）。
pub fn setup_env() {
    unsafe { env::set_var("TERM", "xterm-256color") };

    // Advertise 24-bit color support.
    unsafe { env::set_var("COLORTERM", "truecolor") };
}
