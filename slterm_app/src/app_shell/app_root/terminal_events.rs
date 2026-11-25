//! 终端视图事件路由：退出态、agent 命令态与任务栏进度。

use gpui::{Context, Window};
use slterm_workspace::{PanelId, RunIntent};

use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::view::TerminalViewEvent;

use super::AppRoot;

impl AppRoot {
    fn sync_agent_command(&mut self, panel_id: PanelId, started: bool, cx: &mut Context<'_, Self>) {
        let Some(terminal_id) = self.terminal_identity(panel_id, cx) else {
            return;
        };
        let Some(project_id) = self
            .snapshot
            .terminal(terminal_id)
            .map(|record| record.project_id())
        else {
            return;
        };
        let pane_id = PaneId(panel_id.sequence());
        if started {
            let program = self
                .terminal_panel(project_id, terminal_id, cx)
                .and_then(|(_, terminal)| terminal.read(cx).running_program.clone());
            self.agents.command_started(pane_id, program.as_deref());
        } else {
            self.agents.command_finished(pane_id);
        }
        let agent = self.agents.session_agent(pane_id);
        let current = self
            .snapshot
            .terminal(terminal_id)
            .and_then(|record| record.agent());
        if current == agent.as_ref() {
            cx.notify();
            return;
        }
        match self.snapshot.with_terminal_agent(terminal_id, agent) {
            Ok(candidate) => {
                self.commit_project_snapshot(candidate, cx);
                cx.notify();
            }
            Err(error) => log::warn!("终端 Agent 身份提交失败: {error}"),
        }
    }

    pub(crate) fn on_terminal_event(
        &mut self,
        panel_id: PanelId,
        event: &TerminalViewEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            TerminalViewEvent::TitleChanged => cx.notify(),
            TerminalViewEvent::Exited => {
                let Some(terminal_id) = self.terminal_identity(panel_id, cx) else {
                    return;
                };
                self.agents.process_exited(PaneId(panel_id.sequence()));
                match self
                    .snapshot
                    .with_terminal_run_intent(terminal_id, RunIntent::Exited)
                {
                    Ok(candidate) => {
                        self.commit_project_snapshot(candidate, cx);
                        cx.notify();
                    }
                    Err(error) => log::warn!("终端退出状态提交失败: {error}"),
                }
            }
            TerminalViewEvent::CommandStarted => self.sync_agent_command(panel_id, true, cx),
            TerminalViewEvent::CommandFinished => self.sync_agent_command(panel_id, false, cx),
            // 只反映活跃项目的聚焦 pane；后台 pane 的进度不抢同一条任务栏。
            TerminalViewEvent::ProgressChanged(progress) => {
                let active = self.snapshot.registry().active();
                if active.is_some_and(|project| self.focused_panel(project, cx) == Some(panel_id)) {
                    crate::notify::observe_taskbar_progress(
                        crate::gpui_terminal::PaneId(panel_id.sequence()),
                        *progress,
                        crate::windowing::focus::window_hwnd(cx),
                    );
                }
            }
            TerminalViewEvent::UserInput(_) | TerminalViewEvent::CommandSubmitted(_) => {}
        }
    }
}
