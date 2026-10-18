//! 壳侧全局 `Settings`:`RuntimeSettings` 的 GUI 消费视图。
//!
//! 值权威在 `slterm-settings`;本结构只做两件事:把宽容解析后的运行时
//! 设置折算成终端/壳直接消费的形态(字体四族、逻辑字号、调色板、滚速),
//! 以及承载 `Changed` 热应用后的全局事实源。设置页写入后经
//! [`Settings::from_runtime`] 重建并 `cx.set_global`——消费方始终读同一份。

use std::path::{Path, PathBuf};

use gpui::Global;

use crate::app_shell::editor_theme::{self, EditorThemeTokens};
use crate::gpui_terminal::colors::Palette;
use crate::i18n::{LanguagePreference, UiLanguage};
use crate::theme_library::ThemeLibraryStore;
use slterm_settings::{BlurMode, RawSettings, RuntimeSettings, ThemeEffects, ThemeUiColors};

/// 默认字号:11.25pt × 4/3 = 15px(与上游配置缺省同链)。
const DEFAULT_FONT_SIZE_PX: f32 = 15.0;
/// 回滚行数(D02-3 裁决值)。
const SCROLLING_HISTORY: usize = 10_000;

/// 应用运行期全局设置;设置页提交后整体替换并扇出到各终端视图。
#[derive(Clone)]
pub struct Settings {
    pub font_family: String,
    pub font_bold_family: String,
    pub font_italic_family: String,
    pub font_bold_italic_family: String,
    /// GPUI 逻辑像素(1pt = 4/3 px @96dpi)。
    pub font_size_px: f32,
    pub ligatures: bool,
    /// 暗色单份(仅暗色定位;D06-3)。
    pub palette: Palette,
    /// 编辑器语义槽(源码态/gutter/diff 的配色单源)。
    pub editor_theme: EditorThemeTokens,
    /// 选区完成即复制。
    pub copy_on_select: bool,
    /// 补全呈现形态(Inline/Popup/Hybrid;设置值本体归 `slterm-settings` 键域)。
    pub completion_style: slterm_settings::CompletionStyleName,
    /// AI 内联补全 ghost 开关。
    pub ghost: bool,
    pub scroll_speed: f32,
    /// 解析后的界面语言(System 档的 locale 协商只在此处发生一次;
    /// 渲染路径读缓存,不再逐帧探测系统 locale)。
    pub language: UiLanguage,
    /// 常驻系统托盘图标:1 Hz 节拍按此值增删图标,
    /// 线程留驻不随开关销毁。
    pub tray: bool,
    /// 静默启动:与 `tray` 合取决定首窗是否隐藏;
    /// 显式 CLI 启动(`--working-directory`/`-e`)恒不静默。
    pub silent_start: bool,
    /// 窗口背景材质(D09-1 五档,默认 None;自定义主题可声明默认,
    /// 用户显式值恒权威)。
    pub blur: BlurMode,
    /// 窗口不透明度:只透壳底色与终端默认背景,文字层恒不透明。
    pub opacity: f32,
    /// 非聚焦 pane 压暗层开关。用户设置域无独立键:自定义主题
    /// `effects.veil` 显式值权威,未声明/无自定义主题/解析失败 = 开启。
    pub veil: bool,
    /// 壁纸路径;None = 无壁纸。加载/缩放/内存预算归 wallpaper 模块。
    pub background_image: Option<PathBuf>,
    /// 壁纸自身透明度,独立于窗口 opacity。
    pub background_image_opacity: f32,
    /// 壁纸 fit/alignment 存原文,解析归渲染层。
    pub background_image_fit: Option<String>,
    pub background_image_alignment: Option<String>,
    /// 壁纸铺满整窗(含侧栏/标题栏)而非仅终端内容区。
    pub background_image_cover_chrome: bool,
    /// 发现新版本后是否允许后台预取安装包(安装授权仍归更新弹窗)。
    pub auto_download_updates: bool,
}

impl Global for Settings {}

impl Default for Settings {
    fn default() -> Self {
        Self::from_runtime(&RuntimeSettings::from_raw(&RawSettings::default()))
    }
}

