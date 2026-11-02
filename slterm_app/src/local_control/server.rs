//! loopback 控制面服务器与连接生命周期。

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use slterm_runtime_protocol::{
    ClientRequest, ControlScope, Credential, DomainErrorCode, HandoverParams, HandoverResponse,
    HelloResponse, JsonRpcError, JsonRpcErrorData, JsonRpcResponse, parse_client_request,
};

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

impl LocalControlServer {
    pub(crate) fn start<F>(
        credential: Credential,
        hello: HelloResponse,
        on_handover: F,
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
        let accept = {
            let shutdown = Arc::clone(&shutdown);
            let workers = Arc::clone(&workers);
            let active_workers = Arc::clone(&active_workers);
            std::thread::Builder::new()
                .name("slterm-control-accept".to_owned())
                .spawn(move || {
                    accept_loop(
                        listener,
                        credential,
                        hello,
                        callback,
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

fn accept_loop<F>(
    listener: TcpListener,
    credential: Credential,
    hello: HelloResponse,
    callback: Arc<F>,
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
            let callback = Arc::clone(&callback);
            let credential = credential.clone();
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
                    handle_connection(stream, credential, hello, callback, shutdown)
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
    credential: Credential,
    hello: HelloResponse,
    callback: Arc<F>,
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
    if request.params.scope != ControlScope::Handover
        || !credential_matches(&credential, &request.params.credential)
    {
        return;
    }
    if write_json(&mut stream, &JsonRpcResponse::result(request.id, hello)).is_err() {
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
        let Ok(ClientRequest::Handover(request)) = parse_client_request(&frame) else {
            return;
        };
        if shutdown.load(Ordering::SeqCst) {
            let error = JsonRpcError::new(
                -32004,
                "closing",
                Some(JsonRpcErrorData {
                    code: DomainErrorCode::Closing,
                }),
            );
            let _ = write_json(
                &mut stream,
                &JsonRpcResponse::<HandoverResponse>::error(request.id, error),
            );
            return;
        }
        let response = match callback(request.params) {
            Ok(result) if result.is_accepted() => JsonRpcResponse::result(request.id, result),
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
}

fn credential_matches(expected: &Credential, actual: &Credential) -> bool {
    let expected = expected.as_str().as_bytes();
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
