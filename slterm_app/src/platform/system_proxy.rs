//! Windows 系统代理的单一解析点：更新请求与终端子进程环境共用。
//!
//! 只读 `HKCU\...\Internet Settings` 的 `ProxyEnable`/`ProxyServer`/
//! `ProxyOverride`，不写注册表、不缓存。`ProxyServer` 支持裸地址与
//! `http=`/`https=`/`socks=` 条目；socks 归一为 `socks5://`，避免把
//! SOCKS 端口当 HTTP 代理。

/// 已启用的系统代理投影；空字段表示该协议没有单独条目。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SystemProxy {
    pub(crate) http: Option<String>,
    pub(crate) https: Option<String>,
    pub(crate) socks: Option<String>,
    pub(crate) no_proxy: Vec<String>,
}

impl SystemProxy {
    /// 目标协议的代理 URL 与例外表；未知协议回退 http 条目，再回退 socks。
    pub(crate) fn for_scheme(&self, scheme: &str) -> Option<(&str, &[String])> {
        let exact = match scheme {
            "http" => self.http.as_deref(),
            "https" => self.https.as_deref(),
            _ => None,
        };
        exact
            .or(self.http.as_deref())
            .or(self.socks.as_deref())
            .map(|url| (url, self.no_proxy.as_slice()))
    }

    /// 子进程环境变量；HTTPS 缺条目时回退 http 条目，NO_PROXY 用逗号连接。
    pub(crate) fn environment(&self) -> Vec<(&'static str, String)> {
        let mut environment = Vec::new();
        if let Some(url) = &self.http {
            environment.push(("HTTP_PROXY", url.clone()));
        }
        if let Some(url) = self.https.as_ref().or(self.http.as_ref()) {
            environment.push(("HTTPS_PROXY", url.clone()));
        }
        if let Some(url) = &self.socks {
            environment.push(("ALL_PROXY", url.clone()));
        }
        if !self.no_proxy.is_empty() {
            environment.push(("NO_PROXY", self.no_proxy.join(",")));
        }
        environment
    }
}

/// 读取当前用户的系统代理；未启用、缺值或读取失败返回 `None`。
pub(crate) fn read() -> Option<SystemProxy> {
    internal::read()
}

/// 把 `ProxyServer` 与 `ProxyOverride` 原文解析成代理投影（测试与夹具入口）。
#[cfg(test)]
pub(crate) fn parse(server: &str, overrides: &str) -> Option<SystemProxy> {
    internal::parse(server, overrides)
}

mod internal {
    const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    use super::SystemProxy;

    #[cfg(windows)]
    pub(super) fn read() -> Option<SystemProxy> {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(INTERNET_SETTINGS)
            .ok()?;
        let enabled: u32 = key.get_value("ProxyEnable").ok()?;
        if enabled != 1 {
            return None;
        }
        let server: String = key.get_value("ProxyServer").ok()?;
        let overrides: String = key.get_value("ProxyOverride").unwrap_or_default();
        parse(&server, &overrides)
    }

    #[cfg(not(windows))]
    pub(super) fn read() -> Option<SystemProxy> {
        None
    }

    pub(super) fn parse(server: &str, overrides: &str) -> Option<SystemProxy> {
        let server = server.trim();
        if server.is_empty() {
            return None;
        }
        let no_proxy = overrides
            .split(';')
            .map(str::trim)
            // `<local>` 表示「不带点的主机名」；环境变量与更新请求都无法表达。
            .filter(|entry| !entry.is_empty() && *entry != "<local>")
            .map(str::to_owned)
            .collect();

        if !server.contains('=') {
            let url = with_http_scheme(server);
            return Some(SystemProxy {
                http: Some(url.clone()),
                https: Some(url),
                socks: None,
                no_proxy,
            });
        }

        let mut proxy = SystemProxy {
            no_proxy,
            ..SystemProxy::default()
        };
        for part in server.split(';') {
            // Windows 会写入 `ftp=` 等额外条目，末尾也常有 `;`。
            let Some((protocol, address)) = part.split_once('=') else {
                continue;
            };
            let address = address.trim();
            if address.is_empty() {
                continue;
            }
            match protocol.trim().to_ascii_lowercase().as_str() {
                "http" => proxy.http = Some(with_http_scheme(address)),
                "https" => proxy.https = Some(with_http_scheme(address)),
                "socks" => proxy.socks = Some(with_socks_scheme(address)),
                _ => {}
            }
        }
        (proxy.http.is_some() || proxy.https.is_some() || proxy.socks.is_some()).then_some(proxy)
    }

