//! Runtime 写命令在 UI owner 上的执行器(04 篇 M7.5)。
//!
//! 全序:transport 线程 `RuntimeDispatch` → 壳事件队列 → 前台泵 →
//! **本模块**把命令映射到 Workspace/TerminalView 既有入口 → `respond`。
//! 回包形状按 M7.4 缝合点:`{action: {window_id, pane_id}(或 action.agent),
//! snapshot: RuntimeSnapshot}`;prompt/paste 成功回包携带**提交后**快照与
//! `state_change_seq`(CLI `--wait` 基线链依赖)。
//!
//! 生命周期:AgentStart/AgentFork 成功后以 worktree provenance 注册
//! managed agent;SessionEnd/pane 关闭关闭注册项;TurnDone 完成委派并把
//! 回传提示写回 origin pane。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{App, Context, Window};
use serde_json::{Value, json};
use slterm_split::{PaneId, SplitDirection, TabId};

use super::super::SltermShellEvent;
use super::runtime_projection;
use super::{SltermWorkspace, WorkspaceTab};
use crate::ai_hook::{AiHookEvent, AiHookKind};
use crate::gpui_terminal::view::TerminalView;
use crate::runtime_api::{
    ApiError, RuntimeCommand, RuntimeDispatch, RuntimeSplitDirection, RuntimeTaskState,
    SINGLE_WINDOW_ID, capture_process_tree,
};
use crate::runtime_exec;

/// run 身份只在本进程内单调;OSC 133 记账归 02 篇 M9。
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

/// 忙碌判据:等待用户/正在运行/需要 attention 的 pane 不允许静默关闭。
fn busy_state(state: RuntimeTaskState) -> bool {
    matches!(
        state,
        RuntimeTaskState::Running | RuntimeTaskState::WaitingInput | RuntimeTaskState::Attention
    )
}

fn target_not_found(what: &str) -> ApiError {
    ApiError::new("target_not_found", format!("{what} does not exist"))
}

fn confirmation_required(busy: &[u64]) -> ApiError {
    ApiError::new(
        "confirmation_required",
        "the target contains panes that are running or waiting for input",
    )
    .details(json!({ "busy_pane_ids": busy }))
}

