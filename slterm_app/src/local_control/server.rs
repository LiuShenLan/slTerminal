//! loopback 控制面服务器与连接生命周期。

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use slterm_runtime_protocol::{
    AgentCloseParams, AgentForkParams, AgentGetRequest, AgentInputParams, AgentListResponse,
    AgentReadRequest, AgentResource, AgentResumeParams, AgentStartParams, AgentWaitRequest,
    AppCloseParams, AppFocusParams, ClientRequest, ControlMethod, ControlScope, Credential,
    DescribeResponse, DomainErrorCode, EventNotification, EventsSubscribeParams,
    EventsUnsubscribedResponse, GitDeleteParams, GitDiffRequest, GitDiffResponse,
    GitOpenDeletedParams, GitOpenDiffParams, GitRollbackParams, GitStatusRequest,
    GitStatusResponse, HandoverParams, HandoverResponse, HelloResponse, JsonRpcError,
    JsonRpcErrorData, JsonRpcId, JsonRpcResponse, OPERATION_WAIT_LIMIT_MS, OperationQueryParams,
    OperationResource, OperationWaitParams, PanelActivateParams, PanelCloseParams,
    PanelGroupMergeParams, PanelGroupResizeParams, PanelGroupSplitParams, PanelMoveParams,
    PanelOpenFileParams, PanelOpenSettingsParams, PanelSaveParams, ProjectActivateParams,
    ProjectListResponse, ProjectRemoveParams, ProjectRenameParams, ProjectReorderParams,
    RuntimeEvent, RuntimeMethod, RuntimeRequest, RuntimeResult, SnapshotResources,
    SnapshotResponse, SplitDirectionWire, TerminalCloseParams, TerminalCreateParams,
    TerminalExecuteParams, TerminalInputParams, TerminalReadRequest, TerminalReadResponse,
    TerminalReconnectParams, TerminalRenameParams, TerminalWaitRequest, TerminalWaitResponse,
    ViewportSize, parse_client_request,
};
use slterm_split::SplitDirection;
use slterm_workspace::{LogicalSize, PanelGroupId, ProjectId, ViewKey};

use super::events::Subscription;
use super::mutations::{RuntimeMutation, RuntimeMutationRequest};
use super::reads::{
    PAGE_BUDGET_BYTES, ReadError, ReadGeneration, TerminalReadTarget, TerminalTextSnapshot,
    read_page, resource_token,
};
use crate::secrets::SecretBytes;

const REQUEST_BUDGET: usize = 128 * 1024;
const RESPONSE_BUDGET: usize = 1024 * 1024;
const CONNECTION_LIMIT: usize = 64;
pub(super) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const READ_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 只监听 loopback 的控制面；所有线程与活跃流均由本对象统一回收。
pub(crate) struct LocalControlServer {
    endpoint: String,
    shutdown: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
    workers: Arc<Mutex<BTreeMap<u64, Worker>>>,
    active_workers: Arc<AtomicUsize>,
    // join 错误只有安全静态消息；保存后可让重复 shutdown 重建同一错误。
    termination_error: Option<io::Error>,
}

struct Worker {
    stream: TcpStream,
    handle: JoinHandle<()>,
}

/// worker 结束即移除备用流；panic 路径也必须让客户端观察到真实 EOF。
struct WorkerGuard {
    id: u64,
    workers: Arc<Mutex<BTreeMap<u64, Worker>>>,
    active_workers: Arc<AtomicUsize>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.active_workers.fetch_sub(1, Ordering::SeqCst);
        self.workers
            .lock()
            .expect("worker registry poisoned")
            .remove(&self.id);
    }
}

/// Runtime 快照端口：由应用壳按请求生成同一时刻的只读资源。
pub(crate) type SnapshotCallback =
    Arc<dyn Fn() -> Result<SnapshotResources, JsonRpcError> + Send + Sync + 'static>;

/// Runtime Operation 查询端口：只经唯一工作区写线程读取持久结果。
pub(crate) type OperationQueryCallback =
    Arc<dyn Fn(&str) -> Result<Option<OperationResource>, JsonRpcError> + Send + Sync + 'static>;

/// Runtime 事件订阅端口：校验三元组并注册容量 64 的订阅队列。
pub(crate) type EventSubscribeCallback = Arc<
    dyn Fn(EventsSubscribeParams) -> Result<Subscription, JsonRpcError> + Send + Sync + 'static,
>;

/// Runtime 写意图端口：登记、执行与终态全部经唯一派发器。
pub(crate) type MutationCallback =
    Arc<dyn Fn(RuntimeMutationRequest) -> Result<OperationResource, JsonRpcError> + Send + Sync>;

/// Runtime 终端读取端口：只返回前台复核对代次后的不可变文本快照。
pub(crate) type ReadCallback =
    Arc<dyn Fn(TerminalReadTarget) -> Result<TerminalTextSnapshot, JsonRpcError> + Send + Sync>;

/// 终端等待状态目标：稳定身份、PTY 代次与可选 Agent 身份。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalStatusTarget {
    pub(crate) terminal_id: slterm_workspace::TerminalId,
    pub(crate) generation: u64,
    /// `Some` 表示 agent.wait，必须再匹配该终端当前对外身份。
    pub(crate) agent_id: Option<String>,
}

/// Runtime 终端等待端口：读取前台最近发布帧的 revision。
pub(crate) type WaitStatusCallback =
    Arc<dyn Fn(TerminalStatusTarget) -> Result<u64, JsonRpcError> + Send + Sync>;

/// Runtime Agent 资源端口：由前台持久会话身份与运行期状态派生。
pub(crate) type AgentsCallback =
    Arc<dyn Fn() -> Result<Vec<AgentResource>, JsonRpcError> + Send + Sync>;

/// Runtime Git 状态端口：目标解析在前台，仓库读取在后台执行体。
pub(crate) type GitStatusCallback = Arc<
    dyn Fn(Option<slterm_workspace::ProjectId>) -> Result<GitStatusResponse, JsonRpcError>
        + Send
        + Sync,
>;

/// Runtime Git diff 端口：返回实际项目身份与 diff 文本快照。
pub(crate) type GitDiffCallback = Arc<
    dyn Fn(
            Option<slterm_workspace::ProjectId>,
            String,
        )
            -> Result<(slterm_workspace::ProjectId, super::git::GitDiffSnapshot), JsonRpcError>
        + Send
        + Sync,
>;

