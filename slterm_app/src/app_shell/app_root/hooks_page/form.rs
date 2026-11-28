//! Hooks 页 GUI 表单：事件树、matcher 组、handler 字段与待新增 handler。
//!
//! 表单只把合法模型写回同一 JSON 草稿（经严格解析与 schema 诊断），
//! 字段声明与保存校验共用 `slterm_hooks` 的单一目录；非法输入只留行内诊断，
//! 不改写草稿，也不另建文件写通道。

use std::collections::BTreeMap;

#[cfg(all(test, feature = "gpui-test-support"))]
use gpui::Focusable as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, SharedString, Styled as _, Subscription, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};
use serde_json::{Map, Value};
use slterm_hooks::{HookEvent, HookHandlerType};

use super::super::AppRoot;
use super::super::settings_overlay::SettingsView;
use crate::ai_hook::draft_host::HooksDraftKey;
use crate::ai_hook::editor::HooksEditError;
use crate::ai_hook::editor::{
    HandlerField, apply_field_flag, apply_field_text, field_message, handler_type_message,
    load_field_text, load_flag, missing_required_field,
};
use crate::i18n::{Message, UiLanguage};

/// 表单中的稳定输入身份；只用于 UI 实体缓存，不进入草稿或持久状态。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FormInputKey {
    Matcher {
        event: String,
        group: usize,
    },
    Field {
        event: String,
        group: usize,
        handler: usize,
        field: HandlerField,
    },
    Pending {
        field: HandlerField,
    },
}

impl FormInputKey {
    fn selector(&self) -> String {
        match self {
            Self::Matcher { event, group } => {
                format!("settings-hooks-matcher-{event}-{group}")
            }
            Self::Field {
                event,
                group,
                handler,
                field,
            } => format!(
                "settings-hooks-field-{event}-{group}-{handler}-{}",
                field.key()
            ),
            Self::Pending { field } => {
                format!("settings-hooks-pending-field-{}", field.key())
            }
        }
    }
}

/// 待新增 handler：先在前台表单中收集字段，完整后才写入模型。
#[derive(Clone, Debug)]
struct PendingHandler {
    event: String,
    group: usize,
    kind: HookHandlerType,
    handler: Map<String, Value>,
}

/// GUI 表单的瞬态状态：输入实体、行内诊断、待新增 handler 与同步代次。
#[derive(Default)]
pub(in crate::app_shell::app_root) struct HooksFormState {
    inputs: BTreeMap<FormInputKey, Entity<InputState>>,
    subscriptions: BTreeMap<FormInputKey, Subscription>,
    errors: BTreeMap<FormInputKey, String>,
    pending: Option<PendingHandler>,
    pending_error: Option<String>,
    synced_key: Option<HooksDraftKey>,
    synced_generation: u64,
}

impl HooksFormState {
    /// 切层／关闭等上下文变化：释放全部输入与诊断。
    pub(super) fn reset(&mut self) {
        self.inputs.clear();
        self.subscriptions.clear();
        self.errors.clear();
        self.pending = None;
        self.pending_error = None;
        self.synced_key = None;
        self.synced_generation = 0;
    }

    /// 模式切换：只要求下一次渲染重新同步，不销毁输入。
    pub(in crate::app_shell::app_root) fn reset_sync(&mut self) {
        self.synced_key = None;
        self.synced_generation = 0;
    }

    fn needs_sync(&self, key: HooksDraftKey, generation: u64) -> bool {
        self.synced_key != Some(key) || self.synced_generation != generation
    }

    fn mark_synced(&mut self, key: HooksDraftKey, generation: u64) {
        self.synced_key = Some(key);
        self.synced_generation = generation;
    }

    /// 结构性变更后丢弃输入实体与诊断；下次渲染从模型重建。
    fn reset_inputs(&mut self) {
        self.inputs.clear();
        self.subscriptions.clear();
        self.errors.clear();
        self.pending_error = None;
    }

