//! 命令行面:同 exe 双人格。
//!
//! - 带子命令(`ctl` / `env` / `window` / `tab` / `pane` / `agent`)= CLI 客户端,
//!   请求-响应后退出;输出一律 JSON(`--pretty` 只改排版)。
//! - 无子命令 = GUI 启动;`--working-directory` / `-e` 两个启动参数保留 M3 语义。
//!
//! 资源 + 动词是完整协议之上的薄别名:同一套 `request_once`、同一响应信封、
//! 同样的 generation / `after_seq` 竞态保护;短的只是命令行,不是语义。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[cfg(test)]
use crate::error::AppError;
#[cfg(test)]
use std::ffi::OsString;

/// 短命令查询超时(D04-2 首版照搬,重新标定归待沉淀)。
pub const SHORT_TIMEOUT_MS: u64 = 30_000;
/// wait 默认上限:编码 Agent 以分钟计,复用查询超时会把正常一轮打成 timeout。
pub const SHORT_WAIT_TIMEOUT_MS: u64 = 600_000;

/// 顶层入口:`slterm` 无子命令 = GUI,有子命令 = CLI 客户端。
#[derive(Parser, Debug)]
#[command(
    name = "slterm",
    bin_name = "slterm",
    version = env!("CARGO_PKG_VERSION"),
    about = "slTerminal:Windows-native GPUI terminal with a local runtime control plane",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// GUI 启动:首个终端的工作目录。
    #[arg(long, value_name = "DIRECTORY")]
    pub working_directory: Option<PathBuf>,

    /// GUI 启动:在首个终端执行的命令;其余参数原样透传(必须末位)。
    #[arg(
        short = 'e',
        long = "command",
        value_name = "COMMAND",
        allow_hyphen_values = true,
        num_args = 1..
    )]
    pub gui_command: Option<Vec<String>>,

    #[command(subcommand)]
    pub subcommands: Option<Subcommands>,
}

/// 同 exe 子命令面。其余领域的子命令(设置/安装器)归各自模块,本步不立。
#[derive(Subcommand, Debug)]
pub enum Subcommands {
    /// Full protocol surface for scripts and long-lived integrations.
    Ctl(ControlOptions),
    /// Report this pane's identity, the control-plane path, and every command
    /// available to it. Answers offline as well as online.
    Env(EnvOptions),
    /// Control terminal windows.
    Window(WindowResourceOptions),
    /// Control tabs within one terminal window.
    Tab(TabResourceOptions),
    /// Inspect and drive terminal panes.
    Pane(PaneOptions),
    /// Inspect and drive the AI agents running in panes.
    Agent(AgentOptions),
    /// Diagnose the native notification pipeline: AUMID registration,
    /// configured duration, and a synchronous test toast.
    NotifyTest,
    /// Installer hook: remove slTerminal hook entries from every supported
    /// AI CLI before the installation directory is deleted.
    SetupAi(SetupAiOptions),
}

#[derive(Args, Debug)]
pub struct SetupAiOptions {
    /// Remove hook integrations. This is the only supported operation.
    #[arg(long)]
    pub remove: bool,
}

/// `-e` 启动命令(启动链一次性消费;serde 持久化面归会话域)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// 无参形态:启动链只产 `WithArgs`;本变体为 `LaunchSession` 预埋。
    #[allow(dead_code)]
    Just(String),
    WithArgs {
        program: String,
        args: Vec<String>,
    },
}

impl Program {
    /// 消费点 = WorkspaceStartup::LaunchTerminal 的 spawn 参数组装。
    pub fn program(&self) -> &str {
        match self {
            Self::Just(program) => program,
            Self::WithArgs { program, .. } => program,
        }
    }

    /// 消费点同上(LaunchTerminal 组装)。
    pub fn args(&self) -> &[String] {
        match self {
            Self::Just(_) => &[],
            Self::WithArgs { args, .. } => args,
        }
    }
}

