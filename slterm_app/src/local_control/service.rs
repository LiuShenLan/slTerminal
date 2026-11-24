//! 首实例控制面服务组合：凭据、真实运行元数据、发现记录与统一关闭。

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use slterm_runtime_protocol::{
    ControlScope, DiscoveryRecord, DomainErrorCode, HandoverParams, HandoverResponse,
    HelloResponse, JsonRpcError, JsonRpcErrorData,
};

use super::credentials::load_or_create_at;
use super::discovery::{DiscoveryGuard, endpoint_path, publish_at};
use super::events::EventHub;
use super::operations::{
    DispatchOutcome, DispatchRequest, OperationDispatcher, operation_resource, writer_error,
};
use super::reads::TerminalTextSnapshot;
use super::server::{
    AgentsCallback, EventSubscribeCallback, LocalControlServer, MutationCallback,
    OperationQueryCallback, ReadCallback, RuntimeCallbacks, SnapshotCallback, WaitStatusCallback,
};
use crate::app_shell::{
    SltermShellEvent, try_send_handover, try_send_runtime_agents, try_send_runtime_mutation,
    try_send_runtime_read, try_send_runtime_snapshot, try_send_runtime_wait,
};
use crate::persistence::metadata::WorkspaceMetadata;
use crate::persistence::workspace_writer::WorkspaceWriterClient;
use crate::secrets::{CredentialStore, SecretBytes, WindowsCredentialStore};

pub(super) const HANDOVER_REPLY_TIMEOUT: Duration = Duration::from_secs(4);
/// 快照只读前台内存；2 秒足够覆盖事件泵唤醒与回执调度。
pub(super) const SNAPSHOT_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
/// 写意图在壳内执行的完成回执上限；与 handover 的 4 秒一致。
const MUTATION_REPLY_TIMEOUT: Duration = Duration::from_secs(4);
const PROTOCOL_DOMAIN: &[u8] = b"slterm-runtime-v2";
const RUNTIME_SCHEMA: &[u8] = include_bytes!("../../../assets/runtime-api-v2.schema.json");

pub(super) type HandoverCallback =
    Arc<dyn Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static>;

/// 事件发布端口：窗口只经它向 hub 提交轻量事件；队列满时返回 false。
#[derive(Clone)]
pub(crate) struct RuntimeEventPublisher {
    publish: Arc<dyn Fn(slterm_runtime_protocol::RuntimeEvent) -> bool + Send + Sync>,
}

impl RuntimeEventPublisher {
    pub(crate) fn from_callback(
        publish: impl Fn(slterm_runtime_protocol::RuntimeEvent) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            publish: Arc::new(publish),
        }
    }

    pub(crate) fn publish(&self, event: slterm_runtime_protocol::RuntimeEvent) -> bool {
        (self.publish)(event)
    }
}

/// 读取或创建指定作用域的线缆凭据句柄；只由持锁首实例调用。
///
/// 句柄在 server 认证点直接与请求凭据比较，不落成第二份字符串副本。
fn load_credential(
    scope: ControlScope,
    local_root: &Path,
    store: &dyn CredentialStore,
) -> io::Result<SecretBytes> {
    load_or_create_at(scope, local_root, store)
}

/// 已持 OS 单实例锁的首实例服务；显式 shutdown 只能在 GPUI 外执行。
pub(crate) struct LocalControlService {
    server: LocalControlServer,
    event_hub: Arc<EventHub>,
    discovery_path: std::path::PathBuf,
    record: Option<DiscoveryRecord>,
    discovery: Option<DiscoveryGuard>,
    closed: bool,
}

