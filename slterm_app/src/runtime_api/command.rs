//! Runtime 请求参数族、输入校验与方法路由(04 篇锚点)。
//!
//! 参数校验全部在 transport 侧、任何 UI 动作之前完成:`deny_unknown_fields`
//! 由各 `*Params` 结构承担;数值区间/字符边界/控制字符由本模块的
//! `validate_*` 函数承担。等待语义是一等公民:`after_seq` 基线 + 状态匹配
//! 共同决定命中,提交前的旧空闲态不能把「等 settled」判成已完成。

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use slterm_split::TabId;

use super::agent_api;
use super::git;
use super::orchestrate;
use super::{
    ApiError, ApiRequest, DEFAULT_EXEC_OUTPUT_BYTES, DEFAULT_READ_LINES, MAX_EXEC_OUTPUT_BYTES,
    MAX_KEY_REPEAT, MAX_PANE_RATIO, MAX_PROMPT_BYTES, MAX_READ_LINES, MAX_TAB_NAME_BYTES, MAX_WAIT,
    MIN_PANE_RATIO, RuntimeCommand, RuntimeKeyModifiers, RuntimePane, RuntimePaneProcesses,
    RuntimeProcess, RuntimeSplitDirection, RuntimeTaskState, RuntimeWaitState,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SubscribeParams {
    #[serde(default)]
    pub(super) since_revision: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaitParams {
    #[serde(default)]
    pub(super) window_id: Option<u64>,
    pub(super) pane_id: u64,
    pub(super) state: RuntimeWaitState,
    /// 1..=86_400_000;越界 `invalid_params`。
    pub(super) timeout_ms: u64,
    /// 提交时捕获的基线跃迁计数;缺席 = 旧语义(只看状态)。
    #[serde(default)]
    pub(super) after_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetParams {
    #[serde(default)]
    window_id: Option<u64>,
    #[serde(default)]
    pane_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowTargetParams {
    #[serde(default)]
    window_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewTabParams {
    #[serde(default)]
    window_id: Option<u64>,
    #[serde(default)]
    cwd: Option<std::path::PathBuf>,
    /// 与设置 `shell=` 同一套 id;缺省 = 默认 shell。
    #[serde(default)]
    shell: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TabTargetParams {
    #[serde(default)]
    window_id: Option<u64>,
    /// 窗口内零基索引;与稳定 `tab_id` 双轨定位。
    tab_index: usize,
    #[serde(default)]
    tab_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameTabParams {
    #[serde(default)]
    window_id: Option<u64>,
    tab_index: usize,
    #[serde(default)]
    tab_id: Option<u64>,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveTabParams {
    #[serde(default)]
    window_id: Option<u64>,
    tab_index: usize,
    #[serde(default)]
    tab_id: Option<u64>,
    to_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SplitParams {
    #[serde(default)]
    window_id: Option<u64>,
    #[serde(default)]
    pane_id: Option<u64>,
    direction: RuntimeSplitDirection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaneParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoomPaneParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    zoomed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResizePaneParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    ratio: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    text: String,
    #[serde(default = "default_true")]
    submit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PasteParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    text: String,
    #[serde(default = "default_true")]
    submit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    #[serde(default = "default_read_lines")]
    lines: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SendKeyParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    key: super::RuntimeKey,
    #[serde(default)]
    modifiers: RuntimeKeyModifiers,
    #[serde(default = "default_key_repeat")]
    repeat: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    command: String,
    #[serde(default = "default_true")]
    wait: bool,
    #[serde(default = "default_run_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecParams {
    #[serde(default)]
    window_id: Option<u64>,
    pane_id: u64,
    argv: Vec<String>,
    #[serde(default = "default_run_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_exec_output_bytes")]
    max_output_bytes: usize,
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_read_lines() -> usize {
    DEFAULT_READ_LINES
}

fn default_key_repeat() -> u16 {
    1
}

fn default_run_timeout_ms() -> u64 {
    super::COMMAND_TIMEOUT.as_millis() as u64
}

pub(super) fn default_command_timeout_ms() -> u64 {
    super::COMMAND_TIMEOUT.as_millis() as u64
}

fn default_exec_output_bytes() -> usize {
    DEFAULT_EXEC_OUTPUT_BYTES
}

/// 方法参数解析结果:读方法在 transport 线程直接消费,写方法转成
/// [`RuntimeCommand`] 交给同一派发原语。
#[derive(Debug)]
pub(super) enum ParsedMethod {
    Describe,
    Snapshot,
    Subscribe(SubscribeParams),
    Wait(WaitParams),
    AgentsList(agent_api::AgentsParams),
    AgentGet(agent_api::AgentTargetParams),
    AgentDelegate(agent_api::AgentDelegateParams),
    AgentWait(agent_api::AgentWaitParams),
    Orchestrate(Box<orchestrate::OrchestrateParams>),
    Command(Box<RuntimeCommand>),
}

/// 唯一路由表:makes `capabilities == Schema 方法枚举 == 可路由方法` 可测。
/// 未列出的方法一律 `method_not_found`(不静默成功)。
pub(super) fn parse_method(method: &str, params: &Value) -> Result<ParsedMethod, ApiError> {
    let parsed = match method {
        "runtime.describe" => {
            parse_empty_params(params)?;
            ParsedMethod::Describe
        }
        "runtime.snapshot" => {
            parse_empty_params(params)?;
            ParsedMethod::Snapshot
        }
        "events.subscribe" => ParsedMethod::Subscribe(parse_params(params)?),
        "pane.wait" => {
            let wait: WaitParams = parse_params(params)?;
            validate_wait_timeout(wait.timeout_ms)?;
            ParsedMethod::Wait(wait)
        }
        "agents.list" => ParsedMethod::AgentsList(parse_params(params)?),
        "agent.get" => {
            let target: agent_api::AgentTargetParams = parse_params(params)?;
            agent_api::validate_target_params(&target)?;
            ParsedMethod::AgentGet(target)
        }
        "agent.delegate" => {
            let delegate: agent_api::AgentDelegateParams = parse_params(params)?;
            agent_api::validate_delegate_params(&delegate)?;
            ParsedMethod::AgentDelegate(delegate)
        }
        "agent.wait" => {
            let wait: agent_api::AgentWaitParams = parse_params(params)?;
            agent_api::validate_wait_params(&wait)?;
            ParsedMethod::AgentWait(wait)
        }
        "runtime.orchestrate" => {
            ParsedMethod::Orchestrate(Box::new(orchestrate::parse_and_validate(params)?))
        }
        method => ParsedMethod::Command(Box::new(RuntimeCommand::from_params(method, params)?)),
    };
    Ok(parsed)
}

impl RuntimeCommand {
    /// 请求信封 → 写命令。设计锚点保留 `from_request` 形态;内部走
    /// [`Self::from_params`],避免编排步骤为了复用解析而伪造信封。
    pub(super) fn from_request(request: &ApiRequest) -> Result<Self, ApiError> {
        Self::from_params(&request.method, &request.params)
    }

    pub(super) fn from_params(method: &str, params: &Value) -> Result<Self, ApiError> {
        match method {
            "runtime.snapshot" => Ok(Self::Snapshot),
            "window.close" => {
                let params: WindowTargetParams = parse_params(params)?;
                Ok(Self::CloseWindow {
                    window_id: params.window_id,
                })
            }
            "window.focus" => {
                let params: TargetParams = parse_params(params)?;
                Ok(Self::Focus {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                })
            }
            "tab.new" => {
                let params: NewTabParams = parse_params(params)?;
                Ok(Self::NewTab {
                    window_id: params.window_id,
                    cwd: params.cwd,
                    shell_id: params.shell,
                })
            }
            "tab.close" => {
                let params: TabTargetParams = parse_params(params)?;
                Ok(Self::CloseTab {
                    window_id: params.window_id,
                    tab_index: params.tab_index,
                    tab_id: params.tab_id.map(TabId),
                })
            }
            "tab.rename" => {
                let params: RenameTabParams = parse_params(params)?;
                validate_tab_name(&params.name)?;
                Ok(Self::RenameTab {
                    window_id: params.window_id,
                    tab_index: params.tab_index,
                    tab_id: params.tab_id.map(TabId),
                    name: params.name,
                })
            }
            "tab.move" => {
                let params: MoveTabParams = parse_params(params)?;
                Ok(Self::MoveTab {
                    window_id: params.window_id,
                    tab_index: params.tab_index,
                    tab_id: params.tab_id.map(TabId),
                    to_index: params.to_index,
                })
            }
            "pane.split" => {
                let params: SplitParams = parse_params(params)?;
                Ok(Self::Split {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    direction: params.direction,
                })
            }
            "pane.close" => {
                let params: PaneParams = parse_params(params)?;
                Ok(Self::ClosePane {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                })
            }
            "pane.zoom" => {
                let params: ZoomPaneParams = parse_params(params)?;
                Ok(Self::ZoomPane {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    zoomed: params.zoomed,
                })
            }
            "pane.resize" => {
                let params: ResizePaneParams = parse_params(params)?;
                if !params.ratio.is_finite()
                    || !(MIN_PANE_RATIO..=MAX_PANE_RATIO).contains(&params.ratio)
                {
                    return Err(ApiError::invalid_params(format!(
                        "ratio must be between {MIN_PANE_RATIO} and {MAX_PANE_RATIO}"
                    )));
                }
                Ok(Self::ResizePane {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    ratio: params.ratio,
                })
            }
            "pane.prompt" => {
                let params: PromptParams = parse_params(params)?;
                validate_prompt(&params.text)?;
                Ok(Self::Prompt {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    text: params.text,
                    submit: params.submit,
                })
            }
            "pane.paste" => {
                let params: PasteParams = parse_params(params)?;
                validate_paste_text(&params.text)?;
                Ok(Self::Paste {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    text: params.text,
                    submit: params.submit,
                })
            }
            "pane.read" => {
                let params: ReadParams = parse_params(params)?;
                if params.lines == 0 || params.lines > MAX_READ_LINES {
                    return Err(ApiError::invalid_params(format!(
                        "lines must be between 1 and {MAX_READ_LINES}"
                    )));
                }
                Ok(Self::ReadPane {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    lines: params.lines,
                })
            }
            "pane.procs" => {
                let params: PaneParams = parse_params(params)?;
                Ok(Self::Procs {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                })
            }
            "pane.send_key" => {
                let params: SendKeyParams = parse_params(params)?;
                if params.repeat == 0 || params.repeat > MAX_KEY_REPEAT {
                    return Err(ApiError::invalid_params(format!(
                        "repeat must be between 1 and {MAX_KEY_REPEAT}"
                    )));
                }
                if params.key.letter().is_some() && !params.modifiers.control {
                    return Err(ApiError::invalid_params(
                        "letter keys require control=true; use pane.prompt for printable text",
                    ));
                }
                Ok(Self::SendKey {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    key: params.key,
                    modifiers: params.modifiers,
                    repeat: params.repeat,
                })
            }
            "pane.run" => {
                let params: RunParams = parse_params(params)?;
                validate_command_line(&params.command)?;
                validate_wait_timeout(params.timeout_ms)?;
                Ok(Self::Run {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    command: params.command,
                    wait: params.wait,
                    timeout_ms: params.timeout_ms,
                })
            }
            "pane.exec" => {
                let params: ExecParams = parse_params(params)?;
                validate_exec_argv(&params.argv)?;
                validate_wait_timeout(params.timeout_ms)?;
                if !(1..=MAX_EXEC_OUTPUT_BYTES).contains(&params.max_output_bytes) {
                    return Err(ApiError::invalid_params(format!(
                        "max_output_bytes must be between 1 and {MAX_EXEC_OUTPUT_BYTES}"
                    )));
                }
                Ok(Self::Exec {
                    window_id: params.window_id,
                    pane_id: params.pane_id,
                    argv: params.argv,
                    timeout_ms: params.timeout_ms,
                    max_output_bytes: params.max_output_bytes,
                })
            }
            method if method.starts_with("agent.") => {
                agent_api::command_from_method(method, params)
            }
            method if method.starts_with("git.") => git::command_from_method(method, params),
            method => Err(ApiError::new(
                "method_not_found",
                format!("runtime API method {method:?} does not exist"),
            )),
        }
    }
}

fn parse_empty_params(value: &Value) -> Result<(), ApiError> {
    match value.as_object() {
        Some(object) if object.is_empty() => Ok(()),
        Some(_) => Err(ApiError::invalid_params(
            "this method does not accept parameters",
        )),
        None => Err(ApiError::invalid_params("params must be an object")),
    }
}

pub(super) fn validate_wait_timeout(timeout_ms: u64) -> Result<(), ApiError> {
    if timeout_ms == 0 || Duration::from_millis(timeout_ms) > MAX_WAIT {
        Err(ApiError::invalid_params(
            "timeout_ms must be between 1 and 86400000",
        ))
    } else {
        Ok(())
    }
}

#[allow(dead_code)] // 消费点 = pane.procs 的 UI 执行器(M7.5 接线)。
pub(crate) fn capture_process_tree(
    window_id: u64,
    pane_id: u64,
    root_pid: u32,
) -> Result<RuntimePaneProcesses, ApiError> {
    let entries = crate::process_tree::descendants(root_pid).map_err(|message| {
        ApiError::new(
            "process_query_failed",
            "failed to read the pane process tree",
        )
        .details(serde_json::json!({ "root_pid": root_pid, "reason": message }))
    })?;
    let processes = entries
        .into_iter()
        .map(|entry| {
            let display_name = crate::process_tree::display_name(&entry.executable);
            let agent_kind = crate::ai_agents::AgentKind::parse(&display_name)
                .map(|kind| kind.slug().to_owned());
            RuntimeProcess {
                pid: entry.pid,
                parent_pid: (entry.pid != root_pid).then_some(entry.parent_pid),
                display_name,
                executable: entry.executable,
                depth: entry.depth,
                agent_kind,
            }
        })
        .collect();
    Ok(RuntimePaneProcesses {
        window_id,
        pane_id,
        root_pid,
        processes,
    })
}

pub(super) fn parse_params<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    // 协议层 params 恒为对象;先拦非对象,避免 serde 对「全默认字段结构」
    // 的宽松形态把数组/null 静默当成空参数。
    if !value.is_object() {
        return Err(ApiError::invalid_params("params must be an object"));
    }
    serde_json::from_value(value.clone())
        .map_err(|error| ApiError::invalid_params(format!("invalid method parameters: {error}")))
}

fn validate_tab_name(name: &str) -> Result<(), ApiError> {
    if name.len() > MAX_TAB_NAME_BYTES {
        return Err(ApiError::invalid_params(format!(
            "tab name exceeds the {MAX_TAB_NAME_BYTES}-byte limit"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "tab name contains control characters",
        ));
    }
    Ok(())
}

/// prompt 是单行纯文本:非空、≤32KiB、禁一切控制字符。
pub(crate) fn validate_prompt(text: &str) -> Result<(), ApiError> {
    if text.is_empty() {
        return Err(ApiError::invalid_params("prompt text must not be empty"));
    }
    if text.len() > MAX_PROMPT_BYTES {
        return Err(ApiError::invalid_params(format!(
            "prompt text exceeds the {MAX_PROMPT_BYTES}-byte limit"
        )));
    }
    if text.chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "prompt text contains control characters; pane.prompt accepts one plain-text line",
        ));
    }
    Ok(())
}

/// 内部多行文本(chat message):保留换行,但仍不能把 ESC/NUL 送进终端。
#[allow(dead_code)] // 消费点 = 03/08 篇内部 send-to-chat 链(M8 接线)。
pub(crate) fn validate_chat_message(text: &str) -> Result<(), ApiError> {
    validate_multiline_text(text, "chat message")
}

/// paste 保留 CR/LF/TAB 作为 bracketed-paste 文本内容,拒绝其余控制字符。
pub(crate) fn validate_paste_text(text: &str) -> Result<(), ApiError> {
    validate_multiline_text(text, "paste text")
}

fn validate_multiline_text(text: &str, label: &str) -> Result<(), ApiError> {
    if text.trim().is_empty() {
        return Err(ApiError::invalid_params(format!(
            "{label} must not be empty"
        )));
    }
    if text.len() > MAX_PROMPT_BYTES {
        return Err(ApiError::invalid_params(format!(
            "{label} exceeds the {MAX_PROMPT_BYTES}-byte limit"
        )));
    }
    if text
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(ApiError::invalid_params(format!(
            "{label} contains unsupported terminal control characters"
        )));
    }
    Ok(())
}

/// pane.run 只接受单行 shell 命令。
pub(crate) fn validate_command_line(command: &str) -> Result<(), ApiError> {
    if command.trim().is_empty() {
        return Err(ApiError::invalid_params("command must not be empty"));
    }
    if command.len() > MAX_PROMPT_BYTES {
        return Err(ApiError::invalid_params(format!(
            "command exceeds the {MAX_PROMPT_BYTES}-byte limit"
        )));
    }
    if command.chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "command contains control characters; pane.run accepts one plain-text shell line",
        ));
    }
    Ok(())
}

fn validate_exec_argv(argv: &[String]) -> Result<(), ApiError> {
    if argv.is_empty() || argv[0].trim().is_empty() {
        return Err(ApiError::invalid_params(
            "argv must contain a non-empty program",
        ));
    }
    if argv.len() > 256 {
        return Err(ApiError::invalid_params(
            "argv may contain at most 256 elements",
        ));
    }
    let bytes = argv.iter().try_fold(0_usize, |total, arg| {
        total
            .checked_add(arg.len())
            .and_then(|value| value.checked_add(1))
    });
    if bytes.is_none_or(|bytes| bytes > MAX_PROMPT_BYTES) {
        return Err(ApiError::invalid_params(format!(
            "argv exceeds the {MAX_PROMPT_BYTES}-byte limit"
        )));
    }
    if argv.iter().any(|arg| arg.contains('\0')) {
        return Err(ApiError::invalid_params("argv must not contain NUL bytes"));
    }
    if argv[0].chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "argv program must not contain control characters",
        ));
    }
    Ok(())
}

