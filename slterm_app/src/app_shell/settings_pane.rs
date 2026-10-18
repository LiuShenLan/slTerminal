//! GUI 设置页:左导航 + 右表单,路由式搜索与统一持久化入口。
//!
//! 薄壳纪律:本页不定义设置语义——键名、值域、出厂默认、宽容解析全部
//! 在 `slterm-settings`;写盘一律经 `settings_store::save_settings_blocking`
//! (测试注入路径走同函数的 `_at` 形态;白名单校验 → 段级浅合并 → 原子写
//! → .bak)。
//!
//! 三纪律(06 篇跨领域合同):
//! - 预览不脏盘:滑条拖动只改内存态(本页 `runtime` + 全局 `Settings`),
//!   松手才归一化提交;
//! - 提交可回滚:写失败时用最近一次成功读盘的 `persisted` 快照复位控件;
//! - 失败可感知:stderr 记录 + 语义状态条 + 窗口通知。
//!
//! 值/显示分离:固定下拉的显示文案单点在 `localization`;语言切换只刷新
//! 显示项与占位符,不重建控件实体(重建会丢焦点/编辑值/undo/订阅)。

mod agents;
mod design;
mod localization;
mod navigation;
mod pages;
mod persistence;
pub(crate) mod registry;
mod theme_editor;

/// 主题编辑器颜色槽:测试与后续取色面板的统一入口。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) use theme_editor::ThemeColorSlot;

use std::io;
use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, PathPromptOptions,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::select::{SelectEvent, SelectState};
use gpui_component::slider::{SliderEvent, SliderState};
use gpui_component::{ActiveTheme as _, Icon, IconName, v_flex};
use slterm_settings::json::Value;
use slterm_settings::keys::KEYBIND_SEGMENT;
use slterm_settings::{
    BellModeName, BlurMode, CellWidthModeName, CompletionStyleName, Ligatures,
    NotificationDuration, RuntimeSettings, TabRevealName, ThemeName, VcsDisplayName,
};

use crate::app_shell::config::Settings;
use crate::app_shell::workspace::keyboard_bindings;
use crate::i18n::Message;
use crate::theme_library::package::{Author, CheckedPackage, export_package};
use design::{
    SETTINGS_GROUP_GAP, SETTINGS_NAV_ICON_SIZE, SETTINGS_NAV_ROW_HEIGHT, SETTINGS_NAV_WIDTH,
};
use localization::{
    UiLanguage, localized_input_placeholder, localized_select_labels, resolve_language,
};
use navigation::{
    matching_sections, section_icon, section_ids, section_label, visible_nav_sections,
};

/// 共享下拉实体类型(值与显示分离由 `localized_select_labels` 提供)。
type SharedSelect = Entity<SelectState<Vec<SharedString>>>;

/// 壁纸适应模式持久化值;顺序即下拉索引,解析语义归
/// `wallpaper::image_layout`(默认 `uniform_to_fill`)。
const WALLPAPER_FIT_VALUES: [&str; 4] = ["fill", "uniform", "uniform_to_fill", "none"];
/// 壁纸九宫格对齐持久化值;顺序即下拉索引(默认 `center`)。
const WALLPAPER_ALIGNMENT_VALUES: [&str; 9] = [
    "top-left",
    "top",
    "top-right",
    "left",
    "center",
    "right",
    "bottom-left",
    "bottom",
    "bottom-right",
];
/// 主题包导出元数据(作者 = 品牌名,版本 = 应用版本,许可证 = GPL-3.0)。
const THEME_PACKAGE_AUTHOR: &str = crate::brand::NAME;
const THEME_PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
const THEME_PACKAGE_LICENSE: &str = "GPL-3.0";

/// 一个固定下拉:键、实体、稳定值表(选中索引 → 持久化值)。
struct SelectEntry {
    key: &'static str,
    select: SharedSelect,
    values: &'static [&'static str],
}

/// 宿主订阅事件:设置已写盘(热应用)/ 导出工作区 / 请求关闭设置 tab。
pub(crate) enum SettingsPaneEvent {
    Changed,
    /// 导出工作区到指定路径;窗口态采集与写盘归宿主。
    ExportWorkspace {
        path: PathBuf,
    },
    Close,
}

impl EventEmitter<SettingsPaneEvent> for SettingsPane {}

/// 操作结果语义(状态条按成功/失败渲染,不裸出错误码)。
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SettingsStatus {
    /// 配置文件损坏:已回退 .bak/默认值,原件隔离保留。
    LoadedCorrupted,
    /// 恢复默认成功;`backup` = 原设置备份路径(可告知用户)。
    Reset { backup: Option<PathBuf> },
    /// 写盘/读取失败。
    Failed(String),
    /// 主题包导入成功;`name` = 库内消歧后的显示名。
    ThemeImported { name: String },
    /// 主题包导出成功;`path` = 成品输出路径。
    ThemeExported { path: PathBuf },
    /// 主题包导入失败(校验/安装原因)。
    ThemeImportFailed(String),
    /// 主题包导出失败(读取主题/写包原因)。
    ThemeExportFailed(String),
}

impl SettingsStatus {
    pub(crate) fn is_error(&self) -> bool {
        matches!(
            self,
            Self::LoadedCorrupted
                | Self::Failed(_)
                | Self::ThemeImportFailed(_)
                | Self::ThemeExportFailed(_)
        )
    }

    pub(crate) fn text(&self, language: UiLanguage) -> String {
        match self {
            Self::LoadedCorrupted => language.text(Message::SettingsStatusCorrupted).to_owned(),
            Self::Reset { backup: Some(path) } => {
                let path = path.display().to_string();
                language.format(Message::SettingsStatusResetBackup, &[("path", &path)])
            }
            Self::Reset { backup: None } => {
                language.text(Message::SettingsStatusResetDone).to_owned()
            }
            Self::Failed(error) => {
                language.format(Message::SettingsStatusOperationFailed, &[("error", error)])
            }
            Self::ThemeImported { name } => {
                language.format(Message::SettingsStatusThemeImported, &[("name", name)])
            }
            Self::ThemeExported { path } => {
                let path = path.display().to_string();
                language.format(Message::SettingsStatusThemeExported, &[("path", &path)])
            }
            Self::ThemeImportFailed(error) => language.format(
                Message::SettingsStatusThemeImportFailed,
                &[("error", error)],
            ),
            Self::ThemeExportFailed(error) => language.format(
                Message::SettingsStatusThemeExportFailed,
                &[("error", error)],
            ),
        }
    }
}