    fn clear_pending(&mut self) {
        self.pending = None;
        self.pending_error = None;
        let pending_keys = self
            .inputs
            .keys()
            .filter(|key| matches!(key, FormInputKey::Pending { .. }))
            .cloned()
            .collect::<Vec<_>>();
        for key in pending_keys {
            self.inputs.remove(&key);
            self.subscriptions.remove(&key);
            self.errors.remove(&key);
        }
    }

    fn begin_pending(&mut self, event: String, group: usize, kind: HookHandlerType) {
        self.clear_pending();
        let mut handler = Map::new();
        handler.insert("type".to_owned(), Value::String(kind.as_str().to_owned()));
        self.pending = Some(PendingHandler {
            event,
            group,
            kind,
            handler,
        });
    }

    fn input_error(&self, key: &FormInputKey) -> Option<&str> {
        self.errors.get(key).map(String::as_str)
    }
}

/// 表单结构在编辑期间失配（外部形态变化等）的封闭文案。
fn form_internal_error(language: UiLanguage) -> String {
    language
        .text(Message::SettingsAgentsHooksFormDisabled)
        .to_owned()
}

fn ordered_events(model: &Value) -> Vec<String> {
    let Some(events) = model.as_object() else {
        return Vec::new();
    };
    let mut names = events.keys().cloned().collect::<Vec<_>>();
    names.sort_by_key(|name| {
        HookEvent::parse(name)
            .and_then(|event| HookEvent::ALL.iter().position(|known| *known == event))
            .unwrap_or(usize::MAX)
    });
    names
}

fn unconfigured_events(model: &Value) -> Vec<HookEvent> {
    let configured = model.as_object();
    HookEvent::ALL
        .into_iter()
        .filter(|event| configured.is_none_or(|events| !events.contains_key(event.as_str())))
        .collect()
}

fn event_groups_mut<'a>(model: &'a mut Value, event: &str) -> Result<&'a mut Vec<Value>, String> {
    model
        .get_mut(event)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "event groups missing".to_owned())
}

fn group_object_mut(groups: &mut [Value], group: usize) -> Result<&mut Map<String, Value>, String> {
    groups
        .get_mut(group)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "matcher group missing".to_owned())
}

fn handler_object_mut(
    group: &mut Map<String, Value>,
    handler: usize,
) -> Result<&mut Map<String, Value>, String> {
    group
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .and_then(|handlers| handlers.get_mut(handler))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "handler missing".to_owned())
}

/// 表单样式按钮；点击行为由调用方附加。
fn action_button(id: String, label: SharedString, color: Hsla) -> gpui::Stateful<gpui::Div> {
    let selector = id.clone();
    div()
        .id(SharedString::from(id))
        .px_2()
        .h(px(20.0))
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(color)
        .text_xs()
        .cursor_pointer()
        .debug_selector(move || selector)
        .child(label)
}

