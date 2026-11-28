//! 设置页 Agent 基础配置：展示过滤/名单/别名/运行时清单与 Hooks 授权开关。
//!
//! 配置读取、命名空间校验与设置写盘都在后台；Hooks 探测与安装/卸载同为
//! 阻塞 I/O，前台只读取快照、提交显式设置值并展示脱敏状态。

use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};
use slterm_settings::json::Value;
use slterm_settings::{AgentCliConfig, AgentDisplayMode, AgentFilesConfig, AgentHook, AgentRootId};

use crate::ai_agents::AgentKind;

use super::AppRoot;
use super::settings_overlay::SettingsView;
use super::task_service::{CancellationToken, TaskId};
use crate::ai_hook::integrations::{AgentIntegration, ApplyOutcome, HookInspection, agents};
use crate::ai_hook::{HOOK_JOURNAL_DIR, HookCommitState, HookJournalPort};
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

/// AgentHooks 状态区：每 agent 一行状态与授权开关，另加刷新入口。
fn render_agent_hook_controls(
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
    let mut cleanup_failures = Vec::new();
    let mut usage = None;
    if let Some(root) = root_entity.as_ref() {
        let root = root.read(cx);
        integrations = root.agent_integrations.clone();
        inspecting = root.agent_hooks_inspecting;
        failed = root.agent_hooks_error;
        enabled = agent_row_states(root.settings.values());
        confirmations = root.agent_hook_confirmations.clone();
        cleanup_failures = root.agent_hook_cleanup_failures.iter().copied().collect();
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
    let cleanup_blocks = cleanup_failures
        .into_iter()
        .map(|hook| {
            let slug = hook.slug();
            let retry_root = view.root.clone();
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .mb_2()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().warning)
                .debug_selector(move || format!("settings-agent-hook-cleanup-{slug}"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child(language.text(Message::SettingsAgentsCleanupPending)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(hook.label()),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "settings-agent-hook-cleanup-retry-{slug}"
                        )))
                        .debug_selector(move || format!("settings-agent-hook-cleanup-retry-{slug}"))
                        .text_xs()
                        .cursor_pointer()
                        .text_color(cx.theme().accent)
                        .child(language.text(Message::SettingsAgentsCleanupRetry))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_view, _, _, cx| {
                                let Some(root_handle) = retry_root.upgrade() else {
                                    return;
                                };
                                root_handle.update(cx, |root, cx| {
                                    root.retry_agent_hook_cleanup(hook, cx);
                                });
                            }),
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
        .children(cleanup_blocks)
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
            let result =
                apply(hook, enabled).map(|()| ApplyOutcome::Applied(HookCommitState::Committed));
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
            Ok(ApplyOutcome::Applied(state)) => {
                self.agent_hooks_error = false;
                self.agent_hook_confirmations
                    .retain(|confirmation| confirmation.hook != hook);
                self.record_hook_commit_state(hook, state);
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
        result: Result<HookCommitState, String>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::AgentHookApply(hook));
        match result {
            Ok(state) => {
                self.agent_hooks_error = false;
                self.record_hook_commit_state(hook, state);
                self.request_agent_hooks_inspect(cx);
                self.refresh_user_hooks_after_install(cx);
            }
            Err(_) => self.agent_hooks_error = true,
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    fn record_hook_commit_state(&mut self, hook: AgentHook, state: HookCommitState) {
        if state == HookCommitState::CommittedCleanupFailed {
            self.agent_hook_cleanup_failures.insert(hook);
        } else {
            self.agent_hook_cleanup_failures.remove(&hook);
        }
    }

    /// 已提交但材料清理失败：只重试清理，不重写配置。
    pub(super) fn retry_agent_hook_cleanup(&mut self, hook: AgentHook, cx: &mut Context<'_, Self>) {
        #[cfg(test)]
        if let Some(cleanup) = self.agent_hook_cleanup_for_test {
            if cleanup(hook) {
                self.agent_hook_cleanup_failures.remove(&hook);
            }
            self.reload_settings_snapshot(cx);
            cx.notify();
            return;
        }
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let clean = cx
                .background_executor()
                .spawn(async move { crate::ai_hook::integrations::retry_cleanup(hook) })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::AgentHookCleanup(hook));
                if clean {
                    root.agent_hook_cleanup_failures.remove(&hook);
                }
                root.reload_settings_snapshot(cx);
                cx.notify();
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::AgentHookCleanup(hook), cancellation, task)
        {
            log::warn!("agent hook 清理重试任务注册失败: {error:?}");
        }
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
        confirm: fn(AgentHook) -> Result<HookCommitState, String>,
    ) {
        self.agent_hook_outcome_for_test = Some(outcome);
        self.agent_hook_confirm_for_test = Some(confirm);
    }

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn set_agent_hook_cleanup_for_test(&mut self, cleanup: fn(AgentHook) -> bool) {
        self.agent_hook_cleanup_for_test = Some(cleanup);
    }
}

