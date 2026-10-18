//! 设置中心 Agent 组 hooks 页(06 篇轨二 + 03 篇安装权威联调)。
//!
//! 两条轨道在这里汇合而不交叉:
//! - 轨一(接入偏好):每 agent 的 `AgentHook` 键经设置页统一写通道落盘;
//!   安装/卸载由 03 篇安装权威执行(设置页不直接改 provider 文件)。
//! - 轨二(配置编辑):数据源 = `integrations::inspect()` 的 installed/
//!   needs_repair/helper_missing 三态与 config_path;编辑器只编辑用户/第三方
//!   handler,slterm marker 认领组只读展示,保存时由 `HooksDocument` 原样保留。
//!   编辑面默认结构化表单(五型字段),原始 JSON 保留为高级模式;两者共用
//!   同一 `HooksDocument::save_editable` 保存通道。
//!
//! 文件 I/O(inspect / 文档加载 / 保存)与既有 settings 写通道一样在本页
//! 同步完成;inspect 放后台执行器,完成回前台刷新。测试注入 tempdir 假 home,
//! 绝不触碰真实 `%USERPROFILE%`。

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, ParentElement as _,
    SharedString, Styled as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, Disableable as _, IndexPath, h_flex, v_flex};
use serde_json::{Map, Value, json};
use slterm_settings::AgentHook;

use crate::ai_hook::editor::{
    COMMON_FIELDS, HandlerField, HookHandlerType, HooksDocument, JsonShape, field_applies,
    form_load_error, handler_type_message, hook_events, insert_json_field, insert_string_field,
    invalid_field, load_field_text, load_flag, type_fields,
};
use crate::ai_hook::integrations::{self, AgentIntegration};
use crate::i18n::Message;

use super::SettingsPane;
use super::design::SETTINGS_GROUP_GAP;
use super::localization::UiLanguage;

/// hooks 页数据源与安装权威的注入端。生产恒 `Default`;测试注入 tempdir
/// 假 home(编辑域与三态联调只锚 claude)。
#[derive(Clone, Debug)]
pub(super) enum HooksTarget {
    Default,
    #[allow(dead_code)] // 生产不可达:消费点 = gpui-test-support 联调用例。
    Claude {
        config_dir: PathBuf,
        helper: Option<String>,
        executable: Option<PathBuf>,
    },
}

impl HooksTarget {
    fn inspect(&self) -> Vec<AgentIntegration> {
        match self {
            Self::Default => integrations::inspect(),
            Self::Claude {
                config_dir,
                helper,
                executable,
            } => integrations::inspect_with(Some(integrations::ClaudeProbe {
                config_dir: config_dir.as_path(),
                helper: helper.as_deref(),
                executable: executable.as_deref(),
            })),
        }
    }

    /// 安装权威应用入口(偏好已由设置页写通道落盘)。
    /// C5:confirmed=true 时 SEC-12 审查门恒放行(用户已确认原文)。
    pub(super) fn apply_with_confirmation(
        &self,
        agent: AgentHook,
        enabled: bool,
        confirmed: bool,
    ) -> Result<(), String> {
        match self {
            Self::Default => {
                if confirmed {
                    integrations::apply_enabled_confirmed(agent, enabled)
                } else {
                    integrations::apply_enabled(agent, enabled)
                }
            }
            Self::Claude {
                config_dir, helper, ..
            } => {
                if agent != AgentHook::Claude {
                    return Err("injected hooks target only supports Claude".to_owned());
                }
                let helper = helper
                    .as_deref()
                    .ok_or("The slterm hook helper is missing from this installation.")?;
                if confirmed {
                    integrations::apply_claude_confirmed_at(config_dir, helper, enabled)
                } else {
                    integrations::apply_claude_at(config_dir, helper, enabled)
                }
            }
        }
    }

    /// C5:待确认的 statusLine 原文(无命中 → None)。
    pub(super) fn statusline_confirmation(&self, agent: AgentHook) -> Option<String> {
        match self {
            Self::Default => integrations::statusline_confirmation(agent),
            Self::Claude { config_dir, .. } => {
                integrations::claude_statusline_confirmation_at(config_dir)
            }
        }
    }

    fn document(&self) -> Result<HooksDocument, String> {
        match self {
            Self::Default => HooksDocument::load_user(),
            Self::Claude { config_dir, .. } => {
                if !config_dir.is_dir() {
                    return Err("Claude 配置目录不存在;先启动一次 Claude，然后刷新".to_owned());
                }
                HooksDocument::load_at(&config_dir.join("settings.json"))
            }
        }
    }
}

/// 编辑面呈现模式:默认结构化表单;JSON 文本保留为高级模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HooksFormMode {
    Form,
    Json,
}

/// hooks 页状态:inspect 行 + 序列号(迟到回调不复活旧结果)+ 编辑文档。
pub(super) struct AgentSettingsState {
    pub(super) rows: Option<Vec<AgentIntegration>>,
    pub(super) loading: bool,
    pub(super) sequence: u64,
    pub(super) feedback: Option<(AgentHook, Result<bool, String>)>,
    /// C5:SEC-12 命中的 statusLine 原文;有值时行内出现「确认并重试」。
    pub(super) suspicious_statusline: Option<String>,
    pub(super) document: Option<HooksDocument>,
    pub(super) document_error: Option<String>,
    pub(super) hooks_editor: Entity<InputState>,
    pub(super) editor_saved: bool,
    pub(super) editor_error: Option<String>,
    pub(super) form_mode: HooksFormMode,
    /// 结构化表单;文档无法表单化时为 None(`form_error` 说明原因,JSON 模式仍可用)。
    pub(super) form: Option<HookFormState>,
    pub(super) form_error: Option<String>,
    form_subscriptions: Vec<gpui::Subscription>,
}

impl AgentSettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<'_, SettingsPane>) -> Self {
        let hooks_editor = cx.new(|cx| InputState::new(window, cx).multi_line(true).rows(10));
        Self {
            rows: None,
            loading: false,
            sequence: 0,
            feedback: None,
            suspicious_statusline: None,
            document: None,
            document_error: None,
            hooks_editor,
            editor_saved: false,
            editor_error: None,
            form_mode: HooksFormMode::Form,
            form: None,
            form_error: None,
            form_subscriptions: Vec::new(),
        }
    }
}

/// 一条 handler 的表单行:类型下拉 + 全量文本输入槽(按型显隐)+ 开关 +
/// 无法建模的未知键原样保留。
struct HandlerFormRow {
    id: u64,
    kind: HookHandlerType,
    type_select: Entity<SelectState<Vec<SharedString>>>,
    texts: HashMap<HandlerField, Entity<InputState>>,
    asynchronous: bool,
    async_rewake: bool,
    continue_on_block: bool,
    extra: Map<String, Value>,
}

