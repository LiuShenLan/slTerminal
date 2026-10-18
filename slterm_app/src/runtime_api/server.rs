//! Runtime 端点发现、单实例服务端与二次启动移交。
//!
//! 三级发现:`CHILD_ENDPOINT`(进程级)优先于显式
//! `SLTERM_RUNTIME_ENDPOINT`,后者优先于 `runtime.port` 端口文件;显式 env
//! 存在但非法时不回落端口文件(语义红线:不允许「指了 A 却悄悄连 B」)。
//!
//! 单实例启动全序:LifetimeFileLock(拿不到 = 有主)→ 存活探测既有端点
//! (通 = 有活实例)→ bind 127.0.0.1:0 → 原子写端口文件 → serve 线程 →
//! 登记 `CHILD_ENDPOINT`。Drop 只删自己仍是最新的记录。二次启动不再建窗:
//! 首发形态经 `window.focus` + `tab.new` 把「聚焦现有唯一窗 + 新开 tab」
//! 交给首实例后退出(D12-5 回落面关闭);开启 `legacy-attach` feature 时
//! 改回 ATTACH + `tab.new`。陈旧端口文件由 400ms connect 超时兜底接管,
//! 移交降级从不挂起。
//!
//! 端口文件内容 `"<port> <128bit-token> <protocol_version>\n"`,token 属
//! 能力边界:类型层隔离在 `Endpoint`,日志一律不插值。

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::transport;
use super::{ApiRequest, ApiResponse, EventSink, PROTOCOL_VERSION, RuntimeHub};

/// 端口文件名(文件名不变,目录 = 设置数据目录)。
pub(super) const PORT_FILE_NAME: &str = "runtime.port";

/// 显式端点环境变量;值形态 `"<port> <token>"`。
pub(crate) const ENDPOINT_ENV: &str = "SLTERM_RUNTIME_ENDPOINT";

/// 本进程作为宿主时的端点;serve 线程 spawn 成功后登记。
/// 子进程经 [`apply_child_endpoint`] 继承,宿主本身经 [`read_endpoint`] 消费。
static CHILD_ENDPOINT: Mutex<Option<Endpoint>> = Mutex::new(None);

/// 移交链 connect 超时(原 mux.rs CONNECT_TIMEOUT;陈旧端口文件靠它兜底)。
const HANDOVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(400);
/// 移交链读写超时(原 mux.rs IO_TIMEOUT)。
const HANDOVER_IO_TIMEOUT: Duration = Duration::from_millis(700);
/// 双启动抢锁重试环:D04-2 契约数值 40 × 25ms。
const HANDOVER_ATTEMPTS: u32 = 40;
const HANDOVER_RETRY_DELAY: Duration = Duration::from_millis(25);
/// Drop 自连唤醒阻塞 accept 的 connect 上限;只是解阻塞手段,失败即忽略。
const STOP_WAKE_CONNECT_TIMEOUT: Duration = Duration::from_millis(400);

/// 端点发现结果:port + 128bit token。
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Endpoint {
    pub(super) port: u16,
    pub(super) token: String,
}

// 手写 Debug:token 是能力边界,禁止随 `{:?}` 落日志。
impl fmt::Debug for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Endpoint")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// 端口文件所在路径(settings data_dir 单点)。
pub(super) fn port_file() -> PathBuf {
    crate::platform::dirs::data_dir().join(PORT_FILE_NAME)
}

/// 登记/清空本进程作为宿主的端点(serve 成功后调用;测试可注入假端点)。
pub(crate) fn register_child_endpoint(endpoint: Option<Endpoint>) {
    *CHILD_ENDPOINT
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = endpoint;
}

/// 清空端点(测试隔离与异常清理共用)。
#[allow(dead_code)] // 生产 Drop 自行比较清理;此函数是显式清理面。
pub(crate) fn clear_child_endpoint() {
    register_child_endpoint(None);
}

