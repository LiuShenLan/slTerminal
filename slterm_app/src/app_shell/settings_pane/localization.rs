//! 值/显示分离单点:稳定持久化值 → 当前语言显示文案。
//!
//! 控件创建与语言切换刷新共用同一入口 [`localized_select_labels`] /
//! [`localized_input_placeholder`],因此 `SelectState` 不会留着构造时的
//! 旧语言;控件实体跨语言保留(重建会丢焦点/编辑值/undo/订阅)。
//!
//! 显示文案统一走 build.rs 生成的 typed `Message`(静态表查询);
//! 语言选择器由 `LanguagePref::ALL` 注册表驱动,显式语言一律 native name
//! 自显示。`debug_assert` 钉住标签数与值数对齐。

use gpui::SharedString;
use slterm_settings::{LanguagePref, SCROLLBACK_VALUES};

/// 既有消费方(navigation/theme_editor/设置页)经本模块取同一渲染枚举。
pub(crate) use crate::i18n::UiLanguage;
use crate::i18n::{LanguagePreference, Message};

/// 持久化偏好 → 渲染态。System 档经系统 locale 协商,解析不出回英文。
/// 调用方应缓存结果(设置页在运行时重载点更新,渲染路径不重复探测)。
pub(crate) fn resolve_language(preference: LanguagePref) -> UiLanguage {
    LanguagePreference::from(preference).resolved()
}

