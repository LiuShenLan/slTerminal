//! LocalControlServer 的真实 loopback 生命周期测试。

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slterm_runtime_protocol::{
    ControlScope, Credential, DocumentResource, EmptyParams, EventsSubscribeParams,
    EventsSubscribedResponse, HandoverMethod, HandoverParams, HandoverRequest, HandoverResponse,
    HelloMethod, HelloParams, HelloRequest, HelloResponse, JsonRpcError, JsonRpcErrorData,
    JsonRpcId, JsonRpcRequest, OperationQueryParams, OperationResource, OperationStatus,
    OperationWaitParams, PanelGroupResource, PanelGroupSplitParams, PanelGroupSplitRequest,
    PanelOpenSettingsParams, ProjectActivateParams, ProjectActivateRequest, ProjectResource,
    RuntimeEvent, RuntimeMethod, RuntimeRequest, SnapshotResources, SplitDirectionWire,
    TerminalResource, ViewResource, ViewportSize,
};

use super::LocalControlServer;
use crate::secrets::SecretBytes;

fn test_secret(value: &str) -> SecretBytes {
    SecretBytes::new(value.to_owned().into_bytes())
}

fn unavailable_mutation(
    _: super::mutations::RuntimeMutationRequest,
) -> Result<OperationResource, JsonRpcError> {
    Err(JsonRpcError::new(
        -32001,
        "mutation not ready",
        Some(JsonRpcErrorData {
            code: slterm_runtime_protocol::DomainErrorCode::NotReady,
        }),
    ))
}

// 负载裕度，非断言/语义放宽：等待真实 loopback 线程调度与关闭。
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

struct ChunkedReader {
    inner: std::io::Cursor<Vec<u8>>,
    chunk_size: usize,
}

impl ChunkedReader {
    fn new(input: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            inner: std::io::Cursor::new(input),
            chunk_size,
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let limit = self.chunk_size.min(output.len());
        self.inner.read(&mut output[..limit])
    }
}

fn hello_response() -> HelloResponse {
    HelloResponse::new("store", "stream", 7, "protocol-hash", "schema-hash")
}

fn start_server() -> (LocalControlServer, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start(test_secret("sk-test"), hello_response(), move |_| {
        callback_calls.fetch_add(1, Ordering::SeqCst);
        Ok(HandoverResponse::accepted())
    })
    .unwrap();
    (server, calls)
}

fn connect(server: &LocalControlServer) -> TcpStream {
    TcpStream::connect(server.endpoint()).unwrap()
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, value: &T) {
    let mut encoded = serde_json::to_vec(value).unwrap();
    encoded.push(b'\n');
    stream.write_all(&encoded).unwrap();
}

fn hello(credential: &str) -> HelloRequest {
    JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: "loopback-test".to_owned(),
            credential: Credential::new(credential),
            scope: ControlScope::Handover,
        },
    )
}

fn focus() -> HandoverRequest {
    JsonRpcRequest::new(
        JsonRpcId::Number(2),
        HandoverMethod::InstanceHandover,
        HandoverParams::Focus,
    )
}

fn read_line_result(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut line = String::new();
    std::io::BufReader::new(stream).read_line(&mut line)?;
    Ok(line)
}

fn read_line(stream: &mut TcpStream) -> String {
    read_line_result(stream).unwrap()
}

