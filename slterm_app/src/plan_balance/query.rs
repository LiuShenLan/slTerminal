//! 套餐查询注册表(F10 语义原样):URL 匹配集 + fetch trait;静态切片。
//!
//! 查询语义零改动:URL 归一化只小写化 + 去尾斜杠(规格字面,不加 trim);
//! ureq 为纯阻塞 HTTP,调用方负责后台执行器;错误消息只含 planId + 错误
//! 类别,禁止拼接 token 与 Authorization 头(SEC-18 红线)。

use std::time::Duration;

use super::FetchOutcome;
use crate::error::AppError;
use crate::secrets::SecretBytes;

/// 套餐查询 trait:新增套餐 = 新实现 + QUERIES 一行注册。
pub(crate) trait PlanQuery: Send + Sync + std::fmt::Debug {
    fn plan_id(&self) -> &'static str;
    /// URL 匹配集(元素须为已归一化形态:小写、无尾斜杠)。
    fn base_urls(&self) -> &'static [&'static str];
    /// 阻塞 HTTP 查询(调用方须在 GPUI background executor 上执行);
    /// 错误消息禁止含 token/Authorization(红线)。
    fn fetch(&self, token: &SecretBytes) -> Result<FetchOutcome, AppError>;
}

static DEEPSEEK: super::deepseek::DeepSeekQuery = super::deepseek::DeepSeekQuery;
static KIMI: super::kimi::KimiQuery = super::kimi::KimiQuery;

/// 按注册序(emit 数组顺序即此序)。
pub(crate) static QUERIES: &[&dyn PlanQuery] = &[&DEEPSEEK, &KIMI];

/// URL 归一化(规格字面):小写化 + 去尾部斜杠。
pub(crate) fn normalize_base_url(url: &str) -> String {
    url.to_lowercase().trim_end_matches('/').to_string()
}

/// 归一化后与匹配集逐项精确相等(参数化查找供 L1 注入)。
pub(crate) fn find_query_by_url<'a>(
    base_url: &str,
    queries: &'a [&'a dyn PlanQuery],
) -> Option<&'a dyn PlanQuery> {
    let normalized = normalize_base_url(base_url);
    queries
        .iter()
        .copied()
        .find(|query| query.base_urls().contains(&normalized.as_str()))
}

/// ureq agent 工厂:timeout_global 覆盖连接/读写全程;4xx/5xx 默认即 Err。
pub(crate) fn http_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// 查询错误统一映射(消息只含 planId + 错误类别,禁止拼 token)。
pub(crate) fn query_err(plan_id: &str, error: ureq::Error) -> AppError {
    let kind = match &error {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::Timeout(_) => "超时".to_string(),
        _ => "网络错误".to_string(),
    };
    log::warn!("套餐余量 HTTP 查询失败: plan={plan_id} error={error}");
    AppError::Unknown(format!("套餐 {plan_id} 查询失败: {kind}"))
}

/// 单请求本地 HTTP 服务:端到端验证请求形态(方法/路径/Authorization)与
/// 响应解析,不出网、不依赖真实 API(测试形态)。
#[cfg(test)]
pub(crate) mod test_support {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;
    use std::time::Duration;

