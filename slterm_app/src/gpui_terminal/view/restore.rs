//! Agent 会话恢复：就绪信号或有界超时后，每个 PTY 代次只注入一次。

use std::time::Duration;

use gpui::Context;
use slterm_agents::AgentSession;

use super::TerminalView;
use crate::app_shell::config::Settings;

const POWERSHELL_RESUME_TIMEOUT: Duration = Duration::from_secs(10);
const CMD_RESUME_DELAY: Duration = Duration::from_millis(500);

fn resume_delay(shell: Option<&str>) -> Duration {
    if shell.is_some_and(|shell| {
        std::path::Path::new(shell)
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case("cmd"))
    }) {
        CMD_RESUME_DELAY
    } else {
        POWERSHELL_RESUME_TIMEOUT
    }
}

pub(crate) struct PendingResume {
    command: String,
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
        self.pending_resume = None;
        if self.exited.is_some()
            || !cx
                .try_global::<Settings>()
                .is_none_or(|settings| settings.resume_ai)
        {
            return;
        }
        let Some(command) = agent.and_then(AgentSession::resume_command) else {
            return;
        };
        let delay = resume_delay(
            self.exec_context
                .as_ref()
                .and_then(crate::completion::PaneExecContext::shell_program),
        );
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
        if self.exited.is_some() {
            return;
        }
        let mut command = pending.command.into_bytes();
        command.push(b'\r');
        if self
            .send_worker_command(slterm_terminal::event_loop::Msg::Input(
                std::borrow::Cow::Owned(command),
            ))
            .is_err()
        {
            // 不携带命令正文；本代次已消费，不以自动重试制造重复执行。
            self.error = Some("Agent 会话恢复注入失败".to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_delay_is_shell_specific_and_bounded() {
        for shell in ["cmd.exe", "C:/Windows/System32/CMD.EXE"] {
            assert_eq!(resume_delay(Some(shell)), CMD_RESUME_DELAY);
        }
        for shell in [None, Some("pwsh.exe"), Some("powershell.exe")] {
            assert_eq!(resume_delay(shell), POWERSHELL_RESUME_TIMEOUT);
        }
    }
}
