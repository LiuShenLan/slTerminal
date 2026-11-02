//! LocalControlService 的真实凭据、发现、回执与关闭组合测试。

use std::cell::Cell;
use std::collections::HashMap;
use std::io;
use std::net::TcpStream;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};
use slterm_runtime_protocol::{
    ControlScope, Credential, DomainErrorCode, HandoverParams, HandoverResponse, HelloMethod,
    HelloParams, HelloRequest, HelloResponse, JsonRpcId, JsonRpcResponse,
};

use super::client::{HandoverError, handover};
use super::credentials::load_credential_at;
use super::discovery::read_discovery_at;
use super::server::LocalControlServer;
use super::service::LocalControlService;
use crate::app_shell::SltermShellEvent;
use crate::persistence::metadata::{RunMarker, WorkspaceMetadata};
use crate::secrets::{CredentialStore, SecretBytes};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const SHORT_REPLY_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Default)]
struct MemoryCredentialStore {
    entries: Mutex<HashMap<String, Vec<u8>>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
        Ok(self
            .entries
            .lock()
            .expect("内存凭据锁")
            .get(target)
            .cloned()
            .map(SecretBytes::new))
    }

    fn store(&self, target: &str, secret: &[u8]) -> io::Result<()> {
        self.entries
            .lock()
            .expect("内存凭据锁")
            .insert(target.to_owned(), secret.to_vec());
        Ok(())
    }

    fn delete(&self, target: &str) -> io::Result<()> {
        self.entries.lock().expect("内存凭据锁").remove(target);
        Ok(())
    }
}

fn metadata() -> WorkspaceMetadata {
    WorkspaceMetadata {
        store_epoch: "store-real-epoch".to_owned(),
        run: RunMarker {
            run_id: "run-real-stream".to_owned(),
            process_id: 4321,
            process_created_at_ms: 1_700_000_000_123,
        },
        clean_shutdown: false,
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn event_channel() -> (
    SyncSender<SltermShellEvent>,
    mpsc::Receiver<SltermShellEvent>,
) {
    mpsc::sync_channel(64)
}

fn start_service(
    discovery_path: &std::path::Path,
    store: &dyn CredentialStore,
    events: SyncSender<SltermShellEvent>,
) -> LocalControlService {
    LocalControlService::start_at(metadata(), events, store, discovery_path, TEST_TIMEOUT)
        .expect("启动真实 loopback 服务组合")
}

fn load_test_credential(root: &std::path::Path, store: &dyn CredentialStore) -> SecretBytes {
    load_credential_at(ControlScope::Handover, root, store).expect("只读加载同一 handover 凭据")
}

fn handover_once(
    discovery_path: &std::path::Path,
    root: &std::path::Path,
    store: &dyn CredentialStore,
) -> Result<(), HandoverError> {
    let record = read_discovery_at(discovery_path).expect("读取本次发现记录");
    handover(
        &record,
        load_test_credential(root, store),
        HandoverParams::Focus,
    )
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, value: &T) {
    let mut encoded = serde_json::to_vec(value).unwrap();
    encoded.push(b'\n');
    std::io::Write::write_all(stream, &encoded).unwrap();
}

fn read_hello(stream: &mut TcpStream, secret: SecretBytes) -> HelloResponse {
    let secret = secret.into_string().expect("测试凭据为 UTF-8");
    let request = HelloRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: "service-contract-test".to_owned(),
            credential: Credential::new(secret.as_str()),
            scope: ControlScope::Handover,
        },
    );
    write_json(stream, &request);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
    match serde_json::from_str::<JsonRpcResponse<HelloResponse>>(&line).unwrap() {
        JsonRpcResponse::Result { result, .. } => result,
        JsonRpcResponse::Error { error, .. } => panic!("hello 必须成功: {error:?}"),
    }
}