fn child_endpoint() -> Option<Endpoint> {
    CHILD_ENDPOINT
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

/// 注入前按名清旧值(Windows 大小写不敏感),有 `CHILD_ENDPOINT` 才写
/// `"<port> <token>"`;无值则不注入(不留死值)。
pub(crate) fn apply_child_endpoint(env: &mut HashMap<String, String>) {
    apply_child_endpoint_value(env, child_endpoint().as_ref());
}

fn apply_child_endpoint_value(env: &mut HashMap<String, String>, endpoint: Option<&Endpoint>) {
    env.retain(|key, _| !key.eq_ignore_ascii_case(ENDPOINT_ENV));
    if let Some(endpoint) = endpoint {
        env.insert(
            ENDPOINT_ENV.to_owned(),
            format!("{} {}", endpoint.port, endpoint.token),
        );
    }
}

/// 三级发现:本进程 `CHILD_ENDPOINT` > 显式 env > 端口文件。
///
/// 显式 env 存在但非法 → `None`,绝不回落端口文件(防选错实例)。
pub(super) fn read_endpoint() -> Option<Endpoint> {
    let child = child_endpoint();
    let explicit = std::env::var_os(ENDPOINT_ENV);
    read_endpoint_with(child, explicit.as_deref(), &port_file())
}

fn read_endpoint_with(
    child: Option<Endpoint>,
    explicit: Option<&OsStr>,
    port_path: &Path,
) -> Option<Endpoint> {
    if let Some(endpoint) = child {
        return Some(endpoint);
    }
    if let Some(value) = explicit {
        // 非法显式端点 = 明确不可达,不是「回去猜另一个实例」。
        return value.to_str().and_then(parse_endpoint);
    }
    read_endpoint_from(port_path)
}

fn read_endpoint_from(path: &Path) -> Option<Endpoint> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|data| parse_endpoint(&data))
}

/// 端点 → loopback 地址:控制面只监听本机。
pub(super) fn endpoint_addr(endpoint: &Endpoint) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port))
}

/// 解析 `"<port> <token> [<protocol_version>]"`:port 非 0,token 恰 32 位 hex。
pub(super) fn parse_endpoint(data: &str) -> Option<Endpoint> {
    let mut parts = data.split_whitespace();
    let port = parts.next()?.parse().ok()?;
    let token = parts.next()?;
    if port == 0 || token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(Endpoint {
        port,
        token: token.to_owned(),
    })
}

/// 两个 OS 种子 `RandomState` 哈希拼 32 hex;只挡其他本机用户,不承诺抵御同用户攻击者。
pub(super) fn fresh_token() -> String {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut first = RandomState::new().build_hasher();
    let mut second = RandomState::new().build_hasher();
    first.write_u32(std::process::id());
    second.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0),
    );
    format!("{:016x}{:016x}", first.finish(), second.finish())
}

/// 原子写端口文件(临时文件 + rename;读者永不见撕裂内容)。
pub(super) fn write_port_file(path: &Path, endpoint: &Endpoint) -> io::Result<()> {
    let contents = format!(
        "{} {} {}\n",
        endpoint.port, endpoint.token, PROTOCOL_VERSION
    );
    crate::atomic_file::write(path, contents.as_bytes())
}

/// 进程级 owner 锁:OS 句柄独占,进程死即释放,健康进程永不被超时回收。
pub(super) fn try_owner_lock(
    path: &Path,
) -> io::Result<Option<crate::atomic_file::LifetimeFileLock>> {
    crate::atomic_file::try_lifetime_lock(path)
}

/// 普通二次启动:把「聚焦现有唯一窗 + 新建默认 shell 标签页」交给首实例。
pub(crate) fn try_open_default_tab_existing(shell: Option<&str>) -> bool {
    try_open_tab_existing_at(None, None, shell)
}

/// Explorer 右键/带 `--working-directory` 的二次启动:移交 cwd 到新标签页。
pub(crate) fn try_open_directory_existing(dir: &Path, shell: Option<&str>) -> bool {
    try_open_tab_existing_at(None, Some(dir), shell)
}

/// 移交请求参数:只放**有值**的键。第二实例与首实例可能不是同一个构建,
/// 缺省键让对方按 `serde(default)` 走原行为;空白 shell 当作没给。
fn handover_params(dir: Option<&Path>, shell: Option<&str>) -> serde_json::Value {
    let mut params = serde_json::Map::new();
    if let Some(dir) = dir {
        params.insert("cwd".to_owned(), serde_json::json!(dir));
    }
    if let Some(shell) = shell.map(str::trim).filter(|shell| !shell.is_empty()) {
        params.insert("shell".to_owned(), serde_json::json!(shell));
    }
    serde_json::Value::Object(params)
}