/// 一个 matcher 组的表单行:事件下拉 + matcher 文本 + handler 列表。
struct HookGroupForm {
    event_select: Entity<SelectState<Vec<SharedString>>>,
    matcher_input: Entity<InputState>,
    handlers: Vec<HandlerFormRow>,
    extra: Map<String, Value>,
}

/// 结构化表单整体状态;`next_id` 用于类型下拉订阅的稳定身份。
pub(super) struct HookFormState {
    groups: Vec<HookGroupForm>,
    next_id: u64,
}

impl SettingsPane {
    pub(super) fn refresh_agents(&mut self, cx: &mut Context<'_, Self>) {
        if self.agents.loading {
            return;
        }
        self.agents.loading = true;
        self.agents.sequence = self.agents.sequence.wrapping_add(1);
        let sequence = self.agents.sequence;
        let target = self.hooks_target.clone();
        let task = cx.background_executor().spawn(async move {
            let rows = target.inspect();
            let document = target.document();
            (rows, document)
        });
        cx.spawn(async move |this, cx| {
            let (rows, document) = task.await;
            let _ = this.update_in(cx, |pane, window, cx| {
                if pane.agents.sequence != sequence {
                    return;
                }
                pane.agents.rows = Some(rows);
                pane.agents.loading = false;
                pane.set_editor_document(document, window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_editor_document(
        &mut self,
        document: Result<HooksDocument, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match document {
            Ok(document) => {
                let text = document.editable_json();
                self.agents.document = Some(document);
                self.agents.document_error = None;
                self.agents
                    .hooks_editor
                    .update(cx, |state, cx| state.set_value(text, window, cx));
                self.rebuild_hooks_form(window, cx);
            }
            Err(error) => {
                self.agents.document = None;
                self.agents.document_error = Some(error);
                self.agents.form = None;
                self.agents.form_error = None;
                self.agents.form_subscriptions.clear();
            }
        }
        self.agents.editor_saved = false;
        self.agents.editor_error = None;
    }

    /// 文档 → 表单模型。文档可加载但无法表单化(未知事件/类型/字段形态)
    /// 时只记录 `form_error`,不阻断 JSON 高级模式。
    fn rebuild_hooks_form(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let language = self.language(cx);
        // 旧行实体随表单一起丢弃,其类型下拉订阅同步解除。
        self.agents.form_subscriptions.clear();
        let mut subscriptions = Vec::new();
        let Some(document) = self.agents.document.as_ref() else {
            self.agents.form = None;
            self.agents.form_error = None;
            return;
        };
        let result = build_hook_form(document, language, window, cx, &mut subscriptions);
        self.agents.form_subscriptions = subscriptions;
        match result {
            Ok(form) => {
                self.agents.form = Some(form);
                self.agents.form_error = None;
            }
            Err(error) => {
                self.agents.form = None;
                self.agents.form_error = Some(error);
            }
        }
    }

    /// 切换编辑面呈现模式(表单 / JSON);两种模式共用同一文档与保存通道。
    fn set_hooks_form_mode(&mut self, mode: HooksFormMode, cx: &mut Context<'_, Self>) {
        if self.agents.form_mode == mode {
            return;
        }
        self.agents.form_mode = mode;
        self.agents.editor_saved = false;
        self.agents.editor_error = None;
        cx.notify();
    }

    /// 轨一写偏好(本页统一写通道)+ 轨二交给安装权威应用。
    fn set_agent_enabled(
        &mut self,
        agent: AgentHook,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.agents.loading {
            return;
        }
        let Some(row) = self
            .agents
            .rows
            .as_ref()
            .and_then(|rows| rows.iter().find(|row| row.agent == agent))
        else {
            return;
        };
        if !can_toggle(row) {
            return;
        }
        self.agents.feedback = None;
        // A2:偏好写盘与安装权威应用同在一个后台任务内串行完成;前台先清
        // 反馈,结果回包后刷新行。
        self.enqueue_persist(
            super::persistence::PersistJob::AgentHook {
                hook: agent,
                enabled,
                confirmed: false,
            },
            cx,
        );
        self.status = None;
        cx.notify();
        let _ = window;
    }

    /// C5:用户确认 SEC-12 原文后重试(审查门恒放行)。
    fn set_agent_enabled_confirmed(
        &mut self,
        agent: AgentHook,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.agents.feedback = None;
        self.enqueue_persist(
            super::persistence::PersistJob::AgentHook {
                hook: agent,
                enabled,
                confirmed: true,
            },
            cx,
        );
        self.status = None;
        cx.notify();
        let _ = window;
    }

    /// 轨二提交(JSON 高级模式):编辑文本 → SEC-05 + marker 边界 → 原子写 → 重读。
    fn save_hooks_editor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(document) = self.agents.document.clone() else {
            return;
        };
        let raw = self.agents.hooks_editor.read(cx).value().to_string();
        self.apply_hooks_save(document, &raw, window, cx);
    }

    /// 轨二提交(结构化表单):表单 → JSON → 与 JSON 模式同一保存通道。
    /// UI 侧绝不直接写盘,失败时保持文档与磁盘原样。
    fn save_hooks_form(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(document) = self.agents.document.clone() else {
            return;
        };
        let language = self.language(cx);
        let raw = match self
            .agents
            .form
            .as_ref()
            .map(|form| collect_hook_form(form, language, cx))
        {
            Some(Ok(value)) => value.to_string(),
            Some(Err(error)) => {
                self.agents.editor_saved = false;
                self.agents.editor_error = Some(error);
                cx.notify();
                return;
            }
            None => return,
        };
        self.apply_hooks_save(document, &raw, window, cx);
    }

    /// 保存成功后的统一收敛:重读文档、刷新 JSON 与表单、更新状态。
    fn apply_hooks_save(
        &mut self,
        document: HooksDocument,
        raw: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match document.save_editable(raw) {
            Ok(()) => {
                match document.reload() {
                    Ok(reloaded) => {
                        let text = reloaded.editable_json();
                        self.agents.document = Some(reloaded);
                        self.agents.document_error = None;
                        self.agents
                            .hooks_editor
                            .update(cx, |state, cx| state.set_value(text, window, cx));
                        self.rebuild_hooks_form(window, cx);
                    }
                    Err(error) => {
                        self.agents.document = None;
                        self.agents.document_error = Some(error);
                        self.agents.form = None;
                        self.agents.form_error = None;
                        self.agents.form_subscriptions.clear();
                    }
                }
                self.agents.editor_error = None;
                self.agents.editor_saved = self.agents.document.is_some();
            }
            Err(error) => {
                self.agents.editor_saved = false;
                self.agents.editor_error = Some(error);
            }
        }
        cx.notify();
    }

    /// 类型下拉切换:清空新类型不适配的字段(通用字段保留)。
    fn switch_hooks_handler_kind(
        &mut self,
        id: u64,
        kind: HookHandlerType,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        {
            let Some(form) = self.agents.form.as_mut() else {
                return;
            };
            let Some(row) = form
                .groups
                .iter_mut()
                .flat_map(|group| group.handlers.iter_mut())
                .find(|row| row.id == id)
            else {
                return;
            };
            if row.kind == kind {
                return;
            }
            row.kind = kind;
            let selected = HookHandlerType::ALL
                .iter()
                .position(|candidate| *candidate == kind)
                .unwrap_or(0);
            row.type_select.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::default().row(selected)), window, cx);
            });
            for field in HandlerField::ALL {
                if field_applies(kind, field) {
                    continue;
                }
                if field.is_flag() {
                    match field {
                        HandlerField::Async => row.asynchronous = false,
                        HandlerField::AsyncRewake => row.async_rewake = false,
                        HandlerField::ContinueOnBlock => row.continue_on_block = false,
                        _ => {}
                    }
                } else if let Some(input) = row.texts.get(&field) {
                    input.update(cx, |state, cx| state.set_value(String::new(), window, cx));
                }
            }
        }
        self.agents.editor_saved = false;
        cx.notify();
    }

    /// 新增事件组(默认事件 = 白名单首项,handler 列表为空)。
    fn add_hooks_form_group(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let language = self.language(cx);
        let event = hook_events().first().copied()?;
        let event_select = build_event_select(event, window, cx).ok()?;
        let matcher_input = make_input(
            String::new(),
            language.text(Message::SettingsAgentsFormMatcher),
            window,
            cx,
        );
        let form = self.agents.form.as_mut()?;
        form.groups.push(HookGroupForm {
            event_select,
            matcher_input,
            handlers: Vec::new(),
            extra: Map::new(),
        });
        let index = form.groups.len() - 1;
        self.agents.editor_saved = false;
        cx.notify();
        Some(index)
    }

    /// 新增 handler(默认 command 型);返回新行下标。
    fn add_hooks_form_handler_with_kind(
        &mut self,
        group: usize,
        kind: HookHandlerType,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let language = self.language(cx);
        let next_id = self.agents.form.as_ref()?.next_id;
        let (row, subscription) = build_handler_row(next_id, kind, language, window, cx);
        self.agents.form_subscriptions.push(subscription);
        let index = {
            let form = self.agents.form.as_mut()?;
            form.next_id = next_id + 1;
            let group = form.groups.get_mut(group)?;
            group.handlers.push(row);
            group.handlers.len() - 1
        };
        self.agents.editor_saved = false;
        cx.notify();
        Some(index)
    }

    fn add_hooks_form_handler(
        &mut self,
        group: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.add_hooks_form_handler_with_kind(group, HookHandlerType::Command, window, cx);
    }

    fn remove_hooks_form_group(&mut self, group: usize, cx: &mut Context<'_, Self>) {
        if let Some(form) = self.agents.form.as_mut()
            && group < form.groups.len()
        {
            form.groups.remove(group);
        }
        self.agents.editor_saved = false;
        cx.notify();
    }

    fn remove_hooks_form_handler(
        &mut self,
        group: usize,
        handler: usize,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(form) = self.agents.form.as_mut()
            && let Some(group) = form.groups.get_mut(group)
            && handler < group.handlers.len()
        {
            group.handlers.remove(handler);
        }
        self.agents.editor_saved = false;
        cx.notify();
    }

    /// 布尔开关(异步/异步后唤醒/阻断后继续)只改表单状态,提交时序列化。
    fn set_hooks_form_flag(
        &mut self,
        group: usize,
        handler: usize,
        field: HandlerField,
        value: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(row) = self
            .agents
            .form
            .as_mut()
            .and_then(|form| form.groups.get_mut(group))
            .and_then(|group| group.handlers.get_mut(handler))
        {
            match field {
                HandlerField::Async => row.asynchronous = value,
                HandlerField::AsyncRewake => row.async_rewake = value,
                HandlerField::ContinueOnBlock => row.continue_on_block = value,
                _ => {}
            }
        }
        self.agents.editor_saved = false;
        cx.notify();
    }

    fn agent_hook_row(
        &self,
        index: usize,
        row: &AgentIntegration,
        total: usize,
        cx: &Context<'_, Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let language = self.language(cx);
        let agent = row.agent;
        let state = &row.inspection;
        let checked = state.enabled;
        let disabled = self.agents.loading || !can_toggle(row);
        let status = language.text(agent_status(row));
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let executable = row
            .executable
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| language.text(Message::SettingsAgentsNotDetected).to_owned());
        let config = state
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let note = self
            .agents
            .feedback
            .as_ref()
            .filter(|(kind, _)| *kind == agent)
            .map(|(_, result)| match result {
                Ok(true) => language.text(Message::SettingsAgentsEnabled).to_owned(),
                Ok(false) => language.text(Message::SettingsAgentsDisabled).to_owned(),
                Err(error) => language.format(Message::SettingsAgentsFailed, &[("error", error)]),
            })
            .or_else(|| {
                state.error.as_ref().map(|error| {
                    language.format(Message::SettingsAgentsFailed, &[("error", error)])
                })
            })
            .or_else(|| {
                (state.helper_missing && row.executable.is_some()).then(|| {
                    language
                        .text(Message::SettingsAgentsHelperMissing)
                        .to_owned()
                })
            })
            .or_else(|| {
                (!state.available && row.executable.is_some())
                    .then(|| language.text(Message::SettingsAgentsInitialize).to_owned())
            });
        let mut text = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(agent_name(agent)),
            )
            .child(
                div()
                    .debug_selector(move || format!("agent-cli-path-{index}"))
                    .w_full()
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(executable),
            );
        if !config.is_empty() {
            text = text.child(
                div()
                    .debug_selector(move || format!("agent-hook-config-{index}"))
                    .w_full()
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(config),
            );
        }
        if let Some(note) = note {
            text = text.child(
                div()
                    .debug_selector(move || format!("agent-hook-note-{index}"))
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(SharedString::from(note)),
            );
        }
        // C5:SEC-12 命中的原命令必须原文可见,并由用户显式确认后重试。
        if agent == AgentHook::Claude
            && let Some(suspicious) = self.agents.suspicious_statusline.as_ref()
        {
            text = text.child(
                div()
                    .debug_selector(move || format!("agent-hook-suspended-{index}"))
                    .text_size(px(11.0))
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(suspicious.clone())),
            );
            text = text.child(
                Button::new(SharedString::from(format!("agent-hook-confirm-{index}")))
                    .label(language.text(Message::SettingsAgentsConfirmStatusline))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_agent_enabled_confirmed(agent, true, window, cx);
                    })),
            );
        }
        h_flex()
            .id(SharedString::from(format!("agent-hook-row-{index}")))
            .debug_selector(move || format!("agent-hook-row-{index}"))
            .w_full()
            .min_h(px(64.0))
            .px_4()
            .py_3()
            .gap_3()
            .border_b_1()
            .border_color(if index + 1 < total {
                border
            } else {
                gpui::transparent_black()
            })
            .child(text)
            .child(
                div()
                    .debug_selector(move || format!("agent-hook-status-{index}"))
                    .flex_shrink_0()
                    .max_w(px(180.0))
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(SharedString::from(status)),
            )
            .child(
                div()
                    .debug_selector(move || format!("agent-hook-toggle-{index}"))
                    .flex_shrink_0()
                    .min_w(px(40.0))
                    .min_h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Switch::new(SharedString::from(format!(
                            "agent-hook-{}",
                            agent.settings_key()
                        )))
                        .checked(checked)
                        .disabled(disabled)
                        .on_click(cx.listener(
                            move |this, enabled, window, cx| {
                                this.set_agent_enabled(agent, *enabled, window, cx);
                            },
                        )),
                    ),
            )
    }

    fn hooks_editor_section(&self, cx: &Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        let mut section = self
            .group(language.text(Message::SettingsAgentsEditorGroup), cx)
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(language.text(Message::SettingsAgentsEditorDescription)),
            )
            // user 层标注:设置页没有 project/local 上下文,不做选择器。
            .child(
                div()
                    .debug_selector(|| "settings-hooks-scope".to_owned())
                    .mt(px(2.0))
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(language.text(Message::SettingsAgentsScopeUser)),
            );
        if let Some(error) = self.agents.document_error.as_ref() {
            return section.child(
                div()
                    .debug_selector(|| "settings-hooks-document-error".to_owned())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .text_color(cx.theme().danger)
                    .child(
                        language.format(Message::SettingsAgentsEditorInvalid, &[("error", error)]),
                    ),
            );
        }
        let Some(document) = self.agents.document.as_ref() else {
            return section;
        };
        let owned_count = document.owned_group_count().to_string();
        let owned_events = document.owned_events();
        section = section.child(
            div()
                .debug_selector(|| "settings-hooks-owned".to_owned())
                .mt(px(8.0))
                .text_size(px(12.0))
                .text_color(muted)
                .child(language.format(
                    Message::SettingsAgentsEditorOwned,
                    &[("count", &owned_count)],
                )),
        );
        for (event, groups) in owned_events {
            section = section.child(
                div()
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(SharedString::from(format!("{event} × {groups}"))),
            );
        }
        section =
            section
                .child(self.hooks_form_mode_toggle(cx))
                .child(match self.agents.form_mode {
                    HooksFormMode::Form => self.hooks_form_section(cx),
                    HooksFormMode::Json => self.hooks_json_section(cx),
                });
        if let Some(error) = self.agents.editor_error.as_ref() {
            section = section.child(
                div()
                    .debug_selector(|| "settings-hooks-editor-error".to_owned())
                    .mt(px(6.0))
                    .text_size(px(12.0))
                    .text_color(cx.theme().danger)
                    .child(
                        language.format(Message::SettingsAgentsEditorInvalid, &[("error", error)]),
                    ),
            );
        } else if self.agents.editor_saved {
            section = section.child(
                div()
                    .debug_selector(|| "settings-hooks-editor-saved".to_owned())
                    .mt(px(6.0))
                    .text_size(px(12.0))
                    .text_color(cx.theme().success)
                    .child(language.text(Message::SettingsAgentsEditorSaved)),
            );
        }
        section
    }

    /// 表单 / JSON 模式切换(默认结构化表单)。
    fn hooks_form_mode_toggle(&self, cx: &Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        h_flex()
            .w_full()
            .mt(px(10.0))
            .mb(px(6.0))
            .gap_2()
            .child(
                div()
                    .debug_selector(|| "settings-hooks-mode-form".to_owned())
                    .child(
                        Button::new("settings-hooks-mode-form")
                            .label(language.text(Message::SettingsAgentsModeForm))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_hooks_form_mode(HooksFormMode::Form, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "settings-hooks-mode-json".to_owned())
                    .child(
                        Button::new("settings-hooks-mode-json")
                            .label(language.text(Message::SettingsAgentsModeJson))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_hooks_form_mode(HooksFormMode::Json, cx);
                            })),
                    ),
            )
    }

    /// 结构化表单主体:事件组卡片 + 保存(默认模式)。
    fn hooks_form_section(&self, cx: &Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        if let Some(error) = self.agents.form_error.as_ref() {
            return div()
                .debug_selector(|| "settings-hooks-form-error".to_owned())
                .w_full()
                .text_size(px(12.0))
                .text_color(cx.theme().danger)
                .child(SharedString::from(error.clone()));
        }
        let Some(form) = self.agents.form.as_ref() else {
            return div();
        };
        let mut body = v_flex()
            .w_full()
            .gap(px(10.0))
            .debug_selector(|| "settings-hooks-form".to_owned());
        if form.groups.is_empty() {
            body = body.child(
                div()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(language.text(Message::SettingsAgentsFormEmpty)),
            );
        }
        for (group_index, group) in form.groups.iter().enumerate() {
            body = body.child(self.hooks_form_group(group_index, group, cx));
        }
        body.child(
            h_flex()
                .w_full()
                .justify_between()
                .gap_2()
                .child(
                    Button::new("settings-hooks-form-add-group")
                        .label(language.text(Message::SettingsAgentsFormAddGroup))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_hooks_form_group(window, cx);
                        })),
                )
                .child(
                    div()
                        .debug_selector(|| "settings-hooks-form-save".to_owned())
                        .child(
                            Button::new("settings-hooks-form-save-button")
                                .label(language.text(Message::SettingsAgentsEditorApply))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_hooks_form(window, cx);
                                })),
                        ),
                ),
        )
    }

    fn hooks_form_group(
        &self,
        group_index: usize,
        group: &HookGroupForm,
        cx: &Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let mut card = v_flex()
            .w_full()
            .gap(px(8.0))
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(border)
            .debug_selector(move || format!("settings-hooks-form-group-{group_index}"))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child(language.text(Message::SettingsAgentsFormEvent)),
                    )
                    .child(
                        div()
                            .w(px(180.0))
                            .debug_selector(move || {
                                format!("settings-hooks-form-event-{group_index}")
                            })
                            .child(Select::new(&group.event_select)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child(language.text(Message::SettingsAgentsFormMatcher)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(move || {
                                format!("settings-hooks-form-matcher-{group_index}")
                            })
                            .child(Input::new(&group.matcher_input)),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "settings-hooks-form-remove-group-{group_index}"
                        )))
                        .label(language.text(Message::SettingsAgentsFormRemoveGroup))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_hooks_form_group(group_index, cx);
                        })),
                    ),
            );
        for (handler_index, handler) in group.handlers.iter().enumerate() {
            card = card.child(self.hooks_form_handler(group_index, handler_index, handler, cx));
        }
        card.child(
            Button::new(SharedString::from(format!(
                "settings-hooks-form-add-handler-{group_index}"
            )))
            .label(language.text(Message::SettingsAgentsFormAddHandler))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.add_hooks_form_handler(group_index, window, cx);
            })),
        )
    }

    fn hooks_form_handler(
        &self,
        group_index: usize,
        handler_index: usize,
        handler: &HandlerFormRow,
        cx: &Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let mut card = v_flex()
            .w_full()
            .gap(px(6.0))
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .debug_selector(move || {
                format!("settings-hooks-form-handler-{group_index}-{handler_index}")
            })
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child(language.text(Message::SettingsAgentsFormType)),
                    )
                    .child(
                        div()
                            .w(px(160.0))
                            .debug_selector(move || {
                                format!("settings-hooks-form-type-{group_index}-{handler_index}")
                            })
                            .child(Select::new(&handler.type_select)),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(SharedString::from(format!(
                            "settings-hooks-form-remove-handler-{group_index}-{handler_index}"
                        )))
                        .label(language.text(Message::SettingsAgentsFormRemoveHandler))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_hooks_form_handler(group_index, handler_index, cx);
                        })),
                    ),
            );
        for field in type_fields(handler.kind).iter().chain(COMMON_FIELDS.iter()) {
            card =
                card.child(self.hooks_form_field(group_index, handler_index, handler, *field, cx));
        }
        card
    }

    /// 单字段行:布尔字段用开关,其余用文本输入。
    fn hooks_form_field(
        &self,
        group_index: usize,
        handler_index: usize,
        handler: &HandlerFormRow,
        field: HandlerField,
        cx: &Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        let key = field.key();
        let label = language.text(field.message());
        let required = handler.kind.required_fields().contains(&key);
        let caption = if required {
            SharedString::from(format!("{label} *"))
        } else {
            SharedString::from(label)
        };
        if field.is_flag() {
            let checked = match field {
                HandlerField::Async => handler.asynchronous,
                HandlerField::AsyncRewake => handler.async_rewake,
                HandlerField::ContinueOnBlock => handler.continue_on_block,
                _ => false,
            };
            return h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .debug_selector(move || {
                    format!("settings-hooks-form-flag-{group_index}-{handler_index}-{key}")
                })
                .child(div().text_size(px(11.0)).text_color(muted).child(caption))
                .child(
                    Switch::new(SharedString::from(format!(
                        "settings-hooks-form-switch-{group_index}-{handler_index}-{key}"
                    )))
                    .checked(checked)
                    .on_click(cx.listener(move |this, enabled, _, cx| {
                        this.set_hooks_form_flag(group_index, handler_index, field, *enabled, cx);
                    })),
                );
        }
        let Some(input) = handler.texts.get(&field) else {
            return div();
        };
        v_flex()
            .w_full()
            .gap(px(2.0))
            .debug_selector(move || {
                format!("settings-hooks-form-field-{group_index}-{handler_index}-{key}")
            })
            .child(div().text_size(px(11.0)).text_color(muted).child(caption))
            .child(Input::new(input).w_full())
    }

    /// JSON 高级模式:原始文本编辑框(既有双轨边界不变)。
    fn hooks_json_section(&self, cx: &Context<'_, Self>) -> gpui::Div {
        let language = self.language(cx);
        div()
            .w_full()
            .child(
                div()
                    .mb(px(4.0))
                    .text_size(px(12.0))
                    .child(language.text(Message::SettingsAgentsEditorEditable)),
            )
            .child(
                div()
                    .debug_selector(|| "settings-hooks-editor".to_owned())
                    .w_full()
                    .child(Input::new(&self.agents.hooks_editor).h(px(220.0))),
            )
            .child(
                h_flex().w_full().justify_end().mt(px(8.0)).child(
                    div()
                        .debug_selector(|| "settings-hooks-editor-save".to_owned())
                        .child(
                            Button::new("settings-hooks-editor-save")
                                .label(language.text(Message::SettingsAgentsEditorApply))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_hooks_editor(window, cx);
                                })),
                        ),
                ),
            )
    }

    pub(super) fn page_agents(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        if self.agents.rows.is_none() && !self.agents.loading {
            self.refresh_agents(cx);
        }
        let language = self.language(cx);
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let mut list = v_flex()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(border);
        if let Some(rows) = self.agents.rows.as_ref() {
            let total = rows.len();
            for (index, row) in rows.iter().enumerate() {
                list = list.child(self.agent_hook_row(index, row, total, cx));
            }
        }
        let mut integration = self
            .group(language.text(Message::SettingsAgentsIntegrationGroup), cx)
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child(language.text(Message::SettingsAgentsDescription)),
                    )
                    .child(
                        Button::new("agents-refresh")
                            .label(language.text(Message::SettingsAgentsRefresh))
                            .disabled(self.agents.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_agents(cx))),
                    ),
            );
        if self.agents.loading {
            integration = integration.child(
                div()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(language.text(Message::SettingsAgentsChecking)),
            );
        }
        integration = integration.child(div().mt(px(8.0)).child(list)).child(
            div()
                .mt(px(6.0))
                .text_size(px(11.0))
                .text_color(muted)
                .child(language.text(Message::SettingsAgentsRestart)),
        );
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(integration)
            .child(self.hooks_editor_section(cx))
    }
}

