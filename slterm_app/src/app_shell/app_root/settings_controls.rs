//! 设置页字段控件模型：注册表驱动、i18n 标签与选项值的单一映射。
//!
//! 字段目录与校验仍归 `slterm_settings::SettingsRegistry`；本表只增加页面
//! 归属、控件形态、显示范围与选项文案。键集合双向强等于注册表，缺控件或
//! 孤儿控件都由单元测试拦截。

use slterm_settings::SettingsKeyDef;
use slterm_settings::json::Value;
use slterm_settings::keys::SettingsValueType;

use crate::i18n::Message;
use crate::settings_state::SettingsPageId;

/// 一个选项的展示文案；`None` = 直接显示存储值（主题名、语言专名）。
pub(super) struct ChoiceOption {
    pub value: &'static str,
    pub label: Option<Message>,
}

const fn option(value: &'static str, label: Message) -> ChoiceOption {
    ChoiceOption {
        value,
        label: Some(label),
    }
}

const fn literal(value: &'static str) -> ChoiceOption {
    ChoiceOption { value, label: None }
}

/// 控件形态；数值范围是显示边界，注册表校验仍是最终闸门。
pub(super) enum SettingsControl {
    Toggle,
    Choice(&'static [ChoiceOption]),
    Stepper { min: f64, max: f64, step: f64 },
    Text,
    Color,
}

/// 页面内一个字段行。
pub(super) struct SettingsFieldUi {
    pub key: &'static str,
    pub page: SettingsPageId,
    pub section: Message,
    pub label: Message,
    pub description: Option<Message>,
    pub control: SettingsControl,
}

/// 选项值 → 注册表类型对应的 JSON 值；数值型选项按字面量解析。
pub(super) fn typed_value(def: &SettingsKeyDef, option_value: &str) -> Value {
    match def.value_type {
        SettingsValueType::Boolean => Value::Bool(option_value == "true"),
        SettingsValueType::Integer => Value::from(
            option_value
                .parse::<usize>()
                .expect("控件选项必须是整数字面量"),
        ),
        SettingsValueType::Number => Value::Number(
            option_value
                .parse::<f64>()
                .expect("控件选项必须是数值字面量"),
        ),
        SettingsValueType::String => Value::String(option_value.to_owned()),
    }
}

const LANGUAGES: &[ChoiceOption] = &[
    option("system", Message::SettingsOptionsFollowSystem),
    literal("zh-CN"),
    literal("en-US"),
];

const THEMES: &[ChoiceOption] = &[
    literal("Linear"),
    literal("BreezeDark"),
    literal("MintDark"),
    literal("Nord"),
    literal("CatppuccinMocha"),
    literal("CatppuccinFrappe"),
    literal("CatppuccinMacchiato"),
    literal("GlassDark"),
];

const BLUR_MODES: &[ChoiceOption] = &[
    option("none", Message::SettingsOptionsBlurNone),
    option("mica", Message::SettingsOptionsMica),
    option("mica-alt", Message::SettingsOptionsMicaAlt),
    option("aero", Message::SettingsOptionsAero),
    option("acrylic", Message::SettingsOptionsAcrylic),
];

const WALLPAPER_FITS: &[ChoiceOption] = &[
    option("fill", Message::SettingsAppearanceWallpaperFitFill),
    option("uniform", Message::SettingsAppearanceWallpaperFitUniform),
    option(
        "uniform_to_fill",
        Message::SettingsAppearanceWallpaperFitUniformToFill,
    ),
    option("none", Message::SettingsAppearanceWallpaperFitNone),
];

const WALLPAPER_ALIGNMENTS: &[ChoiceOption] = &[
    option("top-left", Message::SettingsAppearanceWallpaperAlignTopLeft),
    option("top", Message::SettingsAppearanceWallpaperAlignTop),
    option(
        "top-right",
        Message::SettingsAppearanceWallpaperAlignTopRight,
    ),
    option("left", Message::SettingsAppearanceWallpaperAlignLeft),
    option("center", Message::SettingsAppearanceWallpaperAlignCenter),
    option("right", Message::SettingsAppearanceWallpaperAlignRight),
    option(
        "bottom-left",
        Message::SettingsAppearanceWallpaperAlignBottomLeft,
    ),
    option("bottom", Message::SettingsAppearanceWallpaperAlignBottom),
    option(
        "bottom-right",
        Message::SettingsAppearanceWallpaperAlignBottomRight,
    ),
];

const DENSITIES: &[ChoiceOption] = &[
    option("standard", Message::SettingsOptionsStandard),
    option("compact", Message::SettingsOptionsDensityCompact),
];

const CELL_WIDTHS: &[ChoiceOption] = &[
    option("compact", Message::SettingsOptionsCellWidthCompact),
    option("relaxed", Message::SettingsOptionsRelaxed),
];

const TAB_REVEALS: &[ChoiceOption] = &[
    option("slide", Message::SettingsOptionsSlide),
    option("instant", Message::SettingsOptionsInstant),
];

const VCS_DISPLAYS: &[ChoiceOption] = &[
    option("auto", Message::SettingsOptionsAutoDetect),
    option("git", Message::SettingsOptionsGitOnly),
];

const LIGATURE_MODES: &[ChoiceOption] = &[
    option("on", Message::SettingsOptionsLigaturesOn),
    option("off", Message::SettingsOptionsLigaturesOff),
    option("theme", Message::SettingsOptionsFollowTheme),
];

const SCROLLBACK_LINES: &[ChoiceOption] = &[
    literal("1000"),
    literal("2000"),
    literal("5000"),
    literal("10000"),
    literal("20000"),
    literal("50000"),
    literal("100000"),
];

const CURSOR_SHAPES: &[ChoiceOption] = &[
    option("block", Message::SettingsOptionsBlock),
    option("beam", Message::SettingsOptionsBar),
    option("underline", Message::SettingsOptionsUnderline),
    option("hollow", Message::SettingsOptionsHollow),
];

const CURSOR_MOTIONS: &[ChoiceOption] = &[
    option("off", Message::SettingsOptionsCursorMotionOff),
    option("smooth", Message::SettingsOptionsSmooth),
];

const NEW_TAB_POSITIONS: &[ChoiceOption] = &[
    option("after_current", Message::SettingsOptionsAfterCurrent),
    option("end", Message::SettingsOptionsEndOfList),
];

const BELL_MODES: &[ChoiceOption] = &[
    option("none", Message::SettingsOptionsBellOff),
    option("visual", Message::SettingsOptionsVisual),
    option("audible", Message::SettingsOptionsSound),
    option("both", Message::SettingsOptionsVisualSound),
];

const ACCEPT_KEYS: &[ChoiceOption] = &[
    option("right", Message::SettingsOptionsRightArrow),
    option("tab", Message::SettingsOptionsTab),
    option("both", Message::SettingsOptionsTabOrRight),
];

const COMPLETION_STYLES: &[ChoiceOption] = &[
    option("inline", Message::SettingsOptionsInline),
    option("popup", Message::SettingsOptionsPopup),
    option("hybrid", Message::SettingsOptionsHybrid),
];

const NOTIFICATION_DURATIONS: &[ChoiceOption] = &[
    option("default", Message::SettingsOptionsDurationDefault),
    option("5", Message::SettingsOptionsDurationFive),
    option("10", Message::SettingsOptionsDurationTen),
    option("30", Message::SettingsOptionsDurationThirty),
    option("90", Message::SettingsOptionsDurationNinety),
    option("persistent", Message::SettingsOptionsPersistent),
];

const fn field(
    key: &'static str,
    page: SettingsPageId,
    section: Message,
    label: Message,
    description: Option<Message>,
    control: SettingsControl,
) -> SettingsFieldUi {
    SettingsFieldUi {
        key,
        page,
        section,
        label,
        description,
        control,
    }
}

use SettingsControl::{Choice, Color, Stepper, Text, Toggle};

/// 全部页面字段；顺序即渲染顺序，同页相邻同 section 形成一个分组。
pub(super) static FIELDS: &[SettingsFieldUi] = &[
    // ---- Appearance：显示 / 字体 / 卡片几何 ----
    field(
        "theme",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceTheme,
        Some(Message::SettingsAppearanceThemeDescription),
        Choice(THEMES),
    ),
    field(
        "opacity",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceOpacity,
        Some(Message::SettingsAppearanceOpacityDescription),
        Stepper {
            min: 0.05,
            max: 1.0,
            step: 0.05,
        },
    ),
    field(
        "blur",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceBlur,
        None,
        Choice(BLUR_MODES),
    ),
    field(
        "background",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceBackgroundColor,
        None,
        Color,
    ),
    field(
        "theme_foreground",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceThemeForeground,
        None,
        Color,
    ),
    field(
        "background_image",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceWallpaperPath,
        Some(Message::SettingsAppearanceWallpaperPathDescription),
        Text,
    ),
    field(
        "background_image_opacity",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceWallpaperOpacity,
        Some(Message::SettingsAppearanceWallpaperOpacityDescription),
        Stepper {
            min: 0.0,
            max: 1.0,
            step: 0.05,
        },
    ),
    field(
        "background_image_fit",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceWallpaperFit,
        None,
        Choice(WALLPAPER_FITS),
    ),
    field(
        "background_image_alignment",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceWallpaperAlignment,
        None,
        Choice(WALLPAPER_ALIGNMENTS),
    ),
    field(
        "background_image_cover_chrome",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceWallpaperCoverChrome,
        Some(Message::SettingsAppearanceWallpaperCoverChromeDescription),
        Toggle,
    ),
    field(
        "density",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceDensity,
        None,
        Choice(DENSITIES),
    ),
    field(
        "cell_width_mode",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceCellWidth,
        None,
        Choice(CELL_WIDTHS),
    ),
    field(
        "tab_reveal",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceTabReveal,
        None,
        Choice(TAB_REVEALS),
    ),
    field(
        "tab_close_visible",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceTabClose,
        Some(Message::SettingsAppearanceTabCloseDescription),
        Toggle,
    ),
    field(
        "cjk_bold_regular",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceCjkBold,
        None,
        Toggle,
    ),
    field(
        "vcs_display",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceDisplay,
        Message::SettingsAppearanceVcsDisplay,
        None,
        Choice(VCS_DISPLAYS),
    ),
    field(
        "font_family",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceFontFamily,
        Some(Message::SettingsAppearanceFontFamilyDescription),
        Text,
    ),
    field(
        "font_family_cjk",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceFontFamilyCjk,
        None,
        Text,
    ),
    field(
        "ui_font_family",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceUiFontFamily,
        None,
        Text,
    ),
    field(
        "ui_font_size",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceUiFontSize,
        None,
        Stepper {
            min: 8.0,
            max: 24.0,
            step: 1.0,
        },
    ),
    field(
        "font_size",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceFontSize,
        Some(Message::SettingsAppearanceFontSizeDescription),
        Stepper {
            min: 4.0,
            max: 96.0,
            step: 1.0,
        },
    ),
    field(
        "ligatures",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceFonts,
        Message::SettingsAppearanceLigatures,
        Some(Message::SettingsAppearanceLigaturesDescription),
        Choice(LIGATURE_MODES),
    ),
    field(
        "pane_card_radius",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceCardGeometry,
        Message::SettingsAppearancePaneCardRadius,
        None,
        Stepper {
            min: 0.0,
            max: slterm_settings::MAX_PANE_CARD_RADIUS as f64,
            step: 1.0,
        },
    ),
    field(
        "pane_card_gutter",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceCardGeometry,
        Message::SettingsAppearancePaneCardGutter,
        None,
        Stepper {
            min: 0.0,
            max: slterm_settings::MAX_PANE_CARD_GUTTER as f64,
            step: 1.0,
        },
    ),
    field(
        "pane_card_shadow",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceCardGeometry,
        Message::SettingsAppearancePaneCardShadow,
        None,
        Toggle,
    ),
    field(
        "pane_card_divider",
        SettingsPageId::Appearance,
        Message::SettingsAppearanceCardGeometry,
        Message::SettingsAppearancePaneCardDivider,
        None,
        Stepper {
            min: 0.0,
            max: slterm_settings::MAX_PANE_CARD_DIVIDER as f64,
            step: 1.0,
        },
    ),
    // ---- Scrolling ----
    field(
        "scrollback_lines",
        SettingsPageId::Scrolling,
        Message::SettingsSidebarScrolling,
        Message::SettingsAppearanceScrollback,
        Some(Message::SettingsAppearanceScrollbackDescription),
        Choice(SCROLLBACK_LINES),
    ),
    field(
        "scroll_speed",
        SettingsPageId::Scrolling,
        Message::SettingsSidebarScrolling,
        Message::SettingsAppearanceScrollSpeed,
        Some(Message::SettingsAppearanceScrollSpeedDescription),
        Stepper {
            min: 0.25,
            max: 4.0,
            step: 0.25,
        },
    ),
    // ---- Interaction：输入 / 光标 / 标签 ----
    field(
        "copy_on_select",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionCopyOnSelect,
        None,
        Toggle,
    ),
    field(
        "multiline_paste_confirm",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionPasteConfirm,
        Some(Message::SettingsInteractionPasteConfirmDescription),
        Toggle,
    ),
    field(
        "panel_resize",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionPanelResize,
        None,
        Toggle,
    ),
    field(
        "document_autosave",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionDocumentAutosave,
        None,
        Toggle,
    ),
    field(
        "osc52_unfocused",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionOsc52Unfocused,
        None,
        Toggle,
    ),
    field(
        "ctrl_wheel_font_zoom",
        SettingsPageId::Interaction,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionCtrlWheelZoom,
        None,
        Toggle,
    ),
    field(
        "cursor_shape",
        SettingsPageId::Interaction,
        Message::SettingsInteractionCursor,
        Message::SettingsInteractionCursorShape,
        None,
        Choice(CURSOR_SHAPES),
    ),
    field(
        "cursor_blink",
        SettingsPageId::Interaction,
        Message::SettingsInteractionCursor,
        Message::SettingsInteractionCursorBlink,
        None,
        Toggle,
    ),
    field(
        "cursor_motion",
        SettingsPageId::Interaction,
        Message::SettingsInteractionCursor,
        Message::SettingsInteractionCursorMotion,
        None,
        Choice(CURSOR_MOTIONS),
    ),
    field(
        "new_tab_position",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionNewTabPosition,
        None,
        Choice(NEW_TAB_POSITIONS),
    ),
    field(
        "focus_follows_mouse",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionFocusFollows,
        None,
        Toggle,
    ),
    field(
        "dim_inactive_panes",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionDimInactive,
        None,
        Toggle,
    ),
    field(
        "ghost",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesGhost,
        None,
        Toggle,
    ),
    field(
        "completion_style",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesCompletionStyle,
        None,
        Choice(COMPLETION_STYLES),
    ),
    field(
        "accept",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesAccept,
        None,
        Choice(ACCEPT_KEYS),
    ),
    field(
        "bell",
        SettingsPageId::Interaction,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesBell,
        None,
        Choice(BELL_MODES),
    ),
    // ---- Session ----
    field(
        "shell",
        SettingsPageId::Session,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedShell,
        None,
        Text,
    ),
    field(
        "startup_directory",
        SettingsPageId::Session,
        Message::SettingsAdvancedSession,
        Message::SettingsProfilesStartupDirectory,
        Some(Message::SettingsProfilesStartupDirectoryDescription),
        Text,
    ),
    field(
        "restore_session",
        SettingsPageId::Session,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedRestoreSession,
        None,
        Toggle,
    ),
    field(
        "resume_ai",
        SettingsPageId::Session,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedResumeAi,
        None,
        Toggle,
    ),
    field(
        "keep_session",
        SettingsPageId::Session,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedKeepSession,
        None,
        Toggle,
    ),
    field(
        "silent_start",
        SettingsPageId::Session,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedSilentStart,
        Some(Message::SettingsAdvancedSilentStartTrayHint),
        Toggle,
    ),
    field(
        "launch_at_login",
        SettingsPageId::Session,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedLaunchAtLogin,
        None,
        Toggle,
    ),
    field(
        "fetch",
        SettingsPageId::Session,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedWelcome,
        None,
        Toggle,
    ),
    // ---- System ----
    field(
        "language",
        SettingsPageId::System,
        Message::SettingsLocalizationGroup,
        Message::SettingsLocalizationLanguage,
        Some(Message::SettingsLocalizationDescription),
        Choice(LANGUAGES),
    ),
    field(
        "terminal_proxy",
        SettingsPageId::System,
        Message::SettingsAdvancedEnvironment,
        Message::SettingsAdvancedProxy,
        Some(Message::SettingsAdvancedProxyDescription),
        Toggle,
    ),
    field(
        "refresh_environment",
        SettingsPageId::System,
        Message::SettingsAdvancedEnvironment,
        Message::SettingsProfilesRefreshEnvironment,
        Some(Message::SettingsProfilesRefreshEnvironmentDescription),
        Toggle,
    ),
    field(
        "tray",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedTray,
        None,
        Toggle,
    ),
    field(
        "auto_check_updates",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedCheckUpdates,
        None,
        Toggle,
    ),
    field(
        "auto_download_updates",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedDownloadUpdates,
        Some(Message::SettingsAdvancedDownloadUpdatesDescription),
        Toggle,
    ),
    field(
        "ai_toasts",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesAiToasts,
        None,
        Toggle,
    ),
    field(
        "desktop_notifications",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesDesktopNotifications,
        Some(Message::SettingsProfilesDesktopNotificationsDescription),
        Toggle,
    ),
    field(
        "taskbar_flash",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesTaskbarFlash,
        Some(Message::SettingsProfilesTaskbarFlashDescription),
        Toggle,
    ),
    field(
        "notification_duration",
        SettingsPageId::System,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesNotificationDuration,
        Some(Message::SettingsProfilesNotificationDurationDescription),
        Choice(NOTIFICATION_DURATIONS),
    ),
];

/// 页面字段迭代器；顺序即静态表顺序。
pub(super) fn fields_for_page(
    page: SettingsPageId,
) -> impl Iterator<Item = &'static SettingsFieldUi> {
    FIELDS.iter().filter(move |field| field.page == page)
}

