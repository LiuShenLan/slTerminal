//! 资源 + 动词薄别名。
//!
//! [`super::cli`] 暴露完整协议;这里给同一批能力配一套按资源分组的入口:
//!
//! ```text
//! slterm env                        我在哪、控制面在哪、有哪些命令
//! slterm pane list                  所有 pane
//! slterm pane read <id>             读某个 pane 的输出
//! slterm pane send <id> <文本>      写一行进某个 pane
//! slterm agent send <名字> <任务>   派任务并提交
//! ```
//!
//! `pane` 与 `agent` 分成两个资源不只是好读——agent 路径带 generation 绑定
//! (会话退出重开后不会把任务投给新会话),pane 路径没有这层保护。
//!
//! 这些都是完整协议的**薄别名**:同一个 `request_once`、同一 `ApiResponse`
//! 信封、同样的 generation / `after_seq` 竞态保护。短的只是命令行。

use std::error::Error;
use std::fs::File;
use std::io::Read;

use serde_json::{Value, json};

use super::MAX_PROMPT_BYTES;
use super::cli::{
    CliError, PrintedCliError, print_response, request_once, require_submission_baseline,
    snapshot_of, validated_timeout, wait_state_name,
};
use super::command::validate_paste_text;
use crate::cli::{
    AgentCommand, AgentDelegateOptions, AgentOptions, AgentPasteOptions, AgentReadOptions,
    AgentSendOptions, AgentWaitOptions, ControlWaitState, EnvOptions, ListOptions,
    PaneCloseOptions, PaneCommand, PaneExecOptions, PaneOptions, PanePasteOptions, PaneReadOptions,
    PaneResizeOptions, PaneSendOptions, PaneWaitOptions, PaneZoomOptions, PasteSourceOptions,
    TabCloseOptions, TabCommand, TabMoveOptions, TabRenameOptions, TabResourceOptions,
    WindowCloseOptions, WindowCommand, WindowResourceOptions,
};
use std::time::Duration;

/// 与完整协议一致的错误出口:失败也回 JSON 信封;`PrintedCliError` 表示响应
/// 已打印,不再包第二层。
fn finish(pretty: bool, outcome: Result<(), Box<dyn Error>>) -> Result<(), Box<dyn Error>> {
    match outcome {
        Ok(()) => Ok(()),
        Err(error) if error.downcast_ref::<PrintedCliError>().is_some() => Err(error),
        Err(error) => {
            let code = error
                .downcast_ref::<CliError>()
                .map_or("cli_transport_error", CliError::code);
            let response =
                super::ApiResponse::failure("cli", super::ApiError::new(code, error.to_string()));
            print_response(&response, pretty)
        }
    }
}

pub(super) fn window(options: WindowResourceOptions) -> Result<(), Box<dyn Error>> {
    match options.command {
        WindowCommand::Close(options) => {
            let pretty = options.output.pretty;
            finish(pretty, window_close(options))
        }
    }
}

pub(super) fn tab(options: TabResourceOptions) -> Result<(), Box<dyn Error>> {
    match options.command {
        TabCommand::Close(options) => {
            let pretty = options.output.pretty;
            finish(pretty, tab_close(options))
        }
        TabCommand::Rename(options) => {
            let pretty = options.output.pretty;
            finish(pretty, tab_rename(options))
        }
        TabCommand::Move(options) => {
            let pretty = options.output.pretty;
            finish(pretty, tab_move(options))
        }
    }
}

pub(super) fn pane(options: PaneOptions) -> Result<(), Box<dyn Error>> {
    match options.command {
        PaneCommand::List(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_list(options))
        }
        PaneCommand::Read(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_read(options))
        }
        PaneCommand::Send(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_send(options))
        }
        PaneCommand::Paste(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_paste(options))
        }
        PaneCommand::Wait(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_wait(options))
        }
        PaneCommand::Exec(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_exec(options))
        }
        PaneCommand::Close(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_close(options))
        }
        PaneCommand::Zoom(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_zoom(options))
        }
        PaneCommand::Resize(options) => {
            let pretty = options.output.pretty;
            finish(pretty, pane_resize(options))
        }
    }
}

pub(super) fn agent(options: AgentOptions) -> Result<(), Box<dyn Error>> {
    match options.command {
        AgentCommand::List(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_list(options))
        }
        AgentCommand::Send(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_send(options))
        }
        AgentCommand::Delegate(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_delegate(options))
        }
        AgentCommand::Paste(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_paste(options))
        }
        AgentCommand::Read(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_read(options))
        }
        AgentCommand::Wait(options) => {
            let pretty = options.output.pretty;
            finish(pretty, agent_wait(options))
        }
    }
}

// ─── env:不依赖控制面的发现层入口 ──────────────────────────────────────

