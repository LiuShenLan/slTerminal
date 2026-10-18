//! 设置页写通道与控件同步:统一持久化入口、失败回滚、失败可感知。
//!
//! 三纪律落点:预览方法只改内存;提交方法经 `settings_store` 写盘并在
//! 失败时读回 `persisted` 快照复位控件;错误记录 stderr + 状态条 + 窗口
//! 通知。页面渲染只调这些入口,不自行写盘。

use std::io;

use gpui::{App, Context, PathPromptOptions, SharedString, Window};
use gpui_component::WindowExt as _;
use slterm_settings::json::Value;
use slterm_settings::{RuntimeSettings, normalize_scroll_speed};

use crate::app_shell::config::Settings;
use crate::i18n::Message;

use super::localization::{
    UiLanguage, localized_input_placeholder, localized_select_labels, resolve_language,
};
use super::navigation::{matching_sections, section_ids};
use super::{SettingsPane, SettingsPaneEvent, SettingsStatus};

impl SettingsPane {
    /// 统一写盘入口:写成功 → 重载单一事实源与全局 `Settings` → 通知宿主。
    pub(super) fn try_persist(
        &mut self,
        updates: &[(&'static str, Value)],
        cx: &mut Context<'_, Self>,
    ) -> io::Result<()> {
        self.settings_target.save(updates)?;
        self.apply_persisted_runtime(cx);
        Ok(())
    }

    /// 主题应用路径专用:成组偏好对照「编辑动作前观察到的字节」写入。
    /// 外部写入者已改设置时返回 Err(AlreadyExists),不静默覆盖。
    pub(super) fn try_persist_checked(
        &mut self,
        updates: &[(&'static str, Value)],
        expected: &crate::settings_store::SettingsRevision,
        cx: &mut Context<'_, Self>,
    ) -> io::Result<()> {
        self.settings_target.save_checked(expected, updates)?;
        self.apply_persisted_runtime(cx);
        Ok(())
    }

    /// 重载磁盘 → 全局事实源 → `Changed`。成功写盘后调用;损坏标记随之刷新。
    pub(super) fn apply_persisted_runtime(&mut self, cx: &mut Context<'_, Self>) {
        match self.settings_target.load() {
            Ok(result) => {
                let runtime = RuntimeSettings::from_raw(&result.data);
                // 语言解析只在此处发生一次(System 档探测系统 locale);
                // 渲染路径读缓存,不再逐帧调用。
                self.language = resolve_language(runtime.language);
                self.runtime = runtime.clone();
                self.persisted = runtime;
                self.status = result.corrupted.then_some(SettingsStatus::LoadedCorrupted);
            }
            Err(error) => {
                // 写成功但读失败属异常路径:保留最近成功读盘快照,状态条告警。
                log::warn!("设置写盘后重载失败: {error}");
                self.status = Some(SettingsStatus::Failed(error.to_string()));
            }
        }
        // 开机启动显示以真实 .lnk 为准(09 篇 M10.6):默认目标的每次
        // 重载都从 Startup 文件夹回读,避免设置键与文件状态漂移;测试
        // 注入路径不碰真实用户目录。
        if self.settings_target.is_default() {
            self.runtime.launch_at_login = crate::platform::startup::launch_at_login();
        }
        cx.set_global(Settings::from_runtime_with_theme_root(
            &self.runtime,
            self.settings_target.theme_root().as_deref(),
        ));
        // 组件库 locale 与自有文案同步热应用(分层:组件内置文案/自有 catalog)。
        gpui_component::set_locale(self.language.gpui_component_locale());
        cx.emit(SettingsPaneEvent::Changed);
        cx.refresh_windows();
        cx.notify();
    }

    /// 失败可感知:stderr + 语义状态条 + 窗口通知;控件由调用方按
    /// `persisted` 复位。
    pub(super) fn report_failure(
        &mut self,
        error: &io::Error,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        crate::app_shell::try_write_stderr(format_args!(
            "[slterm] settings persist failed: {error}"
        ));
        let message = error.to_string();
        self.status = Some(SettingsStatus::Failed(message.clone()));
        let language = self.language(cx);
        window.push_notification(
            gpui_component::notification::Notification::error(
                language.text(Message::SettingsStatusSaveFailed),
            )
            .message(message),
            cx,
        );
        cx.notify();
    }

    /// 当前渲染语言:构造/每次运行时重载时解析并缓存,渲染路径零系统调用。
    pub(crate) fn language(&self, cx: &App) -> UiLanguage {
        let _ = cx;
        self.language
    }

    /// 设置 tab 展示名(语言热应用后由宿主重读)。
    pub(crate) fn title(&self, cx: &App) -> SharedString {
        self.language(cx).text(Message::SettingsTitle).into()
    }

    /// 语言切换只刷显示项与占位符:实体保留(丢失的焦点/编辑值/undo/
    /// 订阅都不重建)。
    pub(super) fn refresh_localized_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = self.language(cx);
        for entry in &self.selects {
            let selected = entry.select.read(cx).selected_index(cx);
            let items = localized_select_labels(entry.key, entry.values, language);
            entry.select.update(cx, |state, cx| {
                state.set_items(items, window, cx);
                state.set_selected_index(selected, window, cx);
            });
        }
        for (input, key) in [
            (&self.font_family_input, "font_family"),
            (&self.font_size_input, "font_size"),
            (&self.startup_directory_input, "startup_directory"),
        ] {
            let placeholder = localized_input_placeholder(key, language);
            input.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx)
            });
        }
        let placeholder = SharedString::from(language.text(Message::SettingsNavSearch));
        self.search_input.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }

