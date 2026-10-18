//! Runtime 端点发现与端口文件落盘的原语(04 篇 M7)。
//!
//! M7.4 落三级发现:`CHILD_ENDPOINT`(进程级)优先于显式
//! `SLTERM_RUNTIME_ENDPOINT`,后者优先于 `runtime.port` 端口文件;显式 env
//! 存在但非法时不回落端口文件(语义红线:不允许「指了 A 却悄悄连 B」)。
//! `RuntimeServer` 启动全序与移交链归 M7.5。
//!
//! 端口文件内容 `"<port> <128bit-token> <protocol_version>\n"`,token 属
//! 能力边界:类型层隔离在 `Endpoint`,日志一律不插值。

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::PROTOCOL_VERSION;

/// 端口文件名(01 篇 D 节:文件名不变,目录 = 设置数据目录)。
pub(super) const PORT_FILE_NAME: &str = "runtime.port";

/// 显式端点环境变量(01 篇 C 节);值形态 `"<port> <token>"`。
pub(crate) const ENDPOINT_ENV: &str = "SLTERM_RUNTIME_ENDPOINT";

/// 本进程作为宿主时的端点;serve 线程 spawn 成功后登记(M7.5)。
/// 子进程经 [`apply_child_endpoint`] 继承,宿主本身经 [`read_endpoint`] 消费。
static CHILD_ENDPOINT: Mutex<Option<Endpoint>> = Mutex::new(None);

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

/// 端口文件所在路径(settings data_dir 单点,01 篇 D 节)。
pub(super) fn port_file() -> PathBuf {
    crate::platform::dirs::data_dir().join(PORT_FILE_NAME)
}

/// 登记/清空本进程作为宿主的端点(M7.5 serve 成功后调用;测试可注入假端点)。
#[allow(dead_code)] // 生产消费点 = M7.5 `RuntimeServer::spawn_at` 尾序。
pub(crate) fn register_child_endpoint(endpoint: Option<Endpoint>) {
    *CHILD_ENDPOINT
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = endpoint;
}

/// 清空端点(serve 失败的清理路径与测试隔离共用)。
#[allow(dead_code)] // 生产消费点 = M7.5 启动失败清理。
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
#[allow(dead_code)] // 生产消费点 = M7.5 `RuntimeServer::spawn_at`。
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
#[allow(dead_code)] // 生产消费点 = M7.5 `RuntimeServer::spawn_at`。
pub(super) fn write_port_file(path: &Path, endpoint: &Endpoint) -> io::Result<()> {
    let contents = format!(
        "{} {} {}\n",
        endpoint.port, endpoint.token, PROTOCOL_VERSION
    );
    crate::atomic_file::write(path, contents.as_bytes())
}

/// 进程级 owner 锁:OS 句柄独占,进程死即释放,健康进程永不被超时回收。
#[allow(dead_code)] // 生产消费点 = M7.5 `RuntimeServer::spawn_callback`。
pub(super) fn try_owner_lock(
    path: &Path,
) -> io::Result<Option<crate::atomic_file::LifetimeFileLock>> {
    crate::atomic_file::try_lifetime_lock(path)
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
}