/// 事件下拉:事件名是协议标识,保持原文不做翻译。
fn build_event_select(
    event: &str,
    window: &mut Window,
    cx: &mut Context<'_, SettingsPane>,
) -> Result<Entity<SelectState<Vec<SharedString>>>, String> {
    let events = hook_events();
    let selected = events
        .iter()
        .position(|candidate| *candidate == event)
        .ok_or_else(|| event.to_owned())?;
    let labels = events
        .iter()
        .map(|event| SharedString::from(*event))
        .collect::<Vec<_>>();
    Ok(cx.new(|cx| SelectState::new(labels, Some(IndexPath::default().row(selected)), window, cx)))
}

/// 类型下拉:五型标签本地化,选中索引与 `HookHandlerType::ALL` 同序。
fn build_type_select(
    kind: HookHandlerType,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsPane>,
) -> Entity<SelectState<Vec<SharedString>>> {
    let labels = HookHandlerType::ALL
        .iter()
        .map(|kind| SharedString::from(language.text(handler_type_message(*kind))))
        .collect::<Vec<_>>();
    let selected = HookHandlerType::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(0);
    cx.new(|cx| SelectState::new(labels, Some(IndexPath::default().row(selected)), window, cx))
}

fn make_input(
    value: String,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<'_, SettingsPane>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

/// 一条 handler 行:类型下拉 + 全量文本输入槽 + 类型切换订阅。
fn build_handler_row(
    id: u64,
    kind: HookHandlerType,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsPane>,
) -> (HandlerFormRow, gpui::Subscription) {
    let type_select = build_type_select(kind, language, window, cx);
    let subscription = cx.subscribe_in(
        &type_select,
        window,
        move |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
            if !matches!(event, SelectEvent::Confirm(Some(_))) {
                return;
            }
            let Some(index) = entity.read(cx).selected_index(cx).map(|path| path.row) else {
                return;
            };
            let Some(kind) = HookHandlerType::ALL.get(index).copied() else {
                return;
            };
            this.switch_hooks_handler_kind(id, kind, window, cx);
        },
    );
    let mut texts = HashMap::new();
    for field in HandlerField::ALL {
        if field.is_flag() {
            continue;
        }
        let input = make_input(String::new(), language.text(field.message()), window, cx);
        texts.insert(field, input);
    }
    (
        HandlerFormRow {
            id,
            kind,
            type_select,
            texts,
            asynchronous: false,
            async_rewake: false,
            continue_on_block: false,
            extra: Map::new(),
        },
        subscription,
    )
}

