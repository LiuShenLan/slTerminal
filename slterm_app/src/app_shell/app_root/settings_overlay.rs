//! 无项目设置浮层：AppRoot 独立持有实体、应用级 ViewKey 与焦点注册。
//!
//! 页面内容由 `settings_controls` 的字段表驱动：标量控件实时写运行值与
//! 唯一写队列，AppRoot 负责全局快照热应用与字段级副作用。

use std::collections::BTreeMap;
use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px,
    relative,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};
use slterm_settings::SettingsRegistry;
use slterm_settings::json::Value;
use slterm_workspace::{PanelGroupId, ProjectId, ViewKey};

use crate::ai_hook::draft_host::HooksDraftOwner;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::{FocusRegistration, FocusToken};
use super::panel_host::PanelContent;
use super::settings_controls::{
    FIELDS, SettingsControl, SettingsFieldUi, fields_for_page, typed_value,
};
use crate::settings_state::{
    SettingsPageId, SettingsViewState, SettingsViewStateStore, theme_field_followups,
};

pub(super) struct SettingsView {
    pub(super) focus_handle: gpui::FocusHandle,
    selected_page: SettingsPageId,
    /// 应用级浮层可用 Escape／返回工作区关闭；项目面板走统一面板关闭链。
    overlay: bool,
    view_key: ViewKey,
    view_states: Option<SettingsViewStateStore>,
    /// 字段修改统一交给 AppRoot：运行值、写队列、热应用与副作用单点。
    pub(super) root: WeakEntity<AppRoot>,
    /// Windows 集成端口：设置页展示实际启动项与托盘可用性的只读事实。
    windows: std::sync::Arc<dyn crate::platform::windows_integration::WindowsIntegration>,
    /// 文本／颜色字段的输入实体按字段键缓存，跨页切换保留未提交编辑。
    inputs: BTreeMap<&'static str, Entity<InputState>>,
    /// 仅用于保持 InputState 订阅存活，不直接读取。
    #[allow(dead_code)]
    input_subscriptions: Vec<Subscription>,
    /// 校验失败瞬态标记：只在本次视图内提示，不进入持久状态。
    invalid_fields: BTreeMap<&'static str, Instant>,
    /// 稳定面板身份；项目设置面板安装后绑定，Hooks 草稿键据此隔离。
    panel_id: Option<slterm_workspace::PanelId>,
    hooks_layer: crate::ai_hook::draft_host::HooksDraftLayer,
    /// Hooks JSON 多行编辑器；内容由帧间同步与草稿一致。
    hooks_input: Option<Entity<InputState>>,
    #[allow(dead_code)]
    hooks_input_subscription: Option<Subscription>,
    hooks_sync_key: Option<crate::ai_hook::draft_host::HooksDraftKey>,
    hooks_synced_generation: u64,
}

/// 设置视图的共享运行根与平台端口：构造参数聚合，避免位置参数继续膨胀。
pub(super) struct SettingsViewRoots {
    pub(super) root: WeakEntity<AppRoot>,
    pub(super) windows:
        std::sync::Arc<dyn crate::platform::windows_integration::WindowsIntegration>,
}

impl SettingsView {
    fn new_overlay(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        roots: SettingsViewRoots,
        initial_values: BTreeMap<&'static str, Option<Value>>,
    ) -> Self {
        Self::with_state(
            window,
            cx,
            ViewKey::AppSettingsOverlay,
            None,
            true,
            roots,
            initial_values,
        )
    }