    /// 代码侧改值(恢复默认/语言切换)后把控件显示拉回运行时值。
    pub(super) fn sync_controls_from_runtime(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = self.runtime.clone();
        for entry in &self.selects {
            let current = settings_value_of(&runtime, entry.key);
            let row = entry
                .values
                .iter()
                .position(|value| *value == current)
                .unwrap_or(0);
            entry.select.update(cx, |state, cx| {
                state.set_selected_index(
                    Some(gpui_component::IndexPath::default().row(row)),
                    window,
                    cx,
                );
            });
        }
        self.scroll_speed_slider.update(cx, |state, cx| {
            state.set_value(runtime.scroll_speed, window, cx)
        });
        self.opacity_slider
            .update(cx, |state, cx| state.set_value(runtime.opacity, window, cx));
        let font_family = runtime.font_family.clone().unwrap_or_default();
        self.font_family_input
            .update(cx, |state, cx| state.set_value(font_family, window, cx));
        let font_size = runtime
            .font_size_px
            .map(|size| format!("{size}"))
            .unwrap_or_default();
        self.font_size_input
            .update(cx, |state, cx| state.set_value(font_size, window, cx));
        let startup_directory = runtime.startup_directory.clone().unwrap_or_default();
        self.startup_directory_input.update(cx, |state, cx| {
            state.set_value(startup_directory, window, cx)
        });
    }

