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
use crate::ai_hook::integrations::{AgentIntegration, HookInspection, agents};
use crate::i18n::Message;

/// 每行开关按同一 catalog 派生顺序读取对应设置键。
fn agent_row_states(values: &slterm_settings::RawSettings) -> Vec<bool> {
    agents()
        .into_iter()
        .map(|agent| agent.enabled(values))
        .collect()
}

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
    let mut enabled = Vec::new();
    if let Some(root) = root_entity.as_ref() {
        let root = root.read(cx);
        integrations = root.agent_integrations.clone();
        inspecting = root.agent_hooks_inspecting;
        failed = root.agent_hooks_error;
        enabled = agent_row_states(root.settings.values());
    }

    let rows = agents()
        .into_iter()
        .enumerate()
        .map(|(index, agent)| {
            let slug = agent.slug();
            let name = agent.label();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 行开关按 catalog 派生顺序读取各自设置键，不依赖 AgentHook::ALL 下标。
    #[test]
    fn agent_row_states_follow_catalog_order_not_all_order() {
        let raw = slterm_settings::RawSettings::from_json_bytes(
            br#"{"agent_hooks":{"slterm_ai_hooks_cursor":true}}"#,
        );
        let states = agent_row_states(&raw);
        let cursor = agents()
            .into_iter()
            .position(|agent| agent == AgentHook::Cursor)
            .expect("Cursor 必须有 hooks 适配器");
        assert!(states[cursor], "Cursor 行必须读取自己的设置键");
        // Claude/Codex 走全局兜底默认开启，Cursor 显式开启。
        assert_eq!(
            states.iter().filter(|enabled| **enabled).count(),
            3,
            "默认两行加显式开启一行"
        );
    }

    /// 设置域 Hooks 名单与 AgentCatalog 的适配器声明集合完全一致。
    #[test]
    fn catalog_hooks_match_the_settings_hook_enum() {
        let catalog = agents();
        let expected = crate::ai_agents::AgentKind::ALL
            .into_iter()
            .filter(|kind| kind.catalog().hooks_adapter.is_some())
            .count();
        assert_eq!(catalog.len(), expected);
        let mut catalog_slugs: Vec<_> = catalog.iter().map(|agent| agent.slug()).collect();
        let mut settings_slugs: Vec<_> = AgentHook::ALL.iter().map(|agent| agent.slug()).collect();
        catalog_slugs.sort_unstable();
        settings_slugs.sort_unstable();
        assert_eq!(catalog_slugs, settings_slugs);
    }
}
