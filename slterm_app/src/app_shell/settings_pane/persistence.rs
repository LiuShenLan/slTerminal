//! 设置页写通道与控件同步:统一持久化入口、失败回滚、失败可感知。
//!
//! A2 相位机:所有设置类写盘(普通保存/条件保存/开机启动/恢复默认/hook
//! 授权)都在后台执行器上运行;前台只做三件事——入队、控件即时回显、
//! 消费结果(Committed 应用回读 / Failed 按 `persisted` 快照回滚)。高频
//! 提交在队尾按 key 合并,后台任务串行执行,不占 GPUI 前台线程。
//!
//! 三纪律落点:预览方法只改内存;提交方法只入队;失败时统一回滚 + toast。

use std::io;
use std::path::PathBuf;

use gpui::{App, Context, PathPromptOptions, SharedString, Window};
use gpui_component::WindowExt as _;
use slterm_settings::json::Value;
use slterm_settings::{AgentHook, RuntimeSettings, normalize_scroll_speed};

use crate::app_shell::config::Settings;
use crate::i18n::Message;

use super::localization::{
    UiLanguage, localized_input_placeholder, localized_select_labels, resolve_language,
};
use super::navigation::{matching_sections, section_ids};
use super::{SettingsPane, SettingsPaneEvent, SettingsStatus, SettingsTarget, agents};