/// 文档可编辑子树 → 表单模型;未知事件/类型/字段形态返回可展示的错误。
fn build_hook_form(
    document: &HooksDocument,
    language: UiLanguage,
    window: &mut Window,
    cx: &mut Context<'_, SettingsPane>,
    subscriptions: &mut Vec<gpui::Subscription>,
) -> Result<HookFormState, String> {
    let events = document
        .editable_value()
        .as_object()
        .ok_or_else(|| form_load_error(language, "hooks"))?;
    let mut groups = Vec::new();
    let mut next_id = 0u64;
    for (event, groups_value) in events {
        let event_groups = groups_value
            .as_array()
            .ok_or_else(|| form_load_error(language, event))?;
        for (group_index, group_value) in event_groups.iter().enumerate() {
            let group_path = format!("{event}[{group_index}]");
            let object = group_value
                .as_object()
                .ok_or_else(|| form_load_error(language, &group_path))?;
            let matcher = match object.get("matcher") {
                Some(Value::String(matcher)) => matcher.clone(),
                Some(_) => return Err(form_load_error(language, &format!("{group_path}.matcher"))),
                None => String::new(),
            };
            let handler_values = object
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| form_load_error(language, &format!("{group_path}.hooks")))?;
            let event_select = build_event_select(event, window, cx).map_err(|event| {
                language.format(
                    Message::SettingsAgentsFormUnsupportedEvent,
                    &[("event", &event)],
                )
            })?;
            let mut handlers = Vec::with_capacity(handler_values.len());
            for (handler_index, handler_value) in handler_values.iter().enumerate() {
                let path = format!("{group_path}.hooks[{handler_index}]");
                let handler = handler_value
                    .as_object()
                    .ok_or_else(|| form_load_error(language, &path))?;
                let kind_text = handler
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| form_load_error(language, &format!("{path}.type")))?;
                let kind = HookHandlerType::parse_type(kind_text).ok_or_else(|| {
                    language.format(
                        Message::SettingsAgentsFormUnsupportedType,
                        &[("kind", kind_text)],
                    )
                })?;
                let (mut row, subscription) =
                    build_handler_row(next_id, kind, language, window, cx);
                next_id += 1;
                for field in HandlerField::ALL {
                    if field.is_flag() {
                        continue;
                    }
                    let value = load_field_text(handler, field, language, &path)?;
                    if let Some(input) = row.texts.get(&field) {
                        input.update(cx, |state, cx| state.set_value(value, window, cx));
                    }
                }
                row.asynchronous = load_flag(handler, HandlerField::Async, language, &path)?;
                row.async_rewake = load_flag(handler, HandlerField::AsyncRewake, language, &path)?;
                row.continue_on_block =
                    load_flag(handler, HandlerField::ContinueOnBlock, language, &path)?;
                row.extra = handler
                    .iter()
                    .filter(|(key, _)| {
                        key.as_str() != "type" && HandlerField::from_key(key.as_str()).is_none()
                    })
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                subscriptions.push(subscription);
                handlers.push(row);
            }
            let extra = object
                .iter()
                .filter(|(key, _)| key.as_str() != "matcher" && key.as_str() != "hooks")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            groups.push(HookGroupForm {
                event_select,
                matcher_input: make_input(
                    matcher,
                    language.text(Message::SettingsAgentsFormMatcher),
                    window,
                    cx,
                ),
                handlers,
                extra,
            });
        }
    }
    Ok(HookFormState { groups, next_id })
}

