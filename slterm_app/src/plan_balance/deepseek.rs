//! deepseek 套餐(规格 §5.1,F10 语义原样):GET /user/balance,取 balance_infos[0]。
//!
//! 请求头组装是 `SecretBytes::as_bytes` 的允许借出点之一(B60 白名单):
//! token 只在 Authorization 组装瞬时栈帧中以 `Zeroizing<String>` 存在,
//! 不进入日志、错误消息或任何快照/ DTO 类型。

use std::time::Duration;

use zeroize::Zeroizing;

use super::query::{PlanQuery, http_agent, query_err};
use super::{AmountInfo, FetchOutcome};
use crate::error::AppError;
use crate::secrets::SecretBytes;

#[derive(Debug)]
pub(crate) struct DeepSeekQuery;

const TIMEOUT: Duration = Duration::from_secs(5);
const BALANCE_ENDPOINT: &str = "https://api.deepseek.com/user/balance";

impl PlanQuery for DeepSeekQuery {
    fn plan_id(&self) -> &'static str {
        "deepseek"
    }

    fn base_urls(&self) -> &'static [&'static str] {
        &["https://api.deepseek.com/anthropic"]
    }

    fn fetch(&self, token: &SecretBytes) -> Result<FetchOutcome, AppError> {
        self.fetch_at(BALANCE_ENDPOINT, TIMEOUT, token)
    }
}

impl DeepSeekQuery {
    /// 显式 endpoint/timeout 形态:生产走常量,L1 用本地 HTTP 服务验证请求形态。
    fn fetch_at(
        &self,
        endpoint: &str,
        timeout: Duration,
        token: &SecretBytes,
    ) -> Result<FetchOutcome, AppError> {
        // 借出点:token 字节仅用于拼 Authorization;拼串本体 Zeroizing。
        let token_text = std::str::from_utf8(token.as_bytes())
            .map_err(|_| AppError::Unknown("deepseek 查询失败: token 非 UTF-8".into()))?;
        let authorization = Zeroizing::new(format!("Bearer {token_text}"));
        let resp = http_agent(timeout)
            .get(endpoint)
            .header("Accept", "application/json")
            .header("Authorization", authorization.as_str())
            .call()
            .map_err(|error| query_err(self.plan_id(), error))?;
        let body: serde_json::Value = resp
            .into_body()
            .read_json()
            .map_err(|error| AppError::Unknown(format!("deepseek 响应读取失败: {error}")))?;
        parse_deepseek_balance(&body)
    }
}

/// 响应解析(纯函数,罐装 JSON 可测):
/// balance_infos[0] 的 total_balance/currency;空数组/字段缺失/非字符串 → Err。
pub(crate) fn parse_deepseek_balance(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
    let first = body
        .get("balance_infos")
        .and_then(|value| value.as_array())
        .and_then(|infos| infos.first())
        .ok_or_else(|| AppError::Unknown("deepseek 响应缺 balance_infos[0]".into()))?;
    let value = first
        .get("total_balance")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::Unknown("deepseek 响应缺 total_balance".into()))?;
    let currency = first
        .get("currency")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::Unknown("deepseek 响应缺 currency".into()))?;
    Ok(FetchOutcome {
        frozen: false,
        amount: Some(AmountInfo {
            value: value.into(),
            currency: currency.into(),
        }),
        windows: None,
    })
}

#[cfg(test)]
mod deepseek_tests {
    use super::*;
    use crate::plan_balance::query::test_support::{request_contains, serve_once, serve_silent};

    fn parse(json: &str) -> Result<FetchOutcome, AppError> {
        parse_deepseek_balance(&serde_json::from_str(json).expect("合法 JSON 夹具"))
    }

    fn test_token() -> SecretBytes {
        SecretBytes::new(b"sk-test-deepseek".to_vec())
    }

    // ── 解析纯函数(6 例) ──