/// Agent 基础配置提交失败分类；前台只经封闭文案展示。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentFilesConfigError {
    /// 设置段读取失败；保留原文件并展示默认值。
    Read,
    /// 名称／别名形状或段结构非法。
    Invalid,
    /// 自定义名单内重复。
    DuplicateName,
    /// 别名与内置命令或其他 CLI 冲突。
    DuplicateAlias,
    /// 统一设置写盘失败。
    Save,
}

/// 测试注入的 Agent 基础配置保存端口；生产走统一设置写通道。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) type AgentFilesConfigSaveHook =
    std::sync::Arc<dyn Fn(&AgentFilesConfig) -> std::io::Result<()> + Send + Sync>;

/// 生产读取 `agent_files` 段；IO 与解码失败都显式返回。
pub(super) fn load_agent_files_config() -> Result<AgentFilesConfig, AgentFilesConfigError> {
    let loaded = crate::settings_store::load_settings().map_err(|_| AgentFilesConfigError::Read)?;
    AgentFilesConfig::from_segment(
        loaded
            .data
            .external_segment(slterm_settings::AGENT_FILES_SEGMENT),
    )
    .map_err(|_| AgentFilesConfigError::Read)
}

/// 生产保存：域外段整段替换，经统一设置写通道（白名单、锁、原子写、备份）。
fn save_agent_files_config(config: &AgentFilesConfig) -> std::io::Result<()> {
    crate::settings_store::save_settings_blocking(&[(
        slterm_settings::AGENT_FILES_SEGMENT,
        config.to_segment(),
    )])
}

/// 候选配置校验：名单/别名形状与唯一性，别名扣除全部内置命令命名空间。
fn validate_agent_files_candidate(config: &AgentFilesConfig) -> Result<(), AgentFilesConfigError> {
    let mut namespace = std::collections::BTreeMap::new();
    for kind in AgentKind::ALL {
        for name in std::iter::once(kind.slug())
            .chain(kind.aliases().iter().copied())
            .chain(std::iter::once(kind.catalog().builtin_command))
        {
            namespace.insert(name.trim().to_lowercase(), ());
        }
    }
    for kind in AgentKind::ALL {
        let Some(cli) = config.cli(kind.slug()) else {
            continue;
        };
        match slterm_settings::validate_agent_name_list(&cli.custom_names) {
            Ok(()) => {}
            Err(slterm_settings::AgentNameError::Duplicate) => {
                return Err(AgentFilesConfigError::DuplicateName);
            }
            Err(_) => return Err(AgentFilesConfigError::Invalid),
        }
        match slterm_settings::validate_agent_alias_list(&cli.aliases) {
            Ok(()) => {}
            Err(slterm_settings::AgentAliasError::Duplicate) => {
                return Err(AgentFilesConfigError::DuplicateAlias);
            }
            Err(_) => return Err(AgentFilesConfigError::Invalid),
        }
        for alias in &cli.aliases {
            if namespace.insert(alias.trim().to_lowercase(), ()).is_some() {
                return Err(AgentFilesConfigError::DuplicateAlias);
            }
        }
    }
    for (_, names) in config.runtime_entries() {
        match slterm_settings::validate_agent_name_list(names) {
            Ok(()) => {}
            Err(slterm_settings::AgentNameError::Duplicate) => {
                return Err(AgentFilesConfigError::DuplicateName);
            }
            Err(_) => return Err(AgentFilesConfigError::Invalid),
        }
    }
    Ok(())
}