/// 表单 → hooks JSON(事件名 → matcher 组数组);同事件多组按键合并。
fn collect_hook_form(
    form: &HookFormState,
    language: UiLanguage,
    cx: &App,
) -> Result<Value, String> {
    let mut root = Map::new();
    for group in &form.groups {
        let (event, value) = collect_group(group, language, cx)?;
        let entry = root
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(array) = entry.as_array_mut() else {
            return Err(form_load_error(language, "hooks"));
        };
        array.push(value);
    }
    Ok(Value::Object(root))
}

fn collect_group(
    group: &HookGroupForm,
    language: UiLanguage,
    cx: &App,
) -> Result<(String, Value), String> {
    let events = hook_events();
    let index = group
        .event_select
        .read(cx)
        .selected_index(cx)
        .map(|path| path.row)
        .unwrap_or(0);
    let event = events
        .get(index)
        .or_else(|| events.first())
        .copied()
        .unwrap_or("Stop")
        .to_owned();
    let matcher = group.matcher_input.read(cx).value().to_string();
    let mut object = group.extra.clone();
    if !matcher.trim().is_empty() {
        object.insert("matcher".to_owned(), Value::String(matcher));
    }
    let mut handlers = Vec::with_capacity(group.handlers.len());
    for handler in &group.handlers {
        handlers.push(collect_handler(handler, language, cx)?);
    }
    object.insert("hooks".to_owned(), Value::Array(handlers));
    Ok((event, Value::Object(object)))
}