impl LocalControlService {
    /// 生产入口；调用方必须先取得并持续持有 OS 单实例锁。
    ///
    /// `writer` 为唯一工作区写线程的 client；缺失时 operation 查询明确 NotReady。
    pub(crate) fn start(
        metadata: WorkspaceMetadata,
        events: SyncSender<SltermShellEvent>,
        writer: Option<WorkspaceWriterClient>,
    ) -> io::Result<Self> {
        let store = WindowsCredentialStore;
        Self::start_with(
            metadata,
            events,
            &store,
            &endpoint_path(),
            HANDOVER_REPLY_TIMEOUT,
            writer,
            |handover, runtime, hello, on_handover, callbacks| {
                LocalControlServer::start_with_runtime(
                    handover,
                    runtime,
                    hello,
                    move |params| on_handover(params),
                    callbacks,
                )
            },
        )
    }

    #[cfg(test)]
    pub(super) fn start_at(
        metadata: WorkspaceMetadata,
        events: SyncSender<SltermShellEvent>,
        store: &dyn CredentialStore,
        discovery_path: &Path,
        reply_timeout: Duration,
        writer: Option<WorkspaceWriterClient>,
    ) -> io::Result<Self> {
        Self::start_with(
            metadata,
            events,
            store,
            discovery_path,
            reply_timeout,
            writer,
            |handover, runtime, hello, on_handover, callbacks| {
                LocalControlServer::start_with_runtime(
                    handover,
                    runtime,
                    hello,
                    move |params| on_handover(params),
                    callbacks,
                )
            },
        )
    }

