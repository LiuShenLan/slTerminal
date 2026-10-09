//! 应用内通知卡片存活时长,独立于投递与审批通道。

use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationDuration {
    /// 不覆盖:保留各调用方原有存活时长(Default 是不覆盖而非具体秒数)。
    #[default]
    Default,
    FiveSeconds,
    TenSeconds,
    ThirtySeconds,
    NinetySeconds,
    Persistent,
}

impl NotificationDuration {
    pub const ALL: [Self; 6] = [
        Self::Default,
        Self::FiveSeconds,
        Self::TenSeconds,
        Self::ThirtySeconds,
        Self::NinetySeconds,
        Self::Persistent,
    ];
    pub const VALUES: [&'static str; 6] = ["default", "5", "10", "30", "90", "persistent"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim() {
            value if value.eq_ignore_ascii_case("default") => Some(Self::Default),
            "5" => Some(Self::FiveSeconds),
            "10" => Some(Self::TenSeconds),
            "30" => Some(Self::ThirtySeconds),
            "90" => Some(Self::NinetySeconds),
            value if value.eq_ignore_ascii_case("persistent") => Some(Self::Persistent),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::FiveSeconds => "5",
            Self::TenSeconds => "10",
            Self::ThirtySeconds => "30",
            Self::NinetySeconds => "90",
            Self::Persistent => "persistent",
        }
    }

    /// `None` 表示原本常驻(如更新操作卡),Default 档透传调用方原时长。
    pub fn timeout(self, default: Option<Duration>) -> Option<Duration> {
        match self {
            Self::Default => default,
            Self::FiveSeconds => Some(Duration::from_secs(5)),
            Self::TenSeconds => Some(Duration::from_secs(10)),
            Self::ThirtySeconds => Some(Duration::from_secs(30)),
            Self::NinetySeconds => Some(Duration::from_secs(90)),
            Self::Persistent => None,
        }
    }
}

#[cfg(test)]
mod notification_duration_tests {
    use super::*;

    #[test]
    fn every_duration_round_trips_and_timeout_semantics_hold() {
        for (duration, value) in NotificationDuration::ALL
            .into_iter()
            .zip(NotificationDuration::VALUES)
        {
            assert_eq!(duration.settings_value(), value);
            assert_eq!(NotificationDuration::from_settings(value), Some(duration));
        }
        for (value, seconds) in [("5", 5), ("10", 10), ("30", 30), ("90", 90)] {
            assert_eq!(
                NotificationDuration::from_settings(value)
                    .unwrap()
                    .timeout(None),
                Some(Duration::from_secs(seconds))
            );
        }
        assert_eq!(
            NotificationDuration::from_settings(" PERSISTENT ")
                .unwrap()
                .timeout(Some(Duration::from_secs(5))),
            None
        );
        // Default = 不覆盖:透传每种通知各自的原存活时长。
        for default in [
            Some(Duration::from_secs(5)),
            Some(Duration::from_secs(90)),
            None,
        ] {
            assert_eq!(NotificationDuration::Default.timeout(default), default);
        }
    }

    #[test]
    fn invalid_values_are_rejected_so_the_caller_falls_back_to_default() {
        for value in ["", "invalid", "-1", "0", "1", "999999999999999999999"] {
            assert_eq!(NotificationDuration::from_settings(value), None, "{value}");
        }
    }
}
