//! Runtime 控制面传输主链:accept 配额、首行分流、信封校验、读写行。
//!
//! 方法路由单点:`command::parse_method` 是全表(与 capabilities/Schema 方法
//! 枚举三向一致)。读方法(`describe`/`subscribe`/`wait`/`agents.*`/`agent.get`/
//! `agent.wait`)在 transport 线程直接对 hub 读写;写方法与编排经
//! `dispatch_runtime_command` 进 UI owner。未知方法显式 `method_not_found`,
//! 不伪造成功。

use std::io::{BufRead, BufReader, Error as IoError, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::agent_api;
use super::command::{ParsedMethod, SubscribeParams, WaitParams, parse_method, wait_matches};
#[cfg(feature = "legacy-attach")]
use super::emit_attach;
use super::orchestrate;
use super::run_wait::wait_run_phased;
use super::{
    ApiError, ApiEvent, ApiRequest, ApiResponse, CAPABILITIES, COMMAND_TIMEOUT, EventSink,
    FEATURES, IO_TIMEOUT, MAX_CLIENTS, MAX_REQUEST_BYTES, PROTOCOL_NAME, PROTOCOL_VERSION,
    RuntimeCommand, RuntimeDispatch, RuntimeHub, SCHEMA_PATH, SUPPORTED_VERSIONS, emit_control,
};

mod audit;

/// accept 循环:按 `MAX_CLIENTS` 计数,每连接一线程,配额随连接线程 Drop 归还。
/// `stop` 置位后退出(循环头与 accept 返回后各查一次);阻塞中的 accept 由
/// `RuntimeServer::drop` 自连本机端口唤醒,不做非阻塞轮询。
pub(super) fn serve(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    token: String,
    sink: EventSink,
    hub: RuntimeHub,
) {
    let active = Arc::new(AtomicUsize::new(0));
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let stream = match listener.accept() {
            Ok((stream, _peer)) => stream,
            Err(_) => continue,
        };
        // Drop 的自连唤醒连接只用于解除 accept 阻塞,不再派发处理线程。
        if stop.load(Ordering::Acquire) {
            return;
        }
        let Some(guard) = admit_client(&active) else {
            continue;
        };
        let token = token.clone();
        let sink = sink.clone();
        let hub = hub.clone();
        let _ = thread::Builder::new()
            .name("slterm-runtime-client".to_owned())
            .spawn(move || {
                let _guard = guard;
                if let Err(error) = handle_connection(stream, &token, &sink, &hub) {
                    log::debug!("runtime client disconnected: {error}");
                }
            });
    }
}

struct ActiveClient(std::sync::Arc<AtomicUsize>);

impl Drop for ActiveClient {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 计数先占后判:超限立刻归还,不让被拒连接挤掉已在线配额。
fn admit_client(active: &std::sync::Arc<AtomicUsize>) -> Option<ActiveClient> {
    if active.fetch_add(1, Ordering::AcqRel) >= MAX_CLIENTS {
        active.fetch_sub(1, Ordering::AcqRel);
        return None;
    }
    Some(ActiveClient(active.clone()))
}

/// 单连接单请求:读超时 `IO_TIMEOUT`,首行超限 `request_too_large`。
pub(super) fn handle_connection(
    mut stream: TcpStream,
    token: &str,
    sink: &EventSink,
    hub: &RuntimeHub,
) -> Result<(), IoError> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let reader = BufReader::new(stream.try_clone()?);
    let mut limited = reader.take((MAX_REQUEST_BYTES + 1) as u64);
    let mut line = String::new();
    limited.read_line(&mut line)?;
    if line.len() > MAX_REQUEST_BYTES {
        return write_response(
            &mut stream,
            &ApiResponse::failure(
                "unknown",
                ApiError::new("request_too_large", "request too large"),
            ),
        );
    }

    // 首行非 JSON:首发默认静默丢弃(D12-5 回落面关闭);开启 legacy-attach
    // 后走 ATTACH/PING 探测,token 不符仍静默丢弃。
    if !line.trim_start().starts_with('{') {
        #[cfg(feature = "legacy-attach")]
        {
            return handle_legacy(&mut stream, &line, token, sink);
        }
        #[cfg(not(feature = "legacy-attach"))]
        {
            let _ = (token, sink);
            return Ok(());
        }
    }

    let raw: Value = match serde_json::from_str(&line) {
        Ok(raw) => raw,
        Err(error) => {
            return write_response(
                &mut stream,
                &ApiResponse::failure(
                    "unknown",
                    ApiError::new("invalid_request", format!("invalid JSON: {error}")),
                ),
            );
        }
    };
    // 认证先于详细解析:未认证的本机进程不得把解析错误当协议预言机。
    if raw.get("token").and_then(Value::as_str) != Some(token) {
        return Ok(());
    }
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let request: ApiRequest = match serde_json::from_value(raw) {
        Ok(request) => request,
        Err(error) => {
            return write_response(
                &mut stream,
                &ApiResponse::failure(
                    id,
                    ApiError::new(
                        "invalid_request",
                        format!("invalid request envelope: {error}"),
                    ),
                ),
            );
        }
    };

    if request.protocol != PROTOCOL_NAME || request.version != PROTOCOL_VERSION {
        let error = ApiError::new(
            "protocol_version_mismatch",
            format!(
                "requested protocol {:?} v{}; this runtime supports {} v{}",
                request.protocol, request.version, PROTOCOL_NAME, PROTOCOL_VERSION
            ),
        )
        .details(json!({ "supported_versions": SUPPORTED_VERSIONS }));
        return write_response(&mut stream, &ApiResponse::failure(request.id, error));
    }

    // 唯一路由表:解析成功的方法一定在 capabilities 面内;未知方法显式
    // `method_not_found`,不伪造成功。
    let parsed = match parse_method(&request.method, &request.params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return write_response(&mut stream, &ApiResponse::failure(request.id, error));
        }
    };
    match parsed {
        ParsedMethod::Describe => write_response(
            &mut stream,
            &ApiResponse::success(request.id, runtime_description()),
        ),
        ParsedMethod::Snapshot => {
            dispatch_connection(&mut stream, request.id, RuntimeCommand::Snapshot, sink, hub)
        }
        ParsedMethod::Subscribe(params) => {
            subscribe_connection(&mut stream, request.id, params, hub)
        }
        ParsedMethod::Wait(params) => wait_connection(&mut stream, request.id, params, hub),
        ParsedMethod::AgentsList(params) => {
            agent_api::agents_connection(&mut stream, request.id, params, hub)
        }
        ParsedMethod::AgentGet(params) => {
            agent_api::agent_get_connection(&mut stream, request.id, params, hub)
        }
        ParsedMethod::AgentDelegate(params) => {
            agent_api::agent_delegate_connection(&mut stream, request.id, params, sink, hub)
        }
        ParsedMethod::AgentWait(params) => {
            agent_api::agent_wait_connection(&mut stream, request.id, params, hub)
        }
        ParsedMethod::Orchestrate(params) => {
            orchestrate::orchestrate_connection(&mut stream, request.id, *params, sink, hub)
        }
        ParsedMethod::Command(command) => {
            dispatch_connection(&mut stream, request.id, *command, sink, hub)
        }
    }
}