#[allow(clippy::too_many_arguments)]
fn ensure_input(
    view: &mut SettingsView,
    key: FormInputKey,
    initial: &str,
    sync: bool,
    placeholder: Option<Message>,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> Entity<InputState> {
    if let Some(existing) = view.hooks_form_mut().inputs.get(&key) {
        let input = existing.clone();
        if sync {
            let value = initial.to_owned();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        return input;
    }
    let value = initial.to_owned();
    let placeholder = placeholder.map(|message| language.text(message).to_owned());
    let input = cx.new(|cx| {
        let mut state = InputState::new(window, cx);
        if let Some(placeholder) = placeholder.clone() {
            state = state.placeholder(placeholder);
        }
        state.set_value(value.clone(), window, cx);
        state
    });
    let subscription_key = key.clone();
    let subscription = cx.subscribe_in(&input, window, move |view, input, event, _window, cx| {
        if matches!(event, InputEvent::Change) {
            view.on_form_input_change(subscription_key.clone(), input.clone(), cx);
        }
    });
    let form = view.hooks_form_mut();
    form.inputs.insert(key.clone(), input.clone());
    form.subscriptions.insert(key, subscription);
    input
}

/// 渲染 GUI 模式；`model` 为最后合法模型（无草稿时取已重载文档）。
pub(super) fn render_hooks_form(
    view: &mut SettingsView,
    model: Option<&Value>,
    key: Option<HooksDraftKey>,
    generation: u64,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let sync = key.is_some_and(|key| view.hooks_form_mut().needs_sync(key, generation));
    if sync {
        // 模式切换／外部重载后整表从模型重建，顺带清掉已不存在的 handler 输入与旧诊断。
        view.hooks_form_mut().reset_inputs();
    }
    let mut children: Vec<AnyElement> = Vec::new();
    let Some(model) = model else {
        return div()
            .flex()
            .flex_col()
            .gap_1()
            .debug_selector(|| "settings-hooks-form".to_owned())
            .child(language.text(Message::SettingsAgentsHooksDocumentNotLoaded))
            .into_any_element();
    };

    let events = ordered_events(model);
    if events.is_empty() {
        children.push(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentsFormEmpty))
                .into_any_element(),
        );
    }
    let unconfigured = unconfigured_events(model);
    let add_event_children = unconfigured
        .into_iter()
        .map(|event| {
            let name = event.as_str().to_owned();
            let selector = format!("settings-hooks-add-event-{name}");
            let event_for_click = name.clone();
            action_button(selector, SharedString::from(name), cx.theme().border)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.add_form_event(event_for_click.clone(), cx)
                    }),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    children.push(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pt_2()
            .debug_selector(|| "settings-hooks-add-events".to_owned())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAgentsHooksAddEvent)),
            )
            .when(add_event_children.is_empty(), |element| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentsHooksAllEventsConfigured)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(add_event_children),
            )
            .into_any_element(),
    );
    // 事件添加入口固定在事件树之前，长表单下仍可见。
    for event in &events {
        children.push(render_event_node(
            view, model, event, sync, language, window, cx,
        ));
    }

    if let Some(key) = key {
        view.hooks_form_mut().mark_synced(key, generation);
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .debug_selector(|| "settings-hooks-form".to_owned())
        .children(children)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_event_node(
    view: &mut SettingsView,
    model: &Value,
    event: &str,
    sync: bool,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let groups = model
        .get(event)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let supports_matcher = HookEvent::parse(event).is_some_and(HookEvent::supports_matcher);
    let event_owned = event.to_owned();
    let remove_selector = format!("settings-hooks-remove-event-{event}");
    let event_for_remove = event_owned.clone();
    let header = div()
        .flex()
        .justify_between()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(SharedString::from(event.to_owned())),
        )
        .child(
            action_button(
                remove_selector,
                language
                    .text(Message::SettingsAgentsHooksRemoveEvent)
                    .into(),
                cx.theme().danger,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| {
                    view.remove_form_event(event_for_remove.clone(), cx)
                }),
            ),
        );
    let mut node = div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .debug_selector({
            let selector = format!("settings-hooks-event-{event}");
            move || selector.clone()
        })
        .child(header);
    for (group_index, group) in groups.iter().enumerate() {
        node = node.child(render_group_node(
            view,
            group,
            event,
            group_index,
            supports_matcher,
            sync,
            language,
            window,
            cx,
        ));
    }
    let event_for_group = event_owned.clone();
    node = node.child(
        action_button(
            format!("settings-hooks-add-group-{event}"),
            language.text(Message::SettingsAgentsFormAddGroup).into(),
            cx.theme().border,
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| view.add_form_group(event_for_group.clone(), cx)),
        ),
    );
    node.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_group_node(
    view: &mut SettingsView,
    group: &Value,
    event: &str,
    group_index: usize,
    supports_matcher: bool,
    sync: bool,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let mut children: Vec<AnyElement> = Vec::new();
    let group_object = group.as_object();
    if supports_matcher {
        let initial = group_object
            .and_then(|group| group.get("matcher"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let input_key = FormInputKey::Matcher {
            event: event.to_owned(),
            group: group_index,
        };
        let selector = input_key.selector();
        let input = ensure_input(
            view,
            input_key,
            &initial,
            sync,
            Some(Message::SettingsAgentsFormMatcher),
            language,
            window,
            cx,
        );
        children.push(
            div()
                .flex()
                .items_center()
                .gap_2()
                .debug_selector({
                    let selector = selector.clone();
                    move || selector.clone()
                })
                .child(
                    div()
                        .text_xs()
                        .w(px(120.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentsFormMatcher)),
                )
                .child(
                    div()
                        .w(px(220.0))
                        .debug_selector({
                            let selector = format!("{selector}-input");
                            move || selector.clone()
                        })
                        .child(Input::new(&input)),
                )
                .into_any_element(),
        );
    } else {
        children.push(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .debug_selector({
                    let selector = format!("settings-hooks-all-matches-{event}-{group_index}");
                    move || selector.clone()
                })
                .child(language.text(Message::SettingsAgentsHooksAllMatches))
                .into_any_element(),
        );
    }

    let handlers = group_object
        .and_then(|group| group.get("hooks"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (handler_index, handler) in handlers.iter().enumerate() {
        children.push(render_handler_node(
            view,
            handler,
            event,
            group_index,
            handler_index,
            sync,
            language,
            window,
            cx,
        ));
    }

    let add_handler_buttons = HookHandlerType::ALL
        .into_iter()
        .map(|kind| {
            let event = event.to_owned();
            let selector = format!(
                "settings-hooks-add-handler-{event}-{group_index}-{}",
                kind.as_str()
            );
            action_button(
                selector,
                language.text(handler_type_message(kind)).into(),
                cx.theme().border,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| {
                    view.begin_pending_handler(event.clone(), group_index, kind, cx)
                }),
            )
        })
        .collect::<Vec<_>>();
    children.push(
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsAgentsFormAddHandler)),
            )
            .children(add_handler_buttons)
            .into_any_element(),
    );

    let pending_matches = view
        .hooks_form_mut()
        .pending
        .as_ref()
        .is_some_and(|pending| pending.event == event && pending.group == group_index);
    if pending_matches {
        children.push(render_pending_handler(
            view,
            event,
            group_index,
            language,
            window,
            cx,
        ));
    }

    let event_for_remove = event.to_owned();
    children.push(
        action_button(
            format!("settings-hooks-remove-group-{event}-{group_index}"),
            language.text(Message::SettingsAgentsFormRemoveGroup).into(),
            cx.theme().danger,
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| {
                view.remove_form_group(event_for_remove.clone(), group_index, cx)
            }),
        )
        .into_any_element(),
    );

    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border.opacity(0.7))
        .debug_selector({
            let selector = format!("settings-hooks-group-{event}-{group_index}");
            move || selector.clone()
        })
        .children(children)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_handler_node(
    view: &mut SettingsView,
    handler: &Value,
    event: &str,
    group_index: usize,
    handler_index: usize,
    sync: bool,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let kind = handler
        .get("type")
        .and_then(Value::as_str)
        .and_then(HookHandlerType::parse);
    let Some(kind) = kind else {
        return div()
            .text_xs()
            .text_color(cx.theme().danger)
            .child(
                language.format(
                    Message::SettingsAgentsFormUnsupportedType,
                    &[(
                        "kind",
                        handler
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )],
                ),
            )
            .into_any_element();
    };
    let handler_object = handler.as_object();
    let event_for_remove = event.to_owned();
    let mut children: Vec<AnyElement> = vec![
        div()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .child(language.text(handler_type_message(kind))),
            )
            .child(
                action_button(
                    format!("settings-hooks-remove-handler-{event}-{group_index}-{handler_index}"),
                    language
                        .text(Message::SettingsAgentsFormRemoveHandler)
                        .into(),
                    cx.theme().danger,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.remove_form_handler(
                            event_for_remove.clone(),
                            group_index,
                            handler_index,
                            cx,
                        )
                    }),
                ),
            )
            .into_any_element(),
    ];
    let path = format!("{event}[{group_index}].hooks[{handler_index}]");
    for field in kind.fields() {
        children.push(render_handler_field(
            view,
            handler_object,
            event,
            group_index,
            handler_index,
            *field,
            &path,
            sync,
            language,
            window,
            cx,
        ));
    }
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border.opacity(0.7))
        .debug_selector({
            let selector = format!("settings-hooks-handler-{event}-{group_index}-{handler_index}");
            move || selector.clone()
        })
        .children(children)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_handler_field(
    view: &mut SettingsView,
    handler: Option<&Map<String, Value>>,
    event: &str,
    group_index: usize,
    handler_index: usize,
    field: HandlerField,
    path: &str,
    sync: bool,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let empty = Map::new();
    let handler = handler.unwrap_or(&empty);
    let label = language.text(field_message(field));
    let input_key = FormInputKey::Field {
        event: event.to_owned(),
        group: group_index,
        handler: handler_index,
        field,
    };
    let selector = input_key.selector();
    let mut row = div()
        .flex()
        .items_start()
        .gap_2()
        .debug_selector({
            let selector = selector.clone();
            move || selector.clone()
        })
        .child(
            div()
                .text_xs()
                .w(px(180.0))
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(label)),
        );
    let mut error: Option<String> = None;
    if field.kind() == crate::ai_hook::editor::FieldKind::Flag {
        let value = match load_flag(handler, field, language, path) {
            Ok(value) => value,
            Err(detail) => {
                error = Some(detail);
                false
            }
        };
        let event_for_toggle = event.to_owned();
        let toggle = div()
            .id(SharedString::from(selector.clone()))
            .w(px(36.0))
            .h(px(20.0))
            .rounded_full()
            .border_1()
            .border_color(if value {
                cx.theme().accent
            } else {
                cx.theme().border
            })
            .bg(if value {
                cx.theme().accent.opacity(0.35)
            } else {
                cx.theme().transparent
            })
            .cursor_pointer()
            .debug_selector({
                let selector = selector.clone();
                move || selector.clone()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| {
                    view.toggle_form_flag(
                        event_for_toggle.clone(),
                        group_index,
                        handler_index,
                        field,
                        !value,
                        cx,
                    )
                }),
            )
            .child(
                div()
                    .size(px(14.0))
                    .m(px(3.0))
                    .rounded_full()
                    .when(value, |dot| dot.ml(px(19.0)))
                    .bg(if value {
                        cx.theme().accent
                    } else {
                        cx.theme().muted_foreground
                    }),
            );
        row = row.child(toggle);
    } else {
        let (initial, load_error) = match load_field_text(handler, field, language, path) {
            Ok(text) => (text, None),
            Err(detail) => (String::new(), Some(detail)),
        };
        error = load_error;
        if let Some(detail) = view.hooks_form_mut().input_error(&input_key) {
            error = Some(detail.to_owned());
        }
        let input = ensure_input(view, input_key, &initial, sync, None, language, window, cx);
        row = row.child(
            div()
                .w(px(300.0))
                .debug_selector({
                    let selector = format!("{selector}-input");
                    move || selector.clone()
                })
                .child(Input::new(&input)),
        );
    }
    if let Some(error) = error {
        row = row.child(
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .debug_selector({
                    let selector = format!("{selector}-error");
                    move || selector.clone()
                })
                .child(SharedString::from(error)),
        );
    }
    row.into_any_element()
}