    fn with_http_scheme(address: &str) -> String {
        if address.contains("://") {
            address.to_owned()
        } else {
            format!("http://{address}")
        }
    }

    fn with_socks_scheme(address: &str) -> String {
        if address.contains("://") {
            address.to_owned()
        } else {
            format!("socks5://{address}")
        }
    }
}

#[cfg(test)]
mod system_proxy_tests {
    use super::{SystemProxy, parse};

    #[test]
    fn bare_address_applies_to_http_and_https() {
        let proxy = parse("127.0.0.1:7890", "").unwrap();
        assert_eq!(proxy.http.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(proxy.https.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(proxy.socks, None);
    }

    #[test]
    fn per_protocol_entries_keep_their_own_address() {
        let proxy = parse("http=127.0.0.1:7890;https=127.0.0.1:7891", "").unwrap();
        assert_eq!(proxy.http.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(proxy.https.as_deref(), Some("http://127.0.0.1:7891"));
        assert_eq!(
            proxy.for_scheme("https").map(|(url, _)| url),
            Some("http://127.0.0.1:7891")
        );
        assert_eq!(
            proxy.for_scheme("ftp").map(|(url, _)| url),
            Some("http://127.0.0.1:7890"),
            "未知协议回退 http 条目"
        );
    }

    #[test]
    fn socks_entries_use_socks5_and_serve_as_last_fallback() {
        let proxy = parse("socks=127.0.0.1:1080", "").unwrap();
        assert_eq!(proxy.socks.as_deref(), Some("socks5://127.0.0.1:1080"));
        assert_eq!(proxy.http, None);
        assert_eq!(
            proxy.for_scheme("https").map(|(url, _)| url),
            Some("socks5://127.0.0.1:1080")
        );
    }

    #[test]
    fn stray_parts_and_empty_values_do_not_discard_the_setting() {
        let proxy = parse("http=127.0.0.1:7890;", "").unwrap();
        assert_eq!(proxy.http.as_deref(), Some("http://127.0.0.1:7890"));
        let proxy = parse("ftp=1.2.3.4:21;http=127.0.0.1:7890;<local>", "").unwrap();
        assert_eq!(proxy.http.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(parse("", ""), None);
        assert_eq!(parse("   ", ""), None);
        assert_eq!(parse("http=", ""), None);
        assert_eq!(parse("ftp=1.2.3.4:21", ""), None);
    }

    #[test]
    fn environment_exports_proxy_channels_and_no_proxy() {
        let proxy = parse(
            "http=127.0.0.1:7890;socks=127.0.0.1:1080",
            "corp.local;*.internal;<local>;",
        )
        .unwrap();
        let environment = proxy.environment();
        assert!(environment.contains(&("HTTP_PROXY", "http://127.0.0.1:7890".to_owned())));
        assert_eq!(
            environment
                .iter()
                .find(|(name, _)| *name == "HTTPS_PROXY")
                .map(|(_, value)| value.as_str()),
            Some("http://127.0.0.1:7890"),
            "HTTPS 缺条目时回退 http"
        );
        assert!(environment.contains(&("ALL_PROXY", "socks5://127.0.0.1:1080".to_owned())));
        assert!(environment.contains(&("NO_PROXY", "corp.local,*.internal".to_owned())));
        assert_eq!(proxy.no_proxy, ["corp.local", "*.internal"]);
    }

    #[test]
    fn environment_omits_absent_channels() {
        let proxy = SystemProxy::default();
        assert!(proxy.environment().is_empty());
    }
}