impl Settings {
    /// 生产入口:读 settings.json 并折算为消费视图。读取失败(权限/IO)
    /// 不把故障升级成启动失败——记录告警后回默认值,损坏三态的主文件
    /// quarantine 已在读通道内完成。
    pub fn load() -> Self {
        match crate::settings_store::load_settings() {
            Ok(result) => {
                if result.corrupted {
                    log::warn!("settings.json 损坏,已回退备份或默认值");
                }
                Self::from_runtime(&RuntimeSettings::from_raw(&result.data))
            }
            Err(error) => {
                log::warn!("读取设置失败,使用默认值: {error}");
                Self::default()
            }
        }
    }

    /// 宽容解析后的运行时设置 → 壳消费视图。字体四族共用同一字族链
    /// (粗体/斜体字族键不在字体键白名单);字号缺省 = 15px;连字 Theme 档
    /// 在主题 typography 落地前按「开」处理(与 `Ligatures::enabled(None)`
    /// 同语义)。
    pub fn from_runtime(runtime: &RuntimeSettings) -> Self {
        Self::from_runtime_with_theme_root(runtime, None)
    }

    /// 同 [`Self::from_runtime`],但主题库根显式指定(测试 tempdir 与
    /// 设置页显式路径注入;生产传 None = 默认数据目录)。
    pub fn from_runtime_with_theme_root(
        runtime: &RuntimeSettings,
        theme_root: Option<&Path>,
    ) -> Self {
        let family = runtime
            .font_family
            .as_deref()
            .map(str::trim)
            .filter(|family| !family.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned());
        let (palette, ui, effects) = resolve_theme(runtime, theme_root);
        let (opacity, blur) = effective_material(runtime, effects.as_ref());
        // 无自定义主题(含解析失败)恒为开启;不伪造 settings.json 键。
        let veil = effects
            .as_ref()
            .and_then(|effects| effects.veil)
            .unwrap_or(true);
        Settings {
            font_family: family.clone(),
            font_bold_family: family.clone(),
            font_italic_family: family.clone(),
            font_bold_italic_family: family,
            font_size_px: runtime.font_size_px.unwrap_or(DEFAULT_FONT_SIZE_PX),
            ligatures: runtime.ligatures.enabled(None),
            editor_theme: editor_theme::editor_theme_tokens(runtime.theme.reviewed_palette(), &ui),
            palette,
            copy_on_select: runtime.copy_on_select,
            completion_style: runtime.completion_style,
            ghost: runtime.ghost,
            scroll_speed: runtime.scroll_speed,
            language: LanguagePreference::from(runtime.language).resolved(),
            tray: runtime.tray,
            silent_start: runtime.silent_start,
            blur,
            opacity,
            veil,
            background_image: runtime.background_image.as_deref().map(PathBuf::from),
            background_image_opacity: runtime.background_image_opacity,
            background_image_fit: runtime.background_image_fit.clone(),
            background_image_alignment: runtime.background_image_alignment.clone(),
            background_image_cover_chrome: runtime.background_image_cover_chrome,
            auto_download_updates: runtime.auto_download_updates,
        }
    }

    /// 静默启动判定:公式单点在 `platform::startup`,
    /// 壳视图只提供两个设置位,不重复实现合取。
    pub fn start_hidden(&self) -> bool {
        crate::platform::startup::start_hidden_for(self.silent_start, self.tray)
    }

    /// 引擎 `Term` 的启动配置。
    ///
    /// ConPTY 关键位照上游钉死(因果链):侧载 ConPTY 的起桥 DA1 已被宿主
    /// 预应答,Term 必须吞掉自己的重复应答(`suppress_bringup_da1`,上游 =
    /// `conpty_sideload_enabled()`——core 已无该读法,Options 默认侧载恒真,
    /// 此处硬编码 true);resize 后 ConPTY 静默重锚主屏,网格必须同语义
    /// (`conpty_resize`);`kitty_keyboard` 开增强键报。
    pub fn term_config(&self) -> slterm_terminal::term::Config {
        slterm_terminal::term::Config {
            scrolling_history: SCROLLING_HISTORY,
            suppress_bringup_da1: true,
            conpty_resize: true,
            kitty_keyboard: true,
            ..Default::default()
        }
    }
}