fn render_pending_handler(
    view: &mut SettingsView,
    event: &str,
    group_index: usize,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let Some(pending) = view.hooks_form_mut().pending.clone() else {
        return div().into_any_element();
    };
    let mut children: Vec<AnyElement> = vec![
        div()
            .text_xs()
            .text_color(cx.theme().foreground)
            .child(language.text(Message::SettingsAgentsHooksPendingTitle))
            .child(SharedString::from(" · "))
            .child(language.text(handler_type_message(pending.kind)))
            .into_any_element(),
    ];
    for field in pending.kind.fields() {
        let label = language.text(field_message(*field));
        let input_key = FormInputKey::Pending { field: *field };
        if field.kind() == crate::ai_hook::editor::FieldKind::Flag {
            let value = pending
                .handler
                .get(field.key())
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let field = *field;
            let selector = format!("settings-hooks-pending-toggle-{}", field.key());
            children.push(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .w(px(180.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(label)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(selector.clone()))
                            .w(px(36.0))
                            .h(px(20.0))
                            .rounded_full()
                            .border_1()
                            .border_color(if value {
                                cx.theme().accent
                            } else {
                                cx.theme().border
                            })
                            .bg(if value {
                                cx.theme().accent.opacity(0.35)
                            } else {
                                cx.theme().transparent
                            })
                            .cursor_pointer()
                            .debug_selector(move || selector.clone())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    view.toggle_pending_flag(field, cx)
                                }),
                            )
                            .child(
                                div()
                                    .size(px(14.0))
                                    .m(px(3.0))
                                    .rounded_full()
                                    .when(value, |dot| dot.ml(px(19.0)))
                                    .bg(if value {
                                        cx.theme().accent
                                    } else {
                                        cx.theme().muted_foreground
                                    }),
                            ),
                    )
                    .into_any_element(),
            );
            continue;
        }
        let error = view
            .hooks_form_mut()
            .input_error(&input_key)
            .map(str::to_owned);
        let initial = match pending.handler.get(field.key()) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            Some(value) => serde_json::to_string(value).unwrap_or_default(),
            None => String::new(),
        };
        let input = ensure_input(view, input_key, &initial, false, None, language, window, cx);
        let mut row = div()
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .w(px(180.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(label)),
            )
            .child(
                div()
                    .w(px(300.0))
                    .debug_selector({
                        let selector =
                            format!("settings-hooks-pending-field-{}-input", field.key());
                        move || selector.clone()
                    })
                    .child(Input::new(&input)),
            );
        if let Some(error) = error {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector({
                        let selector =
                            format!("settings-hooks-pending-field-{}-error", field.key());
                        move || selector.clone()
                    })
                    .child(SharedString::from(error)),
            );
        }
        children.push(row.into_any_element());
    }
    if let Some(error) = view.hooks_form_mut().pending_error.clone() {
        children.push(
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .debug_selector(|| "settings-hooks-pending-error".to_owned())
                .child(SharedString::from(error))
                .into_any_element(),
        );
    }
    children.push(
        div()
            .flex()
            .gap_2()
            .child(
                action_button(
                    "settings-hooks-pending-add".to_owned(),
                    language.text(Message::SettingsAgentsHooksPendingAdd).into(),
                    cx.theme().accent,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| view.commit_pending_handler(cx)),
                ),
            )
            .child(
                action_button(
                    "settings-hooks-pending-cancel".to_owned(),
                    language
                        .text(Message::SettingsAgentsHooksPendingCancel)
                        .into(),
                    cx.theme().border,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| view.cancel_pending_handler(cx)),
                ),
            )
            .into_any_element(),
    );
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().accent)
        .debug_selector({
            let selector = format!("settings-hooks-pending-{event}-{group_index}");
            move || selector.clone()
        })
        .children(children)
        .into_any_element()
}