#[test]
fn local_control_accepts_hello_and_focus_over_loopback() {
    let (mut server, calls) = start_server();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let hello_reply = read_line(&mut stream);
    assert!(hello_reply.contains("store"));
    assert!(hello_reply.contains("instance.handover"));

    write_json(&mut stream, &focus());
    let focus_reply = read_line(&mut stream);
    assert!(focus_reply.contains("accepted"));
    assert!(focus_reply.contains("true"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn local_control_invalid_credential_closes_without_response() {
    let (mut server, calls) = start_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(&mut stream, &hello("sk-wrong"));

    assert_eq!(read_line(&mut stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn local_control_rejects_handover_before_hello() {
    let (mut server, calls) = start_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(&mut stream, &focus());

    assert_eq!(read_line(&mut stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn local_control_rejects_oversized_frame() {
    let (mut server, calls) = start_server();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    stream.write_all(&vec![b' '; 128 * 1024 + 1]).unwrap();

    assert_eq!(read_line(&mut stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn local_control_shutdown_releases_open_connection_and_listener() {
    let (mut server, calls) = start_server();
    let endpoint = server.endpoint().to_owned();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);

    server.shutdown().unwrap();
    server.shutdown().unwrap();
    assert!(TcpStream::connect(&endpoint).is_err());
    assert_eq!(read_line(&mut stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn local_control_accepts_near_budget_frame_with_next_frame_prefix() {
    let (mut server, calls) = start_server();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);
    let mut frame = serde_json::to_vec(&focus()).unwrap();
    let next_prefix = b"{\"jsonrpc\":";
    let padding = 128 * 1024 - frame.len() - 1;
    frame.extend_from_slice(&vec![b' '; padding]);
    frame.push(b'\n');
    frame.extend_from_slice(next_prefix);
    stream.write_all(&frame).unwrap();

    let response = read_line(&mut stream);
    assert!(response.contains("accepted"));
    assert!(response.contains("true"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn local_control_frame_budget_excludes_next_frame_prefix() {
    let mut input = serde_json::to_vec(&focus()).unwrap();
    let next_prefix = b"{\"jsonrpc\":";
    let padding = 128 * 1024 - input.len() - 2;
    input.extend_from_slice(&vec![b' '; padding]);
    input.push(b'\n');
    input.extend_from_slice(next_prefix);
    let mut reader = ChunkedReader::new(input, 1023);
    let mut buffer = Vec::new();

    let frame = super::server::read_frame(
        &mut reader,
        &mut buffer,
        std::time::Instant::now() + super::server::REQUEST_TIMEOUT,
        &AtomicBool::new(false),
    )
    .unwrap()
    .unwrap();
    assert_eq!(frame.len(), 128 * 1024 - 1);
    assert_eq!(buffer, next_prefix);
}

#[test]
fn local_control_rejected_handover_is_not_a_success_result() {
    let mut server =
        LocalControlServer::start(test_secret("sk-test"), hello_response(), move |_| {
            Ok(serde_json::from_str::<HandoverResponse>("{\"accepted\":false}").unwrap())
        })
        .unwrap();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);
    write_json(&mut stream, &focus());

    let response = read_line(&mut stream);
    assert!(response.contains("error"));
    assert!(!response.contains("accepted"));
    server.shutdown().unwrap();
}

#[test]
fn local_control_response_budget_is_enforced() {
    let mut server = LocalControlServer::start(
        test_secret("sk-test"),
        HelloResponse::new(
            "x".repeat(1024 * 1024),
            "stream",
            7,
            "protocol-hash",
            "schema-hash",
        ),
        move |_| Ok(HandoverResponse::accepted()),
    )
    .unwrap();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(&mut stream, &hello("sk-test"));

    assert_eq!(read_line(&mut stream), "");
    server.shutdown().unwrap();
}

#[test]
fn local_control_worker_panic_closes_connection() {
    let mut server = LocalControlServer::start(
        test_secret("sk-test"),
        hello_response(),
        move |_| -> Result<HandoverResponse, slterm_runtime_protocol::JsonRpcError> {
            panic!("injected worker panic")
        },
    )
    .unwrap();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);
    write_json(&mut stream, &focus());
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();

    assert_eq!(read_line(&mut stream), "");
    server.shutdown().unwrap();
}

#[test]
fn local_control_shutdown_replays_worker_join_failure() {
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
    let mut server = LocalControlServer::start(
        test_secret("sk-test"),
        hello_response(),
        move |_| -> Result<HandoverResponse, slterm_runtime_protocol::JsonRpcError> {
            started_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            panic!("injected repeated-shutdown worker panic")
        },
    )
    .unwrap();
    let mut stream = connect(&server);
    write_json(&mut stream, &hello("sk-test"));
    let _ = read_line(&mut stream);
    write_json(&mut stream, &focus());
    started_rx.recv().unwrap();

    std::thread::scope(|scope| {
        let shutdown = scope.spawn(|| server.shutdown());
        assert_eq!(read_line(&mut stream), "");
        release_tx.send(()).unwrap();
        assert!(shutdown.join().unwrap().is_err());
    });
    assert!(
        server.shutdown().is_err(),
        "后续 shutdown 必须保留首次 join 终止错误"
    );
    assert_eq!(server.active_workers(), 0);
}

#[test]
fn local_control_shutdown_continues_after_worker_join_failure() {
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
    let mut server = LocalControlServer::start(
        test_secret("sk-test"),
        hello_response(),
        move |_| -> Result<HandoverResponse, slterm_runtime_protocol::JsonRpcError> {
            started_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            panic!("injected worker panic")
        },
    )
    .unwrap();
    let mut first = connect(&server);
    write_json(&mut first, &hello("sk-test"));
    let _ = read_line(&mut first);
    let mut second = connect(&server);
    write_json(&mut second, &hello("sk-test"));
    let _ = read_line(&mut second);
    write_json(&mut first, &focus());
    started_rx.recv().unwrap();

    std::thread::scope(|scope| {
        let shutdown = scope.spawn(|| server.shutdown());
        assert_eq!(read_line(&mut first), "");
        release_tx.send(()).unwrap();
        let result = shutdown.join().unwrap();
        assert!(result.is_err());
    });
    assert_eq!(read_line(&mut second), "");
    assert_eq!(server.active_workers(), 0);
}

#[test]
fn local_control_enforces_connection_limit() {
    let (mut server, _calls) = start_server();
    let mut streams = Vec::new();
    for _ in 0..64 {
        let mut stream = connect(&server);
        write_json(&mut stream, &hello("sk-test"));
        let _ = read_line(&mut stream);
        streams.push(stream);
    }
    let mut extra = connect(&server);
    extra.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(&mut extra, &hello("sk-test"));

    match read_line_result(&mut extra) {
        Ok(line) => assert_eq!(line, ""),
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset),
    }
    drop(streams);
    server.shutdown().unwrap();
}

#[test]
fn local_control_handshake_times_out_after_five_seconds() {
    let (mut server, _calls) = start_server();
    let mut stream = connect(&server);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let started = std::time::Instant::now();

    assert_eq!(read_line(&mut stream), "");
    assert!(started.elapsed() >= Duration::from_secs(4));
    assert!(started.elapsed() < Duration::from_secs(10));
    server.shutdown().unwrap();
}

#[test]
fn local_control_request_deadline_expires_without_waiting() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();
    let _ = listener.accept().unwrap();
    let error = super::server::read_frame(
        &mut stream,
        &mut Vec::new(),
        std::time::Instant::now(),
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(super::server::REQUEST_TIMEOUT, Duration::from_secs(600));
    assert_eq!(super::server::HANDSHAKE_TIMEOUT, Duration::from_secs(5));
}

fn hello_scoped(credential: &str, scope: ControlScope) -> HelloRequest {
    JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: "loopback-test".to_owned(),
            credential: Credential::new(credential),
            scope,
        },
    )
}

fn start_dual_scope_server() -> (LocalControlServer, Arc<AtomicUsize>) {
    start_dual_scope_server_with_snapshot(|| Ok(SnapshotResources::default()))
}

fn start_dual_scope_server_with_snapshot<S>(
    on_snapshot: S,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    S: Fn() -> Result<SnapshotResources, JsonRpcError> + Send + Sync + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        super::server::RuntimeCallbacks {
            snapshot: Arc::new(on_snapshot),
            operation: Arc::new(|_| Ok(None)),
            events: Arc::new(|_| {
                Err(JsonRpcError::new(
                    -32001,
                    "event subscription not ready",
                    Some(JsonRpcErrorData {
                        code: slterm_runtime_protocol::DomainErrorCode::NotReady,
                    }),
                ))
            }),
            mutation: Arc::new(unavailable_mutation),
        },
    )
    .unwrap();
    (server, calls)
}

fn runtime_request(method: RuntimeMethod) -> RuntimeRequest {
    RuntimeRequest::new(JsonRpcId::Number(2), method, EmptyParams {})
}

fn operation_get(operation_id: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(3),
        RuntimeMethod::OperationGet,
        OperationQueryParams {
            operation_id: operation_id.to_owned(),
        },
    )
}

fn operation_wait(operation_id: &str, timeout_ms: u64) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(4),
        RuntimeMethod::OperationWait,
        OperationWaitParams {
            operation_id: operation_id.to_owned(),
            timeout_ms: Some(timeout_ms),
        },
    )
}

fn operation_resource(status: OperationStatus) -> OperationResource {
    OperationResource {
        operation_id: "operation-a".to_owned(),
        status,
        result: status
            .eq(&OperationStatus::Succeeded)
            .then(|| serde_json::json!({"revision": 3})),
        error: matches!(
            status,
            OperationStatus::Failed | OperationStatus::Cancelled | OperationStatus::OutcomeUnknown
        )
        .then(|| serde_json::json!({"code": "side_effect_unknown"})),
    }
}

fn start_dual_scope_server_with_operation<O>(
    on_operation: O,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    O: Fn(&str) -> Result<Option<OperationResource>, JsonRpcError> + Send + Sync + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        super::server::RuntimeCallbacks {
            snapshot: Arc::new(|| Ok(SnapshotResources::default())),
            operation: Arc::new(on_operation),
            events: Arc::new(|_| {
                Err(JsonRpcError::new(
                    -32001,
                    "event subscription not ready",
                    Some(JsonRpcErrorData {
                        code: slterm_runtime_protocol::DomainErrorCode::NotReady,
                    }),
                ))
            }),
            mutation: Arc::new(unavailable_mutation),
        },
    )
    .unwrap();
    (server, calls)
}

fn subscribe_request() -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(5),
        RuntimeMethod::EventsSubscribe,
        EventsSubscribeParams {
            store_epoch: "store".to_owned(),
            stream_epoch: "stream".to_owned(),
            since_revision: 0,
        },
    )
}

fn unsubscribe_request() -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(6),
        RuntimeMethod::EventsUnsubscribe,
        EmptyParams {},
    )
}

fn project_activate_request(project_id: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(7),
        RuntimeMethod::ProjectActivate,
        ProjectActivateParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-activate".to_owned(),
            expected_revision: None,
            request: ProjectActivateRequest {
                project_id: project_id.to_owned(),
            },
        },
    )
}