/// legacy 首行:ATTACH(唤醒/还原窗口,fire-and-forget)/ PING(token 不符静默)。
#[cfg(feature = "legacy-attach")]
pub(super) fn handle_legacy(
    stream: &mut TcpStream,
    line: &str,
    token: &str,
    sink: &EventSink,
) -> Result<(), IoError> {
    let mut parts = line.split_whitespace();
    let verb = parts.next().unwrap_or("");
    if parts.next() != Some(token) {
        return Ok(());
    }
    match verb {
        "ATTACH" => {
            emit_attach(sink);
            stream.write_all(b"OK\n")
        }
        "PING" => stream.write_all(b"OK\n"),
        _ => Ok(()),
    }
}

/// describe:版本/能力/环境契约/资源命令四段。capabilities 与协议 Schema
/// 方法枚举单源(裁剪后首发面)。
pub(super) fn runtime_description() -> Value {
    json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_NAME,
        "protocol_version": PROTOCOL_VERSION,
        "supported_versions": SUPPORTED_VERSIONS,
        "schema": SCHEMA_PATH,
        "capabilities": CAPABILITIES,
        // 附加参数能力无法从 capabilities 探测:旧版静默忽略未知 after_seq
        // 仍带竞态,严格客户端必须检查 feature 串。
        "features": FEATURES,
        // 环境契约:让外部客户端探测变量名而不是硬编码;实现归 AI 集成域
        // `agent_env`。remote_marker 随 SSH 整支不迁。
        "env": {
            "term_program": crate::agent_env::TERM_PROGRAM,
            "pane": crate::agent_env::PANE_ENV,
            "cli": crate::agent_env::CLI_ENV,
            "bin_dir": crate::agent_env::BIN_DIR_ENV,
            "process": crate::agent_env::PROCESS_ENV,
            "bin_dir_on_path": true
        },
        // 资源 + 动词是同一批能力的易发现入口,语义与 capabilities 一致。
        "commands": {
            "env": ["env"],
            "window": ["close"],
            "tab": ["close", "rename", "move"],
            "pane": ["list", "read", "send", "paste", "wait", "exec", "close", "zoom", "resize"],
            "agent": ["list", "send", "delegate", "paste", "read", "wait"]
        }
    })
}

/// 订阅:首行确认(subscription_id/current_revision)后逐行推快照。
/// `since_revision` 断点续传:current 不比基线新则不下发首行。
pub(super) fn subscribe_connection(
    stream: &mut TcpStream,
    request_id: String,
    params: SubscribeParams,
    hub: &RuntimeHub,
) -> Result<(), IoError> {
    let (subscription_id, current, receiver) = hub.subscribe();
    write_response(
        stream,
        &ApiResponse::success(
            request_id,
            json!({
                "subscription_id": subscription_id,
                "current_revision": current.as_ref().map_or(0, |snapshot| snapshot.revision)
            }),
        ),
    )?;
    // 订阅是长连接:读侧已结束,写侧不能沿用单请求 5s 超时。
    stream.set_write_timeout(None)?;
    if let Some(snapshot) = current
        && params
            .since_revision
            .is_none_or(|revision| snapshot.revision > revision)
    {
        write_json_line(stream, &ApiEvent::snapshot(snapshot))?;
    }
    while let Ok(snapshot) = receiver.recv() {
        write_json_line(stream, &ApiEvent::snapshot(snapshot))?;
    }
    Ok(())
}

/// wait 专链:先验当前快照,未命中进 recv_timeout 循环;生命周期错误优先
/// 于超时。超时 details 带 after_seq 与 observed_state_change_seq,区分
/// 「一直没动」与「跃迁了但没到目标态」。
pub(super) fn wait_connection(
    stream: &mut TcpStream,
    request_id: String,
    params: WaitParams,
    hub: &RuntimeHub,
) -> Result<(), IoError> {
    let timeout = Duration::from_millis(params.timeout_ms);

    // 先订阅再读 current:否则「读到 current 之后、订阅建立之前」的跃迁
    // 会丢事件,等待只能走满超时。
    let (_, current, receiver) = hub.subscribe();
    let mut observed = None;
    let mut target_window = params.window_id;
    if let Some(snapshot) = current {
        match snapshot.pane_target(target_window, params.pane_id) {
            Ok((window_id, pane)) => {
                target_window = Some(window_id);
                if let Some(error) = hub.pane_lifecycle_error(target_window, params.pane_id) {
                    return write_response(stream, &ApiResponse::failure(request_id, error));
                }
                if wait_matches(pane, params.state, params.after_seq) {
                    return write_response(
                        stream,
                        &ApiResponse::success(
                            request_id,
                            serde_json::to_value(snapshot).map_err(IoError::other)?,
                        ),
                    );
                }
                observed = Some((pane.task_state, pane.state_change_seq));
            }
            Err(error) if error.code == "target_not_found" => {
                if let Some(lifecycle) = hub.pane_lifecycle_error(target_window, params.pane_id) {
                    return write_response(stream, &ApiResponse::failure(request_id, lifecycle));
                }
            }
            Err(error) => {
                return write_response(stream, &ApiResponse::failure(request_id, error));
            }
        }
    } else if let Some(error) = hub.pane_lifecycle_error(target_window, params.pane_id) {
        return write_response(stream, &ApiResponse::failure(request_id, error));
    }

    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let snapshot = match receiver.recv_timeout(remaining) {
            Ok(snapshot) => snapshot,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if let Some(error) = hub.pane_lifecycle_error(target_window, params.pane_id) {
            return write_response(stream, &ApiResponse::failure(request_id, error));
        }
        match snapshot.pane_target(target_window, params.pane_id) {
            Ok((window_id, pane)) => {
                target_window = Some(window_id);
                if wait_matches(pane, params.state, params.after_seq) {
                    return write_response(
                        stream,
                        &ApiResponse::success(
                            request_id,
                            serde_json::to_value(snapshot).map_err(IoError::other)?,
                        ),
                    );
                }
                observed = Some((pane.task_state, pane.state_change_seq));
            }
            Err(error) => {
                return write_response(stream, &ApiResponse::failure(request_id, error));
            }
        }
    }

    if let Some(error) = hub.pane_lifecycle_error(target_window, params.pane_id) {
        return write_response(stream, &ApiResponse::failure(request_id, error));
    }

    let detail = match observed {
        Some((state, seq)) => {
            format!("last observed state {state:?} at state_change_seq {seq}")
        }
        None => "the pane was never present in a published snapshot".to_owned(),
    };
    write_response(
        stream,
        &ApiResponse::failure(
            request_id,
            ApiError::new(
                "timeout",
                format!(
                    "pane {} did not reach the requested state before timeout; {detail}",
                    params.pane_id
                ),
            )
            .details(json!({
                "pane_id": params.pane_id,
                "window_id": target_window,
                "after_seq": params.after_seq,
                "observed_state_change_seq": observed.map(|(_, seq)| seq)
            })),
        ),
    )
}

