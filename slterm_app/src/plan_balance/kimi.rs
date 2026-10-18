//! kimi 套餐(实测口径,2026-08 实证修正字段漂移;F10 语义原样):
//! GET /coding/v1/usages,双时间窗 + 配额耗尽冻结态。
//!
//! 实证红线(随迁):
//! - `Authorization: Bearer`(非 X-Kimi-Authorization——2026-08 实测修正);
//! - 5h 窗数值承载于 `limits[i].detail` 内层(外层无),7d 窗为顶层 `usage`;
//! - `remaining` 恒在、`used` 可缺;数值字段按字符串解析;
//! - 展示口径 = 已用百分比:used 优先、remaining 换算回退;
//! - `totalQuota` 无 used 字段(实测可为空对象);非冻结时任一窗口解析失败
//!   → 整体 Err(全有或全无,防窗口重置瞬间 5h 段丢失)。

use std::time::Duration;

use zeroize::Zeroizing;

use super::query::{PlanQuery, http_agent, query_err};
use super::{FetchOutcome, WindowInfo, WindowsInfo};
use crate::error::AppError;
use crate::secrets::SecretBytes;

#[derive(Debug)]
pub(crate) struct KimiQuery;

const TIMEOUT: Duration = Duration::from_secs(8);
const USAGES_ENDPOINT: &str = "https://api.kimi.com/coding/v1/usages";

impl PlanQuery for KimiQuery {
    fn plan_id(&self) -> &'static str {
        "kimi"
    }

    fn base_urls(&self) -> &'static [&'static str] {
        &["https://api.kimi.com/coding"]
    }

    fn fetch(&self, token: &SecretBytes) -> Result<FetchOutcome, AppError> {
        self.fetch_at(USAGES_ENDPOINT, TIMEOUT, token)
    }
}

impl KimiQuery {
    /// 显式 endpoint/timeout 形态:生产走常量,L1 用本地 HTTP 服务验证请求形态。
    fn fetch_at(
        &self,
        endpoint: &str,
        timeout: Duration,
        token: &SecretBytes,
    ) -> Result<FetchOutcome, AppError> {
        // 借出点:token 字节仅用于拼 Authorization;拼串本体 Zeroizing。
        let token_text = std::str::from_utf8(token.as_bytes())
            .map_err(|_| AppError::Unknown("kimi 查询失败: token 非 UTF-8".into()))?;
        let authorization = Zeroizing::new(format!("Bearer {token_text}"));
        let resp = http_agent(timeout)
            .get(endpoint)
            .header("Authorization", authorization.as_str())
            .call()
            .map_err(|error| query_err(self.plan_id(), error))?;
        let body: serde_json::Value = resp
            .into_body()
            .read_json()
            .map_err(|error| AppError::Unknown(format!("kimi 响应读取失败: {error}")))?;
        parse_kimi_usages(&body)
    }
}