fn start_dual_scope_server_with_mutation<M>(
    on_mutation: M,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    M: Fn(super::mutations::RuntimeMutationRequest) -> Result<OperationResource, JsonRpcError>
        + Send
        + Sync
        + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        super::server::RuntimeCallbacks {
            snapshot: Arc::new(|| Ok(sample_resources())),
            operation: Arc::new(|_| Ok(None)),
            events: Arc::new(|_| {
                Err(JsonRpcError::new(
                    -32001,
                    "event subscription not ready",
                    Some(JsonRpcErrorData {
                        code: slterm_runtime_protocol::DomainErrorCode::NotReady,
                    }),
                ))
            }),
            mutation: Arc::new(on_mutation),
        },
    )
    .unwrap();
    (server, calls)
}

fn terminal_view_key() -> String {
    slterm_workspace::ViewKey::Terminal {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".parse().unwrap(),
    }
    .to_canonical_json()
    .unwrap()
}

fn panel_split_request(view: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(8),
        RuntimeMethod::PanelGroupSplit,
        PanelGroupSplitParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-split".to_owned(),
            expected_revision: None,
            request: PanelGroupSplitRequest {
                view: view.to_owned(),
                target_group: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a".to_owned(),
                direction: SplitDirectionWire::LeftRight,
                ratio: 0.5,
                viewport: ViewportSize {
                    width: 800.0,
                    height: 600.0,
                },
            },
        },
    )
}