    pub(super) fn with_state(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        view_key: ViewKey,
        view_states: Option<SettingsViewStateStore>,
        overlay: bool,
        roots: SettingsViewRoots,
        initial_values: BTreeMap<&'static str, Option<Value>>,
    ) -> Self {
        let SettingsViewRoots { root, windows } = roots;
        let selected_page = view_states
            .as_ref()
            .and_then(|store| store.state(&view_key))
            .map(|state| state.selected_page())
            .unwrap_or(SettingsPageId::Appearance);
        // 重建视图（关闭重开、重启）恒回 User 层；项目／Local 层由用户显式切换。
        let hooks_layer = crate::ai_hook::draft_host::HooksDraftLayer::User;
        let mut inputs = BTreeMap::new();
        let mut input_subscriptions = Vec::new();
        for field in FIELDS {
            if !matches!(
                field.control,
                SettingsControl::Text | SettingsControl::Color
            ) {
                continue;
            }
            let initial = text_field_value(initial_values.get(field.key).and_then(Option::as_ref));
            let key = field.key;
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_value(initial, window, cx);
                state
            });
            let subscription = cx.subscribe_in(
                &input,
                window,
                move |view, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        view.on_input_change(key, input.clone(), window, cx);
                    }
                },
            );
            inputs.insert(key, input);
            input_subscriptions.push(subscription);
        }
        let hooks_input = cx.new(|cx| InputState::new(window, cx).multi_line(true));
        let hooks_input_subscription = cx.subscribe_in(
            &hooks_input,
            window,
            |view, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    view.on_hooks_text_change(input.clone(), window, cx);
                }
            },
        );
        Self {
            focus_handle: cx.focus_handle(),
            selected_page,
            overlay,
            view_key,
            view_states,
            root,
            windows,
            inputs,
            input_subscriptions,
            invalid_fields: BTreeMap::new(),
            panel_id: None,
            hooks_layer,
            hooks_input: Some(hooks_input),
            hooks_input_subscription: Some(hooks_input_subscription),
            hooks_sync_key: None,
            hooks_synced_generation: 0,
        }
    }

    /// Hooks 层可绑定的项目：仅项目 Settings 面板提供 Project／Local 层。
    pub(super) fn hooks_project(&self) -> Option<ProjectId> {
        match &self.view_key {
            ViewKey::Settings { project_id } => Some(*project_id),
            _ => None,
        }
    }

    /// 应用级浮层身份：没有项目面板 PanelId，Hooks 草稿归 Overlay 所有者。
    pub(super) const fn is_overlay(&self) -> bool {
        self.overlay
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) const fn view_key(&self) -> ViewKey {
        ViewKey::AppSettingsOverlay
    }

    pub(super) const fn current_page(&self) -> SettingsPageId {
        self.selected_page
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) const fn selected_page_for_test(&self) -> SettingsPageId {
        self.selected_page
    }

    fn select_page(&mut self, page: SettingsPageId, cx: &mut Context<'_, Self>) {
        if self.selected_page == page {
            return;
        }
        self.selected_page = page;
        // AgentHooks 页首次进入才探测安装状态：探测是阻塞 I/O，只在后台执行。
        if page == SettingsPageId::AgentHooks
            && let Some(root) = self.root.upgrade()
        {
            root.update(cx, |root, cx| root.request_agent_hooks_inspect(cx));
        }
        if page == SettingsPageId::Hooks {
            self.request_hooks_draft(cx);
        }
        if let Some(store) = &self.view_states {
            // 声明式页内参数随页面保留；选中页是唯一可变的持久事实。
            let params = store
                .state(&self.view_key)
                .map(|state| state.declared_page_params().clone())
                .unwrap_or_default();
            let mut state = SettingsViewState::new(page);
            for (key, value) in params {
                state = state.with_page_param(&key, &value);
            }
            store.checkpoint(&self.view_key, state);
        }
        cx.notify();
    }

    fn request_close(cx: &mut Context<'_, Self>) {
        cx.emit(SettingsViewEvent::Close);
    }

    /// 各字段当前值快照；借用只在函数内完成，渲染与监听不持有 store 引用。
    fn field_values(&self, cx: &App) -> BTreeMap<&'static str, Option<Value>> {
        let registry = SettingsRegistry::new();
        let root_entity = self.root.upgrade();
        let root_ref = root_entity.as_ref().map(|root| root.read(cx));
        let fallback = slterm_settings::RawSettings::default();
        let raw = root_ref
            .as_ref()
            .map_or(&fallback, |root| root.settings.values());
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

    /// 控件提交：注册表校验是唯一闸门；非法值只显示瞬态提示。
    fn commit_field(&mut self, key: &'static str, value: Value, cx: &mut Context<'_, Self>) {
        let registry = SettingsRegistry::new();
        if registry.validate(key, &value) != Some(true) {
            self.invalid_fields.insert(key, Instant::now());
            cx.notify();
            return;
        }
        self.invalid_fields.remove(key);
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| root.apply_settings_field(key, value, cx));
        }
        cx.notify();
    }

    fn on_input_change(
        &mut self,
        key: &'static str,
        input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let text = input.read(cx).value().to_string();
        self.commit_field(key, Value::String(text), cx);
    }

    /// Hooks JSON 编辑只写内存草稿；非法文本保留最后合法模型。
    fn on_hooks_text_change(
        &mut self,
        input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(key) = self.hooks_sync_key else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| root.update_hooks_draft_text(key, &text, cx));
        }
    }

    /// 面板安装后绑定稳定身份；载入由 AppRoot 在绑定后显式触发。
    pub(super) fn bind_panel_id(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        cx: &mut Context<'_, Self>,
    ) {
        if self.panel_id == Some(panel_id) {
            return;
        }
        self.panel_id = Some(panel_id);
        cx.notify();
    }

    pub(super) const fn panel_id_value(&self) -> Option<slterm_workspace::PanelId> {
        self.panel_id
    }

    pub(super) const fn hooks_layer_value(&self) -> crate::ai_hook::draft_host::HooksDraftLayer {
        self.hooks_layer
    }

    pub(super) fn set_hooks_layer_value(
        &mut self,
        layer: crate::ai_hook::draft_host::HooksDraftLayer,
    ) {
        self.hooks_layer = layer;
    }

    pub(super) fn reset_hooks_sync(&mut self) {
        self.hooks_sync_key = None;
    }

    pub(super) fn hooks_input(&self) -> Option<&Entity<InputState>> {
        self.hooks_input.as_ref()
    }

    /// 输入框内容是否已经对应当前草稿键与同步代次。
    pub(super) fn hooks_sync_matches(
        &self,
        key: crate::ai_hook::draft_host::HooksDraftKey,
        generation: u64,
    ) -> bool {
        self.hooks_sync_key == Some(key) && self.hooks_synced_generation == generation
    }

    /// 帧间同步：把草稿文本回填到多行编辑器并记录已同步代次。
    pub(super) fn apply_hooks_sync(
        &mut self,
        key: crate::ai_hook::draft_host::HooksDraftKey,
        text: String,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(input) = &self.hooks_input {
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        }
        self.hooks_sync_key = Some(key);
        self.hooks_synced_generation = generation;
    }

    fn render_page(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        if self.selected_page == SettingsPageId::AgentHooks {
            return super::settings_agents::render_agent_hooks_page(self, cx);
        }
        if self.selected_page == SettingsPageId::Hooks {
            return super::hooks_page::render_hooks_page(self, window, cx);
        }
        let values = self.field_values(cx);
        let mut children: Vec<AnyElement> = Vec::new();
        let mut current_section = None;
        for field in fields_for_page(self.selected_page) {
            if current_section != Some(field.section) {
                current_section = Some(field.section);
                children.push(
                    div()
                        .pt_3()
                        .pb_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            cx.global::<crate::app_shell::config::Settings>()
                                .language
                                .text(field.section),
                        )
                        .into_any_element(),
                );
            }
            children.push(self.render_field(
                field,
                values.get(field.key).and_then(Option::as_ref),
                cx,
            ));
        }
        if children.is_empty() {
            children.push(
                div()
                    .p_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        cx.global::<crate::app_shell::config::Settings>()
                            .language
                            .text(self.selected_page.message()),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(children)
            .into_any_element()
    }

    fn render_field(
        &mut self,
        field: &'static SettingsFieldUi,
        current: Option<&Value>,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let registry = SettingsRegistry::new();
        let def = registry
            .lookup(field.key)
            .expect("控件字段必须已在注册表登记");
        let key = field.key;
        let control: AnyElement = match &field.control {
            SettingsControl::Toggle => {
                let enabled = matches!(current, Some(Value::Bool(true)));
                let next = Value::Bool(!enabled);
                div()
                    .id(gpui::ElementId::Name(
                        format!("settings-toggle-{key}").into(),
                    ))
                    .w(px(36.0))
                    .h(px(20.0))
                    .rounded_full()
                    .border_1()
                    .border_color(if enabled {
                        cx.theme().accent
                    } else {
                        cx.theme().border
                    })
                    .bg(if enabled {
                        cx.theme().accent.opacity(0.35)
                    } else {
                        cx.theme().transparent
                    })
                    .cursor_pointer()
                    .debug_selector(move || format!("settings-field-{key}-toggle"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| view.commit_field(key, next.clone(), cx)),
                    )
                    .child(
                        div()
                            .size(px(14.0))
                            .m(px(3.0))
                            .rounded_full()
                            .when(enabled, |dot| dot.ml(px(19.0)))
                            .bg(if enabled {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    .into_any_element()
            }
            SettingsControl::Choice(options) => {
                let current_text = value_text(current);
                let pills = options.iter().map(|option| {
                    let is_selected = current_text.as_deref() == Some(option.value);
                    let value = typed_value(def, option.value);
                    let label = option
                        .label
                        .map(|message| language.text(message))
                        .unwrap_or(option.value);
                    let pill_key = option.value;
                    // 不受支持的材质档位不可选：不提供点击入口，不宣称静默生效。
                    let disabled = !crate::platform::windows_integration::choice_is_selectable(
                        key,
                        option.value,
                        &*self.windows,
                    );
                    div()
                        .id(gpui::ElementId::Name(
                            format!("settings-choice-{key}-{pill_key}").into(),
                        ))
                        .px_2()
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .text_xs()
                        .border_color(if is_selected {
                            cx.theme().accent
                        } else {
                            cx.theme().border
                        })
                        .text_color(if is_selected {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .debug_selector(move || format!("settings-choice-{key}-{pill_key}"))
                        .when(disabled, |pill| pill.opacity(0.4))
                        .when(!disabled, |pill| {
                            pill.cursor_pointer().on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    view.commit_field(key, value.clone(), cx)
                                }),
                            )
                        })
                        .child(label)
                });
                div()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap_1()
                    .children(pills)
                    .into_any_element()
            }
            SettingsControl::Stepper { min, max, step } => {
                let (min, max, step) = (*min, *max, *step);
                let current_number = current.and_then(value_number);
                let display = current_number
                    .map(format_number)
                    .unwrap_or_else(|| "—".to_owned());
                let step_value = move |direction: f64| {
                    let base = current_number.unwrap_or(min);
                    let next = (base + direction * step).clamp(min, max);
                    let snapped = min + ((next - min) / step).round() * step;
                    Value::Number(if (snapped - snapped.round()).abs() < 1e-9 {
                        snapped.round()
                    } else {
                        snapped
                    })
                };
                let down = step_value(-1.0);
                let up = step_value(1.0);
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(gpui::ElementId::Name(
                                format!("settings-step-down-{key}").into(),
                            ))
                            .px_2()
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .cursor_pointer()
                            .debug_selector(move || format!("settings-field-{key}-down"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    view.commit_field(key, down.clone(), cx)
                                }),
                            )
                            .child("−"),
                    )
                    .child(
                        div()
                            .min_w(px(44.0))
                            .text_center()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(display),
                    )
                    .child(
                        div()
                            .id(gpui::ElementId::Name(
                                format!("settings-step-up-{key}").into(),
                            ))
                            .px_2()
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .cursor_pointer()
                            .debug_selector(move || format!("settings-field-{key}-up"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    view.commit_field(key, up.clone(), cx)
                                }),
                            )
                            .child("+"),
                    )
                    .into_any_element()
            }
            SettingsControl::Text | SettingsControl::Color => {
                let input = self.inputs.get(key).expect("文本字段的输入实体必须已创建");
                div()
                    .w(px(220.0))
                    .debug_selector(move || format!("settings-field-{key}-input"))
                    .child(Input::new(input))
                    .into_any_element()
            }
        };
        let invalid = self.invalid_fields.contains_key(key);
        // Windows 实际状态说明：设置值只表达意图，外部改动必须显形。
        let note =
            crate::platform::windows_integration::settings_note(key, current, &*self.windows);
        div()
            .py_1()
            .flex()
            .justify_between()
            .items_start()
            .gap_4()
            .debug_selector(move || format!("settings-field-{key}"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(language.text(field.label)),
                    )
                    .when_some(field.description, |element, description| {
                        element.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text(description)),
                        )
                    })
                    .when(invalid, |element| {
                        element.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .debug_selector(move || format!("settings-field-{key}-invalid"))
                                .child(
                                    language.text(crate::i18n::Message::SettingsStatusInvalidValue),
                                ),
                        )
                    })
                    .when_some(note, |element, note| {
                        let (message, path) = integration_note_message(&note);
                        let text = match path {
                            Some(path) => language.format(message, &[("path", &path)]),
                            None => language.text(message).to_owned(),
                        };
                        element.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .debug_selector(move || format!("settings-field-{key}-actual"))
                                .child(text),
                        )
                    }),
            )
            .child(control)
            .into_any_element()
    }
}