/// 写方法派发:参数已在 transport 侧校验完;关键方法审计日志只记 id/目标/
/// 字节数,不记 prompt/paste 正文。
pub(super) fn dispatch_connection(
    stream: &mut TcpStream,
    request_id: String,
    command: RuntimeCommand,
    sink: &EventSink,
    hub: &RuntimeHub,
) -> Result<(), IoError> {
    audit::audit_command(&request_id, &command);
    let response = match dispatch_runtime_command(command, sink, hub) {
        Ok(result) => ApiResponse::success(request_id, result),
        Err(error) => ApiResponse::failure(request_id, error),
    };
    write_response(stream, &response)
}

/// 执行一个已经解析的 Runtime 原语。普通请求与编排步骤共享这里,worktree
/// 事务、run 等待与 UI 超时在两条路径上语义一致:
///
/// 1. `prepare_dispatch_command` 在 transport 线程预建 worktree(agent.fork);
/// 2. Exec 的派发预算 = 命令超时 + 2s,其余 = `COMMAND_TIMEOUT`;
/// 3. UI 不可用/无回执 = `runtime_unavailable`;超时 = `runtime_timeout`,
///    已建 worktree 保留(checkout 可能已被晚到的 PTY 使用);
/// 4. `Run { wait: true }` 在成功后分段限时等待 OSC 133 真实结果。
pub(super) fn dispatch_runtime_command(
    command: RuntimeCommand,
    sink: &EventSink,
    hub: &RuntimeHub,
) -> Result<Value, ApiError> {
    dispatch_runtime_command_impl(command, sink, hub, None)
}

#[cfg(test)]
pub(super) fn dispatch_runtime_command_with_timeout(
    command: RuntimeCommand,
    sink: &EventSink,
    hub: &RuntimeHub,
    timeout: Duration,
) -> Result<Value, ApiError> {
    dispatch_runtime_command_impl(command, sink, hub, Some(timeout))
}