/// 观测到的环境契约值(离线时全部可缺省)。
struct ObservedEnv {
    term_program: Option<String>,
    term_program_version: Option<String>,
    pane_id: u64,
    cli: Option<String>,
    bin_dir: Option<String>,
}

fn observe_env() -> ObservedEnv {
    let pane_id = std::env::var(crate::agent_env::PANE_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let executable = crate::agent_env::executable();
    let cli = std::env::var(crate::agent_env::CLI_ENV)
        .ok()
        .or_else(|| executable.as_ref().map(|path| path.display().to_string()));
    let bin_dir = std::env::var(crate::agent_env::BIN_DIR_ENV)
        .ok()
        .or_else(|| {
            executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(|path| path.display().to_string())
        });
    ObservedEnv {
        term_program: std::env::var("TERM_PROGRAM").ok(),
        term_program_version: std::env::var("TERM_PROGRAM_VERSION").ok(),
        pane_id,
        cli,
        bin_dir,
    }
}

/// `slterm env` —— 发现层入口。
///
/// 环境那半段**不依赖控制面**:即使没有运行实例、端口文件过期、或这根本
/// 不是 slTerm 的 pane,命令仍然成功返回并如实说明缺什么。探测命令若在
/// 「没连上」时整体失败,调用方唯一能学到的就是「不知道」。
pub(super) fn env(options: EnvOptions) -> Result<(), Box<dyn Error>> {
    let pretty = options.output.pretty;
    finish(pretty, env_inner(options))
}

fn env_inner(options: EnvOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let observed = observe_env();
    let runtime = probe_runtime(timeout);
    let reachable = runtime
        .get("reachable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // 控制面没通时不再发第二次请求——答案已经写在 runtime 段里了。
    let self_pane = (reachable && observed.pane_id > 0)
        .then(|| describe_self(observed.pane_id, timeout))
        .flatten();
    let response = env_report(observed, runtime, self_pane);
    print_response(&response, options.output.pretty)
}

/// 响应组装(离线可用):四个契约键恒在,pane 缺省为 0、CLI/bin 目录回落
/// 到当前可执行文件——运行时不可达不等于什么都不报告。
fn env_report(
    observed: ObservedEnv,
    runtime: Value,
    self_pane: Option<Value>,
) -> super::ApiResponse {
    super::ApiResponse::success(
        "env",
        json!({
            "inside_slterm": observed.term_program.as_deref() == Some(crate::agent_env::TERM_PROGRAM),
            "term_program": observed.term_program,
            "term_program_version": observed.term_program_version,
            "pane_id": observed.pane_id,
            "cli": observed.cli,
            "bin_dir": observed.bin_dir,
            "runtime": runtime,
            "self": self_pane,
            "commands": command_catalog(),
        }),
    )
}

/// 探测控制面。三种结果都要能区分:通、没通、通了但拒绝。
fn probe_runtime(timeout: Duration) -> Value {
    match request_once("runtime.describe", json!({}), timeout) {
        Ok(response) if response.ok => json!({ "reachable": true, "describe": response.result }),
        Ok(response) => json!({ "reachable": true, "error": response.error }),
        Err(error) => json!({
            "reachable": false,
            "error": { "code": "runtime_unavailable", "message": error.to_string() },
        }),
    }
}

/// 从快照里摘出调用者自己那个 pane;失败降级为 `None`(锦上添花段)。
fn describe_self(pane_id: u64, timeout: Duration) -> Option<Value> {
    let response = request_once("runtime.snapshot", json!({}), timeout).ok()?;
    if !response.ok {
        return None;
    }
    let snapshot = snapshot_of(response.result.as_ref()?)?;
    serde_json::to_value(snapshot.pane(None, pane_id).ok()?).ok()
}

/// 命令清单:给**可直接复制执行**的样例而不是抽象签名。模型照抄一条完整
/// 命令的成功率远高于自己按参数表拼装。
fn command_catalog() -> Value {
    json!([
        {
            "command": "slterm window close <window>",
            "purpose": "Close an idle window. Busy panes return confirmation_required.",
            "example": "slterm window close 1",
        },
        {
            "command": "slterm tab close <tab> --window <window>",
            "purpose": "Close an idle tab by its zero-based index within one window.",
            "example": "slterm tab close 2 --window 1",
        },
        {
            "command": "slterm tab rename <tab> <name> --window <window>",
            "purpose": "Set a tab's custom name; an empty name restores its generated title.",
            "example": "slterm tab rename 2 tests --window 1",
        },
        {
            "command": "slterm tab move <tab> <to> --window <window>",
            "purpose": "Move a tab to another index in the same window.",
            "example": "slterm tab move 2 0 --window 1",
        },
        {
            "command": "slterm pane list",
            "purpose": "List every pane with id, task state, cwd, and Git branch.",
            "example": "slterm pane list",
        },
        {
            "command": "slterm pane read <pane>",
            "purpose": "Read the tail of a pane's real terminal buffer.",
            "example": "slterm pane read 17 --lines 80",
        },
        {
            "command": "slterm pane send <pane> <text>",
            "purpose": "Write one line into a pane and press Enter.",
            "example": "slterm pane send 17 \"cargo test\" --wait",
        },
        {
            "command": "slterm pane paste <pane> [text|--stdin|--from-file]",
            "purpose": "Paste bounded UTF-8 as one bracketed block.",
            "example": "slterm pane paste 17 --from-file task.txt --wait",
        },
        {
            "command": "slterm pane wait <pane>",
            "purpose": "Block until a pane settles.",
            "example": "slterm pane wait 17 --after-seq 41",
        },
        {
            "command": "slterm pane exec <pane> -- <program> [args]",
            "purpose": "Run an independent non-TTY child in the pane's cwd and capture stdout/stderr separately.",
            "example": "slterm pane exec 17 -- cargo test",
        },
        {
            "command": "slterm pane close <pane>",
            "purpose": "Close an idle pane. Busy panes return confirmation_required.",
            "example": "slterm pane close 17",
        },
        {
            "command": "slterm pane zoom <pane> --zoomed <true|false>",
            "purpose": "Idempotently enable or disable focused-pane zoom for the pane's tab.",
            "example": "slterm pane zoom 17 --zoomed true",
        },
        {
            "command": "slterm pane resize <pane> <ratio>",
            "purpose": "Set the pane's share of its direct parent split, from 0.05 through 0.95.",
            "example": "slterm pane resize 17 0.60",
        },
        {
            "command": "slterm agent list",
            "purpose": "List panes running an AI CLI, with session identity and generation. Resolve a delegation target here first — never send to whichever pane happens to be focused.",
            "example": "slterm agent list",
        },
        {
            "command": "slterm agent send <agent> <task>",
            "purpose": "Hand one task to an agent and submit it, bound to the agent's current generation.",
            "example": "slterm agent send codex \"fix the login regression\" --wait",
        },
        {
            "command": "slterm agent delegate <agent> <task>",
            "purpose": "Delegate one task and route the target Agent's final answer back to the calling Agent pane. Requires SLTERM_PANE_ID.",
            "example": "slterm agent delegate codex \"review the change and report the result\"",
        },
        {
            "command": "slterm agent paste <agent> [text|--stdin|--from-file]",
            "purpose": "Paste a bounded multi-line task into the same managed-agent generation.",
            "example": "Get-Content task.md | slterm agent paste codex --stdin --wait",
        },
        {
            "command": "slterm agent read <agent>",
            "purpose": "Read the tail of what the agent printed, straight from its terminal grid.",
            "example": "slterm agent read codex --lines 80",
        },
        {
            "command": "slterm agent wait <agent>",
            "purpose": "Block until the agent's turn ends. Pass --after-seq with the counter observed before dispatching.",
            "example": "slterm agent wait codex --after-seq 41",
        },
        {
            "command": "slterm ctl --help",
            "purpose": "The full protocol: split panes, start or fork agents into isolated Git worktrees, run commands for a real exit code, subscribe to state events, or drive a whole multi-agent layout in one orchestrate request.",
            "example": "slterm ctl describe --pretty",
        },
    ])
}

// ─── window / tab ────────────────────────────────────────────────────────

fn window_close(options: WindowCloseOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "window.close",
        json!({ "window_id": options.window }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn tab_close(options: TabCloseOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "tab.close",
        json!({ "window_id": options.window, "tab_index": options.tab, "force": options.force }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn tab_rename(options: TabRenameOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "tab.rename",
        json!({
            "window_id": options.window,
            "tab_index": options.tab,
            "name": options.name
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn tab_move(options: TabMoveOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "tab.move",
        json!({
            "window_id": options.window,
            "tab_index": options.tab,
            "to_index": options.to
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

// ─── pane ────────────────────────────────────────────────────────────────

fn pane_list(options: ListOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once("runtime.snapshot", json!({}), timeout)?;
    // 快照是整棵窗口树;这条命令的用途是「给我一张可选目标表」,压平成一维行。
    let Some(flattened) = flatten_panes(&response, options.window) else {
        // 形状不对(例如 runtime 报错)原样透传,不伪造空列表。
        return print_response(&response, options.output.pretty);
    };
    print_response(
        &super::ApiResponse::success("pane.list", flattened),
        options.output.pretty,
    )
}

fn flatten_panes(response: &super::ApiResponse, window: Option<u64>) -> Option<Value> {
    let snapshot = snapshot_of(response.result.as_ref()?)?;
    let mut panes = Vec::new();
    for tree in snapshot
        .windows
        .iter()
        .filter(|tree| window.is_none_or(|id| tree.id == id))
    {
        for tab in &tree.tabs {
            for pane in &tab.panes {
                panes.push(json!({
                    "window_id": tree.id,
                    "tab_index": tab.index,
                    "tab_active": tab.active,
                    "pane_id": pane.id,
                    "pane_active": pane.active,
                    "title": pane.title,
                    "cwd": pane.cwd,
                    "branch": pane.branch,
                    "agent": pane.agent,
                    "task_state": pane.task_state,
                    "state_change_seq": pane.state_change_seq,
                }));
            }
        }
    }
    Some(json!({ "revision": snapshot.revision, "panes": panes }))
}

fn pane_read(options: PaneReadOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.read",
        json!({ "window_id": options.window, "pane_id": options.pane, "lines": options.lines }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_send(options: PaneSendOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let wait_timeout = submission_wait_timeout(options.wait, options.wait_timeout_ms, timeout)?;
    let response = request_once(
        "pane.prompt",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "text": joined(&options.text),
            "submit": !options.no_submit,
        }),
        timeout,
    )?;
    if !response.ok || !options.wait {
        return print_response(&response, options.output.pretty);
    }
    // 基线取自提交后的那张快照。用它当 `after_seq` 才让随后的等待意味着
    // 「又静下来了」,而不是「本来就是静的」。
    let baseline = require_submission_baseline(dispatched_state_change_seq(
        &response,
        options.window,
        Some(options.pane),
    ))?;
    let response = request_once(
        "pane.wait",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "state": wait_state_name(ControlWaitState::Settled),
            "timeout_ms": wait_timeout.as_millis() as u64,
            "after_seq": baseline,
        }),
        wait_timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_paste(options: PanePasteOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let wait_timeout = submission_wait_timeout(options.wait, options.wait_timeout_ms, timeout)?;
    let text = read_paste_source(options.source)?;
    let response = request_once(
        "pane.paste",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "text": text,
            "submit": !options.no_submit,
        }),
        timeout,
    )?;
    if !response.ok || !options.wait {
        return print_response(&response, options.output.pretty);
    }
    let baseline = require_submission_baseline(dispatched_state_change_seq(
        &response,
        options.window,
        Some(options.pane),
    ))?;
    let response = request_once(
        "pane.wait",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "state": wait_state_name(ControlWaitState::Settled),
            "timeout_ms": wait_timeout.as_millis() as u64,
            "after_seq": baseline,
        }),
        wait_timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_wait(options: PaneWaitOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.wait",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "state": wait_state_name(options.state),
            "timeout_ms": timeout.as_millis() as u64,
            "after_seq": options.after_seq,
        }),
        timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_exec(options: PaneExecOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.exec",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "argv": options.argv,
            "timeout_ms": options.timeout_ms,
            "max_output_bytes": options.max_output_bytes
        }),
        timeout.saturating_add(Duration::from_secs(2)),
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_close(options: PaneCloseOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.close",
        json!({ "window_id": options.window, "pane_id": options.pane }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_zoom(options: PaneZoomOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.zoom",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "zoomed": options.zoomed
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn pane_resize(options: PaneResizeOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "pane.resize",
        json!({
            "window_id": options.window,
            "pane_id": options.pane,
            "ratio": options.ratio
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

// ─── agent ───────────────────────────────────────────────────────────────

fn agent_list(options: ListOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "agents.list",
        json!({ "window_id": options.window }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn agent_send(options: AgentSendOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let wait_timeout = submission_wait_timeout(options.wait, options.wait_timeout_ms, timeout)?;
    let response = request_once(
        "agent.prompt",
        json!({
            "agent": options.agent,
            "generation": options.generation,
            "text": joined(&options.text),
            "submit": !options.no_submit,
        }),
        timeout,
    )?;
    if !response.ok || !options.wait {
        return print_response(&response, options.output.pretty);
    }
    let baseline = require_submission_baseline(dispatched_state_change_seq(&response, None, None))?;
    // `agent.wait` 要求显式代际——这是「别把另一个会话的静默当成这次任务
    // 完成」的那道锁。派发响应正常都带它;真取不到时明确报错。
    let Some(generation) = options
        .generation
        .or_else(|| dispatched_generation(&response))
    else {
        return Err(CliError::new(
            "submission_outcome_unknown",
            "the task was delivered but the response carried no agent generation; \
             re-resolve the target with `slterm agent list` before waiting",
        )
        .into());
    };
    let response = request_once(
        "agent.wait",
        json!({
            "agent": options.agent,
            "generation": generation,
            "state": wait_state_name(ControlWaitState::Settled),
            "timeout_ms": wait_timeout.as_millis() as u64,
            "after_seq": baseline,
        }),
        wait_timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

fn agent_delegate(options: AgentDelegateOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let origin_pane_id = std::env::var(crate::agent_env::PANE_ENV)
        .ok()
        .and_then(|pane| pane.parse::<u64>().ok())
        .filter(|pane| *pane > 0)
        .ok_or_else(|| {
            CliError::new(
                "runtime_unavailable",
                "agent delegation must be started inside a local slTerminal Agent pane with SLTERM_PANE_ID",
            )
        })?;
    let response = request_once(
        "agent.delegate",
        json!({
            "agent": options.agent,
            "generation": options.generation,
            "text": joined(&options.text),
            "origin_pane_id": origin_pane_id,
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn agent_paste(options: AgentPasteOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let wait_timeout = submission_wait_timeout(options.wait, options.wait_timeout_ms, timeout)?;
    let text = read_paste_source(options.source)?;
    let response = request_once(
        "agent.paste",
        json!({
            "agent": options.agent,
            "generation": options.generation,
            "text": text,
            "submit": !options.no_submit,
        }),
        timeout,
    )?;
    if !response.ok || !options.wait {
        return print_response(&response, options.output.pretty);
    }
    let baseline = require_submission_baseline(dispatched_state_change_seq(&response, None, None))?;
    let Some(generation) = options
        .generation
        .or_else(|| dispatched_generation(&response))
    else {
        return Err(CliError::new(
            "submission_outcome_unknown",
            "the paste was delivered but the response carried no agent generation; \
             re-resolve the target with `slterm agent list` before waiting",
        )
        .into());
    };
    let response = request_once(
        "agent.wait",
        json!({
            "agent": options.agent,
            "generation": generation,
            "state": wait_state_name(ControlWaitState::Settled),
            "timeout_ms": wait_timeout.as_millis() as u64,
            "after_seq": baseline,
        }),
        wait_timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

fn agent_read(options: AgentReadOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let response = request_once(
        "agent.read",
        json!({
            "agent": options.agent,
            "generation": options.generation,
            "lines": options.lines,
        }),
        timeout,
    )?;
    print_response(&response, options.output.pretty)
}

fn agent_wait(options: AgentWaitOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    // 未给代际时用当前活跃代际补齐;期间换代会让 agent.get 先报错,
    // 而不是让等待静静地盯着错目标。
    let generation = match options.generation {
        Some(generation) => generation,
        None => resolve_generation(&options.agent, timeout)?,
    };
    let response = request_once(
        "agent.wait",
        json!({
            "agent": options.agent,
            "generation": generation,
            "state": wait_state_name(options.state),
            "timeout_ms": timeout.as_millis() as u64,
            "after_seq": options.after_seq,
        }),
        timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, options.output.pretty)
}

/// 当前活跃代际。目标不存在或响应缺字段都算失败——猜一个代际去等,
/// 等到的可能是另一个会话。
fn resolve_generation(agent: &str, timeout: Duration) -> Result<u64, Box<dyn Error>> {
    let response = request_once("agent.get", json!({ "agent": agent }), timeout)?;
    if !response.ok {
        let message = response
            .error
            .as_ref()
            .map_or("agent.get failed", |error| error.message.as_str())
            .to_owned();
        return Err(CliError::new("target_not_found", message).into());
    }
    response
        .result
        .as_ref()
        .and_then(|result| result.get("agent")?.get("generation")?.as_u64())
        .ok_or_else(|| {
            CliError::new(
                "target_not_found",
                format!("no live agent matches {agent:?}; run `slterm agent list`"),
            )
            .into()
        })
}

// ─── 共用小件 ────────────────────────────────────────────────────────────

fn read_paste_source(source: PasteSourceOptions) -> Result<String, Box<dyn Error>> {
    let PasteSourceOptions {
        text,
        stdin,
        from_file,
    } = source;
    let value = if stdin {
        read_paste_utf8(std::io::stdin().lock(), "stdin")?
    } else if let Some(path) = from_file {
        let file = File::open(&path).map_err(|error| {
            CliError::new(
                "input_read_error",
                format!("could not open paste file {}: {error}", path.display()),
            )
        })?;
        read_paste_utf8(file, &path.display().to_string())?
    } else if text.is_empty() {
        return Err(CliError::new(
            "invalid_params",
            "provide paste text, --stdin, or --from-file <path>",
        )
        .into());
    } else {
        joined(&text)
    };
    validate_paste_text(&value).map_err(|error| CliError::new("invalid_params", error.message))?;
    Ok(value)
}

fn read_paste_utf8(reader: impl Read, label: &str) -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_PROMPT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::new(
                "input_read_error",
                format!("could not read paste input {label}: {error}"),
            )
        })?;
    if bytes.len() > MAX_PROMPT_BYTES {
        return Err(CliError::new(
            "invalid_params",
            format!("paste text exceeds the {MAX_PROMPT_BYTES}-byte limit"),
        )
        .into());
    }
    String::from_utf8(bytes).map_err(|error| {
        CliError::new(
            "invalid_utf8",
            format!("paste input {label} is not valid UTF-8: {error}"),
        )
        .into()
    })
}

/// 多个词拼一句:`send codex fix login` 与 `send codex "fix login"` 等价,
/// 否则忘记加引号就变成一条截断的任务被真的派出去。
fn joined(text: &[String]) -> String {
    text.join(" ")
}

/// 派发响应里目标 pane 的跃迁计数:动作元数据在 `result.action`、规范快照
/// 在 `result.snapshot`;agent 的窗口与 pane 位于 `action.agent`。
fn dispatched_state_change_seq(
    response: &super::ApiResponse,
    fallback_window: Option<u64>,
    fallback_pane: Option<u64>,
) -> Option<u64> {
    let result = response.result.as_ref()?;
    let action = result.get("action").unwrap_or(result);
    let agent = action.get("agent");
    let window_id = agent
        .and_then(|agent| agent.get("window_id"))
        .and_then(Value::as_u64)
        .or_else(|| action.get("window_id").and_then(Value::as_u64))
        .or(fallback_window);
    let pane_id = action
        .get("agent")
        .and_then(|agent| agent.get("pane_id"))
        .and_then(Value::as_u64)
        .or_else(|| action.get("pane_id").and_then(Value::as_u64))
        .or(fallback_pane)?;
    let snapshot = snapshot_of(result)?;
    snapshot
        .pane(window_id, pane_id)
        .ok()
        .map(|pane| pane.state_change_seq)
}

fn dispatched_generation(response: &super::ApiResponse) -> Option<u64> {
    let result = response.result.as_ref()?;
    result
        .get("action")
        .unwrap_or(result)
        .get("agent")?
        .get("generation")?
        .as_u64()
}

/// 输入写出前先校验等待选项:非法 wait 不能留下一条已提交的任务。
fn submission_wait_timeout(
    wait: bool,
    timeout_ms: u64,
    request_timeout: Duration,
) -> Result<Duration, Box<dyn Error>> {
    if wait {
        validated_timeout(timeout_ms)
    } else {
        Ok(request_timeout)
    }
}

#[cfg(test)]
mod shortcuts_tests {
    use super::*;
    use crate::cli::ShortOutput;
    use crate::runtime_api::MAX_WAIT;
    use crate::runtime_api::server::{Endpoint, clear_child_endpoint, register_child_endpoint};
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::{Mutex, MutexGuard};
    use std::thread::JoinHandle;

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    /// CHILD_ENDPOINT 是进程级发现槽:本组用例串行化,防止并行测试互相看到
    /// 对方的假运行时(全量回归本身也以 --test-threads=1 执行)。
    static ENDPOINT_LOCK: Mutex<()> = Mutex::new(());

    struct ScopedEndpoint {
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedEndpoint {
        fn install(endpoint: Endpoint) -> Self {
            let guard = ENDPOINT_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            register_child_endpoint(Some(endpoint));
            Self { _guard: guard }
        }
    }

    impl Drop for ScopedEndpoint {
        fn drop(&mut self) {
            clear_child_endpoint();
        }
    }

    fn fixture_snapshot(pane_id: u64, state_change_seq: u64) -> Value {
        json!({
            "revision": 1,
            "process_id": 1,
            "app_version": "test",
            "protocol_version": 1,
            "windows": [{
                "id": 1,
                "focused": true,
                "active_tab": 0,
                "focused_pane_id": pane_id,
                "tabs": [{
                    "index": 0,
                    "active": true,
                    "label": "test",
                    "kind": "terminal",
                    "bell": false,
                    "focused_pane_id": pane_id,
                    "panes": [{
                        "id": pane_id,
                        "active": true,
                        "title": "test",
                        "cwd": "D:/work",
                        "branch": "main",
                        "task_state": "idle",
                        "state_change_seq": state_change_seq
                    }]
                }]
            }]
        })
    }

    /// 假运行时:按序接受连接、逐条记录请求,并把模板响应补上请求 id。
    fn fixture_runtime(templates: Vec<Value>) -> (Endpoint, JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind fixture runtime");
        let endpoint = Endpoint {
            port: listener.local_addr().unwrap().port(),
            token: TEST_TOKEN.to_owned(),
        };
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for mut template in templates {
                let (mut stream, _) = listener.accept().expect("accept fixture request");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .expect("read fixture request");
                let request: Value = serde_json::from_str(&line).expect("fixture request is JSON");
                template["id"] = request["id"].clone();
                requests.push(request);
                stream
                    .write_all(format!("{template}\n").as_bytes())
                    .expect("write fixture response");
            }
            requests
        });
        (endpoint, handle)
    }

    fn success(result: Value) -> Value {
        json!({
            "protocol": super::super::PROTOCOL_NAME,
            "version": super::super::PROTOCOL_VERSION,
            "id": "fixture",
            "ok": true,
            "result": result
        })
    }

    #[test]
    fn wait_baseline_uses_the_post_submission_state_change_seq() {
        let (endpoint, server) = fixture_runtime(vec![
            success(json!({
                "action": { "window_id": 1, "pane_id": 17 },
                "snapshot": fixture_snapshot(17, 41)
            })),
            success(json!({ "snapshot": fixture_snapshot(17, 42) })),
        ]);
        let _guard = ScopedEndpoint::install(endpoint);

        pane_send(PaneSendOptions {
            pane: 17,
            text: vec!["cargo".into(), "test".into()],
            no_submit: false,
            wait: true,
            wait_timeout_ms: 1_500,
            window: None,
            output: ShortOutput { pretty: false },
            timeout_ms: 1_000,
        })
        .expect("send with wait succeeds");

        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["method"], "pane.prompt");
        assert_eq!(requests[0]["params"]["text"], "cargo test");
        assert_eq!(requests[0]["params"]["submit"], true);
        assert_eq!(requests[1]["method"], "pane.wait");
        // 基线 = 提交后快照的 state_change_seq,不是提交前观察值。
        assert_eq!(requests[1]["params"]["after_seq"], 41);
        assert_eq!(requests[1]["params"]["state"], "settled");
        assert_eq!(requests[1]["params"]["timeout_ms"], 1_500);
    }

    #[test]
    fn wait_without_a_valid_baseline_fails_instead_of_degrading() {
        let (endpoint, server) = fixture_runtime(vec![success(json!({
            "action": { "window_id": 1, "pane_id": 17 },
            "snapshot": fixture_snapshot(17, 0)
        }))]);
        let _guard = ScopedEndpoint::install(endpoint);

        let error = pane_send(PaneSendOptions {
            pane: 17,
            text: vec!["cargo test".into()],
            no_submit: false,
            wait: true,
            wait_timeout_ms: 1_000,
            window: None,
            output: ShortOutput { pretty: false },
            timeout_ms: 1_000,
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<CliError>().unwrap().code(),
            "submission_outcome_unknown"
        );
        // 只发了一次请求:不降级成无基线等待。
        assert_eq!(server.join().unwrap().len(), 1);
    }

    #[test]
    fn agent_send_resolves_generation_from_the_dispatch_response() {
        let (endpoint, server) = fixture_runtime(vec![
            success(json!({
                "action": { "agent": { "window_id": 1, "pane_id": 9, "generation": 7 } },
                "snapshot": fixture_snapshot(9, 12)
            })),
            success(json!({ "snapshot": fixture_snapshot(9, 13) })),
        ]);
        let _guard = ScopedEndpoint::install(endpoint);

        agent_send(AgentSendOptions {
            agent: "codex".into(),
            text: vec!["review".into(), "this".into()],
            no_submit: false,
            wait: true,
            wait_timeout_ms: 2_000,
            generation: None,
            output: ShortOutput { pretty: false },
            timeout_ms: 1_000,
        })
        .expect("agent send with wait succeeds");

        let requests = server.join().unwrap();
        assert_eq!(requests[0]["method"], "agent.prompt");
        assert_eq!(requests[1]["method"], "agent.wait");
        assert_eq!(requests[1]["params"]["generation"], 7);
        assert_eq!(requests[1]["params"]["after_seq"], 12);
    }

    #[test]
    fn env_report_is_offline_first_and_keeps_the_contract_keys() {
        // 无运行实例、无身份变量的最差形态:命令仍成功,四个契约键俱在。
        let response = env_report(
            ObservedEnv {
                term_program: None,
                term_program_version: None,
                pane_id: 0,
                cli: Some("C:/tools/slterm.exe".into()),
                bin_dir: Some("C:/tools".into()),
            },
            json!({
                "reachable": false,
                "error": { "code": "runtime_unavailable", "message": "no running slterm runtime found" }
            }),
            None,
        );
        assert!(response.ok);
        let result = response.result.expect("env result");
        assert_eq!(result["inside_slterm"], false);
        assert_eq!(result["pane_id"], 0);
        assert_eq!(result["cli"], "C:/tools/slterm.exe");
        assert_eq!(result["bin_dir"], "C:/tools");
        assert_eq!(result["runtime"]["reachable"], false);
        assert!(result.get("term_program").is_some());
        assert!(result.get("self").is_some());
        assert!(result["commands"].is_array());
        let catalog = result["commands"].as_array().unwrap();
        assert!(!catalog.is_empty());
    }

    #[test]
    fn env_reports_identity_when_inside_a_slterm_pane() {
        let response = env_report(
            ObservedEnv {
                term_program: Some("slterm".into()),
                term_program_version: Some("0.1.0".into()),
                pane_id: 17,
                cli: Some("C:/tools/slterm.exe".into()),
                bin_dir: Some("C:/tools".into()),
            },
            json!({ "reachable": true, "describe": { "protocol": "slterm.runtime" } }),
            Some(json!({ "id": 17 })),
        );
        let result = response.result.expect("env result");
        assert_eq!(result["inside_slterm"], true);
        assert_eq!(result["pane_id"], 17);
        assert_eq!(result["self"]["id"], 17);
    }

    #[test]
    fn pasted_words_join_into_one_line() {
        assert_eq!(joined(&["fix".to_owned(), "login".to_owned()]), "fix login");
        assert_eq!(joined(&["fix login".to_owned()]), "fix login");
    }

    #[test]
    fn paste_reader_is_strict_utf8_and_bounded() {
        let text =
            read_paste_utf8(std::io::Cursor::new("first\nsecond"), "memory").expect("valid UTF-8");
        assert_eq!(text, "first\nsecond");
        assert!(read_paste_utf8(std::io::Cursor::new([0xff]), "memory").is_err());
        assert!(
            read_paste_utf8(
                std::io::Cursor::new(vec![b'x'; MAX_PROMPT_BYTES + 1]),
                "memory"
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_extraction_accepts_direct_and_action_results() {
        let snapshot = fixture_snapshot(17, 41);
        assert_eq!(
            snapshot_of(&snapshot)
                .expect("direct snapshot")
                .pane(None, 17)
                .expect("pane")
                .state_change_seq,
            41
        );
        let wrapped = json!({ "action": { "pane_id": 17 }, "snapshot": snapshot });
        assert_eq!(snapshot_of(&wrapped).expect("wrapped").revision, 1);
    }

    #[test]
    fn dispatch_metadata_is_read_from_the_action_envelope() {
        let response = super::super::ApiResponse::success(
            "pane.prompt",
            json!({
                "action": { "window_id": 1, "pane_id": 17 },
                "snapshot": fixture_snapshot(17, 42),
            }),
        );
        assert_eq!(dispatched_state_change_seq(&response, None, None), Some(42));

        let agent = super::super::ApiResponse::success(
            "agent.prompt",
            json!({
                "action": { "agent": { "window_id": 1, "pane_id": 9, "generation": 7 } },
                "snapshot": fixture_snapshot(9, 12),
            }),
        );
        assert_eq!(dispatched_generation(&agent), Some(7));
        assert_eq!(dispatched_state_change_seq(&agent, None, None), Some(12));
    }

    #[test]
    fn catalog_examples_match_their_command() {
        // 清单是给模型照抄的;样例若和声明的命令不一致,抄过去就是错的。
        let catalog = command_catalog();
        for entry in catalog.as_array().expect("catalog is an array") {
            let command = entry["command"].as_str().expect("command");
            let example = entry["example"].as_str().expect("example");
            let mut words = command.split(' ');
            let prefix = words.by_ref().take(2).collect::<Vec<_>>().join(" ");
            assert!(
                example.contains(&prefix),
                "{example:?} should contain {prefix:?}"
            );
            if let Some(verb) = words.next().filter(|verb| !verb.starts_with('-')) {
                assert!(
                    example.contains(verb),
                    "{example:?} should exercise {verb:?}"
                );
            }
            assert!(!entry["purpose"].as_str().unwrap_or_default().is_empty());
        }
    }

    #[test]
    fn catalog_covers_resource_verbs() {
        let catalog = command_catalog();
        let commands: Vec<&str> = catalog
            .as_array()
            .expect("array")
            .iter()
            .map(|entry| entry["command"].as_str().expect("command"))
            .collect();
        for verb in ["list", "read", "send", "paste", "wait"] {
            assert!(
                commands
                    .iter()
                    .any(|command| *command == format!("slterm pane {verb}")
                        || command.starts_with(&format!("slterm pane {verb} "))),
                "pane {verb} missing from the catalog"
            );
            assert!(
                commands
                    .iter()
                    .any(|command| *command == format!("slterm agent {verb}")
                        || command.starts_with(&format!("slterm agent {verb} "))),
                "agent {verb} missing from the catalog"
            );
        }
        for command in [
            "slterm window close <window>",
            "slterm tab close <tab> --window <window>",
            "slterm tab rename <tab> <name> --window <window>",
            "slterm tab move <tab> <to> --window <window>",
            "slterm pane close <pane>",
            "slterm pane zoom <pane> --zoomed <true|false>",
            "slterm pane resize <pane> <ratio>",
            "slterm pane exec <pane> -- <program> [args]",
        ] {
            assert!(
                commands.contains(&command),
                "{command} missing from the catalog"
            );
        }
    }

    #[test]
    fn timeout_bounds_are_enforced_for_wait_aliases() {
        assert!(validated_timeout(0).is_err());
        assert!(validated_timeout(MAX_WAIT.as_millis() as u64 + 1).is_err());
        assert!(submission_wait_timeout(true, 0, Duration::from_secs(1)).is_err());
        assert_eq!(
            submission_wait_timeout(false, 0, Duration::from_secs(1)).unwrap(),
            Duration::from_secs(1)
        );
    }
}
