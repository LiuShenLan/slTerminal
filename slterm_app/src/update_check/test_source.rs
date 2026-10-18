//! 本地假 release 演练通道(09 篇采纳点 34)。
//!
//! 与上游同构:只接受显式回环地址 + 隔离配置目录;官方资产名、URL 与
//! digest 校验仍全程执行,只有 HTTP 传输换成本地夹具。存在 origin 时
//! 允许「同版本重装」彩排,但永不降级。
//!
//! 红线:显式演练通道永不回落公网。`origin()` 未设置时返回 `None`,
//! 生产路径与普通 debug 构建完全无感。

#[cfg(not(debug_assertions))]
compile_error!("update-test-source is for debug acceptance only; never enable it in a release");

/// 测试/演练需要两个条件:显式回环 origin + 隔离配置目录(防污染真实数据)。
pub(crate) fn origin() -> Result<Option<String>, String> {
    let Some(origin) = std::env::var_os("SLTERM_TEST_UPDATE_ORIGIN") else {
        return Ok(None);
    };
    if std::env::var_os("SLTERM_CONFIG_DIR").is_none_or(|path| path.is_empty()) {
        return Err("updater rehearsal requires an isolated SLTERM_CONFIG_DIR".into());
    }
    let origin = origin.to_str().ok_or("test update origin is not UTF-8")?;
    validate_origin(origin)?;
    Ok(Some(origin.to_owned()))
}

/// 只允许 `http://127.0.0.1:<明确端口>`:主机名、路径、认证信息一律拒绝。
fn validate_origin(origin: &str) -> Result<(), String> {
    let valid = origin
        .strip_prefix("http://127.0.0.1:")
        .filter(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|port| port.parse::<u16>().ok())
        .is_some_and(|port| port != 0);
    if valid {
        Ok(())
    } else {
        Err("test update origin must be http://127.0.0.1:<port>".into())
    }
}

/// 演练传输:直连、不跟随重定向、总时限由调用方给定。
pub(crate) fn agent(timeout: std::time::Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .proxy(None)
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

#[cfg(test)]
mod test_source_tests {
    use super::validate_origin;

    #[test]
    fn transport_is_limited_to_an_explicit_loopback_port() {
        assert!(validate_origin("http://127.0.0.1:45678").is_ok());
        for invalid in [
            "https://example.com",
            "http://127.0.0.1:0",
            "http://127.0.0.1:65536",
            "http://127.0.0.1:80@evil.example",
            "http://127.0.0.1:80/path",
            "http://127.0.0.1.evil.example:80",
            "http://localhost:80",
        ] {
            assert!(validate_origin(invalid).is_err(), "{invalid}");
        }
    }
}
