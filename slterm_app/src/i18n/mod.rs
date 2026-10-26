//! 界面语言查询 API:typed `Message` 是唯一入口,查表零分配。
//!
//! 生成物经 `include!` 进入本模块:运行时零解析、零文件读、零锁。
//! `text` 走表索引直取;`format` 是唯一允许创建 `String` 的翻译路径。
//! 新代码与既有代码都不得引入字符串键或英文文案反查通道。

mod format;
mod locale;

pub(crate) use locale::system_locale;

include!(concat!(env!("OUT_DIR"), "/translations.rs"));

impl LanguagePreference {
    /// 设置值(`system`/语言 code)→ 偏好;未知值 None。
    #[allow(dead_code)] // 消费点:合同测试与后续设置热应用路径。
    pub fn parse(value: &str) -> Option<Self> {
        slterm_settings::LanguagePref::from_settings(value).map(Self::from)
    }

    /// 写盘稳定值。
    #[allow(dead_code)] // 消费点:合同测试;设置页当前直接消费 LanguagePref。
    pub const fn as_str(self) -> &'static str {
        self.shared().settings_value()
    }

    /// 显式偏好直接解析;System 档经系统 locale 协商,解析不出回英文。
    pub fn resolved(self) -> UiLanguage {
        self.explicit()
            .unwrap_or_else(|| UiLanguage::for_locale(system_locale().as_deref()))
    }
}

impl UiLanguage {
    /// locale 字符串 → 渲染语言;未知/Nothing 回英文。
    pub fn for_locale(locale: Option<&str>) -> Self {
        locale
            .and_then(slterm_settings::LanguagePref::from_locale)
            .and_then(|preference| LanguagePreference::from(preference).explicit())
            .unwrap_or(Self::EnUs)
    }

    /// 表索引直取:零分配、零文件读、零锁。
    pub const fn text(self, message: Message) -> &'static str {
        MESSAGES[self as usize][message as usize]
    }

    /// 带参数翻译:唯一允许创建 `String` 的翻译路径。
    pub fn format(self, message: Message, args: &[(&str, &str)]) -> String {
        format::substitute(self.text(message), args)
    }
}

#[cfg(test)]
mod i18n_tests {
    use super::{LanguagePreference, Message, UiLanguage};

    /// 渲染枚举与设置注册表同源:数量一致、写盘值可往返。
    #[test]
    fn language_metadata_and_settings_values_remain_in_sync() {
        assert_eq!(
            UiLanguage::ALL.len(),
            slterm_settings::LanguagePref::LANGUAGES.len()
        );
        for preference in LanguagePreference::ALL {
            assert_eq!(
                LanguagePreference::parse(preference.as_str()),
                Some(*preference)
            );
            if let Some(language) = preference.explicit() {
                assert_eq!(language.code(), preference.as_str());
                assert_eq!(preference.resolved(), language);
            }
        }
    }

    /// System 档协商:POSIX 后缀/地区变体归主语言;未知回英文。
    #[test]
    fn negotiates_locale_and_falls_back_to_english() {
        assert_eq!(
            UiLanguage::for_locale(Some("zh_CN.UTF-8")),
            UiLanguage::ZhCn
        );
        assert_eq!(UiLanguage::for_locale(Some("zh-Hant-HK")), UiLanguage::ZhCn);
        assert_eq!(UiLanguage::for_locale(Some("en-GB")), UiLanguage::EnUs);
        assert_eq!(
            UiLanguage::for_locale(Some("unsupported")),
            UiLanguage::EnUs
        );
        assert_eq!(UiLanguage::for_locale(None), UiLanguage::EnUs);
    }

    /// typed 查询直接读取对应语言目录。
    #[test]
    fn typed_lookup_reads_each_language_catalog() {
        assert_eq!(
            UiLanguage::ZhCn.text(Message::SettingsSidebarAppearance),
            "外观"
        );
        assert_eq!(
            UiLanguage::EnUs.text(Message::SettingsSidebarAppearance),
            "Appearance"
        );
    }

    /// format 只替换命名占位符;参数值绝不二次解析为占位符。
    #[test]
    fn format_inserts_arguments_literally() {
        assert_eq!(
            UiLanguage::EnUs.format(Message::SettingsStatusResetBackup, &[("path", "C:\\data")]),
            "Defaults restored; previous settings backed up: C:\\data"
        );
        assert_eq!(
            UiLanguage::EnUs.format(
                Message::SettingsStatusOperationFailed,
                &[("error", "{path}")]
            ),
            "Settings operation failed: {path}"
        );
        assert_eq!(
            UiLanguage::ZhCn.format(
                Message::SettingsStatusOperationFailed,
                &[("error", "磁盘已满")]
            ),
            "设置操作失败：磁盘已满"
        );
        // 缺失参数原样可见。
        assert_eq!(
            UiLanguage::EnUs.format(Message::SettingsStatusOperationFailed, &[]),
            "Settings operation failed: {error}"
        );
    }
}
