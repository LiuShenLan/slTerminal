//! Runtime 控制面传输主链:accept 配额、首行分流、信封校验、读写行。
//!
//! 本步只到信封层:方法专链(describe/subscribe/wait)归 M7.2,写方法派发
//! 归 M7.3;未知方法一律显式 `method_not_found`,不伪造成功。

use std::io::{BufRead, BufReader, Error as IoError, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use serde::Serialize;
use serde_json::Value;

use super::{
    ApiError, ApiRequest, ApiResponse, EventSink, IO_TIMEOUT, MAX_CLIENTS, MAX_REQUEST_BYTES,
    PROTOCOL_NAME, PROTOCOL_VERSION, RuntimeCallback, SUPPORTED_VERSIONS,
};

/// accept 循环:按 `MAX_CLIENTS` 计数,每连接一线程,配额随连接线程 Drop 归还。
#[allow(dead_code)] // 消费点 = M7.5 `RuntimeServer::spawn_at` 的 serve 线程。
pub(super) fn serve(listener: TcpListener, token: String, sink: EventSink) {
    let active = std::sync::Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Some(guard) = admit_client(&active) else {
            continue;
        };
        let token = token.clone();
        let sink = sink.clone();
        let _ = thread::Builder::new()
            .name("slterm-runtime-client".to_owned())
            .spawn(move || {
                let _guard = guard;
                if let Err(error) = handle_connection(stream, &token, &sink) {
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

    // 首行非 JSON → legacy 移交探测(ATTACH/PING);token 不符静默丢弃。
    if !line.trim_start().starts_with('{') {
        return handle_legacy(&mut stream, &line, token, sink);
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
        .details(serde_json::json!({ "supported_versions": SUPPORTED_VERSIONS }));
        return write_response(&mut stream, &ApiResponse::failure(request.id, error));
    }

    // 方法专链归 M7.2,写方法派发归 M7.3;本步未知方法显式报错。
    write_response(
        &mut stream,
        &ApiResponse::failure(
            request.id,
            ApiError::new(
                "method_not_found",
                format!("unknown runtime method {:?}", request.method),
            ),
        ),
    )
}

/// legacy 首行:ATTACH(唤醒/还原窗口,fire-and-forget)/ PING(token 不符静默)。
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
            sink(RuntimeCallback::Attach);
            stream.write_all(b"OK\n")
        }
        "PING" => stream.write_all(b"OK\n"),
        _ => Ok(()),
    }
}

fn write_response(stream: &mut TcpStream, response: &ApiResponse) -> Result<(), IoError> {
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

    /// 只 accept 一次的测试服务端:用真实回环连接跑 `handle_connection`。
    fn spawn_single_connection_server(token: &str, sink: EventSink) -> SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let token = token.to_owned();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept test client");
            let _ = handle_connection(stream, &token, &sink);
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

    #[test]
    fn valid_request_echoes_identity_in_method_not_found() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange_json(
            address,
            &serde_json::json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-7",
                "token": TEST_TOKEN,
                "method": "pane.read",
                "params": { "pane_id": 1 },
            }),
        );
        assert!(!response.ok);
        assert_eq!(response.protocol, PROTOCOL_NAME);
        assert_eq!(response.version, PROTOCOL_VERSION);
        assert_eq!(response.id, "4242-7");
        assert_eq!(response.error.expect("error").code, "method_not_found");
        assert!(response.result.is_none());
    }

    #[test]
    fn missing_params_field_still_reaches_method_dispatch() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange_json(
            address,
            &serde_json::json!({
                "protocol": PROTOCOL_NAME,
                "version": PROTOCOL_VERSION,
                "id": "4242-8",
                "token": TEST_TOKEN,
                "method": "runtime.describe",
            }),
        );
        assert_eq!(response.error.expect("error").code, "method_not_found");
    }

    #[test]
    fn protocol_version_mismatch_lists_supported_versions() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange_json(
            address,
            &serde_json::json!({
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
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange_json(
            address,
            &serde_json::json!({
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
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        // 未知字段本应触发信封解析错误;先认证则整条连接静默丢弃。
        let response = exchange(
            address,
            &serde_json::json!({
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
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange(
            address,
            &serde_json::json!({
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
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange_json(
            address,
            &serde_json::json!({
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
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response: ApiResponse =
            serde_json::from_str(&exchange(address, "{not json")).expect("error envelope");
        assert_eq!(response.id, "unknown");
        assert_eq!(response.error.expect("error").code, "invalid_request");
    }

    #[test]
    fn first_line_over_request_limit_is_rejected() {
        let (sink, _events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
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

    #[test]
    fn legacy_attach_notifies_sink_and_acknowledges() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange(address, &format!("ATTACH {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
        assert!(matches!(
            events
                .recv_timeout(Duration::from_secs(5))
                .expect("attach event"),
            RuntimeCallback::Attach
        ));
    }

    #[test]
    fn legacy_ping_acknowledges_without_attach_event() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange(address, &format!("PING {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
        // 事件(若有)先于 OK 写出,读到 OK 时队列已定型。
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn legacy_wrong_token_is_silently_dropped() {
        let (sink, events) = sink_channel();
        let address = spawn_single_connection_server(TEST_TOKEN, sink);
        let response = exchange(address, "ATTACH sk-test-wrong-token");
        assert!(response.is_empty());
        assert!(events.try_recv().is_err());
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

    #[test]
    fn serve_accepts_connections_and_answers_legacy_ping() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind serve listener");
        let address = listener.local_addr().expect("serve address");
        let (sink, _events) = sink_channel();
        // serve 无停机语义(随进程生命周期,M7.5);测试线程随测试进程退出回收。
        std::thread::spawn(move || serve(listener, TEST_TOKEN.to_owned(), sink));
        let response = exchange(address, &format!("PING {TEST_TOKEN}"));
        assert_eq!(response, "OK\n");
    }
}
