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

/// 已用百分比（2026-09 起，用户偏好展示已用量）：used 优先（used/limit×100），
/// remaining 换算回退（(limit-remaining)/limit×100）；数值字段为字符串（实证口径），
/// limit 缺失/为 0/不可解析 → None（该窗口失败）。
/// 实证（2026-08 实测 + 社区审计）：remaining 恒在、used 可缺——used 优先下，
/// remaining 恒在保证换算回退必成功；used 不可解析等同缺失（落换算回退）。
fn used_percent(used: Option<&str>, remaining: Option<&str>, limit: Option<&str>) -> Option<u8> {
    let limit: f64 = limit?.parse().ok()?;
    if limit <= 0.0 {
        return None;
    }
    if let Some(u) = used.and_then(|v| v.parse::<f64>().ok()) {
        return Some((u / limit * 100.0).round().clamp(0.0, 100.0) as u8);
    }
    let remaining: f64 = remaining?.parse().ok()?;
    Some(
        ((limit - remaining) / limit * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8,
    )
}

/// 单窗口解析：detail 内 used/limit/remaining/resetTime 直接取值（实证：5h 窗
/// 数值承载于 limits[i].detail 内层，7d 窗为顶层 usage——两者均直接含 resetTime）
fn parse_window(detail: &serde_json::Value) -> Result<WindowInfo, AppError> {
    let percent = used_percent(
        detail.get("used").and_then(|v| v.as_str()),
        detail.get("remaining").and_then(|v| v.as_str()),
        detail.get("limit").and_then(|v| v.as_str()),
    )
    .ok_or_else(|| AppError::Unknown("kimi 窗口 used/remaining/limit 解析失败".into()))?;
    let resets_at = detail
        .get("resetTime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(WindowInfo {
        used_percent: percent,
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

    /// 双窗正常（真实响应数字，2026-09 起已用口径）：5h 1%（used 1/100）、
    /// 7d 23%（used 23/100），resetTime 透传
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
        assert_eq!(w.five_hour.used_percent, 1, "5h 窗 used 直读：1/100");
        assert_eq!(
            w.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45Z")
        );
        assert_eq!(w.seven_day.used_percent, 23, "7d 窗 used 直读：23/100");
        assert_eq!(
            w.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45Z")
        );
    }

    /// used 与 remaining 并存且不一致 → used 直读（used 99 / remaining 50——
    /// 矛盾夹具即防回归探针：若改回 remaining 优先，本用例立即失败）
    #[test]
    fn parse_used_preferred_over_remaining() {
        let json = r#"{
            "usage": {"limit": "100", "used": "99", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "99", "remaining": "50"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 99, "应取 used 直读而非 remaining");
        assert_eq!(w.seven_day.used_percent, 99);
    }

    /// remaining 缺失无碍 → used 直读主路径（detail 仅 {limit, used}）
    #[test]
    fn parse_used_direct_remaining_missing() {
        let json = r#"{
            "usage": {"limit": "100", "used": "25"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 1, "used 直读 1/100");
        assert_eq!(w.seven_day.used_percent, 25, "used 直读 25/100");
    }

    /// remaining 垃圾值被忽略 → used 直读成功（1/2）
    #[test]
    fn parse_used_wins_over_unparsable_remaining() {
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
            w.five_hour.used_percent, 50,
            "used 直读 1/2，remaining 不可解析被忽略"
        );
    }

    /// used 与 remaining 均缺失 → 窗口失败 → 整体 Err
    #[test]
    fn parse_missing_used_and_remaining_errors() {
        let json = r#"{
            "usage": {"limit": "100"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// 300min 优先：数组含 60min 与 300min 两条 → 选 300min（窗口选择规则不变，
    /// 命中后 used 直读 1/100）
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
            w.five_hour.used_percent, 1,
            "应命中 300min 窗口而非 limits[0]，used 直读 1/100"
        );
    }

    /// 无 300min 窗口 → 回退 limits[0]（used 直读 1/2）
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
        assert_eq!(
            w.five_hour.used_percent, 50,
            "应回退 limits[0]：used 直读 1/2"
        );
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
        assert!(err.to_string().contains("used/remaining/limit 解析失败"));
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
        assert!(err.to_string().contains("used/remaining/limit 解析失败"));
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
        assert!(err.to_string().contains("used/remaining/limit 解析失败"));
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

    /// resetTime 缺失 → resets_at=None（窗口值保留，仅省略重置段；
    /// 5h 走 used 直读 1/100，7d 无 used 走 remaining 换算 50/100）
    #[test]
    fn parse_missing_reset_time_yields_none() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 1);
        assert!(w.five_hour.resets_at.is_none());
        assert!(w.seven_day.resets_at.is_none());
    }

    /// used>limit（无 remaining）→ 已用 6000/5000=120% → clamp 100
    #[test]
    fn parse_used_over_limit_clamps_hundred() {
        let json = r#"{
            "usage": {"limit": "5000", "used": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "used": "6000"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 100);
    }

    /// remaining>limit（不一致场景，used 缺）→ 换算 (5000-6000)/5000=-20% → clamp 0
    #[test]
    fn parse_remaining_over_limit_fallback_clamps_zero() {
        let json = r#"{
            "usage": {"limit": "5000", "remaining": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "remaining": "6000"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 0, "换算差为负 → clamp 0");
    }

    /// 实测形态：detail 无 used 仅 {limit, remaining, resetTime} → remaining 换算回退
    /// ((100-91)/100=9%——91 与 9 差异显著，防"直接拿 remaining 当结果"回归)
    #[test]
    fn parse_detail_without_used_falls_back_remaining_convert() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 9);
        assert_eq!(w.seven_day.used_percent, 9);
    }

    /// 真实响应固化（2026-08-28 curl 实测原样；user/boosterWallet/authentication
    /// 等解析器不读字段已裁剪）——防下次 API 漂移的回归锚点；2026-09 起口径：
    /// 快照双含 used+remaining → 锚定 used 直读路径（5h 1/100、7d 23/100）
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
        assert_eq!(w.five_hour.used_percent, 1);
        assert_eq!(
            w.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45.334657Z")
        );
        assert_eq!(w.seven_day.used_percent, 23);
        assert_eq!(
            w.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45.334657Z")
        );
    }

    /// used 不可解析（"abc"）等同缺失 → remaining 换算回退 ((100-80)/100=20%)
    #[test]
    fn parse_used_non_numeric_falls_back_remaining() {
        let json = r#"{
            "usage": {"limit": "100", "used": "abc", "remaining": "80"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "abc", "remaining": "80"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 20, "换算回退 20/100");
        assert_eq!(w.seven_day.used_percent, 20, "换算回退 20/100");
    }

    /// 双窗形态各一（5h detail 无 used 走换算 9、7d usage used 直读 23）——
    /// 双窗独立取数互不污染
    #[test]
    fn parse_mixed_window_used_absent_one_side() {
        let json = r#"{
            "usage": {"limit": "100", "used": "23", "remaining": "77"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "91"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(
            w.five_hour.used_percent, 9,
            "5h used 缺 → 换算 (100-91)/100"
        );
        assert_eq!(w.seven_day.used_percent, 23, "7d used 直读 23/100");
    }

    /// limit 非数字串（used/remaining 均在）→ 该窗失败 → 整体 Err
    #[test]
    fn parse_limit_non_numeric_errors() {
        let json = r#"{
            "usage": {"limit": "abc", "used": "5", "remaining": "95"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let err = parse(json).unwrap_err();
        assert!(err.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// remaining 为负（used 缺）→ 换算 (100-(-5))/100=105% → clamp 100
    #[test]
    fn parse_remaining_negative_clamps_hundred() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "-5"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "-5"}}
            ]
        }"#;
        let o = parse(json).unwrap();
        let w = o.windows.unwrap();
        assert_eq!(w.five_hour.used_percent, 100, "105% → clamp 100");
    }

    // 注：fetch 真实 HTTP 查询登记 test-exemptions 既定豁免（真实外部 API 依赖，
    // 解析纯函数已全量覆盖，fetch 仅薄封装）。
}