/// 千分位显示(回滚档位;纯显示,不改持久化值)。
fn format_thousands(number: usize) -> String {
    let digits = number.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// 固定下拉的显示文案。`values` 与返回标签严格同序同数——选中索引即
/// 持久化值索引,语言切换只换显示项不换值。
pub(crate) fn localized_select_labels(
    key: &str,
    values: &[&'static str],
    language: UiLanguage,
) -> Vec<SharedString> {
    let labels: Vec<SharedString> = match key {
        "language" => LanguagePref::ALL
            .iter()
            .map(|preference| match preference {
                LanguagePref::System => language.text(Message::LanguageSystem).into(),
                other => other.native_name().into(),
            })
            .collect(),
        "theme" => values
            .iter()
            .map(|value| SharedString::from(*value))
            .collect(),
        "scrollback_lines" => SCROLLBACK_VALUES
            .iter()
            .map(|value| SharedString::from(format_thousands(*value)))
            .collect(),
        "cursor_shape" => vec![
            language.text(Message::SettingsOptionsBlock).into(),
            language.text(Message::SettingsOptionsBar).into(),
            language.text(Message::SettingsOptionsUnderline).into(),
            language.text(Message::SettingsOptionsHollow).into(),
        ],
        "cursor_motion" => vec![
            language
                .text(Message::SettingsOptionsCursorMotionOff)
                .into(),
            language.text(Message::SettingsOptionsSmooth).into(),
        ],
        "tab_reveal" => vec![
            language.text(Message::SettingsOptionsSlide).into(),
            language.text(Message::SettingsOptionsInstant).into(),
        ],
        "density" => vec![
            language.text(Message::SettingsOptionsStandard).into(),
            language.text(Message::SettingsOptionsDensityCompact).into(),
        ],
        "new_tab_position" => vec![
            language.text(Message::SettingsOptionsAfterCurrent).into(),
            language.text(Message::SettingsOptionsEndOfList).into(),
        ],
        "vcs_display" => vec![
            language.text(Message::SettingsOptionsAutoDetect).into(),
            language.text(Message::SettingsOptionsGitOnly).into(),
        ],
        "ligatures" => vec![
            language.text(Message::SettingsOptionsLigaturesOn).into(),
            language.text(Message::SettingsOptionsLigaturesOff).into(),
            language.text(Message::SettingsOptionsFollowTheme).into(),
        ],
        "bell" => vec![
            language.text(Message::SettingsOptionsBellOff).into(),
            language.text(Message::SettingsOptionsVisual).into(),
            language.text(Message::SettingsOptionsSound).into(),
            language.text(Message::SettingsOptionsVisualSound).into(),
        ],
        "cell_width_mode" => vec![
            language
                .text(Message::SettingsOptionsCellWidthCompact)
                .into(),
            language.text(Message::SettingsOptionsRelaxed).into(),
        ],
        "accept" => vec![
            language.text(Message::SettingsOptionsRightArrow).into(),
            language.text(Message::SettingsOptionsTab).into(),
            language.text(Message::SettingsOptionsTabOrRight).into(),
        ],
        "completion_style" => vec![
            language.text(Message::SettingsOptionsInline).into(),
            language.text(Message::SettingsOptionsPopup).into(),
            language.text(Message::SettingsOptionsHybrid).into(),
        ],
        "notification_duration" => vec![
            language
                .text(Message::SettingsOptionsDurationDefault)
                .into(),
            language.text(Message::SettingsOptionsDurationFive).into(),
            language.text(Message::SettingsOptionsDurationTen).into(),
            language.text(Message::SettingsOptionsDurationThirty).into(),
            language.text(Message::SettingsOptionsDurationNinety).into(),
            language.text(Message::SettingsOptionsPersistent).into(),
        ],
        "blur" => vec![
            language.text(Message::SettingsOptionsBlurNone).into(),
            language.text(Message::SettingsOptionsMica).into(),
            language.text(Message::SettingsOptionsMicaAlt).into(),
            language.text(Message::SettingsOptionsAero).into(),
            language.text(Message::SettingsOptionsAcrylic).into(),
        ],
        _ => values
            .iter()
            .map(|value| SharedString::from(*value))
            .collect(),
    };
    debug_assert_eq!(
        labels.len(),
        values.len(),
        "localized select label/value mismatch: {key}"
    );
    labels
}

/// 输入框占位文案(语言切换时经 `InputState::set_placeholder` 刷新)。
pub(crate) fn localized_input_placeholder(key: &str, language: UiLanguage) -> SharedString {
    let text = match key {
        "font_family" => language.text(Message::SettingsPlaceholderFontFamily),
        "font_size" => language.text(Message::SettingsPlaceholderFontSize),
        "startup_directory" => language.text(Message::SettingsPlaceholderStartupDirectory),
        "backup_passphrase" => language.text(Message::SettingsBackupPassphrasePlaceholder),
        _ => "",
    };
    SharedString::from(text)
}

#[cfg(test)]
mod localization_tests {
    use super::*;
    use slterm_settings::{
        AcceptKeyName, BellModeName, BlurMode, CellWidthModeName, CompletionStyleName,
        CursorMotion, CursorShapeName, DensityName, Ligatures, NewTabPositionName,
        NotificationDuration, TabRevealName, VcsDisplayName,
    };

    /// 每个固定下拉的标签数必须与值数一致(选中索引 ↔ 持久化值的唯一
    /// 契约),且语言切换后值表原样保留。
    #[test]
    fn every_select_keeps_label_and_value_arrays_aligned() {
        let cases: [(&str, &'static [&'static str]); 15] = [
            ("language", LanguagePref::VALUES),
            (
                "scrollback_lines",
                &["1000", "2000", "5000", "10000", "20000", "50000", "100000"],
            ),
            ("cursor_shape", &CursorShapeName::VALUES),
            ("cursor_motion", &CursorMotion::VALUES),
            ("tab_reveal", &TabRevealName::VALUES),
            ("density", &DensityName::VALUES),
            ("new_tab_position", &NewTabPositionName::VALUES),
            ("vcs_display", &VcsDisplayName::VALUES),
            ("ligatures", &Ligatures::VALUES),
            ("bell", &BellModeName::VALUES),
            ("cell_width_mode", &CellWidthModeName::VALUES),
            ("accept", &AcceptKeyName::VALUES),
            ("completion_style", &CompletionStyleName::VALUES),
            ("notification_duration", &NotificationDuration::VALUES),
            ("blur", &BlurMode::VALUES),
        ];
        for language in [UiLanguage::ZhCn, UiLanguage::EnUs] {
            for (key, values) in cases {
                let labels = localized_select_labels(key, values, language);
                assert_eq!(labels.len(), values.len(), "{key} 标签/值数量失配");
            }
        }
    }

    /// 语言下拉 = `LanguagePref::ALL` 顺序;显式语言 native name 自显示,
    /// System 档走双语目录。
    #[test]
    fn language_select_uses_the_registry_and_native_names() {
        let zh = localized_select_labels("language", LanguagePref::VALUES, UiLanguage::ZhCn);
        let zh: Vec<&str> = zh.iter().map(AsRef::as_ref).collect();
        assert_eq!(zh, ["跟随系统", "简体中文", "English"]);
        let en = localized_select_labels("language", LanguagePref::VALUES, UiLanguage::EnUs);
        let en: Vec<&str> = en.iter().map(AsRef::as_ref).collect();
        assert_eq!(en, ["System", "简体中文", "English"]);
        assert_eq!(zh.len(), LanguagePref::ALL.len());
    }

    /// 值/显示分离:同一索引在两种语言下显示不同文案,但值表本身不变。
    #[test]
    fn language_switch_changes_labels_without_touching_values() {
        let zh = localized_select_labels("bell", &BellModeName::VALUES, UiLanguage::ZhCn);
        let en = localized_select_labels("bell", &BellModeName::VALUES, UiLanguage::EnUs);
        assert_eq!(zh[0].as_ref(), "关");
        assert_eq!(en[0].as_ref(), "Off");
        assert_eq!(BellModeName::VALUES[0], "none", "持久化值必须仍是稳定记号");
    }

    #[test]
    fn scrollback_labels_are_thousands_grouped_and_ordered() {
        let labels = localized_select_labels(
            "scrollback_lines",
            &["1000", "2000", "5000", "10000", "20000", "50000", "100000"],
            UiLanguage::ZhCn,
        );
        let labels: Vec<&str> = labels.iter().map(AsRef::as_ref).collect();
        assert_eq!(
            labels,
            [
                "1,000", "2,000", "5,000", "10,000", "20,000", "50,000", "100,000"
            ]
        );
    }

    /// 显式偏好不受系统 locale 影响;System 档至少落在之一。
    #[test]
    fn system_language_negotiates_chinese_regions_and_falls_back_to_english() {
        assert_eq!(resolve_language(LanguagePref::ZhCn), UiLanguage::ZhCn);
        assert_eq!(resolve_language(LanguagePref::EnUs), UiLanguage::EnUs);
        let system = resolve_language(LanguagePref::System);
        assert!(matches!(system, UiLanguage::ZhCn | UiLanguage::EnUs));
    }

    #[test]
    fn placeholders_are_localized_at_the_single_entry_point() {
        assert_eq!(
            localized_input_placeholder("font_family", UiLanguage::ZhCn).as_ref(),
            "输入字体名称"
        );
        assert_eq!(
            localized_input_placeholder("font_family", UiLanguage::EnUs).as_ref(),
            "Enter a font family"
        );
        assert!(
            localized_input_placeholder("unknown", UiLanguage::EnUs)
                .as_ref()
                .is_empty()
        );
    }
}