fn agent_files_error_message(error: AgentFilesConfigError) -> Message {
    match error {
        AgentFilesConfigError::Read => Message::SettingsAgentFilesLoadFailed,
        AgentFilesConfigError::DuplicateName => Message::SettingsAgentFilesDuplicateName,
        AgentFilesConfigError::DuplicateAlias => Message::SettingsAgentFilesDuplicateAlias,
        AgentFilesConfigError::Invalid | AgentFilesConfigError::Save => {
            Message::SettingsAgentFilesSaveFailed
        }
    }
}

impl AppRoot {
    /// 首帧后加载 `agent_files` 段；测试根已标记加载时不触碰真实设置文件。
    pub(super) fn ensure_agent_files_config_loading(&mut self, cx: &mut Context<'_, Self>) {
        if self.agent_files_config_loaded || self.tasks.is_registered(TaskId::AgentFilesConfigLoad)
        {
            return;
        }
        let executor = cx.background_executor().clone();
        self.tasks
            .register_with(
                TaskId::AgentFilesConfigLoad,
                CancellationToken::new(),
                move || {
                    let loading = executor.spawn(async move { load_agent_files_config() });
                    cx.spawn(async move |this, cx| {
                        let result = loading.await;
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(TaskId::AgentFilesConfigLoad);
                            // 注入或更早完成的配置优先，迟到回投不得覆盖。
                            if root.agent_files_config_loaded {
                                return;
                            }
                            root.agent_files_config_loaded = true;
                            match result {
                                Ok(config) => {
                                    root.agent_files_config = config;
                                    root.agent_files_config_error = None;
                                }
                                Err(error) => {
                                    root.agent_files_config_error = Some(error);
                                }
                            }
                            root.sync_agent_files_roots_after_load(cx);
                            cx.notify();
                        });
                    })
                },
            )
            .expect("AgentFilesConfigLoad 单任务登记必须唯一");
    }

    pub(super) fn agent_files_config_error(&self) -> Option<AgentFilesConfigError> {
        self.agent_files_config_error
    }

    /// 基础配置候选提交：后台命名空间校验 + 统一设置写盘，成功才采纳运行值。
    pub(super) fn commit_agent_files_config(
        &mut self,
        candidate: AgentFilesConfig,
        cx: &mut Context<'_, Self>,
    ) {
        // 未加载的配置快照是默认值：先完成读取，避免用默认值覆盖既有条目。
        if !self.agent_files_config_loaded {
            self.ensure_agent_files_config_loading(cx);
            return;
        }
        if self.agent_files_config_saving {
            return;
        }
        self.agent_files_config_saving = true;
        self.agent_files_config_error = None;
        cx.notify();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let save_override = self.agent_files_save_for_test.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    validate_agent_files_candidate(&candidate)?;
                    let saved = {
                        #[cfg(all(test, feature = "gpui-test-support"))]
                        {
                            if let Some(save) = save_override {
                                save(&candidate)
                            } else {
                                save_agent_files_config(&candidate)
                            }
                        }
                        #[cfg(not(all(test, feature = "gpui-test-support")))]
                        {
                            save_agent_files_config(&candidate)
                        }
                    };
                    saved.map_err(|error| {
                        log::warn!("Agent 配置保存失败: {error}");
                        AgentFilesConfigError::Save
                    })?;
                    Ok::<AgentFilesConfig, AgentFilesConfigError>(candidate)
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.finish_agent_files_config_commit(result, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::AgentFilesConfigSave, CancellationToken::new(), task)
        {
            self.agent_files_config_saving = false;
            log::warn!("Agent 配置保存任务注册失败: {error:?}");
        }
    }

    fn finish_agent_files_config_commit(
        &mut self,
        result: Result<AgentFilesConfig, AgentFilesConfigError>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::AgentFilesConfigSave);
        self.agent_files_config_saving = false;
        match result {
            Ok(config) => {
                self.agent_files_config = config;
                self.agent_files_config_error = None;
                self.agent_files_config_loaded = true;
                self.sync_agent_files_roots_after_load(cx);
            }
            Err(error) => self.agent_files_config_error = Some(error),
        }
        cx.notify();
    }

    /// 页级恢复默认：只复位当前 CLI 展示配置与它自己的内置根清单。
    pub(super) fn reset_agent_basic_page(
        &mut self,
        slug: &str,
        builtin_root: AgentRootId,
        cx: &mut Context<'_, Self>,
    ) {
        let mut candidate = self.agent_files_config.clone();
        candidate.reset_cli(slug);
        candidate.reset_runtime_names(builtin_root);
        self.commit_agent_files_config(candidate, cx);
    }
}

