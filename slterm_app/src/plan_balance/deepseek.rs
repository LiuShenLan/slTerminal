//! DeepSeek 余额响应解析:取 `balance_infos[0]` 的金额与币种。

use super::{AmountInfo, PlanBalanceInfo};
use crate::error::AppError;

/// 响应解析(纯函数,罐装 JSON 可测):
/// balance_infos[0] 的 total_balance/currency;空数组/字段缺失/非字符串 → Err。
pub(crate) fn parse_deepseek_balance(
    body: &serde_json::Value,
) -> Result<PlanBalanceInfo, AppError> {
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
    Ok(PlanBalanceInfo {
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

    fn parse(json: &str) -> Result<PlanBalanceInfo, AppError> {
        parse_deepseek_balance(&serde_json::from_str(json).expect("合法 JSON 夹具"))
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

    /// 多币种只取 balance_infos[0]。
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
}
