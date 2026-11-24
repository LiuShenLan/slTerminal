//! DeepSeek/Kimi 套餐响应解析与展示值类型。
//!
//! 解析层只接受 `serde_json::Value`:不发现来源,不读取 CLI 配置或
//! 环境认证材料,不接触凭据、网络与调度。字段缺失或不可解析返回 Err,
//! 不把缺失数据伪造为 0;结果类型不承载 token。

pub(crate) mod deepseek;
pub(crate) mod kimi;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AmountInfo {
    /// 原样透传 total_balance。
    pub value: String,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowsInfo {
    pub five_hour: WindowInfo,
    pub seven_day: WindowInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowInfo {
    /// 已用百分比 = used/limit×100(used 缺失经 remaining 换算回退)。
    pub used_percent: u8,
    /// ISO 字符串,可缺失。
    pub resets_at: Option<String>,
}

/// 单来源套餐解析结果;来源身份、查询时间与错误状态由调用方持有。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanBalanceInfo {
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
}

/// 错过完整周期后才标记过期：从上次成功查询起算，一个周期前仍视为有效。
/// 时钟回拨不判过期，避免把未知时间误标为陈旧。
pub(crate) fn balance_stale(
    last_success: std::time::SystemTime,
    now: std::time::SystemTime,
    period: std::time::Duration,
) -> bool {
    match now.duration_since(last_success) {
        Ok(elapsed) => elapsed >= period,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::balance_stale;
    use std::time::{Duration, SystemTime};

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn plan_balance_stale_after_full_period() {
        let period = Duration::from_secs(60);
        assert!(
            !balance_stale(at(1_000), at(1_059), period),
            "未满一个周期不得标记过期"
        );
        assert!(
            balance_stale(at(1_000), at(1_060), period),
            "满一个完整周期必须标记过期"
        );
        assert!(
            balance_stale(at(1_000), at(2_000), period),
            "长间隔必须过期"
        );
        assert!(
            !balance_stale(at(1_000), at(900), period),
            "时钟回拨不得标记过期"
        );
    }
}
