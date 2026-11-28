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
use slterm_settings::{
    AcceptKeyName, BellModeName, BlurMode, RawSettings, RuntimeSettings, ThemeUiColors,
};

/// 默认字号：行为真值 14 逻辑像素。
const DEFAULT_FONT_SIZE_PX: f32 = 14.0;

static REDUCED_MOTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 启动链在读取设置前写入系统动画偏好；测试与未初始化路径默认不减少动效。
pub(crate) fn set_reduced_motion(reduced: bool) {
    REDUCED_MOTION.store(reduced, std::sync::atomic::Ordering::Relaxed);
}

/// 闪烁默认值：用户显式设置优先；否则跟随系统减少动效。
fn cursor_blink_enabled(explicit: Option<bool>, reduced_motion: bool) -> bool {
    explicit.unwrap_or(!reduced_motion)
}

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
    /// 暗色单份(仅暗色定位;)。
    pub palette: Palette,
    /// 编辑器语义槽(源码态/gutter/diff 的配色单源)。
    pub editor_theme: EditorThemeTokens,
    /// 光标闪烁；形状固定竖线。系统减少动效时默认关闭，用户显式设置优先。
    pub cursor_blink: bool,
    /// 新建终端的历史容量；只影响之后创建的逻辑终端。
    pub scrollback_lines: usize,
    /// 默认 Shell 的用户覆盖；None = 启动时按真实探测链确定。
    pub shell: Option<String>,
    /// 启动目录；无 CLI 指定根时作为首个项目根。
    pub startup_directory: Option<PathBuf>,
    /// ghost 余量的接受键（Right／Tab／Both）。
    pub accept: AcceptKeyName,
    /// BEL 行为：视觉通知与系统响声两条通道。
    pub bell: BellModeName,
    /// 新进程是否刷新注册表环境；关 = 继承启动进程环境。
    pub refresh_environment: bool,
    /// 新终端是否写入 Windows 系统代理环境变量。
    pub terminal_proxy: bool,
    /// ConPTY 输入模式矩阵；字段语义锚定在 `slterm_terminal`。
    pub conpty_input_modes: slterm_terminal::tty::ConptyInputModes,
    /// 选区完成即复制。
    pub copy_on_select: bool,
    /// 全局 Document 自动保存；视图层不得另设开关。
    pub document_autosave: bool,
    /// Diff 共享编辑重算间隔；连续编辑在此窗口内合并。
    pub diff_refresh_interval_ms: u64,
    /// 活跃项目内非聚焦终端的 OSC 52 写入授权；隐藏项目不受影响。
    pub osc52_unfocused: bool,
    /// 危险控制字符或超过 50 行的粘贴是否需要显式确认。
    pub multiline_paste_confirm: bool,
    /// 重启或重连时是否恢复已持久化的 Agent 会话。
    pub resume_ai: bool,
    /// 补全呈现形态(Inline/Popup/Hybrid;设置值本体归 `slterm-settings` 键域)。
    pub completion_style: slterm_settings::CompletionStyleName,
    /// AI 内联补全 ghost 开关。
    pub ghost: bool,
    /// 是否采集命令历史；与补全显示独立。
    pub learn_commands: bool,
    /// 是否采集目录历史；与命令采集独立。
    pub learn_directories: bool,
    pub scroll_speed: f32,
    /// 解析后的界面语言(System 档的 locale 协商只在此处发生一次;
    /// 渲染路径读缓存,不再逐帧探测系统 locale)。
    pub language: UiLanguage,
    /// 常驻系统托盘图标:1 Hz 节拍按此值增删图标,
    /// 线程留驻不随开关销毁。
    pub tray: bool,
    /// 系统桌面通知总开关:只控制系统 toast,应用内反馈仍显示。
    pub desktop_notifications: bool,
    /// 失焦时任务栏闪烁:与桌面通知独立判定。
    pub taskbar_flash: bool,
    /// 静默启动:与 `tray` 合取决定首窗是否隐藏;
    /// 显式 CLI 目录启动恒不静默。
    pub silent_start: bool,
    /// 窗口背景材质(档位,默认 None;自定义主题可声明默认,
    /// 用户显式值恒权威)。
    pub blur: BlurMode,
    /// 窗口不透明度:只透壳底色与终端默认背景,文字层恒不透明。
    pub opacity: f32,
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
    #[allow(dead_code)] // 更新协调器接入后消费。
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
    /// 启动加载同时返回用户可见降级事实；只读取一次，避免二次读盘丢失损坏分类。
    pub fn load_with_warning() -> (Self, Option<crate::settings_state::SettingsLoadWarning>) {
        match crate::settings_store::load_settings() {
            Ok(result) => {
                let warning = if let Some(version) = result.unsupported_version {
                    log::warn!(
                        "settings.json 版本 {version} 高于当前程序支持；使用默认值且不会覆盖原文件"
                    );
                    Some(crate::settings_state::SettingsLoadWarning::UnsupportedVersion(version))
                } else if result.corrupted {
                    log::warn!("settings.json 损坏,已回退备份或默认值");
                    Some(crate::settings_state::SettingsLoadWarning::Corrupted)
                } else {
                    None
                };
                // 启动是冷路径：允许读盘解析主题并预载缓存。
                let settings = Self::from_runtime_with_theme_root(
                    &RuntimeSettings::from_raw(&result.data),
                    None,
                );
                (settings, warning)
            }
            Err(error) => {
                log::warn!("读取设置失败,使用默认值: {error}");
                (
                    Self::default(),
                    Some(crate::settings_state::SettingsLoadWarning::ReadFailed),
                )
            }
        }
    }

    /// 热应用入口:宽容解析后的运行时设置 → 壳消费视图,自定义主题只查
    /// 预载缓存。字体四族共用同一字族链(粗体/斜体字族键不在字体键
    /// 白名单);字号缺省 = 15px,连字按「开」处理。
    pub fn from_runtime(runtime: &RuntimeSettings) -> Self {
        let (palette, ui) = resolve_theme_cached(runtime);
        Self::snapshot(runtime, palette, ui)
    }

    /// 冷路径:显式主题库根解析自定义主题并预载缓存。启动入口与测试
    /// 注入使用;热应用必须走 [`Self::from_runtime`] 的缓存查询。
    pub fn from_runtime_with_theme_root(
        runtime: &RuntimeSettings,
        theme_root: Option<&Path>,
    ) -> Self {
        let (palette, ui) = resolve_theme_cold(runtime, theme_root);
        Self::snapshot(runtime, palette, ui)
    }

    /// 后台重读设置后的主题预载：只解析并安装缓存，不构造消费视图。
    pub(crate) fn preload_theme_cold(runtime: &RuntimeSettings) {
        let _ = resolve_theme_cold(runtime, None);
    }

    fn snapshot(runtime: &RuntimeSettings, mut palette: Palette, ui: ThemeUiColors) -> Self {
        // 终端背景覆盖色优先于主题内置背景；前景覆盖已在 Palette 构造内消费。
        if let Some(background) = runtime.background {
            palette.background = crate::gpui_terminal::colors::from_rgb8(background);
        }
        let family = runtime
            .font_family
            .as_deref()
            .map(str::trim)
            .filter(|family| !family.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned());
        // 材质只由用户设置拥有;主题不再提供材质或视效缺省。
        let opacity = runtime.opacity.clamp(0.0, 1.0);
        let blur = runtime.blur;
        Settings {
            font_family: family.clone(),
            font_bold_family: family.clone(),
            font_italic_family: family.clone(),
            font_bold_italic_family: family,
            font_size_px: runtime.font_size_px.unwrap_or(DEFAULT_FONT_SIZE_PX),
            ligatures: runtime.ligatures.enabled(None),
            editor_theme: editor_theme::editor_theme_tokens(runtime.theme.reviewed_palette(), &ui),
            palette,
            cursor_blink: cursor_blink_enabled(
                runtime.cursor_blink,
                REDUCED_MOTION.load(std::sync::atomic::Ordering::Relaxed),
            ),
            scrollback_lines: runtime.scrollback_lines,
            shell: runtime.shell.clone(),
            startup_directory: runtime.startup_directory.as_deref().map(PathBuf::from),
            accept: runtime.accept,
            bell: runtime.bell,
            refresh_environment: runtime.refresh_environment,
            terminal_proxy: runtime.terminal_proxy,
            conpty_input_modes: slterm_terminal::tty::ConptyInputModes {
                inherit_cursor: runtime.conpty_input_modes.inherit_cursor,
                resize_quirk: runtime.conpty_input_modes.resize_quirk,
                win32_input_mode: runtime.conpty_input_modes.win32_input_mode,
                passthrough_mode: runtime.conpty_input_modes.passthrough_mode,
            },
            copy_on_select: runtime.copy_on_select,
            document_autosave: runtime.document_autosave,
            diff_refresh_interval_ms: runtime.diff_refresh_interval_ms as u64,
            osc52_unfocused: runtime.osc52_unfocused,
            multiline_paste_confirm: runtime.multiline_paste_confirm,
            resume_ai: runtime.resume_ai,
            completion_style: runtime.completion_style,
            ghost: runtime.ghost,
            learn_commands: runtime.learn_commands,
            learn_directories: runtime.learn_directories,
            scroll_speed: runtime.scroll_speed,
            language: LanguagePreference::from(runtime.language).resolved(),
            tray: runtime.tray,
            desktop_notifications: runtime.desktop_notifications,
            taskbar_flash: runtime.taskbar_flash,
            silent_start: runtime.silent_start,
            blur,
            opacity,
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
    /// ConPTY resize 后会静默重锚主屏，网格必须保持同语义
    /// (`conpty_resize`)；`kitty_keyboard` 开增强键报。
    pub fn term_config(&self) -> slterm_terminal::term::Config {
        use slterm_terminal::ansi::{CursorShape, CursorStyle};
        slterm_terminal::term::Config {
            scrolling_history: self.scrollback_lines,
            conpty_resize: true,
            kitty_keyboard: true,
            default_cursor_style: CursorStyle {
                shape: CursorShape::Beam,
                blinking: self.cursor_blink,
            },
            ..Default::default()
        }
    }
}

/// 热应用解析:只查进程内预载缓存,任何字段提交都不读主题文件。
/// 未预载时回退内置主题,等待冷路径安装后再次重建快照。
fn resolve_theme_cached(runtime: &RuntimeSettings) -> (Palette, ThemeUiColors) {
    if let Some(id) = runtime.custom_theme.as_deref() {
        if let Some(definition) = crate::theme_library::cache::get(id) {
            let ui = definition.resolved_ui();
            return (
                Palette::from_definition(&definition, runtime.theme_foreground),
                ui,
            );
        }
        log::warn!("自定义主题 {id} 尚未预载,暂用内置主题");
    }
    resolve_builtin(runtime)
}

/// 冷路径解析:启动与显式主题库操作允许读盘;成功即安装缓存。
/// 解析失败不把启动/热应用升级成故障——回退内置目录默认(的 linear)。
fn resolve_theme_cold(
    runtime: &RuntimeSettings,
    theme_root: Option<&Path>,
) -> (Palette, ThemeUiColors) {
    if let Some(id) = runtime.custom_theme.as_deref() {
        let store = theme_root.map_or_else(ThemeLibraryStore::default, ThemeLibraryStore::new);
        match store.load(id).and_then(|document| {
            document
                .resolve()
                .map_err(crate::theme_library::StoreError::Document)
        }) {
            Ok(definition) => {
                crate::theme_library::cache::install(id, definition.clone());
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
    resolve_builtin(runtime)
}

fn resolve_builtin(runtime: &RuntimeSettings) -> (Palette, ThemeUiColors) {
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
    fn learning_collection_defaults_are_enabled_and_independent() {
        let defaults = Settings::default();
        assert!(defaults.learn_commands);
        assert!(defaults.learn_directories);
        let configured =
            runtime(r#"{"interaction":{"learn_commands":false,"learn_directories":true}}"#);
        let settings = Settings::from_runtime(&configured);
        assert!(!settings.learn_commands);
        assert!(settings.learn_directories);
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
        assert_eq!(settings.font_size_px, 14.0);
        assert!(settings.ligatures);
        assert!(!settings.copy_on_select);
        assert_eq!(settings.scroll_speed, 1.0);
    }

    #[test]
    fn runtime_values_feed_the_shell_view() {
        let runtime = runtime(
            r#"{
                "appearance": {"font_family": "Cascadia Mono", "font_size": 18, "background": "112233"},
                "scrolling": {"scroll_speed": 2.5},
                "interaction": {"ligatures": "off", "copy_on_select": true, "document_autosave": true, "accept": "right", "bell": "visual"},
                "session": {"startup_directory": "C:\\work"},
                "system": {"terminal_proxy": true, "refresh_environment": false},
                "conpty_input_modes": {"passthrough_mode": true, "win32_input_mode": false}
            }"#,
        );
        let settings = Settings::from_runtime(&runtime);
        assert_eq!(settings.font_family, "Cascadia Mono");
        assert_eq!(settings.font_bold_family, "Cascadia Mono");
        assert_eq!(settings.font_size_px, 18.0);
        assert!(!settings.ligatures, "off 档必须关闭连字");
        assert!(settings.copy_on_select);
        assert!(settings.document_autosave);
        assert_eq!(settings.scroll_speed, 2.5);
        assert_eq!(settings.accept, slterm_settings::AcceptKeyName::Right);
        assert_eq!(settings.bell, slterm_settings::BellModeName::Visual);
        assert_eq!(
            settings.startup_directory.as_deref(),
            Some(std::path::Path::new(r"C:\work"))
        );
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8([0x11, 0x22, 0x33]),
            "背景覆盖色必须进入终端调色板"
        );
        assert!(settings.terminal_proxy);
        assert!(!settings.refresh_environment);
        assert!(settings.conpty_input_modes.passthrough_mode);
        assert!(!settings.conpty_input_modes.win32_input_mode);
        assert!(settings.conpty_input_modes.inherit_cursor, "缺字段取默认");
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
        // 默认主题 Linear 的终端色表必须真正进入壳消费视图,
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
    fn integration_custom_theme() {
        let directory = tempfile::tempdir().unwrap();
        let themes = directory.path().join("themes");
        let store = crate::theme_library::ThemeLibraryStore::new(&themes);
        let mut definition =
            slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
        definition.name = "Custom".to_owned();
        definition.terminal.background = [0x01, 0x02, 0x03];
        definition.terminal.foreground = [0xd0, 0xd1, 0xd2];
        definition.terminal.ansi[4] = [0xaa, 0xbb, 0xcc];
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
        // ANSI 槽位进入实例调色板；16–255 由 xterm 公式派生，不随主题保存。
        assert_eq!(
            settings.palette.ansi[4],
            crate::gpui_terminal::colors::from_rgb8([0xaa, 0xbb, 0xcc])
        );
        let derived = settings.palette.resolve(
            slterm_terminal::ansi::Color::Indexed(16),
            &Default::default(),
            false,
        );
        assert_eq!(
            derived,
            crate::gpui_terminal::colors::from_rgb8(slterm_settings::xterm_indexed_color(16))
        );

        // 使用中的自定义主题删除必须先另选。
        assert_eq!(
            crate::theme_library::resolve_theme_deletion(Some(id), id),
            crate::theme_library::ThemeDeletion::RequiresReplacement
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

    /// 热应用只查预载缓存：任何字段提交都不得在 GPUI 前台读主题文件。
    #[test]
    fn hot_snapshot_uses_the_cached_custom_theme_definition_without_disk_io() {
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
        let mut definition =
            slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
        definition.terminal.background = [0x11, 0x22, 0x33];
        crate::theme_library::cache::install(id, definition);
        let parsed = runtime(&format!(r#"{{"appearance": {{"custom_theme": "{id}"}}}}"#));
        let settings = Settings::from_runtime(&parsed);
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8([0x11, 0x22, 0x33])
        );
        // 改其他字段再次重建快照：仍然只读缓存，不重新解析主题文件。
        let changed = runtime(&format!(
            r#"{{"appearance": {{"custom_theme": "{id}", "scroll_speed": 1.5}}}}"#
        ));
        let settings = Settings::from_runtime(&changed);
        assert_eq!(
            settings.palette.background,
            crate::gpui_terminal::colors::from_rgb8([0x11, 0x22, 0x33])
        );
        crate::theme_library::cache::invalidate();
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

    #[test]
    fn term_config_carries_the_anchored_scrollback() {
        // 回滚 10000 行是产品裁决值,不是库默认巧合——钉字面量。
        assert_eq!(Settings::default().term_config().scrolling_history, 10000);
        let settings = Settings {
            scrollback_lines: 5000,
            ..Settings::default()
        };
        assert_eq!(settings.term_config().scrolling_history, 5000);
    }

    #[test]
    fn cursor_blink_prefers_the_explicit_setting_over_reduced_motion() {
        assert!(cursor_blink_enabled(None, false));
        assert!(!cursor_blink_enabled(None, true));
        assert!(cursor_blink_enabled(Some(true), true));
        assert!(!cursor_blink_enabled(Some(false), false));
    }

    #[test]
    fn term_config_uses_a_beam_cursor_with_the_effective_blink() {
        use slterm_terminal::ansi::CursorShape;

        let settings = Settings {
            cursor_blink: false,
            ..Settings::default()
        };
        let config = settings.term_config();
        assert_eq!(config.default_cursor_style.shape, CursorShape::Beam);
        assert!(!config.default_cursor_style.blinking);

        let settings = Settings {
            cursor_blink: true,
            ..Settings::default()
        };
        assert!(settings.term_config().default_cursor_style.blinking);
    }

    #[test]
    fn term_config_pins_the_conpty_and_keyboard_flags() {
        // ConPTY 网格同语义与 kitty 增强键报由壳显式开启。
        let config = Settings::default().term_config();
        assert!(config.conpty_resize);
        assert!(config.kitty_keyboard);
    }
}
