//! 壳侧全局 `Settings`(13 篇锚点):`RuntimeSettings` 的 GUI 消费视图。
//!
//! 值权威在 `slterm-settings`;本结构只做两件事:把宽容解析后的运行时
//! 设置折算成终端/壳直接消费的形态(字体四族、逻辑字号、调色板、滚速),
//! 以及承载 `Changed` 热应用后的全局事实源。设置页写入后经
//! [`Settings::from_runtime`] 重建并 `cx.set_global`——消费方始终读同一份。

use std::path::Path;

use gpui::Global;

use crate::app_shell::editor_theme::{self, EditorThemeTokens};
use crate::gpui_terminal::colors::Palette;
use crate::i18n::{LanguagePreference, UiLanguage};
use crate::theme_library::ThemeLibraryStore;
use slterm_settings::{RawSettings, RuntimeSettings, ThemeUiColors};

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
    /// 编辑器语义槽(源码态/gutter/diff 的配色单源,07 篇改造节 8)。
    pub editor_theme: EditorThemeTokens,
    /// 选区完成即复制。
    pub copy_on_select: bool,
    /// 补全呈现形态(08 篇 M9.5:Inline/Popup/Hybrid,本体归 06 键域)。
    pub completion_style: slterm_settings::CompletionStyleName,
    /// AI 内联补全 ghost 开关。
    pub ghost: bool,
    pub scroll_speed: f32,
    /// 解析后的界面语言(System 档的 locale 协商只在此处发生一次;
    /// 渲染路径读缓存,不再逐帧探测系统 locale)。
    pub language: UiLanguage,
    /// 常驻系统托盘图标(09 篇 M10.3):1 Hz 节拍按此值增删图标,
    /// 线程留驻不随开关销毁。
    pub tray: bool,
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
    /// (粗体/斜体字族键不在本篇白名单);字号缺省 = 15px;连字 Theme 档
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
        let (palette, ui) = resolve_theme(runtime, theme_root);
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
        }
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
/// 返回(终端调色板, UI 语义槽):编辑器的 `EditorThemeTokens` 从同一
/// 分支装配,主题切换时三消费方与终端同步热应用。
fn resolve_theme(runtime: &RuntimeSettings, theme_root: Option<&Path>) -> (Palette, ThemeUiColors) {
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
    )
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
