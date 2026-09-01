//! kimi 套餐（实测口径，2026-08 实证修正规格 §5.2 字段漂移）：GET /coding/v1/usages，
//! 双时间窗 + 配额耗尽冻结态

use super::query::{http_agent, query_err, PlanQuery};
use super::{FetchOutcome, WindowInfo, WindowsInfo};
use crate::error::AppError;
use std::time::Duration;

#[derive(Debug)]
pub struct KimiQuery;

const TIMEOUT: Duration = Duration::from_secs(8);

impl PlanQuery for KimiQuery {
    fn plan_id(&self) -> &'static str {
        "kimi"
    }
    fn base_urls(&self) -> &'static [&'static str] {
        &["https://api.kimi.com/coding"]
    }
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

/// 剩余百分比：remaining 优先（remaining/limit），used 回退（(1-used/limit)*100）；
/// 数值字段为字符串（实证口径），limit 缺失/为 0 → None（该窗口失败）。
/// 实证（2026-08 实测 + 社区审计）：remaining 恒在、used 可缺 → remaining 优先天然覆盖两种形态
fn remaining_percent(
    remaining: Option<&str>,
    used: Option<&str>,
    limit: Option<&str>,
) -> Option<u8> {
    let limit: f64 = limit?.parse().ok()?;
    if limit <= 0.0 {
        return None;
    }
    if let Some(r) = remaining.and_then(|v| v.parse::<f64>().ok()) {
        return Some((r / limit * 100.0).round().clamp(0.0, 100.0) as u8);
    }
    let used: f64 = used?.parse().ok()?;
    Some(((1.0 - used / limit) * 100.0).round().clamp(0.0, 100.0) as u8)
}

/// 单窗口解析：detail 内 remaining/used/limit/resetTime 直接取值（实证：5h 窗
/// 数值承载于 limits[i].detail 内层，7d 窗为顶层 usage——两者均直接含 resetTime）
fn parse_window(detail: &serde_json::Value) -> Result<WindowInfo, AppError> {
    let percent = remaining_percent(
        detail.get("remaining").and_then(|v| v.as_str()),
        detail.get("used").and_then(|v| v.as_str()),
        detail.get("limit").and_then(|v| v.as_str()),
    )
    .ok_or_else(|| AppError::Unknown("kimi 窗口 remaining/used/limit 解析失败".into()))?;
    let resets_at = detail
        .get("resetTime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(WindowInfo {
        remaining_percent: percent,
        resets_at,
    })
}

/// 响应解析（纯函数，罐装 JSON 可测；全有或全无）：
/// 1. frozen（实证口径）：totalQuota.remaining 数值 ≤ 0 → 配额耗尽冻结
///    （不要求窗口解析成功，windows=None）
/// 2. 5h 窗：limits[] 中 window.duration==300 && window.timeUnit=="TIME_UNIT_MINUTE"
///    优先，否则 limits[0]；limits 空 → 失败；数值读取 limits[i].detail 内层
/// 3. 7d 窗：顶层 usage 字段
/// 4. 非触顶时任一窗口失败 → 整体 Err（防窗口重置瞬间 limits 不完整致 5h 段丢失）
pub(crate) fn parse_kimi_usages(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
    // totalQuota 缺失/为空对象（实测形态）/remaining 非数字或非 0 → 未冻结
    let frozen = body
        .get("totalQuota")
        .and_then(|q| q.get("remaining"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|r| r <= 0.0)
        .unwrap_or(false);
    if frozen {
        return Ok(FetchOutcome {
            frozen: true,
            amount: None,
            windows: None,
        });
    }
    let limits = body
        .get("limits")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Unknown("kimi 响应缺 limits".into()))?;
    let five_hour_entry = limits
        .iter()
        .find(|l| {
            l.get("window")
                .and_then(|w| w.get("duration"))
                .and_then(|d| d.as_u64())
                == Some(300)
                && l.get("window")
                    .and_then(|w| w.get("timeUnit"))
                    .and_then(|t| t.as_str())
                    == Some("TIME_UNIT_MINUTE")
        })
        .or_else(|| limits.first())
        .ok_or_else(|| AppError::Unknown("kimi 响应 limits 为空".into()))?;
    // 数值承载于 detail 内层（实证：limits[i].detail 含 limit/used/remaining/resetTime）
    let detail = five_hour_entry
        .get("detail")
        .ok_or_else(|| AppError::Unknown("kimi 响应窗口缺 detail".into()))?;
    let five_hour = parse_window(detail)?;
    let usage = body
        .get("usage")
        .ok_or_else(|| AppError::Unknown("kimi 响应缺 usage".into()))?;
    let seven_day = parse_window(usage)?;
    Ok(FetchOutcome {
        frozen: false,
        amount: None,
        windows: Some(WindowsInfo {
            five_hour,
            seven_day,
        }),
    })
}