/// 设置页壳。
pub(crate) struct SettingsPane {
    pub(crate) focus_handle: FocusHandle,
    /// 本页读写目标(生产 = 默认数据目录;测试注入 tempdir 路径)。
    settings_target: SettingsTarget,
    /// hooks 页数据源/安装权威目标(生产 = 03 篇默认解析;测试注入假 home)。
    hooks_target: agents::HooksTarget,
    /// Agent 组 hooks 页状态(inspect 行 + 编辑文档)。
    agents: agents::AgentSettingsState,
    /// 渲染与预览的单一事实源;每次成功写盘后整体重载。
    runtime: RuntimeSettings,
    /// 解析后的渲染语言缓存:System 档的 locale 探测只在运行时重载点发生,
    /// 渲染路径(`language()`)零系统调用(M6.2 逐帧探测问题收口)。
    language: UiLanguage,
    /// 最近一次成功读盘的快照——写失败回滚的"已存值"。
    persisted: RuntimeSettings,
    /// 当前分区下标(注册表顺序)。
    active_section: usize,
    /// 搜索前的分区;清空查询时回到原页。
    search_origin: Option<usize>,
    search_input: Entity<InputState>,
    selects: Vec<SelectEntry>,
    scroll_speed_slider: Entity<SliderState>,
    opacity_slider: Entity<SliderState>,
    font_family_input: Entity<InputState>,
    font_size_input: Entity<InputState>,
    startup_directory_input: Entity<InputState>,
    status: Option<SettingsStatus>,
    /// 恢复默认的二次确认(第一次点击只武装)。
    reset_armed: bool,
    /// A2 写相位机:设置类写盘全部后台执行,前台只入队与消费结果。
    persist_queue: std::collections::VecDeque<persistence::PersistJob>,
    persist_running: bool,
    /// B57:拖动中的实时透明度;后台回读不得覆盖预览值。
    live_opacity: Option<f32>,
    /// 壁纸 fit/alignment 下拉(独立于通用 `selects`:映射与本地化在此)。
    wallpaper_fit_select: SharedSelect,
    wallpaper_alignment_select: SharedSelect,
    /// 壁纸下拉标签的渲染语言缓存(语言热切换时只换显示项)。
    wallpaper_select_language: UiLanguage,
    /// 壁纸不透明度滑杆(0–100 显示域;持久化域 0–1)。
    wallpaper_opacity_slider: Entity<SliderState>,
    /// 设置页壁纸缩略图:后台解码,前台只持有纹理与原始布局尺寸。
    wallpaper_preview: Option<crate::app_shell::wallpaper::WallpaperPreview>,
    /// 预览对应的已确认设置路径;与运行时路径不一致时触发重载。
    wallpaper_preview_path: Option<PathBuf>,
    /// 预览载入代际:后发请求赢,旧结果直接丢弃。
    wallpaper_preview_generation: u64,
    wallpaper_preview_loading: bool,
    /// 壁纸可读错误(路径不存在/解码失败);有值时页面内联展示。
    wallpaper_error: Option<String>,
    /// B19:主题包导入/导出忙态(防重复点击)。
    theme_import_busy: bool,
    theme_export_busy: bool,
    /// 主题编辑会话(外观页子视图;关闭即丢弃草稿)。
    theme_editor: Option<theme_editor::ThemeEditor>,
    /// 键位录制焦点:进入录制态时取得键盘焦点,下一次 KeyDown 在此消费。
    keybind_focus: FocusHandle,
    /// 正在录制的动作名;None = 非录制态。
    keybind_recording: Option<String>,
    /// 键位编辑拒绝原因(保留键/非法组合);成功写回即清除。
    keybind_error: Option<String>,
    /// 用户 `keybind` 段全表(行序即优先级);每次编辑后随写回更新。
    keybind_pairs: Vec<(String, String)>,
    _subscriptions: Vec<Subscription>,
}

/// 设置页读写目标。生产走 `settings_store` 的默认入口;测试注入显式
/// 路径,绝不触碰真实数据目录(11 篇隔离纪律)。
#[derive(Clone)]
enum SettingsTarget {
    Default,
    Path(PathBuf),
}