/// GUI 启动参数:CLI 子命令之外的唯一形态。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartupOptions {
    pub working_directory: Option<PathBuf>,
    pub command: Option<Program>,
}

impl Cli {
    /// GUI 启动视图:子命令归 CLI 人格,本函数不消费。
    pub fn startup_options(self) -> StartupOptions {
        let command = self.gui_command.and_then(|mut rest| {
            if rest.is_empty() {
                return None;
            }
            let program = rest.remove(0);
            Some(Program::WithArgs {
                program,
                args: rest,
            })
        });
        StartupOptions {
            working_directory: self.working_directory.map(repair_context_menu_dir),
            command,
        }
    }
}

/// 可注入的解析入口(测试用);生产走 `Cli::try_parse()` + `clap::Error::exit`
/// (--help/--version 的退出码语义由 clap 统一)。
#[cfg(test)]
pub fn parse_args<I, S>(args: I) -> Result<Cli, AppError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = std::iter::once(OsString::from("slterm"))
        .chain(args.into_iter().map(Into::into))
        .collect::<Vec<_>>();
    Cli::try_parse_from(args).map_err(|error| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            error.to_string(),
        ))
    })
}

/// 撤销资源管理器右键菜单引号化对盘符根目录的破坏(因果链照上游 cli.rs):
/// 安装器 verb 传 `--working-directory "%V"`;盘符根 `%V` 是 `D:\`,命令行以
/// `\"` 收尾,CommandLineToArgvW 把尾反斜杠读成转义引号——进程实际收到 `D:"`
/// 并被判为非法目录(上游 issue #36)。Windows 路径不可能含双引号,尾部孤立
/// 引号必然是被吞的分隔符,补回即可;其余路径原样返回。
fn repair_context_menu_dir(dir: PathBuf) -> PathBuf {
    if let Some(stripped) = dir.to_string_lossy().strip_suffix('"') {
        return PathBuf::from(format!("{stripped}\\"));
    }
    dir
}

/// 全资源命令共享的薄输出面。
#[derive(Args, Debug)]
pub struct ShortOutput {
    /// Pretty-print the JSON response. Output is JSON either way.
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Args, Debug)]
pub struct EnvOptions {
    #[command(flatten)]
    pub output: ShortOutput,

    /// Maximum time to wait for the runtime probe. The environment half of the
    /// answer is reported regardless.
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

// ─── ctl:完整协议 verb 面 ───────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ControlOptions {
    /// Pretty-print one-shot JSON responses. Streaming subscriptions stay JSON Lines.
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Maximum time to wait for a command response.
    #[arg(long, global = true, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,

    #[command(subcommand)]
    pub command: ControlCommand,
}