#[test]
fn service_publishes_real_run_metadata_and_completes_focus_handover() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    let mut service = start_service(&path, &store, events);
    let expected = metadata();

    let record = read_discovery_at(&path).unwrap();
    assert_eq!(record.run_id, expected.run.run_id);
    assert_eq!(record.process_id, expected.run.process_id);
    assert_eq!(
        record.process_created_at_ms,
        expected.run.process_created_at_ms
    );

    let foreground = std::thread::spawn(move || {
        let SltermShellEvent::Handover { params, completion } =
            event_rx.recv_timeout(TEST_TIMEOUT).unwrap()
        else {
            panic!("服务只能投递 handover 事件");
        };
        assert_eq!(params, HandoverParams::Focus);
        completion
            .try_send(Ok(HandoverResponse::accepted()))
            .unwrap();
    });
    handover_once(&path, temp.path(), &store).expect("Focus accepted 必须返回客户端成功");
    foreground.join().unwrap();
    service.shutdown().unwrap();
}

#[test]
fn service_hello_uses_real_generations_and_packaged_schema_digests() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, _event_rx) = event_channel();
    let mut service = start_service(&path, &store, events);
    let credential = load_test_credential(temp.path(), &store);
    let endpoint = read_discovery_at(&path).unwrap().endpoint;
    let mut stream = TcpStream::connect(endpoint).unwrap();

    let hello = read_hello(&mut stream, credential);
    let expected = metadata();
    let schema = include_bytes!("../../../assets/runtime-api-v2.schema.json");
    let mut protocol_input = b"slterm-runtime-v2".to_vec();
    protocol_input.extend_from_slice(schema);
    assert_eq!(hello.store_epoch, expected.store_epoch);
    assert_eq!(hello.stream_epoch, expected.run.run_id);
    assert_eq!(hello.revision, 0);
    assert_eq!(hello.schema_sha256, hex_digest(schema));
    assert_eq!(hello.protocol_sha256, hex_digest(&protocol_input));
    assert!(!hello.store_epoch.contains("placeholder"));
    assert!(!hello.stream_epoch.contains("placeholder"));
    drop(stream);
    service.shutdown().unwrap();
}

#[test]
fn service_preserves_structured_closing_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    let mut service = start_service(&path, &store, events);
    let foreground = std::thread::spawn(move || {
        let SltermShellEvent::Handover { completion, .. } =
            event_rx.recv_timeout(TEST_TIMEOUT).unwrap()
        else {
            panic!("服务只能投递 handover 事件");
        };
        completion
            .try_send(Err(slterm_runtime_protocol::JsonRpcError::new(
                -32004,
                "closing",
                Some(slterm_runtime_protocol::JsonRpcErrorData {
                    code: DomainErrorCode::Closing,
                }),
            )))
            .unwrap();
    });

    let error = handover_once(&path, temp.path(), &store).expect_err("Closing 必须失败");
    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32004,
            domain: Some(DomainErrorCode::Closing),
        }
    ));
    foreground.join().unwrap();
    service.shutdown().unwrap();
}

#[test]
fn service_maps_dropped_completion_sender_to_closing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    let mut service = start_service(&path, &store, events);
    let foreground = std::thread::spawn(move || {
        let SltermShellEvent::Handover { completion, .. } =
            event_rx.recv_timeout(TEST_TIMEOUT).unwrap()
        else {
            panic!("服务只能投递 handover 事件");
        };
        drop(completion);
    });

    let error = handover_once(&path, temp.path(), &store).expect_err("回执断连必须失败");
    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32004,
            domain: Some(DomainErrorCode::Closing),
        }
    ));
    foreground.join().unwrap();
    service.shutdown().unwrap();
}

#[test]
fn service_maps_disconnected_event_receiver_to_closing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    drop(event_rx);
    let mut service = start_service(&path, &store, events);

    let error = handover_once(&path, temp.path(), &store).expect_err("事件接收端断开必须失败");
    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32004,
            domain: Some(DomainErrorCode::Closing),
        }
    ));
    service.shutdown().unwrap();
}