/// 后台写任务(A2)。普通保存可在队尾合并;条件保存与系统副作用不合并。
pub(super) enum PersistJob {
    Save {
        updates: Vec<(&'static str, Value)>,
        /// `Some` = 条件保存(主题应用):仅当磁盘字节仍等于该快照才写。
        expected: Option<crate::settings_store::SettingsRevision>,
        /// 成功后关闭主题编辑器(仅主题应用路径)。
        close_theme_editor: bool,
    },
    LaunchAtLogin {
        enabled: bool,
    },
    Reset,
    AgentHook {
        hook: AgentHook,
        enabled: bool,
        /// C5:用户已在设置页确认 SEC-12 命中的 statusLine 原文。
        confirmed: bool,
    },
}

/// hooks 安装失败;C5 携带待确认原文供设置页展示。
struct HookFailure {
    message: String,
    suspicious_statusline: Option<String>,
}

/// 任务结果(后台线程产出,前台消费)。
struct PersistOutcome {
    job: PersistJob,
    /// 设置写结果;`Reset` 携带备份路径。
    write: io::Result<Option<PathBuf>>,
    /// 写成功后的回读(含 corrupted 标记)。
    load: Option<io::Result<crate::settings_store::LoadResult>>,
    /// 开机启动的真实 .lnk 回读(仅默认目标)。
    launch_at_login: Option<bool>,
    /// hooks 安装权威的应用结果(仅 `AgentHook` 任务)。
    hook_apply: Option<Result<(), HookFailure>>,
}

/// 后台执行体:纯阻塞 IO,不触碰任何 GPUI 状态。
fn execute_job(
    job: PersistJob,
    target: SettingsTarget,
    hooks_target: agents::HooksTarget,
) -> PersistOutcome {
    match &job {
        PersistJob::Save {
            updates, expected, ..
        } => {
            let write = match expected {
                Some(expected) => target.save_checked(expected, updates),
                None => target.save(updates),
            }
            .map(|()| None);
            let (load, launch_at_login) = read_after_write(&target, write.is_ok());
            PersistOutcome {
                job,
                write,
                load,
                launch_at_login,
                hook_apply: None,
            }
        }
        PersistJob::LaunchAtLogin { enabled } => {
            let write = crate::platform::startup::set_launch_at_login(*enabled)
                .and_then(|()| target.save(&[("launch_at_login", Value::Bool(*enabled))]))
                .map(|()| None);
            let (load, launch_at_login) = read_after_write(&target, write.is_ok());
            PersistOutcome {
                job,
                write,
                load,
                launch_at_login,
                hook_apply: None,
            }
        }
        PersistJob::Reset => {
            let write = target.reset();
            let (load, launch_at_login) = read_after_write(&target, write.is_ok());
            PersistOutcome {
                job,
                write,
                load,
                launch_at_login,
                hook_apply: None,
            }
        }
        PersistJob::AgentHook {
            hook,
            enabled,
            confirmed,
        } => {
            let write = target
                .save(&[(hook.settings_key(), Value::Bool(*enabled))])
                .map(|()| None);
            let hook_apply = write.as_ref().ok().map(|_| {
                hooks_target
                    .apply_with_confirmation(*hook, *enabled, *confirmed)
                    .map_err(|message| HookFailure {
                        suspicious_statusline: hooks_target.statusline_confirmation(*hook),
                        message,
                    })
            });
            let (load, launch_at_login) = read_after_write(&target, write.is_ok());
            PersistOutcome {
                job,
                write,
                load,
                launch_at_login,
                hook_apply,
            }
        }
    }
}

/// 写成功后的回读:设置文件 + 默认目标下的真实开机启动状态。
fn read_after_write(
    target: &SettingsTarget,
    written: bool,
) -> (
    Option<io::Result<crate::settings_store::LoadResult>>,
    Option<bool>,
) {
    if !written {
        return (None, None);
    }
    let load = Some(target.load());
    let launch_at_login = target
        .is_default()
        .then(crate::platform::startup::launch_at_login);
    (load, launch_at_login)
}

impl SettingsPane {
    /// 统一写入口(异步):合并到队尾普通保存并入队。
    pub(super) fn try_persist(
        &mut self,
        updates: &[(&'static str, Value)],
        cx: &mut Context<'_, Self>,
    ) {
        self.enqueue_persist(
            PersistJob::Save {
                updates: updates.to_vec(),
                expected: None,
                close_theme_editor: false,
            },
            cx,
        );
    }

    /// 条件保存(主题应用):携带编辑动作前观察到的字节快照,不可合并。
    pub(super) fn try_persist_checked(
        &mut self,
        updates: &[(&'static str, Value)],
        expected: &crate::settings_store::SettingsRevision,
        close_theme_editor: bool,
        cx: &mut Context<'_, Self>,
    ) {
        self.enqueue_persist(
            PersistJob::Save {
                updates: updates.to_vec(),
                expected: Some(expected.clone()),
                close_theme_editor,
            },
            cx,
        );
    }

    /// 入队:普通保存与队尾同类合并(同 key 后者覆盖),其余原样排队。
    pub(super) fn enqueue_persist(&mut self, job: PersistJob, cx: &mut Context<'_, Self>) {
        match job {
            PersistJob::Save {
                updates,
                expected: None,
                close_theme_editor: false,
            } => {
                if let Some(PersistJob::Save {
                    updates: queued,
                    expected: None,
                    close_theme_editor: false,
                }) = self.persist_queue.back_mut()
                {
                    for (key, value) in updates {
                        queued.retain(|(existing, _)| *existing != key);
                        queued.push((key, value));
                    }
                    self.start_next_persist(cx);
                    return;
                }
                self.persist_queue.push_back(PersistJob::Save {
                    updates,
                    expected: None,
                    close_theme_editor: false,
                });
            }
            other => self.persist_queue.push_back(other),
        }
        self.start_next_persist(cx);
    }

    /// 启动下一个后台写任务(串行:同一时刻至多一个在飞)。
    fn start_next_persist(&mut self, cx: &mut Context<'_, Self>) {
        if self.persist_running {
            return;
        }
        let Some(job) = self.persist_queue.pop_front() else {
            return;
        };
        self.persist_running = true;
        let target = self.settings_target.clone();
        let hooks_target = self.hooks_target.clone();
        let task = cx
            .background_executor()
            .spawn(async move { execute_job(job, target, hooks_target) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            let _ = this.update_in(cx, |pane, window, cx| {
                pane.finish_persist(outcome, window, cx);
            });
        })
        .detach();
    }

    /// 消费后台结果:成功应用回读与任务副作用;失败按 `persisted` 回滚。
    fn finish_persist(
        &mut self,
        outcome: PersistOutcome,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.persist_running = false;
        let PersistOutcome {
            job,
            write,
            load,
            launch_at_login,
            hook_apply,
        } = outcome;
        match write {
            Ok(backup) => {
                let mut language_changed = false;
                if let Some(load) = load {
                    match load {
                        Ok(result) => {
                            let previous = self.language;
                            self.apply_load_result(result, launch_at_login, cx);
                            language_changed = self.language != previous;
                        }
                        Err(error) => {
                            log::warn!("设置写盘后重载失败: {error}");
                            self.status = Some(SettingsStatus::Failed(error.to_string()));
                        }
                    }
                }
                match &job {
                    PersistJob::Reset => {
                        self.status = Some(SettingsStatus::Reset { backup });
                    }
                    PersistJob::AgentHook { hook, enabled, .. } => {
                        match hook_apply.unwrap_or(Ok(())) {
                            Ok(()) => {
                                self.agents.suspicious_statusline = None;
                                self.agents.feedback = Some((*hook, Ok(*enabled)));
                            }
                            // C5:命中 SEC-12 时保留原文,行内出现「确认并重试」。
                            Err(failure) => {
                                self.agents.suspicious_statusline = failure.suspicious_statusline;
                                self.agents.feedback = Some((*hook, Err(failure.message)));
                            }
                        }
                        self.refresh_agents(cx);
                    }
                    PersistJob::Save {
                        close_theme_editor: true,
                        ..
                    } => {
                        self.theme_editor = None;
                    }
                    _ => {
                        self.status = None;
                    }
                }
                if language_changed {
                    self.refresh_localized_controls(window, cx);
                }
            }
            Err(error) => {
                // 失败回滚:运行时、控件与全局事实源回到最近一次成功读盘快照。
                self.runtime = self.persisted.clone();
                self.sync_controls_from_runtime(window, cx);
                self.publish_global_settings(cx);
                self.report_failure(&error, window, cx);
                if let PersistJob::AgentHook { hook, .. } = &job {
                    self.agents.suspicious_statusline = None;
                    self.agents.feedback = Some((*hook, Err(error.to_string())));
                    self.refresh_agents(cx);
                }
            }
        }
        cx.notify();
        self.start_next_persist(cx);
    }

    /// 应用一次成功回读:语言/运行时/持久化快照/损坏标记/全局事实源。
    fn apply_load_result(
        &mut self,
        result: crate::settings_store::LoadResult,
        launch_at_login: Option<bool>,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = RuntimeSettings::from_raw(&result.data);
        self.language = resolve_language(runtime.language);
        self.runtime = runtime.clone();
        self.persisted = runtime;
        self.status = result.corrupted.then_some(SettingsStatus::LoadedCorrupted);
        if let Some(launch) = launch_at_login {
            self.runtime.launch_at_login = launch;
        }
        // B57:拖动中的透明度预览优先于回读的持久化值,回包不得闪回旧值。
        if let Some(live) = self.live_opacity {
            self.runtime.opacity = live;
            crate::app_shell::wallpaper::set_opacity_live(live, cx);
            crate::app_shell::theme::reapply_shell_opacity(cx);
        }
        self.publish_global_settings(cx);
    }

    /// 把 `self.runtime` 发布为全局事实源并广播。
    fn publish_global_settings(&mut self, cx: &mut Context<'_, Self>) {
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
        self.try_persist(&[(key, persisted)], cx);
        self.status = None;
        cx.notify();
        let _ = window;
    }

    pub(super) fn toggle(
        &mut self,
        key: &'static str,
        value: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if key == "launch_at_login" {
            // 开机启动是真系统副作用:写/删 Startup .lnk 与设置键同在一个
            // 后台任务内完成,失败整笔回滚。
            self.enqueue_persist(PersistJob::LaunchAtLogin { enabled: value }, cx);
            self.status = None;
            cx.notify();
            let _ = window;
            return;
        }
        self.try_persist(&[(key, Value::Bool(value))], cx);
        self.status = None;
        cx.notify();
        let _ = window;
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
        self.try_persist(&[(key, update)], cx);
        self.status = None;
        cx.notify();
        let _ = window;
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
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let speed = normalize_scroll_speed(value);
        // 失败回滚统一由 `finish_persist` 按 persisted 快照执行。
        self.try_persist(&[("scroll_speed", Value::Number(f64::from(speed)))], cx);
        self.status = None;
        cx.notify();
    }

    /// 滑块拖动快路径(09 篇点 41):只改视效全局与壳色 alpha,
    /// 不读设置文件、不重建壁纸纹理;落盘等停手后的 commit。
    pub(super) fn preview_opacity(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        self.runtime.opacity = value.clamp(0.0, 1.0);
        self.live_opacity = Some(self.runtime.opacity);
        crate::app_shell::wallpaper::set_opacity_live(self.runtime.opacity, cx);
        crate::app_shell::theme::reapply_shell_opacity(cx);
        cx.refresh_windows();
        cx.notify();
    }

    pub(super) fn commit_opacity(
        &mut self,
        value: f32,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let opacity = value.clamp(0.0, 1.0);
        // 松手后预览态结束:回包以持久化值为准。
        self.live_opacity = None;
        // 失败回滚统一由 `finish_persist` 按 persisted 快照执行。
        self.try_persist(&[("opacity", Value::Number(f64::from(opacity)))], cx);
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

    /// 恢复默认:一次点击武装,二次确认入队后台执行(A2)。
    pub(super) fn confirm_reset(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.reset_armed {
            self.reset_armed = true;
            cx.notify();
            return;
        }
        self.reset_armed = false;
        self.persist_queue.push_back(PersistJob::Reset);
        self.start_next_persist(cx);
        cx.notify();
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

    /// A2 相位机探针:队列与在飞状态。
    pub(crate) fn persist_state_for_test(&self) -> (usize, bool) {
        (self.persist_queue.len(), self.persist_running)
    }

    pub(crate) fn preview_scroll_speed_for_test(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        self.preview_scroll_speed(value, cx);
    }

    pub(crate) fn preview_opacity_for_test(&mut self, value: f32, cx: &mut Context<'_, Self>) {
        self.preview_opacity(value, cx);
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
