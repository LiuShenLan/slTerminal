//! CLI 客户端(04 篇 M7.4):同 exe 双人格的请求-响应侧。
//!
//! CLI 与任何外部客户端走**同一条**序列化协议:同一 `ApiRequest`/`ApiResponse`
//! 信封、同一端点发现、同一身份逐字段校验。CLI 不进程内调用处理器,
//! 因此协议行为不可能只在 GUI 内「碰巧可用」。
//!
//! `--wait` 基线取自提交后快照的 `state_change_seq`:缺有效非零基线时明确报错,
//! 绝不降级成「无基线等待」——那会把提交前的 idle 误判成已完成。

use std::error::Error;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::server::{Endpoint, endpoint_addr, read_endpoint};
use super::{
    ApiError, ApiRequest, ApiResponse, CONNECT_TIMEOUT, IO_TIMEOUT, MAX_REQUEST_BYTES, MAX_WAIT,
    RuntimeSnapshot,
};
use crate::cli::{
    ControlCommand, ControlOptions, ControlSplitDirection, ControlWaitState, SetupAiOptions,
    Subcommands,
};

/// 插件响应的保留量上限:与请求上限分开,整次 I/O 共用一个 deadline。
const MAX_PLUGIN_RESPONSE_BYTES: usize = 256 * 1024;

/// 建连并写出单条请求行:connect/read/write 全部带超时,写完关写半边。
fn client_stream(
    endpoint: &Endpoint,
    request: &ApiRequest,
    timeout: Option<Duration>,
) -> Result<TcpStream, std::io::Error> {
    let mut stream = TcpStream::connect_timeout(&endpoint_addr(endpoint), CONNECT_TIMEOUT)?;
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    serde_json::to_writer(&mut stream, request).map_err(std::io::Error::other)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)?;
    Ok(stream)
}

/// 单请求:三级发现拿端点;无端点 = `runtime_unavailable`(不猜端口)。
pub(super) fn request_once(
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<ApiResponse, Box<dyn Error>> {
    let endpoint = read_endpoint()
        .ok_or_else(|| CliError::new("runtime_unavailable", "no running slterm runtime found"))?;
    let request = ApiRequest::new(endpoint.token.clone(), method, params);
    read_response(&endpoint, &request, timeout)
        .map_err(|error| input_transport_error(method, error))
}

/// 插件入口:请求/响应双侧上限,整次交换共用一个 deadline。
#[allow(dead_code)] // 消费点 = 未来插件宿主(04 篇扩展位),与 CLI 共用同一协议。
pub(crate) fn request_once_bounded(
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<ApiResponse, ApiError> {
    let endpoint = read_endpoint()
        .ok_or_else(|| ApiError::new("runtime_unavailable", "no running slterm runtime found"))?;
    let request = ApiRequest::new(endpoint.token.clone(), method, params);
    let mut bytes = serde_json::to_vec(&request)
        .map_err(|error| ApiError::invalid_params(error.to_string()))?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(ApiError::new(
            "request_too_large",
            "plugin request exceeds 128 KiB",
        ));
    }
    bytes.push(b'\n');
    bounded_exchange(&endpoint, &bytes, timeout)
        .and_then(|bytes| decode_plugin_response(&request, &bytes))
        .map_err(|error| {
            let error = input_transport_error(method, error);
            if let Some(error) = error.downcast_ref::<CliError>() {
                ApiError::new(error.code, error.message.clone())
            } else {
                ApiError::new("runtime_unavailable", error.to_string())
            }
        })
}

/// 响应身份逐字段校验:protocol/version/id 一致;ok 与 result/error 互斥。
fn decode_plugin_response(
    request: &ApiRequest,
    bytes: &[u8],
) -> Result<ApiResponse, Box<dyn Error>> {
    let response: ApiResponse = serde_json::from_slice(bytes)?;
    response.verify_identity(request).map_err(|_| {
        CliError::new(
            "invalid_runtime_response",
            "runtime response identity or envelope mismatch",
        )
    })?;
    Ok(response)
}

/// 有界交换:连接/写/读共享一个 deadline;对端先关读侧容错。
fn bounded_exchange(
    endpoint: &Endpoint,
    request: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "plugin runtime request timed out",
                )
            })
    };
    let mut stream =
        TcpStream::connect_timeout(&endpoint_addr(endpoint), remaining()?.min(CONNECT_TIMEOUT))?;
    let mut pending = request;
    while !pending.is_empty() {
        stream.set_write_timeout(Some(remaining()?))?;
        let written = stream.write(pending)?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "runtime write stopped",
            )
            .into());
        }
        pending = &pending[written..];
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        stream.set_read_timeout(Some(remaining()?))?;
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(CliError::new(
                "invalid_runtime_response",
                "runtime closed before a complete JSON line",
            )
            .into());
        }
        let newline = buffer[..count].iter().position(|byte| *byte == b'\n');
        let end = newline.unwrap_or(count);
        if bytes.len() + end > MAX_PLUGIN_RESPONSE_BYTES {
            return Err(CliError::new(
                "invalid_runtime_response",
                "plugin response exceeds 256 KiB",
            )
            .into());
        }
        bytes.extend_from_slice(&buffer[..end]);
        if newline.is_some() {
            return Ok(bytes);
        }
    }
}