#[test]
fn service_maps_receipt_timeout_to_not_ready() {
    assert_eq!(
        super::service::HANDOVER_REPLY_TIMEOUT,
        Duration::from_secs(4),
        "生产回执等待必须小于客户端 5 秒总 deadline"
    );
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    let mut service =
        LocalControlService::start_at(metadata(), events, &store, &path, SHORT_REPLY_TIMEOUT)
            .unwrap();
    let foreground = std::thread::spawn(move || {
        let SltermShellEvent::Handover { completion, .. } =
            event_rx.recv_timeout(TEST_TIMEOUT).unwrap()
        else {
            panic!("服务只能投递 handover 事件");
        };
        // 持有回执发送端但不答复，确保走超时而不是断连。
        std::thread::sleep(Duration::from_millis(250));
        drop(completion);
    });
    let started = Instant::now();

    let error = handover_once(&path, temp.path(), &store).expect_err("回执超时必须失败");
    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32001,
            domain: Some(DomainErrorCode::NotReady),
        }
    ));
    assert!(started.elapsed() < TEST_TIMEOUT);
    service.shutdown().unwrap();
    foreground.join().unwrap();
}

#[test]
fn service_preserves_queue_full_helper_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, _event_rx) = event_channel();
    for _ in 0..64 {
        events.try_send(SltermShellEvent::TrayQuit).unwrap();
    }
    let mut service = start_service(&path, &store, events);

    let error = handover_once(&path, temp.path(), &store).expect_err("队列满必须失败");
    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32003,
            domain: Some(DomainErrorCode::QueueFull),
        }
    ));
    service.shutdown().unwrap();
}

#[test]
fn service_shutdown_joins_inflight_receipt_wait_and_reclaims_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, event_rx) = event_channel();
    let mut service =
        LocalControlService::start_at(metadata(), events, &store, &path, SHORT_REPLY_TIMEOUT)
            .unwrap();
    let record = read_discovery_at(&path).unwrap();
    let endpoint = record.endpoint;
    let credential = load_test_credential(temp.path(), &store);
    let (client_done_tx, client_done_rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let result = handover(&record, credential, HandoverParams::Focus);
        client_done_tx.send(result).unwrap();
    });
    let SltermShellEvent::Handover { completion, .. } =
        event_rx.recv_timeout(TEST_TIMEOUT).unwrap()
    else {
        panic!("服务必须已投递 handover，worker 随后等待本次回执");
    };

    let started = Instant::now();
    service.shutdown().unwrap();
    let elapsed = started.elapsed();
    let client_result = client_done_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("shutdown 必须让在途 client 退出");
    client.join().unwrap();
    drop(completion);

    assert!(client_result.is_err());
    assert!(
        elapsed < Duration::from_secs(2),
        "在途回执 join 超过测试期限: {elapsed:?}"
    );
    assert!(!path.exists());
    assert!(TcpStream::connect(endpoint).is_err());
}

#[test]
fn service_shutdown_closes_listener_revokes_endpoint_and_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    let store = MemoryCredentialStore::default();
    let (events, _event_rx) = event_channel();
    let mut service = start_service(&path, &store, events);
    let endpoint = read_discovery_at(&path).unwrap().endpoint;

    service.shutdown().unwrap();
    service.shutdown().unwrap();
    assert!(!path.exists());
    assert!(TcpStream::connect(endpoint).is_err());
}

#[test]
fn service_shutdown_attempts_both_failures_and_returns_first_error() {
    let attempts = Cell::new(0);

    let error = super::service::shutdown_components(
        || {
            attempts.set(attempts.get() + 1);
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected server shutdown failure",
            ))
        },
        || {
            attempts.set(attempts.get() + 1);
            Err(io::Error::other("injected discovery revoke failure"))
        },
    )
    .expect_err("双失败必须汇总错误");

    assert_eq!(attempts.get(), 2, "server 失败不得跳过 discovery revoke");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "injected server shutdown failure");
}