impl SettingsTarget {
    /// 生产默认目标:开机启动显示需以真实 Startup `.lnk` 为准;测试注入
    /// 路径绝不读真实用户目录。
    fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }

    fn load(&self) -> io::Result<crate::settings_store::LoadResult> {
        match self {
            Self::Default => crate::settings_store::load_settings(),
            Self::Path(path) => crate::settings_store::load_settings_at(path),
        }
    }

    fn save(&self, updates: &[(&'static str, Value)]) -> io::Result<()> {
        match self {
            Self::Default => crate::settings_store::save_settings_blocking(updates),
            Self::Path(path) => crate::settings_store::save_settings_blocking_at(path, updates),
        }
    }

    /// 条件保存(主题应用路径):携带编辑动作前观察到的字节快照。
    fn save_checked(
        &self,
        expected: &crate::settings_store::SettingsRevision,
        updates: &[(&'static str, Value)],
    ) -> io::Result<()> {
        match self {
            Self::Default => {
                crate::settings_store::save_settings_checked(expected, updates).map(|_| ())
            }
            Self::Path(path) => {
                crate::settings_store::save_settings_checked_at(path, expected, updates).map(|_| ())
            }
        }
    }

    fn reset(&self) -> io::Result<Option<PathBuf>> {
        // B3:重置与保存共用 SAVE_LOCK,消除「重置后并发保存丢失更新」窗口。
        crate::settings_store::with_save_lock(|| match self {
            Self::Default => slterm_settings::restore_default_settings(),
            Self::Path(path) => slterm_settings::restore_default_settings_at(path),
        })
    }

    /// 观察当前设置文件字节快照(主题应用乐观并发基准)。
    fn revision(&self) -> io::Result<crate::settings_store::SettingsRevision> {
        match self {
            Self::Default => crate::settings_store::read_revision(),
            Self::Path(path) => crate::settings_store::read_revision_at(path),
        }
    }

    /// 用户 `keybind` 段读取(生产默认数据目录;显式路径测试隔离)。
    fn keybind_pairs(&self) -> Vec<(String, String)> {
        match self {
            Self::Default => keyboard_bindings::load_keybind_pairs(),
            Self::Path(path) => keyboard_bindings::load_keybind_pairs_at(path),
        }
    }

    /// 设置文件所在目录:导出等对话框的起始目录(测试目标 = 设置父目录)。
    fn settings_directory(&self) -> PathBuf {
        match self {
            Self::Default => slterm_settings::settings_dir(),
            Self::Path(path) => path
                .parent()
                .map_or_else(slterm_settings::settings_dir, std::path::Path::to_path_buf),
        }
    }

    /// 主题库根:显式路径时与设置文件同目录(`themes/`),生产走默认。
    fn theme_root(&self) -> Option<PathBuf> {
        match self {
            Self::Default => None,
            Self::Path(path) => path.parent().map(|parent| parent.join("themes")),
        }
    }

    /// 主题库实例(编辑/应用共用)。
    pub(super) fn theme_store(&self) -> crate::theme_library::ThemeLibraryStore {
        self.theme_root().map_or_else(
            crate::theme_library::ThemeLibraryStore::default,
            crate::theme_library::ThemeLibraryStore::new,
        )
    }
}

impl SettingsPane {
    /// 生产构造:读写默认数据目录下的 settings.json。
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_target(
            SettingsTarget::Default,
            agents::HooksTarget::Default,
            window,
            cx,
        )
    }

    /// 显式路径构造(测试 tempdir 隔离)。
    pub(crate) fn new_at(
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_target(
            SettingsTarget::Path(settings_path),
            agents::HooksTarget::Default,
            window,
            cx,
        )
    }

    /// 注入假 home 的构造(hooks 页联调用例:安装器写 tempdir,绝不碰
    /// 真实 `%USERPROFILE%`)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn new_at_with_claude_hooks(
        settings_path: PathBuf,
        claude_config_dir: PathBuf,
        helper: Option<String>,
        executable: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_target(
            SettingsTarget::Path(settings_path),
            agents::HooksTarget::Claude {
                config_dir: claude_config_dir,
                helper,
                executable,
            },
            window,
            cx,
        )
    }

    fn new_with_target(
        settings_target: SettingsTarget,
        hooks_target: agents::HooksTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 注册表幂等注册:页面路由/搜索别名的单源就位后才能取分区顺序。
        registry::register_builtin_pages();
        let using_default_target = settings_target.is_default();
        let (mut runtime, persisted, status) = match settings_target.load() {
            Ok(result) => {
                let runtime = RuntimeSettings::from_raw(&result.data);
                let status = result.corrupted.then_some(SettingsStatus::LoadedCorrupted);
                (runtime.clone(), runtime, status)
            }
            Err(error) => {
                log::warn!("设置页读取失败,使用默认值: {error}");
                let runtime = RuntimeSettings::from_raw(&slterm_settings::RawSettings::default());
                (
                    runtime.clone(),
                    runtime,
                    Some(SettingsStatus::Failed(error.to_string())),
                )
            }
        };
        // 开机启动显示以实际 .lnk 为准(09 篇 M10.6):安装器与设置页管理
        // 同一 Startup 快捷方式,设置键只承载最近一次用户意图。测试注入
        // 目标(tempdir)不读取真实 Startup 文件夹。
        if using_default_target {
            runtime.launch_at_login = crate::platform::startup::launch_at_login();
        }
        let language = resolve_language(runtime.language);
        let mut subscriptions = Vec::new();

        let search_input = cx.new(|cx| InputState::new(window, cx));
        subscriptions.push(cx.subscribe_in(
            &search_input,
            window,
            |this, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.update_settings_search(cx);
                }
            },
        ));

        let font_family_input = cx.new(|cx| InputState::new(window, cx));
        font_family_input.update(cx, |state, cx| {
            state.set_placeholder(
                localized_input_placeholder("font_family", language),
                window,
                cx,
            );
        });
        let font_size_input = cx.new(|cx| InputState::new(window, cx));
        font_size_input.update(cx, |state, cx| {
            state.set_placeholder(
                localized_input_placeholder("font_size", language),
                window,
                cx,
            );
        });
        let startup_directory_input = cx.new(|cx| InputState::new(window, cx));
        startup_directory_input.update(cx, |state, cx| {
            state.set_placeholder(
                localized_input_placeholder("startup_directory", language),
                window,
                cx,
            );
        });

        let scroll_speed_slider =
            Self::create_scroll_speed_slider(&persisted, window, cx, &mut subscriptions);
        let opacity_slider =
            Self::create_opacity_slider(&persisted, window, cx, &mut subscriptions);
        let wallpaper_fit_select =
            Self::create_wallpaper_fit_select(&persisted, language, window, cx, &mut subscriptions);
        let wallpaper_alignment_select = Self::create_wallpaper_alignment_select(
            &persisted,
            language,
            window,
            cx,
            &mut subscriptions,
        );
        let wallpaper_opacity_slider =
            Self::create_wallpaper_opacity_slider(&persisted, window, cx, &mut subscriptions);
        let keybind_pairs = settings_target.keybind_pairs();
        let mut pane = Self {
            focus_handle: cx.focus_handle(),
            settings_target,
            hooks_target,
            agents: agents::AgentSettingsState::new(window, cx),
            runtime,
            language,
            persisted,
            active_section: 0,
            search_origin: None,
            search_input,
            selects: Vec::new(),
            scroll_speed_slider,
            opacity_slider,
            font_family_input,
            font_size_input,
            startup_directory_input,
            status,
            reset_armed: false,
            persist_queue: std::collections::VecDeque::new(),
            persist_running: false,
            live_opacity: None,
            wallpaper_fit_select,
            wallpaper_alignment_select,
            wallpaper_select_language: language,
            wallpaper_opacity_slider,
            wallpaper_preview: None,
            wallpaper_preview_path: None,
            wallpaper_preview_generation: 0,
            wallpaper_preview_loading: false,
            wallpaper_error: None,
            theme_import_busy: false,
            theme_export_busy: false,
            theme_editor: None,
            keybind_focus: cx.focus_handle(),
            keybind_recording: None,
            keybind_error: None,
            keybind_pairs,
            _subscriptions: subscriptions,
        };
        pane.add_builtin_selects(language, window, cx);
        pane.sync_controls_from_runtime(window, cx);
        pane
    }

    fn create_scroll_speed_slider(
        runtime: &RuntimeSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Entity<SliderState> {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(slterm_settings::MIN_SCROLL_SPEED)
                .max(slterm_settings::MAX_SCROLL_SPEED)
                .step(slterm_settings::SCROLL_SPEED_STEP)
                .default_value(runtime.scroll_speed)
        });
        subscriptions.push(cx.subscribe_in(
            &slider,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(value) => this.preview_scroll_speed(value.start(), cx),
                SliderEvent::Release(value) => {
                    this.commit_scroll_speed(value.start(), window, cx);
                }
            },
        ));
        // 可访问性增减走 set_value,不发 Change/Release;值偏离已存值时补一次
        // 预览+提交(与鼠标路径同一归一化入口)。
        subscriptions.push(cx.observe_in(&slider, window, |this, slider, window, cx| {
            let value = slider.read(cx).value().start();
            if (value - this.runtime.scroll_speed).abs() > f32::EPSILON {
                this.preview_scroll_speed(value, cx);
                this.commit_scroll_speed(value, window, cx);
            }
        }));
        slider
    }

    fn create_opacity_slider(
        runtime: &RuntimeSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Entity<SliderState> {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.05)
                .default_value(runtime.opacity)
        });
        subscriptions.push(cx.subscribe_in(
            &slider,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(value) => this.preview_opacity(value.start(), cx),
                SliderEvent::Release(value) => this.commit_opacity(value.start(), window, cx),
            },
        ));
        subscriptions.push(cx.observe_in(&slider, window, |this, slider, window, cx| {
            let value = slider.read(cx).value().start();
            if (value - this.runtime.opacity).abs() > 1e-4 {
                this.preview_opacity(value, cx);
                this.commit_opacity(value, window, cx);
            }
        }));
        slider
    }

    /// 壁纸 fit 下拉:索引 ↔ `WALLPAPER_FIT_VALUES`;默认索引对齐
    /// `BackgroundImageFit::default`(uniform_to_fill)。
    fn create_wallpaper_fit_select(
        runtime: &RuntimeSettings,
        language: UiLanguage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> SharedSelect {
        let selected = runtime
            .background_image_fit
            .as_deref()
            .and_then(|value| {
                WALLPAPER_FIT_VALUES
                    .iter()
                    .position(|candidate| *candidate == value)
            })
            .unwrap_or(2);
        let select = cx.new(|cx| {
            SelectState::new(
                wallpaper_fit_labels(language),
                Some(gpui_component::IndexPath::default().row(selected)),
                window,
                cx,
            )
        });
        subscriptions.push(cx.subscribe_in(
            &select,
            window,
            |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if !matches!(event, SelectEvent::Confirm(Some(_))) {
                    return;
                }
                let Some(row) = entity.read(cx).selected_index(cx).map(|path| path.row) else {
                    return;
                };
                let Some(value) = WALLPAPER_FIT_VALUES.get(row).copied() else {
                    return;
                };
                this.commit_wallpaper_fit(value, window, cx);
            },
        ));
        select
    }

    /// 壁纸对齐下拉(九宫格);默认索引对齐 `BackgroundImageAlignment::default`。
    fn create_wallpaper_alignment_select(
        runtime: &RuntimeSettings,
        language: UiLanguage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> SharedSelect {
        let selected = runtime
            .background_image_alignment
            .as_deref()
            .and_then(|value| {
                WALLPAPER_ALIGNMENT_VALUES
                    .iter()
                    .position(|candidate| *candidate == value)
            })
            .unwrap_or(4);
        let select = cx.new(|cx| {
            SelectState::new(
                wallpaper_alignment_labels(language),
                Some(gpui_component::IndexPath::default().row(selected)),
                window,
                cx,
            )
        });
        subscriptions.push(cx.subscribe_in(
            &select,
            window,
            |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if !matches!(event, SelectEvent::Confirm(Some(_))) {
                    return;
                }
                let Some(row) = entity.read(cx).selected_index(cx).map(|path| path.row) else {
                    return;
                };
                let Some(value) = WALLPAPER_ALIGNMENT_VALUES.get(row).copied() else {
                    return;
                };
                this.commit_wallpaper_alignment(value, window, cx);
            },
        ));
        select
    }

    /// 壁纸不透明度滑杆:显示域 0–100,拖动只改内存/视效,松手落盘。
    fn create_wallpaper_opacity_slider(
        runtime: &RuntimeSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Entity<SliderState> {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(runtime.background_image_opacity.clamp(0.0, 1.0) * 100.0)
        });
        subscriptions.push(cx.subscribe_in(
            &slider,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(value) => this.preview_wallpaper_opacity(value.start(), cx),
                SliderEvent::Release(value) => {
                    this.commit_wallpaper_opacity(value.start(), window, cx);
                }
            },
        ));
        // 可访问性增减走 set_value,不发 Change/Release;值偏离运行时值
        // 时补一次预览+提交(与鼠标路径同一归一化入口)。
        subscriptions.push(cx.observe_in(&slider, window, |this, slider, window, cx| {
            let value = slider.read(cx).value().start();
            if (value - this.runtime.background_image_opacity * 100.0).abs() > 0.5 {
                this.preview_wallpaper_opacity(value, cx);
                this.commit_wallpaper_opacity(value, window, cx);
            }
        }));
        slider
    }

    /// 语言热切换:壁纸下拉不进入通用 `selects`,在页面渲染入口补齐
    /// 显示项刷新(实体与选中索引保留,不重建控件)。
    pub(super) fn sync_wallpaper_select_language(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.wallpaper_select_language == self.language {
            return;
        }
        self.wallpaper_select_language = self.language;
        let language = self.language;
        let fit_selected = self.wallpaper_fit_select.read(cx).selected_index(cx);
        self.wallpaper_fit_select.update(cx, |state, cx| {
            state.set_items(wallpaper_fit_labels(language), window, cx);
            state.set_selected_index(fit_selected, window, cx);
        });
        let alignment_selected = self.wallpaper_alignment_select.read(cx).selected_index(cx);
        self.wallpaper_alignment_select.update(cx, |state, cx| {
            state.set_items(wallpaper_alignment_labels(language), window, cx);
            state.set_selected_index(alignment_selected, window, cx);
        });
    }

    /// 选择壁纸:平台文件选择器;取消零写入,解码成功后才写设置键。
    pub(super) fn pick_wallpaper(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let language = self.language(cx);
        let prompt = language
            .text(Message::SettingsAppearanceWallpaperPickPrompt)
            .to_owned();
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |pane, cx| pane.begin_wallpaper_preview(path, cx));
        })
        .detach();
    }

    /// 后台解码选定路径;成功才展示缩略图并写入 `background_image`。
    fn begin_wallpaper_preview(&mut self, path: PathBuf, cx: &mut Context<'_, Self>) {
        self.wallpaper_preview_generation = self.wallpaper_preview_generation.wrapping_add(1);
        let generation = self.wallpaper_preview_generation;
        self.wallpaper_preview_loading = true;
        self.wallpaper_error = None;
        let persist_path = path.to_string_lossy().into_owned();
        let task = crate::app_shell::wallpaper::load_preview_async(path.clone(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |pane, cx| {
                if pane.wallpaper_preview_generation != generation {
                    return;
                }
                pane.wallpaper_preview_loading = false;
                match result {
                    Ok(preview) => {
                        pane.replace_wallpaper_preview(Some(preview), cx);
                        pane.wallpaper_preview_path = Some(path);
                        pane.wallpaper_error = None;
                        pane.runtime.background_image = Some(persist_path.clone());
                        pane.try_persist(&[("background_image", Value::String(persist_path))], cx);
                        pane.status = None;
                    }
                    Err(error) => {
                        let message = pane.wallpaper_preview_error_text(error);
                        pane.wallpaper_error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 运行时路径与已确认预览路径不一致时重载预览(启动/回读/恢复路径)。
    fn sync_wallpaper_preview(&mut self, cx: &mut Context<'_, Self>) {
        let desired = self.runtime.background_image.clone().map(PathBuf::from);
        if desired == self.wallpaper_preview_path {
            return;
        }
        self.wallpaper_preview_path = desired.clone();
        self.wallpaper_preview_generation = self.wallpaper_preview_generation.wrapping_add(1);
        let generation = self.wallpaper_preview_generation;
        let Some(path) = desired else {
            self.wallpaper_preview_loading = false;
            self.replace_wallpaper_preview(None, cx);
            self.wallpaper_error = None;
            return;
        };
        self.wallpaper_preview_loading = true;
        let task = crate::app_shell::wallpaper::load_preview_async(path, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |pane, cx| {
                if pane.wallpaper_preview_generation != generation {
                    return;
                }
                pane.wallpaper_preview_loading = false;
                match result {
                    Ok(preview) => {
                        pane.replace_wallpaper_preview(Some(preview), cx);
                        pane.wallpaper_error = None;
                    }
                    Err(error) => {
                        let message = pane.wallpaper_preview_error_text(error);
                        pane.wallpaper_error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 替换预览纹理;旧纹理延后释放,避免在窗口 update 期间动渲染资源。
    fn replace_wallpaper_preview(
        &mut self,
        preview: Option<crate::app_shell::wallpaper::WallpaperPreview>,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(previous) = self.wallpaper_preview.take() {
            cx.defer(move |cx| cx.drop_image(previous.image, None));
        }
        self.wallpaper_preview = preview;
    }

    /// 预览失败原因 → 可读文案(不裸出内部错误枚举)。
    fn wallpaper_preview_error_text(
        &self,
        error: crate::app_shell::wallpaper::WallpaperPreviewError,
    ) -> String {
        use crate::app_shell::wallpaper::WallpaperPreviewError;
        let message = match error {
            WallpaperPreviewError::Read => Message::SettingsAppearanceWallpaperErrorRead,
            WallpaperPreviewError::Decode => Message::SettingsAppearanceWallpaperErrorDecode,
            WallpaperPreviewError::TooLarge => Message::SettingsAppearanceWallpaperErrorTooLarge,
        };
        self.language.text(message).to_owned()
    }

    /// 清除壁纸:预览与设置键同帧置空,写盘走统一通道。
    pub(super) fn clear_wallpaper(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        self.wallpaper_preview_generation = self.wallpaper_preview_generation.wrapping_add(1);
        self.wallpaper_preview_path = None;
        self.wallpaper_preview_loading = false;
        self.replace_wallpaper_preview(None, cx);
        self.wallpaper_error = None;
        self.runtime.background_image = None;
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().background_image = None;
        }
        crate::app_shell::wallpaper::refresh(cx);
        self.try_persist(&[("background_image", Value::Null)], cx);
        self.status = None;
        cx.notify();
    }

    fn commit_wallpaper_fit(
        &mut self,
        value: &'static str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.runtime.background_image_fit = Some(value.to_owned());
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().background_image_fit = Some(value.to_owned());
        }
        crate::app_shell::wallpaper::refresh(cx);
        self.try_persist(
            &[("background_image_fit", Value::String(value.to_owned()))],
            cx,
        );
        self.status = None;
        cx.notify();
    }

    fn commit_wallpaper_alignment(
        &mut self,
        value: &'static str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.runtime.background_image_alignment = Some(value.to_owned());
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().background_image_alignment = Some(value.to_owned());
        }
        crate::app_shell::wallpaper::refresh(cx);
        self.try_persist(
            &[(
                "background_image_alignment",
                Value::String(value.to_owned()),
            )],
            cx,
        );
        self.status = None;
        cx.notify();
    }

    /// 壁纸不透明度拖动快路径:只改全局事实源与视效 alpha,不触盘。
    fn preview_wallpaper_opacity(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        let opacity = (value / 100.0).clamp(0.0, 1.0);
        self.runtime.background_image_opacity = opacity;
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().background_image_opacity = opacity;
        }
        crate::app_shell::wallpaper::set_wallpaper_opacity_live(opacity, cx);
        cx.refresh_windows();
        cx.notify();
    }

    fn commit_wallpaper_opacity(
        &mut self,
        value: f32,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let opacity = (value / 100.0).clamp(0.0, 1.0);
        self.runtime.background_image_opacity = opacity;
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().background_image_opacity = opacity;
        }
        self.try_persist(
            &[(
                "background_image_opacity",
                Value::Number(f64::from(opacity)),
            )],
            cx,
        );
        self.status = None;
        cx.notify();
    }

    /// 主题包导入:选择器 → 后台 `CheckedPackage::open + install` → 前台提示。
    pub(super) fn import_theme_package(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.theme_import_busy {
            return;
        }
        let language = self.language(cx);
        let prompt = language
            .text(Message::SettingsAppearanceThemeImportPrompt)
            .to_owned();
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        self.theme_import_busy = true;
        self.status = None;
        cx.notify();
        let store = self.settings_target.theme_store();
        cx.spawn(async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(mut paths))) => paths.pop(),
                _ => None,
            };
            let Some(path) = path else {
                let _ = this.update(cx, |pane, cx| {
                    pane.theme_import_busy = false;
                    cx.notify();
                });
                return;
            };
            if !has_zip_extension(&path) {
                let message = language
                    .text(Message::SettingsAppearanceThemeImportZipRequired)
                    .to_owned();
                let _ = this.update(cx, |pane, cx| {
                    pane.theme_import_busy = false;
                    pane.status = Some(SettingsStatus::ThemeImportFailed(message));
                    cx.notify();
                });
                return;
            }
            let task = cx.background_executor().spawn(async move {
                CheckedPackage::open(&path)
                    .and_then(|package| package.install(&store))
                    .map(|document| document.name().to_owned())
                    .map_err(|error| error.to_string())
            });
            let result = task.await;
            let _ = this.update(cx, |pane, cx| pane.finish_theme_import(result, cx));
        })
        .detach();
    }

    fn finish_theme_import(&mut self, result: Result<String, String>, cx: &mut Context<'_, Self>) {
        self.theme_import_busy = false;
        match result {
            Ok(name) => {
                self.status = Some(SettingsStatus::ThemeImported { name });
                // 主题库已变更:走既有 Changed 路径刷新消费方(编辑器模板
                // 在下次打开时从 store 重新派生)。
                cx.emit(SettingsPaneEvent::Changed);
            }
            Err(error) => self.status = Some(SettingsStatus::ThemeImportFailed(error)),
        }
        cx.notify();
    }

    /// 主题包导出:选目标 → 后台读取当前自定义主题并 `export_package`。
    pub(super) fn export_theme_package(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.theme_export_busy {
            return;
        }
        let Some(theme_id) = self.runtime.custom_theme.clone() else {
            return;
        };
        let language = self.language(cx);
        let directory = self.theme_package_directory();
        let suggested = format!("{theme_id}.slterm-theme.zip");
        let picked = cx.prompt_for_new_path(&directory, Some(&suggested));
        self.theme_export_busy = true;
        self.status = None;
        cx.notify();
        let store = self.settings_target.theme_store();
        cx.spawn(async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(path))) => Some(path),
                _ => None,
            };
            let Some(output) = path else {
                let _ = this.update(cx, |pane, cx| {
                    pane.theme_export_busy = false;
                    cx.notify();
                });
                return;
            };
            if !has_zip_extension(&output) {
                let message = language
                    .text(Message::SettingsAppearanceThemeExportZipRequired)
                    .to_owned();
                let _ = this.update(cx, |pane, cx| {
                    pane.theme_export_busy = false;
                    pane.status = Some(SettingsStatus::ThemeExportFailed(message));
                    cx.notify();
                });
                return;
            }
            let task = cx.background_executor().spawn(async move {
                let document = store.load(&theme_id).map_err(|error| error.to_string())?;
                export_package(
                    &document,
                    Author {
                        name: THEME_PACKAGE_AUTHOR.to_owned(),
                        github: None,
                    },
                    THEME_PACKAGE_VERSION.to_owned(),
                    THEME_PACKAGE_LICENSE.to_owned(),
                    &output,
                    None,
                )
                .map_err(|error| error.to_string())?;
                Ok::<PathBuf, String>(output)
            });
            let result = task.await;
            let _ = this.update(cx, |pane, cx| pane.finish_theme_export(result, cx));
        })
        .detach();
    }

    fn finish_theme_export(&mut self, result: Result<PathBuf, String>, cx: &mut Context<'_, Self>) {
        self.theme_export_busy = false;
        self.status = Some(match result {
            Ok(path) => SettingsStatus::ThemeExported { path },
            Err(error) => SettingsStatus::ThemeExportFailed(error),
        });
        cx.notify();
    }

    /// 导出对话框的起始目录:显式测试目标 = 设置文件父目录;生产 =
    /// 默认数据目录(与主题库同一根)。
    fn theme_package_directory(&self) -> PathBuf {
        self.settings_target
            .theme_root()
            .and_then(|root| root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(slterm_settings::settings_dir)
    }

    /// 进入键位录制:录制焦点接管下一次 KeyDown;Esc 由录制处理函数取消。
    pub(super) fn begin_keybind_recording(
        &mut self,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.keybind_recording = Some(action.to_owned());
        self.keybind_error = None;
        window.focus(&self.keybind_focus, cx);
        cx.notify();
    }

    /// 取消录制(零写入)。
    pub(super) fn cancel_keybind_recording(&mut self, cx: &mut Context<'_, Self>) {
        if self.keybind_recording.take().is_some() {
            self.keybind_error = None;
            cx.notify();
        }
    }

    /// 录制按键:先准入;保留键/非法组合给可读错误且不写盘,成功则整表写回。
    pub(super) fn handle_keybind_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(action) = self.keybind_recording.clone() else {
            return;
        };
        // 长按重复事件不重复改绑;事件仍由录制态吞掉。
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        if event.keystroke.key.eq_ignore_ascii_case("escape") {
            self.cancel_keybind_recording(cx);
            return;
        }
        let combo = event.keystroke.unparse();
        let admission =
            keyboard_bindings::admit_keybind_pairs(vec![(combo.clone(), action.clone())]);
        if !admission.rejected.is_empty() {
            let message = if keyboard_bindings::is_reserved_combo(&combo) {
                Message::SettingsInteractionKeybindReservedError
            } else {
                Message::SettingsInteractionKeybindInvalidError
            };
            self.keybind_error = Some(self.language(cx).format(message, &[("combo", &combo)]));
            cx.notify();
            return;
        }
        let mut pairs = self.keybind_pairs.clone();
        keyboard_bindings::rebind_action(&mut pairs, &action, &combo);
        self.keybind_recording = None;
        self.keybind_error = None;
        self.write_keybind_pairs(pairs, cx);
    }

    /// 清除动作绑定:撤自定义行并把默认键精确收回(`none` 标记)。
    pub(super) fn clear_keybind_action(&mut self, action: &str, cx: &mut Context<'_, Self>) {
        let mut pairs = self.keybind_pairs.clone();
        keyboard_bindings::clear_action(&mut pairs, action);
        self.keybind_recording = None;
        self.keybind_error = None;
        self.write_keybind_pairs(pairs, cx);
    }

    /// 恢复动作默认绑定:撤自定义行与默认键的 `none` 标记。
    pub(super) fn reset_keybind_action(&mut self, action: &str, cx: &mut Context<'_, Self>) {
        let mut pairs = self.keybind_pairs.clone();
        keyboard_bindings::reset_action(&mut pairs, action);
        self.keybind_recording = None;
        self.keybind_error = None;
        self.write_keybind_pairs(pairs, cx);
    }

    /// 键位整表写回:经统一设置写通道(外部段整表替换、行序即优先级)。
    fn write_keybind_pairs(&mut self, pairs: Vec<(String, String)>, cx: &mut Context<'_, Self>) {
        let value = slterm_settings::keys::keybind_value(&pairs);
        self.keybind_pairs = pairs;
        self.status = None;
        self.try_persist(&[(KEYBIND_SEGMENT, value)], cx);
        cx.notify();
    }

    /// 导出工作区:选目标路径后发事件,窗口态采集与写盘归宿主。
    pub(super) fn prompt_export_workspace(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let directory = self.settings_target.settings_directory();
        let picked = cx.prompt_for_new_path(&directory, Some("slterm-workspace.json"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let _ = this.update(cx, |_pane, cx| {
                cx.emit(SettingsPaneEvent::ExportWorkspace { path });
            });
        })
        .detach();
    }

    fn add_builtin_selects(
        &mut self,
        language: UiLanguage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = self.runtime.clone();
        let entries: Vec<(&'static str, &'static [&'static str], String)> = vec![
            (
                "language",
                slterm_settings::LanguagePref::VALUES,
                runtime.language.settings_value().to_owned(),
            ),
            (
                "theme",
                &ThemeName::BUILTIN_NAMES,
                runtime.theme.prompt_name().to_owned(),
            ),
            (
                "ligatures",
                &Ligatures::VALUES,
                runtime.ligatures.settings_value().to_owned(),
            ),
            (
                "density",
                &slterm_settings::DensityName::VALUES,
                runtime.density.settings_value().to_owned(),
            ),
            (
                "tab_reveal",
                &TabRevealName::VALUES,
                runtime.tab_reveal.settings_value().to_owned(),
            ),
            (
                "cell_width_mode",
                &CellWidthModeName::VALUES,
                runtime.cell_width_mode.settings_value().to_owned(),
            ),
            (
                "completion_style",
                &CompletionStyleName::VALUES,
                runtime.completion_style.settings_value().to_owned(),
            ),
            (
                "accept",
                &slterm_settings::AcceptKeyName::VALUES,
                runtime.accept.settings_value().to_owned(),
            ),
            (
                "bell",
                &BellModeName::VALUES,
                runtime.bell.settings_value().to_owned(),
            ),
            (
                "notification_duration",
                &NotificationDuration::VALUES,
                runtime.notification_duration.settings_value().to_owned(),
            ),
            (
                "cursor_shape",
                &slterm_settings::CursorShapeName::VALUES,
                runtime
                    .cursor_shape
                    .map(|shape| shape.settings_value())
                    .unwrap_or("beam")
                    .to_owned(),
            ),
            (
                "cursor_motion",
                &slterm_settings::CursorMotion::VALUES,
                runtime.cursor_motion.settings_value().to_owned(),
            ),
            (
                "new_tab_position",
                &slterm_settings::NewTabPositionName::VALUES,
                runtime.new_tab_position.settings_value().to_owned(),
            ),
            (
                "vcs_display",
                &VcsDisplayName::VALUES,
                runtime.vcs_display.settings_value().to_owned(),
            ),
            (
                "blur",
                &BlurMode::VALUES,
                runtime.blur.settings_value().to_owned(),
            ),
            (
                "scrollback_lines",
                &SCROLLBACK_LABELS,
                runtime.scrollback_lines.to_string(),
            ),
        ];
        for (key, values, current) in entries {
            self.add_select(key, values, &current, language, window, cx);
        }
    }

    fn add_select(
        &mut self,
        key: &'static str,
        values: &'static [&'static str],
        current: &str,
        language: UiLanguage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let selected = values
            .iter()
            .position(|value| *value == current)
            .unwrap_or(0);
        let select = cx.new(|cx| {
            SelectState::new(
                localized_select_labels(key, values, language),
                Some(gpui_component::IndexPath::default().row(selected)),
                window,
                cx,
            )
        });
        self._subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if !matches!(event, SelectEvent::Confirm(Some(_))) {
                    return;
                }
                let row = entity.read(cx).selected_index(cx).map(|path| path.row);
                let Some(value) = row.and_then(|row| values.get(row)).copied() else {
                    return;
                };
                this.commit_select(key, value, window, cx);
            },
        ));
        self.selects.push(SelectEntry {
            key,
            select,
            values,
        });
    }

    fn render_nav(&mut self, window: &Window, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let language = self.language(cx);
        let active_bg = cx.theme().sidebar_accent;
        let active_fg = cx.theme().sidebar_accent_foreground;
        let muted = cx.theme().muted_foreground;
        let foreground = cx.theme().foreground;
        let hover_bg = cx.theme().list_hover;
        let hairline = cx.theme().border;
        let mut nav = v_flex()
            .id("settings-nav")
            .debug_selector(|| "settings-nav".to_owned())
            .w(px(SETTINGS_NAV_WIDTH))
            .h_full()
            .flex_shrink_0()
            .px_2()
            .pt(px(12.0))
            .pb(px(8.0))
            .gap(px(2.0))
            .text_sm()
            .line_height(px(20.0))
            .border_r_1()
            .border_color(hairline)
            .child(
                div()
                    .id("settings-back")
                    .debug_selector(|| "settings-back".to_owned())
                    .mx_1()
                    .mb(px(10.0))
                    .h(px(SETTINGS_NAV_ROW_HEIGHT))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(muted)
                    .hover(move |item| item.bg(hover_bg).text_color(foreground))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsPaneEvent::Close)))
                    .child(Icon::new(IconName::ArrowLeft).size(px(SETTINGS_NAV_ICON_SIZE)))
                    .child(language.text(Message::SettingsNavBack)),
            )
            .child(
                div()
                    .debug_selector(|| "settings-search".to_owned())
                    .mx_1()
                    .mb(px(12.0))
                    .border_b_1()
                    .border_color(hairline)
                    .child(
                        Input::new(&self.search_input)
                            .appearance(false)
                            .focus_bordered(false)
                            .cleanable(true)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .size(px(SETTINGS_NAV_ICON_SIZE))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .aria_label(language.text(Message::SettingsNavSearch)),
                    ),
            );
        let searching = !self.search_input.read(cx).value().trim().is_empty();
        let matches = matching_sections(&self.search_input.read(cx).value(), language);
        let visible: Vec<usize> = visible_nav_sections()
            .filter(|index| !searching || matches.contains(index))
            .collect();
        let mut last_group = None;
        for index in visible {
            let group = registry::get_all().get(index).map(|def| def.group);
            if group != last_group {
                if group == Some(registry::SettingsPageGroup::Agent) {
                    nav = nav.child(
                        div()
                            .debug_selector(|| "settings-nav-group-agents".to_owned())
                            .mt(px(12.0))
                            .mb(px(4.0))
                            .px(px(10.0))
                            .text_size(px(11.0))
                            .text_color(muted)
                            .child(language.text(Message::SettingsSidebarGroupAgents)),
                    );
                }
                last_group = group;
            }
            let active = index == self.active_section;
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("settings-nav-item-{index}")))
                    .debug_selector({
                        let id = section_ids()
                            .get(index)
                            .copied()
                            .unwrap_or("unknown")
                            .to_owned();
                        move || format!("settings-nav-{id}")
                    })
                    .px(px(10.0))
                    .ml_1()
                    .mr_1()
                    .h(px(SETTINGS_NAV_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded_md()
                    .cursor_pointer()
                    .font_weight(if active {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .when(active, |item| item.bg(active_bg).text_color(active_fg))
                    .when(!active, |item| {
                        item.text_color(muted).hover(move |s| s.bg(hover_bg))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activate_section(index, cx);
                    }))
                    .child(
                        Icon::new(section_icon(index))
                            .size(px(SETTINGS_NAV_ICON_SIZE))
                            .flex_shrink_0()
                            .text_color(if active { active_fg } else { muted }),
                    )
                    .child(section_label(index, language)),
            );
        }
        // 搜索期间被过滤的页不出行;其余空间交给底部留白。
        let _ = window;
        nav.child(div().flex_1().min_h(px(24.0))).into_any_element()
    }

    fn render_status(
        &self,
        cx: &Context<'_, Self>,
        language: UiLanguage,
    ) -> Option<gpui::AnyElement> {
        let status = self.status.as_ref()?;
        let color = if status.is_error() {
            cx.theme().danger
        } else {
            cx.theme().success
        };
        Some(
            div()
                .id("settings-status")
                .debug_selector(|| "settings-status".to_owned())
                .w_full()
                .mb(px(12.0))
                .px(px(12.0))
                .py(px(8.0))
                .rounded_md()
                .border_1()
                .border_color(color.opacity(0.45))
                .bg(color.opacity(0.08))
                .text_sm()
                .text_color(color)
                .child(status.text(language))
                .into_any_element(),
        )
    }

    fn current_page_id(&self) -> &'static str {
        section_ids()
            .get(self.active_section)
            .copied()
            .unwrap_or("application")
    }

    fn render_page(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let page_id = self.current_page_id();
        let content = match page_id {
            // 主题编辑是外观页的子视图:打开时替换整页,返回恢复。
            "appearance" if self.theme_editor.is_some() => {
                self.page_theme_editor(window, cx).into_any_element()
            }
            "appearance" => self.page_appearance(window, cx).into_any_element(),
            "profiles" => self.page_profiles(window, cx).into_any_element(),
            "agents" => self.page_agents(window, cx).into_any_element(),
            "interaction" => self.page_interaction(window, cx).into_any_element(),
            "advanced" => self.page_advanced(window, cx).into_any_element(),
            "localization" => self.page_localization(window, cx).into_any_element(),
            "reset" => self.page_reset(window, cx).into_any_element(),
            // application:品牌位;关于/赞助入口归 12 篇。
            _ => self.page_application(window, cx).into_any_element(),
        };
        div()
            .id(SharedString::from(format!("settings-page-{page_id}")))
            .debug_selector(move || format!("settings-page-{page_id}"))
            .w_full()
            .child(content)
            .into_any_element()
    }

    fn page_application(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        v_flex().w_full().gap(px(SETTINGS_GROUP_GAP)).child(
            self.group(language.text(Message::SettingsApplicationGroup), cx)
                .child(
                    self.row(
                        language.text(Message::SettingsApplicationName),
                        "",
                        div()
                            .text_color(cx.theme().foreground)
                            .child(crate::brand::NAME),
                        cx,
                    ),
                )
                .child(
                    self.row(
                        language.text(Message::SettingsApplicationVersion),
                        "",
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(env!("CARGO_PKG_VERSION")),
                        cx,
                    ),
                ),
        )
    }

    fn page_localization(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        v_flex().w_full().gap(px(SETTINGS_GROUP_GAP)).child(
            self.group(language.text(Message::SettingsLocalizationGroup), cx)
                .child(self.select_row(
                    "language",
                    language.text(Message::SettingsLocalizationLanguage),
                    language.text(Message::SettingsLocalizationDescription),
                    cx,
                )),
        )
    }

    fn page_reset(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        let label = if self.reset_armed {
            language.text(Message::SettingsResetAgain)
        } else {
            language.text(Message::SettingsResetAction)
        };
        let button = Button::new("settings-reset-defaults")
            .label(label)
            .on_click(cx.listener(|this, _, window, cx| this.confirm_reset(window, cx)));
        v_flex().w_full().gap(px(SETTINGS_GROUP_GAP)).child(
            self.group(language.text(Message::SettingsResetGroup), cx)
                .child(self.row(
                    language.text(Message::SettingsResetAction),
                    language.text(Message::SettingsResetDescription),
                    button,
                    cx,
                )),
        )
    }
}