/// 提交类方法的传输失败改写为 `submission_outcome_unknown`:输入可能已送达,
/// 先读目标状态再决定是否重试,不盲目重复投递。
fn input_transport_error(method: &str, error: Box<dyn Error>) -> Box<dyn Error> {
    if matches!(
        method,
        "pane.prompt" | "pane.paste" | "agent.prompt" | "agent.paste" | "agent.delegate"
    ) {
        CliError::new(
            "submission_outcome_unknown",
            format!(
                "{method} did not return a confirmed response: {error}. Input may already have \
                 reached the target; read its state and output before retrying."
            ),
        )
        .into()
    } else {
        error
    }
}

/// `--wait` 基线:必须来自提交后快照的非零计数;缺了就明确失败。
pub(super) fn require_submission_baseline(baseline: Option<u64>) -> Result<u64, CliError> {
    baseline.filter(|seq| *seq > 0).ok_or_else(|| {
        CliError::new(
            "submission_outcome_unknown",
            "input was accepted but the response has no valid state baseline; completion is \
             unconfirmed. Read the target before retrying; do not resend automatically.",
        )
    })
}

fn read_response(
    endpoint: &Endpoint,
    request: &ApiRequest,
    timeout: Duration,
) -> Result<ApiResponse, Box<dyn Error>> {
    let stream = client_stream(endpoint, request, Some(timeout))?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.is_empty() {
        return Err(CliError::new(
            "invalid_runtime_response",
            "runtime closed the connection without a response",
        )
        .into());
    }
    Ok(serde_json::from_str(&line)?)
}

// ─── 输出面 ──────────────────────────────────────────────────────────────

/// 打印响应信封;`ok=false` 时返回「已打印」标记错误,外层不再包第二层。
pub(super) fn print_response(response: &ApiResponse, pretty: bool) -> Result<(), Box<dyn Error>> {
    write_cli_response(&mut std::io::stdout().lock(), response, pretty)
}

fn write_cli_response(
    output: &mut impl Write,
    response: &ApiResponse,
    pretty: bool,
) -> Result<(), Box<dyn Error>> {
    let mut json = if pretty {
        serde_json::to_string_pretty(response)?
    } else {
        serde_json::to_string(response)?
    };
    json.push('\n');
    write_cli_output(output, json.as_bytes())?;
    if response.ok {
        Ok(())
    } else {
        let error = response
            .error
            .as_ref()
            .map_or("runtime request failed", |error| &error.message);
        Err(PrintedCliError(error.to_owned()).into())
    }
}

