//! 终端光标平滑移动:纯表现层动画,显式 opt-in(默认关),无效值回默认。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorMotion {
    #[default]
    Off,
    Smooth,
}

impl CursorMotion {
    pub const ALL: [Self; 2] = [Self::Off, Self::Smooth];
    pub const VALUES: [&'static str; 2] = ["off", "smooth"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim() {
            value if value.eq_ignore_ascii_case("off") => Some(Self::Off),
            value if value.eq_ignore_ascii_case("smooth") => Some(Self::Smooth),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Smooth => "smooth",
        }
    }
}

#[cfg(test)]
mod cursor_motion_tests {
    use super::*;

    #[test]
    fn cursor_motion_is_opt_in_and_rejects_legacy_boolean_spellings() {
        // JSON 世界布尔拼写不是合法值:类型/拼写不符一律 None → 默认关。
        for value in ["", "true", "1", "90", "unknown"] {
            assert_eq!(CursorMotion::from_settings(value), None, "{value}");
        }
        assert_eq!(
            CursorMotion::from_settings(" SMOOTH "),
            Some(CursorMotion::Smooth)
        );
        for value in CursorMotion::VALUES {
            assert_eq!(
                CursorMotion::from_settings(value).unwrap().settings_value(),
                value
            );
        }
    }
}