/// Agent 基础配置页：AgentHooks 状态区 + 按 CLI 的展示、名单、别名与运行时清单。
pub(super) fn render_agent_basic_page(
    view: &mut SettingsView,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let basic = render_agent_basic_config(view, window, cx);
    let hooks = render_agent_hook_controls(view, cx);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(basic)
        .child(hooks)
        .into_any_element()
}

fn render_agent_basic_config(
    view: &mut SettingsView,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    view.ensure_agent_basic_inputs(window, cx);
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let selected = view.agent_cli_slug();
    let kind = AgentKind::ALL
        .into_iter()
        .find(|kind| kind.slug() == selected)
        .unwrap_or(AgentKind::Claude);
    let root_handle = view.root.upgrade();
    let (config, saving, error) = root_handle
        .as_ref()
        .map(|root| {
            let root = root.read(cx);
            (
                root.agent_files_config.clone(),
                root.agent_files_config_saving || !root.agent_files_config_loaded,
                root.agent_files_config_error(),
            )
        })
        .unwrap_or_default();
    let cli_config = config.cli(kind.slug()).cloned().unwrap_or_default();
    let builtin_root = kind.builtin_root_id();
    let runtime_names = config.runtime_names(builtin_root).to_vec();

    let cli_pills = AgentKind::ALL.into_iter().map(|kind| {
        let slug = kind.slug();
        let is_selected = slug == selected;
        div()
            .id(gpui::ElementId::Name(
                format!("settings-agent-cli-{slug}").into(),
            ))
            .debug_selector(move || format!("settings-agent-cli-{slug}"))
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
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, window, cx| {
                    view.select_agent_cli(slug, window, cx);
                }),
            )
            .child(kind.display_name())
    });

    let mode_pill = |mode: AgentDisplayMode, label: Message, id: &'static str| {
        let is_selected = cli_config.display_mode == mode;
        let root = view.root.clone();
        div()
            .id(gpui::ElementId::Name(
                format!("settings-agent-display-{id}").into(),
            ))
            .debug_selector(move || format!("settings-agent-display-{id}"))
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
            .cursor_pointer()
            .child(language.text(label))
            .on_mouse_down(MouseButton::Left, {
                let slug = kind.slug();
                let candidate = config.clone();
                let mut next = cli_config.clone();
                next.display_mode = mode;
                move |_, _, cx| {
                    let Some(root) = root.upgrade() else {
                        return;
                    };
                    let mut update = candidate.clone();
                    update.set_cli(slug, next.clone());
                    root.update(cx, |root, cx| {
                        root.commit_agent_files_config(update, cx);
                    });
                }
            })
    };

    let runtime_toggle = {
        let enabled = cli_config.show_runtime_files;
        let root = view.root.clone();
        let candidate = config.clone();
        let slug = kind.slug();
        let mut next = cli_config.clone();
        next.show_runtime_files = !enabled;
        let custom_mode = cli_config.display_mode == AgentDisplayMode::Custom;
        div()
            .id("settings-agent-show-runtime")
            .debug_selector(|| "settings-agent-show-runtime".to_owned())
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .when(custom_mode, |element| element.opacity(0.5))
            .child(if enabled { "✓" } else { "–" })
            .child(language.text(Message::SettingsAgentFilesShowRuntimeFiles))
            // 自定义模式忽略运行时开关，但保留其值；禁用点击避免误以为生效。
            .when(!custom_mode, |element| {
                element
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let Some(root) = root.upgrade() else {
                            return;
                        };
                        let mut update = candidate.clone();
                        update.set_cli(slug, next.clone());
                        root.update(cx, |root, cx| {
                            root.commit_agent_files_config(update, cx);
                        });
                    })
            })
    };

    let custom_input = view
        .agent_custom_input
        .as_ref()
        .map(|input| Input::new(input).into_any_element());
    let alias_input = view
        .agent_alias_input
        .as_ref()
        .map(|input| Input::new(input).into_any_element());
    let runtime_input = view
        .agent_runtime_input
        .as_ref()
        .map(|input| Input::new(input).into_any_element());

    let name_chip = |name: &str,
                     id: String,
                     action: fn(
        &mut SettingsView,
        String,
        &mut Window,
        &mut Context<'_, SettingsView>,
    )| {
        let label = name.to_owned();
        let root_action = action;
        let outer_id = id.clone();
        let inner_id = id.clone();
        div()
            .id(gpui::ElementId::Name(outer_id.clone().into()))
            .debug_selector(move || outer_id.clone())
            .px_2()
            .h(px(20.0))
            .flex()
            .items_center()
            .gap_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().foreground)
            .child(name.to_owned())
            .child(
                div()
                    .id(gpui::ElementId::Name(format!("{inner_id}-remove").into()))
                    .debug_selector(move || format!("{inner_id}-remove"))
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .child("×")
                    .on_mouse_down(MouseButton::Left, {
                        let label = label.clone();
                        cx.listener(move |view, _, window, cx| {
                            cx.stop_propagation();
                            root_action(view, label.clone(), window, cx);
                        })
                    }),
            )
    };

    let mut name_chips = div().flex().flex_wrap().gap_1();
    for (index, name) in cli_config.custom_names.iter().enumerate() {
        name_chips = name_chips.child(name_chip(
            name,
            format!("settings-agent-custom-chip-{index}"),
            SettingsView::agent_remove_custom_name,
        ));
    }
    if cli_config.custom_names.is_empty() {
        name_chips = name_chips.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentFilesEmpty)),
        );
    }
    let mut alias_chips = div().flex().flex_wrap().gap_1();
    for (index, alias) in cli_config.aliases.iter().enumerate() {
        alias_chips = alias_chips.child(name_chip(
            alias,
            format!("settings-agent-alias-chip-{index}"),
            SettingsView::agent_remove_alias,
        ));
    }
    if cli_config.aliases.is_empty() {
        alias_chips = alias_chips.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentFilesEmpty)),
        );
    }
    let mut runtime_chips = div().flex().flex_wrap().gap_1();
    for (index, name) in runtime_names.iter().enumerate() {
        runtime_chips = runtime_chips.child(name_chip(
            name,
            format!("settings-agent-runtime-chip-{index}"),
            SettingsView::agent_remove_runtime_name,
        ));
    }
    if runtime_names.is_empty() {
        runtime_chips = runtime_chips.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentFilesEmpty)),
        );
    }

    let add_button =
        |id: &'static str,
         label: Message,
         action: fn(&mut SettingsView, &mut Window, &mut Context<'_, SettingsView>)| {
            div()
                .id(gpui::ElementId::Name(id.into()))
                .debug_selector(move || id.to_owned())
                .px_2()
                .h(px(22.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .text_xs()
                .cursor_pointer()
                .text_color(cx.theme().accent)
                .child(language.text(label))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| action(view, window, cx)),
                )
        };

    let error_block = view.agent_basic_error.map(|message| {
        div()
            .debug_selector(|| "settings-agent-basic-error".to_owned())
            .text_xs()
            .text_color(cx.theme().danger)
            .child(language.text(message))
    });
    let persisted_error = error.map(|error| {
        div()
            .debug_selector(|| "settings-agent-files-error".to_owned())
            .text_xs()
            .text_color(cx.theme().danger)
            .child(language.text(agent_files_error_message(error)))
    });
    let saving_block = saving.then(|| {
        div()
            .debug_selector(|| "settings-agent-files-saving".to_owned())
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(language.text(Message::SettingsAgentFilesSaving))
    });

    div()
        .flex()
        .flex_col()
        .gap_2()
        .pt_3()
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(language.text(Message::SettingsAgentFilesTitle)),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .debug_selector(|| "settings-agent-cli-list".to_owned())
                .children(cli_pills),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentFilesDisplayMode)),
                )
                .child(mode_pill(
                    AgentDisplayMode::All,
                    Message::SettingsAgentFilesModeAll,
                    "all",
                ))
                .child(mode_pill(
                    AgentDisplayMode::Custom,
                    Message::SettingsAgentFilesModeCustom,
                    "custom",
                ))
                .child(runtime_toggle),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentFilesCustomNamesHint)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .children(custom_input)
                        .child(add_button(
                            "settings-agent-custom-add",
                            Message::SettingsAgentFilesAdd,
                            SettingsView::agent_add_custom_name,
                        )),
                )
                .child(name_chips),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsAgentFilesAliasesHint)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .children(alias_input)
                        .child(add_button(
                            "settings-agent-alias-add",
                            Message::SettingsAgentFilesAdd,
                            SettingsView::agent_add_alias,
                        )),
                )
                .child(alias_chips),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text(Message::SettingsAgentFilesRuntimeHint)),
                        )
                        .child(
                            div()
                                .id("settings-agent-runtime-reset")
                                .debug_selector(|| "settings-agent-runtime-reset".to_owned())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text(Message::SettingsAgentFilesResetRuntime))
                                .on_mouse_down(MouseButton::Left, {
                                    let root = view.root.clone();
                                    let base = config.clone();
                                    move |_, _, cx| {
                                        let Some(root) = root.upgrade() else {
                                            return;
                                        };
                                        let mut update = base.clone();
                                        update.reset_runtime_names(builtin_root);
                                        root.update(cx, |root, cx| {
                                            root.commit_agent_files_config(update, cx);
                                        });
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .children(runtime_input)
                        .child(add_button(
                            "settings-agent-runtime-add",
                            Message::SettingsAgentFilesAdd,
                            SettingsView::agent_add_runtime_name,
                        )),
                )
                .child(runtime_chips),
        )
        .children(error_block)
        .children(persisted_error)
        .children(saving_block)
        .into_any_element()
}

