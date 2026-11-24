//! 设置页 AgentHooks 页：安装状态探测快照与授权开关的后台执行。
//!
//! 探测与安装/卸载都是阻塞 I/O，只在后台执行体运行；前台只读取快照、
//! 提交显式设置值并展示脱敏状态。

use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_settings::AgentHook;
use slterm_settings::json::Value;

use super::AppRoot;
use super::settings_overlay::SettingsView;
use super::task_service::{CancellationToken, TaskId};
use crate::ai_hook::integrations::{AGENTS, AgentIntegration, HookInspection};
use crate::i18n::Message;

/// 展示顺序与显示名同 `ai_hook::integrations::AGENTS`。
const AGENT_NAMES: [(&str, &str); 9] = [
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("opencode", "OpenCode"),
    ("cursor", "Cursor"),
    ("kimi", "Kimi"),
    ("pi", "Pi"),
    ("omp", "Oh My Pi"),
    ("copilot", "Copilot"),
    ("grok", "Grok"),
];

fn agent_state_message(integration: Option<&AgentIntegration>) -> Message {
    let Some(integration) = integration else {
        return Message::SettingsAgentsChecking;
    };
    let inspection: &HookInspection = &integration.inspection;
    if inspection.error.is_some() {
        return Message::SettingsAgentsNeedsAttention;
    }
    if integration.executable.is_none() {
        return Message::SettingsAgentsNotDetected;
    }
    if !inspection.available {
        return Message::SettingsAgentsInitialize;
    }
    if inspection.helper_missing {
        return Message::SettingsAgentsHelperMissing;
    }
    if inspection.needs_repair {
        return Message::SettingsAgentsNeedsRepair;
    }
    if inspection.installed {
        return Message::SettingsAgentsInstalled;
    }
    Message::SettingsAgentsNotInstalled
}

/// AgentHooks 页内容：每 agent 一行状态与授权开关，另加刷新入口。
pub(super) fn render_agent_hooks_page(
    view: &SettingsView,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let root_entity = view.root.upgrade();
    let mut integrations: Option<Vec<AgentIntegration>> = None;
    let mut inspecting = false;
    let mut failed = false;
    let mut enabled = [false; 9];
    if let Some(root) = root_entity.as_ref() {
        let root = root.read(cx);
        integrations = root.agent_integrations.clone();
        inspecting = root.agent_hooks_inspecting;
        failed = root.agent_hooks_error;
        enabled = AgentHook::ALL.map(|hook| hook.enabled(root.settings.values()));
    }

    let rows = AGENTS
        .into_iter()
        .zip(AGENT_NAMES)
        .enumerate()
        .map(|(index, (agent, (slug, name)))| {
            let integration = integrations
                .as_ref()
                .and_then(|list| list.iter().find(|entry| entry.agent == agent));
            let state = language.text(agent_state_message(integration));
            let is_enabled = enabled[index];
            let root_handle = view.root.clone();
            div()
                .py_1()
                .flex()
                .justify_between()
                .items_center()
                .gap_4()
                .debug_selector(move || format!("settings-agent-{slug}"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(name),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .debug_selector(move || format!("settings-agent-{slug}-state"))
                                .child(state),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("settings-agent-{slug}-toggle")))
                        .w(px(36.0))
                        .h(px(20.0))
                        .rounded_full()
                        .border_1()
                        .border_color(if is_enabled {
                            cx.theme().accent
                        } else {
                            cx.theme().border
                        })
                        .bg(if is_enabled {
                            cx.theme().accent.opacity(0.35)
                        } else {
                            cx.theme().transparent
                        })
                        .cursor_pointer()
                        .debug_selector(move || format!("settings-agent-{slug}-toggle"))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_view, _, _, cx| {
                                let Some(root_handle) = root_handle.upgrade() else {
                                    return;
                                };
                                root_handle.update(cx, |root, cx| {
                                    root.set_agent_hook_enabled(agent, !is_enabled, cx);
                                });
                            }),
                        )
                        .child(
                            div()
                                .size(px(14.0))
                                .m(px(3.0))
                                .rounded_full()
                                .when(is_enabled, |dot| dot.ml(px(19.0)))
                                .bg(if is_enabled {
                                    cx.theme().accent
                                } else {
                                    cx.theme().muted_foreground
                                }),
                        ),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();

    let refresh_root = view.root.clone();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .pb_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentsDescription)),
        )
        .when(failed, |element| {
            element.child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "settings-agents-error".to_owned())
                    .child(language.text(Message::SettingsAgentsApplyFailed)),
            )
        })
        .child(
            div()
                .id("settings-agents-refresh")
                .mb_2()
                .px_2()
                .h(px(22.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .text_xs()
                .cursor_pointer()
                .debug_selector(|| "settings-agents-refresh".to_owned())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_view, _, _, cx| {
                        let Some(root_handle) = refresh_root.upgrade() else {
                            return;
                        };
                        root_handle.update(cx, |root, cx| {
                            root.request_agent_hooks_inspect(cx);
                        });
                    }),
                )
                .child(if inspecting {
                    language.text(Message::SettingsAgentsChecking)
                } else {
                    language.text(Message::SettingsAgentsRefreshState)
                }),
        )
        .children(rows)
        .into_any_element()
}

