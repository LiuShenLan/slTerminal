//! 界面语言查询 API(06 篇):typed `Message` 是唯一入口,查表零分配。
//!
//! 生成物经 `include!` 进入本模块:运行时零解析、零文件读、零锁。
//! `text` 走表索引直取;`pick` 是中英即时分支的渐进迁移桥;`tr` 是字符串
//! 键兼容层;`format` 是唯一允许创建 `String` 的翻译路径。
//! 桥不是继续堆双语硬编码的理由——新代码一律 `text(Message::…)`。

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

    /// 旧双语硬编码迁移桥:中英即时分支;未来新增语言经 `source_id` 反查,
    /// 歧义英文短语不生成反查项,无命中直用原文。当前两语均走即时分支。
    #[allow(dead_code)] // 消费点:合同测试;壳内存量双语已收编,桥保留不扩大。
    pub fn pick<'text>(self, zh_cn: &'text str, en_us: &'text str) -> &'text str {
        if self == Self::ZhCn {
            return zh_cn;
        }
        if self == Self::EnUs {
            return en_us;
        }
        source_id(en_us)
            .map(|message| self.text(message))
            .unwrap_or(en_us)
    }

    /// 字符串键兼容层;未知键原样返回(便于逐步迁移)。
    #[allow(dead_code)] // 消费点:合同测试;新代码一律 typed `text`。
    pub fn tr(self, key: &'static str) -> &'static str {
        message_id(key)
            .map(|message| self.text(message))
            .unwrap_or(key)
    }

    /// 带参数翻译:唯一允许创建 `String` 的翻译路径。
    pub fn format(self, message: Message, args: &[(&str, &str)]) -> String {
        format::substitute(self.text(message), args)
    }

    /// 字符串键 + 参数(兼容层)。
    #[allow(dead_code)] // 消费点:合同测试;新代码一律 typed `format`。
    pub fn tr_args(self, key: &'static str, args: &[(&str, &str)]) -> String {
        format::substitute(self.tr(key), args)
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

    /// typed 查询与字符串键兼容层读到同一份表。
    #[test]
    fn typed_and_compatibility_lookups_agree() {
        assert_eq!(
            UiLanguage::ZhCn.text(Message::SettingsSidebarAppearance),
            "外观"
        );
        assert_eq!(
            UiLanguage::EnUs.text(Message::SettingsSidebarAppearance),
            "Appearance"
        );
        for language in UiLanguage::ALL {
            assert_eq!(
                language.tr("settings.sidebar.appearance"),
                language.text(Message::SettingsSidebarAppearance)
            );
            assert_eq!(language.tr("missing.message.id"), "missing.message.id");
        }
    }

    /// 迁移桥:中英即时分支;未迁移文案按语言各自取原值。
    #[test]
    fn inline_migration_preserves_bilingual_text_and_english_fallback() {
        assert_eq!(UiLanguage::ZhCn.pick("网络", "Network"), "网络");
        assert_eq!(UiLanguage::EnUs.pick("网络", "Network"), "Network");
        assert_eq!(
            UiLanguage::EnUs.pick("未迁移文案", "Unmigrated text"),
            "Unmigrated text"
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

    /// 字符串键 + 参数兼容层。
    #[test]
    fn tr_args_matches_typed_format() {
        assert_eq!(
            UiLanguage::ZhCn.tr_args("settings.status.operation_failed", &[("error", "boom")]),
            UiLanguage::ZhCn.format(Message::SettingsStatusOperationFailed, &[("error", "boom")])
        );
    }
}
