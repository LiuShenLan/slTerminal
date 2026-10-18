//! 设置中心 Agent 组 hooks 页(06 篇轨二 + 03 篇安装权威联调)。
//!
//! 两条轨道在这里汇合而不交叉:
//! - 轨一(接入偏好):每 agent 的 `AgentHook` 键经设置页统一写通道落盘;
//!   安装/卸载由 03 篇安装权威执行(设置页不直接改 provider 文件)。
//! - 轨二(配置编辑):数据源 = `integrations::inspect()` 的 installed/
//!   needs_repair/helper_missing 三态与 config_path;编辑器只编辑用户/第三方
//!   handler,slterm marker 认领组只读展示,保存时由 `HooksDocument` 原样保留。
//!
//! 文件 I/O(inspect / 文档加载 / 保存)与既有 settings 写通道一样在本页
//! 同步完成;inspect 放后台执行器,完成回前台刷新。测试注入 tempdir 假 home,
//! 绝不触碰真实 `%USERPROFILE%`。

use std::path::PathBuf;

use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, ParentElement as _, SharedString,
    Styled as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, Disableable as _, h_flex, v_flex};
use slterm_settings::AgentHook;

use crate::ai_hook::editor::HooksDocument;
use crate::ai_hook::integrations::{self, AgentIntegration};
use crate::i18n::Message;

use super::SettingsPane;
use super::design::SETTINGS_GROUP_GAP;

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
        }
    }
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
            }
            Err(error) => {
                self.agents.document = None;
                self.agents.document_error = Some(error);
            }
        }
        self.agents.editor_saved = false;
        self.agents.editor_error = None;
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

    /// 轨二提交:编辑文本 → SEC-05 + marker 边界 → 原子写 → 重读。
    fn save_hooks_editor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(document) = self.agents.document.clone() else {
            return;
        };
        let raw = self.agents.hooks_editor.read(cx).value().to_string();
        match document.save_editable(&raw) {
            Ok(()) => {
                match document.reload() {
                    Ok(reloaded) => {
                        let text = reloaded.editable_json();
                        self.agents.document = Some(reloaded);
                        self.agents.document_error = None;
                        self.agents
                            .hooks_editor
                            .update(cx, |state, cx| state.set_value(text, window, cx));
                    }
                    Err(error) => {
                        self.agents.document = None;
                        self.agents.document_error = Some(error);
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
        section = section
            .child(
                div()
                    .mt(px(10.0))
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
            );
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