/// 客户端等待状态名;`settled` = 非 running(≠ 成功)。
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlWaitState {
    Idle,
    Running,
    WaitingInput,
    Attention,
    Finished,
    Failed,
    Settled,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSplitDirection {
    Right,
    Down,
}

#[derive(Subcommand, Debug)]
pub enum ControlCommand {
    /// Describe the protocol version, runtime version, and available capabilities.
    Describe,
    /// Read the authoritative window, tab, pane, and task-state projection.
    Snapshot,
    /// Execute one typed multi-step terminal workflow in a single Runtime request.
    Orchestrate {
        /// Inline UTF-8 JSON object containing steps and on_error.
        #[arg(long, conflicts_with = "file", required_unless_present = "file")]
        spec: Option<String>,
        /// Read the UTF-8 workflow JSON object from a file.
        #[arg(long, value_name = "FILE", conflicts_with = "spec")]
        file: Option<PathBuf>,
    },
    /// List only panes slTerminal recognizes as AI agents.
    Agents {
        #[arg(long)]
        window: Option<u64>,
    },
    /// Start a verified AI CLI in a new terminal tab and assign a stable name.
    AgentStart {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        resume_session_id: Option<String>,
    },
    /// Create an isolated Git worktree, then start a named AI CLI in that checkout.
    AgentFork {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        source_pane: Option<u64>,
        #[arg(long)]
        source_cwd: Option<PathBuf>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        resume_session_id: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        allow_dirty_source: bool,
    },
    /// Resolve one managed agent by stable id or active name.
    AgentGet {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        generation: Option<u64>,
    },
    /// Send one plain-text prompt to a managed agent.
    AgentPrompt {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        generation: Option<u64>,
        #[arg(long)]
        text: String,
        #[arg(long)]
        no_submit: bool,
    },
    /// Paste one bounded UTF-8 block into a managed agent.
    AgentPaste {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        generation: Option<u64>,
        #[arg(long)]
        text: String,
        #[arg(long)]
        no_submit: bool,
    },
    /// Read the terminal-buffer tail owned by a managed agent.
    AgentRead {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        generation: Option<u64>,
        #[arg(long, default_value_t = crate::runtime_api::DEFAULT_READ_LINES)]
        lines: usize,
    },
    /// Wait for the same managed-agent generation to reach a semantic state.
    AgentWait {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        generation: u64,
        #[arg(long, value_enum, default_value = "settled")]
        state: ControlWaitState,
        #[arg(long)]
        after_seq: Option<u64>,
    },
    /// Stream state snapshots whenever their semantic content changes.
    Subscribe {
        /// Resume after this revision; the current snapshot is sent when newer.
        #[arg(long)]
        since: Option<u64>,
    },
    /// Close an idle window. Busy panes return confirmation_required.
    CloseWindow {
        #[arg(long)]
        window: Option<u64>,
    },
    /// Focus a window or one of its panes.
    Focus {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: Option<u64>,
    },
    /// Create a default-shell tab in the target window.
    NewTab {
        #[arg(long)]
        window: Option<u64>,
    },
    /// Close an idle tab by its zero-based index.
    CloseTab {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long = "tab")]
        tab_index: usize,
        /// Discard unsaved editor changes instead of failing with dirty_document.
        #[arg(long)]
        force: bool,
    },
    /// Set a tab's custom name. An empty value restores the generated title.
    RenameTab {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long = "tab")]
        tab_index: usize,
        #[arg(long)]
        name: String,
    },
    /// Move a tab to another index in the same window.
    MoveTab {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long = "tab")]
        tab_index: usize,
        #[arg(long = "to")]
        to_index: usize,
    },
    /// Split the focused pane in the target window.
    Split {
        #[arg(long)]
        window: Option<u64>,
        /// Split this pane instead of the currently focused pane.
        #[arg(long)]
        pane: Option<u64>,
        #[arg(long, value_enum, default_value = "right")]
        direction: ControlSplitDirection,
    },
    /// Close an idle pane.
    ClosePane {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
    },
    /// Explicitly enable or disable focused-pane zoom for the pane's tab.
    ZoomPane {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long, action = clap::ArgAction::Set)]
        zoomed: bool,
    },
    /// Set this pane's share of its direct parent split.
    ResizePane {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long)]
        ratio: f32,
    },
    /// Send one plain-text prompt to a pane, optionally submitting it with Enter.
    Prompt {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long)]
        text: String,
        /// Write the text without appending Enter.
        #[arg(long)]
        no_submit: bool,
        /// After sending, wait until the pane reaches this task state.
        #[arg(long, value_enum)]
        wait: Option<ControlWaitState>,
    },
    /// Paste one bounded UTF-8 block into a pane using bracketed-paste mode.
    Paste {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long)]
        text: String,
        /// Paste the block without appending Enter.
        #[arg(long)]
        no_submit: bool,
        /// After submitting, wait until the pane reaches this task state.
        #[arg(long, value_enum)]
        wait: Option<ControlWaitState>,
    },
    /// Read the latest logical lines from a pane's real terminal buffer.
    Read {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long, default_value_t = crate::runtime_api::DEFAULT_READ_LINES)]
        lines: usize,
    },
    /// List the real local process tree rooted at a pane's PTY shell.
    Procs {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
    },
    /// Send a restricted named control key using the pane's active terminal mode.
    SendKey {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long)]
        key: String,
        #[arg(long)]
        shift: bool,
        #[arg(long)]
        alt: bool,
        #[arg(long)]
        control: bool,
        #[arg(long, default_value_t = 1)]
        repeat: u16,
    },
    /// Run one shell command and return its real OSC 133 exit status.
    Run {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long)]
        command: String,
        /// Submit the command and return its run id without waiting for completion.
        #[arg(long)]
        no_wait: bool,
    },
    /// Run an argv vector as an independent non-TTY child and capture both streams.
    ExecPane {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(
            long,
            default_value_t = crate::runtime_api::DEFAULT_EXEC_OUTPUT_BYTES
        )]
        max_output_bytes: usize,
        #[arg(last = true, required = true, num_args = 1.., value_name = "ARGV")]
        argv: Vec<String>,
    },
    /// Block until a pane reaches a semantic task state.
    Wait {
        #[arg(long)]
        window: Option<u64>,
        #[arg(long)]
        pane: u64,
        #[arg(long, value_enum, default_value = "settled")]
        state: ControlWaitState,
        #[arg(long)]
        after_seq: Option<u64>,
    },
}