    pub(super) fn start_with<B>(
        metadata: WorkspaceMetadata,
        events: SyncSender<SltermShellEvent>,
        store: &dyn CredentialStore,
        discovery_path: &Path,
        reply_timeout: Duration,
        writer: Option<WorkspaceWriterClient>,
        bind: B,
    ) -> io::Result<Self>
    where
        B: FnOnce(
            SecretBytes,
            SecretBytes,
            HelloResponse,
            HandoverCallback,
            RuntimeCallbacks,
        ) -> io::Result<LocalControlServer>,
    {
        let local_root = discovery_path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "控制面发现路径缺少数据根")
        })?;
        let credential = load_credential(ControlScope::Handover, local_root, store)?;
        let runtime_credential = load_credential(ControlScope::Runtime, local_root, store)?;
        let (schema_sha256, protocol_sha256) = runtime_contract_hashes();
        let hello = HelloResponse::new(
            metadata.store_epoch.clone(),
            metadata.run.run_id.clone(),
            0,
            protocol_sha256,
            schema_sha256,
        );
        let snapshot_events = events.clone();
        let mutation_events = events.clone();
        let read_events = events.clone();
        let wait_events = events.clone();
        let agent_events = events.clone();
        let on_handover: HandoverCallback = Arc::new(move |params| {
            let receipt = try_send_handover(&events, params)?;
            match receipt.recv_timeout(reply_timeout) {
                Ok(response) => response,
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "handover not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // 快照请求经同一壳事件通道到前台；worker 只做有界等待。
        let on_snapshot: SnapshotCallback = Arc::new(move || {
            let receipt = try_send_runtime_snapshot(&snapshot_events)?;
            match receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(result) => result,
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "snapshot not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // 终端读取经前台定位 worker，再由后台等待文本快照；前台从不锁实时网格。
        let on_read: ReadCallback = Arc::new(move |target| {
            let generation = target.generation;
            let receipt = try_send_runtime_read(&read_events, target.terminal_id, generation)?;
            let worker_receipt = match receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(Ok(receipt)) => receipt,
                Ok(Err(error)) => return Err(error),
                Err(RecvTimeoutError::Timeout) => {
                    return Err(control_error(
                        DomainErrorCode::NotReady,
                        -32001,
                        "terminal read not ready",
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(control_error(DomainErrorCode::Closing, -32004, "closing"));
                }
            };
            // worker 文本复制可能涉及整段回滚，等待仍在连接线程内且有界。
            match worker_receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(snapshot) => Ok(TerminalTextSnapshot {
                    generation,
                    revision: snapshot.revision,
                    text: snapshot.text,
                }),
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "terminal read not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // 终端等待只读前台最近帧 revision；每轮请求都经同一壳事件通道。
        let on_wait_status: WaitStatusCallback = Arc::new(move |target| {
            let receipt =
                try_send_runtime_wait(&wait_events, target.terminal_id, target.generation)?;
            match receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(result) => result,
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "terminal wait not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // Agent 资源由前台按请求时刻派生；worker 只做有界等待。
        let on_agents: AgentsCallback = Arc::new(move || {
            let receipt = try_send_runtime_agents(&agent_events)?;
            match receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(result) => result,
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "agent list not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // Operation 查询只经唯一写线程；缺失 writer 时明确 NotReady。
        let operation_writer = writer.clone();
        let on_operation: OperationQueryCallback = Arc::new(move |operation_id| {
            let Some(writer) = &operation_writer else {
                return Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "operation query not ready",
                ));
            };
            let receipt = writer.query_operation(operation_id).map_err(writer_error)?;
            match receipt.recv_timeout(SNAPSHOT_REPLY_TIMEOUT) {
                Ok(Ok(Some(operation))) => Ok(Some(operation_resource(operation))),
                Ok(Ok(None)) => Ok(None),
                Ok(Err(_error)) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "operation store unavailable",
                )),
                Err(RecvTimeoutError::Timeout) => Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "operation query not ready",
                )),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
                }
            }
        });
        // 事件环由专用 hub 线程独占；发布者只经有界命令通道进入。
        let event_hub = Arc::new(EventHub::start(
            metadata.store_epoch.clone(),
            metadata.run.run_id.clone(),
        )?);
        let hub_for_events = Arc::clone(&event_hub);
        let on_events: EventSubscribeCallback =
            Arc::new(move |params| hub_for_events.subscribe(params));
        // 写派发只经唯一 writer；执行体把意图经壳事件交给 AppRoot。
        let dispatcher = writer.map(|writer| {
            Arc::new(OperationDispatcher::new(
                writer,
                metadata.store_epoch.clone(),
            ))
        });
        let on_mutation: MutationCallback = Arc::new(move |request| {
            let Some(dispatcher) = &dispatcher else {
                return Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "mutation not ready",
                ));
            };
            let method = request.mutation.method();
            let params = request.mutation.params_value().map_err(|_| {
                control_error(
                    DomainErrorCode::InvalidRequest,
                    -32000,
                    "invalid mutation params",
                )
            })?;
            dispatcher.dispatch(
                DispatchRequest {
                    client_id: &request.client_id,
                    mutation_id: &request.mutation_id,
                    method,
                    params: &params,
                    // 资源代次由执行体在提交前复核，派发器不重复猜测。
                    expected_revision: None,
                    current_revision: 0,
                    resource_generations: BTreeMap::new(),
                },
                || match try_send_runtime_mutation(&mutation_events, request.mutation) {
                    Ok(receipt) => match receipt.recv_timeout(MUTATION_REPLY_TIMEOUT) {
                        Ok(Ok(())) => DispatchOutcome::Succeeded(serde_json::json!({})),
                        Ok(Err(error)) => DispatchOutcome::Failed(error_value(error)),
                        Err(_) => DispatchOutcome::OutcomeUnknown(
                            serde_json::json!({"code": "outcome_unknown"}),
                        ),
                    },
                    Err(error) => DispatchOutcome::Failed(error_value(error)),
                },
            )
        });
        let server = bind(
            credential,
            runtime_credential,
            hello,
            on_handover,
            RuntimeCallbacks {
                snapshot: on_snapshot,
                operation: on_operation,
                events: on_events,
                mutation: on_mutation,
                read: on_read,
                wait_status: on_wait_status,
                agents: on_agents,
            },
        )?;
        let record = DiscoveryRecord {
            protocol_version: 2,
            endpoint: server
                .endpoint()
                .parse()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            run_id: metadata.run.run_id,
            process_id: metadata.run.process_id,
            process_created_at_ms: metadata.run.process_created_at_ms,
        };
        Ok(Self {
            server,
            event_hub,
            discovery_path: discovery_path.to_owned(),
            record: Some(record),
            discovery: None,
            closed: false,
        })
    }

    /// 事件发布端口：可在窗口闭包中持有，只经有界命令通道进入 hub。
    pub(crate) fn event_publisher(&self) -> RuntimeEventPublisher {
        RuntimeEventPublisher::from_callback({
            let hub = Arc::clone(&self.event_hub);
            move |event| hub.publish(event)
        })
    }

    /// 窗口可服务后才发布发现记录；失败立即关闭监听，且不允许重发到死端点。
    pub(crate) fn publish_discovery(&mut self) -> io::Result<()> {
        if self.discovery.is_some() {
            return Ok(());
        }
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "控制面已关闭，不能发布发现记录",
            ));
        }
        let record = self
            .record
            .take()
            .expect("未开放的控制面必须保留本次发现记录");
        let discovery = match publish_at(&self.discovery_path, record) {
            Ok(discovery) => discovery,
            Err(publish_error) => {
                self.closed = true;
                // 保留发布根因与 kind；server 与 hub 回收失败都必须一并可观察。
                let server_cleanup = self.server.shutdown().err();
                if let Err(hub_error) = self.event_hub.shutdown()
                    && server_cleanup.is_none()
                {
                    return Err(io::Error::new(
                        publish_error.kind(),
                        format!("{publish_error}; 控制面事件环回收失败: {hub_error}"),
                    ));
                }
                return match server_cleanup {
                    None => Err(publish_error),
                    Some(cleanup_error) => Err(io::Error::new(
                        publish_error.kind(),
                        format!("{publish_error}; 控制面 server 回收失败: {cleanup_error}"),
                    )),
                };
            }
        };
        self.discovery = Some(discovery);
        Ok(())
    }

    /// 三个关闭动作都必定尝试；未发布记录时只关闭监听，重复调用保持幂等。
    pub(crate) fn shutdown(&mut self) -> io::Result<()> {
        self.closed = true;
        shutdown_components(
            || self.server.shutdown(),
            || match self.discovery.as_mut() {
                Some(discovery) => discovery.revoke(),
                None => Ok(()),
            },
            || self.event_hub.shutdown(),
        )
    }
}