#[test]
fn service_publish_failure_reports_real_server_cleanup_failure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    std::fs::create_dir(&path).unwrap();
    let store = MemoryCredentialStore::default();
    let store_ref = &store;
    let root = temp.path().to_owned();
    let (events, _event_rx) = event_channel();
    let bound_endpoint = Arc::new(Mutex::new(None));
    let bound_by_server = Arc::clone(&bound_endpoint);
    let client_handle = Arc::new(Mutex::new(None));
    let client_handle_by_server = Arc::clone(&client_handle);
    let (worker_started_tx, worker_started_rx) = mpsc::channel();
    let (release_worker_tx, release_worker_rx) = mpsc::channel();
    let release_worker_rx = Arc::new(Mutex::new(release_worker_rx));
    let (client_done_tx, client_done_rx) = mpsc::channel();

    let error = match LocalControlService::start_with(
        metadata(),
        events,
        store_ref,
        &path,
        TEST_TIMEOUT,
        move |credential, hello, _on_handover| {
            let server = LocalControlServer::start(
                credential,
                hello,
                move |_| -> Result<HandoverResponse, slterm_runtime_protocol::JsonRpcError> {
                    worker_started_tx.send(()).unwrap();
                    release_worker_rx.lock().unwrap().recv().unwrap();
                    panic!("injected publish-cleanup worker panic")
                },
            )?;
            *bound_by_server.lock().unwrap() = Some(server.endpoint().to_owned());
            let run = metadata().run;
            let record = slterm_runtime_protocol::DiscoveryRecord {
                protocol_version: 2,
                endpoint: server.endpoint().parse().unwrap(),
                run_id: run.run_id,
                process_id: run.process_id,
                process_created_at_ms: run.process_created_at_ms,
            };
            let credential = load_test_credential(&root, store_ref);
            let client = std::thread::spawn(move || {
                let result = handover(&record, credential, HandoverParams::Focus);
                // server shutdown 断开流后 client 才返回，再释放 worker panic。
                release_worker_tx.send(()).unwrap();
                client_done_tx.send(result).unwrap();
            });
            *client_handle_by_server.lock().unwrap() = Some(client);
            worker_started_rx
                .recv_timeout(TEST_TIMEOUT)
                .expect("worker 必须在发布失败前等待释放");
            Ok(server)
        },
    ) {
        Ok(_) => panic!("目录不能被发布为发现记录"),
        Err(error) => error,
    };

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        error.to_string().contains("os error 5"),
        "聚合后必须保留发布根因: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("control worker terminated unexpectedly"),
        "返回错误必须保留真实 server 回收失败: {error}"
    );
    assert!(path.is_dir());
    assert!(client_done_rx.recv_timeout(TEST_TIMEOUT).unwrap().is_err());
    client_handle
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .unwrap();
    let endpoint = bound_endpoint.lock().unwrap().take().unwrap();
    assert!(
        TcpStream::connect(&endpoint).is_err(),
        "即使 worker panic，listener 也必须关闭"
    );
}

#[test]
fn service_publish_failure_reclaims_listener_without_new_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("endpoint.json");
    std::fs::create_dir(&path).unwrap();
    let store = MemoryCredentialStore::default();
    let (events, _event_rx) = event_channel();
    let bound_endpoint = Arc::new(Mutex::new(None));
    let bound_by_server = Arc::clone(&bound_endpoint);

    let error = match LocalControlService::start_with(
        metadata(),
        events,
        &store,
        &path,
        TEST_TIMEOUT,
        move |credential, hello, on_handover| {
            let server =
                LocalControlServer::start(credential, hello, move |params| on_handover(params))?;
            *bound_by_server.lock().unwrap() = Some(server.endpoint().to_owned());
            Ok(server)
        },
    ) {
        Ok(_) => panic!("目录不能被发布为发现记录"),
        Err(error) => error,
    };
    assert!(error.kind() != io::ErrorKind::NotFound);
    assert!(path.is_dir());
    let endpoint = bound_endpoint.lock().unwrap().take().unwrap();
    assert!(
        TcpStream::connect(&endpoint).is_err(),
        "发布失败必须回收已绑定 listener"
    );
}
