//! Agent 会话恢复：就绪信号或有界超时后，每个 PTY 代次只注入一次。

use gpui::Context;
use slterm_agents::AgentSession;

use super::TerminalView;
use crate::app_shell::app_root::navigation::{
    restore_injection_timeout, restore_shell_from_program,
};
use crate::app_shell::config::Settings;

/// 显式 Agent 命令的注入结局；取消或销毁不属于两者，回执方按结果不明处理。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentCommandOutcome {
    Injected,
    Failed,
}

/// 只回传成功或失败；丢弃回调表示无法证明注入结果。
pub(crate) type AgentCommandCompletion = Box<dyn FnOnce(AgentCommandOutcome) + Send>;

pub(crate) struct PendingResume {
    pub(super) command: String,
    completion: Option<AgentCommandCompletion>,
    /// 丢弃即取消；提示符提前就绪时不保留迟到写入。
    _timeout: gpui::Task<()>,
}

impl TerminalView {
    /// 只有恢复与显式重连调用；普通新建不从历史状态派生命令。
    pub(crate) fn schedule_agent_resume(
        &mut self,
        agent: Option<&AgentSession>,
        cx: &mut Context<'_, Self>,
    ) {
        if !cx
            .try_global::<Settings>()
            .is_none_or(|settings| settings.resume_ai)
        {
            return;
        }
        let Some(command) = agent.and_then(AgentSession::resume_command) else {
            return;
        };
        self.schedule_agent_command(command, None, cx);
    }

    /// 显式 Agent 启动／恢复／分支注入：不读 `resume_ai`，提示符或超时后只执行一次。
    ///
    /// 命令必须由 AgentCatalog 构造并校验；退出、关闭、重连替换与 View
    /// 销毁都会取消待注入任务，迟到回执不补写。显式调用方经 completion
    /// 等待真实注入落定；恢复调用不需要回执。
    pub(crate) fn schedule_agent_command(
        &mut self,
        command: String,
        completion: Option<AgentCommandCompletion>,
        cx: &mut Context<'_, Self>,
    ) {
        // 被替换的旧回执不再兑现，等待方按结果不明处理。
        self.pending_resume = None;
        if self.exited.is_some() {
            if let Some(completion) = completion {
                completion(AgentCommandOutcome::Failed);
            }
            return;
        }
        let delay = restore_injection_timeout(restore_shell_from_program(
            self.exec_context
                .as_ref()
                .and_then(crate::completion::PaneExecContext::shell_program),
        ));
        let executor = cx.background_executor().clone();
        let timeout = cx.spawn(async move |this, cx| {
            executor.timer(delay).await;
            let _ = this.update(cx, |view, cx| {
                view.inject_agent_resume();
                cx.notify();
            });
        });
        self.pending_resume = Some(PendingResume {
            command,
            completion,
            _timeout: timeout,
        });
        self.inject_agent_resume_if_ready();
    }

    /// 不可变帧中的真实提示符边界是唯一提前就绪证据。
    pub(super) fn inject_agent_resume_if_ready(&mut self) {
        if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.prompt().is_some())
        {
            self.inject_agent_resume();
        }
    }

    fn inject_agent_resume(&mut self) {
        let Some(pending) = self.pending_resume.take() else {
            return;
        };
        let PendingResume {
            command,
            mut completion,
            _timeout,
        } = pending;
        if self.exited.is_some() {
            if let Some(completion) = completion.take() {
                completion(AgentCommandOutcome::Failed);
            }
            return;
        }
        let mut command = command.into_bytes();
        command.push(b'\r');
        let sent = self
            .send_worker_command(slterm_terminal::event_loop::Msg::Input(
                std::borrow::Cow::Owned(command),
            ))
            .is_ok();
        if !sent {
            // 不携带命令正文；本代次已消费，不以自动重试制造重复执行。
            // 单次发送失败是瞬态提示，不替换终端画面。
            self.notice = Some("Agent 会话恢复注入失败".to_owned());
        }
        if let Some(completion) = completion.take() {
            completion(if sent {
                AgentCommandOutcome::Injected
            } else {
                AgentCommandOutcome::Failed
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn resume_delay_is_shell_specific_and_bounded() {
        use crate::app_shell::app_root::navigation::RestoreShell;
        use std::time::Duration;

        for shell in ["cmd.exe", "C:/Windows/System32/CMD.EXE"] {
            assert_eq!(restore_shell_from_program(Some(shell)), RestoreShell::Cmd);
            assert_eq!(
                restore_injection_timeout(RestoreShell::Cmd),
                Duration::from_millis(500)
            );
        }
        for shell in ["pwsh.exe", "C:/PowerShell/7/powershell.exe"] {
            assert_eq!(
                restore_shell_from_program(Some(shell)),
                RestoreShell::PowerShell
            );
            assert_eq!(
                restore_injection_timeout(RestoreShell::PowerShell),
                Duration::from_secs(10)
            );
        }
        for shell in [None, Some("bash.exe")] {
            assert_eq!(
                restore_shell_from_program(shell),
                RestoreShell::Other,
                "未知程序不得冒充 PowerShell"
            );
            assert_eq!(
                restore_injection_timeout(RestoreShell::Other),
                Duration::from_millis(500)
            );
        }
    }

    /// 显式 Agent 注入没有可用 worker 时必须回报失败，而不是静默丢回执。
    #[cfg(feature = "gpui-test-support")]
    #[gpui::test]
    fn explicit_agent_injection_reports_failure_without_a_worker(cx: &mut gpui::TestAppContext) {
        use std::sync::mpsc;

        cx.update(|cx| {
            gpui_component::init(cx);
            crate::gpui_terminal::init(cx);
            cx.set_global(Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let (terminal, window) = cx.add_window_view(|window, cx| {
            TerminalView::restore_error(
                crate::gpui_terminal::PaneId(1),
                (80, 24),
                crate::gpui_terminal::view::TerminalLaunch::Local {
                    cwd: None,
                    shell: Some(slterm_terminal::tty::Shell::new(
                        "slterm-test-missing-shell-executable".into(),
                        Vec::new(),
                    )),
                    shell_name: None,
                    explicit_command: false,
                },
                "测试禁用真实 PTY".to_owned(),
                window,
                cx,
            )
        });
        let (outcome_tx, outcome_rx) = mpsc::sync_channel(1);
        window.update(|_window, cx| {
            terminal.update(cx, |view, cx| {
                view.schedule_agent_command(
                    "agent".to_owned(),
                    Some(Box::new(move |outcome| {
                        let _ = outcome_tx.try_send(outcome);
                    })),
                    cx,
                );
            });
        });
        window.executor().advance_clock(Duration::from_secs(11));
        window.run_until_parked();

        assert_eq!(
            outcome_rx.try_recv().expect("注入必须回报结局"),
            AgentCommandOutcome::Failed
        );
    }
}