fn dispatch_runtime_command_impl(
    command: RuntimeCommand,
    sink: &EventSink,
    hub: &RuntimeHub,
    timeout_override: Option<Duration>,
) -> Result<Value, ApiError> {
    let (command, mut worktree_transaction) = agent_api::prepare_dispatch_command(command, hub)?;
    let run_wait = match &command {
        RuntimeCommand::Run {
            wait: true,
            timeout_ms,
            ..
        } => Some(Duration::from_millis(*timeout_ms)),
        _ => None,
    };
    let dispatch_timeout = timeout_override.unwrap_or_else(|| match &command {
        RuntimeCommand::Exec { timeout_ms, .. } => {
            Duration::from_millis(*timeout_ms).saturating_add(Duration::from_secs(2))
        }
        _ => COMMAND_TIMEOUT,
    });
    let (dispatch, receiver) = RuntimeDispatch::new(command);
    if !emit_control(sink, dispatch) {
        return Err(agent_api::rollback_prepared_worktree(
            ApiError::new(
                "runtime_unavailable",
                "the shell event loop is not available",
            ),
            worktree_transaction.take(),
        ));
    }
    match receiver.recv_timeout(dispatch_timeout) {
        Ok(Ok(mut result)) => {
            if let Some(transaction) = worktree_transaction.take() {
                let provenance = transaction.commit();
                agent_api::attach_worktree_result(&mut result, &provenance);
            }
            if let Some(timeout) = run_wait {
                let action = result.get("action").unwrap_or(&result);
                let target = (
                    action.get("window_id").and_then(Value::as_u64),
                    action.get("pane_id").and_then(Value::as_u64),
                    action.get("run_id").and_then(Value::as_u64),
                );
                match target {
                    (Some(window_id), Some(pane_id), Some(run_id)) => {
                        let run = wait_run_phased(hub, sink, window_id, pane_id, run_id, timeout)?;
                        Ok(json!({ "run": run, "snapshot": hub.current() }))
                    }
                    _ => Err(ApiError::new(
                        "invalid_runtime_response",
                        "pane.run did not return its window, pane, and run identity",
                    )),
                }
            } else {
                Ok(result)
            }
        }
        Ok(Err(error)) => Err(agent_api::rollback_prepared_worktree(
            error,
            worktree_transaction.take(),
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            // dispatch 被壳侧丢弃 = 命令没执行过;与「已受理但未知」不同,
            // 这里可以安全回滚 worktree。
            Err(agent_api::rollback_prepared_worktree(
                ApiError::new(
                    "runtime_unavailable",
                    "the shell event queue dropped the runtime command",
                ),
                worktree_transaction.take(),
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let mut error = ApiError::new(
                "runtime_timeout",
                "runtime event thread did not answer in time",
            );
            if let Some(transaction) = worktree_transaction.take() {
                // UI 派发可能在响应通道超时之后才到达。晚到的 PTY 可能已经在
                // 使用刚建出的 cwd,绝不能在未知态回收 checkout。
                let provenance = transaction.commit();
                error.details = Some(json!({
                    "worktree": provenance,
                    "cleanup_deferred": true,
                    "reason": "the UI outcome is unknown; slTerminal did not remove the worktree"
                }));
            }
            Err(error)
        }
    }
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    response: &ApiResponse,
) -> Result<(), IoError> {
    write_json_line(stream, response)
}

/// 单行 JSON 写出(响应与订阅事件共用);换行即帧边界,写后立即 flush。
pub(super) fn write_json_line<T: Serialize>(
    stream: &mut TcpStream,
    value: &T,
) -> Result<(), IoError> {
    serde_json::to_writer(&mut *stream, value).map_err(IoError::other)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod transport_tests {
    use super::super::{RuntimeCallback, RuntimeSnapshot};
    use super::*;
    use std::net::{Shutdown, SocketAddr};
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    // SEC-18:测试一律假值占位,真实凭据禁入仓库。
    const TEST_TOKEN: &str = "sk-test-runtime-token";

    fn sink_channel() -> (EventSink, mpsc::Receiver<RuntimeCallback>) {
        let (sender, receiver) = mpsc::channel();
        let sink: EventSink = Arc::new(move |callback| {
            let _ = sender.send(callback);
        });
        (sink, receiver)
    }

    /// 测试 UI owner:同步执行闭包并 respond;闭包可断言命令形状。
    fn sink_responding(
        respond: impl Fn(&RuntimeCommand) -> Result<Value, ApiError> + Send + Sync + 'static,
    ) -> EventSink {
        Arc::new(move |callback| {
            if let RuntimeCallback::Control(dispatch) = callback {
                let response = respond(&dispatch.command);
                dispatch.respond(response);
            }
        })
    }

    /// 测试 UI owner:受理命令但不回执(模拟事件线程卡死/未知态)。
    fn sink_holding() -> (EventSink, Arc<std::sync::Mutex<Vec<Arc<RuntimeDispatch>>>>) {
        let held = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_held = held.clone();
        let sink: EventSink = Arc::new(move |callback| {
            if let RuntimeCallback::Control(dispatch) = callback {
                sink_held.lock().expect("held dispatches").push(dispatch);
            }
        });
        (sink, held)
    }

    /// 临时 Git 仓库(worktree 事务夹具)。
    fn test_git_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("create repository directory");
        let git = |args: &[&str]| {
            let mut command = std::process::Command::new("git");
            command.arg("-C").arg(directory.path()).args(args);
            crate::platform::process::hidden_command(&mut command)
                .output()
                .expect("run git")
        };
        assert!(git(&["init", "--initial-branch=main"]).status.success());
        std::fs::write(directory.path().join("tracked.txt"), "tracked")
            .expect("write tracked file");
        assert!(git(&["add", "tracked.txt"]).status.success());
        assert!(
            git(&[
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "initial"
            ])
            .status
            .success()
        );
        directory
    }

    fn branch_exists(repository: &std::path::Path, branch: &str) -> bool {
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(repository).args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ]);
        crate::platform::process::hidden_command(&mut command)
            .status()
            .expect("query branch")
            .success()
    }

    fn pane(id: u64, state: super::super::RuntimeTaskState) -> super::super::RuntimePane {
        super::super::RuntimePane {
            id,
            active: false,
            title: String::new(),
            cwd: "D:/work".to_owned(),
            branch: "main".to_owned(),
            running_program: None,
            agent: None,
            task_state: state,
            state_change_seq: 0,
            active_run: None,
            last_run: None,
        }
    }

    fn detected_agent(kind: &str, session_id: Option<&str>) -> super::super::RuntimeAgent {
        super::super::RuntimeAgent {
            agent_id: None,
            generation: None,
            name: None,
            worktree: None,
            kind: kind.to_owned(),
            display_name: kind.to_owned(),
            session_id: session_id.map(str::to_owned),
            state_source: super::super::RuntimeAgentStateSource::Hook,
            state_rule: None,
            hook_seen: true,
        }
    }

    fn pane_with_agent(
        id: u64,
        agent: super::super::RuntimeAgent,
        state: super::super::RuntimeTaskState,
    ) -> super::super::RuntimePane {
        super::super::RuntimePane {
            agent: Some(agent),
            ..pane(id, state)
        }
    }

    const TEST_CODEX_SESSION: &str = "b5f6c1c2-1111-2222-3333-444455556666";

    /// 委派夹具:hub + target managed agent,origin pane 3 / target pane 4。
    fn delegation_fixture() -> (
        RuntimeHub,
        super::super::RuntimeManagedAgent,
        super::super::RuntimeSnapshot,
    ) {
        let hub = RuntimeHub::new();
        let target = hub
            .register_agent(
                "worker".to_owned(),
                crate::ai_agents::AgentKind::Codex,
                super::super::SINGLE_WINDOW_ID,
                4,
                Some(TEST_CODEX_SESSION.to_owned()),
                None,
            )
            .expect("register target");
        let snapshot = snapshot_with_panes(vec![
            pane_with_agent(
                3,
                detected_agent("claude", Some("claude-session-1")),
                super::super::RuntimeTaskState::Finished,
            ),
            pane_with_agent(
                4,
                detected_agent("codex", Some(TEST_CODEX_SESSION)),
                super::super::RuntimeTaskState::Finished,
            ),
        ]);
        hub.publish(snapshot.clone());
        (hub, target, snapshot)
    }

    fn snapshot_with_panes(panes: Vec<super::super::RuntimePane>) -> super::super::RuntimeSnapshot {
        let focused_pane_id = panes.first().map(|pane| pane.id);
        super::super::RuntimeSnapshot::new(vec![super::super::RuntimeWindow {
            id: super::super::SINGLE_WINDOW_ID,
            focused: true,
            active_tab: 0,
            focused_pane_id,
            tabs: vec![super::super::RuntimeTab {
                tab_id: None,
                index: 0,
                active: true,
                label: "shell".to_owned(),
                kind: "terminal".to_owned(),
                bell: false,
                focused_pane_id,
                zoomed_pane_id: None,
                layout: Some(super::super::RuntimeLayout::Pane {
                    pane_id: panes.first().map_or(0, |pane| pane.id),
                }),
                panes,
            }],
        }])
    }

    /// 只 accept 一次的测试服务端:用真实回环连接跑 `handle_connection`。
    fn spawn_single_connection_server(token: &str, sink: EventSink, hub: RuntimeHub) -> SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let token = token.to_owned();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept test client");
            let _ = handle_connection(stream, &token, &sink, &hub);
        });
        address
    }

    fn connect(address: SocketAddr) -> TcpStream {
        let stream = TcpStream::connect(address).expect("connect test server");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("read timeout");
        stream
    }

    fn send_request(stream: &mut TcpStream, request: &Value) {
        stream
            .write_all(
                serde_json::to_string(request)
                    .expect("serialize request")
                    .as_bytes(),
            )
            .expect("write request");
        stream.write_all(b"\n").expect("write newline");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
    }

    fn exchange(address: SocketAddr, line: &str) -> String {
        let mut stream = connect(address);
        stream.write_all(line.as_bytes()).expect("write request");
        stream.write_all(b"\n").expect("write newline");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        response
    }

    fn exchange_json(address: SocketAddr, request: &Value) -> ApiResponse {
        let response = exchange(
            address,
            &serde_json::to_string(request).expect("serialize request"),
        );
        serde_json::from_str(&response).expect("response must be an envelope")
    }

    fn request(id: &str, method: &str, params: Value) -> Value {
        json!({
            "protocol": PROTOCOL_NAME,
            "version": PROTOCOL_VERSION,
            "id": id,
            "token": TEST_TOKEN,
            "method": method,
            "params": params,
        })
    }

    fn read_json_line(reader: &mut BufReader<TcpStream>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read line");
        serde_json::from_str(&line).expect("json line")
    }

    #[test]
    fn valid_request_echoes_identity_in_method_not_found() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(address, &request("4242-7", "runtime.unknown", json!({})));
        assert!(!response.ok);
        assert_eq!(response.protocol, PROTOCOL_NAME);
        assert_eq!(response.version, PROTOCOL_VERSION);
        assert_eq!(response.id, "4242-7");
        assert_eq!(response.error.expect("error").code, "method_not_found");
        assert!(response.result.is_none());
    }

    #[test]
    fn missing_params_field_defaults_to_empty_object_for_describe() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(
            address,
            &json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-8",
                "token": TEST_TOKEN,
                "method": "runtime.describe",
            }),
        );
        assert!(response.ok);
        assert_eq!(
            response.result.expect("describe result")["protocol"],
            PROTOCOL_NAME
        );
    }

    #[test]
    fn protocol_version_mismatch_lists_supported_versions() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(
            address,
            &json!({
                "protocol": "other.runtime",
                "version": PROTOCOL_VERSION,
                "id": "4242-9",
                "token": TEST_TOKEN,
                "method": "runtime.describe",
            }),
        );
        let error = response.error.expect("error");
        assert_eq!(error.code, "protocol_version_mismatch");
        assert_eq!(
            error.details.expect("details")["supported_versions"],
            serde_json::to_value(SUPPORTED_VERSIONS).expect("serialize supported versions")
        );
    }

    #[test]
    fn unsupported_version_number_is_rejected() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(
            address,
            &json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION + 1,
                "id": "4242-10",
                "token": TEST_TOKEN,
                "method": "runtime.describe",
            }),
        );
        assert_eq!(
            response.error.expect("error").code,
            "protocol_version_mismatch"
        );
    }

    #[test]
    fn wrong_token_is_dropped_before_envelope_validation() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        // 未知字段本应触发信封解析错误;先认证则整条连接静默丢弃。
        let response = exchange(
            address,
            &json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-11",
                "token": "sk-test-wrong-token",
                "method": "runtime.describe",
                "oracle": true,
            })
            .to_string(),
        );
        assert!(response.is_empty());
    }

    #[test]
    fn missing_token_is_dropped_silently() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange(
            address,
            &json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-12",
                "method": "runtime.describe",
            })
            .to_string(),
        );
        assert!(response.is_empty());
    }

    #[test]
    fn authenticated_unknown_envelope_field_is_rejected() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(
            address,
            &json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-13",
                "token": TEST_TOKEN,
                "method": "runtime.describe",
                "extra": true,
            }),
        );
        assert_eq!(response.id, "4242-13");
        assert_eq!(response.error.expect("error").code, "invalid_request");
    }

    #[test]
    fn invalid_json_reports_invalid_request() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response: ApiResponse =
            serde_json::from_str(&exchange(address, "{not json")).expect("error envelope");
        assert_eq!(response.id, "unknown");
        assert_eq!(response.error.expect("error").code, "invalid_request");
    }

    #[test]
    fn first_line_over_request_limit_is_rejected() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let mut stream = connect(address);
        // 行内容(含换行)恰为上限 + 1 字节;服务端整行收完后按长度拒绝。
        let mut payload = vec![b' '; MAX_REQUEST_BYTES + 1];
        payload[MAX_REQUEST_BYTES] = b'\n';
        stream.write_all(&payload).expect("write oversized request");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let response: ApiResponse = serde_json::from_str(&response).expect("error envelope");
        assert_eq!(response.id, "unknown");
        assert_eq!(response.error.expect("error").code, "request_too_large");
    }

    #[cfg(feature = "legacy-attach")]
    #[test]
    fn legacy_attach_notifies_sink_and_acknowledges() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange(address, &format!("ATTACH {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
        assert!(matches!(
            events
                .recv_timeout(Duration::from_secs(5))
                .expect("attach event"),
            RuntimeCallback::Attach
        ));
    }

    #[cfg(feature = "legacy-attach")]
    #[test]
    fn legacy_ping_acknowledges_without_attach_event() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange(address, &format!("PING {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
        // 事件(若有)先于 OK 写出,读到 OK 时队列已定型。
        assert!(events.try_recv().is_err());
    }

    #[cfg(feature = "legacy-attach")]
    #[test]
    fn legacy_wrong_token_is_silently_dropped() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange(address, "ATTACH sk-test-wrong-token");
        assert!(response.is_empty());
        assert!(events.try_recv().is_err());
    }

    /// D12-5 首发面:未开启 legacy-attach 时非 JSON 首行静默丢弃,
    /// 不产生事件也不回写任何字节。
    #[cfg(not(feature = "legacy-attach"))]
    #[test]
    fn legacy_lines_are_silently_dropped_without_the_feature() {
        for line in [
            format!("ATTACH {TEST_TOKEN}"),
            format!("PING {TEST_TOKEN}"),
            "ATTACH sk-test-wrong-token".to_owned(),
        ] {
            let (sink, events) = sink_channel();
            let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
            let response = exchange(address, &line);
            assert!(response.is_empty(), "{line}");
            assert!(events.try_recv().is_err(), "{line}");
        }
    }

    #[test]
    fn client_quota_counts_live_connections_and_releases_on_drop() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut clients = Vec::new();
        for _ in 0..MAX_CLIENTS {
            clients.push(admit_client(&active).expect("within quota"));
        }
        assert!(admit_client(&active).is_none());
        assert_eq!(active.load(Ordering::Acquire), MAX_CLIENTS);

        clients.pop();
        let extra = admit_client(&active).expect("released slot");
        assert_eq!(active.load(Ordering::Acquire), MAX_CLIENTS);
        drop(extra);
        drop(clients);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

    #[cfg(feature = "legacy-attach")]
    #[test]
    fn serve_accepts_connections_and_answers_legacy_ping() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind serve listener");
        let address = listener.local_addr().expect("serve address");
        let (sink, _events) = sink_channel();
        let stop = Arc::new(AtomicBool::new(false));
        let serve_stop = stop.clone();
        let handle = std::thread::spawn(move || {
            serve(
                listener,
                serve_stop,
                TEST_TOKEN.to_owned(),
                sink,
                RuntimeHub::new(),
            )
        });
        let response = exchange(address, &format!("PING {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
        // 置位后自连唤醒 accept,serve 线程必须退出,不留测试线程。
        stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&address, Duration::from_millis(400));
        handle.join().expect("serve 线程随停止位退出");
    }

    #[test]
    fn describe_reports_protocol_version_and_schema() {
        let description = runtime_description();
        assert_eq!(description["protocol"], PROTOCOL_NAME);
        assert_eq!(description["protocol_version"], PROTOCOL_VERSION);
        assert_eq!(
            description["supported_versions"],
            serde_json::to_value(SUPPORTED_VERSIONS).expect("versions")
        );
        assert_eq!(description["app_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(description["schema"], SCHEMA_PATH);
    }

    /// 首发 capability 面逐字钉死;顺序即协议公布序。
    #[test]
    fn describe_capabilities_match_first_release_surface() {
        let expected = [
            "runtime.describe",
            "runtime.snapshot",
            "runtime.orchestrate",
            "events.subscribe",
            "agents.list",
            "agent.start",
            "agent.fork",
            "agent.get",
            "agent.delegate",
            "agent.prompt",
            "agent.paste",
            "agent.read",
            "agent.wait",
            "window.close",
            "window.focus",
            "tab.new",
            "tab.close",
            "tab.rename",
            "tab.move",
            "pane.split",
            "pane.close",
            "pane.zoom",
            "pane.resize",
            "pane.prompt",
            "pane.paste",
            "pane.read",
            "pane.procs",
            "pane.send_key",
            "pane.run",
            "pane.exec",
            "pane.wait",
            "git.status",
            "git.diff",
            "git.rollback",
            "git.unstage",
        ];
        assert_eq!(
            runtime_description()["capabilities"],
            serde_json::to_value(expected.to_vec()).expect("capabilities")
        );
    }

    #[test]
    fn describe_features_match_first_release_surface() {
        let expected = [
            "pane.wait.after_seq",
            "pane.wait.lifecycle",
            "agent.wait.identity",
            "agent.delegate.callback",
            "agent.fork.transactional_worktree",
            "agent.worktree.provenance",
            "events.pane_lifecycle",
            "runtime.orchestrate.typed_steps",
            "runtime.orchestrate.agent_ready",
            "env.pane_identity",
            "cli.resource_verbs",
            "cli.paste_sources",
            "pane.exec.non_tty",
        ];
        assert_eq!(
            runtime_description()["features"],
            serde_json::to_value(expected.to_vec()).expect("features")
        );
    }

    #[test]
    fn describe_env_contract_has_the_exact_variable_set() {
        let description = runtime_description();
        let env = description["env"].as_object().expect("env object");
        let mut keys: Vec<_> = env.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "bin_dir",
                "bin_dir_on_path",
                "cli",
                "pane",
                "process",
                "term_program"
            ]
        );
        assert_eq!(env["term_program"], crate::agent_env::TERM_PROGRAM);
        assert_eq!(env["pane"], crate::agent_env::PANE_ENV);
        assert_eq!(env["cli"], crate::agent_env::CLI_ENV);
        assert_eq!(env["bin_dir"], crate::agent_env::BIN_DIR_ENV);
        assert_eq!(env["process"], crate::agent_env::PROCESS_ENV);
        assert_eq!(env["bin_dir_on_path"], true);
    }

    #[test]
    fn describe_commands_have_the_exact_resource_set() {
        let description = runtime_description();
        let commands = description["commands"]
            .as_object()
            .expect("commands object");
        let mut keys: Vec<_> = commands.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["agent", "env", "pane", "tab", "window"]);
    }

    /// B26 防漂移:describe.commands 的动词必须与 CLI 实际命令清单逐项一致。
    /// 从 clap 的 `Command` 树派生，不复制第二份手写表。
    #[test]
    fn describe_commands_match_the_cli_catalog() {
        use clap::CommandFactory;

        let description = runtime_description();
        let commands = description["commands"]
            .as_object()
            .expect("commands object");
        let cli = crate::cli::Cli::command();
        for (resource, verbs) in commands {
            let resource_command = cli
                .get_subcommands()
                .find(|command| command.get_name() == resource)
                .unwrap_or_else(|| panic!("CLI 缺少资源子命令 {resource}"));
            let subcommands: Vec<&str> = resource_command
                .get_subcommands()
                .map(|command| command.get_name())
                .collect();
            // 顶层单动词资源(env):命令本身就是动词;其余资源取子命令名。
            let mut cli_verbs: Vec<&str> = if subcommands.is_empty() {
                vec![resource.as_str()]
            } else {
                subcommands
            };
            let mut described: Vec<&str> = verbs
                .as_array()
                .expect("describe verbs array")
                .iter()
                .map(|verb| verb.as_str().expect("verb string"))
                .collect();
            cli_verbs.sort_unstable();
            described.sort_unstable();
            assert_eq!(
                described, cli_verbs,
                "资源 {resource} 的 describe 动词与 CLI 清单不一致"
            );
        }
        // CLI 不得隐藏未公布的资源动词(non-resource 入口除外)。
        let mut cli_resources: Vec<&str> = cli
            .get_subcommands()
            .map(|command| command.get_name())
            .filter(|name| matches!(*name, "env" | "window" | "tab" | "pane" | "agent"))
            .collect();
        let mut described_resources: Vec<&str> = commands.keys().map(String::as_str).collect();
        cli_resources.sort_unstable();
        described_resources.sort_unstable();
        assert_eq!(
            described_resources, cli_resources,
            "CLI 资源命令面与 describe 公布面不一致"
        );
    }

    /// 协议 Schema 与描述单源:方法枚举 == capabilities(裁剪后首发面)。
    #[test]
    fn schema_method_enum_matches_describe_capabilities() {
        let schema: Value =
            serde_json::from_str(include_str!("../../../assets/runtime-api-v1.schema.json"))
                .expect("protocol schema must stay valid JSON");
        let methods: Vec<&str> = schema["$defs"]["request"]["properties"]["method"]["enum"]
            .as_array()
            .expect("method enum")
            .iter()
            .map(|method| method.as_str().expect("method string"))
            .collect();
        let description = runtime_description();
        let capabilities = description["capabilities"]
            .as_array()
            .expect("capabilities")
            .iter()
            .map(|method| method.as_str().expect("capability string"))
            .collect::<Vec<_>>();
        assert_eq!(methods, capabilities);
        for removed in [
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
            assert!(
                !methods.contains(&removed),
                "schema still enumerates trimmed method {removed}"
            );
        }
    }

    #[test]
    fn describe_serves_over_the_wire() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(address, &request("4242-20", "runtime.describe", json!({})));
        assert!(response.ok);
        let result = response.result.expect("describe result");
        assert_eq!(result["protocol"], PROTOCOL_NAME);
        assert!(result["capabilities"].as_array().is_some());
    }

    #[test]
    fn subscribe_connection_acknowledges_and_streams_snapshots() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub.clone());
        let mut stream = connect(address);
        send_request(
            &mut stream,
            &request("4242-30", "events.subscribe", json!({})),
        );
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

        let response: ApiResponse =
            serde_json::from_value(read_json_line(&mut reader)).expect("subscribe ack");
        assert!(response.ok);
        let result = response.result.expect("ack result");
        assert_eq!(result["subscription_id"], 1);
        assert_eq!(result["current_revision"], 1);

        let first: ApiEvent<RuntimeSnapshot> =
            serde_json::from_value(read_json_line(&mut reader)).expect("current snapshot");
        assert_eq!(first.event, "runtime.snapshot");
        assert_eq!(first.revision, 1);

        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Running,
        )]));
        let second: ApiEvent<RuntimeSnapshot> =
            serde_json::from_value(read_json_line(&mut reader)).expect("changed snapshot");
        assert_eq!(second.revision, 2);
        assert_eq!(
            second.data.windows[0].tabs[0].panes[0].task_state,
            super::super::RuntimeTaskState::Running
        );
    }

    #[test]
    fn subscribe_since_revision_skips_current_when_not_newer() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub.clone());
        let mut stream = connect(address);
        send_request(
            &mut stream,
            &request(
                "4242-31",
                "events.subscribe",
                json!({ "since_revision": 1 }),
            ),
        );
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let _ack: ApiResponse = serde_json::from_value(read_json_line(&mut reader)).expect("ack");

        // current 不比基线新:不得下发首行。超时设在 reader 持有的底层
        // 句柄上,Windows 的 SO_RCVTIMEO 语义随句柄生效。
        reader
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("short read timeout");
        let mut probe = String::new();
        let error = reader
            .read_line(&mut probe)
            .expect_err("no immediate event");
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ),
            "unexpected read error: {error:?}"
        );

        reader
            .get_ref()
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("restore read timeout");
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Running,
        )]));
        let event: ApiEvent<RuntimeSnapshot> =
            serde_json::from_value(read_json_line(&mut reader)).expect("new event");
        assert_eq!(event.revision, 2);
    }

    #[test]
    fn subscribe_replays_current_when_since_revision_is_older() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub.clone());
        let mut stream = connect(address);
        send_request(
            &mut stream,
            &request(
                "4242-32",
                "events.subscribe",
                json!({ "since_revision": 0 }),
            ),
        );
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let _ack: ApiResponse = serde_json::from_value(read_json_line(&mut reader)).expect("ack");
        let event: ApiEvent<RuntimeSnapshot> =
            serde_json::from_value(read_json_line(&mut reader)).expect("replayed snapshot");
        assert_eq!(event.revision, 1);
    }

    #[test]
    fn wait_connection_returns_immediately_when_state_matches() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(
            address,
            &request(
                "4242-40",
                "pane.wait",
                json!({ "pane_id": 1, "state": "idle", "timeout_ms": 1000 }),
            ),
        );
        assert!(response.ok);
        let snapshot: RuntimeSnapshot =
            serde_json::from_value(response.result.expect("snapshot")).expect("snapshot payload");
        assert_eq!(snapshot.revision, 1);
    }

    #[test]
    fn wait_connection_timeout_reports_observed_sequence() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        // 基线等于当前计数:状态匹配也不得命中,只能等到超时。
        let response = exchange_json(
            address,
            &request(
                "4242-41",
                "pane.wait",
                json!({ "pane_id": 1, "state": "idle", "timeout_ms": 120, "after_seq": 1 }),
            ),
        );
        let error = response.error.expect("timeout error");
        assert_eq!(error.code, "timeout");
        let details = error.details.expect("details");
        assert_eq!(details["after_seq"], 1);
        assert_eq!(details["observed_state_change_seq"], 1);
    }

    #[test]
    fn wait_connection_rejects_out_of_range_timeout() {
        for timeout_ms in [0, 86_400_001] {
            let (sink, _events) = sink_channel();
            let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
            let response = exchange_json(
                address,
                &request(
                    "4242-42",
                    "pane.wait",
                    json!({ "pane_id": 1, "state": "idle", "timeout_ms": timeout_ms }),
                ),
            );
            assert_eq!(response.error.expect("error").code, "invalid_params");
        }
    }

    #[test]
    fn wait_connection_lifecycle_error_precedes_timeout() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Running,
        )]));
        hub.record_pane_exited(super::super::SINGLE_WINDOW_ID, 1);
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(
            address,
            &request(
                "4242-43",
                "pane.wait",
                json!({ "pane_id": 1, "state": "idle", "timeout_ms": 5_000 }),
            ),
        );
        assert_eq!(response.error.expect("error").code, "pane_exited");
    }

    #[test]
    fn wait_connection_wakes_on_later_transition() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Running,
        )]));
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub.clone());
        let waiter = std::thread::spawn(move || {
            exchange_json(
                address,
                &request(
                    "4242-44",
                    "pane.wait",
                    json!({ "pane_id": 1, "state": "settled", "timeout_ms": 5_000 }),
                ),
            )
        });
        // 无论等待方先订阅还是先读到 current,状态归位后都必须命中。
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Finished,
        )]));
        let response = waiter.join().expect("waiter thread");
        assert!(response.ok, "{:?}", response.error);
        let snapshot: RuntimeSnapshot =
            serde_json::from_value(response.result.expect("snapshot")).expect("snapshot payload");
        assert_eq!(snapshot.revision, 2);
    }

    /// 三向一致:capabilities == Schema 方法枚举 == 可路由方法。
    /// 未路由的方法只可能是 `method_not_found`。
    #[test]
    fn capabilities_schema_and_routing_table_are_three_way_consistent() {
        let schema: Value =
            serde_json::from_str(include_str!("../../../assets/runtime-api-v1.schema.json"))
                .expect("protocol schema must stay valid JSON");
        let methods: Vec<&str> = schema["$defs"]["request"]["properties"]["method"]["enum"]
            .as_array()
            .expect("method enum")
            .iter()
            .map(|method| method.as_str().expect("method string"))
            .collect();
        assert_eq!(methods, CAPABILITIES);

        for method in methods {
            match parse_method(method, &json!({})) {
                Ok(_) => {}
                Err(error) => assert_ne!(
                    error.code, "method_not_found",
                    "capability {method} is not routable"
                ),
            }
            // 已路由方法对未知参数一律 invalid_params,不会静默忽略。
            let mut params = json!({ "oracle": true });
            if matches!(
                method,
                "runtime.describe" | "runtime.snapshot" | "events.subscribe" | "agents.list"
            ) {
                params = json!({ "oracle": true });
            }
            assert_eq!(
                parse_method(method, &params)
                    .err()
                    .unwrap_or_else(|| panic!("{method} must reject unknown params"))
                    .code,
                "invalid_params",
                "{method}"
            );
        }

        for trimmed in [
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
            assert_eq!(
                parse_method(trimmed, &json!({}))
                    .expect_err("trimmed method")
                    .code,
                "method_not_found",
                "{trimmed}"
            );
        }
    }

    #[test]
    fn runtime_snapshot_is_routed_to_the_shell_owner() {
        let hub = RuntimeHub::new();
        hub.publish(snapshot_with_panes(vec![pane(
            1,
            super::super::RuntimeTaskState::Idle,
        )]));
        let sink = sink_responding(|command| match command {
            RuntimeCommand::Snapshot => Ok(json!({ "revision": 7, "served": true })),
            other => panic!("unexpected command: {other:?}"),
        });
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(address, &request("4242-50", "runtime.snapshot", json!({})));
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(response.result.expect("snapshot result")["served"], true);
    }

    #[test]
    fn git_status_is_routed_to_the_git_command_family() {
        let sink = sink_responding(|command| match command {
            RuntimeCommand::Git { request, .. } => {
                assert_eq!(request.operation, "status");
                assert_eq!(request.expected_cwd, "D:/repo");
                Ok(json!({ "action": { "root": "D:/repo" } }))
            }
            other => panic!("unexpected command: {other:?}"),
        });
        let address = spawn_single_connection_server(TEST_TOKEN, sink, RuntimeHub::new());
        let response = exchange_json(
            address,
            &request(
                "4242-51",
                "git.status",
                json!({ "window_id": 1, "pane_id": 2, "expected_cwd": "D:/repo" }),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(
            response.result.expect("git result")["action"]["root"],
            "D:/repo"
        );
    }

    #[test]
    fn dispatch_timeout_defers_worktree_cleanup() {
        let repository = test_git_repository();
        let target = repository.path().join("worktrees").join("stuck");
        let hub = RuntimeHub::new();
        let command = RuntimeCommand::from_params(
            "agent.fork",
            &json!({
                "source_cwd": repository.path(),
                "name": "stuck",
                "kind": "codex",
                "branch": "slterm/stuck",
                "path": target
            }),
        )
        .expect("agent.fork parses");
        let (sink, held) = sink_holding();
        let error =
            dispatch_runtime_command_with_timeout(command, &sink, &hub, Duration::from_millis(50))
                .expect_err("UI did not answer");
        assert_eq!(error.code, "runtime_timeout");
        let details = error.details.expect("timeout details");
        assert_eq!(details["cleanup_deferred"], true);
        assert_eq!(
            details["worktree"]["path"],
            target.to_string_lossy().as_ref()
        );
        assert!(
            target.join("tracked.txt").is_file(),
            "deferred cleanup must keep the checkout for a late PTY"
        );
        assert!(branch_exists(repository.path(), "slterm/stuck"));
        // 释放 dispatch(= UI 侧最终丢弃),随后 tempdir 随用例回收。
        held.lock().expect("held dispatches").clear();
    }

    #[test]
    fn dispatch_failure_rolls_back_the_prepared_worktree() {
        let repository = test_git_repository();
        let target = repository.path().join("worktrees").join("failed");
        let command = RuntimeCommand::from_params(
            "agent.fork",
            &json!({
                "source_cwd": repository.path(),
                "name": "failed",
                "kind": "codex",
                "branch": "slterm/failed",
                "path": target
            }),
        )
        .expect("agent.fork parses");
        let sink = sink_responding(|command| {
            assert!(matches!(
                command,
                RuntimeCommand::AgentStart {
                    worktree: Some(_),
                    ..
                }
            ));
            Err(ApiError::new(
                "action_failed",
                "simulated UI launch failure",
            ))
        });
        let error = dispatch_runtime_command_with_timeout(
            command,
            &sink,
            &RuntimeHub::new(),
            Duration::from_secs(5),
        )
        .expect_err("UI failure");
        assert_eq!(error.code, "action_failed");
        assert!(
            !target.exists(),
            "failed dispatch must roll back the checkout"
        );
        assert!(!branch_exists(repository.path(), "slterm/failed"));
    }

    #[test]
    fn dispatch_success_commits_the_worktree_and_attaches_provenance() {
        let repository = test_git_repository();
        let target = repository.path().join("worktrees").join("ready");
        let command = RuntimeCommand::from_params(
            "agent.fork",
            &json!({
                "source_cwd": repository.path(),
                "name": "ready",
                "kind": "codex",
                "branch": "slterm/ready",
                "path": target
            }),
        )
        .expect("agent.fork parses");
        let sink = sink_responding(|command| {
            let RuntimeCommand::AgentStart { worktree, .. } = command else {
                panic!("agent.fork must become AgentStart");
            };
            assert!(worktree.is_some());
            Ok(json!({ "action": { "pane_id": 4 } }))
        });
        let result = dispatch_runtime_command_with_timeout(
            command,
            &sink,
            &RuntimeHub::new(),
            Duration::from_secs(5),
        )
        .expect("dispatched");
        assert_eq!(
            result["action"]["worktree"]["path"],
            target.to_string_lossy().as_ref()
        );
        assert!(target.join("tracked.txt").is_file());
        assert!(branch_exists(repository.path(), "slterm/ready"));
    }

    #[test]
    fn dropped_dispatch_reports_runtime_unavailable_and_rolls_back() {
        let repository = test_git_repository();
        let target = repository.path().join("worktrees").join("dropped");
        let command = RuntimeCommand::from_params(
            "agent.fork",
            &json!({
                "source_cwd": repository.path(),
                "name": "dropped",
                "kind": "codex",
                "branch": "slterm/dropped",
                "path": target
            }),
        )
        .expect("agent.fork parses");
        // 闭包没有保存 dispatch:回调返回时 Arc 被丢弃 → Disconnected。
        let sink: EventSink = Arc::new(|_callback| {});
        let error = dispatch_runtime_command_with_timeout(
            command,
            &sink,
            &RuntimeHub::new(),
            Duration::from_secs(5),
        )
        .expect_err("dropped dispatch");
        assert_eq!(error.code, "runtime_unavailable");
        assert!(!target.exists());
        assert!(!branch_exists(repository.path(), "slterm/dropped"));
    }

    #[test]
    fn agent_delegate_dispatches_a_generation_bound_prompt() {
        let (hub, target, _snapshot) = delegation_fixture();
        let expected_agent_id = target.agent_id.clone();
        let expected_generation = target.generation;
        let sink = sink_responding(move |command| {
            assert!(matches!(
                command,
                RuntimeCommand::AgentPrompt {
                    agent,
                    generation: Some(generation),
                    text,
                    submit: true,
                } if agent == &expected_agent_id
                    && *generation == expected_generation
                    && text == "inspect the build"
            ));
            Ok(json!({ "submitted": true }))
        });
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(
            address,
            &request(
                "4242-60",
                "agent.delegate",
                json!({
                    "agent": target.agent_id,
                    "generation": target.generation,
                    "text": "inspect the build",
                    "origin_pane_id": 3
                }),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(
            response.result.expect("delegate result")["delegation"]["status"],
            "submitted"
        );
    }

    #[test]
    fn agent_wait_identity_failure_is_immediate() {
        let hub = RuntimeHub::new();
        let agent = hub
            .register_agent(
                "worker".to_owned(),
                crate::ai_agents::AgentKind::Codex,
                super::super::SINGLE_WINDOW_ID,
                4,
                Some(TEST_CODEX_SESSION.to_owned()),
                None,
            )
            .expect("register");
        hub.publish(snapshot_with_panes(vec![pane_with_agent(
            4,
            detected_agent("codex", Some(TEST_CODEX_SESSION)),
            super::super::RuntimeTaskState::Idle,
        )]));
        hub.close_agent(&agent.agent_id, "agent_exited");
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(
            address,
            &request(
                "4242-61",
                "agent.wait",
                json!({
                    "agent": agent.agent_id,
                    "generation": agent.generation,
                    "state": "idle",
                    "timeout_ms": 500
                }),
            ),
        );
        assert_eq!(response.error.expect("error").code, "agent_exited");
    }

    #[test]
    fn agent_wait_completes_on_a_matching_transition() {
        let (hub, agent, _snapshot) = delegation_fixture();
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub.clone());
        let waiter = std::thread::spawn(move || {
            exchange_json(
                address,
                &request(
                    "4242-62",
                    "agent.wait",
                    json!({
                        "agent": agent.agent_id,
                        "generation": agent.generation,
                        "state": "running",
                        "timeout_ms": 5_000
                    }),
                ),
            )
        });
        hub.publish(snapshot_with_panes(vec![
            pane_with_agent(
                3,
                detected_agent("claude", Some("claude-session-1")),
                super::super::RuntimeTaskState::Finished,
            ),
            pane_with_agent(
                4,
                detected_agent("codex", Some(TEST_CODEX_SESSION)),
                super::super::RuntimeTaskState::Running,
            ),
        ]));
        let response = waiter.join().expect("waiter thread");
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.expect("agent wait result");
        assert_eq!(result["agent"]["observed"], true);
        assert_eq!(
            result["snapshot"]["windows"][0]["tabs"][0]["panes"][1]["task_state"],
            "running"
        );
    }

    #[test]
    fn agents_list_reports_revision_and_rows() {
        let (hub, _target, _snapshot) = delegation_fixture();
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink, hub);
        let response = exchange_json(address, &request("4242-63", "agents.list", json!({})));
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.expect("agents list result");
        assert_eq!(result["revision"], 1);
        assert_eq!(result["agents"].as_array().expect("agent rows").len(), 2);
        assert_eq!(result["agents"][1]["pane_id"], 4);
        assert_eq!(result["agents"][1]["agent"]["kind"], "codex");
    }
}