    /// 返回 (endpoint 前缀, 完成句柄);句柄输出 = 原始请求头文本。
    pub(crate) fn serve_once(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定本地 HTTP 端口");
        let address = listener.local_addr().expect("读取本地端口");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("接受本地请求");
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("读取请求");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&request).into_owned();
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("写响应");
            stream.flush().expect("刷新响应");
            text
        });
        (format!("http://{address}"), handle)
    }

    /// 接受连接后保持沉默(触发客户端全局超时);句柄用于回收线程。
    pub(crate) fn serve_silent(delay: Duration) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定本地 HTTP 端口");
        let address = listener.local_addr().expect("读取本地端口");
        let handle = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("接受本地请求");
            std::thread::sleep(delay);
        });
        (format!("http://{address}"), handle)
    }

    /// 请求头大小写不敏感查找(HTTP 字段名大小写不敏感)。
    pub(crate) fn request_contains(request: &str, needle: &str) -> bool {
        request.to_lowercase().contains(&needle.to_lowercase())
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    // ── normalize_base_url(4 例;规格字面:仅小写 + 去尾斜杠,不加 trim) ──

    /// 大写宿主 → 全小写。
    #[test]
    fn normalize_lowercases_host() {
        assert_eq!(
            normalize_base_url("https://API.DEEPSEEK.COM/anthropic"),
            "https://api.deepseek.com/anthropic"
        );
    }

    /// 单尾斜杠 → 去除。
    #[test]
    fn normalize_strips_single_trailing_slash() {
        assert_eq!(
            normalize_base_url("https://api.kimi.com/coding/"),
            "https://api.kimi.com/coding"
        );
    }

    /// 多尾斜杠 → 全部去除。
    #[test]
    fn normalize_strips_all_trailing_slashes() {
        assert_eq!(
            normalize_base_url("https://api.kimi.com/coding///"),
            "https://api.kimi.com/coding"
        );
    }

    /// 已归一形态 → 保持不变。
    #[test]
    fn normalize_keeps_normalized_unchanged() {
        assert_eq!(
            normalize_base_url("https://api.deepseek.com/anthropic"),
            "https://api.deepseek.com/anthropic"
        );
    }

    // ── find_query_by_url(4 例,参数化注入) ──

    /// deepseek base_url 命中。
    #[test]
    fn find_hits_deepseek() {
        let query = find_query_by_url("https://api.deepseek.com/anthropic", QUERIES)
            .expect("deepseek 应命中");
        assert_eq!(query.plan_id(), "deepseek");
    }

    /// kimi base_url 命中。
    #[test]
    fn find_hits_kimi() {
        let query = find_query_by_url("https://api.kimi.com/coding", QUERIES).expect("kimi 应命中");
        assert_eq!(query.plan_id(), "kimi");
    }

    /// 大小写 + 尾斜杠归一后仍命中(匹配前统一归一化)。
    #[test]
    fn find_matches_after_normalization() {
        let query = find_query_by_url("HTTPS://API.DEEPSEEK.COM/ANTHROPIC/", QUERIES)
            .expect("归一化后应命中");
        assert_eq!(query.plan_id(), "deepseek");
    }

    /// 未命中(其他厂商 base_url)→ None。
    #[test]
    fn find_unmatched_returns_none() {
        assert!(find_query_by_url("https://api.other.com/v1", QUERIES).is_none());
    }

    // ── 注册表序(1 例,emit 数组顺序即注册序) ──

    /// QUERIES plan_id 序恒为 ["deepseek", "kimi"](渲染顺序依赖此序)。
    #[test]
    fn queries_registry_order() {
        let ids: Vec<&str> = QUERIES.iter().map(|query| query.plan_id()).collect();
        assert_eq!(ids, ["deepseek", "kimi"]);
    }

    // ── query_err 分类(3 例;消息只含 planId + 类别,无 token) ──

    /// HTTP 状态错误 → "HTTP {code}"。
    #[test]
    fn query_err_maps_status_code() {
        let error = query_err("kimi", ureq::Error::StatusCode(500));
        assert_eq!(
            error.to_string(),
            "unknown error: 套餐 kimi 查询失败: HTTP 500"
        );
    }

    /// 超时 → "超时"。
    #[test]
    fn query_err_maps_timeout() {
        let error = query_err("deepseek", ureq::Error::Timeout(ureq::Timeout::Global));
        assert_eq!(
            error.to_string(),
            "unknown error: 套餐 deepseek 查询失败: 超时"
        );
    }

    /// 其余错误 → "网络错误"。
    #[test]
    fn query_err_maps_other_network_errors() {
        let error = query_err("kimi", ureq::Error::HostNotFound);
        assert_eq!(
            error.to_string(),
            "unknown error: 套餐 kimi 查询失败: 网络错误"
        );
    }

    /// 错误消息不得携带 token(SEC-18):分类映射不接受也不拼接凭据。
    #[test]
    fn query_err_never_contains_credential_text() {
        let error = query_err("kimi", ureq::Error::StatusCode(401));
        assert!(!error.to_string().contains("sk-test"));
        assert!(!format!("{error:?}").contains("sk-test"));
    }
}