impl SltermWorkspace {
    /// 壳事件泵入口(前台线程,持有 Window)。
    pub(crate) fn handle_shell_event(
        &mut self,
        event: SltermShellEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            // 单窗语义:ATTACH = 聚焦现有唯一窗,不建第二窗。
            SltermShellEvent::MuxAttach => {
                window.activate_window();
                cx.notify();
            }
            SltermShellEvent::RuntimeControl(dispatch) => {
                self.execute_runtime_dispatch(dispatch, window, cx);
            }
            // 消费(右键「在此处打开」)归 09 篇。
            SltermShellEvent::OpenDirectories(_) => {}
        }
    }

    /// 执行一条写命令并回执;Exec 是异步执行器,自行 respond。
    fn execute_runtime_dispatch(
        &mut self,
        dispatch: Arc<RuntimeDispatch>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let command = dispatch.command.clone();
        if let RuntimeCommand::Exec {
            window_id, pane_id, ..
        } = &command
        {
            let started = self.start_pane_exec(dispatch.clone(), *window_id, *pane_id, cx);
            if let Err(error) = started {
                dispatch.respond(Err(error));
            }
            return;
        }
        let result = self.execute_runtime_command(&command, window, cx);
        dispatch.respond(result);
    }

    /// `pane.exec`:独立非 TTY 子进程由 runtime_exec 线程执行并回投。
    fn start_pane_exec(
        &self,
        dispatch: Arc<RuntimeDispatch>,
        window_id: Option<u64>,
        pane_id: u64,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), ApiError> {
        ensure_window(window_id)?;
        let (_, _, view) = self.pane_entry(pane_id)?;
        let (cwd, context) = {
            let view = view.read(cx);
            (
                view.cwd.clone(),
                view.runtime_exec_context().ok_or_else(|| {
                    ApiError::new(
                        "exec_unavailable",
                        "the pane has no frozen PTY environment to reuse",
                    )
                })?,
            )
        };
        runtime_exec::spawn(dispatch, context, cwd);
        Ok(())
    }

    fn execute_runtime_command(
        &mut self,
        command: &RuntimeCommand,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        match command {
            RuntimeCommand::Snapshot => {
                let snapshot = self.publish_runtime_snapshot(cx);
                serde_json::to_value(snapshot)
                    .map_err(|error| ApiError::new("invalid_runtime_response", error.to_string()))
            }
            RuntimeCommand::CloseWindow { window_id } => {
                ensure_window(*window_id)?;
                let busy = self.busy_panes(self.tabs.iter().flat_map(tab_pane_ids));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                let result = self.action_result(json!({ "window_id": SINGLE_WINDOW_ID }), cx)?;
                window.remove_window();
                Ok(result)
            }
            RuntimeCommand::Focus { window_id, pane_id } => {
                ensure_window(*window_id)?;
                window.activate_window();
                if let Some(pane_id) = pane_id {
                    let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                    if tab_ix != self.active {
                        self.activate_tab(tab_ix, window, cx);
                    }
                    self.focus_pane(tab_ix, PaneId(*pane_id), window, cx);
                }
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::NewTab {
                window_id,
                cwd,
                shell_id,
            } => {
                ensure_window(*window_id)?;
                let shell = resolve_shell(shell_id.as_deref())?;
                self.add_terminal_tab(cwd.clone(), shell, false, window, cx);
                let tab_id = self.tabs.last().map(|entry| entry.id.0);
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": self.focused_pane().map(|pane| pane.0),
                        "tab_id": tab_id
                    }),
                    cx,
                )
            }
            RuntimeCommand::CloseTab {
                window_id,
                tab_index,
                tab_id,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                let busy = self.busy_panes(self.tab_pane_ids(index));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                let closed = self.tabs[index].id.0;
                // 关最后一个 tab = 关窗;先取快照再摘除,避免在已移除窗口
                // 的上下文里继续投影。
                let result = self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "tab_id": closed, "tab_index": index }),
                    cx,
                )?;
                self.close_tab_at(index, window, cx);
                Ok(result)
            }
            RuntimeCommand::RenameTab {
                window_id,
                tab_index,
                tab_id,
                name,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                if let Some(entry) = self.tabs.get_mut(index) {
                    entry.meta.custom_name = (!name.is_empty()).then(|| name.clone());
                }
                self.sync_window_title(window, cx);
                cx.notify();
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "tab_id": self.tabs[index].id.0 }),
                    cx,
                )
            }
            RuntimeCommand::MoveTab {
                window_id,
                tab_index,
                tab_id,
                to_index,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                if *to_index >= self.tabs.len() {
                    return Err(ApiError::invalid_params(format!(
                        "to_index {} is outside the tab list",
                        to_index
                    )));
                }
                self.move_tab_index(index, *to_index);
                self.sync_window_title(window, cx);
                cx.notify();
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "tab_id": self.tabs[*to_index].id.0,
                        "tab_index": to_index
                    }),
                    cx,
                )
            }
            RuntimeCommand::Split {
                window_id,
                pane_id,
                direction,
            } => {
                ensure_window(*window_id)?;
                if let Some(pane_id) = pane_id {
                    let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                    if tab_ix != self.active {
                        self.activate_tab(tab_ix, window, cx);
                    }
                    self.focus_pane(tab_ix, PaneId(*pane_id), window, cx);
                }
                let direction = match direction {
                    RuntimeSplitDirection::LeftRight => SplitDirection::LeftRight,
                    RuntimeSplitDirection::TopBottom => SplitDirection::TopBottom,
                };
                let new_pane = self.split_focused(direction, window, cx).ok_or_else(|| {
                    ApiError::new("invalid_state", "the focused pane cannot be split")
                })?;
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": new_pane.0 }),
                    cx,
                )
            }
            RuntimeCommand::ClosePane { window_id, pane_id } => {
                ensure_window(*window_id)?;
                self.pane_entry(*pane_id)?;
                let busy = self.busy_panes(std::iter::once(PaneId(*pane_id)));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                // 关最后一个 pane = 关窗:同样先取快照再摘除。
                let result = self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )?;
                self.close_pane(PaneId(*pane_id), window, cx);
                Ok(result)
            }
            RuntimeCommand::ZoomPane {
                window_id,
                pane_id,
                zoomed,
            } => {
                ensure_window(*window_id)?;
                let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                if tab_ix != self.active {
                    self.activate_tab(tab_ix, window, cx);
                }
                self.focus_pane(tab_ix, PaneId(*pane_id), window, cx);
                let mut effective = *zoomed;
                let changed = match self.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab) {
                    Some(WorkspaceTab::Terminal {
                        panes,
                        zoomed: current,
                        ..
                    }) => {
                        let next = *zoomed && panes.len() > 1;
                        effective = next;
                        let changed = *current != next;
                        *current = next;
                        changed
                    }
                    _ => false,
                };
                if changed {
                    self.mark_structural_resize(tab_ix, cx);
                    cx.notify();
                }
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "zoomed": effective
                    }),
                    cx,
                )
            }
            RuntimeCommand::ResizePane {
                window_id,
                pane_id,
                ratio,
            } => {
                ensure_window(*window_id)?;
                let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                let resized = match self.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab) {
                    Some(WorkspaceTab::Terminal { tree, .. }) => {
                        tree.set_leaf_parent_ratio(PaneId(*pane_id), *ratio)
                    }
                    _ => false,
                };
                if !resized {
                    return Err(ApiError::new(
                        "invalid_state",
                        "the pane has no direct parent split to resize",
                    ));
                }
                self.mark_structural_resize(tab_ix, cx);
                cx.notify();
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id, "ratio": ratio }),
                    cx,
                )
            }
            RuntimeCommand::Prompt {
                window_id,
                pane_id,
                text,
                submit,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| view.runtime_prompt(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(*pane_id), text);
                }
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::Paste {
                window_id,
                pane_id,
                text,
                submit,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| view.runtime_paste(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(*pane_id), text);
                }
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::ReadPane {
                window_id,
                pane_id,
                lines,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                let task_state = self.pane_task_state(PaneId(*pane_id));
                let read =
                    view.read(cx)
                        .runtime_read(SINGLE_WINDOW_ID, *pane_id, *lines, task_state)?;
                self.action_result(json!(read), cx)
            }
            RuntimeCommand::Procs { window_id, pane_id } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                let root_pid = view.read(cx).runtime_shell_pid().ok_or_else(|| {
                    ApiError::new(
                        "process_query_failed",
                        "the pane has no live shell process to inspect",
                    )
                })?;
                let processes = capture_process_tree(SINGLE_WINDOW_ID, *pane_id, root_pid)?;
                self.action_result(json!(processes), cx)
            }
            RuntimeCommand::SendKey {
                window_id,
                pane_id,
                key,
                modifiers,
                repeat,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| {
                    view.runtime_send_key(*key, *modifiers, *repeat, cx)
                })?;
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "key": key.as_str(),
                        "repeat": repeat
                    }),
                    cx,
                )
            }
            RuntimeCommand::Run {
                window_id,
                pane_id,
                command,
                timeout_ms,
                ..
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| view.runtime_prompt(command, true, cx))?;
                self.agent_hub.submitted(PaneId(*pane_id), command);
                let run_id = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
                // OSC 133 真实 exit code 归 02 篇 M9;此后 wait 链如实报
                // run_start_timeout / exit_code_unavailable,绝不伪造 0。
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "run_id": run_id,
                        "timeout_ms": timeout_ms
                    }),
                    cx,
                )
            }
            RuntimeCommand::Exec { .. } => unreachable!("Exec is handled asynchronously"),
            RuntimeCommand::Git { .. } => Err(ApiError::new(
                "git_unavailable",
                "Git method execution is not wired yet (07 篇子进程封装)",
            )),
            RuntimeCommand::AgentStart {
                window_id,
                pane_id,
                name,
                kind,
                cwd,
                session_id,
                command,
                worktree,
            } => self.start_agent(
                *window_id,
                *pane_id,
                name,
                *kind,
                cwd.clone(),
                session_id.clone(),
                command,
                worktree.clone(),
                window,
                cx,
            ),
            // prepare_dispatch_command 已把 agent.fork 转成 AgentStart;直接
            // 到达 UI 说明调用方绕过了事务原语,拒绝而不是无事务启动。
            RuntimeCommand::AgentFork { .. } => Err(ApiError::new(
                "invalid_runtime_command",
                "agent.fork must be prepared by the dispatch transaction",
            )),
            RuntimeCommand::AgentPrompt {
                agent,
                generation,
                text,
                submit,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                view.update(cx, |view, cx| view.runtime_prompt(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(target.pane_id), text);
                }
                self.action_result(json!({ "agent": target }), cx)
            }
            RuntimeCommand::AgentPaste {
                agent,
                generation,
                text,
                submit,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                view.update(cx, |view, cx| view.runtime_paste(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(target.pane_id), text);
                }
                self.action_result(json!({ "agent": target }), cx)
            }
            RuntimeCommand::AgentRead {
                agent,
                generation,
                lines,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                let task_state = self.pane_task_state(PaneId(target.pane_id));
                let read = view.read(cx).runtime_read(
                    SINGLE_WINDOW_ID,
                    target.pane_id,
                    *lines,
                    task_state,
                )?;
                self.action_result(json!({ "agent": target, "read": read }), cx)
            }
        }
    }

    /// AgentStart/AgentFork(已事务化)的 UI 落地:新 tab 或既有 pane 里
    /// 提交冷启动命令,随后以 worktree provenance 注册 managed agent。
    #[allow(clippy::too_many_arguments)]
    fn start_agent(
        &mut self,
        window_id: Option<u64>,
        pane_id: Option<u64>,
        name: &str,
        kind: crate::ai_agents::AgentKind,
        cwd: Option<std::path::PathBuf>,
        session_id: Option<String>,
        command: &str,
        worktree: Option<crate::git_worktree::WorktreeProvenance>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        ensure_window(window_id)?;
        let created_tab = pane_id.is_none();
        let target = match pane_id {
            Some(pane_id) => {
                self.pane_entry(pane_id)?;
                pane_id
            }
            None => {
                self.add_terminal_tab(cwd, None, false, window, cx);
                self.focused_pane()
                    .map(|pane| pane.0)
                    .ok_or_else(|| ApiError::new("action_failed", "no pane to launch into"))?
            }
        };
        let (_, _, view) = self.pane_entry(target)?;
        let launched = view.update(cx, |view, cx| view.runtime_prompt(command, true, cx));
        if let Err(error) = launched {
            self.rollback_created_tab(created_tab, window, cx);
            return Err(error);
        }
        let agent = self.runtime_hub.register_agent(
            name.to_owned(),
            kind,
            SINGLE_WINDOW_ID,
            target,
            session_id,
            worktree,
        );
        match agent {
            Ok(agent) => self.action_result(json!({ "agent": agent }), cx),
            Err(error) => {
                self.rollback_created_tab(created_tab, window, cx);
                Err(error)
            }
        }
    }

    fn rollback_created_tab(
        &mut self,
        created_tab: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if created_tab {
            self.close_tab_at(self.active, window, cx);
        }
    }

    /// 提交后快照随回执返回:transport 侧 CLI 以 `state_change_seq` 作
    /// `--wait` 基线(缺基线不降级)。
    fn action_result(&self, action: Value, cx: &App) -> Result<Value, ApiError> {
        let snapshot = self.publish_runtime_snapshot(cx);
        let snapshot = serde_json::to_value(snapshot)
            .map_err(|error| ApiError::new("invalid_runtime_response", error.to_string()))?;
        Ok(json!({ "action": action, "snapshot": snapshot }))
    }

    fn pane_task_state(&self, pane_id: PaneId) -> RuntimeTaskState {
        self.agent_hub
            .host(pane_id)
            .map_or(RuntimeTaskState::Idle, |host| {
                runtime_projection::runtime_task_state(host.status())
            })
    }

    fn pane_entry(
        &self,
        pane_id: u64,
    ) -> Result<(usize, usize, gpui::Entity<TerminalView>), ApiError> {
        for (tab_ix, entry) in self.tabs.iter().enumerate() {
            let WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
                continue;
            };
            if let Some(position) = panes.iter().position(|pane| pane.id.0 == pane_id) {
                return Ok((tab_ix, position, panes[position].view.clone()));
            }
        }
        Err(target_not_found(&format!("pane {pane_id}")))
    }

    fn tab_index_for(&self, tab_id: Option<TabId>, index: usize) -> Result<usize, ApiError> {
        if let Some(tab_id) = tab_id {
            return self
                .tabs
                .iter()
                .position(|entry| entry.id == tab_id)
                .ok_or_else(|| target_not_found(&format!("tab {}", tab_id.0)));
        }
        if index < self.tabs.len() {
            Ok(index)
        } else {
            Err(target_not_found(&format!("tab index {index}")))
        }
    }

    fn busy_panes(&self, panes: impl Iterator<Item = PaneId>) -> Vec<u64> {
        panes
            .filter(|pane| busy_state(self.pane_task_state(*pane)))
            .map(|pane| pane.0)
            .collect()
    }

    fn tab_pane_ids(&self, index: usize) -> impl Iterator<Item = PaneId> + '_ {
        self.tab_panes(index).map(|pane| pane.id)
    }

    /// tab 重排的活跃下标调整(稳定 TabId 不受位置变化影响)。
    fn move_tab_index(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let entry = self.tabs.remove(from);
        self.tabs.insert(to, entry);
        self.active = if self.active == from {
            to
        } else {
            let mut index = self.active;
            if index > from {
                index -= 1;
            }
            if index >= to {
                index += 1;
            }
            index
        };
    }

    /// 关闭任意 tab(活跃 tab 快捷键与 runtime `tab.close` 共用)。
    ///
    /// 脏文件 tab 先走确认路径(documents.rs);确认/保存在途期间关闭被
    /// 拦截,避免草稿被静默丢弃。
    pub(super) fn close_tab_at(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.guard_file_tab_close(index, window, cx) {
            return;
        }
        self.finish_close_tab(index, window, cx);
    }

    /// 确认后的实际摘除:逐 pane 显式 shutdown,不留孤儿 PTY;最后一个
    /// tab = 关窗。脏确认流程只在本方法放行后到达。
    pub(super) fn finish_close_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(entry) = self.tabs.get(index) else {
            return;
        };
        if let WorkspaceTab::Terminal { panes, .. } = &entry.tab {
            let views: Vec<_> = panes.iter().map(|pane| pane.view.clone()).collect();
            let ids: Vec<_> = panes.iter().map(|pane| pane.id).collect();
            for view in &views {
                view.read(cx).shutdown();
            }
            for id in ids {
                self.forget_pane(id);
            }
        }
        self.remove_tab_entry(index);
        if self.tabs.is_empty() {
            // 空会话干净落盘:下次启动不恢复,也不把这次关标签误判成崩溃。
            self.save_on_tabs_closed(window, cx);
            window.remove_window();
            return;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// hook 批次后的 managed agent 生命周期与委派回传(前台线程)。
    pub(crate) fn process_agent_lifecycle_events(
        &mut self,
        events: &[AiHookEvent],
        cx: &mut Context<'_, Self>,
    ) {
        for event in events {
            match event.kind {
                AiHookKind::SessionEnd => {
                    if let Some(pane) = event.pane {
                        self.runtime_hub.close_agents_for_pane(
                            SINGLE_WINDOW_ID,
                            pane,
                            "agent_exited",
                        );
                    }
                }
                AiHookKind::TurnDone => {
                    self.runtime_hub.complete_delegations_for_turn(event);
                    let Some(pane) = event.pane else { continue };
                    let Some(identity) = self
                        .runtime_hub
                        .active_agent_in_pane(SINGLE_WINDOW_ID, pane)
                        .map(|agent| (agent.agent_id, agent.generation))
                    else {
                        continue;
                    };
                    let Some(callback) = self
                        .runtime_hub
                        .take_delegation_callback(&identity.0, identity.1)
                    else {
                        continue;
                    };
                    let origin = callback.origin.pane_id;
                    let Ok((_, _, view)) = self.pane_entry(origin) else {
                        log::warn!(
                            "delegation callback dropped task_id={} reason=origin_pane_missing",
                            callback.task_id
                        );
                        continue;
                    };
                    if view
                        .update(cx, |view, cx| {
                            view.runtime_prompt(&callback.prompt, true, cx)
                        })
                        .is_ok()
                    {
                        self.agent_hub.submitted(PaneId(origin), &callback.prompt);
                    }
                }
                _ => {}
            }
        }
    }
}

