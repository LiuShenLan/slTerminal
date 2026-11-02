//! 首实例控制面服务组合：凭据、真实运行元数据、发现记录与统一关闭。

use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use slterm_runtime_protocol::{
    ControlScope, Credential, DiscoveryRecord, DomainErrorCode, HandoverParams, HandoverResponse,
    HelloResponse, JsonRpcError, JsonRpcErrorData,
};

use super::credentials::load_or_create_at;
use super::discovery::{DiscoveryGuard, endpoint_path, publish_at};
use super::server::LocalControlServer;
use crate::app_shell::{SltermShellEvent, try_send_handover};
use crate::persistence::metadata::WorkspaceMetadata;
use crate::secrets::{CredentialStore, WindowsCredentialStore};

pub(super) const HANDOVER_REPLY_TIMEOUT: Duration = Duration::from_secs(4);
const PROTOCOL_DOMAIN: &[u8] = b"slterm-runtime-v2";
const RUNTIME_SCHEMA: &[u8] = include_bytes!("../../../assets/runtime-api-v2.schema.json");

pub(super) type HandoverCallback =
    Arc<dyn Fn(HandoverParams) -> Result<HandoverResponse, JsonRpcError> + Send + Sync + 'static>;

/// 已持 OS 单实例锁的首实例服务；显式 shutdown 只能在 GPUI 外执行。
pub(crate) struct LocalControlService {
    server: LocalControlServer,
    discovery: DiscoveryGuard,
}

impl LocalControlService {
    /// 生产入口；调用方必须先取得并持续持有 OS 单实例锁。
    pub(crate) fn start(
        metadata: WorkspaceMetadata,
        events: SyncSender<SltermShellEvent>,
    ) -> io::Result<Self> {
        let store = WindowsCredentialStore;
        Self::start_with(
            metadata,
            events,
            &store,
            &endpoint_path(),
            HANDOVER_REPLY_TIMEOUT,
            |credential, hello, on_handover| {
                LocalControlServer::start(credential, hello, move |params| on_handover(params))
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
    ) -> io::Result<Self> {
        Self::start_with(
            metadata,
            events,
            store,
            discovery_path,
            reply_timeout,
            |credential, hello, on_handover| {
                LocalControlServer::start(credential, hello, move |params| on_handover(params))
            },
        )
    }

    pub(super) fn start_with<B>(
        metadata: WorkspaceMetadata,
        events: SyncSender<SltermShellEvent>,
        store: &dyn CredentialStore,
        discovery_path: &Path,
        reply_timeout: Duration,
        bind: B,
    ) -> io::Result<Self>
    where
        B: FnOnce(Credential, HelloResponse, HandoverCallback) -> io::Result<LocalControlServer>,
    {
        let local_root = discovery_path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "控制面发现路径缺少数据根")
        })?;
        let credential = load_or_create_at(ControlScope::Handover, local_root, store)?;
        let credential = credential.into_string().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "控制面凭据不是有效 UTF-8")
        })?;
        let credential = Credential::new(credential.as_str());
        let (schema_sha256, protocol_sha256) = runtime_contract_hashes();
        let hello = HelloResponse::new(
            metadata.store_epoch.clone(),
            metadata.run.run_id.clone(),
            0,
            protocol_sha256,
            schema_sha256,
        );
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
        let mut server = bind(credential, hello, on_handover)?;
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
        let discovery = match publish_at(discovery_path, record) {
            Ok(discovery) => discovery,
            Err(publish_error) => {
                // 保留发布根因与 kind；回收失败必须以安全文本一并可观察。
                return match server.shutdown() {
                    Ok(()) => Err(publish_error),
                    Err(cleanup_error) => Err(io::Error::new(
                        publish_error.kind(),
                        format!("{publish_error}; 控制面 server 回收失败: {cleanup_error}"),
                    )),
                };
            }
        };
        Ok(Self { server, discovery })
    }

    /// 两个关闭动作都必定尝试；重复调用沿用底层幂等与失败重试语义。
    pub(crate) fn shutdown(&mut self) -> io::Result<()> {
        let Self { server, discovery } = self;
        shutdown_components(|| server.shutdown(), || discovery.revoke())
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
) -> io::Result<()> {
    let server = server();
    let discovery = discovery();
    match (server, discovery) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn control_error(domain: DomainErrorCode, numeric: i32, message: &'static str) -> JsonRpcError {
    JsonRpcError::new(numeric, message, Some(JsonRpcErrorData { code: domain }))
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
