//! LocalControlServer 的真实 loopback 生命周期测试。

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use slterm_runtime_protocol::{
    ControlScope, Credential, HandoverMethod, HandoverParams, HandoverRequest, HandoverResponse,
    HelloMethod, HelloParams, HelloRequest, HelloResponse, JsonRpcId, JsonRpcRequest,
};

use super::LocalControlServer;

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
    let server =
        LocalControlServer::start(Credential::new("sk-test"), hello_response(), move |_| {
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
        LocalControlServer::start(Credential::new("sk-test"), hello_response(), move |_| {
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
        Credential::new("sk-test"),
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
        Credential::new("sk-test"),
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
fn local_control_shutdown_continues_after_worker_join_failure() {
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
    let mut server = LocalControlServer::start(
        Credential::new("sk-test"),
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
