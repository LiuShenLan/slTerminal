//! 命名 Agent 方法族:解析、查询、等待与事务型 worktree 准备。
//!
//! `agent.start` / `agent.fork` 的冷启动命令只放官方文档核实过拼法的 CLI;
//! identity 全程 generation 绑定:退出重开不会把任务投给新会话。`agent.fork`
//! 的 worktree 事务在 transport 线程预建,UI 派发失败回滚、超时保留 checkout
//! (`cleanup_deferred`)。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use super::command::{
    default_read_lines, default_true, parse_params, validate_agent_name, validate_agent_selector,
    validate_paste_text, validate_prompt, validate_wait_timeout, wait_matches,
};
use super::transport::{dispatch_runtime_command, write_response};
use super::{
    ApiError, ApiRequest, ApiResponse, EventSink, MAX_READ_LINES, RuntimeCommand, RuntimeHub,
    RuntimeWaitState,
};
use crate::ai_agents::{AgentKind, launchable_kinds};
use crate::git_worktree::{WorktreeProvenance, WorktreeRequest, WorktreeTransaction};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentsParams {
    #[serde(default)]
    pub(super) window_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentTargetParams {
    pub(super) agent: String,
    #[serde(default)]
    pub(super) generation: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentDelegateParams {
    pub(super) agent: String,
    #[serde(default)]
    pub(super) generation: Option<u64>,
    pub(super) text: String,
    pub(super) origin_pane_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentWaitParams {
    pub(super) agent: String,
    pub(super) generation: u64,
    pub(super) state: RuntimeWaitState,
    pub(super) timeout_ms: u64,
    #[serde(default)]
    pub(super) after_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentStartParams {
    #[serde(default)]
    window_id: Option<u64>,
    #[serde(default)]
    pane_id: Option<u64>,
    name: String,
    kind: String,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default)]
    resume_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentForkParams {
    #[serde(default)]
    window_id: Option<u64>,
    #[serde(default)]
    source_pane_id: Option<u64>,
    #[serde(default)]
    source_cwd: Option<PathBuf>,
    name: String,
    kind: String,
    #[serde(default)]
    resume_session_id: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    allow_dirty_source: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentPromptParams {
    agent: String,
    #[serde(default)]
    generation: Option<u64>,
    text: String,
    #[serde(default = "default_true")]
    submit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentPasteParams {
    agent: String,
    #[serde(default)]
    generation: Option<u64>,
    text: String,
    #[serde(default = "default_true")]
    submit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentReadParams {
    agent: String,
    #[serde(default)]
    generation: Option<u64>,
    #[serde(default = "default_read_lines")]
    lines: usize,
}

/// agent 方法路由。只处理 `agent.start/fork/prompt/paste/read`;其余
/// `agent.*`(get/delegate/wait 是专链)由 transport 路由表接管。
pub(super) fn command_from_method(
    method: &str,
    params: &Value,
) -> Result<RuntimeCommand, ApiError> {
    match method {
        "agent.start" => {
            let params: AgentStartParams = parse_params(params)?;
            validate_agent_name(&params.name)?;
            if params.pane_id.is_some() && params.cwd.is_some() {
                return Err(ApiError::invalid_params(
                    "agent.start cannot combine pane_id with cwd; an existing pane keeps its shell cwd",
                ));
            }
            let (kind, session_id, command) =
                verified_agent_launch(&params.kind, params.resume_session_id)?;
            Ok(RuntimeCommand::AgentStart {
                window_id: params.window_id,
                pane_id: params.pane_id,
                name: params.name,
                kind,
                cwd: params.cwd,
                session_id,
                command,
                worktree: None,
            })
        }
        "agent.fork" => {
            let params: AgentForkParams = parse_params(params)?;
            validate_agent_name(&params.name)?;
            if params.source_pane_id.is_none() && params.source_cwd.is_none() {
                return Err(ApiError::invalid_params(
                    "source_pane_id or source_cwd is required",
                ));
            }
            let (kind, session_id, command) =
                verified_agent_launch(&params.kind, params.resume_session_id)?;
            Ok(RuntimeCommand::AgentFork {
                window_id: params.window_id,
                source_pane_id: params.source_pane_id,
                source_cwd: params.source_cwd,
                name: params.name,
                kind,
                session_id,
                command,
                branch: params.branch,
                base: params.base,
                path: params.path,
                allow_dirty_source: params.allow_dirty_source,
            })
        }
        "agent.prompt" => {
            let params: AgentPromptParams = parse_params(params)?;
            validate_agent_selector(&params.agent)?;
            validate_generation(params.generation)?;
            validate_prompt(&params.text)?;
            Ok(RuntimeCommand::AgentPrompt {
                agent: params.agent,
                generation: params.generation,
                text: params.text,
                submit: params.submit,
            })
        }
        "agent.paste" => {
            let params: AgentPasteParams = parse_params(params)?;
            validate_agent_selector(&params.agent)?;
            validate_generation(params.generation)?;
            validate_paste_text(&params.text)?;
            Ok(RuntimeCommand::AgentPaste {
                agent: params.agent,
                generation: params.generation,
                text: params.text,
                submit: params.submit,
            })
        }
        "agent.read" => {
            let params: AgentReadParams = parse_params(params)?;
            validate_agent_selector(&params.agent)?;
            validate_generation(params.generation)?;
            if params.lines == 0 || params.lines > MAX_READ_LINES {
                return Err(ApiError::invalid_params(format!(
                    "lines must be between 1 and {MAX_READ_LINES}"
                )));
            }
            Ok(RuntimeCommand::AgentRead {
                agent: params.agent,
                generation: params.generation,
                lines: params.lines,
            })
        }
        method => Err(ApiError::new(
            "method_not_found",
            format!("runtime API method {method:?} does not exist"),
        )),
    }
}

/// generation 从 1 起;0 是非法基线。
pub(super) fn validate_generation(generation: Option<u64>) -> Result<(), ApiError> {
    if generation == Some(0) {
        Err(ApiError::invalid_params(
            "generation must be a positive managed-agent generation",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_target_params(params: &AgentTargetParams) -> Result<(), ApiError> {
    validate_agent_selector(&params.agent)?;
    validate_generation(params.generation)
}

pub(super) fn validate_delegate_params(params: &AgentDelegateParams) -> Result<(), ApiError> {
    validate_agent_selector(&params.agent)?;
    validate_generation(params.generation)?;
    validate_prompt(&params.text)
}

pub(super) fn validate_wait_params(params: &AgentWaitParams) -> Result<(), ApiError> {
    validate_agent_selector(&params.agent)?;
    validate_generation(Some(params.generation))?;
    validate_wait_timeout(params.timeout_ms)
}

fn verified_agent_launch(
    kind: &str,
    resume_session_id: Option<String>,
) -> Result<(AgentKind, Option<String>, String), ApiError> {
    let kind = AgentKind::parse(kind)
        .ok_or_else(|| ApiError::invalid_params(format!("unknown agent kind {kind:?}")))?;
    let command = match resume_session_id.as_deref() {
        Some(session_id) => kind.resume_command(session_id).ok_or_else(|| {
            ApiError::new(
                "agent_resume_unsupported",
                "the agent kind or session id does not have a verified resume command",
            )
        })?,
        None if launchable_kinds().any(|launchable| launchable == kind) => kind
            .start_command()
            .expect("launchable kinds always expose a cold-start command"),
        None => {
            return Err(ApiError::new(
                "agent_launch_unsupported",
                format!(
                    "cold start is not verified for agent kind {:?}",
                    kind.slug()
                ),
            ));
        }
    };
    Ok((kind, resume_session_id, command))
}

/// `agent.fork` 的 transport 侧事务:解析源目录 → 校验 → 建 branch/worktree
/// → 变成带 provenance 的 `AgentStart`。失败在 UI 动作之前返回。
pub(super) fn prepare_dispatch_command(
    command: RuntimeCommand,
    hub: &RuntimeHub,
) -> Result<(RuntimeCommand, Option<WorktreeTransaction>), ApiError> {
    // 名字冲突必须在任何 UI 动作之前拒绝:注册发生在 UI owner 上,晚到
    // 的冲突会让一个已经启动的 pane 处于无注册身份的半态。
    if let RuntimeCommand::AgentStart { name, .. } = &command {
        hub.ensure_agent_name_available(name)?;
    }
    let RuntimeCommand::AgentFork {
        window_id,
        source_pane_id,
        source_cwd,
        name,
        kind,
        session_id,
        command,
        branch,
        base,
        path,
        allow_dirty_source,
    } = command
    else {
        return Ok((command, None));
    };

    hub.ensure_agent_name_available(&name)?;
    let (window_id, source_cwd) = match source_pane_id {
        Some(pane_id) => {
            let snapshot = hub.current().ok_or_else(|| {
                ApiError::new(
                    "runtime_unavailable",
                    "no runtime snapshot is available to resolve source_pane_id",
                )
            })?;
            let (resolved_window, pane) = snapshot.pane_target(window_id, pane_id)?;
            if pane.cwd.trim().is_empty() {
                return Err(ApiError::new(
                    "cwd_unavailable",
                    "the source pane has not reported a working directory",
                )
                .details(json!({ "window_id": resolved_window, "pane_id": pane_id })));
            }
            let cwd = PathBuf::from(&pane.cwd);
            if !cwd.is_absolute() {
                return Err(ApiError::new(
                    "cwd_unavailable",
                    "the source pane did not report an absolute working directory",
                )
                .details(json!({
                    "window_id": resolved_window,
                    "pane_id": pane_id,
                    "cwd": pane.cwd
                })));
            }
            (Some(resolved_window), cwd)
        }
        None => (
            window_id,
            source_cwd.expect("agent.fork validation requires source_cwd without a pane"),
        ),
    };

    let transaction = WorktreeTransaction::prepare(WorktreeRequest {
        source_cwd,
        agent_name: name.clone(),
        branch,
        base,
        path,
        allow_dirty_source,
    })?;
    let provenance = transaction.provenance().clone();
    let cwd = provenance.path.clone();
    Ok((
        RuntimeCommand::AgentStart {
            window_id,
            pane_id: None,
            name,
            kind,
            cwd: Some(cwd),
            session_id,
            command,
            worktree: Some(provenance),
        },
        Some(transaction),
    ))
}

pub(super) fn rollback_prepared_worktree(
    mut error: ApiError,
    transaction: Option<WorktreeTransaction>,
) -> ApiError {
    if let Some(transaction) = transaction
        && let Err(rollback) = transaction.rollback()
    {
        error.details = Some(json!({
            "operation": error.details,
            "rollback": rollback
        }));
    }
    error
}

pub(super) fn attach_worktree_result(result: &mut Value, provenance: &WorktreeProvenance) {
    let encoded = serde_json::to_value(provenance).unwrap_or(Value::Null);
    if let Some(Value::Object(action)) = result.get_mut("action") {
        action.insert("worktree".to_owned(), encoded);
    } else if let Value::Object(action) = result {
        action.insert("worktree".to_owned(), encoded);
    }
}

pub(super) fn agents_connection(
    stream: &mut std::net::TcpStream,
    request_id: String,
    params: AgentsParams,
    hub: &RuntimeHub,
) -> Result<(), std::io::Error> {
    let Some(snapshot) = hub.current() else {
        return write_response(
            stream,
            &ApiResponse::failure(
                request_id,
                ApiError::new(
                    "runtime_unavailable",
                    "the runtime has not published its first workspace snapshot yet",
                ),
            ),
        );
    };
    if let Some(window_id) = params.window_id
        && !snapshot.windows.iter().any(|window| window.id == window_id)
    {
        return write_response(
            stream,
            &ApiResponse::failure(
                request_id,
                ApiError::new(
                    "target_not_found",
                    format!("window {window_id} does not exist"),
                ),
            ),
        );
    }

    let mut agents = Vec::new();
    for window in snapshot
        .windows
        .iter()
        .filter(|window| params.window_id.is_none_or(|id| window.id == id))
    {
        for tab in &window.tabs {
            for pane in &tab.panes {
                let Some(agent) = pane.agent.clone() else {
                    continue;
                };
                agents.push(super::RuntimeAgentPane {
                    window_id: window.id,
                    tab_index: tab.index,
                    tab_active: tab.active,
                    pane_id: pane.id,
                    pane_active: pane.active,
                    title: pane.title.clone(),
                    cwd: pane.cwd.clone(),
                    branch: pane.branch.clone(),
                    agent,
                    task_state: pane.task_state,
                    state_change_seq: pane.state_change_seq,
                });
            }
        }
    }
    write_response(
        stream,
        &ApiResponse::success(
            request_id,
            json!({ "revision": snapshot.revision, "agents": agents }),
        ),
    )
}

pub(super) fn agent_get_connection(
    stream: &mut std::net::TcpStream,
    request_id: String,
    params: AgentTargetParams,
    hub: &RuntimeHub,
) -> Result<(), std::io::Error> {
    log::info!(
        "runtime agent.get request_id={} agent={} generation={:?}",
        request_id,
        params.agent,
        params.generation
    );
    let agent = match hub.managed_agent(&params.agent, params.generation, false) {
        Ok(agent) => agent,
        Err(error) => return write_response(stream, &ApiResponse::failure(request_id, error)),
    };
    let pane = hub.current().and_then(|snapshot| {
        snapshot
            .pane(Some(agent.window_id), agent.pane_id)
            .ok()
            .cloned()
    });
    write_response(
        stream,
        &ApiResponse::success(request_id, json!({ "agent": agent, "pane": pane })),
    )
}

pub(super) fn agent_delegate_connection(
    stream: &mut std::net::TcpStream,
    request_id: String,
    params: AgentDelegateParams,
    sink: &EventSink,
    hub: &RuntimeHub,
) -> Result<(), std::io::Error> {
    let target = match hub.active_agent(&params.agent, params.generation) {
        Ok(target) => target,
        Err(error) => return write_response(stream, &ApiResponse::failure(request_id, error)),
    };
    let mut delegation = match hub.begin_delegation(params.origin_pane_id, &target) {
        Ok(delegation) => delegation,
        Err(error) => return write_response(stream, &ApiResponse::failure(request_id, error)),
    };
    log::info!(
        "runtime agent.delegate request_id={} task_id={} origin_pane={} target_agent={} generation={}",
        request_id,
        delegation.task_id,
        params.origin_pane_id,
        target.agent_id,
        target.generation
    );
    let command = RuntimeCommand::AgentPrompt {
        agent: target.agent_id.clone(),
        generation: Some(target.generation),
        text: params.text,
        submit: true,
    };
    let response = match dispatch_runtime_command(command, sink, hub) {
        Ok(mut result) => {
            delegation.status = "submitted".to_owned();
            if let Some(object) = result.as_object_mut() {
                object.insert(
                    "delegation".to_owned(),
                    serde_json::to_value(&delegation).unwrap_or(Value::Null),
                );
            } else {
                result = json!({ "result": result, "delegation": delegation });
            }
            ApiResponse::success(request_id, result)
        }
        Err(error) => {
            hub.cancel_delegation(&delegation.task_id);
            ApiResponse::failure(request_id, error)
        }
    };
    write_response(stream, &response)
}

pub(super) fn agent_wait_connection(
    stream: &mut std::net::TcpStream,
    request_id: String,
    params: AgentWaitParams,
    hub: &RuntimeHub,
) -> Result<(), std::io::Error> {
    log::info!(
        "runtime agent.wait request_id={} agent={} generation={} state={:?} after_seq={:?}",
        request_id,
        params.agent,
        params.generation,
        params.state,
        params.after_seq
    );
    let timeout = Duration::from_millis(params.timeout_ms);
    let agent = match hub.active_agent(&params.agent, Some(params.generation)) {
        Ok(agent) => agent,
        Err(error) => return write_response(stream, &ApiResponse::failure(request_id, error)),
    };
    let (_, current, receiver) = hub.subscribe();
    let deadline = Instant::now() + timeout;
    let mut snapshot = current;
    let mut observed = None;
    loop {
        // 每次快照到达先重验 identity:退出/顶替立即失败,不静默换目标。
        if let Err(error) = hub.active_agent(&agent.agent_id, Some(agent.generation)) {
            return write_response(stream, &ApiResponse::failure(request_id, error));
        }
        if let Some(current) = snapshot.take() {
            match current.pane(Some(agent.window_id), agent.pane_id) {
                Ok(pane) if wait_matches(pane, params.state, params.after_seq) => {
                    let active = match hub.active_agent(&agent.agent_id, Some(agent.generation)) {
                        Ok(active) => active,
                        Err(error) => {
                            return write_response(
                                stream,
                                &ApiResponse::failure(request_id, error),
                            );
                        }
                    };
                    return write_response(
                        stream,
                        &ApiResponse::success(
                            request_id,
                            json!({ "agent": active, "snapshot": current }),
                        ),
                    );
                }
                Ok(pane) => observed = Some((pane.task_state, pane.state_change_seq)),
                Err(error) => {
                    let error = hub
                        .active_agent(&agent.agent_id, Some(agent.generation))
                        .err()
                        .unwrap_or_else(|| {
                            ApiError::new("agent_closed", error.message).details(json!({
                                "agent_id": agent.agent_id,
                                "generation": agent.generation
                            }))
                        });
                    return write_response(stream, &ApiResponse::failure(request_id, error));
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(next) => snapshot = Some(next),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return write_response(
                    stream,
                    &ApiResponse::failure(
                        request_id,
                        ApiError::new("runtime_unavailable", "runtime subscription disconnected"),
                    ),
                );
            }
        }
    }
    write_response(
        stream,
        &ApiResponse::failure(
            request_id,
            ApiError::new(
                "timeout",
                format!(
                    "agent {:?} did not reach the requested state before timeout",
                    agent.name
                ),
            )
            .details(json!({
                "agent_id": agent.agent_id,
                "generation": agent.generation,
                "after_seq": params.after_seq,
                "observed_state": observed.map(|(state, _)| state),
                "observed_state_change_seq": observed.map(|(_, seq)| seq)
            })),
        ),
    )
}

/// 测试/编排复用:给 agent.start 构造一条最小请求。
#[allow(dead_code)] // 消费点 = orchestrate::agent_start_command。
pub(super) fn agent_start_request(
    window_id: Option<u64>,
    pane_id: u64,
    name: String,
    kind: String,
    resume_session_id: Option<String>,
) -> ApiRequest {
    ApiRequest::new(
        String::new(),
        "agent.start",
        json!({
            "window_id": window_id,
            "pane_id": pane_id,
            "name": name,
            "kind": kind,
            "resume_session_id": resume_session_id
        }),
    )
}

#[cfg(test)]
mod agent_api_tests {
    use super::*;
    use serde_json::Value;

    fn schema_launch_kinds() -> Vec<String> {
        let schema: Value =
            serde_json::from_str(include_str!("../../../assets/runtime-api-v1.schema.json"))
                .expect("runtime API schema must stay valid JSON");
        schema["$defs"]["launch_agent_kind"]["enum"]
            .as_array()
            .expect("launch_agent_kind enum")
            .iter()
            .map(|slug| slug.as_str().expect("launch agent kind slug").to_owned())
            .collect()
    }

    /// schema 的 launch_agent_kind 枚举与 `launchable_kinds()` 必须逐项相等:
    /// 任一侧新增/删除可冷启动 kind 而另一侧未同步,此用例即红。
    #[test]
    fn launch_agent_kind_schema_equals_cold_start_capability() {
        let mut declared = schema_launch_kinds();
        let mut launchable: Vec<String> = launchable_kinds()
            .map(|kind| kind.slug().to_owned())
            .collect();
        declared.sort_unstable();
        launchable.sort_unstable();
        assert_eq!(
            declared, launchable,
            "schema 枚举必须等于 start_command() 可用的 kind 集合"
        );

        for slug in declared {
            let kind = AgentKind::parse(&slug).expect("schema slug 必须是已登记 kind");
            assert_eq!(kind.slug(), slug, "schema slug 必须是规范 slug");
            assert!(
                kind.start_command().is_some(),
                "schema slug {slug} 必须带有核实过的冷启动命令"
            );
            let (resolved, session_id, command) =
                verified_agent_launch(&slug, None).expect("schema slug 必须可冷启动");
            assert_eq!(resolved, kind);
            assert_eq!(session_id, None);
            assert_eq!(Some(command), kind.start_command());
        }
    }

    /// 合法 kind 但未核实冷启动 → `agent_launch_unsupported`;未知 slug →
    /// `invalid_params`。两种语义不得混用。
    #[test]
    fn valid_kind_without_cold_start_reports_launch_unsupported() {
        assert_eq!(AgentKind::parse("gemini"), Some(AgentKind::Gemini));
        assert_eq!(AgentKind::Gemini.start_command(), None);
        let error = verified_agent_launch("gemini", None).expect_err("gemini has no cold start");
        assert_eq!(error.code, "agent_launch_unsupported");

        let error =
            verified_agent_launch("not-an-agent", None).expect_err("unknown slug must fail");
        assert_eq!(error.code, "invalid_params");
    }
}
