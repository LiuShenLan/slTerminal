//! kimi 套餐（规格 §5.2）：GET /coding/v1/usages，双时间窗 + 月限额触顶态

use std::time::Duration;
use crate::error::AppError;
use super::query::{http_agent, query_err, PlanQuery};
use super::{FetchOutcome, WindowInfo, WindowsInfo};

#[derive(Debug)]
pub struct KimiQuery;

const TIMEOUT: Duration = Duration::from_secs(8);

impl PlanQuery for KimiQuery {
    fn plan_id(&self) -> &'static str { "kimi" }
    fn base_urls(&self) -> &'static [&'static str] { &["https://api.kimi.com/coding"] }
    fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError> {
        let resp = http_agent(TIMEOUT)
            .get("https://api.kimi.com/coding/v1/usages")
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|e| query_err(self.plan_id(), e))?;
        let body: serde_json::Value = resp
            .into_body()
            .read_json()
            .map_err(|e| AppError::Unknown(format!("kimi 响应读取失败: {e}")))?;
        parse_kimi_usages(&body)
    }
}

/// 剩余百分比（D13）：(1 - used/limit) * 100 四舍五入，clamp 0–100；
/// 数值字段为字符串（规格口径），limit 缺失/为 0/非字符串 → None（该窗口失败）
fn remaining_percent(used: Option<&str>, limit: Option<&str>) -> Option<u8> {
    let used: f64 = used?.parse().ok()?;
    let limit: f64 = limit?.parse().ok()?;
    if limit <= 0.0 { return None; }
    Some(((1.0 - used / limit) * 100.0).round().clamp(0.0, 100.0) as u8)
}

/// 单窗口解析：used/limit/resetTime（resetTime 路径由调用方给）
fn parse_window(entry: &serde_json::Value, reset_path: &[&str]) -> Result<WindowInfo, AppError> {
    let percent = remaining_percent(
        entry.get("used").and_then(|v| v.as_str()),
        entry.get("limit").and_then(|v| v.as_str()),
    )
    .ok_or_else(|| AppError::Unknown("kimi 窗口 used/limit 解析失败".into()))?;
    let resets_at = reset_path
        .iter()
        .try_fold(entry, |v, k| v.get(k)) // 链式取 resetTime：任一步缺失即 None（手动 fold 触发 clippy manual_try_fold）
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(WindowInfo { remaining_percent: percent, resets_at })
}