/// Runtime 作用域的应用回调集合。
pub(crate) struct RuntimeCallbacks {
    pub(crate) snapshot: SnapshotCallback,
    /// 事件环当前末尾 revision；快照与事件订阅必须共用同一代次。
    pub(crate) event_revision: Arc<dyn Fn() -> u64 + Send + Sync>,
    pub(crate) operation: OperationQueryCallback,
    pub(crate) events: EventSubscribeCallback,
    pub(crate) mutation: MutationCallback,
    pub(crate) read: ReadCallback,
    pub(crate) wait_status: WaitStatusCallback,
    pub(crate) agents: AgentsCallback,
    pub(crate) git_status: GitStatusCallback,
    pub(crate) git_diff: GitDiffCallback,
}

/// operation.wait 轮询间隔：只做有界查询与关闭检查。
const OPERATION_WAIT_POLL: Duration = Duration::from_millis(100);

impl LocalControlServer {
    /// 只服务 handover 作用域（测试与无 Runtime 组合）。
    pub(crate) fn start<F>(
        credential: SecretBytes,
        hello: HelloResponse,
        on_handover: F,
    ) -> io::Result<Self>
    where
        F: Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static,
    {
        Self::start_scoped(
            credential,
            None,
            hello,
            on_handover,
            RuntimeCallbacks {
                event_revision: Arc::new(|| 0),
                snapshot: Arc::new(not_ready_snapshot),
                operation: Arc::new(not_ready_operation),
                events: Arc::new(not_ready_events),
                mutation: Arc::new(not_ready_mutation),
                read: Arc::new(not_ready_read),
                wait_status: Arc::new(not_ready_wait_status),
                agents: Arc::new(not_ready_agents),
                git_status: Arc::new(not_ready_git_status),
                git_diff: Arc::new(not_ready_git_diff),
            },
        )
    }

    /// 同一端点服务 Runtime 与 handover 两个作用域，凭据各自独立。
    pub(crate) fn start_with_runtime<F>(
        handover_credential: SecretBytes,
        runtime_credential: SecretBytes,
        hello: HelloResponse,
        on_handover: F,
        callbacks: RuntimeCallbacks,
    ) -> io::Result<Self>
    where
        F: Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static,
    {
        Self::start_scoped(
            handover_credential,
            Some(runtime_credential),
            hello,
            on_handover,
            callbacks,
        )
    }

    fn start_scoped<F>(
        credential: SecretBytes,
        runtime_credential: Option<SecretBytes>,
        hello: HelloResponse,
        on_handover: F,
        callbacks: RuntimeCallbacks,
    ) -> io::Result<Self>
    where
        F: Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static,
    {
        let listener = TcpListener::bind((IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = listener.local_addr()?.to_string();
        let shutdown = Arc::new(AtomicBool::new(false));
        let workers = Arc::new(Mutex::new(BTreeMap::new()));
        let active_workers = Arc::new(AtomicUsize::new(0));
        let callback = Arc::new(on_handover);
        let credentials = Arc::new(ScopedCredentials {
            handover: credential,
            runtime: runtime_credential,
        });
        let accept = {
            let shutdown = Arc::clone(&shutdown);
            let workers = Arc::clone(&workers);
            let active_workers = Arc::clone(&active_workers);
            std::thread::Builder::new()
                .name("slterm-control-accept".to_owned())
                .spawn(move || {
                    accept_loop(
                        listener,
                        credentials,
                        hello,
                        ConnectionHandlers {
                            handover: callback,
                            runtime: Arc::new(callbacks),
                        },
                        shutdown,
                        workers,
                        active_workers,
                    )
                })?
        };
        Ok(Self {
            endpoint,
            shutdown,
            accept: Some(accept),
            workers,
            active_workers,
            termination_error: None,
        })
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn active_workers(&self) -> usize {
        self.active_workers.load(Ordering::SeqCst)
    }

    /// 停止接收新请求，断开活跃连接并等待全部线程退出。
    pub(crate) fn shutdown(&mut self) -> io::Result<()> {
        self.stop()
    }

    fn stop(&mut self) -> io::Result<()> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            // 首次关闭已取走并回收全部资源；历史 join 失败只记录一次，
            // 不把重复关闭永久判为失败，否则正常退出无法重试完成。
            if let Some(error) = &self.termination_error {
                log::warn!("control plane previously failed during shutdown: {error}");
            }
            return Ok(());
        }
        let mut first_error = None;
        if let Some(accept) = self.accept.take()
            && let Err(error) = join(accept)
        {
            first_error = Some(error);
        }
        let workers = std::mem::take(&mut *self.workers.lock().expect("worker registry poisoned"));
        for (_, worker) in workers {
            let _ = worker.stream.shutdown(Shutdown::Both);
            if let Err(error) = join(worker.handle)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            self.termination_error = Some(io::Error::new(error.kind(), error.to_string()));
            Err(error)
        } else {
            Ok(())
        }
    }
}

impl Drop for LocalControlServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// 同一端点的两项作用域凭据句柄；未开放的作用域为 None。
struct ScopedCredentials {
    handover: SecretBytes,
    runtime: Option<SecretBytes>,
}

/// 每个连接共享的应用回调；避免把回调拆成更多线程参数。
struct ConnectionHandlers<F> {
    handover: Arc<F>,
    runtime: Arc<RuntimeCallbacks>,
}

impl ScopedCredentials {
    fn for_scope(&self, scope: ControlScope) -> Option<&SecretBytes> {
        match scope {
            ControlScope::Handover => Some(&self.handover),
            ControlScope::Runtime => self.runtime.as_ref(),
        }
    }
}

fn accept_loop<F>(
    listener: TcpListener,
    credentials: Arc<ScopedCredentials>,
    hello: HelloResponse,
    handlers: ConnectionHandlers<F>,
    shutdown: Arc<AtomicBool>,
    workers: Arc<Mutex<BTreeMap<u64, Worker>>>,
    active_workers: Arc<AtomicUsize>,
) where
    F: Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static,
{
    let mut next_id = 0_u64;
    let worker_registry = Arc::clone(&workers);
    for incoming in listener.incoming() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(READ_POLL_INTERVAL);
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(error) => {
                if !shutdown.load(Ordering::SeqCst) {
                    log::warn!("控制面接受连接失败: {error}");
                    std::thread::sleep(READ_POLL_INTERVAL);
                }
                continue;
            }
        };
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        {
            let mut workers = workers.lock().expect("worker registry poisoned");
            workers.retain(|_, worker| !worker.handle.is_finished());
            if workers.len() >= CONNECTION_LIMIT {
                continue;
            }
            next_id += 1;
            let worker_stream = match stream.try_clone() {
                Ok(stream) => stream,
                Err(error) => {
                    log::warn!("控制面连接句柄复制失败: {error}");
                    continue;
                }
            };
            let callback = Arc::clone(&handlers.handover);
            let runtime = Arc::clone(&handlers.runtime);
            let credentials = credentials.clone();
            let hello = hello.clone();
            let shutdown = Arc::clone(&shutdown);
            let workers_for_thread = Arc::clone(&worker_registry);
            let active_workers_for_thread = Arc::clone(&active_workers);
            active_workers.fetch_add(1, Ordering::SeqCst);
            let handle = std::thread::Builder::new()
                .name(format!("slterm-control-{next_id}"))
                .spawn(move || {
                    let _guard = WorkerGuard {
                        id: next_id,
                        workers: workers_for_thread,
                        active_workers: active_workers_for_thread,
                    };
                    handle_connection(stream, credentials, hello, callback, runtime, shutdown)
                });
            match handle {
                Ok(handle) => {
                    workers.insert(
                        next_id,
                        Worker {
                            stream: worker_stream,
                            handle,
                        },
                    );
                }
                Err(error) => {
                    active_workers.fetch_sub(1, Ordering::SeqCst);
                    log::warn!("控制面连接线程创建失败: {error}");
                }
            }
        }
    }
}