#[test]
fn runtime_panel_mutations_route_through_dispatcher_and_reject_bad_views() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "panel_group.split");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-split".to_owned(),
            status: OperationStatus::Succeeded,
            result: Some(serde_json::json!({})),
            error: None,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &panel_split_request(&terminal_view_key()));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    write_json(&mut stream, &panel_split_request("not-a-view-key"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_panel_open_settings_routes_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "panel.open_settings");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-settings".to_owned(),
            status: OperationStatus::Succeeded,
            result: Some(serde_json::json!({})),
            error: None,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    let request = RuntimeRequest::new(
        JsonRpcId::Number(9),
        RuntimeMethod::PanelOpenSettings,
        PanelOpenSettingsParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-settings".to_owned(),
            expected_revision: None,
            request: EmptyParams {},
        },
    );
    write_json(&mut stream, &request);
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_project_list_and_mutations_route_through_callbacks() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "project.activate");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-project".to_owned(),
            status: OperationStatus::Succeeded,
            result: Some(serde_json::json!({})),
            error: None,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &runtime_request(RuntimeMethod::ProjectList));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["projects"][0]["display_name"], "Repo");

    write_json(
        &mut stream,
        &project_activate_request("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    // 非法稳定身份在调用派发器前拒绝。
    write_json(&mut stream, &project_activate_request("not-a-uuid"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_events<E>(on_events: E) -> (LocalControlServer, Arc<AtomicUsize>)
where
    E: Fn(EventsSubscribeParams) -> Result<super::events::Subscription, JsonRpcError>
        + Send
        + Sync
        + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        super::server::RuntimeCallbacks {
            snapshot: Arc::new(|| Ok(SnapshotResources::default())),
            operation: Arc::new(|_| Ok(None)),
            events: Arc::new(on_events),
            mutation: Arc::new(unavailable_mutation),
        },
    )
    .unwrap();
    (server, calls)
}

#[test]
fn runtime_subscription_streams_events_and_accepts_unsubscribe() {
    let (events_tx, events_rx) = mpsc::sync_channel(super::events::CLIENT_QUEUE_CAPACITY);
    let receiver_slot = Arc::new(Mutex::new(Some(events_rx)));
    let slot = Arc::clone(&receiver_slot);
    let (mut server, _calls) = start_dual_scope_server_with_events(move |_| {
        let events = slot
            .lock()
            .expect("订阅槽")
            .take()
            .expect("每次连接只订阅一次");
        Ok(super::events::Subscription {
            confirmed: EventsSubscribedResponse {
                stream_epoch: "stream".to_owned(),
                revision: 0,
            },
            events,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &subscribe_request());
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["stream_epoch"], "stream");
    assert_eq!(reply["result"]["revision"], 0);

    events_tx
        .send(RuntimeEvent {
            revision: 1,
            kind: "project.activated".to_owned(),
            resource: "project-a".to_owned(),
            generation: 1,
        })
        .unwrap();
    let notification: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(notification["method"], "events.event");
    assert_eq!(notification["params"]["revision"], 1);
    assert_eq!(notification["params"]["kind"], "project.activated");
    assert_eq!(notification["params"]["resource"], "project-a");

    write_json(&mut stream, &unsubscribe_request());
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"], serde_json::json!({}));
    assert_eq!(read_line(&mut stream), "", "取消订阅后连接必须结束");
    server.shutdown().unwrap();
}

#[test]
fn runtime_operation_get_returns_persisted_result_or_structured_unknown() {
    let (mut server, calls) = start_dual_scope_server_with_operation(|operation_id| {
        Ok((operation_id == "operation-a")
            .then(|| operation_resource(OperationStatus::OutcomeUnknown)))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &operation_get("operation-a"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["operation_id"], "operation-a");
    assert_eq!(reply["result"]["status"], "outcome_unknown");
    assert_eq!(reply["result"]["error"]["code"], "side_effect_unknown");

    write_json(&mut stream, &operation_get("operation-missing"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "unknown_operation");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_operation_wait_polls_until_terminal_then_returns_result() {
    let polls = Arc::new(AtomicUsize::new(0));
    let polls_by_callback = Arc::clone(&polls);
    let (mut server, _calls) = start_dual_scope_server_with_operation(move |operation_id| {
        assert_eq!(operation_id, "operation-a");
        let round = polls_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(Some(if round == 0 {
            operation_resource(OperationStatus::Running)
        } else {
            operation_resource(OperationStatus::Succeeded)
        }))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &operation_wait("operation-a", 2_000));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(reply["result"]["result"]["revision"], 3);
    assert!(polls.load(Ordering::SeqCst) >= 2);
    server.shutdown().unwrap();
}

fn sample_resources() -> SnapshotResources {
    SnapshotResources {
        projects: vec![ProjectResource {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            display_name: "Repo".to_owned(),
            root: "D:/work/a".to_owned(),
            active: true,
        }],
        groups: vec![PanelGroupResource {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            group_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c".to_owned(),
            views: vec![ViewResource {
                kind: "terminal".to_owned(),
                key: r#"{"kind":"terminal","version":1}"#.to_owned(),
            }],
            active_view: r#"{"kind":"terminal","version":1}"#.to_owned(),
        }],
        terminals: vec![TerminalResource {
            terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            number: 1,
            cwd: "D:/work/a".to_owned(),
            shell: "pwsh.exe".to_owned(),
            title: None,
            run_intent: "launch".to_owned(),
        }],
        documents: vec![DocumentResource {
            document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".to_owned(),
            path: "D:/work/a/README.md".to_owned(),
        }],
    }
}

#[test]
fn runtime_describe_returns_registry_and_contract_hashes() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(value["result"]["protocol_sha256"], "protocol-hash");
    assert_eq!(value["result"]["schema_sha256"], "schema-hash");
    let methods: Vec<&str> = value["result"]["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|method| method["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "runtime.hello",
            "runtime.describe",
            "runtime.snapshot",
            "runtime.operation_get",
            "runtime.operation_wait",
            "events.subscribe",
            "events.unsubscribe",
            "project.list",
            "project.activate",
            "project.rename",
            "project.reorder",
            "project.remove",
            "panel_group.split",
            "panel_group.merge",
            "panel_group.resize",
            "panel.activate",
            "panel.move",
            "panel.open_settings",
            "terminal.create",
            "instance.handover"
        ]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_snapshot_returns_injected_resources_with_hello_generation() {
    let (mut server, calls) = start_dual_scope_server_with_snapshot(|| Ok(sample_resources()));
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Snapshot));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(value["result"]["store_epoch"], "store");
    assert_eq!(value["result"]["stream_epoch"], "stream");
    assert_eq!(value["result"]["revision"], 7);
    let resources = &value["result"]["resources"];
    assert_eq!(resources["projects"][0]["display_name"], "Repo");
    assert_eq!(resources["terminals"][0]["run_intent"], "launch");
    assert_eq!(resources["documents"][0]["path"], "D:/work/a/README.md");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_snapshot_failure_returns_structured_error_and_keeps_connection() {
    let (mut server, _calls) = start_dual_scope_server_with_snapshot(|| {
        Err(JsonRpcError::new(
            -32001,
            "not ready",
            Some(JsonRpcErrorData {
                code: slterm_runtime_protocol::DomainErrorCode::NotReady,
            }),
        ))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Snapshot));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(value["error"]["data"]["code"], "not_ready");

    // 结构化错误不关闭连接；后续 describe 仍可用。
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));
    assert!(read_line(&mut stream).contains("protocol-hash"));
    server.shutdown().unwrap();
}

#[test]
fn handover_connection_rejects_runtime_methods_after_handshake() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test", ControlScope::Handover),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));

    assert_eq!(
        read_line(&mut stream),
        "",
        "handover 连接不得执行 Runtime 方法"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_scope_handshake_uses_its_own_credential() {
    let (mut server, _calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    let reply = read_line(&mut stream);

    assert!(
        reply.contains("store"),
        "Runtime 作用域必须完成握手: {reply}"
    );
    server.shutdown().unwrap();
}

#[test]
fn runtime_and_handover_credentials_are_isolated() {
    let (mut server, calls) = start_dual_scope_server();

    // Runtime 作用域不接受 handover 凭据。
    let mut runtime_stream = connect(&server);
    runtime_stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut runtime_stream,
        &hello_scoped("sk-test", ControlScope::Runtime),
    );
    assert_eq!(read_line(&mut runtime_stream), "");

    // Handover 作用域不接受 runtime 凭据。
    let mut handover_stream = connect(&server);
    handover_stream
        .set_read_timeout(Some(TEST_TIMEOUT))
        .unwrap();
    write_json(
        &mut handover_stream,
        &hello_scoped("sk-test-runtime", ControlScope::Handover),
    );
    assert_eq!(read_line(&mut handover_stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_v1_rejected() {
    let (mut server, _calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    let v1 = JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 1,
            client_id: "v1-client".to_owned(),
            credential: Credential::new("sk-test-runtime"),
            scope: ControlScope::Runtime,
        },
    );
    write_json(&mut stream, &v1);

    assert_eq!(read_line(&mut stream), "", "v1 协议必须无响应断开");
    server.shutdown().unwrap();
}

#[test]
fn runtime_connection_rejects_handover_method_after_handshake() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &focus());

    assert_eq!(read_line(&mut stream), "", "Runtime 连接不得执行 handover");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}