/// 已用百分比(用户偏好展示已用量):used 优先(used/limit×100),
/// remaining 换算回退((limit-remaining)/limit×100);数值字段为字符串(实证口径),
/// limit 缺失/为 0/不可解析 → None(该窗口失败)。
/// 实证(2026-08 实测 + 社区审计):remaining 恒在、used 可缺——used 优先下,
/// remaining 恒在保证换算回退必成功;used 不可解析等同缺失(落换算回退)。
fn used_percent(used: Option<&str>, remaining: Option<&str>, limit: Option<&str>) -> Option<u8> {
    let limit: f64 = limit?.parse().ok()?;
    if limit <= 0.0 {
        return None;
    }
    if let Some(used) = used.and_then(|value| value.parse::<f64>().ok()) {
        return Some((used / limit * 100.0).round().clamp(0.0, 100.0) as u8);
    }
    let remaining: f64 = remaining?.parse().ok()?;
    Some(
        ((limit - remaining) / limit * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8,
    )
}

/// 单窗口解析:detail 内 used/limit/remaining/resetTime 直接取值(实证:5h 窗
/// 数值承载于 limits[i].detail 内层,7d 窗为顶层 usage——两者均直接含 resetTime)。
fn parse_window(detail: &serde_json::Value) -> Result<WindowInfo, AppError> {
    let percent = used_percent(
        detail.get("used").and_then(|value| value.as_str()),
        detail.get("remaining").and_then(|value| value.as_str()),
        detail.get("limit").and_then(|value| value.as_str()),
    )
    .ok_or_else(|| AppError::Unknown("kimi 窗口 used/remaining/limit 解析失败".into()))?;
    let resets_at = detail
        .get("resetTime")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    Ok(WindowInfo {
        used_percent: percent,
        resets_at,
    })
}

/// 响应解析(纯函数,罐装 JSON 可测;全有或全无):
/// 1. frozen(实证口径):totalQuota.remaining 数值 ≤ 0 → 配额耗尽冻结
///    (不要求窗口解析成功,windows=None);
/// 2. 5h 窗:limits[] 中 window.duration==300 && window.timeUnit=="TIME_UNIT_MINUTE"
///    优先,否则 limits[0];limits 空 → 失败;数值读取 limits[i].detail 内层;
/// 3. 7d 窗:顶层 usage 字段;
/// 4. 非触顶时任一窗口失败 → 整体 Err。
pub(crate) fn parse_kimi_usages(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
    // totalQuota 缺失/为空对象(实测形态)/remaining 非数字或非 0 → 未冻结
    let frozen = body
        .get("totalQuota")
        .and_then(|quota| quota.get("remaining"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|remaining| remaining <= 0.0);
    if frozen {
        return Ok(FetchOutcome {
            frozen: true,
            amount: None,
            windows: None,
        });
    }
    let limits = body
        .get("limits")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Unknown("kimi 响应缺 limits".into()))?;
    let five_hour_entry = limits
        .iter()
        .find(|limit| {
            limit
                .get("window")
                .and_then(|window| window.get("duration"))
                .and_then(|duration| duration.as_u64())
                == Some(300)
                && limit
                    .get("window")
                    .and_then(|window| window.get("timeUnit"))
                    .and_then(|unit| unit.as_str())
                    == Some("TIME_UNIT_MINUTE")
        })
        .or_else(|| limits.first())
        .ok_or_else(|| AppError::Unknown("kimi 响应 limits 为空".into()))?;
    // 数值承载于 detail 内层(实证:limits[i].detail 含 limit/used/remaining/resetTime)
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
    use crate::plan_balance::query::test_support::{request_contains, serve_once};

    fn parse(json: &str) -> Result<FetchOutcome, AppError> {
        parse_kimi_usages(&serde_json::from_str(json).expect("合法 JSON 夹具"))
    }

    fn test_token() -> SecretBytes {
        SecretBytes::new(b"sk-test-kimi".to_vec())
    }

    /// 双窗正常(真实响应数字,已用口径):5h 1%(used 1/100)、7d 23%(used 23/100)。
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
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        let windows = outcome.windows.expect("双窗应存在");
        assert_eq!(windows.five_hour.used_percent, 1, "5h 窗 used 直读:1/100");
        assert_eq!(
            windows.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45Z")
        );
        assert_eq!(windows.seven_day.used_percent, 23, "7d 窗 used 直读:23/100");
        assert_eq!(
            windows.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45Z")
        );
    }

    /// used 与 remaining 并存且不一致 → used 直读(矛盾夹具即防回归探针:
    /// 若改回 remaining 优先,本用例立即失败)。
    #[test]
    fn parse_used_preferred_over_remaining() {
        let json = r#"{
            "usage": {"limit": "100", "used": "99", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "99", "remaining": "50"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(
            windows.five_hour.used_percent, 99,
            "应取 used 直读而非 remaining"
        );
        assert_eq!(windows.seven_day.used_percent, 99);
    }

    /// remaining 缺失无碍 → used 直读主路径(detail 仅 {limit, used})。
    #[test]
    fn parse_used_direct_remaining_missing() {
        let json = r#"{
            "usage": {"limit": "100", "used": "25"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 1, "used 直读 1/100");
        assert_eq!(windows.seven_day.used_percent, 25, "used 直读 25/100");
    }

    /// remaining 垃圾值被忽略 → used 直读成功(1/2)。
    #[test]
    fn parse_used_wins_over_unparsable_remaining() {
        let json = r#"{
            "usage": {"limit": "2", "used": "1", "remaining": "abc"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "2", "used": "1", "remaining": "abc"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(
            windows.five_hour.used_percent, 50,
            "used 直读 1/2,remaining 不可解析被忽略"
        );
    }

    /// used 与 remaining 均缺失 → 窗口失败 → 整体 Err。
    #[test]
    fn parse_missing_used_and_remaining_errors() {
        let json = r#"{
            "usage": {"limit": "100"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100"}}
            ]
        }"#;
        let error = parse(json).expect_err("窗口应失败");
        assert!(error.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// 300min 优先:数组含 60min 与 300min 两条 → 选 300min。
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
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(
            windows.five_hour.used_percent, 1,
            "应命中 300min 窗口而非 limits[0],used 直读 1/100"
        );
    }

    /// 无 300min 窗口 → 回退 limits[0](used 直读 1/2)。
    #[test]
    fn parse_falls_back_to_limits_first() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 60, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "2", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(
            windows.five_hour.used_percent, 50,
            "应回退 limits[0]:used 直读 1/2"
        );
    }

    /// limits 空数组 → 整体 Err。
    #[test]
    fn parse_empty_limits_errors() {
        let json = r#"{"totalQuota": {"remaining": "99"}, "limits": [], "usage": {}}"#;
        let error = parse(json).expect_err("空 limits 应失败");
        assert!(error.to_string().contains("limits 为空"));
    }

    /// 窗口条目缺 detail(结构再漂移)→ 整体 Err。
    #[test]
    fn parse_missing_detail_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"}}
            ]
        }"#;
        let error = parse(json).expect_err("缺 detail 应失败");
        assert!(error.to_string().contains("缺 detail"));
    }

    /// 5h limit="0" → 该窗口解析失败 → 整体 Err(防 5h 段丢失)。
    #[test]
    fn parse_zero_limit_five_hour_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "0", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let error = parse(json).expect_err("零 limit 应失败");
        assert!(error.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// 5h limit 缺失 → 整体 Err。
    #[test]
    fn parse_missing_limit_five_hour_errors() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"used": "1", "remaining": "1"}}
            ]
        }"#;
        let error = parse(json).expect_err("缺 limit 应失败");
        assert!(error.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// 7d limit 缺失 → 整体 Err。
    #[test]
    fn parse_missing_seven_day_limit_errors() {
        let json = r#"{
            "usage": {"remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "1"}}
            ]
        }"#;
        let error = parse(json).expect_err("7d 缺 limit 应失败");
        assert!(error.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// 冻结:totalQuota.remaining="0" → frozen=true,窗口缺失仍 Ok。
    #[test]
    fn parse_frozen_when_total_quota_remaining_zero() {
        let json = r#"{"totalQuota": {"limit": "100", "remaining": "0"}}"#;
        let outcome = parse(json).expect("冻结态应成功");
        assert!(outcome.frozen);
        assert!(outcome.amount.is_none());
        assert!(outcome.windows.is_none());
    }

    /// remaining 为小数正残值 → 未冻结且窗口正常解析。
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
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        assert!(outcome.windows.is_some());
    }

    /// totalQuota 缺失 → 未冻结且窗口正常解析。
    #[test]
    fn parse_not_frozen_when_total_quota_missing() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "50"}}
            ]
        }"#;
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        assert!(outcome.windows.is_some());
    }

    /// totalQuota 为空对象(2026-08 实测形态)→ 未冻结且窗口正常解析。
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
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        assert!(outcome.windows.is_some(), "空对象不应中断窗口解析");
    }

    /// remaining 非数字串 → 未冻结(parse 失败回落 false)且窗口正常解析。
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
        let outcome = parse(json).expect("应解析成功");
        assert!(!outcome.frozen);
        assert!(outcome.windows.is_some());
    }

    /// 冻结时不要求窗口解析成功(windows=None 仍 Ok)。
    #[test]
    fn parse_frozen_true_even_without_windows() {
        let json = r#"{"totalQuota": {"limit": "10", "remaining": "0"}}"#;
        let outcome = parse(json).expect("冻结态应成功");
        assert!(outcome.frozen);
        assert!(outcome.amount.is_none());
        assert!(outcome.windows.is_none());
    }

    /// 非冻结但 usage 缺失 → 整体 Err。
    #[test]
    fn parse_missing_usage_errors() {
        let json = r#"{
            "totalQuota": {"remaining": "99"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let error = parse(json).expect_err("缺 usage 应失败");
        assert!(error.to_string().contains("缺 usage"));
    }

    /// resetTime 缺失 → resets_at=None(窗口值保留,仅省略重置段)。
    #[test]
    fn parse_missing_reset_time_yields_none() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "50"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 1);
        assert!(windows.five_hour.resets_at.is_none());
        assert!(windows.seven_day.resets_at.is_none());
    }

    /// used>limit(无 remaining)→ 已用 6000/5000=120% → clamp 100。
    #[test]
    fn parse_used_over_limit_clamps_hundred() {
        let json = r#"{
            "usage": {"limit": "5000", "used": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "used": "6000"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 100);
    }

    /// remaining>limit(不一致场景,used 缺)→ 换算 (5000-6000)/5000=-20% → clamp 0。
    #[test]
    fn parse_remaining_over_limit_fallback_clamps_zero() {
        let json = r#"{
            "usage": {"limit": "5000", "remaining": "6000"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "5000", "remaining": "6000"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 0, "换算差为负 → clamp 0");
    }

    /// 实测形态:detail 无 used 仅 {limit, remaining, resetTime} → remaining 换算回退
    /// ((100-91)/100=9%——91 与 9 差异显著,防「直接拿 remaining 当结果」回归)。
    #[test]
    fn parse_detail_without_used_falls_back_remaining_convert() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "91", "resetTime": "2026-02-25T04:01:38Z"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 9);
        assert_eq!(windows.seven_day.used_percent, 9);
    }

    /// 真实响应固化(2026-08-28 curl 实测原样;解析器不读字段已裁剪)——
    /// 防下次 API 漂移的回归锚点;已用口径:双含 used+remaining → used 直读路径。
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
        let outcome = parse(json).expect("真实响应应解析成功");
        assert!(!outcome.frozen, "totalQuota 空对象 → 未冻结");
        let windows = outcome.windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 1);
        assert_eq!(
            windows.five_hour.resets_at.as_deref(),
            Some("2026-08-28T22:18:45.334657Z")
        );
        assert_eq!(windows.seven_day.used_percent, 23);
        assert_eq!(
            windows.seven_day.resets_at.as_deref(),
            Some("2026-09-04T00:18:45.334657Z")
        );
    }

    /// used 不可解析("abc")等同缺失 → remaining 换算回退 ((100-80)/100=20%)。
    #[test]
    fn parse_used_non_numeric_falls_back_remaining() {
        let json = r#"{
            "usage": {"limit": "100", "used": "abc", "remaining": "80"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "abc", "remaining": "80"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 20, "换算回退 20/100");
        assert_eq!(windows.seven_day.used_percent, 20, "换算回退 20/100");
    }

    /// 双窗形态各一(5h detail 无 used 走换算 9、7d usage used 直读 23)——
    /// 双窗独立取数互不污染。
    #[test]
    fn parse_mixed_window_used_absent_one_side() {
        let json = r#"{
            "usage": {"limit": "100", "used": "23", "remaining": "77"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "91"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(
            windows.five_hour.used_percent, 9,
            "5h used 缺 → 换算 (100-91)/100"
        );
        assert_eq!(windows.seven_day.used_percent, 23, "7d used 直读 23/100");
    }

    /// limit 非数字串(used/remaining 均在)→ 该窗失败 → 整体 Err。
    #[test]
    fn parse_limit_non_numeric_errors() {
        let json = r#"{
            "usage": {"limit": "abc", "used": "5", "remaining": "95"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "used": "1", "remaining": "99"}}
            ]
        }"#;
        let error = parse(json).expect_err("非数字 limit 应失败");
        assert!(error.to_string().contains("used/remaining/limit 解析失败"));
    }

    /// remaining 为负(used 缺)→ 换算 (100-(-5))/100=105% → clamp 100。
    #[test]
    fn parse_remaining_negative_clamps_hundred() {
        let json = r#"{
            "usage": {"limit": "100", "remaining": "-5"},
            "limits": [
                {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                 "detail": {"limit": "100", "remaining": "-5"}}
            ]
        }"#;
        let windows = parse(json).expect("应解析成功").windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 100, "105% → clamp 100");
    }

    // ── 本地 HTTP 服务(请求形态验证,不出网) ──

    /// GET /coding/v1/usages + Authorization Bearer(非 X-Kimi-Authorization)+ 双窗解析。
    #[test]
    fn fetch_uses_bearer_header_and_parses_usages() {
        let (base, server) = serve_once(
            "HTTP/1.1 200 OK",
            r#"{
                "usage": {"limit": "100", "used": "23", "remaining": "77"},
                "limits": [
                    {"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                     "detail": {"limit": "100", "used": "1", "remaining": "99"}}
                ],
                "totalQuota": {}
            }"#,
        );
        let outcome = KimiQuery
            .fetch_at(&format!("{base}/coding/v1/usages"), TIMEOUT, &test_token())
            .expect("本地查询应成功");
        let windows = outcome.windows.expect("双窗");
        assert_eq!(windows.five_hour.used_percent, 1);
        assert_eq!(windows.seven_day.used_percent, 23);

        let request = server.join().expect("本地服务线程");
        assert!(request_contains(&request, "GET /coding/v1/usages HTTP/1.1"));
        assert!(
            request_contains(&request, "authorization: bearer sk-test-kimi"),
            "Authorization 头缺失或形态错误: {request}"
        );
        assert!(
            !request_contains(&request, "x-kimi-authorization"),
            "不得使用已实证废弃的 X-Kimi-Authorization 头"
        );
    }

    /// 响应体不可解析 → 读取错误,消息不含 token。
    #[test]
    fn fetch_maps_unreadable_body_without_leaking_token() {
        let (base, server) = serve_once("HTTP/1.1 200 OK", "not json");
        let error = KimiQuery
            .fetch_at(&format!("{base}/coding/v1/usages"), TIMEOUT, &test_token())
            .expect_err("坏响应体应失败");
        let _ = server.join();
        let message = error.to_string();
        assert!(message.contains("响应读取失败"), "实际: {message}");
        assert!(
            !message.contains("sk-test-kimi"),
            "错误消息泄露 token: {message}"
        );
    }

    /// token 非 UTF-8 → 联网前拒绝。
    #[test]
    fn fetch_rejects_non_utf8_token_before_network() {
        let token = SecretBytes::new(vec![0xff, 0xfe]);
        let error = KimiQuery
            .fetch_at(
                "http://127.0.0.1:9/unreachable",
                Duration::from_millis(50),
                &token,
            )
            .expect_err("非 UTF-8 token 应失败");
        assert!(error.to_string().contains("token 非 UTF-8"));
    }
}