/// ATTACH 与 tab.new 落同一事件队列:窗口先聚焦,新标签随后创建。
/// `port_path = None` 走三级发现;测试传显式端口文件隔离真实数据目录。
fn try_open_tab_existing_at(
    port_path: Option<&Path>,
    dir: Option<&Path>,
    shell: Option<&str>,
) -> bool {
    #[cfg(feature = "legacy-attach")]
    if legacy_request_at("ATTACH", port_path).is_none() {
        return false;
    }
    #[cfg(not(feature = "legacy-attach"))]
    if !focus_existing_window_at(port_path) {
        return false;
    }
    let Some(endpoint) = endpoint_for_path(port_path) else {
        return false;
    };
    request_to(&endpoint, "tab.new", handover_params(dir, shell))
        .is_some_and(|response| response.ok)
}

/// D12-5 首发形态的交接唤醒:版本化 `window.focus` 语义等价于旧 ATTACH
/// (最小化先还原再前置);UI owner 回执后才继续 `tab.new`。
#[cfg(not(feature = "legacy-attach"))]
fn focus_existing_window_at(port_path: Option<&Path>) -> bool {
    let Some(endpoint) = endpoint_for_path(port_path) else {
        return false;
    };
    request_to(&endpoint, "window.focus", serde_json::json!({})).is_some_and(|response| response.ok)
}

fn endpoint_for_path(port_path: Option<&Path>) -> Option<Endpoint> {
    match port_path {
        Some(path) => read_endpoint_from(path),
        None => read_endpoint(),
    }
}

/// 存活探测:开启 `legacy-attach` 时用 PING,首发形态用版本化
/// `runtime.describe`;失败一律视为无活实例。
fn endpoint_is_alive(endpoint: &Endpoint) -> bool {
    #[cfg(feature = "legacy-attach")]
    {
        legacy_request_to("PING", endpoint).is_some()
    }
    #[cfg(not(feature = "legacy-attach"))]
    {
        request_to(endpoint, "runtime.describe", serde_json::json!({}))
            .is_some_and(|response| response.ok)
    }
}

#[cfg(feature = "legacy-attach")]
fn legacy_request_at(verb: &str, port_path: Option<&Path>) -> Option<()> {
    let endpoint = endpoint_for_path(port_path)?;
    legacy_request_to(verb, &endpoint)
}

