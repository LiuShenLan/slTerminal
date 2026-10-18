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
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::select::{SelectEvent, SelectState};
use gpui_component::slider::{SliderEvent, SliderState};
use gpui_component::{ActiveTheme as _, Icon, IconName, v_flex};
use slterm_settings::json::Value;
use slterm_settings::{
    BellModeName, BlurMode, CellWidthModeName, CompletionStyleName, Ligatures,
    NotificationDuration, RuntimeSettings, TabRevealName, ThemeName, VcsDisplayName,
};

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

/// 一个固定下拉:键、实体、稳定值表(选中索引 → 持久化值)。
struct SelectEntry {
    key: &'static str,
    select: SharedSelect,
    values: &'static [&'static str],
}

/// 宿主订阅事件:设置已写盘(热应用)/ 请求关闭设置 tab。
pub(crate) enum SettingsPaneEvent {
    Changed,
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
}

impl SettingsStatus {
    pub(crate) fn is_error(&self) -> bool {
        matches!(self, Self::LoadedCorrupted | Self::Failed(_))
    }

    pub(crate) fn text(&self, language: UiLanguage) -> String {
        match self {
            Self::LoadedCorrupted => language
                .pick(
                    "配置文件损坏，已回退到备份或默认值；损坏原件已保留。",
                    "The settings file was corrupted; backup or defaults were loaded and the original kept.",
                )
                .to_owned(),
            Self::Reset { backup: Some(path) } => format!(
                "{}: {}",
                language.pick("已恢复默认设置，原设置已备份", "Defaults restored; previous settings backed up"),
                path.display()
            ),
            Self::Reset { backup: None } => {
                language.pick("已恢复默认设置", "Defaults restored").to_owned()
            }
            Self::Failed(error) => format!(
                "{}: {error}",
                language.pick("设置操作失败", "Settings operation failed")
            ),
        }
    }
}

/// 设置页壳。
pub(crate) struct SettingsPane {
    pub(crate) focus_handle: FocusHandle,
    /// 本页读写目标(生产 = 默认数据目录;测试注入 tempdir 路径)。
    settings_target: SettingsTarget,
    /// 渲染与预览的单一事实源;每次成功写盘后整体重载。
    runtime: RuntimeSettings,
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
    /// 主题编辑会话(外观页子视图;关闭即丢弃草稿)。
    theme_editor: Option<theme_editor::ThemeEditor>,
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
        match self {
            Self::Default => slterm_settings::restore_default_settings(),
            Self::Path(path) => slterm_settings::restore_default_settings_at(path),
        }
    }

    /// 观察当前设置文件字节快照(主题应用乐观并发基准)。
    fn revision(&self) -> io::Result<crate::settings_store::SettingsRevision> {
        match self {
            Self::Default => crate::settings_store::read_revision(),
            Self::Path(path) => crate::settings_store::read_revision_at(path),
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
        Self::new_with_target(SettingsTarget::Default, window, cx)
    }

    /// 显式路径构造(测试 tempdir 隔离)。
    pub(crate) fn new_at(
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_target(SettingsTarget::Path(settings_path), window, cx)
    }

    fn new_with_target(
        settings_target: SettingsTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 注册表幂等注册:页面路由/搜索别名的单源就位后才能取分区顺序。
        registry::register_builtin_pages();
        let (runtime, persisted, status) = match settings_target.load() {
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
        let mut pane = Self {
            focus_handle: cx.focus_handle(),
            settings_target,
            runtime,
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
            theme_editor: None,
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
                    .child(language.pick("返回工作区", "Back to workspace")),
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
                            .aria_label(language.pick("在全部设置中搜索", "Search all settings")),
                    ),
            );
        let searching = !self.search_input.read(cx).value().trim().is_empty();
        let matches = matching_sections(&self.search_input.read(cx).value(), language);
        for index in visible_nav_sections() {
            let active = index == self.active_section;
            let matched = matches.contains(&index);
            if searching && !matched {
                continue;
            }
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
            self.group(language.pick("应用", "Application"), cx)
                .child(
                    self.row(
                        language.pick("名称", "Name"),
                        "",
                        div()
                            .text_color(cx.theme().foreground)
                            .child(crate::brand::NAME),
                        cx,
                    ),
                )
                .child(
                    self.row(
                        language.pick("版本", "Version"),
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
            self.group(language.pick("界面语言", "Interface language"), cx)
                .child(self.select_row(
                    "language",
                    language.pick("语言", "Language"),
                    language.pick(
                        "System 跟随系统区域设置；切换后立即刷新界面文案。",
                        "System follows the OS locale; the interface refreshes immediately.",
                    ),
                    cx,
                )),
        )
    }

    fn page_reset(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        let label = if self.reset_armed {
            language.pick("再次点击确认重置", "Click again to confirm")
        } else {
            language.pick("恢复默认设置", "Restore defaults")
        };
        let button = Button::new("settings-reset-defaults")
            .label(label)
            .on_click(cx.listener(|this, _, window, cx| this.confirm_reset(window, cx)));
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.pick("重置", "Reset"), cx)
                    .child(self.row(
                        language.pick("恢复默认设置", "Restore defaults"),
                        language.pick(
                            "删除已登记设置键并回到出厂值；未知键与 Agent 接入授权保留，原设置先备份。",
                            "Removes registered settings and restores factory values; unknown keys and agent authorizations are kept, and the previous settings are backed up first.",
                        ),
                        button,
                        cx,
                    )),
            )
    }
}

/// 回滚档位持久化值表(显示侧千分位由 localization 派生)。
const SCROLLBACK_LABELS: [&str; 7] = ["1000", "2000", "5000", "10000", "20000", "50000", "100000"];

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