impl AppRoot {
    /// 后台探测各 CLI 的 hook 安装状态；重复请求复用同一任务。
    pub(super) fn request_agent_hooks_inspect(&mut self, cx: &mut Context<'_, Self>) {
        if self.agent_hooks_inspecting {
            return;
        }
        #[cfg(test)]
        if let Some(probe) = self.agent_hooks_inspect_for_test {
            self.agent_integrations = Some(probe());
            self.agent_hooks_inspecting = false;
            cx.notify();
            return;
        }
        self.agent_hooks_inspecting = true;
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let integrations = cx
                .background_executor()
                .spawn(async { crate::ai_hook::integrations::inspect() })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.agent_integrations = Some(integrations);
                root.agent_hooks_inspecting = false;
                root.tasks.complete(TaskId::AgentHooksInspect);
                cx.notify();
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::AgentHooksInspect, cancellation, task)
        {
            self.agent_hooks_inspecting = false;
            log::warn!("agent hooks 探测任务注册失败: {error:?}");
        }
    }

    /// 授权开关：显式设置值立即入队，安装/卸载在后台执行并回投结果。
    pub(super) fn set_agent_hook_enabled(
        &mut self,
        hook: AgentHook,
        enabled: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.settings.queue_explicit(
            &[(hook.settings_key(), Value::Bool(enabled))],
            Instant::now(),
        ) {
            return;
        }
        #[cfg(test)]
        if let Some(apply) = self.agent_hook_apply_for_test {
            let result = apply(hook, enabled);
            self.finish_agent_hook_apply(hook, result, cx);
            return;
        }
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::ai_hook::integrations::apply_enabled(hook, enabled) })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.finish_agent_hook_apply(hook, result, cx);
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::AgentHookApply(hook), cancellation, task)
        {
            log::warn!("agent hook 安装任务注册失败: {error:?}");
        }
    }

    /// 安装/卸载回执：成功重新探测状态，失败只保留脱敏标记。
    fn finish_agent_hook_apply(
        &mut self,
        hook: AgentHook,
        result: Result<(), String>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::AgentHookApply(hook));
        match result {
            Ok(()) => {
                self.agent_hooks_error = false;
                self.request_agent_hooks_inspect(cx);
            }
            Err(_) => {
                self.agent_hooks_error = true;
            }
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn replace_agent_hooks_for_test(
        &mut self,
        inspect: fn() -> Vec<AgentIntegration>,
        apply: fn(AgentHook, bool) -> Result<(), String>,
    ) {
        self.agent_hooks_inspect_for_test = Some(inspect);
        self.agent_hook_apply_for_test = Some(apply);
    }
}
