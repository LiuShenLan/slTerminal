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

/// 路径类文本字段的真实目标约束；注册表只校验类型与非空，语义在这里。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PathRule {
    /// 自定义 Shell：绝对路径且为 Windows 可执行文件。
    ShellExecutable,
    /// 启动目录：绝对路径且为真实目录。
    Directory,
    /// 背景图片：绝对路径且为真实文件。
    File,
}

/// 字段键 → 路径规则；非路径字段返回 `None`。
pub(super) fn path_rule(key: &str) -> Option<PathRule> {
    match key {
        "shell" => Some(PathRule::ShellExecutable),
        "startup_directory" => Some(PathRule::Directory),
        "background_image" => Some(PathRule::File),
        _ => None,
    }
}

/// 无需 I/O 的形状校验：绝对路径；Shell 还要求可执行扩展名。
pub(super) fn path_shape_ok(text: &str, rule: PathRule) -> bool {
    let path = std::path::Path::new(text);
    if !path.is_absolute() {
        return false;
    }
    match rule {
        PathRule::ShellExecutable => path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "exe" | "com" | "bat" | "cmd"
                )
            }),
        PathRule::Directory | PathRule::File => true,
    }
}

/// 真实目标校验；只允许在后台执行体调用。
pub(super) fn path_target_allowed(text: &str, rule: PathRule) -> bool {
    let Ok(metadata) = std::fs::metadata(text) else {
        return false;
    };
    match rule {
        PathRule::Directory => metadata.is_dir(),
        PathRule::ShellExecutable | PathRule::File => metadata.is_file(),
    }
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
    option("compact", Message::SettingsOptionsDensityCompact),
    option("standard", Message::SettingsOptionsStandard),
    option("loose", Message::SettingsOptionsDensityLoose),
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
            min: 8.0,
            max: 32.0,
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
    // ---- Scrolling ----
    field(
        "scrollback_lines",
        SettingsPageId::Terminal,
        Message::SettingsSidebarScrolling,
        Message::SettingsAppearanceScrollback,
        Some(Message::SettingsAppearanceScrollbackDescription),
        Choice(SCROLLBACK_LINES),
    ),
    field(
        "scroll_speed",
        SettingsPageId::Terminal,
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
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionCopyOnSelect,
        None,
        Toggle,
    ),
    field(
        "multiline_paste_confirm",
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionPasteConfirm,
        Some(Message::SettingsInteractionPasteConfirmDescription),
        Toggle,
    ),
    field(
        "panel_resize",
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionPanelResize,
        None,
        Toggle,
    ),
    field(
        "document_autosave",
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionDocumentAutosave,
        None,
        Toggle,
    ),
    field(
        "osc52_unfocused",
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionOsc52Unfocused,
        None,
        Toggle,
    ),
    field(
        "ctrl_wheel_font_zoom",
        SettingsPageId::Terminal,
        Message::SettingsInteractionMouse,
        Message::SettingsInteractionCtrlWheelZoom,
        None,
        Toggle,
    ),
    field(
        "cursor_blink",
        SettingsPageId::Terminal,
        Message::SettingsInteractionCursor,
        Message::SettingsInteractionCursorBlink,
        None,
        Toggle,
    ),
    field(
        "new_tab_position",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionNewTabPosition,
        None,
        Choice(NEW_TAB_POSITIONS),
    ),
    field(
        "focus_follows_mouse",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionFocusFollows,
        None,
        Toggle,
    ),
    field(
        "dim_inactive_panes",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsInteractionDimInactive,
        None,
        Toggle,
    ),
    field(
        "ghost",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesGhost,
        None,
        Toggle,
    ),
    field(
        "completion_style",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesCompletionStyle,
        None,
        Choice(COMPLETION_STYLES),
    ),
    field(
        "accept",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesAccept,
        None,
        Choice(ACCEPT_KEYS),
    ),
    field(
        "bell",
        SettingsPageId::Terminal,
        Message::SettingsInteractionTabs,
        Message::SettingsProfilesBell,
        None,
        Choice(BELL_MODES),
    ),
    // ---- Session ----
    field(
        "shell",
        SettingsPageId::Terminal,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedShell,
        None,
        Text,
    ),
    field(
        "startup_directory",
        SettingsPageId::Terminal,
        Message::SettingsAdvancedSession,
        Message::SettingsProfilesStartupDirectory,
        Some(Message::SettingsProfilesStartupDirectoryDescription),
        Text,
    ),
    field(
        "restore_session",
        SettingsPageId::Terminal,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedRestoreSession,
        None,
        Toggle,
    ),
    field(
        "resume_ai",
        SettingsPageId::Terminal,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedResumeAi,
        None,
        Toggle,
    ),
    field(
        "keep_session",
        SettingsPageId::Terminal,
        Message::SettingsAdvancedSession,
        Message::SettingsAdvancedKeepSession,
        None,
        Toggle,
    ),
    field(
        "silent_start",
        SettingsPageId::Terminal,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedSilentStart,
        Some(Message::SettingsAdvancedSilentStartTrayHint),
        Toggle,
    ),
    field(
        "launch_at_login",
        SettingsPageId::Terminal,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedLaunchAtLogin,
        None,
        Toggle,
    ),
    field(
        "fetch",
        SettingsPageId::Terminal,
        Message::SettingsProfilesStartup,
        Message::SettingsAdvancedWelcome,
        None,
        Toggle,
    ),
    // ---- System ----
    field(
        "language",
        SettingsPageId::Application,
        Message::SettingsLocalizationGroup,
        Message::SettingsLocalizationLanguage,
        Some(Message::SettingsLocalizationDescription),
        Choice(LANGUAGES),
    ),
    field(
        "terminal_proxy",
        SettingsPageId::Application,
        Message::SettingsAdvancedEnvironment,
        Message::SettingsAdvancedProxy,
        Some(Message::SettingsAdvancedProxyDescription),
        Toggle,
    ),
    field(
        "refresh_environment",
        SettingsPageId::Application,
        Message::SettingsAdvancedEnvironment,
        Message::SettingsProfilesRefreshEnvironment,
        Some(Message::SettingsProfilesRefreshEnvironmentDescription),
        Toggle,
    ),
    field(
        "tray",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedTray,
        None,
        Toggle,
    ),
    field(
        "auto_check_updates",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedCheckUpdates,
        None,
        Toggle,
    ),
    field(
        "auto_download_updates",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsAdvancedDownloadUpdates,
        Some(Message::SettingsAdvancedDownloadUpdatesDescription),
        Toggle,
    ),
    field(
        "ai_toasts",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesAiToasts,
        None,
        Toggle,
    ),
    field(
        "desktop_notifications",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesDesktopNotifications,
        Some(Message::SettingsProfilesDesktopNotificationsDescription),
        Toggle,
    ),
    field(
        "taskbar_flash",
        SettingsPageId::Application,
        Message::SettingsAdvancedUpdates,
        Message::SettingsProfilesTaskbarFlash,
        Some(Message::SettingsProfilesTaskbarFlashDescription),
        Toggle,
    ),
    field(
        "notification_duration",
        SettingsPageId::Application,
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
            SettingsPageId::Terminal,
            SettingsPageId::Application,
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

    /// 宿主管理或已退休的键：不进标量字段行，但注册表仍保留旧值。
    /// 光标形状固定竖线、平滑移动不在行为真值内，旧键不再暴露。
    const EDITOR_MANAGED: [&str; 3] = ["custom_theme", "cursor_shape", "cursor_motion"];

    /// 最终排除项不得出现在真实字段表；密度按规格给紧凑／标准／宽松三档。
    #[test]
    fn excluded_appearance_fields_are_absent_and_density_has_three_levels() {
        const EXCLUDED: [&str; 5] = [
            "cursor_shape",
            "pane_card_radius",
            "pane_card_gutter",
            "pane_card_shadow",
            "pane_card_divider",
        ];
        for field in FIELDS {
            assert!(
                !EXCLUDED.contains(&field.key),
                "排除项 {} 不得出现在设置页字段表",
                field.key
            );
        }
        let density = FIELDS
            .iter()
            .find(|field| field.key == "density")
            .expect("密度字段必须存在");
        let SettingsControl::Choice(options) = &density.control else {
            panic!("密度字段必须是选择控件");
        };
        assert_eq!(
            options
                .iter()
                .map(|option| option.value)
                .collect::<Vec<_>>(),
            ["compact", "standard", "loose"]
        );
    }

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