fn handle_connection<F>(
    mut stream: TcpStream,
    credentials: Arc<ScopedCredentials>,
    hello: HelloResponse,
    callback: Arc<F>,
    runtime: Arc<RuntimeCallbacks>,
    shutdown: Arc<AtomicBool>,
) where
    F: Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static,
{
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(READ_POLL_INTERVAL)).is_err()
        || stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
    {
        return;
    }
    let mut buffer = Vec::new();
    let Some(frame) = read_frame(
        &mut stream,
        &mut buffer,
        Instant::now() + HANDSHAKE_TIMEOUT,
        &shutdown,
    )
    .ok()
    .flatten() else {
        return;
    };
    let Ok(ClientRequest::Hello(request)) = parse_client_request(&frame) else {
        return;
    };
    // 首包作用域决定本连接凭据来源；Runtime 作用域只在该作用域开放时存在。
    let scope = request.params.scope;
    let authorized = credentials
        .for_scope(scope)
        .is_some_and(|expected| credential_matches(expected, &request.params.credential));
    if !authorized {
        return;
    }
    let hello = hello.for_scope(scope);
    if write_json(
        &mut stream,
        &JsonRpcResponse::result(request.id, hello.clone()),
    )
    .is_err()
    {
        return;
    }
    loop {
        let Some(frame) = read_frame(
            &mut stream,
            &mut buffer,
            Instant::now() + REQUEST_TIMEOUT,
            &shutdown,
        )
        .ok()
        .flatten() else {
            return;
        };
        // 认证后参数错误必须回结构化 InvalidRequest 并保留连接；只有
        // 无法解析 id 时才断开。作用域不匹配仍由下方直接断开。
        let request = match parse_client_request(&frame) {
            Ok(request) => request,
            Err(_) => {
                if write_protocol_error(&mut stream, &frame).is_err() {
                    return;
                }
                continue;
            }
        };
        if shutdown.load(Ordering::SeqCst) {
            let _ = match request {
                ClientRequest::Runtime(request) => write_json(
                    &mut stream,
                    &JsonRpcResponse::<RuntimeResult>::error(request.id, closing_error()),
                ),
                ClientRequest::Handover(request) => write_json(
                    &mut stream,
                    &JsonRpcResponse::<HandoverResponse>::error(request.id, closing_error()),
                ),
                ClientRequest::Hello(request) => write_json(
                    &mut stream,
                    &JsonRpcResponse::<HelloResponse>::error(request.id, closing_error()),
                ),
            };
            return;
        }
        match request {
            ClientRequest::Runtime(request) => {
                if scope != ControlScope::Runtime {
                    return;
                }
                // 路由与能力摘要共用方法注册表作用域；未登记或不匹配即拒绝，
                // 不依赖请求枚举变体形成第二套作用域真值。
                let method_name = match serde_json::to_value(request.method) {
                    Ok(serde_json::Value::String(name)) => name,
                    _ => return,
                };
                if !slterm_runtime_protocol::lookup(&method_name)
                    .is_some_and(|spec| spec.scopes.contains(&ControlScope::Runtime))
                {
                    if write_json(
                        &mut stream,
                        &JsonRpcResponse::<RuntimeResult>::error(
                            request.id,
                            invalid_request_error(),
                        ),
                    )
                    .is_err()
                    {
                        return;
                    }
                    continue;
                }
                let response_id = request.id.clone();
                let response = match request.method {
                    // describe 只读方法注册表与握手同一散列，不需要回前台。
                    RuntimeMethod::Describe => JsonRpcResponse::result(
                        request.id,
                        RuntimeResult::Describe(DescribeResponse::for_scope(
                            hello.protocol_sha256.clone(),
                            hello.schema_sha256.clone(),
                            scope,
                        )),
                    ),
                    // 事件环接入前，revision 固定为握手起点；Hub 切片接管实时代次。
                    RuntimeMethod::Snapshot => match (runtime.snapshot)() {
                        Ok(resources) => JsonRpcResponse::result(
                            request.id,
                            RuntimeResult::Snapshot(SnapshotResponse {
                                store_epoch: hello.store_epoch.clone(),
                                stream_epoch: hello.stream_epoch.clone(),
                                revision: (runtime.event_revision)(),
                                resources,
                            }),
                        ),
                        Err(error) => JsonRpcResponse::error(request.id, error),
                    },
                    RuntimeMethod::OperationGet => {
                        let Some(params) = request.parse::<OperationQueryParams>() else {
                            return;
                        };
                        match (runtime.operation)(&params.operation_id) {
                            Ok(Some(resource)) => JsonRpcResponse::result(
                                request.id,
                                RuntimeResult::Operation(resource),
                            ),
                            Ok(None) => {
                                JsonRpcResponse::error(request.id, unknown_operation_error())
                            }
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    RuntimeMethod::AgentList => match (runtime.agents)() {
                        Ok(agents) => JsonRpcResponse::result(
                            request.id,
                            RuntimeResult::Agents(AgentListResponse { agents }),
                        ),
                        Err(error) => JsonRpcResponse::error(request.id, error),
                    },
                    RuntimeMethod::AgentGet => {
                        let Some(params) = request.parse::<AgentGetRequest>() else {
                            return;
                        };
                        match (runtime.agents)() {
                            Ok(agents) => {
                                let matched: Vec<AgentResource> = agents
                                    .into_iter()
                                    .filter(|agent| agent.agent_id == params.agent_id)
                                    .collect();
                                if matched.is_empty() {
                                    JsonRpcResponse::error(request.id, unknown_agent_error())
                                } else {
                                    JsonRpcResponse::result(
                                        request.id,
                                        RuntimeResult::Agents(AgentListResponse {
                                            agents: matched,
                                        }),
                                    )
                                }
                            }
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    RuntimeMethod::AgentInput => {
                        let Some(params) = request.parse::<AgentInputParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::AgentInput {
                                    agent_id: params.request.agent_id,
                                    terminal_id,
                                    generation: params.request.generation,
                                    data: params.request.data,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::AgentClose => {
                        let Some(params) = request.parse::<AgentCloseParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::AgentClose {
                                    agent_id: params.request.agent_id,
                                    terminal_id,
                                    generation: params.request.generation,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::AgentStart => {
                        let Some(params) = request.parse::<AgentStartParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::AgentStart {
                                kind: params.request.kind,
                                project_root: params.request.project_root,
                                cwd: params.request.cwd,
                            }),
                        )
                    }
                    RuntimeMethod::AgentResume => {
                        let Some(params) = request.parse::<AgentResumeParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::AgentResume {
                                source: params.request.source,
                                session_id: params.request.session_id,
                                session_file: params.request.session_file,
                                project_root: params.request.project_root,
                                cwd: params.request.cwd,
                            }),
                        )
                    }
                    RuntimeMethod::AgentFork => {
                        let Some(params) = request.parse::<AgentForkParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::AgentFork {
                                source: params.request.source,
                                session_id: params.request.session_id,
                                project_root: params.request.project_root,
                                cwd: params.request.cwd,
                            }),
                        )
                    }
                    RuntimeMethod::GitStatus => {
                        let Some(params) = request.parse::<GitStatusRequest>() else {
                            return;
                        };
                        let project_id = match params.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        match project_id {
                            Some(project_id) => match (runtime.git_status)(project_id) {
                                Ok(status) => JsonRpcResponse::result(
                                    request.id,
                                    RuntimeResult::GitStatus(status),
                                ),
                                Err(error) => JsonRpcResponse::error(request.id, error),
                            },
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::GitDiff => {
                        let Some(params) = request.parse::<GitDiffRequest>() else {
                            return;
                        };
                        let project_id = match params.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        match project_id {
                            Some(project_id) => respond_git_diff(
                                &runtime,
                                request.id,
                                project_id,
                                params.path,
                                params.cursor.as_deref(),
                            ),
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::GitRollback => {
                        let Some(params) = request.parse::<GitRollbackParams>() else {
                            return;
                        };
                        let project_id = match params.request.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        let mutation = project_id.map(|project_id| RuntimeMutation::GitRollback {
                            project_id,
                            path: params.request.path,
                            expected: params.request.expected,
                        });
                        match mutation {
                            Some(mutation) => respond_mutation(
                                &runtime,
                                request.id,
                                params.client_id,
                                params.mutation_id,
                                params.expected_revision,
                                Ok(mutation),
                            ),
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::GitDelete => {
                        let Some(params) = request.parse::<GitDeleteParams>() else {
                            return;
                        };
                        let project_id = match params.request.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        let mutation = project_id.map(|project_id| RuntimeMutation::GitDelete {
                            project_id,
                            path: params.request.path,
                            expected: params.request.expected,
                            permanent: params.request.permanent,
                        });
                        match mutation {
                            Some(mutation) => respond_mutation(
                                &runtime,
                                request.id,
                                params.client_id,
                                params.mutation_id,
                                params.expected_revision,
                                Ok(mutation),
                            ),
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::GitOpenDiff => {
                        let Some(params) = request.parse::<GitOpenDiffParams>() else {
                            return;
                        };
                        let project_id = match params.request.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        let mutation = project_id.map(|project_id| RuntimeMutation::GitOpenDiff {
                            project_id,
                            path: params.request.path,
                        });
                        match mutation {
                            Some(mutation) => respond_mutation(
                                &runtime,
                                request.id,
                                params.client_id,
                                params.mutation_id,
                                params.expected_revision,
                                Ok(mutation),
                            ),
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::GitOpenDeleted => {
                        let Some(params) = request.parse::<GitOpenDeletedParams>() else {
                            return;
                        };
                        let project_id = match params.request.project_id.as_deref() {
                            None => Some(None),
                            Some(text) => parse_project_id(text).ok().map(Some),
                        };
                        let mutation =
                            project_id.map(|project_id| RuntimeMutation::GitOpenDeleted {
                                project_id,
                                path: params.request.path,
                            });
                        match mutation {
                            Some(mutation) => respond_mutation(
                                &runtime,
                                request.id,
                                params.client_id,
                                params.mutation_id,
                                params.expected_revision,
                                Ok(mutation),
                            ),
                            None => JsonRpcResponse::error(request.id, invalid_request_error()),
                        }
                    }
                    RuntimeMethod::PanelClose => {
                        let Some(params) = request.parse::<PanelCloseParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_view(&params.request.view).map(|view| {
                                RuntimeMutation::PanelClose {
                                    view,
                                    documents: params.request.documents,
                                }
                            }),
                        )
                    }
                    RuntimeMethod::AppFocus => {
                        let Some(params) = request.parse::<AppFocusParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::AppFocus),
                        )
                    }
                    RuntimeMethod::AppClose => {
                        let Some(params) = request.parse::<AppCloseParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::AppClose {
                                documents: params.request.documents,
                                hooks: params.request.hooks,
                            }),
                        )
                    }
                    RuntimeMethod::OperationWait => {
                        match wait_operation(&runtime, &request, &shutdown, &stream) {
                            Ok(resource) => JsonRpcResponse::result(
                                request.id,
                                RuntimeResult::Operation(resource),
                            ),
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    RuntimeMethod::EventsSubscribe => {
                        let Some(params) = request.parse::<EventsSubscribeParams>() else {
                            return;
                        };
                        match (runtime.events)(params) {
                            Ok(subscription) => {
                                let confirmed = subscription.confirmed.clone();
                                let response = JsonRpcResponse::result(
                                    request.id,
                                    RuntimeResult::Subscribed(confirmed),
                                );
                                if write_json(&mut stream, &response).is_err() {
                                    return;
                                }
                                // 订阅连接只承载事件；取消订阅或断开后结束本连接。
                                let _ = stream_subscription(
                                    &mut stream,
                                    subscription.events,
                                    &shutdown,
                                );
                                return;
                            }
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    // 未订阅连接没有可取消的订阅。
                    RuntimeMethod::EventsUnsubscribe => JsonRpcResponse::error(
                        request.id,
                        not_ready_error("no active subscription"),
                    ),
                    RuntimeMethod::ProjectList => match (runtime.snapshot)() {
                        Ok(resources) => JsonRpcResponse::result(
                            request.id,
                            RuntimeResult::Projects(ProjectListResponse {
                                projects: resources.projects,
                            }),
                        ),
                        Err(error) => JsonRpcResponse::error(request.id, error),
                    },
                    RuntimeMethod::ProjectActivate => {
                        let Some(params) = request.parse::<ProjectActivateParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_project_id(&params.request.project_id)
                                .map(|project_id| RuntimeMutation::ProjectActivate { project_id }),
                        )
                    }
                    RuntimeMethod::ProjectRename => {
                        let Some(params) = request.parse::<ProjectRenameParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_project_id(&params.request.project_id).map(|project_id| {
                                RuntimeMutation::ProjectRename {
                                    project_id,
                                    name: params.request.name,
                                }
                            }),
                        )
                    }
                    RuntimeMethod::ProjectReorder => {
                        let Some(params) = request.parse::<ProjectReorderParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_project_id(&params.request.project_id).and_then(|project_id| {
                                usize::try_from(params.request.new_index)
                                    .map(|new_index| RuntimeMutation::ProjectReorder {
                                        project_id,
                                        new_index,
                                    })
                                    .map_err(|_| invalid_request_error())
                            }),
                        )
                    }
                    RuntimeMethod::ProjectRemove => {
                        let Some(params) = request.parse::<ProjectRemoveParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_project_id(&params.request.project_id).map(|project_id| {
                                RuntimeMutation::ProjectRemove {
                                    project_id,
                                    documents: params.request.documents,
                                    hooks: params.request.hooks,
                                }
                            }),
                        )
                    }
                    RuntimeMethod::PanelGroupSplit => {
                        let Some(params) = request.parse::<PanelGroupSplitParams>() else {
                            return;
                        };
                        let mutation = parse_view(&params.request.view).and_then(|view| {
                            parse_group(&params.request.target_group).map(|target_group| {
                                RuntimeMutation::PanelGroupSplit {
                                    view,
                                    target_group,
                                    direction: split_direction(params.request.direction),
                                    ratio: params.request.ratio,
                                    viewport: logical_viewport(params.request.viewport),
                                }
                            })
                        });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::PanelGroupMerge => {
                        let Some(params) = request.parse::<PanelGroupMergeParams>() else {
                            return;
                        };
                        let mutation =
                            parse_group(&params.request.source_group).and_then(|source_group| {
                                parse_group(&params.request.target_group).map(|target_group| {
                                    RuntimeMutation::PanelGroupMerge {
                                        source_group,
                                        target_group,
                                    }
                                })
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::PanelGroupResize => {
                        let Some(params) = request.parse::<PanelGroupResizeParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::PanelGroupResize {
                                path: params.request.path,
                                ratio: params.request.ratio,
                                viewport: logical_viewport(params.request.viewport),
                            }),
                        )
                    }
                    RuntimeMethod::PanelActivate => {
                        let Some(params) = request.parse::<PanelActivateParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_view(&params.request.view)
                                .map(|view| RuntimeMutation::PanelActivate { view }),
                        )
                    }
                    RuntimeMethod::TerminalInput => {
                        let Some(params) = request.parse::<TerminalInputParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::TerminalInput {
                                    terminal_id,
                                    generation: params.request.generation,
                                    data: params.request.data,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::TerminalExecute => {
                        let Some(params) = request.parse::<TerminalExecuteParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::TerminalExecute {
                                    terminal_id,
                                    generation: params.request.generation,
                                    command: params.request.command,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::TerminalRead => {
                        let Some(params) = request.parse::<TerminalReadRequest>() else {
                            return;
                        };
                        respond_read(
                            &runtime,
                            request.id,
                            &params.terminal_id,
                            params.generation,
                            None,
                            params.cursor.as_deref(),
                        )
                    }
                    RuntimeMethod::AgentRead => {
                        let Some(params) = request.parse::<AgentReadRequest>() else {
                            return;
                        };
                        respond_read(
                            &runtime,
                            request.id,
                            &params.terminal_id,
                            params.generation,
                            Some(params.agent_id),
                            params.cursor.as_deref(),
                        )
                    }
                    RuntimeMethod::AgentWait => {
                        match wait_terminal(&runtime, &request, &shutdown, &stream) {
                            Ok(resource) => JsonRpcResponse::result(
                                request.id,
                                RuntimeResult::TerminalWait(resource),
                            ),
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    RuntimeMethod::TerminalWait => {
                        match wait_terminal(&runtime, &request, &shutdown, &stream) {
                            Ok(resource) => JsonRpcResponse::result(
                                request.id,
                                RuntimeResult::TerminalWait(resource),
                            ),
                            Err(error) => JsonRpcResponse::error(request.id, error),
                        }
                    }
                    RuntimeMethod::TerminalClose => {
                        let Some(params) = request.parse::<TerminalCloseParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::TerminalClose {
                                    terminal_id,
                                    generation: params.request.generation,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::TerminalReconnect => {
                        let Some(params) = request.parse::<TerminalReconnectParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::TerminalReconnect {
                                    terminal_id,
                                    generation: params.request.generation,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::TerminalRename => {
                        let Some(params) = request.parse::<TerminalRenameParams>() else {
                            return;
                        };
                        let mutation =
                            parse_terminal_id(&params.request.terminal_id).map(|terminal_id| {
                                RuntimeMutation::TerminalRename {
                                    terminal_id,
                                    title: params.request.title,
                                }
                            });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                    RuntimeMethod::TerminalCreate => {
                        let Some(params) = request.parse::<TerminalCreateParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::TerminalCreate {
                                project_root: params.request.project_root,
                                cwd: params.request.cwd,
                            }),
                        )
                    }
                    RuntimeMethod::PanelOpenSettings => {
                        let Some(params) = request.parse::<PanelOpenSettingsParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::PanelOpenSettings),
                        )
                    }
                    RuntimeMethod::PanelOpenFile => {
                        let Some(params) = request.parse::<PanelOpenFileParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            Ok(RuntimeMutation::PanelOpenFile {
                                path: params.request.path,
                            }),
                        )
                    }
                    RuntimeMethod::PanelSave => {
                        let Some(params) = request.parse::<PanelSaveParams>() else {
                            return;
                        };
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            parse_view(&params.request.view)
                                .map(|view| RuntimeMutation::PanelSave { view }),
                        )
                    }
                    RuntimeMethod::PanelMove => {
                        let Some(params) = request.parse::<PanelMoveParams>() else {
                            return;
                        };
                        let mutation = parse_view(&params.request.view).and_then(|view| {
                            parse_group(&params.request.target_group).and_then(|target_group| {
                                usize::try_from(params.request.target_index)
                                    .map(|target_index| RuntimeMutation::PanelMove {
                                        view,
                                        target_group,
                                        target_index,
                                    })
                                    .map_err(|_| invalid_request_error())
                            })
                        });
                        respond_mutation(
                            &runtime,
                            request.id,
                            params.client_id,
                            params.mutation_id,
                            params.expected_revision,
                            mutation,
                        )
                    }
                };
                let response = enforce_response_budget(response, response_id);
                if write_json(&mut stream, &response).is_err() {
                    return;
                }
            }
            ClientRequest::Handover(request) => {
                if scope != ControlScope::Handover
                    || !slterm_runtime_protocol::allows(
                        ControlMethod::InstanceHandover,
                        ControlScope::Handover,
                    )
                {
                    return;
                }
                let response = match callback(request.params) {
                    Ok(result) if result.is_accepted() => {
                        JsonRpcResponse::result(request.id, result)
                    }
                    Ok(_) => JsonRpcResponse::error(
                        request.id,
                        JsonRpcError::new(
                            -32000,
                            "handover rejected",
                            Some(JsonRpcErrorData {
                                code: DomainErrorCode::InvalidRequest,
                            }),
                        ),
                    ),
                    Err(error) => JsonRpcResponse::error(request.id, error),
                };
                if write_json(&mut stream, &response).is_err() {
                    return;
                }
            }
            // 握手后再次 hello 不属于已认证会话的合法方法序列。
            ClientRequest::Hello(_) => return,
        }
    }
}

fn closing_error() -> JsonRpcError {
    JsonRpcError::new(
        -32004,
        "closing",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::Closing,
        }),
    )
}

fn not_ready_error(message: &'static str) -> JsonRpcError {
    JsonRpcError::new(
        -32001,
        message,
        Some(JsonRpcErrorData {
            code: DomainErrorCode::NotReady,
        }),
    )
}

fn unknown_operation_error() -> JsonRpcError {
    JsonRpcError::new(
        -32000,
        "unknown operation",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::UnknownOperation,
        }),
    )
}

/// 无 Runtime 组合下的快照端口：明确 NotReady，不伪造空资源。
fn not_ready_snapshot() -> Result<SnapshotResources, JsonRpcError> {
    Err(not_ready_error("snapshot not ready"))
}

fn not_ready_operation(_: &str) -> Result<Option<OperationResource>, JsonRpcError> {
    Err(not_ready_error("operation query not ready"))
}

fn not_ready_events(_: EventsSubscribeParams) -> Result<Subscription, JsonRpcError> {
    Err(not_ready_error("event subscription not ready"))
}

fn not_ready_mutation(_: RuntimeMutationRequest) -> Result<OperationResource, JsonRpcError> {
    Err(not_ready_error("mutation not ready"))
}

fn not_ready_read(_: TerminalReadTarget) -> Result<TerminalTextSnapshot, JsonRpcError> {
    Err(not_ready_error("terminal read not ready"))
}

fn not_ready_wait_status(_: TerminalStatusTarget) -> Result<u64, JsonRpcError> {
    Err(not_ready_error("terminal wait not ready"))
}

fn not_ready_agents() -> Result<Vec<AgentResource>, JsonRpcError> {
    Err(not_ready_error("agent list not ready"))
}

fn not_ready_git_status(
    _: Option<slterm_workspace::ProjectId>,
) -> Result<GitStatusResponse, JsonRpcError> {
    Err(not_ready_error("git status not ready"))
}

fn not_ready_git_diff(
    _: Option<slterm_workspace::ProjectId>,
    _: String,
) -> Result<(slterm_workspace::ProjectId, super::git::GitDiffSnapshot), JsonRpcError> {
    Err(not_ready_error("git diff not ready"))
}

/// terminal.read、agent.read 与 git.diff 共用有界页面响应。
fn respond_git_diff(
    runtime: &RuntimeCallbacks,
    id: JsonRpcId,
    project_id: Option<slterm_workspace::ProjectId>,
    path: String,
    cursor: Option<&str>,
) -> JsonRpcResponse<RuntimeResult> {
    let (project_id, snapshot) = match (runtime.git_diff)(project_id, path.clone()) {
        Ok(result) => result,
        Err(error) => return JsonRpcResponse::error(id, error),
    };
    match read_page(
        snapshot.text.as_bytes(),
        ReadGeneration {
            resource: resource_token(&format!("git:{project_id}:{path}")),
            generation: 0,
            revision: snapshot.revision,
        },
        cursor,
        PAGE_BUDGET_BYTES,
    ) {
        Ok(page) => match String::from_utf8(page.bytes) {
            Ok(data) => JsonRpcResponse::result(
                id,
                RuntimeResult::GitDiff(GitDiffResponse {
                    project_id: project_id.to_string(),
                    path,
                    revision: snapshot.revision,
                    data,
                    next_cursor: page.next_cursor,
                }),
            ),
            Err(_) => JsonRpcResponse::error(id, read_encoding_error()),
        },
        Err(ReadError::StaleCursor) => JsonRpcResponse::error(id, stale_cursor_error()),
    }
}

/// 编码后超限的运行时响应降级为封闭错误；连接保留，客户端可缩小请求重试。
fn enforce_response_budget<T: Serialize>(
    response: JsonRpcResponse<T>,
    id: JsonRpcId,
) -> JsonRpcResponse<T> {
    let fits = serde_json::to_vec(&response).is_ok_and(|encoded| encoded.len() <= RESPONSE_BUDGET);
    if fits {
        response
    } else {
        JsonRpcResponse::error(id, response_too_large_error())
    }
}

fn response_too_large_error() -> JsonRpcError {
    JsonRpcError::new(
        -32008,
        "response too large",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::ResponseTooLarge,
        }),
    )
}

fn unknown_agent_error() -> JsonRpcError {
    JsonRpcError::new(
        -32000,
        "unknown agent",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::UnknownAgent,
        }),
    )
}

/// 游标失效必须显式要求重读，不得把旧页拼成完整结果。
fn stale_cursor_error() -> JsonRpcError {
    JsonRpcError::new(
        -32000,
        "stale cursor",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::StaleCursor,
        }),
    )
}

fn read_encoding_error() -> JsonRpcError {
    not_ready_error("terminal read encoding failed")
}

/// 认证后的协议解析失败：尽力从原始帧取 id，返回封闭 InvalidRequest。
fn write_protocol_error(stream: &mut TcpStream, frame: &[u8]) -> io::Result<()> {
    let value: serde_json::Value = serde_json::from_slice(frame)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid request id"))?;
    let id = value
        .get("id")
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request id"))?;
    let id: JsonRpcId = serde_json::from_value(id)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid request id"))?;
    write_json(
        stream,
        &JsonRpcResponse::<RuntimeResult>::error(id, invalid_request_error()),
    )
}

fn invalid_request_error() -> JsonRpcError {
    JsonRpcError::new(
        -32000,
        "invalid request",
        Some(JsonRpcErrorData {
            code: DomainErrorCode::InvalidRequest,
        }),
    )
}

fn parse_project_id(text: &str) -> Result<ProjectId, JsonRpcError> {
    text.parse::<ProjectId>()
        .map_err(|_| invalid_request_error())
}

fn parse_terminal_id(text: &str) -> Result<slterm_workspace::TerminalId, JsonRpcError> {
    text.parse::<slterm_workspace::TerminalId>()
        .map_err(|_| invalid_request_error())
}

fn parse_view(text: &str) -> Result<ViewKey, JsonRpcError> {
    ViewKey::from_canonical_json(text).map_err(|_| invalid_request_error())
}

fn parse_group(text: &str) -> Result<PanelGroupId, JsonRpcError> {
    text.parse::<PanelGroupId>()
        .map_err(|_| invalid_request_error())
}

fn split_direction(direction: SplitDirectionWire) -> SplitDirection {
    match direction {
        SplitDirectionWire::LeftRight => SplitDirection::LeftRight,
        SplitDirectionWire::TopBottom => SplitDirection::TopBottom,
    }
}

fn logical_viewport(viewport: ViewportSize) -> LogicalSize {
    LogicalSize {
        width: viewport.width,
        height: viewport.height,
    }
}

/// 写意图统一投递给派发器；身份解析失败不登记任何 Operation。
fn respond_mutation(
    runtime: &RuntimeCallbacks,
    id: JsonRpcId,
    client_id: String,
    mutation_id: String,
    expected_revision: Option<u64>,
    mutation: Result<RuntimeMutation, JsonRpcError>,
) -> JsonRpcResponse<RuntimeResult> {
    let mutation = match mutation {
        Ok(mutation) => mutation,
        Err(error) => return JsonRpcResponse::error(id, error),
    };
    match (runtime.mutation)(RuntimeMutationRequest {
        client_id,
        mutation_id,
        expected_revision,
        mutation,
    }) {
        Ok(resource) => JsonRpcResponse::result(id, RuntimeResult::Operation(resource)),
        Err(error) => JsonRpcResponse::error(id, error),
    }
}

/// 订阅连接：事件由独立写线程推送，worker 只等待取消订阅或断开。
///
/// 取消订阅、客户端断开与关闭都先关闭流再 join 事件线程；
/// 事件写与取消确认共用同一写锁，保证 JSON 行不交错。
fn stream_subscription(
    stream: &mut TcpStream,
    events: Receiver<RuntimeEvent>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<()> {
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let writer_for_events = Arc::clone(&writer);
    let shutdown_for_events = Arc::clone(shutdown);
    let events_done = Arc::new(AtomicBool::new(false));
    let events_done_for_thread = Arc::clone(&events_done);
    let event_thread = std::thread::Builder::new()
        .name("slterm-runtime-events".to_owned())
        .spawn(move || {
            loop {
                match events.recv_timeout(OPERATION_WAIT_POLL) {
                    Ok(event) => {
                        let Ok(mut writer) = writer_for_events.lock() else {
                            break;
                        };
                        if write_json(&mut writer, &EventNotification::new(event)).is_err() {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if shutdown_for_events.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            // 事件源结束（滞后被移除、关闭或写失败）必须让连接线程退出，
            // 不能让客户端停在“无事件”的静默状态。
            events_done_for_thread.store(true, Ordering::SeqCst);
        })?;
    let result = (|| -> io::Result<()> {
        let mut buffer = Vec::new();
        loop {
            match read_frame(
                stream,
                &mut buffer,
                Instant::now() + Duration::from_secs(1),
                shutdown,
            ) {
                Ok(Some(frame)) => {
                    let Ok(ClientRequest::Runtime(request)) = parse_client_request(&frame) else {
                        return Ok(());
                    };
                    if request.method != RuntimeMethod::EventsUnsubscribe {
                        return Ok(());
                    }
                    let mut writer = writer
                        .lock()
                        .map_err(|_| io::Error::other("subscription writer poisoned"))?;
                    write_json(
                        &mut writer,
                        &JsonRpcResponse::result(
                            request.id,
                            RuntimeResult::Unsubscribed(EventsUnsubscribedResponse {}),
                        ),
                    )?;
                    return Ok(());
                }
                Ok(None) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                    if events_done.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
    })();
    let _ = stream.shutdown(Shutdown::Both);
    let _ = event_thread.join();
    result
}

/// terminal.wait 在连接线程内轮询前台帧 revision；超时 NotReady、关闭 Closing。
fn wait_terminal(
    runtime: &RuntimeCallbacks,
    request: &RuntimeRequest,
    shutdown: &AtomicBool,
    stream: &TcpStream,
) -> Result<TerminalWaitResponse, JsonRpcError> {
    let Some(target) = parse_wait_target(request) else {
        return Err(not_ready_error("terminal wait params missing"));
    };
    let timeout_ms = target.timeout_ms.unwrap_or(OPERATION_WAIT_LIMIT_MS);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if shutdown.load(Ordering::SeqCst) || client_disconnected(stream) {
            return Err(closing_error());
        }
        let revision = (runtime.wait_status)(TerminalStatusTarget {
            terminal_id: target.terminal_id,
            generation: target.generation,
            agent_id: target.agent_id.clone(),
        })?;
        if revision > target.since_revision {
            return Ok(TerminalWaitResponse {
                terminal_id: target.terminal_id.to_string(),
                generation: target.generation,
                revision,
            });
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(not_ready_error("terminal wait not ready"));
        }
        std::thread::sleep(OPERATION_WAIT_POLL.min(deadline - now));
    }
}

/// terminal.wait 与 agent.wait 的统一等待目标。
struct WaitTarget {
    terminal_id: slterm_workspace::TerminalId,
    generation: u64,
    since_revision: u64,
    timeout_ms: Option<u64>,
    agent_id: Option<String>,
}

fn parse_wait_target(request: &RuntimeRequest) -> Option<WaitTarget> {
    if let Some(params) = request.parse::<TerminalWaitRequest>() {
        return Some(WaitTarget {
            terminal_id: parse_terminal_id(&params.terminal_id).ok()?,
            generation: params.generation,
            since_revision: params.since_revision,
            timeout_ms: params.timeout_ms,
            agent_id: None,
        });
    }
    let params = request.parse::<AgentWaitRequest>()?;
    Some(WaitTarget {
        terminal_id: parse_terminal_id(&params.terminal_id).ok()?,
        generation: params.generation,
        since_revision: params.since_revision,
        timeout_ms: params.timeout_ms,
        agent_id: Some(params.agent_id),
    })
}

/// terminal.read 与 agent.read 共用分页响应；身份与代次由前台复核。
fn respond_read(
    runtime: &RuntimeCallbacks,
    id: JsonRpcId,
    terminal_id_text: &str,
    generation: u64,
    agent_id: Option<String>,
    cursor: Option<&str>,
) -> JsonRpcResponse<RuntimeResult> {
    let cursor_resource = resource_token(&format!(
        "terminal:{terminal_id_text}:{}",
        agent_id.as_deref().unwrap_or("")
    ));
    let target = match parse_terminal_id(terminal_id_text) {
        Ok(terminal_id) => TerminalReadTarget {
            terminal_id,
            generation,
            agent_id,
        },
        Err(error) => return JsonRpcResponse::error(id, error),
    };
    let snapshot = match (runtime.read)(target) {
        Ok(snapshot) => snapshot,
        Err(error) => return JsonRpcResponse::error(id, error),
    };
    match read_page(
        snapshot.text.as_bytes(),
        ReadGeneration {
            resource: cursor_resource,
            generation: snapshot.generation,
            revision: snapshot.revision,
        },
        cursor,
        PAGE_BUDGET_BYTES,
    ) {
        Ok(page) => match String::from_utf8(page.bytes) {
            Ok(data) => JsonRpcResponse::result(
                id,
                RuntimeResult::TerminalRead(TerminalReadResponse {
                    terminal_id: terminal_id_text.to_owned(),
                    generation: snapshot.generation,
                    revision: snapshot.revision,
                    data,
                    next_cursor: page.next_cursor,
                }),
            ),
            Err(_) => JsonRpcResponse::error(id, read_encoding_error()),
        },
        Err(ReadError::StaleCursor) => JsonRpcResponse::error(id, stale_cursor_error()),
    }
}

/// operation.wait 只在后台 worker 轮询；关闭与超时都返回结构化终态。
fn wait_operation(
    runtime: &RuntimeCallbacks,
    request: &RuntimeRequest,
    shutdown: &AtomicBool,
    stream: &TcpStream,
) -> Result<OperationResource, JsonRpcError> {
    let Some(params) = request.parse::<OperationWaitParams>() else {
        return Err(not_ready_error("operation wait params missing"));
    };
    let timeout_ms = params.timeout_ms.unwrap_or(OPERATION_WAIT_LIMIT_MS);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if shutdown.load(Ordering::SeqCst) || client_disconnected(stream) {
            return Err(closing_error());
        }
        match (runtime.operation)(&params.operation_id) {
            Ok(Some(resource)) if resource.status.is_terminal() => return Ok(resource),
            Ok(Some(_)) => {}
            Ok(None) => return Err(unknown_operation_error()),
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(not_ready_error("operation wait timed out"));
        }
        std::thread::sleep(OPERATION_WAIT_POLL);
    }
}

/// 非阻塞探测客户端是否已断开；不读走后续请求数据。
fn client_disconnected(stream: &TcpStream) -> bool {
    let mut probe = [0_u8; 1];
    match stream.peek(&mut probe) {
        Ok(0) => true,
        Ok(_) => false,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
            ) =>
        {
            false
        }
        Err(_) => true,
    }
}

fn credential_matches(expected: &SecretBytes, actual: &Credential) -> bool {
    let expected = expected.as_bytes();
    let actual = actual.as_str().as_bytes();
    let mut difference = 0_u8;
    for (left, right) in expected.iter().zip(actual) {
        difference |= left ^ right;
    }
    difference == 0 && expected.len() == actual.len()
}

pub(super) fn read_frame<R: Read>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
    deadline: Instant,
    shutdown: &AtomicBool,
) -> io::Result<Option<Vec<u8>>> {
    let mut chunk = [0_u8; 1024];
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(None);
        }
        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            return Ok(Some(buffer.drain(..=newline).collect()));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "control request timed out",
            ));
        }
        match reader.read(&mut chunk) {
            Ok(0) => {
                if buffer.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(std::mem::take(buffer)));
            }
            Ok(count) => {
                buffer.extend_from_slice(&chunk[..count]);
                if !buffer.contains(&b'\n') && buffer.len() > REQUEST_BUDGET {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "control request exceeds its budget",
                    ));
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                // 轮询只用于唤醒关闭；总等待仍由 deadline 限定。
            }
            Err(error) => return Err(error),
        }
    }
}

fn write_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> io::Result<()> {
    let mut encoded = serde_json::to_vec(value).map_err(io::Error::other)?;
    if encoded.len() > RESPONSE_BUDGET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "control response exceeds its budget",
        ));
    }
    encoded.push(b'\n');
    stream.write_all(&encoded)
}

fn join(handle: JoinHandle<()>) -> io::Result<()> {
    handle
        .join()
        .map_err(|_| io::Error::other("control worker terminated unexpectedly"))
}
