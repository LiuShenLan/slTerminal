//! Runtime 端点发现与端口文件落盘的原语(04 篇 M7)。
//!
//! 本步只落 `atomic_file` 消费件:owner 锁(OS 句柄独占)与 `runtime.port`
//! 原子写;`RuntimeServer` 启动全序、三级发现与移交链归 M7.4/M7.5。
//!
//! 端口文件内容 `"<port> <128bit-token> <protocol_version>\n"`,token 属
//! 能力边界:类型层隔离在 `Endpoint`,日志一律不插值。

// 消费点 = M7.4 三级发现(Endpoint/parse_endpoint)/M7.5 RuntimeServer
// (owner 锁/原子写/端口路径);本步生产侧暂无调用方。
#![allow(dead_code)]

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::PROTOCOL_VERSION;

/// 端口文件名(01 篇 D 节:文件名不变,目录 = 设置数据目录)。
pub(super) const PORT_FILE_NAME: &str = "runtime.port";

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
}