/// legacy 移交探测:connect/读写在 HANDOVER_* 超时内完成,失败即降级。
#[cfg(feature = "legacy-attach")]
fn legacy_request_to(verb: &str, endpoint: &Endpoint) -> Option<()> {
    let mut stream =
        TcpStream::connect_timeout(&endpoint_addr(endpoint), HANDOVER_CONNECT_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(HANDOVER_IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(HANDOVER_IO_TIMEOUT)).ok()?;
    stream
        .write_all(format!("{verb} {}\n", endpoint.token).as_bytes())
        .ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    (line.trim() == "OK").then_some(())
}

/// 版本化移交请求(与 CLI 同信封);身份校验只认 `ok` 与可解析响应。
fn request_to(endpoint: &Endpoint, method: &str, params: serde_json::Value) -> Option<ApiResponse> {
    let request = ApiRequest::new(endpoint.token.clone(), method, params);
    let mut stream =
        TcpStream::connect_timeout(&endpoint_addr(endpoint), HANDOVER_CONNECT_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(HANDOVER_IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(HANDOVER_IO_TIMEOUT)).ok()?;
    serde_json::to_writer(&mut stream, &request).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.flush().ok()?;
    stream.shutdown(std::net::Shutdown::Write).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// 移交重试环(40 × 25ms):对方可能已拿 owner 锁但端点尚未落盘,
/// 短暂重试后再决定是否退回 client-only 自启。
pub(crate) fn handover_to_existing_instance(
    handover_cwd: Option<&Path>,
    shell: Option<&str>,
) -> bool {
    handover_loop(
        HANDOVER_ATTEMPTS,
        HANDOVER_RETRY_DELAY,
        || match handover_cwd {
            Some(dir) => try_open_directory_existing(dir, shell),
            None => try_open_default_tab_existing(shell),
        },
    )
}

fn handover_loop(attempts: u32, delay: Duration, mut attempt: impl FnMut() -> bool) -> bool {
    for index in 0..attempts {
        if attempt() {
            return true;
        }
        if index + 1 < attempts {
            std::thread::sleep(delay);
        }
    }
    false
}

/// 单实例 Runtime 控制面服务端(随壳生命周期持有,Drop 停 serve 线程并删端口文件)。
pub struct RuntimeServer {
    endpoint: Endpoint,
    /// `None` = 私有实例(env-only 发现;测试/嵌套用)。
    port_file: Option<PathBuf>,
    /// serve 线程停止位;置位后 accept 循环退出。
    stop: Arc<AtomicBool>,
    /// serve 线程句柄(Drop 唤醒并 join;失败路径不产出句柄)。
    join: Option<JoinHandle<()>>,
    /// OS 句柄锁:进程死即释放,健康进程永不被超时回收。
    _owner_lock: Option<crate::atomic_file::LifetimeFileLock>,
}

impl RuntimeServer {
    /// 生产入口:默认端口文件 + 三级发现。
    pub(crate) fn spawn_callback(
        on_event: impl Fn(super::RuntimeCallback) + Send + Sync + 'static,
        hub: RuntimeHub,
    ) -> Option<Self> {
        Self::spawn_at(std::sync::Arc::new(on_event), hub, Some(port_file()))
    }

    /// 启动全序:owner 锁(拿不到 = 有主)→ PING 旧实例 → bind → 原子写
    /// 端口文件 → serve 线程 → 登记 `CHILD_ENDPOINT`。
    pub(super) fn spawn_at(
        sink: EventSink,
        hub: RuntimeHub,
        path: Option<PathBuf>,
    ) -> Option<Self> {
        let owner_lock = match path.as_ref().map(|path| try_owner_lock(path)) {
            Some(Ok(Some(lock))) => Some(lock),
            None => None,
            Some(Ok(None)) => {
                log::info!("Runtime API owner is starting or already running; staying client-only");
                return None;
            }
            Some(Err(error)) => {
                log::warn!("Runtime API: cannot lock owner file: {error}; control plane disabled");
                return None;
            }
        };
        // 锁在手里仍要 PING:上一个进程可能刚死、锁文件残留;此时端口
        // 记录陈旧,connect 400ms 超时后由本进程接管。
        if path
            .as_ref()
            .and_then(|path| read_endpoint_from(path))
            .is_some_and(|endpoint| endpoint_is_alive(&endpoint))
        {
            log::info!("Runtime API server already running; this instance stays client-only");
            return None;
        }

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
        let endpoint = Endpoint {
            port: listener.local_addr().ok()?.port(),
            token: fresh_token(),
        };
        if path
            .as_ref()
            .is_some_and(|path| write_port_file(path, &endpoint).is_err())
        {
            log::warn!("Runtime API: cannot write the port file; control plane disabled");
            return None;
        }

        let server_token = endpoint.token.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let serve_stop = stop.clone();
        let join = std::thread::Builder::new()
            .name("slterm-runtime-api".to_owned())
            .spawn(move || transport::serve(listener, serve_stop, server_token, sink, hub));
        let Ok(join) = join else {
            // 线程未启动 = 监听 socket 随闭包释放;端口文件记账回滚。
            if let Some(path) = path.as_ref() {
                let _ = std::fs::remove_file(path);
            }
            return None;
        };
        register_child_endpoint(Some(endpoint.clone()));
        log::info!("Runtime API listening on 127.0.0.1:{}", endpoint.port);
        Some(Self {
            endpoint,
            port_file: path,
            stop,
            join: Some(join),
            _owner_lock: owner_lock,
        })
    }
}

impl Drop for RuntimeServer {
    fn drop(&mut self) {
        // 顺序:置停止位 → 自连唤醒阻塞的 accept → join serve 线程。
        // 客户端处理线程各自有读写超时,不随 join 等待。
        self.stop.store(true, Ordering::Release);
        let _ =
            TcpStream::connect_timeout(&endpoint_addr(&self.endpoint), STOP_WAKE_CONNECT_TIMEOUT);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }

        // 线程已停:不删别的进程更新的发现记录,只有文件仍是自己的端点才回收。
        if let Some(path) = self.port_file.as_ref()
            && read_endpoint_from(path).as_ref() == Some(&self.endpoint)
        {
            let _ = std::fs::remove_file(path);
        }
        let mut child = CHILD_ENDPOINT
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if child.as_ref() == Some(&self.endpoint) {
            *child = None;
        }
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;

    // SEC-18:测试一律假值占位,真实凭据禁入仓库。
    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn parse_endpoint_accepts_port_token_and_trailing_version() {
        let endpoint =
            parse_endpoint("43210 0123456789abcdef0123456789abcdef 1\n").expect("valid endpoint");
        assert_eq!(endpoint.port, 43210);
        assert_eq!(endpoint.token, TEST_TOKEN);
    }

    #[test]
    fn parse_endpoint_rejects_zero_port_and_malformed_tokens() {
        assert!(parse_endpoint("0 0123456789abcdef0123456789abcdef").is_none());
        assert!(parse_endpoint("65536 0123456789abcdef0123456789abcdef").is_none());
        assert!(parse_endpoint("41000 0123456789abcdef0123456789abcde").is_none());
        assert!(parse_endpoint("41000 z123456789abcdef0123456789abcdef").is_none());
        assert!(parse_endpoint("41000").is_none());
        assert!(parse_endpoint("").is_none());
    }

    #[test]
    fn fresh_token_is_128_bit_hex_and_unique_per_call() {
        let first = fresh_token();
        let second = fresh_token();
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn port_file_write_is_atomic_and_round_trips_through_parser() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let endpoint = Endpoint {
            port: 43210,
            token: TEST_TOKEN.to_owned(),
        };
        write_port_file(&path, &endpoint).expect("write port file");

        let contents = std::fs::read_to_string(&path).expect("read port file");
        assert_eq!(contents, format!("43210 {TEST_TOKEN} {PROTOCOL_VERSION}\n"));
        assert_eq!(parse_endpoint(&contents), Some(endpoint));
        let leftovers = std::fs::read_dir(directory.path())
            .expect("list temp dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("slterm-tmp"))
            .count();
        assert_eq!(leftovers, 0, "atomic write must not leave temp files");
    }

    #[test]
    fn owner_lock_is_exclusive_until_drop() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let owner = try_owner_lock(&path)
            .expect("lock call")
            .expect("first owner");
        assert!(try_owner_lock(&path).expect("second lock call").is_none());
        drop(owner);
        assert!(try_owner_lock(&path).expect("reacquire").is_some());
    }

    #[test]
    fn port_file_lives_in_the_data_directory() {
        let path = port_file();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(PORT_FILE_NAME)
        );
    }

    #[test]
    fn endpoint_debug_does_not_leak_the_token() {
        let endpoint = Endpoint {
            port: 41000,
            token: TEST_TOKEN.to_owned(),
        };
        let rendered = format!("{endpoint:?}");
        assert!(rendered.contains("port: 41000"));
        assert!(!rendered.contains(TEST_TOKEN));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn discovery_prefers_child_endpoint_over_everything_else() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        write_port_file(
            &path,
            &Endpoint {
                port: 41000,
                token: TEST_TOKEN.to_owned(),
            },
        )
        .expect("write port file");
        let child = Endpoint {
            port: 42000,
            token: TEST_TOKEN.to_owned(),
        };
        let explicit = OsStr::new("43000 0123456789abcdef0123456789abcdef");

        let chosen = read_endpoint_with(Some(child.clone()), Some(explicit), &path);
        assert_eq!(chosen, Some(child));
    }

    #[test]
    fn explicit_endpoint_beats_the_port_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        write_port_file(
            &path,
            &Endpoint {
                port: 41000,
                token: TEST_TOKEN.to_owned(),
            },
        )
        .expect("write port file");

        let chosen = read_endpoint_with(
            None,
            Some(OsStr::new("43000 0123456789abcdef0123456789abcdef")),
            &path,
        );
        assert_eq!(
            chosen,
            Some(Endpoint {
                port: 43000,
                token: TEST_TOKEN.to_owned(),
            })
        );
    }

    #[test]
    fn invalid_explicit_endpoint_never_falls_back_to_the_port_file() {
        // 语义红线:指了 A 却悄悄连 B 会让客户端把请求发进错误实例。
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        write_port_file(
            &path,
            &Endpoint {
                port: 41000,
                token: TEST_TOKEN.to_owned(),
            },
        )
        .expect("write port file");

        for invalid in [
            "",
            "0 0123456789abcdef0123456789abcdef",
            "41000 zzz",
            "41000",
        ] {
            assert_eq!(
                read_endpoint_with(None, Some(OsStr::new(invalid)), &path),
                None,
                "{invalid:?} must not fall back to the port file"
            );
        }
    }

    #[test]
    fn port_file_is_the_last_discovery_fallback() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let endpoint = Endpoint {
            port: 41000,
            token: TEST_TOKEN.to_owned(),
        };
        write_port_file(&path, &endpoint).expect("write port file");
        assert_eq!(read_endpoint_with(None, None, &path), Some(endpoint));
    }

    #[test]
    fn child_endpoint_injection_replaces_inherited_values_and_is_idempotent() {
        let endpoint = Endpoint {
            port: 41000,
            token: TEST_TOKEN.to_owned(),
        };
        let mut env = HashMap::from([
            // 大小写不敏感的旧值同样必须清掉;Windows 环境块不区分大小写。
            (
                "slterm_runtime_endpoint".to_owned(),
                "stale endpoint".to_owned(),
            ),
            ("PATH".to_owned(), "C:/Windows".to_owned()),
        ]);
        apply_child_endpoint_value(&mut env, Some(&endpoint));
        assert_eq!(
            env.get(ENDPOINT_ENV).map(String::as_str),
            Some("41000 0123456789abcdef0123456789abcdef")
        );
        assert_eq!(
            env.keys()
                .filter(|key| key.eq_ignore_ascii_case(ENDPOINT_ENV))
                .count(),
            1,
            "旧值必须被清除,不得留下两份"
        );
        let keys = env.len();
        apply_child_endpoint_value(&mut env, Some(&endpoint));
        assert_eq!(env.len(), keys, "重复注入不增长环境块");
        assert_eq!(env.get("PATH").map(String::as_str), Some("C:/Windows"));
    }

    #[test]
    fn no_child_endpoint_injects_nothing_but_still_clears_stale_values() {
        let mut env = HashMap::from([(ENDPOINT_ENV.to_owned(), "stale endpoint".to_owned())]);
        apply_child_endpoint_value(&mut env, None);
        assert!(!env.contains_key(ENDPOINT_ENV), "无宿主端点时不留死值");
    }

    fn capturing_sink() -> (
        EventSink,
        std::sync::mpsc::Receiver<super::super::RuntimeCallback>,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        (
            std::sync::Arc::new(move |callback| {
                let _ = sender.send(callback);
            }),
            receiver,
        )
    }

    /// 端口文件写入、存活探测与 Drop 清理构成一条闭环;私有实例不落盘。
    #[test]
    fn server_writes_endpoint_answers_liveness_probe_and_drop_removes_only_its_own_record() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("server");
        let endpoint = read_endpoint_from(&path).expect("port file");
        assert_eq!(
            read_endpoint().as_ref(),
            Some(&endpoint),
            "CHILD_ENDPOINT 已登记"
        );
        assert!(endpoint_is_alive(&endpoint));

        drop(server);
        assert!(!path.exists(), "Drop 删除自己仍是最新的记录");
        assert!(read_endpoint().is_none(), "Drop 清空 CHILD_ENDPOINT");
    }

    /// B14 防复发:Drop 必须停 serve 线程、释放监听 socket,不留阻塞 accept。
    #[test]
    fn drop_stops_the_serve_thread_and_releases_the_listening_port() {
        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), None).expect("private server");
        let address = endpoint_addr(&server.endpoint);
        drop(server);

        // join 已在 Drop 内完成:连接最终必须被拒(监听 socket 已释放)。
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            match TcpStream::connect_timeout(&address, Duration::from_millis(50)) {
                Err(_) => break,
                Ok(stream) => {
                    drop(stream);
                    assert!(
                        std::time::Instant::now() < deadline,
                        "Drop 后 1 秒内 serve 线程必须退出并释放监听端口"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    /// B14 防复发:挂起不说话的客户端只占客户端线程,Drop 不得等它。
    #[test]
    fn drop_is_not_dragged_by_an_open_client_connection() {
        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), None).expect("private server");
        // 连接后不发送任何字节:客户端处理线程阻塞在首行读取(有读超时)。
        let client = TcpStream::connect(endpoint_addr(&server.endpoint)).expect("client connect");

        let started = std::time::Instant::now();
        drop(server);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "Drop 只 join serve 线程,不等客户端线程;实际 {:?}",
            started.elapsed()
        );
        drop(client);
    }

    #[test]
    fn private_server_keeps_discovery_off_disk_and_supplies_its_children() {
        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), None).expect("private server");
        assert!(server.port_file.is_none());
        assert!(server._owner_lock.is_none());
        assert_eq!(read_endpoint().as_ref(), Some(&server.endpoint));
        assert!(endpoint_is_alive(&server.endpoint));

        let mut env = HashMap::from([(ENDPOINT_ENV.to_owned(), "old endpoint".to_owned())]);
        apply_child_endpoint(&mut env);
        assert_eq!(
            parse_endpoint(&env[ENDPOINT_ENV]).as_ref(),
            Some(&server.endpoint)
        );

        let mut wrong = server.endpoint.clone();
        wrong.token = "00000000000000000000000000000000".into();
        assert!(!endpoint_is_alive(&wrong));

        drop(server);
        apply_child_endpoint(&mut env);
        assert!(!env.contains_key(ENDPOINT_ENV));
    }

    /// 第一实例活着时 owner 锁不可得:第二实例只做客户端,绝不双 serve。
    #[test]
    fn second_instance_stays_client_only_while_the_first_holds_the_lock() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let (sink, _events) = capturing_sink();
        let first =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("first");
        let (second_sink, _second_events) = capturing_sink();
        assert!(
            RuntimeServer::spawn_at(second_sink, RuntimeHub::new(), Some(path.clone())).is_none(),
            "owner 锁已被第一实例持有"
        );
        assert_eq!(
            read_endpoint_from(&path).as_ref(),
            Some(&first.endpoint),
            "第二实例不得改写端口文件"
        );
    }

    /// 被杀进程的陈旧端口文件:connect 超时兜底后由本进程接管。
    #[test]
    fn stale_port_file_is_taken_over_after_connect_timeout() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        // 绑定后立即释放:端口在短时间内无人监听(connection refused)。
        let dead_port = {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
            listener.local_addr().expect("addr").port()
        };
        let stale = Endpoint {
            port: dead_port,
            token: TEST_TOKEN.to_owned(),
        };
        write_port_file(&path, &stale).expect("stale record");

        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("takeover");
        let current = read_endpoint_from(&path).expect("rewritten record");
        assert_eq!(current, server.endpoint);
        assert_ne!(server.endpoint, stale, "陈旧记录被新端点替换");
        assert!(endpoint_is_alive(&current));
    }

    /// Drop 只删自己的记录;别进程写入的新记录保留。
    #[test]
    fn drop_keeps_a_newer_record_written_by_another_process() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let (sink, _events) = capturing_sink();
        let server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("server");
        let replacement = Endpoint {
            port: server.endpoint.port.wrapping_add(1).max(1),
            token: "f".repeat(32),
        };
        write_port_file(&path, &replacement).expect("newer record");
        drop(server);
        assert_eq!(read_endpoint_from(&path), Some(replacement));
    }

    /// 移交链(legacy-attach 开启):ATTACH 先到(聚焦窗口),
    /// tab.new 带 cwd/shell 随后到。
    #[cfg(feature = "legacy-attach")]
    #[test]
    fn handover_sends_attach_then_a_versioned_tab_new() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let workdir = directory.path().join("work");
        std::fs::create_dir_all(&workdir).expect("workdir");
        let (sink, events) = capturing_sink();
        let _server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("server");

        let responder = {
            std::thread::spawn(move || {
                let mut saw_attach = false;
                let mut handover = None;
                while let Ok(callback) = events.recv() {
                    match callback {
                        super::super::RuntimeCallback::Attach => saw_attach = true,
                        super::super::RuntimeCallback::Control(dispatch) => {
                            let super::super::RuntimeCommand::NewTab { cwd, shell_id, .. } =
                                &dispatch.command
                            else {
                                panic!("handover must dispatch tab.new");
                            };
                            handover = Some((cwd.clone(), shell_id.clone()));
                            dispatch.respond(Ok(serde_json::json!({
                                "action": { "window_id": 1, "pane_id": 9 }
                            })));
                        }
                    }
                    if saw_attach && handover.is_some() {
                        break;
                    }
                }
                (saw_attach, handover)
            })
        };

        assert!(try_open_tab_existing_at(
            Some(&path),
            Some(&workdir),
            Some("  powershell  ")
        ));
        let (saw_attach, handover) = responder.join().expect("responder");
        assert!(saw_attach, "ATTACH 必须先于 tab.new");
        let (cwd, shell_id) = handover.expect("tab.new");
        assert_eq!(cwd.as_deref(), Some(workdir.as_path()));
        assert_eq!(
            shell_id.as_deref(),
            Some("powershell"),
            "shell id 去空白后透传"
        );
    }

    /// 移交链(D12-5 首发形态):window.focus 先到,tab.new 随后到;
    /// 全程不出现 legacy Attach 事件。
    #[cfg(not(feature = "legacy-attach"))]
    #[test]
    fn handover_focuses_then_opens_a_versioned_tab() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(PORT_FILE_NAME);
        let workdir = directory.path().join("work");
        std::fs::create_dir_all(&workdir).expect("workdir");
        let (sink, events) = capturing_sink();
        let _server =
            RuntimeServer::spawn_at(sink, RuntimeHub::new(), Some(path.clone())).expect("server");

        let responder = {
            std::thread::spawn(move || {
                let mut saw_focus = false;
                let mut handover = None;
                while let Ok(callback) = events.recv() {
                    match callback {
                        super::super::RuntimeCallback::Attach => {
                            panic!("legacy ATTACH 在首发形态必须关闭")
                        }
                        super::super::RuntimeCallback::Control(dispatch) => {
                            match &dispatch.command {
                                super::super::RuntimeCommand::Focus { window_id, pane_id } => {
                                    assert!(window_id.is_none() && pane_id.is_none());
                                    saw_focus = true;
                                    dispatch.respond(Ok(serde_json::json!({
                                        "action": { "window_id": 1 }
                                    })));
                                }
                                super::super::RuntimeCommand::NewTab { cwd, shell_id, .. } => {
                                    handover = Some((cwd.clone(), shell_id.clone()));
                                    dispatch.respond(Ok(serde_json::json!({
                                        "action": { "window_id": 1, "pane_id": 9 }
                                    })));
                                }
                                other => panic!("handover 只允许 focus/tab.new: {other:?}"),
                            }
                        }
                    }
                    if saw_focus && handover.is_some() {
                        break;
                    }
                }
                (saw_focus, handover)
            })
        };

        assert!(try_open_tab_existing_at(
            Some(&path),
            Some(&workdir),
            Some("  powershell  ")
        ));
        let (saw_focus, handover) = responder.join().expect("responder");
        assert!(saw_focus, "window.focus 必须先于 tab.new");
        let (cwd, shell_id) = handover.expect("tab.new");
        assert_eq!(cwd.as_deref(), Some(workdir.as_path()));
        assert_eq!(
            shell_id.as_deref(),
            Some("powershell"),
            "shell id 去空白后透传"
        );
    }

    #[test]
    fn handover_params_carry_only_present_values() {
        assert_eq!(
            handover_params(None, None),
            serde_json::json!({}),
            "空参数不伪造键"
        );
        assert_eq!(
            handover_params(None, Some("   ")),
            serde_json::json!({}),
            "空白 shell 当作没给"
        );
        assert_eq!(
            handover_params(Some(Path::new("D:/work")), Some("pwsh")),
            serde_json::json!({ "cwd": "D:/work", "shell": "pwsh" })
        );
    }

    #[test]
    fn handover_retry_loop_stops_on_success_and_is_bounded() {
        let mut calls = 0;
        assert!(handover_loop(40, Duration::from_millis(0), || {
            calls += 1;
            calls == 3
        }));
        assert_eq!(calls, 3, "成功即退出,不烧满重试环");

        let mut calls = 0;
        assert!(!handover_loop(4, Duration::from_millis(0), || {
            calls += 1;
            false
        }));
        assert_eq!(calls, 4, "全失败时重试次数有界");
    }

    /// D04-2 契约数值:40 × 25ms 重试环与 400/700ms 移交 I/O。
    #[test]
    fn handover_contract_values_match_d04_2() {
        assert_eq!(HANDOVER_ATTEMPTS, 40);
        assert_eq!(HANDOVER_RETRY_DELAY, Duration::from_millis(25));
        assert_eq!(HANDOVER_CONNECT_TIMEOUT, Duration::from_millis(400));
        assert_eq!(HANDOVER_IO_TIMEOUT, Duration::from_millis(700));
    }
}