    /// 正常 CNY:金额与币种原样透传,无窗口。
    #[test]
    fn parse_ok_cny() {
        let json = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"0.00","topped_up_balance":"12.34"}]}"#;
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        assert_eq!(
            outcome.amount,
            Some(AmountInfo {
                value: "12.34".into(),
                currency: "CNY".into()
            })
        );
        assert!(outcome.windows.is_none());
    }

    /// 正常 USD。
    #[test]
    fn parse_ok_usd() {
        let json = r#"{"balance_infos":[{"currency":"USD","total_balance":"5.00"}]}"#;
        let outcome = parse(json).expect("应解析成功");
        assert_eq!(
            outcome.amount,
            Some(AmountInfo {
                value: "5.00".into(),
                currency: "USD".into()
            })
        );
    }

    /// 空 balance_infos 数组 → Err。
    #[test]
    fn parse_empty_array_errors() {
        let error = parse(r#"{"balance_infos":[]}"#).expect_err("空数组应失败");
        assert!(error.to_string().contains("balance_infos[0]"));
    }

    /// 缺 balance_infos 键 → Err。
    #[test]
    fn parse_missing_balance_infos_key_errors() {
        let error = parse(r#"{"other": 1}"#).expect_err("缺键应失败");
        assert!(error.to_string().contains("balance_infos[0]"));
    }

    /// 缺 total_balance(非字符串/缺失)→ Err。
    #[test]
    fn parse_missing_total_balance_errors() {
        let error = parse(r#"{"balance_infos":[{"currency":"CNY"}]}"#).expect_err("缺金额应失败");
        assert!(error.to_string().contains("total_balance"));
    }

    /// 多币种只取 balance_infos[0](规格 §5.1)。
    #[test]
    fn parse_multi_currency_takes_first() {
        let json = r#"{"balance_infos":[{"currency":"USD","total_balance":"1.00"},{"currency":"CNY","total_balance":"2.00"}]}"#;
        let outcome = parse(json).expect("应解析成功");
        assert_eq!(
            outcome.amount,
            Some(AmountInfo {
                value: "1.00".into(),
                currency: "USD".into()
            })
        );
    }

    // ── 本地 HTTP 服务(请求形态 + 错误映射,不出网) ──

    /// GET 路径 + Authorization Bearer + 响应解析全链。
    #[test]
    fn fetch_sends_bearer_and_parses_balance() {
        let (base, server) = serve_once(
            "HTTP/1.1 200 OK",
            r#"{"balance_infos":[{"currency":"CNY","total_balance":"12.34"}]}"#,
        );
        let outcome = DeepSeekQuery
            .fetch_at(&format!("{base}/user/balance"), TIMEOUT, &test_token())
            .expect("本地查询应成功");
        assert_eq!(
            outcome.amount,
            Some(AmountInfo {
                value: "12.34".into(),
                currency: "CNY".into()
            })
        );
        let request = server.join().expect("本地服务线程");
        assert!(request_contains(&request, "GET /user/balance HTTP/1.1"));
        assert!(
            request_contains(&request, "authorization: bearer sk-test-deepseek"),
            "Authorization 头缺失或形态错误: {request}"
        );
        assert!(request_contains(&request, "accept: application/json"));
    }

    /// HTTP 401 → 分类错误,消息不含 token。
    #[test]
    fn fetch_maps_status_error_without_leaking_token() {
        let (base, server) = serve_once("HTTP/1.1 401 Unauthorized", "{}");
        let error = DeepSeekQuery
            .fetch_at(&format!("{base}/user/balance"), TIMEOUT, &test_token())
            .expect_err("401 应失败");
        let _ = server.join();
        let message = error.to_string();
        assert!(message.contains("HTTP 401"), "实际: {message}");
        assert!(
            !message.contains("sk-test-deepseek"),
            "错误消息泄露 token: {message}"
        );
    }

    /// 超时 → "超时"类别(本地服务接受连接后保持沉默)。
    #[test]
    fn fetch_maps_timeout() {
        let (base, server) = serve_silent(Duration::from_millis(400));
        let error = DeepSeekQuery
            .fetch_at(
                &format!("{base}/user/balance"),
                Duration::from_millis(100),
                &test_token(),
            )
            .expect_err("超时应失败");
        let _ = server.join();
        assert!(error.to_string().contains("超时"), "实际: {error}");
    }

    /// 响应体不可解析 → 读取错误,消息不含 token。
    #[test]
    fn fetch_maps_unreadable_body_without_leaking_token() {
        let (base, server) = serve_once("HTTP/1.1 200 OK", "not json");
        let error = DeepSeekQuery
            .fetch_at(&format!("{base}/user/balance"), TIMEOUT, &test_token())
            .expect_err("坏响应体应失败");
        let _ = server.join();
        let message = error.to_string();
        assert!(message.contains("响应读取失败"), "实际: {message}");
        assert!(
            !message.contains("sk-test-deepseek"),
            "错误消息泄露 token: {message}"
        );
    }

    /// token 非 UTF-8 → 联网前拒绝。
    #[test]
    fn fetch_rejects_non_utf8_token_before_network() {
        let token = SecretBytes::new(vec![0xff, 0xfe]);
        let error = DeepSeekQuery
            .fetch_at(
                "http://127.0.0.1:9/unreachable",
                Duration::from_millis(50),
                &token,
            )
            .expect_err("非 UTF-8 token 应失败");
        assert!(error.to_string().contains("token 非 UTF-8"));
    }
}