fn collect_handler(row: &HandlerFormRow, language: UiLanguage, cx: &App) -> Result<Value, String> {
    let text = |field: HandlerField| {
        row.texts
            .get(&field)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    };
    let mut object = row.extra.clone();
    object.insert(
        "type".to_owned(),
        Value::String(row.kind.as_str().to_owned()),
    );
    match row.kind {
        HookHandlerType::Command => {
            object.insert(
                "command".to_owned(),
                Value::String(text(HandlerField::Command)),
            );
            insert_json_field(
                &mut object,
                "args",
                text(HandlerField::Args),
                JsonShape::Array,
                HandlerField::Args,
                language,
            )?;
            if row.asynchronous {
                object.insert("async".to_owned(), Value::Bool(true));
            }
            if row.async_rewake {
                object.insert("asyncRewake".to_owned(), Value::Bool(true));
            }
            insert_string_field(&mut object, "shell", text(HandlerField::Shell));
        }
        HookHandlerType::Http => {
            object.insert("url".to_owned(), Value::String(text(HandlerField::Url)));
            insert_json_field(
                &mut object,
                "headers",
                text(HandlerField::Headers),
                JsonShape::Object,
                HandlerField::Headers,
                language,
            )?;
            insert_json_field(
                &mut object,
                "allowedEnvVars",
                text(HandlerField::AllowedEnvVars),
                JsonShape::Array,
                HandlerField::AllowedEnvVars,
                language,
            )?;
        }
        HookHandlerType::McpTool => {
            object.insert(
                "server".to_owned(),
                Value::String(text(HandlerField::Server)),
            );
            object.insert("tool".to_owned(), Value::String(text(HandlerField::Tool)));
            insert_json_field(
                &mut object,
                "input",
                text(HandlerField::Input),
                JsonShape::Object,
                HandlerField::Input,
                language,
            )?;
        }
        HookHandlerType::Prompt => {
            object.insert(
                "prompt".to_owned(),
                Value::String(text(HandlerField::Prompt)),
            );
            insert_string_field(&mut object, "model", text(HandlerField::Model));
            if row.continue_on_block {
                object.insert("continueOnBlock".to_owned(), Value::Bool(true));
            }
        }
        HookHandlerType::Agent => {
            object.insert(
                "prompt".to_owned(),
                Value::String(text(HandlerField::Prompt)),
            );
            insert_string_field(&mut object, "model", text(HandlerField::Model));
        }
    }
    insert_string_field(&mut object, "if", text(HandlerField::If));
    let timeout = text(HandlerField::Timeout);
    if !timeout.trim().is_empty() {
        let value = timeout.trim().parse::<u64>().map_err(|_| {
            invalid_field(
                language,
                HandlerField::Timeout,
                language.text(Message::SettingsAgentsFormInvalidTimeout),
            )
        })?;
        object.insert("timeout".to_owned(), json!(value));
    }
    insert_string_field(
        &mut object,
        "statusMessage",
        text(HandlerField::StatusMessage),
    );
    Ok(Value::Object(object))
}

