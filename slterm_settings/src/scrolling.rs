//! 回滚行数与滚速:档位白名单 + 归一钳制。
//!
//! 「档位约束 + 归一钳制」是数值键的通用模式:白名单外的手改值一律回
//! 默认,防手改出 1 行或一千万行的文件炸终端。

/// 新建终端的可选历史容量;已有会话不受该键影响。
pub const SCROLLBACK_VALUES: [usize; 7] = [1000, 2000, 5000, 10000, 20000, 50000, 100000];
/// 默认随上游一万行(实测后再经设置键评估)。
pub const DEFAULT_SCROLLBACK_LINES: usize = 10_000;
pub const DEFAULT_SCROLL_SPEED: f32 = 1.0;
pub const MIN_SCROLL_SPEED: f32 = 0.25;
pub const MAX_SCROLL_SPEED: f32 = 4.0;
pub const SCROLL_SPEED_STEP: f32 = 0.25;

/// 档位外一律回默认。
pub fn scrollback_lines(raw: Option<usize>) -> usize {
    raw.filter(|value| SCROLLBACK_VALUES.contains(value))
        .unwrap_or(DEFAULT_SCROLLBACK_LINES)
}

/// 非有限/越界 → 默认 1.0;界内钳到 [0.25, 4.0]。
pub fn normalize_scroll_speed(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_SCROLL_SPEED, MAX_SCROLL_SPEED)
    } else {
        DEFAULT_SCROLL_SPEED
    }
}

#[cfg(test)]
mod scrolling_tests {
    use super::*;

    #[test]
    fn whitelist_boundaries_accept_exactly_the_seven_steps() {
        assert_eq!(SCROLLBACK_VALUES.len(), 7);
        for value in SCROLLBACK_VALUES {
            assert_eq!(scrollback_lines(Some(value)), value);
        }
        // 档外与缺键一律回默认。
        for value in [
            None,
            Some(0),
            Some(999),
            Some(1001),
            Some(200_000),
            Some(usize::MAX),
        ] {
            assert_eq!(
                scrollback_lines(value),
                DEFAULT_SCROLLBACK_LINES,
                "{value:?}"
            );
        }
    }

    #[test]
    fn scroll_speed_clamps_and_rejects_nonfinite_values() {
        assert_eq!(normalize_scroll_speed(0.25), 0.25);
        assert_eq!(normalize_scroll_speed(1.75), 1.75);
        assert_eq!(normalize_scroll_speed(4.0), 4.0);
        assert_eq!(normalize_scroll_speed(-1.0), MIN_SCROLL_SPEED);
        assert_eq!(normalize_scroll_speed(99.0), MAX_SCROLL_SPEED);
        assert_eq!(normalize_scroll_speed(f32::NAN), DEFAULT_SCROLL_SPEED);
        assert_eq!(normalize_scroll_speed(f32::INFINITY), DEFAULT_SCROLL_SPEED);
    }
}
