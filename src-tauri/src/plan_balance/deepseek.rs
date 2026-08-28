//! deepseek 套餐（规格 §5.1）：GET /user/balance，取 balance_infos[0]

use super::query::{http_agent, query_err, PlanQuery};
use super::{AmountInfo, FetchOutcome};
use crate::error::AppError;
use std::time::Duration;

#[derive(Debug)]
pub struct DeepSeekQuery;

const TIMEOUT: Duration = Duration::from_secs(5);

impl PlanQuery for DeepSeekQuery {
    fn plan_id(&self) -> &'static str {
        "deepseek"
    }
    fn base_urls(&self) -> &'static [&'static str] {
        &["https://api.deepseek.com/anthropic"]
    }
    fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError> {
        let resp = http_agent(TIMEOUT)
            .get("https://api.deepseek.com/user/balance")
            .header("Accept", "application/json")
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|e| query_err(self.plan_id(), e))?;
        let body: serde_json::Value = resp
            .into_body()
            .read_json()
            .map_err(|e| AppError::Unknown(format!("deepseek 响应读取失败: {e}")))?;
        parse_deepseek_balance(&body)
    }
}

/// 响应解析（纯函数，罐装 JSON 可测）：
/// balance_infos[0] 的 total_balance/currency；空数组/字段缺失/非字符串 → Err
pub(crate) fn parse_deepseek_balance(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
    let first = body
        .get("balance_infos")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| AppError::Unknown("deepseek 响应缺 balance_infos[0]".into()))?;
    let value = first
        .get("total_balance")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Unknown("deepseek 响应缺 total_balance".into()))?;
    let currency = first
        .get("currency")
        .and_then(|v| v.as_str())
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
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<FetchOutcome, AppError> {
        parse_deepseek_balance(&serde_json::from_str(json).unwrap())
    }

    /// 正常 CNY：金额与币种原样透传，无窗口
    #[test]
    fn parse_ok_cny() {
        let json = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"0.00","topped_up_balance":"12.34"}]}"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        assert_eq!(
            o.amount,
            Some(AmountInfo {
                value: "12.34".into(),
                currency: "CNY".into()
            })
        );
        assert!(o.windows.is_none());
    }

    /// 正常 USD
    #[test]
    fn parse_ok_usd() {
        let json = r#"{"balance_infos":[{"currency":"USD","total_balance":"5.00"}]}"#;
        let o = parse(json).unwrap();
        assert_eq!(
            o.amount,
            Some(AmountInfo {
                value: "5.00".into(),
                currency: "USD".into()
            })
        );
    }

    /// 空 balance_infos 数组 → Err
    #[test]
    fn parse_empty_array_errors() {
        let json = r#"{"balance_infos":[]}"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("balance_infos[0]"));
    }

    /// 缺 balance_infos 键 → Err
    #[test]
    fn parse_missing_balance_infos_key_errors() {
        let err = parse(r#"{"other": 1}"#).unwrap_err();
        assert!(err.to_string().contains("balance_infos[0]"));
    }

    /// 缺 total_balance（非字符串/缺失）→ Err
    #[test]
    fn parse_missing_total_balance_errors() {
        let json = r#"{"balance_infos":[{"currency":"CNY"}]}"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("total_balance"));
    }

    /// 多币种只取 balance_infos[0]（规格 §5.1）
    #[test]
    fn parse_multi_currency_takes_first() {
        let json = r#"{"balance_infos":[{"currency":"USD","total_balance":"1.00"},{"currency":"CNY","total_balance":"2.00"}]}"#;
        let o = parse(json).unwrap();
        assert_eq!(
            o.amount,
            Some(AmountInfo {
                value: "1.00".into(),
                currency: "USD".into()
            })
        );
    }

    // 注：fetch 真实 HTTP 查询登记 test-inventory 既定豁免（真实外部 API 依赖，
    // 解析纯函数已全量覆盖，fetch 仅薄封装）。
}
