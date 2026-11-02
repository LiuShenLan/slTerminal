//! 一次性 handover 客户端的真实 loopback 与异常服务测试。

use std::io::{self, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;
use slterm_runtime_protocol::{
    ControlMethod, ControlScope, Credential, DiscoveryRecord, DomainErrorCode, HandoverMethod,
    HandoverParams, HandoverRequest, HandoverResponse, HelloMethod, HelloRequest, HelloResponse,
    JsonRpcError, JsonRpcErrorData, JsonRpcId, JsonRpcResponse,
};

use super::client::{HandoverError, find_newline_since, handover};
use super::server::LocalControlServer;
use crate::secrets::SecretBytes;

const TEST_CREDENTIAL: &str = "sk-test-handover";
const SENSITIVE_FIXTURE: &str = "sk-test-sensitive-client-fixture";
const RESPONSE_BUDGET: usize = 1024 * 1024;

fn record(endpoint: &str) -> DiscoveryRecord {
    DiscoveryRecord {
        protocol_version: 2,
        endpoint: endpoint.parse().unwrap(),
        run_id: "run-client-test".to_owned(),
        process_id: 123,
        process_created_at_ms: 456,
    }
}

fn credential(value: &str) -> SecretBytes {
    SecretBytes::new(value.to_owned().into_bytes())
}

fn hello_response() -> HelloResponse {
    HelloResponse::new("store", "stream", 7, "protocol-hash", "schema-hash")
}

fn start_real_server(
    expected_credential: &str,
) -> (LocalControlServer, Arc<Mutex<Vec<HandoverParams>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let callback_received = Arc::clone(&received);
    let server = LocalControlServer::start(
        Credential::new(expected_credential),
        hello_response(),
        move |params| {
            callback_received.lock().unwrap().push(params);
            Ok(HandoverResponse::accepted())
        },
    )
    .unwrap();
    (server, received)
}

fn spawn_fake<F>(handler: F) -> (DiscoveryRecord, JoinHandle<()>)
where
    F: FnOnce(TcpStream) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let record = record(&listener.local_addr().unwrap().to_string());
    let handle = thread::spawn(move || {
        let accept_deadline = Instant::now() + Duration::from_secs(6);
        let (stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < accept_deadline, "假服务等待连接超时");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("假服务接受连接失败: {error}"),
            }
        };
        let stream = stream;
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        handler(stream);
    });
    (record, handle)
}

fn call_fake<F>(handler: F) -> Result<(), HandoverError>
where
    F: FnOnce(TcpStream) + Send + 'static,
{
    let (record, handle) = spawn_fake(handler);
    let result = handover(&record, credential(TEST_CREDENTIAL), HandoverParams::Focus);
    handle.join().unwrap();
    result
}

fn read_frame(stream: &mut TcpStream) -> Vec<u8> {
    let mut frame = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let count = stream.read(&mut byte).unwrap();
        assert_eq!(count, 1, "假服务读取请求时遇到 EOF");
        if byte[0] == b'\n' {
            return frame;
        }
        frame.push(byte[0]);
        assert!(frame.len() <= 128 * 1024, "客户端请求超过测试预算");
    }
}

fn read_json<T: DeserializeOwned>(stream: &mut TcpStream) -> T {
    serde_json::from_slice(&read_frame(stream)).unwrap()
}

fn write_json<T: Serialize>(stream: &mut TcpStream, value: &T) {
    let mut encoded = serde_json::to_vec(value).unwrap();
    encoded.push(b'\n');
    stream.write_all(&encoded).unwrap();
}

fn read_hello(stream: &mut TcpStream) -> HelloRequest {
    let request: HelloRequest = read_json(stream);
    assert_eq!(request.jsonrpc, "2.0");
    assert_eq!(request.id, JsonRpcId::Number(1));
    assert_eq!(request.method, HelloMethod::RuntimeHello);
    assert_eq!(request.params.protocol_version, 2);
    assert_eq!(request.params.client_id, "slterm-handover");
    assert_eq!(request.params.credential.as_str(), TEST_CREDENTIAL);
    assert_eq!(request.params.scope, ControlScope::Handover);
    request
}

