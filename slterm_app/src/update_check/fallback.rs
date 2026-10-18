//! GitHub 公共 latest-release 重定向独立于 REST 配额(09 篇采纳点 26 附注)。
//!
//! 因果链(上游 note `2026-09-23-public-release-fallback.md` 口径随迁):
//! 共享出口 IP 的 REST 配额耗尽时,启动与手动检查会同时 403/429,即使
//! 发布网页可达。只有 REST 403/429 才发起第二次有界请求:
//! - 官方 `/releases/latest` 页面,≤5 跳,终 URL 必须是本仓 HTTPS 且为
//!   精确数字 major.minor.patch tag(可选 v/V 前缀);
//! - 重定向证明版本;随后取官方 `SHA256SUMS`(10s / 64KiB 预算)恢复受验证
//!   的下载元数据。
//!
//! 明确拒绝:嵌 token、猜包 URL、把 API 失败当最新。重定向只证明版本,
//! 不授予安装权限;无 manifest 条目时只保留手动下载。

use std::time::Duration;

use ureq::ResponseExt as _;

use super::LatestRelease;
use super::RELEASE_DOWNLOAD_PREFIX;

const LATEST_PAGE: &str = "https://github.com/LiuShenLan/slTerminal/releases/latest";
const TAG_PREFIX: &str = "https://github.com/LiuShenLan/slTerminal/releases/tag/";

pub(super) fn fetch_latest(status: u16) -> Result<LatestRelease, String> {
    log::info!("update-check: API returned HTTP {status}; checking official latest-release page");
    // github.com 与 api.github.com 的代理设置独立解析。
    let agent = crate::update_proxy::agent(LATEST_PAGE, Duration::from_secs(10));
    let uri = redirected_uri(&agent, LATEST_PAGE)
        .map_err(|error| format!("GitHub HTTP {status}; release page failed: {error}"))?;
    let mut release = release_from_uri(&uri)
        .ok_or_else(|| "官方 latest release 页面不是可识别的稳定 tag".to_owned())?;
    // 官方 manifest 恢复受验证的下载元数据;不可用时仍保留版本发现(手动
    // 下载),绝不由 tag 猜包 URL。
    let url = format!("{RELEASE_DOWNLOAD_PREFIX}v{}/SHA256SUMS", release.version);
    let agent = crate::update_proxy::agent(&url, Duration::from_secs(10));
    if let Ok(mut response) = agent.get(&url).header("User-Agent", "slterm").call()
        && let Ok(text) = response
            .body_mut()
            .with_config()
            .limit(64 * 1024)
            .read_to_string()
    {
        release.asset = super::assets::from_checksums(&release.version, &text);
    }
    Ok(release)
}

fn redirected_uri(agent: &ureq::Agent, url: &str) -> Result<String, ureq::Error> {
    let response = agent
        .get(url)
        .header("User-Agent", "slterm")
        .header("Accept", "text/html")
        .config()
        .max_redirects(5)
        .build()
        .call()?;
    // 终 URI 由 HTTP 客户端提供,不解析不可信 HTML 正文。
    Ok(response.get_uri().to_string())
}

fn release_from_uri(uri: &str) -> Option<LatestRelease> {
    let tag = uri.strip_prefix(TAG_PREFIX)?;
    let version = tag.strip_prefix(['v', 'V']).unwrap_or(tag);
    // 官方稳定发布用 major.minor.patch;拒绝登录页、任意 tag、额外路径/
    // 查询/片段与预发布 tag。
    let components: Vec<_> = version.split('.').collect();
    if components.len() != 3
        || components.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.parse::<u64>().is_err()
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return None;
    }
    Some(LatestRelease {
        version: version.to_owned(),
        asset: None,
    })
}

#[cfg(test)]
mod fallback_tests {
    use super::{TAG_PREFIX, release_from_uri};
    use crate::update_proxy::test_support::{Server, response};

    #[test]
    fn rate_limited_api_falls_back_and_discovers_an_upgrade_without_install_authority() {
        for status in ["403 Forbidden", "429 Too Many Requests"] {
            let server = Server::start(vec![response(status, "", "API rate limit exceeded")]);
            let result = crate::update_check::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |code| {
                    assert!(matches!(code, 403 | 429));
                    Ok(release_from_uri(&format!("{TAG_PREFIX}v1.9.0")).unwrap())
                },
            )
            .unwrap();
            assert!(crate::update_check::is_newer(&result.version, "1.8.2"));
            assert!(!crate::update_check::is_newer(&result.version, "1.9.0"));
            assert!(
                result.asset.is_none(),
                "a version redirect grants no install authority"
            );
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[test]
    fn successful_api_and_other_errors_never_call_the_fallback() {
        for (status, body, succeeds) in [
            ("200 OK", r#"{"tag_name":"v1.9.0","assets":[]}"#, true),
            ("200 OK", "not JSON", false),
            ("404 Not Found", "missing release", false),
            ("500 Internal Server Error", "server error", false),
        ] {
            let server = Server::start(vec![response(status, "", body)]);
            let result = crate::update_check::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |_| panic!("fallback is only for access/rate limiting"),
            );
            assert_eq!(result.is_ok(), succeeds);
            server.finish();
        }
    }

    #[test]
    fn failed_fallback_stays_an_error_instead_of_claiming_up_to_date() {
        let server = Server::start(vec![response("403 Forbidden", "", "rate limited")]);
        let result = crate::update_check::fetch_release_with_fallback(
            &server.agent(&[]),
            "http://api.update.invalid/latest",
            |_| Err("release page unavailable".into()),
        );
        assert_eq!(result.unwrap_err(), "release page unavailable");
        server.finish();
    }

    #[test]
    fn redirect_transport_uses_the_final_location_without_parsing_html() {
        // 本地夹具用 HTTP;生产 agent 强制 HTTPS。
        let destination = "http://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0";
        let server = Server::start(vec![
            response("302 Found", &format!("Location: {destination}\r\n"), ""),
            response(
                "200 OK",
                "Content-Type: text/html\r\n",
                "<html>no metadata</html>",
            ),
        ]);
        let uri = super::redirected_uri(
            &server.agent(&[]),
            "http://github.com/LiuShenLan/slTerminal/releases/latest",
        )
        .unwrap();
        assert_eq!(uri, destination);
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].contains("/releases/latest"));
        assert!(requests[1].contains("/releases/tag/v1.9.0"));
    }

    #[test]
    fn only_exact_official_stable_tag_locations_are_accepted() {
        for tag in ["v1.9.0", "V1.9.0", "1.9.0"] {
            assert_eq!(
                release_from_uri(&format!("{TAG_PREFIX}{tag}"))
                    .unwrap()
                    .version,
                "1.9.0"
            );
        }
        for uri in [
            "https://github.com/LiuShenLan/slTerminal/releases/latest",
            "https://github.com/login",
            "https://github.com/Other/slTerminal/releases/tag/v99.0.0",
            "http://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0",
            "https://github.com.evil.invalid/LiuShenLan/slTerminal/releases/tag/v1.9.0",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0?foo=bar",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0#fragment",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0/extra",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v1.9.0-rc1",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/relay-abcd",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v1..0",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v01.9.0",
            "https://github.com/LiuShenLan/slTerminal/releases/tag/v18446744073709551616.0.0",
        ] {
            assert!(release_from_uri(uri).is_none(), "{uri}");
        }
    }
}
