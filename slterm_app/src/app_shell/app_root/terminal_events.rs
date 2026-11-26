//! 终端视图事件路由：退出态、agent 命令态与任务栏进度。

use gpui::{Context, Window};
use slterm_workspace::{PanelId, RunIntent};

use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::view::TerminalViewEvent;

use super::AppRoot;

impl AppRoot {
    /// pane 序号 → 当前实例中的 PanelId；只用于把 hook 路由回已注册面板。
    fn panel_for_pane(&self, pane: PaneId, cx: &gpui::App) -> Option<PanelId> {
        self.projects.values().find_map(|project| {
            project
                .read(cx)
                .panels
                .keys()
                .copied()
                .find(|panel| panel.sequence() == pane.0)
        })
    }

    /// 事件必须来自目标 pane 的 PTY 进程树；否则只记计数并丢弃。
    fn hook_pane_is_owned(&self, pane: PaneId, client_pid: Option<u32>, cx: &gpui::App) -> bool {
        let Some(client_pid) = client_pid else {
            return false;
        };
        let Some(panel) = self.panel_for_pane(pane, cx) else {
            return false;
        };
        let Some(terminal_id) = self.terminal_identity(panel, cx) else {
            return false;
        };
        let registry = self.terminals.borrow();
        let Some(target) = registry.target(terminal_id) else {
            return false;
        };
        let Ok(process) = registry.process_identity(target) else {
            return false;
        };
        crate::process_tree::is_descendant(client_pid, process.pid().get())
    }

    /// 命名管道事件唯一前台入口：先证明 pane 归属，再交给仲裁器与通知漏斗。
    pub(crate) fn on_ai_hook_events(
        &mut self,
        events: Vec<crate::ai_hook::AiHookEvent>,
        cx: &mut Context<'_, Self>,
    ) {
        let fallback = self
            .snapshot
            .registry()
            .active()
            .and_then(|project| self.focused_panel(project, cx))
            .map(|panel| PaneId(panel.sequence()));
        let mut owned = Vec::new();
        let mut dropped = 0usize;
        for mut event in events {
            let Some(pane) = event.pane.map(PaneId).or(fallback) else {
                dropped += 1;
                continue;
            };
            if !self.hook_pane_is_owned(pane, event.client_pid, cx) {
                dropped += 1;
                continue;
            }
            event.pane = Some(pane.0);
            owned.push(event);
        }
        if dropped > 0 {
            log::debug!("ai_hook: dropped {dropped} unowned pane event(s)");
        }
        let (_accepted, accepted_events) = self.agents.dispatch_lifecycle(owned, None);
        for event in &accepted_events {
            if let Some((notification, pane)) = crate::notify::from_accepted_hook(event, None) {
                crate::notify::deliver_gpui(&notification, pane, cx);
            }
        }
        self.sync_hook_agent_sessions(cx);
        cx.notify();
    }

    /// 把 hook 回填的会话身份提交到同一项目快照；身份未变则零写。
    fn sync_hook_agent_sessions(&mut self, cx: &mut Context<'_, Self>) {
        let mut updates = Vec::new();
        for project in self.projects.values() {
            let project = project.read(cx);
            for panel in project.panels.keys().copied() {
                let Some(terminal_id) = self.terminal_identity(panel, cx) else {
                    continue;
                };
                let pane = PaneId(panel.sequence());
                let agent = self.agents.session_agent(pane);
                let current = self
                    .snapshot
                    .terminal(terminal_id)
                    .and_then(|record| record.agent());
                if current != agent.as_ref() {
                    updates.push((terminal_id, agent));
                }
            }
        }
        if updates.is_empty() {
            return;
        }
        let mut candidate = self.snapshot.clone();
        for (terminal_id, agent) in updates {
            match candidate.with_terminal_agent(terminal_id, agent) {
                Ok(updated) => candidate = updated,
                Err(error) => log::warn!("hook Agent 身份提交失败: {error}"),
            }
        }
        if candidate != self.snapshot {
            self.commit_project_snapshot(candidate, cx);
        }
    }

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