/// 输出端断开(管道消费者退出)是正常终止,不是运行错误。
fn write_cli_output(output: &mut impl Write, bytes: &[u8]) -> Result<bool, std::io::Error> {
    match output.write_all(bytes).and_then(|()| output.flush()) {
        Ok(()) => Ok(true),
        Err(error)
            if error.kind() == std::io::ErrorKind::BrokenPipe
                || (cfg!(windows) && matches!(error.raw_os_error(), Some(109 | 232 | 233))) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

// ─── 入口分发 ────────────────────────────────────────────────────────────

/// CLI 人格总入口:响应一律 JSON;退出码 0 = 成功信封,1 = 失败/已打印错误。
pub(crate) fn run_cli(command: Subcommands) -> i32 {
    let pretty = match &command {
        Subcommands::Ctl(options) => options.pretty,
        Subcommands::Env(options) => options.output.pretty,
        Subcommands::Window(options) => match &options.command {
            crate::cli::WindowCommand::Close(options) => options.output.pretty,
        },
        Subcommands::Tab(options) => match &options.command {
            crate::cli::TabCommand::Close(options) => options.output.pretty,
            crate::cli::TabCommand::Rename(options) => options.output.pretty,
            crate::cli::TabCommand::Move(options) => options.output.pretty,
        },
        Subcommands::Pane(options) => match &options.command {
            crate::cli::PaneCommand::List(options) => options.output.pretty,
            crate::cli::PaneCommand::Read(options) => options.output.pretty,
            crate::cli::PaneCommand::Send(options) => options.output.pretty,
            crate::cli::PaneCommand::Paste(options) => options.output.pretty,
            crate::cli::PaneCommand::Wait(options) => options.output.pretty,
            crate::cli::PaneCommand::Exec(options) => options.output.pretty,
            crate::cli::PaneCommand::Close(options) => options.output.pretty,
            crate::cli::PaneCommand::Zoom(options) => options.output.pretty,
            crate::cli::PaneCommand::Resize(options) => options.output.pretty,
        },
        Subcommands::Agent(options) => match &options.command {
            crate::cli::AgentCommand::List(options) => options.output.pretty,
            crate::cli::AgentCommand::Send(options) => options.output.pretty,
            crate::cli::AgentCommand::Delegate(options) => options.output.pretty,
            crate::cli::AgentCommand::Paste(options) => options.output.pretty,
            crate::cli::AgentCommand::Read(options) => options.output.pretty,
            crate::cli::AgentCommand::Wait(options) => options.output.pretty,
        },
        // notify-test / setup-ai 直接打印诊断文本,不产出 JSON 响应。
        Subcommands::NotifyTest => false,
        Subcommands::SetupAi(_) => false,
    };

    let outcome = match command {
        Subcommands::Ctl(options) => run_control(options),
        Subcommands::Env(options) => super::shortcuts::env(options),
        Subcommands::Window(options) => super::shortcuts::window(options),
        Subcommands::Tab(options) => super::shortcuts::tab(options),
        Subcommands::Pane(options) => super::shortcuts::pane(options),
        Subcommands::Agent(options) => super::shortcuts::agent(options),
        // 同步诊断通道(09 篇采纳点 18):注册 + 时长 + 投递逐段打印,
        // 不启动 GUI、不依赖运行中的实例。
        Subcommands::NotifyTest => return crate::platform::notifications::notify_test(),
        // 卸载器入口(B69):不启动 GUI、不依赖运行中实例。
        Subcommands::SetupAi(options) => return run_setup_ai(options),
    };
    match outcome {
        Ok(()) => 0,
        Err(error) if error.downcast_ref::<PrintedCliError>().is_some() => 1,
        Err(error) => {
            let code = error
                .downcast_ref::<CliError>()
                .map_or("cli_transport_error", |error| error.code);
            let response = ApiResponse::failure("cli", ApiError::new(code, error.to_string()));
            let _ = print_response(&response, pretty);
            1
        }
    }
}

/// 卸载器专用入口(B69):不启动 GUI、不依赖运行中实例。退出码
/// 0 = 全部清理成功(未安装的 agent 不算失败),1 = 任一失败,2 = 用法错误。
fn run_setup_ai(options: SetupAiOptions) -> i32 {
    if !options.remove {
        eprintln!("usage: slterm setup-ai --remove");
        return 2;
    }
    match crate::ai_hook::remove_all_hooks() {
        Ok(failures) if failures.is_empty() => {
            println!("setup-ai: OK (hook integrations removed)");
            0
        }
        Ok(failures) => {
            for (agent, error) in failures {
                eprintln!("setup-ai: {}: {error}", agent.settings_key());
            }
            1
        }
        Err(error) => {
            eprintln!("setup-ai: {error}");
            1
        }
    }
}

/// 超时预算校验:1ms..=MAX_WAIT;越界在执行任何请求前拒绝。
pub(super) fn validated_timeout(timeout_ms: u64) -> Result<Duration, Box<dyn Error>> {
    if timeout_ms == 0 || Duration::from_millis(timeout_ms) > MAX_WAIT {
        return Err(CliError::new(
            "invalid_params",
            format!("timeout must be between 1 and {} ms", MAX_WAIT.as_millis()),
        )
        .into());
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn run_control(options: ControlOptions) -> Result<(), Box<dyn Error>> {
    let timeout = validated_timeout(options.timeout_ms)?;
    let pretty = options.pretty;
    match options.command {
        ControlCommand::Describe => {
            let response = request_once("runtime.describe", json!({}), timeout)?;
            print_response(&response, pretty)
        }
        ControlCommand::Snapshot => {
            let response = request_once("runtime.snapshot", json!({}), timeout)?;
            print_response(&response, pretty)
        }
        ControlCommand::Orchestrate { spec, file } => {
            let source = match (spec, file) {
                (Some(spec), None) => spec,
                (None, Some(path)) => std::fs::read_to_string(path)?,
                _ => {
                    return Err(CliError::new(
                        "invalid_params",
                        "exactly one of --spec or --file is required",
                    )
                    .into());
                }
            };
            let params: Value = serde_json::from_str(&source).map_err(|error| {
                CliError::new(
                    "invalid_params",
                    format!("workflow is not valid UTF-8 JSON: {error}"),
                )
            })?;
            let response = request_once(
                "runtime.orchestrate",
                params,
                timeout.saturating_add(Duration::from_secs(1)),
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Agents { window } => {
            let response = request_once("agents.list", json!({ "window_id": window }), timeout)?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentStart {
            window,
            name,
            kind,
            cwd,
            resume_session_id,
        } => {
            let response = request_once(
                "agent.start",
                json!({
                    "window_id": window,
                    "name": name,
                    "kind": kind,
                    "cwd": cwd,
                    "resume_session_id": resume_session_id
                }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentFork {
            window,
            source_pane,
            source_cwd,
            name,
            kind,
            resume_session_id,
            branch,
            base,
            path,
            allow_dirty_source,
        } => {
            let source_cwd = source_cwd.map(absolute_path).transpose()?;
            let path = path.map(absolute_path).transpose()?;
            let response = request_once(
                "agent.fork",
                json!({
                    "window_id": window,
                    "source_pane_id": source_pane,
                    "source_cwd": source_cwd,
                    "name": name,
                    "kind": kind,
                    "resume_session_id": resume_session_id,
                    "branch": branch,
                    "base": base,
                    "path": path,
                    "allow_dirty_source": allow_dirty_source
                }),
                timeout.saturating_add(super::COMMAND_TIMEOUT),
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentGet { agent, generation } => {
            let response = request_once(
                "agent.get",
                json!({ "agent": agent, "generation": generation }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentPrompt {
            agent,
            generation,
            text,
            no_submit,
        } => {
            let response = request_once(
                "agent.prompt",
                json!({
                    "agent": agent,
                    "generation": generation,
                    "text": text,
                    "submit": !no_submit
                }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentPaste {
            agent,
            generation,
            text,
            no_submit,
        } => {
            let response = request_once(
                "agent.paste",
                json!({
                    "agent": agent,
                    "generation": generation,
                    "text": text,
                    "submit": !no_submit
                }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentRead {
            agent,
            generation,
            lines,
        } => {
            let response = request_once(
                "agent.read",
                json!({ "agent": agent, "generation": generation, "lines": lines }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::AgentWait {
            agent,
            generation,
            state,
            after_seq,
        } => {
            let response = request_once(
                "agent.wait",
                json!({
                    "agent": agent,
                    "generation": generation,
                    "state": wait_state_name(state),
                    "timeout_ms": timeout.as_millis() as u64,
                    "after_seq": after_seq
                }),
                timeout.saturating_add(Duration::from_secs(1)),
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Subscribe { since } => subscribe_cli(since, timeout),
        ControlCommand::CloseWindow { window } => {
            let response = request_once("window.close", json!({ "window_id": window }), timeout)?;
            print_response(&response, pretty)
        }
        ControlCommand::Focus { window, pane } => {
            let response = request_once(
                "window.focus",
                json!({ "window_id": window, "pane_id": pane }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::NewTab { window } => {
            let response = request_once("tab.new", json!({ "window_id": window }), timeout)?;
            print_response(&response, pretty)
        }
        ControlCommand::CloseTab {
            window,
            tab_index,
            force,
        } => {
            let response = request_once(
                "tab.close",
                json!({ "window_id": window, "tab_index": tab_index, "force": force }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::RenameTab {
            window,
            tab_index,
            name,
        } => {
            let response = request_once(
                "tab.rename",
                json!({ "window_id": window, "tab_index": tab_index, "name": name }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::MoveTab {
            window,
            tab_index,
            to_index,
        } => {
            let response = request_once(
                "tab.move",
                json!({
                    "window_id": window,
                    "tab_index": tab_index,
                    "to_index": to_index
                }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Split {
            window,
            pane,
            direction,
        } => {
            let direction = match direction {
                ControlSplitDirection::Right => "left_right",
                ControlSplitDirection::Down => "top_bottom",
            };
            let response = request_once(
                "pane.split",
                json!({ "window_id": window, "pane_id": pane, "direction": direction }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::ClosePane { window, pane } => {
            let response = request_once(
                "pane.close",
                json!({ "window_id": window, "pane_id": pane }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::ZoomPane {
            window,
            pane,
            zoomed,
        } => {
            let response = request_once(
                "pane.zoom",
                json!({ "window_id": window, "pane_id": pane, "zoomed": zoomed }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::ResizePane {
            window,
            pane,
            ratio,
        } => {
            let response = request_once(
                "pane.resize",
                json!({ "window_id": window, "pane_id": pane, "ratio": ratio }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Prompt {
            window,
            pane,
            text,
            no_submit,
            wait,
        } => {
            let response = request_once(
                "pane.prompt",
                json!({
                    "window_id": window,
                    "pane_id": pane,
                    "text": text,
                    "submit": !no_submit
                }),
                timeout,
            )?;
            if !response.ok || wait.is_none() {
                return print_response(&response, pretty);
            }
            // 基线 = 提交后快照的计数;没有它就退化成「本来就静着」。
            let baseline =
                require_submission_baseline(pane_state_change_seq(&response, window, pane))?;
            wait_cli(
                window,
                pane,
                wait.expect("checked above"),
                Some(baseline),
                timeout,
                pretty,
            )
        }
        ControlCommand::Paste {
            window,
            pane,
            text,
            no_submit,
            wait,
        } => {
            let response = request_once(
                "pane.paste",
                json!({
                    "window_id": window,
                    "pane_id": pane,
                    "text": text,
                    "submit": !no_submit
                }),
                timeout,
            )?;
            if !response.ok || wait.is_none() {
                return print_response(&response, pretty);
            }
            let baseline =
                require_submission_baseline(pane_state_change_seq(&response, window, pane))?;
            wait_cli(
                window,
                pane,
                wait.expect("checked above"),
                Some(baseline),
                timeout,
                pretty,
            )
        }
        ControlCommand::Read {
            window,
            pane,
            lines,
        } => {
            let response = request_once(
                "pane.read",
                json!({ "window_id": window, "pane_id": pane, "lines": lines }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Procs { window, pane } => {
            let response = request_once(
                "pane.procs",
                json!({ "window_id": window, "pane_id": pane }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::SendKey {
            window,
            pane,
            key,
            shift,
            alt,
            control,
            repeat,
        } => {
            let response = request_once(
                "pane.send_key",
                json!({
                    "window_id": window,
                    "pane_id": pane,
                    "key": key,
                    "modifiers": { "shift": shift, "alt": alt, "control": control },
                    "repeat": repeat
                }),
                timeout,
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Run {
            window,
            pane,
            command,
            no_wait,
        } => {
            let response = request_once(
                "pane.run",
                json!({
                    "window_id": window,
                    "pane_id": pane,
                    "command": command,
                    "wait": !no_wait,
                    "timeout_ms": options.timeout_ms
                }),
                timeout.saturating_add(Duration::from_secs(1)),
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::ExecPane {
            window,
            pane,
            max_output_bytes,
            argv,
        } => {
            let response = request_once(
                "pane.exec",
                json!({
                    "window_id": window,
                    "pane_id": pane,
                    "argv": argv,
                    "timeout_ms": options.timeout_ms,
                    "max_output_bytes": max_output_bytes
                }),
                timeout.saturating_add(Duration::from_secs(2)),
            )?;
            print_response(&response, pretty)
        }
        ControlCommand::Wait {
            window,
            pane,
            state,
            after_seq,
        } => wait_cli(window, pane, state, after_seq, timeout, pretty),
    }
}

/// 从提交响应里取 pane 的跃迁计数:认 `result.snapshot` 与「result 即快照」
/// 两种形状;形状不对返回 `None`,由调用方明确报错而不是静默降级。
pub(super) fn snapshot_of(result: &Value) -> Option<RuntimeSnapshot> {
    let value = result.get("snapshot").unwrap_or(result);
    serde_json::from_value(value.clone()).ok()
}

fn pane_state_change_seq(
    response: &ApiResponse,
    window_id: Option<u64>,
    pane_id: u64,
) -> Option<u64> {
    let snapshot = snapshot_of(response.result.as_ref()?)?;
    snapshot
        .pane(window_id, pane_id)
        .ok()
        .map(|pane| pane.state_change_seq)
}

fn wait_cli(
    window: Option<u64>,
    pane: u64,
    state: ControlWaitState,
    after_seq: Option<u64>,
    timeout: Duration,
    pretty: bool,
) -> Result<(), Box<dyn Error>> {
    let response = request_once(
        "pane.wait",
        json!({
            "window_id": window,
            "pane_id": pane,
            "state": wait_state_name(state),
            "timeout_ms": timeout.as_millis() as u64,
            "after_seq": after_seq
        }),
        timeout.saturating_add(Duration::from_secs(1)),
    )?;
    print_response(&response, pretty)
}

/// 等待状态名的协议字面量(与 `RuntimeWaitState` 的 snake_case 单源)。
pub(super) fn wait_state_name(state: ControlWaitState) -> &'static str {
    match state {
        ControlWaitState::Idle => "idle",
        ControlWaitState::Running => "running",
        ControlWaitState::WaitingInput => "waiting_input",
        ControlWaitState::Attention => "attention",
        ControlWaitState::Finished => "finished",
        ControlWaitState::Failed => "failed",
        ControlWaitState::Settled => "settled",
    }
}

fn subscribe_cli(since: Option<u64>, timeout: Duration) -> Result<(), Box<dyn Error>> {
    let endpoint = read_endpoint()
        .ok_or_else(|| CliError::new("runtime_unavailable", "no running slterm runtime found"))?;
    let request = ApiRequest::new(
        endpoint.token.clone(),
        "events.subscribe",
        json!({ "since_revision": since }),
    );
    let stream = client_stream(&endpoint, &request, Some(timeout))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(CliError::new(
            "invalid_runtime_response",
            "runtime closed the subscription without an acknowledgement",
        )
        .into());
    }
    let mut output = std::io::stdout().lock();
    if !write_cli_output(&mut output, line.as_bytes())? {
        return Ok(());
    }
    reader.get_mut().set_read_timeout(None)?;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        if !write_cli_output(&mut output, line.as_bytes())? {
            return Ok(());
        }
    }
}

/// `agent.fork` 的 cwd/path 必须是绝对路径;相对输入按当前目录归一。
fn absolute_path(path: PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    std::path::absolute(&path).map_err(|error| {
        CliError::new(
            "invalid_params",
            format!("cannot resolve path {}: {error}", path.display()),
        )
        .into()
    })
}

/// CLI 域错误码 + 消息:与协议错误码同一命名面,便于脚本按 code 分支。
#[derive(Debug)]
pub(super) struct CliError {
    code: &'static str,
    message: String,
}

impl CliError {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CliError {}

/// 响应信封已经打印过(错误信封本身是输出),外层不再包第二层。
#[derive(Debug)]
pub(super) struct PrintedCliError(pub(super) String);

impl fmt::Display for PrintedCliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PrintedCliError {}

#[cfg(test)]
mod cli_client_tests {
    use super::*;
    use crate::cli::SHORT_TIMEOUT_MS;

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn plugin_response_checks_identity_and_envelope() {
        let request = ApiRequest::new(TEST_TOKEN.to_owned(), "runtime.describe", json!({}));
        let response = ApiResponse::success(&request.id, json!({"name":"中文"}));
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(
            decode_plugin_response(&request, &serde_json::to_vec(&value).unwrap()).unwrap(),
            response
        );
        let null = ApiResponse::success(&request.id, Value::Null);
        assert_eq!(
            decode_plugin_response(&request, &serde_json::to_vec(&null).unwrap()).unwrap(),
            null
        );
        let mut missing_result = value.clone();
        missing_result.as_object_mut().unwrap().remove("result");
        assert!(
            decode_plugin_response(&request, &serde_json::to_vec(&missing_result).unwrap())
                .is_err()
        );
        for (key, replacement) in [
            ("id", json!("other")),
            ("protocol", json!("other")),
            ("version", json!(2)),
            ("ok", json!(false)),
        ] {
            let mut invalid = value.clone();
            invalid[key] = replacement;
            assert!(
                decode_plugin_response(&request, &serde_json::to_vec(&invalid).unwrap()).is_err()
            );
        }
    }

    #[test]
    fn submission_missing_or_zero_baseline_cannot_wait_on_old_idle() {
        for baseline in [None, Some(0)] {
            let error = require_submission_baseline(baseline).unwrap_err();
            assert_eq!(error.code(), "submission_outcome_unknown");
            assert!(error.to_string().contains("do not resend automatically"));
        }
        assert_eq!(require_submission_baseline(Some(42)).unwrap(), 42);
    }

    #[test]
    fn submission_transport_failure_is_not_a_safe_retry_signal() {
        for method in [
            "pane.prompt",
            "pane.paste",
            "agent.prompt",
            "agent.paste",
            "agent.delegate",
        ] {
            let error = input_transport_error(
                method,
                std::io::Error::from(std::io::ErrorKind::TimedOut).into(),
            );
            assert_eq!(
                error.downcast_ref::<CliError>().unwrap().code(),
                "submission_outcome_unknown"
            );
            assert!(
                error
                    .to_string()
                    .contains("read its state and output before retrying")
            );
        }
        let error = input_transport_error(
            "pane.read",
            std::io::Error::from(std::io::ErrorKind::TimedOut).into(),
        );
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn json_output_keeps_compact_and_pretty_response_contracts() {
        let response = ApiResponse::success("probe", json!({ "tabs": 2 }));
        for pretty in [false, true] {
            let mut output = Vec::new();
            write_cli_response(&mut output, &response, pretty).unwrap();
            assert_eq!(output.last(), Some(&b'\n'));
            assert_eq!(
                serde_json::from_slice::<ApiResponse>(&output).unwrap(),
                response
            );
        }
    }

    #[test]
    fn closed_output_on_write_or_flush_ends_without_panicking() {
        use std::io;
        struct FailingOutput {
            error: std::io::Error,
            fail_flush: bool,
        }
        impl Write for FailingOutput {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.fail_flush {
                    Ok(bytes.len())
                } else {
                    Err(std::mem::replace(
                        &mut self.error,
                        io::Error::from(io::ErrorKind::Other),
                    ))
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(std::mem::replace(
                    &mut self.error,
                    io::Error::from(io::ErrorKind::Other),
                ))
            }
        }
        let response = ApiResponse::success("probe", json!({}));
        for fail_flush in [false, true] {
            let mut output = FailingOutput {
                error: io::Error::from(io::ErrorKind::BrokenPipe),
                fail_flush,
            };
            write_cli_response(&mut output, &response, false).unwrap();
        }
        let mut output = FailingOutput {
            error: io::Error::from(io::ErrorKind::BrokenPipe),
            fail_flush: false,
        };
        assert!(!write_cli_output(&mut output, b"event\n").unwrap());
        let mut output = FailingOutput {
            error: io::Error::from(io::ErrorKind::PermissionDenied),
            fail_flush: false,
        };
        assert_eq!(
            write_cli_output(&mut output, b"event\n")
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn wait_state_names_match_the_protocol_literals() {
        assert_eq!(wait_state_name(ControlWaitState::Idle), "idle");
        assert_eq!(wait_state_name(ControlWaitState::Running), "running");
        assert_eq!(
            wait_state_name(ControlWaitState::WaitingInput),
            "waiting_input"
        );
        assert_eq!(wait_state_name(ControlWaitState::Attention), "attention");
        assert_eq!(wait_state_name(ControlWaitState::Finished), "finished");
        assert_eq!(wait_state_name(ControlWaitState::Failed), "failed");
        assert_eq!(wait_state_name(ControlWaitState::Settled), "settled");
    }

    #[test]
    fn timeout_bounds_are_enforced() {
        assert!(validated_timeout(0).is_err());
        assert!(validated_timeout(MAX_WAIT.as_millis() as u64 + 1).is_err());
        assert!(validated_timeout(1).is_ok());
        assert_eq!(
            validated_timeout(SHORT_TIMEOUT_MS).unwrap().as_millis(),
            30_000
        );
    }
}