fn accept_hello(stream: &mut TcpStream) {
    let request = read_hello(stream);
    write_json(
        stream,
        &JsonRpcResponse::result(request.id, hello_response()),
    );
}

fn read_handover(stream: &mut TcpStream) -> HandoverRequest {
    let request: HandoverRequest = read_json(stream);
    assert_eq!(request.jsonrpc, "2.0");
    assert_eq!(request.id, JsonRpcId::Number(2));
    assert_eq!(request.method, HandoverMethod::InstanceHandover);
    request
}

fn expect_connection_closed(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut byte = [0_u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => {}
        Ok(_) => panic!("hello 失败后不得发送 handover"),
        Err(error) => panic!("客户端未关闭失败连接: {error}"),
    }
}

fn sized_hello_response(id: JsonRpcId, encoded_len: usize) -> JsonRpcResponse<HelloResponse> {
    let mut hello = hello_response();
    hello.store_epoch.clear();
    let overhead = serde_json::to_vec(&JsonRpcResponse::result(id.clone(), hello.clone()))
        .unwrap()
        .len();
    assert!(encoded_len >= overhead, "测试预算小于响应固定字段");
    hello.store_epoch = "x".repeat(encoded_len - overhead);
    let response = JsonRpcResponse::result(id, hello);
    assert_eq!(serde_json::to_vec(&response).unwrap().len(), encoded_len);
    response
}

fn assert_redacted(error: &HandoverError, sensitive: &str) {
    let display = error.to_string();
    let debug = format!("{error:?}");
    assert!(
        !display.contains(sensitive),
        "Display 泄露敏感值: {display}"
    );
    assert!(!debug.contains(sensitive), "Debug 泄露敏感值: {debug}");
}

#[test]
fn client_focus_succeeds_against_real_loopback_server() {
    let (mut server, received) = start_real_server(TEST_CREDENTIAL);

    handover(
        &record(server.endpoint()),
        credential(TEST_CREDENTIAL),
        HandoverParams::Focus,
    )
    .unwrap();

    assert_eq!(*received.lock().unwrap(), [HandoverParams::Focus]);
    server.shutdown().unwrap();
}

#[test]
fn client_serializes_every_handover_intent_through_dto() {
    let (mut server, received) = start_real_server(TEST_CREDENTIAL);
    let record = record(server.endpoint());
    let cases = [
        HandoverParams::Focus,
        HandoverParams::OpenProject {
            project_root: "D:/work/project".to_owned(),
        },
        HandoverParams::OpenFile {
            path: "D:/work/project/README.md".to_owned(),
        },
        HandoverParams::NewTerminal {
            project_root: Some("D:/work/project".to_owned()),
            cwd: Some("D:/work/project/src".to_owned()),
        },
        HandoverParams::NewTerminal {
            project_root: None,
            cwd: None,
        },
    ];

    for params in cases.iter().cloned() {
        handover(&record, credential(TEST_CREDENTIAL), params).unwrap();
    }

    assert_eq!(*received.lock().unwrap(), cases);
    server.shutdown().unwrap();
}