/// 回滚档位持久化值表(显示侧千分位由 localization 派生)。
const SCROLLBACK_LABELS: [&str; 7] = ["1000", "2000", "5000", "10000", "20000", "50000", "100000"];

/// 壁纸适应模式显示文案;与 [`WALLPAPER_FIT_VALUES`] 严格同序同数。
fn wallpaper_fit_labels(language: UiLanguage) -> Vec<SharedString> {
    [
        Message::SettingsAppearanceWallpaperFitFill,
        Message::SettingsAppearanceWallpaperFitUniform,
        Message::SettingsAppearanceWallpaperFitUniformToFill,
        Message::SettingsAppearanceWallpaperFitNone,
    ]
    .into_iter()
    .map(|message| SharedString::from(language.text(message)))
    .collect()
}

/// 壁纸九宫格对齐显示文案;与 [`WALLPAPER_ALIGNMENT_VALUES`] 严格同序同数。
fn wallpaper_alignment_labels(language: UiLanguage) -> Vec<SharedString> {
    [
        Message::SettingsAppearanceWallpaperAlignTopLeft,
        Message::SettingsAppearanceWallpaperAlignTop,
        Message::SettingsAppearanceWallpaperAlignTopRight,
        Message::SettingsAppearanceWallpaperAlignLeft,
        Message::SettingsAppearanceWallpaperAlignCenter,
        Message::SettingsAppearanceWallpaperAlignRight,
        Message::SettingsAppearanceWallpaperAlignBottomLeft,
        Message::SettingsAppearanceWallpaperAlignBottom,
        Message::SettingsAppearanceWallpaperAlignBottomRight,
    ]
    .into_iter()
    .map(|message| SharedString::from(language.text(message)))
    .collect()
}