#[cfg(test)]
mod kimi_tests {
    use super::*;

    fn parse(json: &str) -> Result<FetchOutcome, AppError> {
        parse_kimi_usages(&serde_json::from_str(json).unwrap())
    }

    /// 双窗正常（真实响应数字）：5h 99% / 7d 77%，resetTime 透传（F10 修复后口径）
    #[test]
    fn parse_ok_both_windows() {
        let json = r#"{
            "usage": {"limit": "100", "used": "23", "remaining": "77", "resetTime": "2026-09-04T00:18:45Z"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99", "resetTime": "2026-08-28T22:18:45Z"}}
            ],
            "totalQuota": {"limit": "100", "remaining": "99"}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.remaining_percent, 99,
            "5h 窗 remaining 优先：99/100"
        );
        assert_eq!(
            w.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45Z")
        );
        assert_eq!(
            w.seven_day.remaining_percent, 77,
            "7d 窗 remaining 优先：77/100"
        );
        assert_eq!(
            w.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45Z")
        );
    }

    /// remaining 与 used 并存且不一致 → remaining 优先（used 推算 99，remaining 50）
    #[test]
    fn parse_remaining_preferred_over_used() {
        let json = r#"{
            "usage": {"limit": "100", "used": "99", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "99", "remaining": "50"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.remaining_percent, 50,
            "应取 remaining 而非 used 推算"
        );
        assert_eq!(w.seven_day.remaining_percent, 50);
    }

    /// remaining 缺失 → 回退 used 推算（detail 仅 {limit, used}）
    #[test]
    fn parse_missing_remaining_falls_back_used() {
        let json = r#"{
            "usage": {"limit": "100", "used": "25"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 99, "(1-1/100) 推算");
        assert_eq!(w.seven_day.remaining_percent, 75, "(1-25/100) 推算");
    }

    /// remaining 非数字串 → 回退 used 推算
    #[test]
    fn parse_remaining_non_numeric_falls_back_used() {
        let json = r#"{
            "usage": {"limit": "2", "used": "1", "remaining": "abc"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "2", "used": "1", "remaining": "abc"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.remaining_percent, 50,
            "remaining 不可解析 → used 推算"
        );
    }

    /// remaining 与 used 均缺失 → 窗口失败 → 整体 Err
    #[test]
    fn parse_missing_remaining_and_used_errors() {
        let json = r#"{
            "usage": {"limit": "100"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("remaining/used/limit 解析失败"));
    }

    /// 300min 优先：数组含 60min 与 300min 两条 → 选 300min（优先规则不变）
    #[test]
    fn parse_prefers_300min_window() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 60, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "2", "used": "1", "remaining": "1"}},
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "24"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.remaining_percent, 24,
            "应命中 300min 窗口而非 limits[0]"
        );
    }

    /// 无 300min 窗口 → 回退 limits[0]
    #[test]
    fn parse_falls_back_to_limits_first() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 60, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "2", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 50, "应回退 limits[0]：1/2");
    }

    /// limits 空数组 → 整体 Err
    #[test]
    fn parse_empty_limits_errors() {
        let json = r#"{"totalQuota": {"remaining": "99"}, "limits": [], "usage": {}}"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("limits 为空"));
    }

    /// 窗口条目缺 detail（结构再漂移）→ 整体 Err
    #[test]
    fn parse_missing_detail_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("缺 detail"));
    }

    /// 5h limit="0" → 该窗口解析失败 → 整体 Err（防 5h 段丢失）
    #[test]
    fn parse_zero_limit_five_hour_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "0", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("remaining/used/limit 解析失败"));
    }

    /// 5h limit 缺失 → 整体 Err
    #[test]
    fn parse_missing_limit_five_hour_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"used": "1", "remaining": "1"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("remaining/used/limit 解析失败"));
    }

    /// 7d limit 缺失 → 整体 Err
    #[test]
    fn parse_missing_seven_day_limit_errors() {
        let json = r#"{
            "usage": {"remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("remaining/used/limit 解析失败"));
    }

    /// 冻结：totalQuota.remaining="0" → frozen=true，窗口缺失仍 Ok
    #[test]
    fn parse_frozen_when_total_quota_remaining_zero() {
        let json = r#"{"totalQuota": {"limit": "100", "remaining": "0"}}"#;
        let o = parse(json).unwrap();
        assert!(o.frozen);
        assert!(o.amount.is_none());
        assert!(o.windows.is_none());
    }

    /// remaining 为小数正残值 → 未冻结且窗口正常解析
    #[test]
    fn parse_not_frozen_when_remaining_fraction() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "50"}}
            ],
            "totalQuota": {"limit": "100", "remaining": "0.5"}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        assert!(o.windows.is_some());
    }

    /// totalQuota 缺失 → 未冻结且窗口正常解析
    #[test]
    fn parse_not_frozen_when_total_quota_missing() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "50"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        assert!(o.windows.is_some());
    }

    /// totalQuota 为空对象（2026-08 实测形态）→ 未冻结且窗口正常解析
    #[test]
    fn parse_not_frozen_when_total_quota_empty_object() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "50"}}
            ],
            "totalQuota": {}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        assert!(o.windows.is_some(), "空对象不应中断窗口解析");
    }

    /// remaining 非数字串 → 未冻结（parse 失败回落 false）且窗口正常解析
    #[test]
    fn parse_not_frozen_when_remaining_non_numeric() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "50"}}
            ],
            "totalQuota": {"limit": "100", "remaining": "abc"}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen);
        assert!(o.windows.is_some());
    }

    /// 冻结时不要求窗口解析成功（windows=None 仍 Ok）
    #[test]
    fn parse_frozen_true_even_without_windows() {
        let json = r#"{"totalQuota": {"limit": "10", "remaining": "0"}}"#;
        let o = parse(json).unwrap();
        assert!(o.frozen);
        assert!(o.amount.is_none());
        assert!(o.windows.is_none());
    }

    /// 非冻结但 usage 缺失 → 整体 Err
    #[test]
    fn parse_missing_usage_errors() {
        let json = r#"{
            "totalQuota": {"remaining": "99"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("缺 usage"));
    }

    /// resetTime 缺失 → resets_at=None（窗口值保留，仅省略重置段）
    #[test]
    fn parse_missing_reset_time_yields_none() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "50"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 50);
        assert!(w.five_hour.resets_at.is_none());
        assert!(w.seven_day.resets_at.is_none());
    }

    /// used>limit（无 remaining）→ (1-used/limit) 为负 → clamp 0
    #[test]
    fn parse_used_over_limit_clamps_zero() {
        let json = r#"{
            "usage": {"limit": "5000", "used": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "used": "6000"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 0);
    }

    /// remaining>limit（不一致场景）→ 超 100 → clamp 100
    #[test]
    fn parse_remaining_over_limit_clamps_hundred() {
        let json = r#"{
            "usage": {"limit": "5000", "remaining": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "remaining": "6000"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.remaining_percent, 100,
            "6000/5000=120 → clamp 100"
        );
    }

    /// vibeusage 实测形态：detail 无 used 仅 {limit, remaining, resetTime} → remaining 优先成功
    #[test]
    fn parse_detail_without_used_uses_remaining() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 91);
        assert_eq!(w.seven_day.remaining_percent, 91);
    }

    /// 真实响应固化（2026-08-28 curl 实测原样；user/boosterWallet/authentication
    /// 等解析器不读字段已裁剪）——防下次 API 漂移的回归锚点
    #[test]
    fn parse_real_response_snapshot() {
        let json = r#"{
            "usage": {"limit": "100", "used": "23", "remaining": "77", "resetTime": "2026-09-04T00:18:45.334657Z"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99", "resetTime": "2026-08-28T22:18:45.334657Z"}}
            ],
            "totalQuota": {},
            "parallel": {"limit": "20"}
        }"#;
        let o = parse(json).unwrap();
        assert!(!o.frozen, "totalQuota 空对象 → 未冻结");
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.remaining_percent, 99);
        assert_eq!(
            w.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45.334657Z")
        );
        assert_eq!(w.seven_day.remaining_percent, 77);
        assert_eq!(
            w.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45.334657Z")
        );
    }

    // 注：fetch 真实 HTTP 查询登记 test-inventory 既定豁免（真实外部 API 依赖，
    // 解析纯函数已全量覆盖，fetch 仅薄封装）。
}