#[test]
fn client_wrong_credential_fails_on_eof_without_leaking_secret() {
    let (mut server, received) = start_real_server(TEST_CREDENTIAL);

    let error = handover(
        &record(server.endpoint()),
        credential(SENSITIVE_FIXTURE),
        HandoverParams::Focus,
    )
    .expect_err("错误凭据必须失败");

    assert!(matches!(
        error,
        HandoverError::Io(ref error) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    assert_redacted(&error, SENSITIVE_FIXTURE);
    assert!(received.lock().unwrap().is_empty());
    server.shutdown().unwrap();
}

#[test]
fn client_missing_handover_capability_stops_before_handover() {
    let error = call_fake(|mut stream| {
        let request = read_hello(&mut stream);
        let mut hello = hello_response();
        hello
            .capabilities
            .methods
            .remove(ControlMethod::InstanceHandover.as_str());
        write_json(&mut stream, &JsonRpcResponse::result(request.id, hello));
        expect_connection_closed(&mut stream);
    })
    .expect_err("缺少 handover 能力必须失败");

    assert!(matches!(error, HandoverError::MissingHandoverCapability));
}

#[derive(Clone, Copy)]
enum BadHelloEnvelope {
    WrongId,
    WrongVersion,
}

#[test]
fn client_invalid_hello_id_or_version_stops_before_handover() {
    for case in [BadHelloEnvelope::WrongId, BadHelloEnvelope::WrongVersion] {
        let error = call_fake(move |mut stream| {
            let request = read_hello(&mut stream);
            let response = match case {
                BadHelloEnvelope::WrongId => {
                    JsonRpcResponse::result(JsonRpcId::Number(99), hello_response())
                }
                BadHelloEnvelope::WrongVersion => JsonRpcResponse::Result {
                    jsonrpc: "1.0".to_owned(),
                    id: request.id,
                    result: hello_response(),
                },
            };
            write_json(&mut stream, &response);
            expect_connection_closed(&mut stream);
        })
        .expect_err("hello 响应信封不匹配必须失败");

        assert!(matches!(error, HandoverError::InvalidResponse));
    }
}

#[derive(Clone, Copy)]
enum BadHandoverEnvelope {
    WrongId,
    WrongVersion,
}

#[test]
fn client_invalid_handover_id_or_version_fails() {
    for case in [
        BadHandoverEnvelope::WrongId,
        BadHandoverEnvelope::WrongVersion,
    ] {
        let error = call_fake(move |mut stream| {
            accept_hello(&mut stream);
            let request = read_handover(&mut stream);
            let response = match case {
                BadHandoverEnvelope::WrongId => {
                    JsonRpcResponse::result(JsonRpcId::Number(99), HandoverResponse::accepted())
                }
                BadHandoverEnvelope::WrongVersion => JsonRpcResponse::Result {
                    jsonrpc: "1.0".to_owned(),
                    id: request.id,
                    result: HandoverResponse::accepted(),
                },
            };
            write_json(&mut stream, &response);
        })
        .expect_err("handover 响应信封不匹配必须失败");

        assert!(matches!(error, HandoverError::InvalidResponse));
    }
}

#[test]
fn client_accepted_false_is_not_success() {
    let error = call_fake(|mut stream| {
        accept_hello(&mut stream);
        let request = read_handover(&mut stream);
        let rejected = serde_json::from_str::<HandoverResponse>("{\"accepted\":false}").unwrap();
        write_json(&mut stream, &JsonRpcResponse::result(request.id, rejected));
    })
    .expect_err("accepted=false 必须失败");

    assert!(matches!(error, HandoverError::HandoverRejected));
}

#[test]
fn client_structured_closing_preserves_only_closed_codes() {
    let error = call_fake(|mut stream| {
        accept_hello(&mut stream);
        let request = read_handover(&mut stream);
        let response = JsonRpcResponse::<HandoverResponse>::error(
            request.id,
            JsonRpcError::new(
                -32004,
                format!("closing; echo={SENSITIVE_FIXTURE}"),
                Some(JsonRpcErrorData {
                    code: DomainErrorCode::Closing,
                }),
            ),
        );
        write_json(&mut stream, &response);
    })
    .expect_err("结构化 Closing 必须失败");

    assert!(matches!(
        error,
        HandoverError::Server {
            code: -32004,
            domain: Some(DomainErrorCode::Closing),
        }
    ));
    assert_redacted(&error, SENSITIVE_FIXTURE);
}

#[test]
fn client_corrupted_response_fails_without_echoing_raw_bytes() {
    let error = call_fake(|mut stream| {
        read_hello(&mut stream);
        stream
            .write_all(format!("{{not-json:{SENSITIVE_FIXTURE}\n").as_bytes())
            .unwrap();
    })
    .expect_err("损坏响应必须失败");

    assert!(matches!(error, HandoverError::InvalidResponse));
    assert_redacted(&error, SENSITIVE_FIXTURE);
}

#[test]
fn client_newline_scan_skips_already_scanned_prefix() {
    let frame = b"old\nnew\n";

    assert_eq!(find_newline_since(frame, 4), Some(7));
    assert_eq!(find_newline_since(frame, 7), Some(7));
    assert_eq!(find_newline_since(frame, 8), None);
}

#[test]
fn client_accepts_response_json_at_exact_budget_before_newline() {
    call_fake(|mut stream| {
        let request = read_hello(&mut stream);
        write_json(
            &mut stream,
            &sized_hello_response(request.id, RESPONSE_BUDGET),
        );
        let request = read_handover(&mut stream);
        write_json(
            &mut stream,
            &JsonRpcResponse::result(request.id, HandoverResponse::accepted()),
        );
    })
    .unwrap();
}

#[test]
fn client_rejects_response_json_over_budget_before_newline() {
    let error = call_fake(|mut stream| {
        let request = read_hello(&mut stream);
        write_json(
            &mut stream,
            &sized_hello_response(request.id, RESPONSE_BUDGET + 1),
        );
        expect_connection_closed(&mut stream);
    })
    .expect_err("超过 1 MiB 的响应必须失败");

    assert!(matches!(error, HandoverError::ResponseTooLarge));
}

#[test]
fn client_total_deadline_bounds_trickling_response() {
    let (stop_tx, stop_rx) = mpsc::channel();
    let (record, handle) = spawn_fake(move |mut stream| {
        read_hello(&mut stream);
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(8) {
            if stop_rx.try_recv().is_ok() || stream.write_all(b" ").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    let started = Instant::now();
    let error = handover(&record, credential(TEST_CREDENTIAL), HandoverParams::Focus)
        .expect_err("持续 trickle 不得延长总等待");
    let elapsed = started.elapsed();
    let _ = stop_tx.send(());
    handle.join().unwrap();

    assert!(matches!(
        error,
        HandoverError::Io(ref error) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert!(elapsed >= Duration::from_secs(4), "总 deadline 提前触发");
    assert!(
        elapsed < Duration::from_secs(7),
        "总 deadline 被 trickle 重置"
    );
}

#[test]
fn client_invalid_endpoint_or_credential_does_not_connect() {
    let listener = TcpListener::bind("0.0.0.0:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let non_loopback = record(&listener.local_addr().unwrap().to_string());
    let error = handover(
        &non_loopback,
        credential(TEST_CREDENTIAL),
        HandoverParams::Focus,
    )
    .expect_err("非 loopback 记录必须在连接前失败");
    assert!(matches!(error, HandoverError::InvalidDiscovery));
    assert!(matches!(
        listener.accept(),
        Err(ref error) if error.kind() == io::ErrorKind::WouldBlock
    ));

    let zero_port = record("127.0.0.1:0");
    let error = handover(
        &zero_port,
        credential(TEST_CREDENTIAL),
        HandoverParams::Focus,
    )
    .expect_err("零端口记录必须在连接前失败");
    assert!(matches!(error, HandoverError::InvalidDiscovery));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let invalid_credential_record = record(&listener.local_addr().unwrap().to_string());
    let error = handover(
        &invalid_credential_record,
        SecretBytes::new(vec![0xff]),
        HandoverParams::Focus,
    )
    .expect_err("非 UTF-8 凭据必须在连接前失败");
    assert!(matches!(error, HandoverError::InvalidCredential));
    assert!(matches!(
        listener.accept(),
        Err(ref error) if error.kind() == io::ErrorKind::WouldBlock
    ));
}
