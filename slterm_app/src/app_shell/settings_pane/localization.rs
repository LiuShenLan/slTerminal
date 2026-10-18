//! 值/显示分离单点:稳定持久化值 → 当前语言显示文案。
//!
//! 控件创建与语言切换刷新共用同一入口 [`localized_select_labels`] /
//! [`localized_input_placeholder`],因此 `SelectState` 不会留着构造时的
//! 旧语言;控件实体跨语言保留(重建会丢焦点/编辑值/undo/订阅)。
//!
//! i18n 静态生成器归后续阶段:本阶段用 `pick(中文, 英文)` 迁移桥即时
//! 分支,键集合届时一次收口;`debug_assert` 钉住标签数与值数对齐。

use gpui::SharedString;
use slterm_settings::{LanguagePref, SCROLLBACK_VALUES};

/// 界面语言(查询桥)。与 `LanguagePref`(设置值)分离:前者是渲染态,
/// 后者是持久化偏好(System 档需要一次 locale 协商)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiLanguage {
    ZhCn,
    EnUs,
}

impl UiLanguage {
    /// 中英即时分支(迁移桥;新文案一律双语文面量进这里)。
    pub(crate) const fn pick<'text>(self, zh_cn: &'text str, en_us: &'text str) -> &'text str {
        match self {
            Self::ZhCn => zh_cn,
            Self::EnUs => en_us,
        }
    }
}

/// 持久化偏好 → 渲染态。System 档经系统 locale 协商,解析不出回英文
/// (与语言注册表「回英文」兜底同语义)。
pub(crate) fn resolve_language(preference: LanguagePref) -> UiLanguage {
    match preference {
        LanguagePref::ZhCn => UiLanguage::ZhCn,
        LanguagePref::EnUs => UiLanguage::EnUs,
        LanguagePref::System => crate::platform::locale::system_locale()
            .as_deref()
            .and_then(LanguagePref::from_locale)
            .map(ui_language_of)
            .unwrap_or(UiLanguage::EnUs),
    }
}

fn ui_language_of(preference: LanguagePref) -> UiLanguage {
    match preference {
        LanguagePref::ZhCn => UiLanguage::ZhCn,
        // System 在协商结果里没有对象;防御性回英文。
        LanguagePref::EnUs | LanguagePref::System => UiLanguage::EnUs,
    }
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
                LanguagePref::System => language.pick("跟随系统", "System").into(),
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
            language.pick("实心框", "Block").into(),
            language.pick("条形", "Bar").into(),
            language.pick("下划线", "Underline").into(),
            language.pick("空心框", "Hollow").into(),
        ],
        "cursor_motion" => vec![
            language.pick("关闭", "Off").into(),
            language.pick("平滑", "Smooth").into(),
        ],
        "tab_reveal" => vec![
            language.pick("滑动", "Slide").into(),
            language.pick("立即", "Instant").into(),
        ],
        "density" => vec![
            language.pick("标准", "Standard").into(),
            language.pick("紧凑", "Compact").into(),
        ],
        "new_tab_position" => vec![
            language.pick("当前标签之后", "After current tab").into(),
            language.pick("列表末尾", "End of list").into(),
        ],
        "vcs_display" => vec![
            language.pick("自动检测", "Auto detect").into(),
            language.pick("仅 Git", "Git only").into(),
        ],
        "ligatures" => vec![
            language.pick("开启", "On").into(),
            language.pick("关闭", "Off").into(),
            language.pick("跟随主题", "Follow theme").into(),
        ],
        "bell" => vec![
            language.pick("关", "Off").into(),
            language.pick("闪烁", "Visual").into(),
            language.pick("声音", "Sound").into(),
            language.pick("闪烁 + 声音", "Visual + sound").into(),
        ],
        "cell_width_mode" => vec![
            language.pick("紧凑", "Compact").into(),
            language.pick("宽松", "Relaxed").into(),
        ],
        "accept" => vec![
            language.pick("右方向键", "Right arrow").into(),
            "Tab".into(),
            language.pick("Tab 或右方向键", "Tab or Right arrow").into(),
        ],
        "completion_style" => vec![
            language.pick("内联", "Inline").into(),
            language.pick("弹窗", "Popup").into(),
            language.pick("混合", "Hybrid").into(),
        ],
        "notification_duration" => vec![
            language.pick("默认", "Default").into(),
            language.pick("5 秒", "5 seconds").into(),
            language.pick("10 秒", "10 seconds").into(),
            language.pick("30 秒", "30 seconds").into(),
            language.pick("90 秒", "90 seconds").into(),
            language.pick("常驻", "Persistent").into(),
        ],
        "blur" => vec![
            language.pick("无", "None").into(),
            "Mica".into(),
            "Mica Alt".into(),
            "Aero".into(),
            "Acrylic".into(),
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
        "font_family" => language.pick("输入字体名称", "Enter a font family"),
        "font_size" => language.pick("例如 15", "e.g. 15"),
        "startup_directory" => language.pick(
            "留空继承当前目录",
            "Leave empty to inherit the current directory",
        ),
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

    #[test]
    fn system_language_negotiates_chinese_regions_and_falls_back_to_english() {
        assert_eq!(
            resolve_language(LanguagePref::ZhCn),
            UiLanguage::ZhCn,
            "显式偏好不受系统 locale 影响"
        );
        assert_eq!(resolve_language(LanguagePref::EnUs), UiLanguage::EnUs);
        // System 档在本机 locale 未知时至少必须落在两家之一(不 panic)。
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