// ─── 资源 + 动词薄别名 ───────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct WindowResourceOptions {
    #[command(subcommand)]
    pub command: WindowCommand,
}

#[derive(Subcommand, Debug)]
pub enum WindowCommand {
    /// Close an idle window. Busy panes require confirmation in the GUI.
    Close(WindowCloseOptions),
}

#[derive(Args, Debug)]
pub struct WindowCloseOptions {
    /// Window id from `slterm pane list`.
    pub window: u64,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct TabResourceOptions {
    #[command(subcommand)]
    pub command: TabCommand,
}

/// Tab 索引以窗口为作用域,`--window` 恒必填。
#[derive(Subcommand, Debug)]
pub enum TabCommand {
    /// Close an idle tab.
    Close(TabCloseOptions),
    /// Set a tab's custom name. Pass an empty name to restore the generated title.
    Rename(TabRenameOptions),
    /// Move a tab to another index in the same window.
    Move(TabMoveOptions),
}

#[derive(Args, Debug)]
pub struct TabCloseOptions {
    /// Zero-based tab index from the runtime snapshot.
    pub tab: usize,
    /// Window containing the tab.
    #[arg(long)]
    pub window: u64,
    /// Discard unsaved editor changes instead of failing with dirty_document.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct TabRenameOptions {
    /// Zero-based tab index from the runtime snapshot.
    pub tab: usize,
    /// New custom name. An empty string restores the generated title.
    pub name: String,
    /// Window containing the tab.
    #[arg(long)]
    pub window: u64,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct TabMoveOptions {
    /// Current zero-based tab index.
    pub tab: usize,
    /// Destination zero-based tab index in the same window.
    pub to: usize,
    /// Window containing the tab.
    #[arg(long)]
    pub window: u64,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneOptions {
    #[command(subcommand)]
    pub command: PaneCommand,
}

/// pane 以 `slterm pane list` 的数字 id 定位。
#[derive(Subcommand, Debug)]
pub enum PaneCommand {
    /// List every pane with id, task state, cwd, and Git branch.
    List(ListOptions),
    /// Read the tail of a pane's real terminal buffer.
    Read(PaneReadOptions),
    /// Write one line into a pane, submitting it with Enter by default.
    Send(PaneSendOptions),
    /// Paste bounded UTF-8 text as one bracketed block.
    Paste(PanePasteOptions),
    /// Block until a pane reaches a semantic task state.
    Wait(PaneWaitOptions),
    /// Execute an argv vector as an independent non-TTY child and capture both streams.
    Exec(PaneExecOptions),
    /// Close an idle pane.
    Close(PaneCloseOptions),
    /// Explicitly enable or disable focused-pane zoom for the pane's tab.
    Zoom(PaneZoomOptions),
    /// Set this pane's share of its direct parent split.
    Resize(PaneResizeOptions),
}

#[derive(Args, Debug)]
pub struct ListOptions {
    #[command(flatten)]
    pub output: ShortOutput,
    /// Restrict the listing to one window.
    #[arg(long)]
    pub window: Option<u64>,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneReadOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    /// Logical terminal rows to read from the buffer tail.
    #[arg(long, default_value_t = crate::runtime_api::DEFAULT_READ_LINES)]
    pub lines: usize,
    /// Disambiguate the pane when several windows are open.
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneSendOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    /// The line to write. Several words are joined with single spaces.
    #[arg(required = true, num_args = 1..)]
    pub text: Vec<String>,
    /// Write the text without pressing Enter.
    #[arg(long)]
    pub no_submit: bool,
    /// After submitting, block until the pane settles again. The baseline comes
    /// from the submission response, so an already-idle pane cannot satisfy the
    /// wait immediately. Conflicts with `--no-submit`.
    #[arg(long, conflicts_with = "no_submit")]
    pub wait: bool,
    /// Ceiling for `--wait`.
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub wait_timeout_ms: u64,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

/// paste 恰好一个本地来源;跨控制面的只有校验后的 UTF-8 载荷。
#[derive(Args, Debug)]
pub struct PasteSourceOptions {
    /// Literal text. Multiple words are joined with single spaces.
    #[arg(num_args = 0.., conflicts_with_all = ["stdin", "from_file"])]
    pub text: Vec<String>,
    /// Read the paste payload from standard input.
    #[arg(long, conflicts_with = "from_file")]
    pub stdin: bool,
    /// Read the paste payload from a UTF-8 file.
    #[arg(long, value_name = "FILE")]
    pub from_file: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct PanePasteOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    #[command(flatten)]
    pub source: PasteSourceOptions,
    /// Paste the block without pressing Enter.
    #[arg(long)]
    pub no_submit: bool,
    /// After submitting, block until the pane settles again.
    #[arg(long, conflicts_with = "no_submit")]
    pub wait: bool,
    /// Ceiling for `--wait`.
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub wait_timeout_ms: u64,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneWaitOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    /// The state to wait for. `settled` covers finished, failed, and
    /// waiting-input, which is what "the work is over" usually means.
    #[arg(long, value_enum, default_value = "settled")]
    pub state: ControlWaitState,
    /// Require the pane's `state_change_seq` to advance past this value. Pass
    /// the counter observed *before* dispatching work.
    #[arg(long)]
    pub after_seq: Option<u64>,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneExecOptions {
    /// Pane id from `slterm pane list`; its current cwd and local environment are reused.
    pub pane: u64,
    #[arg(long)]
    pub window: Option<u64>,
    /// Maximum bytes retained from each stream. Both streams are always fully drained.
    #[arg(
        long,
        default_value_t = crate::runtime_api::DEFAULT_EXEC_OUTPUT_BYTES
    )]
    pub max_output_bytes: usize,
    #[command(flatten)]
    pub output: ShortOutput,
    /// Child-process timeout.
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
    /// Program and arguments. `--` is required so child flags cannot be parsed by slterm.
    #[arg(last = true, required = true, num_args = 1.., value_name = "ARGV")]
    pub argv: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PaneCloseOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneZoomOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    /// Desired zoom state. Requiring the value keeps the command idempotent.
    #[arg(long, action = clap::ArgAction::Set)]
    pub zoomed: bool,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct PaneResizeOptions {
    /// Pane id from `slterm pane list`.
    pub pane: u64,
    /// Desired share of the pane's direct parent split, from 0.05 through 0.95.
    pub ratio: f32,
    #[arg(long)]
    pub window: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct AgentOptions {
    #[command(subcommand)]
    pub command: AgentCommand,
}

/// agent 以名字或稳定 id 定位——不是 pane,重开会话不能继承旧任务。
#[derive(Subcommand, Debug)]
pub enum AgentCommand {
    /// List panes running an AI CLI, with session identity and generation.
    List(ListOptions),
    /// Hand one task to an agent and submit it.
    Send(AgentSendOptions),
    /// Delegate one task and route the target Agent's final answer back here.
    Delegate(AgentDelegateOptions),
    /// Paste bounded UTF-8 text into an agent as one bracketed block.
    Paste(AgentPasteOptions),
    /// Read the tail of what an agent printed.
    Read(AgentReadOptions),
    /// Block until an agent's turn ends.
    Wait(AgentWaitOptions),
}

#[derive(Args, Debug)]
pub struct AgentSendOptions {
    /// Agent name or stable id from `slterm agent list`.
    pub agent: String,
    /// The task itself. Several words are joined with single spaces.
    #[arg(required = true, num_args = 1..)]
    pub text: Vec<String>,
    /// Write the task without submitting it.
    #[arg(long)]
    pub no_submit: bool,
    /// After submitting, block until the turn ends. Conflicts with `--no-submit`.
    #[arg(long, conflicts_with = "no_submit")]
    pub wait: bool,
    /// Ceiling for `--wait`.
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub wait_timeout_ms: u64,
    /// Refuse the send unless the agent is still this generation.
    #[arg(long)]
    pub generation: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct AgentDelegateOptions {
    /// Agent name or stable id from `slterm agent list`.
    pub agent: String,
    /// The delegated task. Several words are joined with single spaces.
    #[arg(required = true, num_args = 1..)]
    pub text: Vec<String>,
    /// Refuse the delegation unless the target is still this generation.
    #[arg(long)]
    pub generation: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct AgentPasteOptions {
    /// Agent name or stable id from `slterm agent list`.
    pub agent: String,
    #[command(flatten)]
    pub source: PasteSourceOptions,
    /// Paste the block without submitting it.
    #[arg(long)]
    pub no_submit: bool,
    /// After submitting, block until the turn ends.
    #[arg(long, conflicts_with = "no_submit")]
    pub wait: bool,
    /// Ceiling for `--wait`.
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub wait_timeout_ms: u64,
    /// Refuse the paste unless the agent is still this generation.
    #[arg(long)]
    pub generation: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct AgentReadOptions {
    /// Agent name or stable id from `slterm agent list`.
    pub agent: String,
    /// Logical terminal rows to read from the buffer tail.
    #[arg(long, default_value_t = crate::runtime_api::DEFAULT_READ_LINES)]
    pub lines: usize,
    #[arg(long)]
    pub generation: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct AgentWaitOptions {
    /// Agent name or stable id from `slterm agent list`.
    pub agent: String,
    /// The state to wait for.
    #[arg(long, value_enum, default_value = "settled")]
    pub state: ControlWaitState,
    /// Require the agent's `state_change_seq` to advance past this value.
    #[arg(long)]
    pub after_seq: Option<u64>,
    /// Pin the wait to one generation. Without it, the currently active
    /// generation is resolved first, so a replaced session fails loudly.
    #[arg(long)]
    pub generation: Option<u64>,
    #[command(flatten)]
    pub output: ShortOutput,
    #[arg(long, default_value_t = SHORT_WAIT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn parse<const N: usize>(args: [&str; N]) -> Result<Cli, AppError> {
        parse_args(args)
    }

    fn startup<const N: usize>(args: [&str; N]) -> StartupOptions {
        parse(args).expect("valid startup args").startup_options()
    }

    #[test]
    fn empty_args_yield_the_default_options() {
        let cli = parse([]).unwrap();
        assert!(cli.subcommands.is_none());
        assert_eq!(cli.startup_options(), StartupOptions::default());
    }

    #[test]
    fn working_directory_accepts_space_and_equals_forms() {
        for args in [
            vec!["--working-directory", "D:/work"],
            vec!["--working-directory=D:/work"],
        ] {
            let options = parse_args(args).unwrap().startup_options();
            assert_eq!(options.working_directory, Some(PathBuf::from("D:/work")));
            assert_eq!(options.command, None);
        }
    }

    #[test]
    fn command_captures_the_tail_verbatim() {
        let options = startup(["-e", "git", "--literal=two words", "--flag"]);
        assert_eq!(
            options.command,
            Some(Program::WithArgs {
                program: "git".into(),
                args: vec!["--literal=two words".into(), "--flag".into()],
            })
        );
        // 命令之后的参数不再解释为本程序旗标。
        let options = startup(["-e", "pwsh", "--working-directory", "x"]);
        assert_eq!(
            options.command,
            Some(Program::WithArgs {
                program: "pwsh".into(),
                args: vec!["--working-directory".into(), "x".into()],
            })
        );
        assert_eq!(options.working_directory, None);
    }

    #[test]
    fn unknown_or_incomplete_flags_are_errors() {
        assert!(parse(["--sploot"]).is_err());
        assert!(parse(["--working-directory"]).is_err());
        assert!(parse(["-e"]).is_err());
    }

    #[test]
    fn resource_verbs_route_to_the_cli_persona() {
        for args in [
            vec!["env"],
            vec!["pane", "list"],
            vec!["pane", "wait", "17"],
            vec!["agent", "list"],
            vec!["window", "close", "1"],
            vec!["tab", "close", "0", "--window", "1"],
            vec!["ctl", "describe"],
            vec!["notify-test"],
            vec!["setup-ai", "--remove"],
        ] {
            let cli = parse_args(args.clone())
                .unwrap_or_else(|error| panic!("{args:?} should parse: {error}"));
            assert!(
                cli.subcommands.is_some(),
                "{args:?} must route to the CLI persona"
            );
        }
    }

    /// 安装器卸载链依赖的 verb 形态:必须是 CLI 人格且带 `--remove`。
    #[test]
    fn setup_ai_remove_parses_to_the_installer_hook() {
        match parse(["setup-ai", "--remove"]).unwrap().subcommands {
            Some(Subcommands::SetupAi(options)) => assert!(options.remove),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse(["setup-ai"]).unwrap().subcommands {
            Some(Subcommands::SetupAi(options)) => assert!(!options.remove),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn unknown_verbs_and_bad_arguments_are_rejected() {
        for args in [
            vec!["bogus"],
            vec!["pane", "bogus"],
            vec!["pane", "wait"],
            vec!["pane", "wait", "not-a-number"],
            vec!["pane", "send", "17"],
            vec!["pane", "resize", "17", "not-a-ratio"],
            vec!["ctl", "describe", "extra"],
        ] {
            assert!(
                parse_args(args.clone()).is_err(),
                "{args:?} must be rejected"
            );
        }
    }

    #[test]
    fn wait_without_submit_is_rejected_at_parse_time() {
        // 逻辑矛盾在执行前拒绝,不能「执行一半静默停」。
        assert!(parse_args(["pane", "send", "17", "hi", "--no-submit", "--wait"]).is_err());
        assert!(parse_args(["pane", "paste", "17", "--stdin", "--no-submit", "--wait"]).is_err());
        assert!(parse_args(["agent", "send", "codex", "hi", "--no-submit", "--wait"]).is_err());
    }

    #[test]
    fn explorer_drive_root_quote_is_repaired() {
        // 防回归(上游 issue #36):盘符根目录尾引号补回分隔符,其余形态不动。
        assert_eq!(
            repair_context_menu_dir(PathBuf::from("D:\"")),
            PathBuf::from("D:\\")
        );
        assert_eq!(
            repair_context_menu_dir(PathBuf::from("D:/work")),
            PathBuf::from("D:/work")
        );
    }

    #[test]
    fn program_accessors_cover_both_variants() {
        let just = Program::Just("pwsh".into());
        assert_eq!(just.program(), "pwsh");
        assert!(just.args().is_empty());
        let full = Program::WithArgs {
            program: "git".into(),
            args: vec!["status".into()],
        };
        assert_eq!(full.program(), "git");
        assert_eq!(full.args(), &["status"]);
    }
}