/// 状态语义:installed/needs_repair 双事实优先,错误次之。
fn agent_status(row: &AgentIntegration) -> Message {
    let state = &row.inspection;
    if state.installed && state.needs_repair {
        Message::SettingsAgentsNeedsRepair
    } else if state.installed {
        Message::SettingsAgentsInstalled
    } else if state.error.is_some() {
        Message::SettingsAgentsNeedsAttention
    } else {
        Message::SettingsAgentsNotInstalled
    }
}

/// 可切换判据:已装(可卸)或「可执行文件 + 配置足迹 + helper 在位」。
fn can_toggle(row: &AgentIntegration) -> bool {
    row.inspection.installed
        || (row.executable.is_some() && row.inspection.available && !row.inspection.helper_missing)
}

/// 品牌名是专有名词,不做翻译(显示名单点;身份枚举归 03 篇后续收口)。
fn agent_name(agent: AgentHook) -> &'static str {
    match agent {
        AgentHook::Claude => "Claude Code",
        AgentHook::Codex => "Codex",
        AgentHook::OpenCode => "OpenCode",
        AgentHook::Pi => "Pi",
        AgentHook::Copilot => "GitHub Copilot",
        AgentHook::Grok => "Grok",
        AgentHook::OhMyPi => "Oh My Pi",
        AgentHook::Cursor => "Cursor",
        AgentHook::Kimi => "Kimi",
    }
}

/// 虚拟窗口用例的驱动端(生产不编译)。
#[cfg(all(test, feature = "gpui-test-support"))]
impl SettingsPane {
    pub(crate) fn hooks_rows_for_test(&self) -> Option<&Vec<AgentIntegration>> {
        self.agents.rows.as_ref()
    }

    pub(crate) fn set_hooks_document_for_test(
        &mut self,
        document: Result<HooksDocument, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.set_editor_document(document, window, cx);
    }

    pub(crate) fn hooks_document_for_test(&self) -> Option<&HooksDocument> {
        self.agents.document.as_ref()
    }

    pub(crate) fn hooks_editor_text_for_test(&self, cx: &gpui::App) -> String {
        self.agents.hooks_editor.read(cx).value().to_string()
    }

    pub(crate) fn set_hooks_editor_text_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.agents
            .hooks_editor
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

