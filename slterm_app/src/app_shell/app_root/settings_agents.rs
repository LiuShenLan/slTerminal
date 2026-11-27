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
use crate::ai_hook::integrations::{AgentIntegration, ApplyOutcome, HookInspection, agents};
use crate::ai_hook::{HOOK_JOURNAL_DIR, HookJournalPort};
use crate::i18n::Message;

/// 安装审查门命中后的待确认原文；命令只进入 UI，不进入日志或通知。
#[derive(Clone)]
pub(super) struct AgentHookConfirmation {
    pub(super) hook: AgentHook,
    pub(super) commands: Vec<String>,
    /// 命中审查门前的设置值；取消时回退到此值。
    pub(super) restore_enabled: bool,
    /// 确认后重试仍需的 Journal 端口；无 writer 的测试根为 None。
    pub(super) journal: Option<HookJournalPort>,
}

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
    let mut confirmations = Vec::new();
    let mut usage = None;
    if let Some(root) = root_entity.as_ref() {
        let root = root.read(cx);
        integrations = root.agent_integrations.clone();
        inspecting = root.agent_hooks_inspecting;
        failed = root.agent_hooks_error;
        enabled = agent_row_states(root.settings.values());
        confirmations = root.agent_hook_confirmations.clone();
        usage = root.agent_hook_usage.clone();
    }
    let usage_block = usage.map(|usage| {
        let percent = usage
            .used_percentage
            .unwrap_or_else(|| 100.0 * usage.used_tokens as f64 / usage.total_tokens as f64);
        let percent = format!("{percent:.0}");
        let used = usage.used_tokens.to_string();
        let total = usage.total_tokens.to_string();
        div()
            .pb_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .debug_selector(|| "settings-agent-usage".to_owned())
            .child(language.format(
                Message::SettingsAgentsContextUsage,
                &[("percent", &percent), ("used", &used), ("total", &total)],
            ))
            .into_any_element()
    });
    let confirmation_blocks = confirmations
        .into_iter()
        .map(|confirmation| {
            let slug = confirmation.hook.slug();
            let retry_root = view.root.clone();
            let cancel_root = view.root.clone();
            let commands = confirmation
                .commands
                .iter()
                .enumerate()
                .map(|(index, command)| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().foreground)
                        .debug_selector(move || {
                            format!("settings-agent-hook-confirm-command-{slug}-{index}")
                        })
                        .child(SharedString::from(command.clone()))
                })
                .collect::<Vec<_>>();
            let hook = confirmation.hook;
            div()
                .flex()
                .flex_col()
                .gap_1()
                .mb_2()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().danger)
                .debug_selector(move || format!("settings-agent-hook-confirmation-{slug}"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(language.text(Message::SettingsAgentsHookConfirmTitle)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentsHookConfirmHint)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(hook.label()),
                )
                .children(commands)
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "settings-agent-hook-confirm-retry-{slug}"
                                )))
                                .debug_selector(move || {
                                    format!("settings-agent-hook-confirm-retry-{slug}")
                                })
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().accent)
                                .child(language.text(Message::SettingsAgentsConfirmStatusline))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |_view, _, _, cx| {
                                        let Some(root_handle) = retry_root.upgrade() else {
                                            return;
                                        };
                                        root_handle.update(cx, |root, cx| {
                                            root.confirm_agent_hook_apply(hook, cx);
                                        });
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "settings-agent-hook-confirm-cancel-{slug}"
                                )))
                                .debug_selector(move || {
                                    format!("settings-agent-hook-confirm-cancel-{slug}")
                                })
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().foreground)
                                .child(language.text(Message::SettingsAgentsHookConfirmCancel))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |_view, _, _, cx| {
                                        let Some(root_handle) = cancel_root.upgrade() else {
                                            return;
                                        };
                                        root_handle.update(cx, |root, cx| {
                                            root.cancel_agent_hook_confirmation(hook, cx);
                                        });
                                    }),
                                ),
                        ),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();

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
    let roots_root = view.root.clone();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .id("settings-open-agent-roots")
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
                .debug_selector(|| "settings-open-agent-roots".to_owned())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_view, _, window, cx| {
                        let Some(root_handle) = roots_root.upgrade() else {
                            return;
                        };
                        root_handle.update(cx, |root, cx| {
                            if let Err(error) = root.open_agent_roots_dialog(window, cx) {
                                log::warn!("Agent 根对话框打开失败: {error:?}");
                            }
                        });
                    }),
                )
                .child(language.text(Message::SettingsAgentRootsOpen)),
        )
        .child(
            div()
                .pb_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentsDescription)),
        )
        .children(usage_block)
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
        .children(confirmation_blocks)
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
            let signal = crate::ai_hook::statusline_signal_path();
            let (integrations, usage) = cx
                .background_executor()
                .spawn(async move {
                    (
                        crate::ai_hook::integrations::inspect(),
                        crate::ai_hook::ContextUsage::read(&signal),
                    )
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.agent_integrations = Some(integrations);
                root.agent_hook_usage = usage;
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

    /// 多文件 hook 注入的 Journal 端口；无 writer 的测试根直接写。
    fn hook_journal_port(&self) -> Option<HookJournalPort> {
        let writer = self.project_writer.clone()?;
        let store_epoch = self.store_epoch.clone()?;
        Some(HookJournalPort::new(
            writer,
            store_epoch,
            crate::platform::dirs::local_dir().join(HOOK_JOURNAL_DIR),
        ))
    }

    /// 授权开关：显式设置值立即入队，安装/卸载在后台执行并回投结果。
    pub(super) fn set_agent_hook_enabled(
        &mut self,
        hook: AgentHook,
        enabled: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let restore_enabled = self
            .settings
            .values()
            .bool_on(hook.settings_key())
            .unwrap_or(false);
        if !self.settings.queue_explicit(
            &[(hook.settings_key(), Value::Bool(enabled))],
            Instant::now(),
        ) {
            return;
        }
        // 同一 agent 的新开关意图取代其尚未确认的旧确认面。
        self.agent_hook_confirmations
            .retain(|confirmation| confirmation.hook != hook);
        let journal = self.hook_journal_port();
        #[cfg(test)]
        if let Some(outcome) = self.agent_hook_outcome_for_test {
            let result = outcome(hook, enabled);
            self.finish_agent_hook_apply(hook, restore_enabled, result, journal, cx);
            return;
        }
        #[cfg(test)]
        if let Some(apply) = self.agent_hook_apply_for_test {
            let result = apply(hook, enabled).map(|()| ApplyOutcome::Applied);
            self.finish_agent_hook_apply(hook, restore_enabled, result, journal, cx);
            return;
        }
        let journal_for_finish = journal.clone();
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                crate::ai_hook::integrations::apply_enabled_outcome(hook, enabled, journal)
            });
            let result = result.await;
            let _ = this.update(cx, |root, cx| {
                root.finish_agent_hook_apply(hook, restore_enabled, result, journal_for_finish, cx);
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::AgentHookApply(hook), cancellation, task)
        {
            log::warn!("agent hook 安装任务注册失败: {error:?}");
        }
    }

    /// 安装/卸载回执：已应用重新探测状态；命中审查门只保留待确认原文。
    fn finish_agent_hook_apply(
        &mut self,
        hook: AgentHook,
        restore_enabled: bool,
        result: Result<ApplyOutcome, String>,
        journal: Option<HookJournalPort>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::AgentHookApply(hook));
        match result {
            Ok(ApplyOutcome::Applied) => {
                self.agent_hooks_error = false;
                self.agent_hook_confirmations
                    .retain(|confirmation| confirmation.hook != hook);
                self.request_agent_hooks_inspect(cx);
                self.refresh_user_hooks_after_install(cx);
            }
            Ok(ApplyOutcome::NeedsConfirmation(commands)) => {
                self.agent_hooks_error = false;
                self.agent_hook_confirmations
                    .retain(|confirmation| confirmation.hook != hook);
                self.agent_hook_confirmations.push(AgentHookConfirmation {
                    hook,
                    commands,
                    restore_enabled,
                    journal,
                });
            }
            Err(_) => {
                self.agent_hooks_error = true;
                self.agent_hook_confirmations
                    .retain(|confirmation| confirmation.hook != hook);
            }
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    /// 用户已确认原命令：经恒放行审查门的通道重试一次安装。
    pub(super) fn confirm_agent_hook_apply(&mut self, hook: AgentHook, cx: &mut Context<'_, Self>) {
        let Some(index) = self
            .agent_hook_confirmations
            .iter()
            .position(|confirmation| confirmation.hook == hook)
        else {
            return;
        };
        let confirmation = self.agent_hook_confirmations.remove(index);
        #[cfg(test)]
        if let Some(confirm) = self.agent_hook_confirm_for_test {
            let result = confirm(hook);
            self.finish_agent_hook_apply_confirmed(hook, result, cx);
            return;
        }
        let journal = confirmation.journal.clone();
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                crate::ai_hook::integrations::apply_enabled_confirmed(hook, true, journal.as_ref())
            });
            let result = result.await;
            let _ = this.update(cx, |root, cx| {
                root.finish_agent_hook_apply_confirmed(hook, result, cx);
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::AgentHookApply(hook), cancellation, task)
        {
            // 登记失败时恢复确认面，避免用户已确认的命令无入口可用。
            self.agent_hook_confirmations.push(confirmation);
            self.agent_hooks_error = true;
            log::warn!("agent hook 确认安装任务注册失败: {error:?}");
            cx.notify();
        }
    }

    /// 确认后重试回执：成功重新探测，失败只保留脱敏标记。
    fn finish_agent_hook_apply_confirmed(
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
                self.refresh_user_hooks_after_install(cx);
            }
            Err(_) => self.agent_hooks_error = true,
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    /// 取消确认：零安装，设置值回退到命中审查门之前。
    pub(super) fn cancel_agent_hook_confirmation(
        &mut self,
        hook: AgentHook,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(index) = self
            .agent_hook_confirmations
            .iter()
            .position(|confirmation| confirmation.hook == hook)
        else {
            return;
        };
        let confirmation = self.agent_hook_confirmations.remove(index);
        self.settings.queue_explicit(
            &[(
                confirmation.hook.settings_key(),
                Value::Bool(confirmation.restore_enabled),
            )],
            Instant::now(),
        );
        self.agent_hooks_error = false;
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

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn set_agent_hook_usage_for_test(
        &mut self,
        usage: Option<crate::ai_hook::ContextUsage>,
    ) {
        self.agent_hook_usage = usage;
    }

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn replace_agent_hook_confirmation_for_test(
        &mut self,
        outcome: fn(AgentHook, bool) -> Result<ApplyOutcome, String>,
        confirm: fn(AgentHook) -> Result<(), String>,
    ) {
        self.agent_hook_outcome_for_test = Some(outcome);
        self.agent_hook_confirm_for_test = Some(confirm);
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