/// 响应解析（纯函数，罐装 JSON 可测；规格 §5.2 全有或全无）：
/// 1. totalQuota.used == "1" → frozen=true（不要求窗口解析成功，windows=None）
/// 2. 5h 窗：limits[] 中 window.duration==300 && window.timeUnit=="TIME_UNIT_MINUTE"
///    优先，否则 limits[0]；limits 空 → 失败
/// 3. 7d 窗：顶层 usage 字段
/// 4. 非触顶时任一窗口失败 → 整体 Err（防窗口重置瞬间 limits 不完整致 5h 段丢失）
pub(crate) fn parse_kimi_usages(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
    let frozen = body
        .get("totalQuota")
        .and_then(|q| q.get("used"))
        .and_then(|v| v.as_str())
        == Some("1");
    if frozen {
        return Ok(FetchOutcome { frozen: true, amount: None, windows: None });
    }
    let limits = body.get("limits").and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Unknown("kimi 响应缺 limits".into()))?;
    let five_hour_entry = limits
        .iter()
        .find(|l| {
            l.get("window").and_then(|w| w.get("duration")).and_then(|d| d.as_u64()) == Some(300)
                && l.get("window").and_then(|w| w.get("timeUnit")).and_then(|t| t.as_str())
                    == Some("TIME_UNIT_MINUTE")
        })
        .or_else(|| limits.first())
        .ok_or_else(|| AppError::Unknown("kimi 响应 limits 为空".into()))?;
    let five_hour = parse_window(five_hour_entry, &["detail", "resetTime"])?;
    let usage = body.get("usage")
        .ok_or_else(|| AppError::Unknown("kimi 响应缺 usage".into()))?;
    let seven_day = parse_window(usage, &["resetTime"])?;
    Ok(FetchOutcome {
        frozen: false,
        amount: None,
        windows: Some(WindowsInfo { five_hour, seven_day }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<FetchOutcome, AppError> {
        parse_kimi_usages(&serde_json::from_str(json).unwrap())
    }

    /// 双窗正常：5h 24% / 7d 42%，resetTime 透传（F10）
    #[test]
    fn parse_ok_both_windows() {
        let json = r#"{
            "totalQuota": {"used": "0", "limit": "10"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "3800", "limit": "5000",
                 "detail": {"resetTime": "2026-08-28T15:00:00Z"}}
            ],
            "usage": {"used": "29000", "limit": "50000", "resetTime": "2026-08-29T00:00:00Z"}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 24);
        assert_eq!(w.five_hour.resets_at.as_deref(), Some("2026-08-28T15:00:00Z"));
        assert_eq!(w.seven_day.remaining_percent, 42);
        assert_eq!(w.seven_day.resets_at.as_deref(), Some("2026-08-29T00:00:00Z"));
    }

    /// 300min 优先：数组含 60min 与 300min 两条 → 选 300min（规格 §5.2 优先规则）
    #[test]
    fn parse_prefers_300min_window() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 60, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "2"},
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "3800", "limit": "5000"}
            ],
            "usage": {"used": "29000", "limit": "50000"}
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 24, "应命中 300min 窗口而非 limits[0]");
    }

    /// 无 300min 窗口 → 回退 limits[0]
    #[test]
    fn parse_falls_back_to_limits_first() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 60, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "2"}
            ],
            "usage": {"used": "29000", "limit": "50000"}
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 50, "应回退 limits[0]");
    }

    /// limits 空数组 → 整体 Err
    #[test]
    fn parse_empty_limits_errors() {
        let json = r#"{"totalQuota": {"used": "0"}, "limits": [], "usage": {}}"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("limits 为空"));
    }

    /// 5h limit="0" → 该窗口解析失败 → 整体 Err（防 5h 段丢失）
    #[test]
    fn parse_zero_limit_five_hour_errors() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "0"}
            ],
            "usage": {"used": "1", "limit": "2"}
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("used/limit 解析失败"));
    }

    /// 7d limit 缺失 → 整体 Err
    #[test]
    fn parse_missing_seven_day_limit_errors() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "2"}
            ],
            "usage": {"used": "1"}
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("used/limit 解析失败"));
    }

    /// 触顶：totalQuota.used=="1" → frozen=true，窗口缺失仍 Ok（不要求窗口解析成功）
    #[test]
    fn parse_frozen_true_even_without_windows() {
        let json = r#"{"totalQuota": {"used": "1", "limit": "10"}}"#;
        let o = parse(json).unwrap();
        assert!(o.frozen);
        assert!(o.amount.is_none());
        assert!(o.windows.is_none());
    }

    /// 非触顶但 usage 缺失 → 整体 Err
    #[test]
    fn parse_missing_usage_errors() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "2"}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("缺 usage"));
    }

    /// resetTime 缺失 → resets_at=None（窗口值保留，仅省略重置段）
    #[test]
    fn parse_missing_reset_time_yields_none() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "1", "limit": "2"}
            ],
            "usage": {"used": "1", "limit": "2"}
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 50);
        assert!(w.five_hour.resets_at.is_none());
        assert!(w.seven_day.resets_at.is_none());
    }

    /// used>limit → (1-used/limit) 为负 → clamp 0（D13）
    #[test]
    fn parse_used_over_limit_clamps_zero() {
        let json = r#"{
            "totalQuota": {"used": "0"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "used": "6000", "limit": "5000"}
            ],
            "usage": {"used": "29000", "limit": "50000"}
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 0);
    }

    // 注：fetch 真实 HTTP 查询登记 test-inventory 既定豁免（真实外部 API 依赖，
    // 解析纯函数已全量覆盖，fetch 仅薄封装）。
}