    pub(crate) fn save_hooks_editor_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.save_hooks_editor(window, cx);
    }

    pub(crate) fn hooks_editor_error_for_test(&self) -> Option<&String> {
        self.agents.editor_error.as_ref()
    }

    /// 测试:切换表单 / JSON 呈现模式。
    pub(crate) fn set_hooks_form_mode_for_test(&mut self, json: bool, cx: &mut Context<'_, Self>) {
        self.set_hooks_form_mode(
            if json {
                HooksFormMode::Json
            } else {
                HooksFormMode::Form
            },
            cx,
        );
    }

    pub(crate) fn hooks_form_group_count_for_test(&self) -> usize {
        self.agents
            .form
            .as_ref()
            .map_or(0, |form| form.groups.len())
    }

    pub(crate) fn hooks_form_handler_count_for_test(&self, group: usize) -> usize {
        self.agents
            .form
            .as_ref()
            .and_then(|form| form.groups.get(group))
            .map_or(0, |group| group.handlers.len())
    }

    pub(crate) fn hooks_form_add_group_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        self.add_hooks_form_group(window, cx)
    }

    pub(crate) fn hooks_form_set_group_event_for_test(
        &mut self,
        group: usize,
        event: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(index) = hook_events()
            .iter()
            .position(|candidate| *candidate == event)
        else {
            return false;
        };
        let Some(select) = self
            .agents
            .form
            .as_ref()
            .and_then(|form| form.groups.get(group))
            .map(|group| group.event_select.clone())
        else {
            return false;
        };
        select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::default().row(index)), window, cx);
        });
        true
    }

    pub(crate) fn hooks_form_set_group_matcher_for_test(
        &mut self,
        group: usize,
        matcher: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(input) = self
            .agents
            .form
            .as_ref()
            .and_then(|form| form.groups.get(group))
            .map(|group| group.matcher_input.clone())
        else {
            return false;
        };
        input.update(cx, |state, cx| {
            state.set_value(matcher.to_owned(), window, cx);
        });
        true
    }

    pub(crate) fn hooks_form_add_handler_for_test(
        &mut self,
        group: usize,
        kind: &str,
        values: &[(&str, &str)],
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let kind = HookHandlerType::parse_type(kind)?;
        let handler = self.add_hooks_form_handler_with_kind(group, kind, window, cx)?;
        for &(field, value) in values {
            self.hooks_form_set_handler_field_for_test(group, handler, field, value, window, cx);
        }
        Some(handler)
    }

    pub(crate) fn hooks_form_set_handler_field_for_test(
        &mut self,
        group: usize,
        handler: usize,
        field: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(field) = HandlerField::from_key(field) else {
            return false;
        };
        let Some(input) = self.handler_text_input(group, handler, field) else {
            return false;
        };
        input.update(cx, |state, cx| {
            state.set_value(value.to_owned(), window, cx);
        });
        self.agents.editor_saved = false;
        true
    }

    pub(crate) fn hooks_form_handler_field_for_test(
        &self,
        group: usize,
        handler: usize,
        field: &str,
        cx: &gpui::App,
    ) -> Option<String> {
        let field = HandlerField::from_key(field)?;
        self.handler_text_input_ro(group, handler, field)
            .map(|input| input.read(cx).value().to_string())
    }

    pub(crate) fn hooks_form_set_handler_kind_for_test(
        &mut self,
        group: usize,
        handler: usize,
        kind: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(kind) = HookHandlerType::parse_type(kind) else {
            return false;
        };
        let Some(id) = self
            .agents
            .form
            .as_ref()
            .and_then(|form| form.groups.get(group))
            .and_then(|group| group.handlers.get(handler))
            .map(|row| row.id)
        else {
            return false;
        };
        self.switch_hooks_handler_kind(id, kind, window, cx);
        true
    }

    pub(crate) fn save_hooks_form_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.save_hooks_form(window, cx);
    }

    pub(crate) fn hooks_form_mode_is_json_for_test(&self) -> bool {
        self.agents.form_mode == HooksFormMode::Json
    }

    pub(crate) fn hooks_form_error_for_test(&self) -> Option<&String> {
        self.agents.form_error.as_ref()
    }

    fn handler_text_input(
        &self,
        group: usize,
        handler: usize,
        field: HandlerField,
    ) -> Option<Entity<InputState>> {
        self.handler_text_input_ro(group, handler, field).cloned()
    }

    fn handler_text_input_ro(
        &self,
        group: usize,
        handler: usize,
        field: HandlerField,
    ) -> Option<&Entity<InputState>> {
        self.agents
            .form
            .as_ref()?
            .groups
            .get(group)?
            .handlers
            .get(handler)?
            .texts
            .get(&field)
    }

    pub(crate) fn set_agent_enabled_for_test(
        &mut self,
        agent: AgentHook,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 后台 inspect 在虚拟窗口里不保证已排水;测试驱动端视为已就绪。
        self.agents.loading = false;
        self.set_agent_enabled(agent, enabled, window, cx);
    }

    /// 同步重读 inspect 行(后台刷新在虚拟窗口里不保证已排水)。
    pub(crate) fn refresh_hooks_rows_for_test(&mut self) {
        self.agents.rows = Some(self.hooks_target.inspect());
    }

    /// C5:待确认的 statusLine 原文(设置页确认重试用例)。
    pub(crate) fn hooks_suspicious_statusline_for_test(&self) -> Option<&String> {
        self.agents.suspicious_statusline.as_ref()
    }

    /// C5:确认后重试(恒放行 SEC-12 审查门)。
    pub(crate) fn set_agent_enabled_confirmed_for_test(
        &mut self,
        agent: AgentHook,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.agents.loading = false;
        self.set_agent_enabled_confirmed(agent, enabled, window, cx);
    }
}

#[cfg(test)]
mod agents_tests {
    use super::*;

    #[test]
    fn status_prefers_repair_and_install_facts_over_errors() {
        let mut row = AgentIntegration {
            agent: AgentHook::Claude,
            executable: Some(PathBuf::from("C:/bin/claude.exe")),
            inspection: integrations::HookInspection {
                available: true,
                installed: true,
                needs_repair: true,
                enabled: true,
                ..Default::default()
            },
        };
        assert_eq!(agent_status(&row), Message::SettingsAgentsNeedsRepair);
        row.inspection.needs_repair = false;
        assert_eq!(agent_status(&row), Message::SettingsAgentsInstalled);
        row.inspection.installed = false;
        row.inspection.enabled = false;
        row.inspection.error = Some("boom".to_owned());
        assert_eq!(agent_status(&row), Message::SettingsAgentsNeedsAttention);
        row.inspection.error = None;
        assert_eq!(agent_status(&row), Message::SettingsAgentsNotInstalled);
    }

    #[test]
    fn toggle_requires_installation_or_a_complete_probe() {
        let mut row = AgentIntegration {
            agent: AgentHook::Claude,
            executable: None,
            inspection: integrations::HookInspection {
                available: true,
                ..Default::default()
            },
        };
        assert!(!can_toggle(&row));
        row.inspection.installed = true;
        assert!(can_toggle(&row), "已装永远可卸");
        row.inspection.installed = false;
        row.executable = Some(PathBuf::from("C:/bin/claude.exe"));
        assert!(can_toggle(&row));
        row.inspection.helper_missing = true;
        assert!(!can_toggle(&row));
    }
}
