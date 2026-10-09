//! 语言注册表:单行注册一种语言,选择器/枚举/查找表同源零分叉。
//!
//! 语言集裁为 zh-CN / en-US 两家(其余九语整删)。`from_locale` 的
//! POSIX 后缀剥离与「地区变体回退主语言」语义保留;上游对 zh 的
//! Hans/Hant/TW/HK/MO 特判在裁掉繁体变体后无对象——zh 家族一律回退到
//! 唯一的中文变体 zh-CN(主语言匹配),与 en-GB → en-US 同一条规则。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageInfo {
    pub preference: LanguagePref,
    pub code: &'static str,
    pub native_name: &'static str,
    pub component_locale: &'static str,
    pub rust_variant: &'static str,
}

macro_rules! languages {
    ($( $variant:ident => ($code:literal, $native:literal, $component:literal) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub enum LanguagePref {
            #[default]
            System,
            $( $variant, )+
        }

        impl LanguagePref {
            pub const ALL: &'static [Self] = &[Self::System, $( Self::$variant, )+];
            pub const VALUES: &'static [&'static str] = &["system", $( $code, )+];
            pub const LANGUAGES: &'static [LanguageInfo] = &[
                $( LanguageInfo {
                    preference: Self::$variant,
                    code: $code,
                    native_name: $native,
                    component_locale: $component,
                    rust_variant: stringify!($variant),
                }, )+
            ];

            pub fn from_settings(value: &str) -> Option<Self> {
                match value.trim() {
                    "system" => Some(Self::System),
                    $( $code => Some(Self::$variant), )+
                    _ => None,
                }
            }

            pub const fn settings_value(self) -> &'static str {
                match self {
                    Self::System => "system",
                    $( Self::$variant => $code, )+
                }
            }

            pub const fn native_name(self) -> &'static str {
                match self {
                    Self::System => "System",
                    $( Self::$variant => $native, )+
                }
            }
        }
    };
}

languages! {
    ZhCn => ("zh-CN", "简体中文", "zh-CN"),
    EnUs => ("en-US", "English", "en"),
}

impl LanguagePref {
    /// 系统 locale → 语言偏好协商:剥离 POSIX 后缀(`.UTF-8`/`@variant`),
    /// 主语言匹配,地区变体回退主语言(en-GB → en-US、zh-Hant → zh-CN)。
    pub fn from_locale(locale: &str) -> Option<Self> {
        let locale = locale.trim().split(['.', '@']).next()?;
        let primary = locale.split(['-', '_']).next()?;
        Self::LANGUAGES
            .iter()
            .find(|info| {
                info.code
                    .split('-')
                    .next()
                    .is_some_and(|base| base.eq_ignore_ascii_case(primary))
            })
            .map(|info| info.preference)
    }
}

#[cfg(test)]
mod language_tests {
    use super::LanguagePref;

    #[test]
    fn every_language_round_trips_without_changing_saved_values() {
        assert_eq!(LanguagePref::VALUES, &["system", "zh-CN", "en-US"]);
        for (preference, value) in LanguagePref::ALL.iter().zip(LanguagePref::VALUES) {
            assert_eq!(preference.settings_value(), *value);
            assert_eq!(LanguagePref::from_settings(value), Some(*preference));
            assert!(!preference.native_name().is_empty());
        }
        assert_eq!(
            LanguagePref::from_settings("zh"),
            None,
            "设置值是完整 code,主语言不算"
        );
        assert_eq!(LanguagePref::from_settings("unknown"), None);
    }

    #[test]
    fn system_locale_negotiates_regions_scripts_and_posix_suffixes() {
        for (locale, expected) in [
            ("zh_CN.UTF-8", LanguagePref::ZhCn),
            ("zh-Hans-TW", LanguagePref::ZhCn),
            ("zh-Hant", LanguagePref::ZhCn),
            ("zh_HK", LanguagePref::ZhCn),
            ("zh-TW", LanguagePref::ZhCn),
            ("en-GB", LanguagePref::EnUs),
            ("en_US.UTF-8", LanguagePref::EnUs),
            ("en", LanguagePref::EnUs),
        ] {
            assert_eq!(
                LanguagePref::from_locale(locale),
                Some(expected),
                "{locale}"
            );
        }
        for locale in ["C.UTF-8", "unknown", "fr-FR", "ja_JP.UTF-8"] {
            assert_eq!(LanguagePref::from_locale(locale), None, "{locale}");
        }
    }
}