/// 主题包对话框的目标/来源扩展名检查(不做前台文件 IO)。
fn has_zip_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

impl Focusable for SettingsPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = self.language(cx);
        let nav = self.render_nav(window, cx);
        let status = self.render_status(cx, language);
        let page = self.render_page(window, cx);
        let title = section_label(self.active_section, language);
        let background = cx.theme().background;
        let foreground = cx.theme().foreground;
        div()
            .size_full()
            .flex()
            .flex_row()
            .track_focus(&self.focus_handle)
            .bg(background)
            .text_color(foreground)
            .text_sm()
            .child(nav)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .px_5()
                    .pt(px(20.0))
                    .pb(px(22.0))
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .children(status)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .overflow_y_scrollbar()
                            // 滚动容器会清空内容层的 display;补一层纵向 flex
                            // 让分组沿交叉轴 stretch 取宽,控件列才对得齐。
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .justify_center()
                                    .child(v_flex().w_full().max_w(px(960.0)).child(page)),
                            ),
                    ),
            )
    }
}

/// B55/B19 虚拟窗口用例的驱动端(生产不编译)。
#[cfg(all(test, feature = "gpui-test-support"))]
impl SettingsPane {
    pub(crate) fn pick_wallpaper_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.pick_wallpaper(window, cx);
    }

    pub(crate) fn clear_wallpaper_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.clear_wallpaper(window, cx);
    }

    pub(crate) fn commit_wallpaper_fit_for_test(
        &mut self,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_wallpaper_fit(value, window, cx);
    }

    pub(crate) fn commit_wallpaper_alignment_for_test(
        &mut self,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_wallpaper_alignment(value, window, cx);
    }

    pub(crate) fn preview_wallpaper_opacity_for_test(
        &mut self,
        value: f32,
        cx: &mut Context<'_, Self>,
    ) {
        self.preview_wallpaper_opacity(value, cx);
    }

    pub(crate) fn commit_wallpaper_opacity_for_test(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_wallpaper_opacity(value, window, cx);
    }

    pub(crate) fn wallpaper_preview_ready_for_test(&self) -> bool {
        self.wallpaper_preview.is_some()
    }

    pub(crate) fn wallpaper_error_for_test(&self) -> Option<&str> {
        self.wallpaper_error.as_deref()
    }

    pub(crate) fn import_theme_package_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.import_theme_package(window, cx);
    }

    pub(crate) fn export_theme_package_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.export_theme_package(window, cx);
    }

    pub(crate) fn theme_import_busy_for_test(&self) -> bool {
        self.theme_import_busy
    }

    pub(crate) fn theme_export_busy_for_test(&self) -> bool {
        self.theme_export_busy
    }

    /// B7:当前用户在 `keybind` 段的内存表(写回后与磁盘同形)。
    pub(crate) fn keybind_pairs_for_test(&self) -> Vec<(String, String)> {
        self.keybind_pairs.clone()
    }

    pub(crate) fn keybind_recording_for_test(&self) -> Option<&str> {
        self.keybind_recording.as_deref()
    }

    pub(crate) fn keybind_error_for_test(&self) -> Option<&str> {
        self.keybind_error.as_deref()
    }
}

