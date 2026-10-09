//! 终端连字偏好与主题优先序:On/Off/Theme 三态,Theme 档把决定权交给
//! 主题 typography 声明。默认开(偏安全侧),无效值回默认。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ligatures {
    #[default]
    On,
    Off,
    Theme,
}

impl Ligatures {
    pub const ALL: [Self; 3] = [Self::On, Self::Off, Self::Theme];
    pub const VALUES: [&'static str; 3] = ["on", "off", "theme"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim() {
            value if value.eq_ignore_ascii_case("on") => Some(Self::On),
            value if value.eq_ignore_ascii_case("off") => Some(Self::Off),
            value if value.eq_ignore_ascii_case("theme") => Some(Self::Theme),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Theme => "theme",
        }
    }

    /// Theme 档缺主题声明时按「开」处理。
    pub fn enabled(self, theme: Option<bool>) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Theme => theme.unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod ligatures_tests {
    use super::*;

    #[test]
    fn preference_round_trips_and_only_theme_mode_defers_to_the_theme() {
        for preference in Ligatures::ALL {
            assert_eq!(
                Ligatures::from_settings(preference.settings_value()),
                Some(preference)
            );
        }
        for theme in [None, Some(false), Some(true)] {
            assert!(Ligatures::On.enabled(theme));
            assert!(!Ligatures::Off.enabled(theme));
            assert_eq!(Ligatures::Theme.enabled(theme), theme.unwrap_or(true));
        }
    }

    #[test]
    fn invalid_values_are_rejected_so_the_caller_keeps_the_on_default() {
        for value in ["", "invalid", "1", "true"] {
            assert_eq!(Ligatures::from_settings(value), None, "{value}");
        }
    }
}