/// 主题选择与应用流(仅暗色):自定义主题经主题库解析,失败回内置主题;
/// `theme_foreground` 是显式前景覆盖,作用于终端正文。主题解析失败不把
/// 启动/热应用升级成故障——回退内置目录默认(D06-3 的 linear)。
///
/// 返回(终端调色板, UI 语义槽, 自定义主题视效缺省):编辑器的
/// `EditorThemeTokens` 从同一分支装配,主题切换时消费方与终端同步热应用。
/// 视效缺省只用于用户未显式写过的 opacity/blur。
fn resolve_theme(
    runtime: &RuntimeSettings,
    theme_root: Option<&Path>,
) -> (Palette, ThemeUiColors, Option<ThemeEffects>) {
    if let Some(id) = runtime.custom_theme.as_deref() {
        let store = theme_root.map_or_else(ThemeLibraryStore::default, ThemeLibraryStore::new);
        match store.load(id).and_then(|document| {
            document
                .resolve()
                .map_err(crate::theme_library::StoreError::Document)
        }) {
            Ok(definition) => {
                let ui = definition.resolved_ui();
                return (
                    Palette::from_definition(&definition, runtime.theme_foreground),
                    ui,
                    Some(definition.effects),
                );
            }
            Err(error) => {
                log::warn!("自定义主题 {id} 解析失败,回退内置主题: {error}");
            }
        }
    }
    (
        Palette::from_term_theme(runtime.theme.term_theme(), runtime.theme_foreground),
        // 内置主题的 UI 语义槽 = 评审过的 ReviewedPalette 镜像。
        ThemeUiColors::from_palette(runtime.theme.reviewed_palette()),
        None,
    )
}

/// 有效材质:用户显式写过的值(含显式 1.0 / none)恒权威;未写过时
/// 可由自定义主题文档提供缺省,再退运行时默认。
fn effective_material(
    runtime: &RuntimeSettings,
    effects: Option<&ThemeEffects>,
) -> (f32, BlurMode) {
    let opacity = if runtime.opacity_is_explicit() {
        runtime.opacity
    } else {
        effects
            .and_then(|effects| effects.opacity)
            .unwrap_or(runtime.opacity)
    };
    let blur = if runtime.blur_is_explicit() {
        runtime.blur
    } else {
        effects
            .and_then(|effects| effects.blur)
            .unwrap_or(runtime.blur)
    };
    (opacity.clamp(0.0, 1.0), blur)
}

#[cfg(test)]
mod config_tests {
    use super::*;

    fn runtime(json: &str) -> RuntimeSettings {
        RuntimeSettings::from_raw(&RawSettings::from_json_bytes(json.as_bytes()))
    }

    #[test]
    fn absent_settings_fall_back_to_the_m3_constant_defaults() {
        let settings = Settings::default();
        for family in [
            &settings.font_family,
            &settings.font_bold_family,
            &settings.font_italic_family,
            &settings.font_bold_italic_family,
        ] {
            assert_eq!(family, crate::app_shell::assets::REQUIRED_FONT_FAMILY);
        }
        assert_eq!(settings.font_size_px, 15.0);
        assert!(settings.ligatures);
        assert!(!settings.copy_on_select);
        assert_eq!(settings.scroll_speed, 1.0);
    }