    pub(super) fn commit_select(
        &mut self,
        key: &'static str,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let persisted = if key == "scrollback_lines" {
            match value.parse::<f64>() {
                Ok(number) => Value::Number(number),
                Err(_) => Value::Null,
            }
        } else {
            Value::String(value.to_owned())
        };
        if let Err(error) = self.try_persist(&[(key, persisted)], cx) {
            self.report_failure(&error, window, cx);
            self.sync_controls_from_runtime(window, cx);
            return;
        }
        self.status = None;
        if key == "language" {
            // 语言热应用:显示项刷新,控件实体保留;窗口标题等由宿主刷新。
            self.refresh_localized_controls(window, cx);
        }
        cx.notify();
    }

    pub(super) fn toggle(
        &mut self,
        key: &'static str,
        value: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if key == "launch_at_login" {
            // 开机启动是真系统副作用(09 篇 M10.6):先写/删 Startup .lnk
            // 再持久化键值。.lnk 失败则不写设置键,避免「设置显示已开、
            // 实际没登记」;设置键写入失败时 .lnk 已生效,下次打开设置页
            // 以文件为准自愈。
            let result = crate::platform::startup::set_launch_at_login(value)
                .and_then(|()| self.try_persist(&[(key, Value::Bool(value))], cx));
            if let Err(error) = result {
                self.report_failure(&error, window, cx);
                self.sync_controls_from_runtime(window, cx);
                return;
            }
            self.status = None;
            cx.notify();
            return;
        }
        if let Err(error) = self.try_persist(&[(key, Value::Bool(value))], cx) {
            self.report_failure(&error, window, cx);
            self.sync_controls_from_runtime(window, cx);
            return;
        }
        self.status = None;
        cx.notify();
    }

    pub(super) fn save_text_input(
        &mut self,
        key: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let update = if key == "font_size" {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Value::Null
            } else {
                match trimmed.parse::<f32>() {
                    // 宽容解析在值层还会钳制;这里先收敛到 4–96 的 GUI 域。
                    Ok(size) => Value::Number(f64::from(size.clamp(4.0, 96.0))),
                    Err(_) => {
                        self.sync_controls_from_runtime(window, cx);
                        return;
                    }
                }
            }
        } else if value.trim().is_empty() {
            Value::Null
        } else {
            Value::String(value)
        };
        if let Err(error) = self.try_persist(&[(key, update)], cx) {
            self.report_failure(&error, window, cx);
            self.sync_controls_from_runtime(window, cx);
            return;
        }
        self.status = None;
        cx.notify();
    }

    /// 起点目录选择(系统目录选择器;取消不改值)。
    pub(super) fn pick_startup_directory(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = self.language(cx);
        let prompt = language
            .text(Message::SettingsProfilesStartupDirectoryPrompt)
            .to_owned();
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
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
            if !path.is_dir() {
                return;
            }
            let value = path.to_string_lossy().into_owned();
            let _ = this.update_in(cx, |pane, window, cx| {
                pane.save_text_input("startup_directory", value, window, cx);
            });
        })
        .detach();
    }

    pub(super) fn preview_scroll_speed(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        let speed = normalize_scroll_speed(value);
        self.runtime.scroll_speed = speed;
        // 预览即热应用:滚轮处理每次事件读全局 Settings,不触盘。
        if cx.has_global::<Settings>() {
            cx.global_mut::<Settings>().scroll_speed = speed;
        }
        cx.notify();
    }

    pub(super) fn commit_scroll_speed(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let speed = normalize_scroll_speed(value);
        if let Err(error) =
            self.try_persist(&[("scroll_speed", Value::Number(f64::from(speed)))], cx)
        {
            // 读回已存值复位控件与预览(失败路径不得留下脏内存态)。
            self.runtime.scroll_speed = self.persisted.scroll_speed;
            self.scroll_speed_slider.update(cx, |state, cx| {
                state.set_value(self.persisted.scroll_speed, window, cx)
            });
            if cx.has_global::<Settings>() {
                cx.global_mut::<Settings>().scroll_speed = self.persisted.scroll_speed;
            }
            self.report_failure(&error, window, cx);
            return;
        }
        self.status = None;
        cx.notify();
    }

    /// 滑块拖动快路径(09 篇点 41):只改视效全局与壳色 alpha,
    /// 不读设置文件、不重建壁纸纹理;落盘等停手后的 commit。
    pub(super) fn preview_opacity(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        self.runtime.opacity = value.clamp(0.0, 1.0);
        crate::app_shell::wallpaper::set_opacity_live(self.runtime.opacity, cx);
        crate::app_shell::theme::reapply_shell_opacity(cx);
        cx.refresh_windows();
        cx.notify();
    }

    pub(super) fn commit_opacity(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let opacity = value.clamp(0.0, 1.0);
        if let Err(error) = self.try_persist(&[("opacity", Value::Number(f64::from(opacity)))], cx)
        {
            self.runtime.opacity = self.persisted.opacity;
            self.opacity_slider.update(cx, |state, cx| {
                state.set_value(self.persisted.opacity, window, cx)
            });
            self.report_failure(&error, window, cx);
            return;
        }
        self.status = None;
        cx.notify();
    }

    pub(super) fn update_settings_search(&mut self, cx: &mut Context<'_, Self>) {
        let query = self.search_input.read(cx).value().to_string();
        let language = self.language(cx);
        if query.trim().is_empty() {
            if let Some(origin) = self.search_origin.take() {
                self.active_section = origin;
            }
        } else {
            self.search_origin.get_or_insert(self.active_section);
            if let Some(first) = matching_sections(&query, language).first() {
                self.active_section = *first;
            }
        }
        cx.notify();
    }

    pub(super) fn activate_section(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        if index >= section_ids().len() || index == self.active_section {
            return;
        }
        self.active_section = index;
        self.reset_armed = false;
        cx.notify();
    }

    /// 恢复默认:一次点击武装,二次确认执行。失败/损坏文件不覆盖。
    pub(super) fn confirm_reset(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.reset_armed {
            self.reset_armed = true;
            cx.notify();
            return;
        }
        self.reset_armed = false;
        match self.settings_target.reset() {
            Ok(backup) => {
                self.status = Some(SettingsStatus::Reset { backup });
                match self.settings_target.load() {
                    Ok(result) => {
                        let runtime = RuntimeSettings::from_raw(&result.data);
                        self.language = resolve_language(runtime.language);
                        self.runtime = runtime.clone();
                        self.persisted = runtime;
                        if result.corrupted {
                            self.status = Some(SettingsStatus::LoadedCorrupted);
                        }
                    }
                    Err(error) => {
                        self.status = Some(SettingsStatus::Failed(error.to_string()));
                    }
                }
                cx.set_global(Settings::from_runtime_with_theme_root(
                    &self.runtime,
                    self.settings_target.theme_root().as_deref(),
                ));
                self.sync_controls_from_runtime(window, cx);
                cx.emit(SettingsPaneEvent::Changed);
                cx.refresh_windows();
                cx.notify();
            }
            Err(error) => {
                crate::app_shell::try_write_stderr(format_args!(
                    "[slterm] settings reset failed: {error}"
                ));
                self.status = Some(SettingsStatus::Failed(error.to_string()));
                cx.notify();
            }
        }
    }
}

