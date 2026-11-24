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
    ClientRequest, ControlScope, Credential, DescribeResponse, DomainErrorCode, EventNotification,
    EventsSubscribeParams, EventsUnsubscribedResponse, HandoverParams, HandoverResponse,
    HelloResponse, JsonRpcError, JsonRpcErrorData, JsonRpcId, JsonRpcResponse,
    OPERATION_WAIT_LIMIT_MS, OperationQueryParams, OperationResource, OperationWaitParams,
    ProjectActivateParams, ProjectListResponse, ProjectRemoveParams, ProjectRenameParams,
    ProjectReorderParams, RuntimeEvent, RuntimeMethod, RuntimeRequest, RuntimeResult,
    SnapshotResources, SnapshotResponse, parse_client_request,
};
use slterm_workspace::ProjectId;

use super::events::Subscription;
use super::mutations::{RuntimeMutation, RuntimeMutationRequest};
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

/// Runtime 作用域的应用回调集合。
pub(crate) struct RuntimeCallbacks {
    pub(crate) snapshot: SnapshotCallback,
    pub(crate) operation: OperationQueryCallback,
    pub(crate) events: EventSubscribeCallback,
    pub(crate) mutation: MutationCallback,
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
                snapshot: Arc::new(not_ready_snapshot),
                operation: Arc::new(not_ready_operation),
                events: Arc::new(not_ready_events),
                mutation: Arc::new(not_ready_mutation),
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
            return match &self.termination_error {
                Some(error) => Err(io::Error::new(error.kind(), error.to_string())),
                None => Ok(()),
            };
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
        // 作用域与方法的对应关系由方法注册表约束；未知组合直接断开。
        let Ok(request) = parse_client_request(&frame) else {
            return;
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
                let response = match request.method {
                    // describe 只读方法注册表与握手同一散列，不需要回前台。
                    RuntimeMethod::Describe => JsonRpcResponse::result(
                        request.id,
                        RuntimeResult::Describe(DescribeResponse::current(
                            hello.protocol_sha256.clone(),
                            hello.schema_sha256.clone(),
                        )),
                    ),
                    // 事件环接入前，revision 固定为握手起点；Hub 切片接管实时代次。
                    RuntimeMethod::Snapshot => match (runtime.snapshot)() {
                        Ok(resources) => JsonRpcResponse::result(
                            request.id,
                            RuntimeResult::Snapshot(SnapshotResponse {
                                store_epoch: hello.store_epoch.clone(),
                                stream_epoch: hello.stream_epoch.clone(),
                                revision: hello.revision,
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
                    RuntimeMethod::OperationWait => {
                        match wait_operation(&runtime, &request, &shutdown) {
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
                            parse_project_id(&params.request.project_id)
                                .map(|project_id| RuntimeMutation::ProjectRemove { project_id }),
                        )
                    }
                };
                if write_json(&mut stream, &response).is_err() {
                    return;
                }
            }
            ClientRequest::Handover(request) => {
                if scope != ControlScope::Handover {
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
                Err(error) if error.kind() == io::ErrorKind::TimedOut => continue,
                Err(error) => return Err(error),
            }
        }
    })();
    let _ = stream.shutdown(Shutdown::Both);
    let _ = event_thread.join();
    result
}

/// operation.wait 只在后台 worker 轮询；关闭与超时都返回结构化终态。
fn wait_operation(
    runtime: &RuntimeCallbacks,
    request: &RuntimeRequest,
    shutdown: &AtomicBool,
) -> Result<OperationResource, JsonRpcError> {
    let Some(params) = request.parse::<OperationWaitParams>() else {
        return Err(not_ready_error("operation wait params missing"));
    };
    let timeout_ms = params.timeout_ms.unwrap_or(OPERATION_WAIT_LIMIT_MS);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if shutdown.load(Ordering::SeqCst) {
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