    #[test]
    fn runtime_values_feed_the_shell_view() {
        let runtime = runtime(
            r#"{
                "appearance": {"font_family": "Cascadia Mono", "font_size": 18},
                "scrolling": {"scroll_speed": 2.5},
                "interaction": {"ligatures": "off", "copy_on_select": true}
            }"#,
        );
        let settings = Settings::from_runtime(&runtime);
        assert_eq!(settings.font_family, "Cascadia Mono");
        assert_eq!(settings.font_bold_family, "Cascadia Mono");
        assert_eq!(settings.font_size_px, 18.0);
        assert!(!settings.ligatures, "off 档必须关闭连字");
        assert!(settings.copy_on_select);
        assert_eq!(settings.scroll_speed, 2.5);
    }

    #[test]
    fn blank_font_family_keeps_the_bundled_fallback() {
        let runtime = runtime(r#"{"appearance": {"font_family": "   "}}"#);
        let settings = Settings::from_runtime(&runtime);
        assert_eq!(
            settings.font_family,
            crate::app_shell::assets::REQUIRED_FONT_FAMILY,
            "空白字族链必须回内嵌 Maple,不能留空导致方框"
        );
    }

    #[test]
    fn default_palette_resolves_to_the_linear_catalog_entry() {
        // D06-3:默认主题 Linear 的终端色表必须真正进入壳消费视图,
        // 不再是旧静态缺省。
        let settings = Settings::default();
        let linear = slterm_settings::ThemeName::Linear.term_theme();
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8(linear.background)
        );
        let foreground = linear.exact.unwrap().foreground;
        assert_eq!(
            settings.palette.foreground,
            crate::gpui_terminal::colors::from_rgb8(foreground)
        );
    }

    #[test]
    fn theme_selection_and_foreground_override_reach_the_palette() {
        let parsed = runtime(r#"{"appearance": {"theme": "Nord"}}"#);
        let settings = Settings::from_runtime(&parsed);
        let nord = slterm_settings::ThemeName::Nord.term_theme();
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8(nord.background)
        );
        assert_eq!(
            settings.palette.ansi[9],
            crate::gpui_terminal::colors::from_rgb8([0xbf, 0x61, 0x6a])
        );

        let parsed =
            runtime(r##"{"appearance": {"theme": "Nord", "theme_foreground": "#123456"}}"##);
        let settings = Settings::from_runtime(&parsed);
        assert_eq!(
            settings.palette.foreground,
            crate::gpui_terminal::colors::from_rgb8([0x12, 0x34, 0x56]),
            "显式前景覆盖必须作用终端正文"
        );
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8(nord.background),
            "覆盖前景不得改主题背景"
        );
    }

    #[test]
    fn custom_theme_documents_resolve_through_the_library_root() {
        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        let store = crate::theme_library::ThemeLibraryStore::new(&themes);
        let mut definition =
            slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
        definition.name = "Custom".to_owned();
        definition.terminal.background = [0x01, 0x02, 0x03];
        definition.terminal.foreground = [0xd0, 0xd1, 0xd2];
        definition.terminal.palette.colors[16] = [0xaa, 0xbb, 0xcc];
        let document = crate::theme_library::from_definition(&definition).unwrap();
        let stored = store.import(&document).unwrap();
        let id = stored.id().unwrap();

        let parsed = runtime(&format!(
            r#"{{"appearance": {{"theme": "Linear", "custom_theme": "{id}"}}}}"#
        ));
        let settings = Settings::from_runtime_with_theme_root(&parsed, Some(&themes));
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8([0x01, 0x02, 0x03])
        );
        assert_eq!(
            settings.palette.foreground,
            crate::gpui_terminal::colors::from_rgb8([0xd0, 0xd1, 0xd2])
        );
        // 完整 256 色表进入 indexed 覆盖(OSC 4/渲染同一来源)。
        let indexed = settings
            .palette
            .indexed
            .iter()
            .find(|(index, _)| *index == 16)
            .map(|(_, color)| *color);
        assert_eq!(
            indexed,
            Some(crate::gpui_terminal::colors::from_rgb8([0xaa, 0xbb, 0xcc]))
        );

        // 缺失/损坏的自定义主题回退内置主题,不炸启动。
        let missing = runtime(r#"{"appearance": {"custom_theme": "nope"}}"#);
        let settings = Settings::from_runtime_with_theme_root(&missing, Some(&themes));
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8(
                slterm_settings::ThemeName::Linear.term_theme().background
            )
        );
    }

    /// 静默启动两条件进入壳视图:silent_start 单独不隐藏(无托盘 =
    /// 不可达进程),两开才成立。
    #[test]
    fn silent_start_reaches_the_shell_view_behind_the_tray_condition() {
        let both = Settings::from_runtime(&runtime(
            r#"{"session":{"silent_start":true},"system":{"tray":true}}"#,
        ));
        assert!(both.silent_start);
        assert!(both.tray);
        assert!(both.start_hidden());

        let no_tray = Settings::from_runtime(&runtime(
            r#"{"session":{"silent_start":true},"system":{"tray":false}}"#,
        ));
        assert!(no_tray.silent_start);
        assert!(!no_tray.tray);
        assert!(!no_tray.start_hidden(), "托盘关闭时不得静默启动");

        let no_silent = Settings::from_runtime(&runtime(
            r#"{"session":{"silent_start":false},"system":{"tray":true}}"#,
        ));
        assert!(!no_silent.start_hidden());
    }

    /// D09-1:主题文档的视效缺省只填用户没写过的键;显式值(含 1.0/none)
    /// 保持权威。
    #[test]
    fn custom_theme_effects_fill_only_unset_material_values() {
        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        let store = crate::theme_library::ThemeLibraryStore::new(&themes);
        let mut definition =
            slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
        definition.name = "Effects".to_owned();
        definition.effects.opacity = Some(0.72);
        definition.effects.blur = Some(BlurMode::Mica);
        let document = crate::theme_library::from_definition(&definition).unwrap();
        let stored = store.import(&document).unwrap();
        let id = stored.id().unwrap();

        let inherited = runtime(&format!(r#"{{"appearance": {{"custom_theme": "{id}"}}}}"#));
        let settings = Settings::from_runtime_with_theme_root(&inherited, Some(&themes));
        assert!((settings.opacity - 0.72).abs() < 1e-6, "缺省继承主题视效");
        assert_eq!(settings.blur, BlurMode::Mica);

        let explicit = runtime(&format!(
            r#"{{"appearance": {{"custom_theme": "{id}", "opacity": 1.0, "blur": "none"}}}}"#
        ));
        let settings = Settings::from_runtime_with_theme_root(&explicit, Some(&themes));
        assert!((settings.opacity - 1.0).abs() < 1e-6, "显式 1.0 必须权威");
        assert_eq!(settings.blur, BlurMode::None, "显式 none 必须权威");
    }

    /// C7:veil 无用户设置键;自定义主题显式值权威,其余路径恒开启。
    #[test]
    fn pane_veil_defaults_on_and_follows_the_custom_theme_switch() {
        assert!(Settings::default().veil, "内置目录默认开启");
        let builtin = runtime(r#"{"appearance": {"theme": "Nord"}}"#);
        assert!(
            Settings::from_runtime(&builtin).veil,
            "内置主题不提供 veil 覆盖"
        );

        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        let store = crate::theme_library::ThemeLibraryStore::new(&themes);
        let import = |name: &str, veil: Option<bool>| {
            let mut definition =
                slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
            definition.name = name.to_owned();
            definition.effects.veil = veil;
            let document = crate::theme_library::from_definition(&definition).unwrap();
            store.import(&document).unwrap().id().unwrap().to_owned()
        };
        let off_id = import("Veil Off", Some(false));
        let default_id = import("Veil Default", None);

        let off = runtime(&format!(
            r#"{{"appearance": {{"custom_theme": "{off_id}"}}}}"#
        ));
        assert!(
            !Settings::from_runtime_with_theme_root(&off, Some(&themes)).veil,
            "主题显式关闭必须生效"
        );
        let default = runtime(&format!(
            r#"{{"appearance": {{"custom_theme": "{default_id}"}}}}"#
        ));
        assert!(
            Settings::from_runtime_with_theme_root(&default, Some(&themes)).veil,
            "主题未声明 veil 时回默认开启"
        );

        // 缺失主题(解析失败)不得把启动/热应用升级成故障,veil 回默认开启。
        let missing = runtime(r#"{"appearance": {"custom_theme": "nope"}}"#);
        assert!(
            Settings::from_runtime_with_theme_root(&missing, Some(&themes)).veil,
            "解析失败回默认开启"
        );
    }

    #[test]
    fn term_config_carries_the_anchored_scrollback() {
        // D02-3:回滚 10000 行是产品裁决值,不是库默认巧合——钉字面量。
        assert_eq!(Settings::default().term_config().scrolling_history, 10000);
    }

    #[test]
    fn term_config_pins_the_conpty_and_keyboard_flags() {
        // 上游同链三键:侧载 DA1 预应答 / ConPTY 网格同语义 / kitty 增强键报。
        let config = Settings::default().term_config();
        assert!(config.suppress_bringup_da1);
        assert!(config.conpty_resize);
        assert!(config.kitty_keyboard);
    }
}