impl SettingsView {
    /// 当前 Agent 基础页 CLI；未选择时回退 catalog 首个已注册项。
    pub(super) fn agent_cli_slug(&self) -> &'static str {
        self.agent_cli
            .filter(|slug| AgentKind::ALL.iter().any(|kind| kind.slug() == *slug))
            .unwrap_or_else(|| AgentKind::ALL[0].slug())
    }

    pub(super) fn select_agent_cli(
        &mut self,
        slug: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.agent_cli == Some(slug) {
            return;
        }
        self.agent_cli = Some(slug);
        self.agent_basic_error = None;
        self.clear_agent_basic_inputs(window, cx);
        cx.notify();
    }

    fn clear_agent_basic_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        for input in [
            self.agent_custom_input.as_ref(),
            self.agent_alias_input.as_ref(),
            self.agent_runtime_input.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
    }

    /// 首次绘制时创建输入实体；占位文案随语言快照固定。
    pub(super) fn ensure_agent_basic_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.agent_custom_input.is_some()
            && self.agent_alias_input.is_some()
            && self.agent_runtime_input.is_some()
        {
            return;
        }
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let name_placeholder = language.text(Message::SettingsAgentFilesNamePlaceholder);
        let alias_placeholder = language.text(Message::SettingsAgentFilesAliasPlaceholder);
        if self.agent_custom_input.is_none() {
            self.agent_custom_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder(name_placeholder)));
        }
        if self.agent_alias_input.is_none() {
            self.agent_alias_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder(alias_placeholder)));
        }
        if self.agent_runtime_input.is_none() {
            self.agent_runtime_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder(name_placeholder)));
        }
    }

    /// 配置读取完成且没有在途写盘时才接受新编辑，避免静默丢弃输入。
    fn agent_files_config_ready(&self, cx: &Context<'_, Self>) -> bool {
        self.root.upgrade().is_some_and(|root| {
            let root = root.read(cx);
            root.agent_files_config_loaded && !root.agent_files_config_saving
        })
    }

    fn agent_files_config_handle(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> Option<(AgentKind, AgentFilesConfig, AgentCliConfig)> {
        let root = self.root.upgrade()?;
        let kind = AgentKind::ALL
            .into_iter()
            .find(|kind| kind.slug() == self.agent_cli_slug())
            .unwrap_or(AgentKind::Claude);
        let config = root.read(cx).agent_files_config.clone();
        let cli = config.cli(kind.slug()).cloned().unwrap_or_default();
        Some((kind, config, cli))
    }

    fn commit_agent_cli_config(
        &mut self,
        kind: AgentKind,
        config: AgentFilesConfig,
        cli: AgentCliConfig,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let mut candidate = config;
        candidate.set_cli(kind.slug(), cli);
        root.update(cx, |root, cx| {
            root.commit_agent_files_config(candidate, cx);
        });
    }

    pub(super) fn agent_add_custom_name(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(input) = self.agent_custom_input.clone() else {
            return;
        };
        if !self.agent_files_config_ready(cx) {
            self.agent_basic_error = Some(Message::SettingsAgentFilesSaving);
            cx.notify();
            return;
        }
        let name = input.read(cx).value().trim().to_owned();
        if slterm_settings::validate_agent_name(&name).is_err() {
            self.agent_basic_error = Some(Message::SettingsAgentFilesInvalidName);
            cx.notify();
            return;
        }
        let Some((kind, config, mut cli)) = self.agent_files_config_handle(cx) else {
            return;
        };
        if slterm_settings::validate_agent_name_list(
            &cli.custom_names
                .iter()
                .cloned()
                .chain(std::iter::once(name.clone()))
                .collect::<Vec<_>>(),
        )
        .is_err()
        {
            self.agent_basic_error = Some(Message::SettingsAgentFilesDuplicateName);
            cx.notify();
            return;
        }
        cli.custom_names.push(name);
        self.agent_basic_error = None;
        input.update(cx, |state, cx| state.set_value("", window, cx));
        self.commit_agent_cli_config(kind, config, cli, cx);
    }

    pub(super) fn agent_remove_custom_name(
        &mut self,
        name: String,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((kind, config, mut cli)) = self.agent_files_config_handle(cx) else {
            return;
        };
        cli.custom_names.retain(|candidate| candidate != &name);
        self.commit_agent_cli_config(kind, config, cli, cx);
    }

    pub(super) fn agent_add_alias(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(input) = self.agent_alias_input.clone() else {
            return;
        };
        if !self.agent_files_config_ready(cx) {
            self.agent_basic_error = Some(Message::SettingsAgentFilesSaving);
            cx.notify();
            return;
        }
        let alias = input.read(cx).value().trim().to_owned();
        if slterm_settings::validate_agent_alias(&alias).is_err() {
            self.agent_basic_error = Some(Message::SettingsAgentFilesInvalidAlias);
            cx.notify();
            return;
        }
        let Some((kind, config, mut cli)) = self.agent_files_config_handle(cx) else {
            return;
        };
        if slterm_settings::validate_agent_alias_list(
            &cli.aliases
                .iter()
                .cloned()
                .chain(std::iter::once(alias.clone()))
                .collect::<Vec<_>>(),
        )
        .is_err()
        {
            self.agent_basic_error = Some(Message::SettingsAgentFilesDuplicateAlias);
            cx.notify();
            return;
        }
        cli.aliases.push(alias);
        self.agent_basic_error = None;
        input.update(cx, |state, cx| state.set_value("", window, cx));
        self.commit_agent_cli_config(kind, config, cli, cx);
    }

    pub(super) fn agent_remove_alias(
        &mut self,
        alias: String,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((kind, config, mut cli)) = self.agent_files_config_handle(cx) else {
            return;
        };
        cli.aliases.retain(|candidate| candidate != &alias);
        self.commit_agent_cli_config(kind, config, cli, cx);
    }

    pub(super) fn agent_add_runtime_name(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(input) = self.agent_runtime_input.clone() else {
            return;
        };
        if !self.agent_files_config_ready(cx) {
            self.agent_basic_error = Some(Message::SettingsAgentFilesSaving);
            cx.notify();
            return;
        }
        let name = input.read(cx).value().trim().to_owned();
        if slterm_settings::validate_agent_name(&name).is_err() {
            self.agent_basic_error = Some(Message::SettingsAgentFilesInvalidName);
            cx.notify();
            return;
        }
        let Some((kind, config, _)) = self.agent_files_config_handle(cx) else {
            return;
        };
        let root_id = kind.builtin_root_id();
        let mut names = config.runtime_names(root_id).to_vec();
        names.push(name);
        if slterm_settings::validate_agent_name_list(&names).is_err() {
            self.agent_basic_error = Some(Message::SettingsAgentFilesDuplicateName);
            cx.notify();
            return;
        }
        self.agent_basic_error = None;
        input.update(cx, |state, cx| state.set_value("", window, cx));
        let mut candidate = config;
        candidate.set_runtime_names(root_id, names);
        let Some(root) = self.root.upgrade() else {
            return;
        };
        root.update(cx, |root, cx| {
            root.commit_agent_files_config(candidate, cx);
        });
    }

    pub(super) fn agent_remove_runtime_name(
        &mut self,
        name: String,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((kind, config, _)) = self.agent_files_config_handle(cx) else {
            return;
        };
        let root_id = kind.builtin_root_id();
        let mut names = config.runtime_names(root_id).to_vec();
        names.retain(|candidate| candidate != &name);
        let mut candidate = config;
        candidate.set_runtime_names(root_id, names);
        let Some(root) = self.root.upgrade() else {
            return;
        };
        root.update(cx, |root, cx| {
            root.commit_agent_files_config(candidate, cx);
        });
    }

    /// 页级恢复默认：当前 CLI 展示配置与内置根运行时清单；自定义根不受影响。
    pub(super) fn reset_agent_basic_page(&mut self, cx: &mut Context<'_, Self>) {
        let Some(root) = self.root.upgrade() else {
            return;
        };
        let kind = AgentKind::ALL
            .into_iter()
            .find(|kind| kind.slug() == self.agent_cli_slug())
            .unwrap_or(AgentKind::Claude);
        let slug = kind.slug();
        let builtin_root = kind.builtin_root_id();
        root.update(cx, |root, cx| {
            root.reset_agent_basic_page(slug, builtin_root, cx);
        });
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