fn ensure_window(window_id: Option<u64>) -> Result<(), ApiError> {
    match window_id {
        Some(id) if id != SINGLE_WINDOW_ID => Err(target_not_found(&format!("window {id}"))),
        _ => Ok(()),
    }
}

/// `tab.new` 的可选 shell id → 启动身份;None = 对端默认 shell 探测链。
fn resolve_shell(
    shell_id: Option<&str>,
) -> Result<Option<(slterm_terminal::tty::Shell, String)>, ApiError> {
    let Some(shell_id) = shell_id else {
        return Ok(None);
    };
    let shell = slterm_terminal::tty::resolve_shell(Some(shell_id)).map_err(|error| {
        ApiError::invalid_params(format!("unknown shell {shell_id:?}: {error}"))
    })?;
    let name = super::shell_display_name(shell.program());
    Ok(Some((shell, name)))
}

fn tab_pane_ids(entry: &super::TabEntry) -> Vec<PaneId> {
    match &entry.tab {
        WorkspaceTab::Terminal { panes, .. } => panes.iter().map(|pane| pane.id).collect(),
        WorkspaceTab::Settings { .. }
        | WorkspaceTab::Document { .. }
        | WorkspaceTab::Code { .. }
        | WorkspaceTab::Image { .. } => Vec::new(),
    }
}