impl SettingsView {
    fn hooks_form_key(&self) -> Option<HooksDraftKey> {
        self.hooks_key()
    }

    /// 测试把真实键盘焦点交给指定表单输入；生产路径由鼠标命中或键盘导航送焦。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(in crate::app_shell::app_root) fn focus_form_input_for_test(
        &self,
        selector: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let input = self
            .hooks_form_ref()
            .inputs
            .iter()
            .find(|(key, _)| key.selector() == selector)
            .map(|(_, input)| input.clone());
        let Some(input) = input else {
            return false;
        };
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        true
    }

    /// 表单输入写回同一草稿；非法值只留行内诊断，不写模型。
    fn on_form_input_change(
        &mut self,
        key: FormInputKey,
        input: Entity<InputState>,
        cx: &mut Context<'_, Self>,
    ) {
        let text = input.read(cx).value().to_string();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let Some(draft_key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        match key.clone() {
            FormInputKey::Matcher { event, group } => {
                let result = root.update(cx, |root, cx| {
                    root.mutate_hooks_draft_model(
                        draft_key,
                        |model| {
                            let groups = event_groups_mut(model, &event)
                                .map_err(|_| form_internal_error(language))?;
                            let group = group_object_mut(groups, group)
                                .map_err(|_| form_internal_error(language))?;
                            if text.trim().is_empty() {
                                group.remove("matcher");
                            } else {
                                group.insert("matcher".to_owned(), Value::String(text));
                            }
                            Ok(())
                        },
                        cx,
                    )
                });
                self.record_form_result(&key, result, cx);
            }
            FormInputKey::Field {
                event,
                group,
                handler,
                field,
            } => {
                let result = root.update(cx, |root, cx| {
                    root.mutate_hooks_draft_model(
                        draft_key,
                        |model| {
                            let groups = event_groups_mut(model, &event)
                                .map_err(|_| form_internal_error(language))?;
                            let group = group_object_mut(groups, group)
                                .map_err(|_| form_internal_error(language))?;
                            let handler_object = handler_object_mut(group, handler)
                                .map_err(|_| form_internal_error(language))?;
                            let kind = handler_object
                                .get("type")
                                .and_then(Value::as_str)
                                .and_then(HookHandlerType::parse)
                                .ok_or_else(|| form_internal_error(language))?;
                            apply_field_text(handler_object, field, &text, kind, language)
                        },
                        cx,
                    )
                });
                self.record_form_result(&key, result, cx);
            }
            FormInputKey::Pending { field } => {
                let Some(pending) = self.hooks_form_mut().pending.as_mut() else {
                    return;
                };
                let result =
                    apply_field_text(&mut pending.handler, field, &text, pending.kind, language);
                let form = self.hooks_form_mut();
                match result {
                    Ok(()) => {
                        form.errors.remove(&key);
                        form.pending_error = None;
                    }
                    Err(error) => {
                        form.errors.insert(key, error);
                    }
                }
                cx.notify();
            }
        }
    }

    fn record_form_result(
        &mut self,
        key: &FormInputKey,
        result: Result<(), String>,
        cx: &mut Context<'_, Self>,
    ) {
        let form = self.hooks_form_mut();
        match result {
            Ok(()) => {
                form.errors.remove(key);
            }
            Err(error) => {
                form.errors.insert(key.clone(), error);
            }
        }
        cx.notify();
    }

    pub(super) fn add_form_event(&mut self, event: String, cx: &mut Context<'_, Self>) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let Some(events) = model.as_object_mut() else {
                        return Err("hooks root is not an object".to_owned());
                    };
                    events
                        .entry(event.clone())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }

    pub(super) fn add_form_group(&mut self, event: String, cx: &mut Context<'_, Self>) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let groups = event_groups_mut(model, &event)?;
                    groups.push(serde_json::json!({"hooks": []}));
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }

    pub(super) fn remove_form_event(&mut self, event: String, cx: &mut Context<'_, Self>) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let Some(events) = model.as_object_mut() else {
                        return Err("hooks root is not an object".to_owned());
                    };
                    events.remove(&event);
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }

    pub(super) fn remove_form_group(
        &mut self,
        event: String,
        group: usize,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let groups = event_groups_mut(model, &event)?;
                    if group < groups.len() {
                        groups.remove(group);
                    }
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }

    pub(super) fn remove_form_handler(
        &mut self,
        event: String,
        group: usize,
        handler: usize,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let groups = event_groups_mut(model, &event)?;
                    let group_object = group_object_mut(groups, group)?;
                    if let Some(handlers) =
                        group_object.get_mut("hooks").and_then(Value::as_array_mut)
                        && handler < handlers.len()
                    {
                        handlers.remove(handler);
                    }
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }

    pub(super) fn toggle_form_flag(
        &mut self,
        event: String,
        group: usize,
        handler: usize,
        field: HandlerField,
        value: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let groups = event_groups_mut(model, &event)?;
                    let group_object = group_object_mut(groups, group)?;
                    let handler_object = handler_object_mut(group_object, handler)?;
                    apply_field_flag(handler_object, field, value);
                    Ok(())
                },
                cx,
            )
        });
        let _ = result;
        cx.notify();
    }

    pub(super) fn begin_pending_handler(
        &mut self,
        event: String,
        group: usize,
        kind: HookHandlerType,
        cx: &mut Context<'_, Self>,
    ) {
        self.hooks_form_mut().begin_pending(event, group, kind);
        cx.notify();
    }

    pub(super) fn cancel_pending_handler(&mut self, cx: &mut Context<'_, Self>) {
        self.hooks_form_mut().clear_pending();
        cx.notify();
    }

    pub(super) fn toggle_pending_flag(&mut self, field: HandlerField, cx: &mut Context<'_, Self>) {
        let Some(pending) = self.hooks_form_mut().pending.as_mut() else {
            return;
        };
        let value = !pending
            .handler
            .get(field.key())
            .and_then(Value::as_bool)
            .unwrap_or(false);
        apply_field_flag(&mut pending.handler, field, value);
        cx.notify();
    }

    pub(super) fn commit_pending_handler(&mut self, cx: &mut Context<'_, Self>) {
        let Some(pending) = self.hooks_form_mut().pending.clone() else {
            return;
        };
        if self
            .hooks_form_mut()
            .errors
            .keys()
            .any(|key| matches!(key, FormInputKey::Pending { .. }))
        {
            // 行内非法字段尚未修正：保持待新增表单，不静默丢弃用户输入。
            cx.notify();
            return;
        }
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        if let Some(missing) = missing_required_field(pending.kind, &pending.handler) {
            let error = language.format(
                Message::SettingsAgentsHooksFormRequired,
                &[("field", language.text(field_message(missing)))],
            );
            self.hooks_form_mut().pending_error = Some(error);
            cx.notify();
            return;
        }
        let Some(key) = self.hooks_form_key() else {
            return;
        };
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let result = root.update(cx, |root, cx| {
            root.mutate_hooks_draft_model(
                key,
                |model| {
                    let groups = event_groups_mut(model, &pending.event)?;
                    let group_object = group_object_mut(groups, pending.group)?;
                    let Some(handlers) =
                        group_object.get_mut("hooks").and_then(Value::as_array_mut)
                    else {
                        return Err("matcher group lacks hooks".to_owned());
                    };
                    handlers.push(Value::Object(pending.handler.clone()));
                    Ok(())
                },
                cx,
            )
        });
        if result.is_ok() {
            let form = self.hooks_form_mut();
            form.clear_pending();
            form.reset_inputs();
        }
        cx.notify();
    }
}