/// 全部字段当前值快照；调用方在 AppRoot 方法内直接读取 store，避免重入 lease。
pub(super) fn initial_field_values(
    raw: &slterm_settings::RawSettings,
) -> std::collections::BTreeMap<&'static str, Option<Value>> {
    let registry = slterm_settings::SettingsRegistry::new();
    FIELDS
        .iter()
        .map(|field| {
            (
                field.key,
                registry
                    .recover(raw, field.key)
                    .and_then(|recovery| recovery.value),
            )
        })
        .collect()
}

#[cfg(test)]
mod settings_control_tests {
    use std::collections::BTreeSet;

    use slterm_settings::SettingsRegistry;
    use slterm_settings::keys::{SettingsDomain, SettingsValueType};

    use super::{FIELDS, SettingsControl};
    use crate::i18n::LanguagePreference;
    use crate::settings_state::SettingsPageId;

    fn displayed_keys() -> BTreeSet<&'static str> {
        let mut keys = BTreeSet::new();
        for page in [
            SettingsPageId::Appearance,
            SettingsPageId::Scrolling,
            SettingsPageId::Interaction,
            SettingsPageId::Session,
            SettingsPageId::System,
        ] {
            for field in super::fields_for_page(page) {
                assert!(
                    keys.insert(field.key),
                    "字段 {} 在多个页面重复出现",
                    field.key
                );
            }
        }
        keys
    }

    /// 主题编辑器宿主管理的键：不进标量字段行，但必须在注册表内。
    const EDITOR_MANAGED: [&str; 1] = ["custom_theme"];

    #[test]
    fn every_setting_field_in_displayed_domains_has_exactly_one_control() {
        let registry = SettingsRegistry::new();
        let expected: BTreeSet<&str> = registry
            .fields()
            .iter()
            .filter(|def| {
                matches!(
                    def.domain,
                    SettingsDomain::Appearance
                        | SettingsDomain::Scrolling
                        | SettingsDomain::Interaction
                        | SettingsDomain::Session
                        | SettingsDomain::System
                ) && !EDITOR_MANAGED.contains(&def.key)
            })
            .map(|def| def.key)
            .collect();
        for key in EDITOR_MANAGED {
            assert!(registry.lookup(key).is_some(), "{key} 不在注册表");
        }
        assert_eq!(displayed_keys(), expected);
        assert_eq!(FIELDS.len(), expected.len());
    }

    #[test]
    fn choice_and_stepper_values_are_accepted_by_the_registry() {
        let registry = SettingsRegistry::new();
        for field in FIELDS {
            let def = registry
                .lookup(field.key)
                .unwrap_or_else(|| panic!("字段 {} 未在注册表登记", field.key));
            match &field.control {
                SettingsControl::Toggle => {
                    assert_eq!(def.value_type, SettingsValueType::Boolean, "{}", field.key)
                }
                SettingsControl::Text | SettingsControl::Color => {
                    assert_eq!(def.value_type, SettingsValueType::String, "{}", field.key)
                }
                SettingsControl::Choice(options) => {
                    assert!(!options.is_empty(), "{} 没有选项", field.key);
                    for option in *options {
                        let value = super::typed_value(def, option.value);
                        assert!(
                            registry.validate(field.key, &value) == Some(true),
                            "{} 的选项 {:?} 未通过校验",
                            field.key,
                            option.value
                        );
                    }
                }
                SettingsControl::Stepper { min, max, step } => {
                    assert!(min < max && *step > 0.0, "{} 的范围无效", field.key);
                    for value in [*min, *max] {
                        assert!(
                            registry
                                .validate(field.key, &slterm_settings::json::Value::Number(value))
                                == Some(true),
                            "{} 的边界 {value} 未通过校验",
                            field.key
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_label_and_section_is_translated_in_both_languages() {
        for language in [
            LanguagePreference::EnUs.resolved(),
            LanguagePreference::ZhCn.resolved(),
        ] {
            for field in FIELDS {
                assert!(!language.text(field.label).is_empty(), "{:?}", field.label);
                assert!(
                    !language.text(field.section).is_empty(),
                    "{:?}",
                    field.section
                );
                if let Some(description) = field.description {
                    assert!(!language.text(description).is_empty(), "{description:?}");
                }
                if let SettingsControl::Choice(options) = &field.control {
                    for option in *options {
                        if let Some(label) = option.label {
                            assert!(!language.text(label).is_empty(), "{label:?}");
                        } else {
                            assert!(!option.value.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn theme_choices_match_the_builtin_catalog() {
        let field = FIELDS
            .iter()
            .find(|field| field.key == "theme")
            .expect("主题字段必须存在");
        let SettingsControl::Choice(options) = &field.control else {
            panic!("主题字段必须是选择控件");
        };
        let values: Vec<&str> = options.iter().map(|option| option.value).collect();
        assert_eq!(values, slterm_settings::ThemeName::BUILTIN_NAMES.to_vec());
    }
}