impl Drop for LocalControlService {
    fn drop(&mut self) {
        // 正常退出必须先在 GPUI 外显式 shutdown；Drop 只尽力回收。
        let _ = self.shutdown();
    }
}

pub(super) fn shutdown_components(
    server: impl FnOnce() -> io::Result<()>,
    discovery: impl FnOnce() -> io::Result<()>,
    events: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let results = [server(), discovery(), events()];
    let mut first_error = None;
    for result in results {
        if let Err(error) = result
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn control_error(domain: DomainErrorCode, numeric: i32, message: &'static str) -> JsonRpcError {
    JsonRpcError::new(numeric, message, Some(JsonRpcErrorData { code: domain }))
}

/// 结构化错误写入 Operation 的错误字段；序列化失败只保留封闭码。
fn error_value(error: JsonRpcError) -> serde_json::Value {
    serde_json::to_value(&error).unwrap_or_else(|_| serde_json::json!({"code": "unknown"}))
}

/// Runtime 合同散列唯一推导点：协议散列覆盖域分隔前缀与同一 schema 原始字节。
fn runtime_contract_hashes() -> (String, String) {
    let schema = Sha256::digest(RUNTIME_SCHEMA);
    let mut protocol = Sha256::new();
    protocol.update(PROTOCOL_DOMAIN);
    protocol.update(RUNTIME_SCHEMA);
    let protocol = protocol.finalize();
    (hex(&schema), hex(&protocol))
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("散列 hex 格式化失败");
    }
    encoded
}