/// 当前字段值的文本形态；数值去掉多余的 `.0`。
fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(format_number(*number)),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Null) | None => None,
        Some(Value::Array(_)) | Some(Value::Object(_)) => None,
    }
}

fn text_field_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn value_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => Some(*number),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn format_number(number: f64) -> String {
    if (number - number.round()).abs() < 1e-9 {
        format!("{}", number.round() as i64)
    } else {
        let mut text = format!("{number:.2}");
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

enum SettingsViewEvent {
    Close,
}

impl EventEmitter<SettingsViewEvent> for SettingsView {}

impl SettingsView {
    /// 设置页可见状态：加载降级、单字段回退与写盘失败都必须显式提示。
    fn status_messages(&self, cx: &App) -> Vec<String> {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut messages = Vec::new();
        let Some(root) = self.root.upgrade() else {
            return messages;
        };
        let root = root.read(cx);
        match root.settings_load_warning {
            Some(crate::settings_state::SettingsLoadWarning::Corrupted) => {
                messages.push(
                    language
                        .text(crate::i18n::Message::SettingsStatusCorrupted)
                        .to_owned(),
                );
            }
            Some(crate::settings_state::SettingsLoadWarning::UnsupportedVersion(version)) => {
                let version = version.to_string();
                messages.push(language.format(
                    crate::i18n::Message::SettingsStatusUnsupportedVersion,
                    &[("version", &version)],
                ));
            }
            Some(crate::settings_state::SettingsLoadWarning::ReadFailed) => {
                messages.push(
                    language
                        .text(crate::i18n::Message::SettingsStatusReadFailed)
                        .to_owned(),
                );
            }
            None => {}
        }
        let registry = SettingsRegistry::new();
        let invalid = registry
            .fields()
            .iter()
            .filter(|definition| {
                matches!(
                    registry
                        .recover(root.settings.values(), definition.key)
                        .map(|recovery| recovery.status),
                    Some(slterm_settings::keys::SettingRecoveryStatus::Invalid)
                )
            })
            .count();
        if invalid > 0 {
            let count = invalid.to_string();
            messages.push(language.format(
                crate::i18n::Message::SettingsStatusInvalidFields,
                &[("count", &count)],
            ));
        }
        if root.settings.persist_failed() {
            messages.push(
                language
                    .text(crate::i18n::Message::SettingsStatusSaveFailed)
                    .to_owned(),
            );
        }
        messages
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let status_messages = self.status_messages(cx);
        let selected = self.selected_page;
        let page_list = SettingsPageId::ALL
            .into_iter()
            .enumerate()
            .map(|(index, page)| {
                let is_selected = page == selected;
                div()
                    .id(("settings-page", index))
                    .h(px(24.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_sm()
                    .when(is_selected, |element| {
                        element.text_color(cx.theme().foreground)
                    })
                    .when(!is_selected, |element| {
                        element.text_color(cx.theme().muted_foreground)
                    })
                    .debug_selector(move || format!("settings-page-{index}"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| view.select_page(page, cx)),
                    )
                    .child(language.text(page.message()))
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .key_context("SettingsPanel")
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                if view.overlay && event.keystroke.key == "escape" {
                    Self::request_close(cx);
                }
            }))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(language.text(crate::i18n::Message::SettingsTitle)),
                    )
                    .when(self.overlay, |element| {
                        element.child(
                            div()
                                .text_color(cx.theme().accent)
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_view, _event, _window, cx| {
                                        Self::request_close(cx)
                                    }),
                                )
                                .child(language.text(crate::i18n::Message::SettingsNavBack)),
                        )
                    }),
            )
            .when(!status_messages.is_empty(), |element| {
                element.child(
                    div()
                        .debug_selector(|| "settings-status".to_owned())
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_4()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().danger.opacity(0.4))
                        .children(status_messages.into_iter().map(|message| {
                            div().text_xs().text_color(cx.theme().danger).child(message)
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_grow(1.0)
                    .min_h_0()
                    .child(
                        div()
                            .w(px(160.0))
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .py_1()
                            .children(page_list),
                    )
                    .child(
                        div()
                            .id("settings-page-scroll")
                            .flex()
                            .flex_col()
                            .flex_grow(1.0)
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .p_4()
                            .debug_selector(|| "settings-page-content".to_owned())
                            .child(self.render_page(window, cx)),
                    ),
            )
    }
}

pub(super) struct SettingsOverlayHost {
    pub(super) view: Entity<SettingsView>,
    registration: FocusRegistration,
    return_focus: FocusToken,
    _close: Subscription,
}

impl AppRoot {
    /// 设置入口：有活跃项目时在聚焦组打开项目设置面板（同项目单例），
    /// 无项目时打开应用级浮层。重复入口只还焦既有视图。
    pub(crate) fn open_settings_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        // 浮层打开期间继续共享应用设置并保留草稿：入口先还焦既有浮层；
        // 项目 Settings 面板只在浮层关闭后的下一次入口使用。
        if self.settings_overlay.is_some() || self.snapshot.registry().active().is_none() {
            return self.open_settings_overlay_from_request(window, cx);
        }
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let view_key = ViewKey::Settings { project_id };
        if self.panel_for_view(project_id, &view_key, cx).is_ok() {
            return self.activate_view(view_key, window, cx);
        }
        let target_group = match self.focused_group(project_id, cx)? {
            Some(group_id) => group_id,
            None => {
                PanelGroupId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7")
            }
        };
        let candidate =
            self.snapshot
                .with_opened_view(project_id, target_group, view_key.clone())?;
        candidate.validate()?;
        let view_states = self.side_view_persistence.settings_view_states();
        let settings_key = view_key.clone();
        let root_handle = cx.entity().downgrade();
        let windows = self.windows.clone();
        let initial_values = super::settings_controls::initial_field_values(self.settings.values());
        let panel_id = self.install_panel(
            super::documents::DocumentPanelTarget {
                project_id,
                view: view_key,
                focus: true,
                activate_project: false,
            },
            window,
            cx,
            move |window, cx| {
                let view = cx.new(|cx| {
                    SettingsView::with_state(
                        window,
                        cx,
                        settings_key,
                        Some(view_states),
                        false,
                        SettingsViewRoots {
                            root: root_handle,
                            windows,
                        },
                        initial_values,
                    )
                });
                let focus = view.read(cx).focus_handle.clone();
                (PanelContent::Settings(view), focus)
            },
        )?;
        self.commit_project_snapshot(candidate, cx);
        self.bind_settings_panel(panel_id, project_id, cx);
        self.request_agent_hooks_if_selected(project_id, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn open_settings_overlay_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(host) = &self.settings_overlay {
            self.focus_manager
                .request_focus(host.registration.token(), window, cx)?;
            cx.notify();
            return Ok(());
        }
        // 项目 Settings 面板由设置阶段接入；此处不得把浮层伪装成项目面板。
        if self.snapshot.registry().active().is_some() {
            return Err(AppRootError::MissingPanel);
        }
        self.sync_focus(window, cx);
        let return_focus = self
            .focus_manager
            .observed_focus_token()
            .unwrap_or_else(|| self.focus_manager.workspace_token());
        let root_handle = cx.entity().downgrade();
        let windows = self.windows.clone();
        let initial_values = super::settings_controls::initial_field_values(self.settings.values());
        let view = cx.new(|cx| {
            SettingsView::new_overlay(
                window,
                cx,
                SettingsViewRoots {
                    root: root_handle,
                    windows,
                },
                initial_values,
            )
        });
        let registration = self.focus_manager.register(
            &view.read(cx).focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self
            .focus_manager
            .request_focus(registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let close = cx.subscribe_in(&view, window, |root, _, event, window, cx| match event {
            SettingsViewEvent::Close => {
                if let Err(error) = root.close_settings_overlay(window, cx) {
                    log::warn!("设置浮层关闭失败: {error:?}");
                }
            }
        });
        let restored_page = view.read(cx).current_page();
        self.settings_overlay = Some(SettingsOverlayHost {
            view,
            registration,
            return_focus,
            _close: close,
        });
        if restored_page == SettingsPageId::AgentHooks {
            self.request_agent_hooks_inspect(cx);
        }
        cx.notify();
        Ok(())
    }

    fn close_settings_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.hook_drafts.owner_has_dirty(HooksDraftOwner::Overlay) {
            super::events::notify_hooks_draft_close_pending(cx);
            return Err(AppRootError::HooksDraftPending);
        }
        let Some(host) = self.settings_overlay.take() else {
            return Ok(());
        };
        if let Err(error) = self.focus_manager.restore_focus(
            &[host.return_focus, self.focus_manager.workspace_token()],
            window,
            cx,
        ) {
            self.settings_overlay = Some(host);
            return Err(error.into());
        }
        self.focus_manager.unregister(host.registration.token())?;
        // 浮层关闭即释放其 Hooks 草稿与文档，不留下无所有者的应用级草稿。
        self.discard_hooks_owner_for_close(HooksDraftOwner::Overlay, cx);
        cx.notify();
        Ok(())
    }

    /// 字段修改的唯一入口：注册表校验 → 运行值与写队列 → 全局快照热应用。
    pub(super) fn apply_settings_field(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<'_, Self>,
    ) {
        let registry = SettingsRegistry::new();
        let Some(def) = registry.lookup(key) else {
            return;
        };
        if registry.validate(key, &value) != Some(true) {
            return;
        }
        let now = Instant::now();
        let accepted = match def.save_mode {
            slterm_settings::SettingsSaveMode::Automatic => {
                self.settings.set_auto(key, value.clone(), now)
            }
            slterm_settings::SettingsSaveMode::Explicit => {
                self.settings.queue_explicit(&[(key, value.clone())], now)
            }
        };
        if !accepted {
            return;
        }
        // 联动清除：选择内置主题即撤销自定义主题覆盖，否则旧主题继续生效。
        for followup in theme_field_followups(key) {
            self.settings.clear_auto(followup, now);
        }
        match key {
            "launch_at_login" => {
                let enabled = matches!(value, Value::Bool(true));
                if let Err(error) = self.windows.set_login_start(enabled) {
                    log::warn!("更新开机启动失败: {error}");
                }
            }
            "tray" => self
                .windows
                .set_tray_enabled(matches!(value, Value::Bool(true))),
            _ => {}
        }
        let theme_commit = match (key, &value) {
            ("custom_theme", Value::String(id)) => Some(id.clone()),
            _ => None,
        };
        if let Some(id) = theme_commit {
            self.reload_theme_definition(id, cx);
        } else {
            self.reload_settings_snapshot(cx);
        }
        cx.notify();
    }

    /// 自定义主题的冷路径预载：读盘与解析只在后台执行，完成后安装缓存
    /// 并重建快照；前台提交路径不做任何主题文件 I/O。
    fn reload_theme_definition(&mut self, id: String, cx: &mut Context<'_, Self>) {
        let store = self.theme_store();
        let cancellation = super::task_service::CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let load_id = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    store.load(&load_id).and_then(|document| {
                        document
                            .resolve()
                            .map_err(crate::theme_library::StoreError::Document)
                    })
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.finish_theme_load(id, result, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(super::task_service::TaskId::ThemeLoad, cancellation, task)
        {
            log::warn!("主题预载任务注册失败: {error:?}");
            self.reload_settings_snapshot(cx);
        }
    }

    fn finish_theme_load(
        &mut self,
        id: String,
        result: Result<slterm_settings::ThemeDefinition, crate::theme_library::StoreError>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(super::task_service::TaskId::ThemeLoad);
        match result {
            Ok(definition) => crate::theme_library::cache::install(&id, definition),
            Err(error) => log::warn!("主题 {id} 预载失败: {error}"),
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    /// 主题库根：生产走默认数据目录；测试注入隔离根，不触碰真实 home。
    fn theme_store(&self) -> crate::theme_library::ThemeLibraryStore {
        #[cfg(test)]
        if let Some(root) = &self.theme_root_for_test {
            return crate::theme_library::ThemeLibraryStore::new(root);
        }
        crate::theme_library::ThemeLibraryStore::default()
    }

    /// 运行值 → 全局设置快照 + 主题/语言/托盘/后台任务即时生效。
    pub(super) fn reload_settings_snapshot(&mut self, cx: &mut Context<'_, Self>) {
        let runtime = self.settings.runtime();
        let settings = crate::app_shell::config::Settings::from_runtime(&runtime);
        let language = settings.language;
        let tray = settings.tray;
        self.apply_settings_to_terminal_views(&settings, cx);
        cx.set_global(settings);
        gpui_component::set_locale(language.gpui_component_locale());
        crate::notify::set_language(language);
        self.windows.set_tray_enabled(tray);
        crate::background_tasks::apply_settings(self.settings.values());
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.refresh_windows();
    }

    /// 设置快照提交后向全部既有终端扇出一次，隐藏项目同样更新；
    /// 新建终端直接读取同一全局快照。
    fn apply_settings_to_terminal_views(
        &self,
        settings: &crate::app_shell::config::Settings,
        cx: &mut Context<'_, Self>,
    ) {
        let terminals = self
            .projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| panel.terminal().cloned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for terminal in terminals {
            terminal.update(cx, |view, cx| view.apply_settings(settings, cx));
        }
    }

    /// 设置去抖到期后交给后台写盘；前台只提交快照与结算回执。
    pub(super) fn poll_settings_save(&mut self, cx: &mut Context<'_, Self>) {
        let Some(entries) = self.settings.begin_save(Instant::now()) else {
            return;
        };
        let cancellation = super::task_service::CancellationToken::new();
        let written = entries.clone();
        let task = cx.spawn(async move |this, cx| {
            let to_write = written.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let pairs: Vec<(&str, Value)> = to_write
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.clone()))
                        .collect();
                    crate::settings_store::save_settings_blocking(&pairs)
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.settings.finish_save(&written, result, Instant::now());
                root.tasks
                    .complete(super::task_service::TaskId::SettingsSave);
                cx.notify();
            });
        });
        if let Err(error) = self.tasks.register(
            super::task_service::TaskId::SettingsSave,
            cancellation,
            task,
        ) {
            // 注册失败时恢复在途标记，下一节拍重试同一批快照。
            self.settings.abort_save();
            log::warn!("设置保存任务注册失败: {error:?}");
        }
    }

    /// 恢复或新建的项目设置面板若停在 AgentHooks 页，打开即探测安装状态。
    pub(super) fn request_agent_hooks_if_selected(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<'_, Self>,
    ) {
        let view = self.projects.get(&project_id).and_then(|project| {
            project
                .read(cx)
                .panels
                .values()
                .find_map(|panel| match &panel.content {
                    PanelContent::Settings(view) => Some(view.clone()),
                    _ => None,
                })
        });
        let Some(view) = view else {
            return;
        };
        if view.read(cx).current_page() == SettingsPageId::AgentHooks {
            self.request_agent_hooks_inspect(cx);
        }
    }

    pub(super) fn render_settings_overlay(&self, cx: &App) -> Option<AnyElement> {
        let host = self.settings_overlay.as_ref()?;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().background.opacity(0.85))
                .debug_selector(|| "app-settings-overlay-backdrop".to_owned())
                .child(
                    div()
                        .w(relative(0.8))
                        .h(relative(0.8))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .p_4()
                        .debug_selector(|| "app-settings-overlay".to_owned())
                        .child(host.view.clone()),
                )
                .into_any_element(),
        )
    }
}

/// Windows 实际状态说明 → typed 文案；只有「指向其他程序」携带路径参数。
fn integration_note_message(
    note: &crate::platform::windows_integration::IntegrationNote,
) -> (crate::i18n::Message, Option<String>) {
    use crate::i18n::Message;
    use crate::platform::windows_integration::IntegrationNote;
    match note {
        IntegrationNote::LoginStartMissing => (Message::SettingsAdvancedLoginStartMissing, None),
        IntegrationNote::LoginStartOtherTarget(target) => (
            Message::SettingsAdvancedLoginStartOtherTarget,
            Some(target.to_string_lossy().into_owned()),
        ),
        IntegrationNote::LoginStartUnreadable => {
            (Message::SettingsAdvancedLoginStartUnreadable, None)
        }
        IntegrationNote::LoginStartEnabledButOff => {
            (Message::SettingsAdvancedLoginStartEnabledButOff, None)
        }
        IntegrationNote::TrayUnavailable => (Message::SettingsAdvancedTrayUnavailable, None),
        IntegrationNote::BlurFallback => (Message::SettingsAppearanceBlurFallback, None),
        IntegrationNote::BlurUnsupported => (Message::SettingsAppearanceBlurUnsupported, None),
    }
}