/// 虚拟窗口用例的只读/驱动端(生产不编译)。
#[cfg(all(test, feature = "gpui-test-support"))]
impl SettingsPane {
    pub(crate) fn runtime_for_test(&self) -> &RuntimeSettings {
        &self.runtime
    }

    /// 渲染语言缓存(语言切换用例:提交后无需再探测系统 locale)。
    pub(crate) fn language_for_test(&self) -> UiLanguage {
        self.language
    }

    pub(crate) fn status_for_test(&self) -> Option<&SettingsStatus> {
        self.status.as_ref()
    }

    pub(crate) fn preview_scroll_speed_for_test(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        self.preview_scroll_speed(value, cx);
    }

    pub(crate) fn commit_scroll_speed_for_test(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_scroll_speed(value, window, cx);
    }

    pub(crate) fn scroll_speed_slider_value_for_test(&self, cx: &App) -> f32 {
        self.scroll_speed_slider.read(cx).value().start()
    }

    /// 语言切换用例:控件实体身份跨刷新不变(只换显示项)。
    pub(crate) fn select_entity_id_for_test(&self, key: &str) -> Option<gpui::EntityId> {
        self.selects
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.select.entity_id())
    }

    /// 开关提交路径(与 `Switch::on_click` 同一入口的驱动端)。
    pub(crate) fn toggle_for_test(
        &mut self,
        key: &'static str,
        value: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.toggle(key, value, window, cx);
    }

    /// 文本输入保存路径(字号热应用用例的驱动端)。
    pub(crate) fn save_text_for_test(
        &mut self,
        key: &'static str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.save_text_input(key, value.to_owned(), window, cx);
    }

    /// 下拉确认路径(与 `SelectEvent::Confirm` 订阅同一提交入口)。
    pub(crate) fn commit_select_for_test(
        &mut self,
        key: &'static str,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_select(key, value, window, cx);
    }
}

/// 运行时设置 → 页面控件的持久化记号(代码侧同步控件的单一映射)。
pub(super) fn settings_value_of(runtime: &RuntimeSettings, key: &str) -> String {
    match key {
        "language" => runtime.language.settings_value().to_owned(),
        "theme" => runtime.theme.prompt_name().to_owned(),
        "ligatures" => runtime.ligatures.settings_value().to_owned(),
        "density" => runtime.density.settings_value().to_owned(),
        "tab_reveal" => runtime.tab_reveal.settings_value().to_owned(),
        "cell_width_mode" => runtime.cell_width_mode.settings_value().to_owned(),
        "completion_style" => runtime.completion_style.settings_value().to_owned(),
        "accept" => runtime.accept.settings_value().to_owned(),
        "bell" => runtime.bell.settings_value().to_owned(),
        "notification_duration" => runtime.notification_duration.settings_value().to_owned(),
        "cursor_shape" => runtime
            .cursor_shape
            .map(|shape| shape.settings_value())
            .unwrap_or("beam")
            .to_owned(),
        "cursor_motion" => runtime.cursor_motion.settings_value().to_owned(),
        "new_tab_position" => runtime.new_tab_position.settings_value().to_owned(),
        "vcs_display" => runtime.vcs_display.settings_value().to_owned(),
        "blur" => runtime.blur.settings_value().to_owned(),
        "scrollback_lines" => runtime.scrollback_lines.to_string(),
        _ => String::new(),
    }
}