#[cfg(test)]
mod settings_pane_tests {
    use super::persistence::settings_value_of;
    use super::*;

    #[test]
    fn runtime_value_mapping_covers_every_select_bound_key() {
        let runtime = RuntimeSettings::from_raw(&slterm_settings::RawSettings::default());
        for key in [
            "language",
            "theme",
            "ligatures",
            "density",
            "tab_reveal",
            "cell_width_mode",
            "completion_style",
            "accept",
            "bell",
            "notification_duration",
            "cursor_shape",
            "cursor_motion",
            "new_tab_position",
            "vcs_display",
            "blur",
            "scrollback_lines",
        ] {
            assert!(
                !settings_value_of(&runtime, key).is_empty(),
                "{key} 缺运行时值映射,代码侧同步会静默落回第 0 项"
            );
        }
    }

    #[test]
    fn status_texts_are_semantic_and_localized() {
        let failed = SettingsStatus::Failed("disk full".to_owned());
        assert!(failed.is_error());
        let zh = failed.text(UiLanguage::ZhCn);
        assert!(zh.contains("设置操作失败"), "{zh}");
        assert!(zh.contains("disk full"), "错误原文必须可见: {zh}");
        let reset = SettingsStatus::Reset { backup: None };
        assert!(!reset.is_error());
        assert_eq!(reset.text(UiLanguage::EnUs), "Defaults restored");
    }
}