impl AppRoot {
    /// 在最后合法模型上改写并写回同一 JSON 草稿；严格解析与 schema 诊断随
    /// `update_hooks_draft_text` / `set_text` 重新计算。
    pub(super) fn mutate_hooks_draft_model(
        &mut self,
        key: HooksDraftKey,
        mutate: impl FnOnce(&mut Value) -> Result<(), String>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        // 没有草稿也没有已载入文档（如项目根缺失）时拒绝表单写入：
        // 不得凭空创建会参与项目生命周期守卫的草稿。
        let mut model = self
            .hook_drafts
            .get(key)
            .map(|draft| draft.model().clone())
            .or_else(|| {
                self.hooks_editor
                    .documents
                    .get(&key)
                    .map(|document| document.editable_value().clone())
            })
            .ok_or_else(|| {
                let language = cx.global::<crate::app_shell::config::Settings>().language;
                language
                    .text(Message::SettingsAgentsHooksFormDisabled)
                    .to_owned()
            })?;
        mutate(&mut model)?;
        let text = serde_json::to_string_pretty(&model).map_err(|error| {
            let language = cx.global::<crate::app_shell::config::Settings>().language;
            super::hooks_edit_error_text(language, &HooksEditError::Serialize(error.to_string()))
        })?;
        self.update_hooks_draft_text(key, &text, cx);
        Ok(())
    }
}