pub(super) fn validate_agent_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(ApiError::invalid_params(
            "agent name must contain between 1 and 64 bytes",
        ));
    }
    if trimmed != name || name.chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "agent name must not have surrounding whitespace or control characters",
        ));
    }
    Ok(())
}

pub(super) fn validate_agent_selector(agent: &str) -> Result<(), ApiError> {
    if agent.trim().is_empty() || agent.len() > 128 || agent.chars().any(char::is_control) {
        return Err(ApiError::invalid_params("agent selector is invalid"));
    }
    Ok(())
}

/// 命中 = 状态匹配 且(带基线时)seq > after_seq。否则对本来就 idle 的
/// pane 等 idle 会立即返回,调用方误以为提交的工作已完成。
pub(super) fn wait_matches(
    pane: &RuntimePane,
    expected: RuntimeWaitState,
    after_seq: Option<u64>,
) -> bool {
    after_seq.is_none_or(|baseline| pane.state_change_seq > baseline)
        && wait_state_matches(pane.task_state, expected)
}

pub(super) fn wait_state_matches(actual: RuntimeTaskState, expected: RuntimeWaitState) -> bool {
    match expected {
        RuntimeWaitState::Idle => actual == RuntimeTaskState::Idle,
        RuntimeWaitState::Running => actual == RuntimeTaskState::Running,
        RuntimeWaitState::WaitingInput => actual == RuntimeTaskState::WaitingInput,
        RuntimeWaitState::Attention => actual == RuntimeTaskState::Attention,
        RuntimeWaitState::Finished => actual == RuntimeTaskState::Finished,
        RuntimeWaitState::Failed => actual == RuntimeTaskState::Failed,
        // settled ≠ 成功:只表示不在运行。
        RuntimeWaitState::Settled => actual != RuntimeTaskState::Running,
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    fn pane(state: RuntimeTaskState, seq: u64) -> RuntimePane {
        RuntimePane {
            id: 1,
            active: false,
            title: String::new(),
            cwd: String::new(),
            branch: String::new(),
            running_program: None,
            agent: None,
            task_state: state,
            state_change_seq: seq,
            active_run: None,
            last_run: None,
        }
    }

    #[test]
    fn wait_matches_requires_the_state_and_the_baseline() {
        let idle = pane(RuntimeTaskState::Idle, 3);
        assert!(wait_matches(&idle, RuntimeWaitState::Idle, None));
        assert!(wait_matches(&idle, RuntimeWaitState::Idle, Some(2)));
        // 基线已追上:即便状态匹配也不得命中。
        assert!(!wait_matches(&idle, RuntimeWaitState::Idle, Some(3)));
        assert!(!wait_matches(&idle, RuntimeWaitState::Running, None));
    }

    #[test]
    fn settled_matches_every_non_running_state() {
        for state in [
            RuntimeTaskState::Idle,
            RuntimeTaskState::WaitingInput,
            RuntimeTaskState::Attention,
            RuntimeTaskState::Finished,
            RuntimeTaskState::Failed,
        ] {
            assert!(
                wait_state_matches(state, RuntimeWaitState::Settled),
                "{state:?}"
            );
        }
        assert!(!wait_state_matches(
            RuntimeTaskState::Running,
            RuntimeWaitState::Settled
        ));
    }

    #[test]
    fn wait_params_reject_unknown_fields() {
        let raw = serde_json::json!({
            "pane_id": 1,
            "state": "idle",
            "timeout_ms": 1000,
            "oracle": true,
        });
        let error = parse_params::<WaitParams>(&raw).expect_err("unknown field");
        assert_eq!(error.code, "invalid_params");
    }

    #[test]
    fn subscribe_params_default_to_no_since_revision() {
        let params: SubscribeParams =
            parse_params(&serde_json::json!({})).expect("empty subscribe params");
        assert_eq!(params.since_revision, None);
    }

    #[test]
    fn prompt_validation_rejects_empty_control_and_oversize() {
        assert!(validate_prompt("please inspect the build").is_ok());
        assert!(validate_prompt("").is_err());
        assert!(validate_prompt("unsafe\u{1b}[2J").is_err());
        assert!(validate_prompt("two\nlines").is_err());
        assert!(validate_prompt(&"x".repeat(MAX_PROMPT_BYTES + 1)).is_err());
    }

    #[test]
    fn paste_validation_keeps_crlf_and_tab_but_rejects_other_controls() {
        assert!(validate_paste_text("first\r\nsecond\tvalue").is_ok());
        assert!(validate_paste_text("unsafe\u{1b}[201~tail").is_err());
        assert!(validate_paste_text("\0binary").is_err());
        assert!(validate_paste_text(" \n\t ").is_err());
        assert!(validate_chat_message("> selected\n\nplease explain\tthis").is_ok());
        assert!(validate_chat_message("unsafe\u{1b}[2J").is_err());
    }

    #[test]
    fn command_and_argv_validation_boundaries_hold() {
        assert!(validate_command_line("cargo test").is_ok());
        assert!(validate_command_line("").is_err());
        assert!(validate_command_line("two\nlines").is_err());
        assert!(validate_exec_argv(&["cmd.exe".into(), "/C".into(), "echo hi".into()]).is_ok());
        assert!(validate_exec_argv(&[]).is_err());
        assert!(validate_exec_argv(&["".into()]).is_err());
        assert!(validate_exec_argv(&["cmd\0.exe".into()]).is_err());
        assert!(validate_exec_argv(&["x\0".into()]).is_err());
        let oversized: Vec<String> = (0..257).map(|_| "a".to_owned()).collect();
        assert!(validate_exec_argv(&oversized).is_err());
    }

    #[test]
    fn agent_name_and_selector_validation_rejects_identity_noise() {
        assert!(validate_agent_name("worker").is_ok());
        assert!(validate_agent_name(" worker").is_err());
        assert!(validate_agent_name("").is_err());
        assert!(validate_agent_name("bad\u{7}").is_err());
        assert!(validate_agent_name(&"a".repeat(65)).is_err());
        assert!(validate_agent_selector("worker").is_ok());
        assert!(validate_agent_selector("bad\u{7}").is_err());
    }

    /// 全表正反向:capabilities 中每个方法都必须有可解析的参数夹具;
    /// 缺失必填、多余字段、类型错、越界四类变异必须得到 `invalid_params`
    /// (`method_not_found` 只留给真正不存在的方法)。
    #[test]
    fn every_capability_method_validates_missing_extra_type_and_range() {
        struct Case {
            method: &'static str,
            fixture: Value,
            required: &'static [&'static str],
            out_of_range: Option<(&'static str, Value)>,
        }

        let revision = "0".repeat(64);
        let cases = vec![
            Case {
                method: "runtime.describe",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "runtime.snapshot",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "runtime.orchestrate",
                fixture: serde_json::json!({"steps":[{"op":"new_tab","id":"tab"}]}),
                required: &["steps"],
                out_of_range: None,
            },
            Case {
                method: "events.subscribe",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "agents.list",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "agent.start",
                fixture: serde_json::json!({"name":"worker","kind":"claude"}),
                required: &["name", "kind"],
                out_of_range: Some((
                    "name",
                    serde_json::json!({"name":"a".repeat(65),"kind":"claude"}),
                )),
            },
            Case {
                method: "agent.fork",
                fixture: serde_json::json!({"name":"worker","kind":"claude","source_cwd":"D:/work"}),
                required: &["name", "kind"],
                out_of_range: Some((
                    "name",
                    serde_json::json!({"name":"a".repeat(65),"kind":"claude","source_cwd":"D:/work"}),
                )),
            },
            Case {
                method: "agent.get",
                fixture: serde_json::json!({"agent":"worker"}),
                required: &["agent"],
                out_of_range: Some(("agent", serde_json::json!({"agent":"x".repeat(129)}))),
            },
            Case {
                method: "agent.delegate",
                fixture: serde_json::json!({"agent":"worker","text":"hello","origin_pane_id":1}),
                required: &["agent", "text", "origin_pane_id"],
                out_of_range: Some((
                    "text",
                    serde_json::json!({"agent":"worker","text":"x".repeat(MAX_PROMPT_BYTES + 1),"origin_pane_id":1}),
                )),
            },
            Case {
                method: "agent.prompt",
                fixture: serde_json::json!({"agent":"worker","text":"hello"}),
                required: &["agent", "text"],
                out_of_range: Some((
                    "text",
                    serde_json::json!({"agent":"worker","text":"x".repeat(MAX_PROMPT_BYTES + 1)}),
                )),
            },
            Case {
                method: "agent.paste",
                fixture: serde_json::json!({"agent":"worker","text":"hello"}),
                required: &["agent", "text"],
                out_of_range: Some((
                    "text",
                    serde_json::json!({"agent":"worker","text":"x".repeat(MAX_PROMPT_BYTES + 1)}),
                )),
            },
            Case {
                method: "agent.read",
                fixture: serde_json::json!({"agent":"worker"}),
                required: &["agent"],
                out_of_range: Some((
                    "lines",
                    serde_json::json!({"agent":"worker","lines":MAX_READ_LINES + 1}),
                )),
            },
            Case {
                method: "agent.wait",
                fixture: serde_json::json!({"agent":"worker","generation":1,"state":"idle","timeout_ms":100}),
                required: &["agent", "generation", "state", "timeout_ms"],
                out_of_range: Some((
                    "timeout_ms",
                    serde_json::json!({"agent":"worker","generation":1,"state":"idle","timeout_ms":0}),
                )),
            },
            Case {
                method: "window.close",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "window.focus",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "tab.new",
                fixture: serde_json::json!({}),
                required: &[],
                out_of_range: None,
            },
            Case {
                method: "tab.close",
                fixture: serde_json::json!({"tab_index":0}),
                required: &["tab_index"],
                out_of_range: Some(("tab_index", serde_json::json!({"tab_index":-1}))),
            },
            Case {
                method: "tab.rename",
                fixture: serde_json::json!({"tab_index":0,"name":"x"}),
                required: &["tab_index", "name"],
                out_of_range: Some((
                    "name",
                    serde_json::json!({"tab_index":0,"name":"x".repeat(MAX_TAB_NAME_BYTES + 1)}),
                )),
            },
            Case {
                method: "tab.move",
                fixture: serde_json::json!({"tab_index":0,"to_index":1}),
                required: &["tab_index", "to_index"],
                out_of_range: Some(("to_index", serde_json::json!({"tab_index":0,"to_index":-1}))),
            },
            Case {
                method: "pane.split",
                fixture: serde_json::json!({"direction":"left_right"}),
                required: &["direction"],
                out_of_range: Some(("direction", serde_json::json!({"direction":"diagonal"}))),
            },
            Case {
                method: "pane.close",
                fixture: serde_json::json!({"pane_id":1}),
                required: &["pane_id"],
                out_of_range: Some(("pane_id", serde_json::json!({"pane_id":"one"}))),
            },
            Case {
                method: "pane.zoom",
                fixture: serde_json::json!({"pane_id":1,"zoomed":true}),
                required: &["pane_id", "zoomed"],
                out_of_range: Some(("zoomed", serde_json::json!({"pane_id":1,"zoomed":"yes"}))),
            },
            Case {
                method: "pane.resize",
                fixture: serde_json::json!({"pane_id":1,"ratio":0.5}),
                required: &["pane_id", "ratio"],
                out_of_range: Some(("ratio", serde_json::json!({"pane_id":1,"ratio":0.96}))),
            },
            Case {
                method: "pane.prompt",
                fixture: serde_json::json!({"pane_id":1,"text":"hello"}),
                required: &["pane_id", "text"],
                out_of_range: Some((
                    "text",
                    serde_json::json!({"pane_id":1,"text":"x".repeat(MAX_PROMPT_BYTES + 1)}),
                )),
            },
            Case {
                method: "pane.paste",
                fixture: serde_json::json!({"pane_id":1,"text":"hello"}),
                required: &["pane_id", "text"],
                out_of_range: Some((
                    "text",
                    serde_json::json!({"pane_id":1,"text":"x".repeat(MAX_PROMPT_BYTES + 1)}),
                )),
            },
            Case {
                method: "pane.read",
                fixture: serde_json::json!({"pane_id":1}),
                required: &["pane_id"],
                out_of_range: Some((
                    "lines",
                    serde_json::json!({"pane_id":1,"lines":MAX_READ_LINES + 1}),
                )),
            },
            Case {
                method: "pane.procs",
                fixture: serde_json::json!({"pane_id":1}),
                required: &["pane_id"],
                out_of_range: Some(("pane_id", serde_json::json!({"pane_id":"one"}))),
            },
            Case {
                method: "pane.send_key",
                fixture: serde_json::json!({"pane_id":1,"key":"enter"}),
                required: &["pane_id", "key"],
                out_of_range: Some((
                    "repeat",
                    serde_json::json!({"pane_id":1,"key":"enter","repeat":0}),
                )),
            },
            Case {
                method: "pane.run",
                fixture: serde_json::json!({"pane_id":1,"command":"dir"}),
                required: &["pane_id", "command"],
                out_of_range: Some((
                    "timeout_ms",
                    serde_json::json!({"pane_id":1,"command":"dir","timeout_ms":0}),
                )),
            },
            Case {
                method: "pane.exec",
                fixture: serde_json::json!({"pane_id":1,"argv":["cmd.exe","/C","echo hi"]}),
                required: &["pane_id", "argv"],
                out_of_range: Some((
                    "max_output_bytes",
                    serde_json::json!({"pane_id":1,"argv":["cmd.exe"],"max_output_bytes":0}),
                )),
            },
            Case {
                method: "pane.wait",
                fixture: serde_json::json!({"pane_id":1,"state":"idle","timeout_ms":100}),
                required: &["pane_id", "state", "timeout_ms"],
                out_of_range: Some((
                    "timeout_ms",
                    serde_json::json!({"pane_id":1,"state":"idle","timeout_ms":0}),
                )),
            },
            Case {
                method: "git.status",
                fixture: serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work"}),
                required: &["window_id", "pane_id", "expected_cwd"],
                out_of_range: Some((
                    "expected_cwd",
                    serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"x".repeat(4097)}),
                )),
            },
            Case {
                method: "git.diff",
                fixture: serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"a.txt"}),
                required: &["window_id", "pane_id", "expected_cwd", "path"],
                out_of_range: Some((
                    "path",
                    serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"x".repeat(4097)}),
                )),
            },
            Case {
                method: "git.rollback",
                fixture: serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"a.txt","revision":revision}),
                required: &["window_id", "pane_id", "expected_cwd", "path", "revision"],
                out_of_range: Some((
                    "revision",
                    serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"a.txt","revision":"not-a-revision"}),
                )),
            },
            Case {
                method: "git.unstage",
                fixture: serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"a.txt","revision":revision}),
                required: &["window_id", "pane_id", "expected_cwd", "path", "revision"],
                out_of_range: Some((
                    "revision",
                    serde_json::json!({"window_id":1,"pane_id":1,"expected_cwd":"D:/work","path":"a.txt","revision":"not-a-revision"}),
                )),
            },
        ];
        for case in &cases {
            let method = case.method;
            assert!(
                parse_method(method, &case.fixture).is_ok(),
                "{method}: valid fixture was rejected: {:?}",
                parse_method(method, &case.fixture).err()
            );

            for field in case.required {
                let mut missing = case.fixture.as_object().expect("object fixture").clone();
                missing.remove(*field);
                let error = parse_method(method, &Value::Object(missing))
                    .err()
                    .unwrap_or_else(|| panic!("{method}: missing {field} must fail"));
                assert_eq!(error.code, "invalid_params", "{method}: missing {field}");
            }

            let mut extra = case.fixture.as_object().expect("object fixture").clone();
            extra.insert("oracle".to_owned(), Value::Bool(true));
            let error = parse_method(method, &Value::Object(extra))
                .err()
                .unwrap_or_else(|| panic!("{method}: unknown field must fail"));
            assert_eq!(error.code, "invalid_params", "{method}: unknown field");

            // 类型错:把第一个必填字段替换成错类型(空必填表的方法用
            // `params` 数组整体拒)。
            if let Some(field) = case.required.first() {
                let mut wrong_type = case.fixture.as_object().expect("object fixture").clone();
                let original = wrong_type.get(*field).cloned().unwrap_or(Value::Null);
                let replacement = match original {
                    Value::String(_) => Value::Bool(true),
                    Value::Number(_) => Value::String("not-a-number".into()),
                    Value::Bool(_) => Value::String("not-a-bool".into()),
                    _ => Value::Null,
                };
                wrong_type.insert((*field).to_owned(), replacement);
                let error = parse_method(method, &Value::Object(wrong_type))
                    .err()
                    .unwrap_or_else(|| panic!("{method}: wrong type for {field} must fail"));
                assert_eq!(error.code, "invalid_params", "{method}: wrong type {field}");
            } else {
                let error = parse_method(method, &Value::Array(Vec::new()))
                    .err()
                    .unwrap_or_else(|| panic!("{method}: non-object params must fail"));
                assert_eq!(error.code, "invalid_params", "{method}: non-object params");
            }

            if let Some((label, mutated)) = &case.out_of_range {
                let error = parse_method(method, mutated)
                    .err()
                    .unwrap_or_else(|| panic!("{method}: out-of-range {label} must fail"));
                assert_eq!(error.code, "invalid_params", "{method}: range {label}");
            }
        }
    }

    #[test]
    fn trimmed_and_unknown_methods_are_not_routable() {
        for method in [
            "window.create",
            "git.history",
            "git.stage",
            "git.commit",
            "git.fetch",
            "git.pull",
            "git.push",
            "tab.focus",
            "tab.open",
            "tab.read",
            "conversation.read",
            "conversation.send",
            "conversation.choose",
            "conversation.key",
        ] {
            let error = parse_method(method, &serde_json::json!({}))
                .expect_err("trimmed method must not route");
            assert_eq!(error.code, "method_not_found", "{method}");
        }
    }
}
