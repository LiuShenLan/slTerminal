//! 单终端局部关闭：原子状态提交后，后台回收 PTY，再按序清理并销毁 Panel。

use gpui::{AnyWindowHandle, App, Context, Entity, Window};
use sha2::{Digest as _, Sha256};
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage, ProjectSnapshot, TerminalId, TerminalRecord,
};

use crate::gpui_terminal::view::TerminalView;
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent,
};

use super::AppRoot;
use super::commands::AppRootError;
use super::task_service::{CancellationToken, TaskId};

/// 已消费的 PTY 只能在显式重试时复用停止事实，不能再次 join。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PtyStopObservation {
    Stopped,
    Panicked,
}

pub(super) struct LocalTerminalClose {
    pub(super) record: TerminalRecord,
    pub(super) panel_id: slterm_workspace::PanelId,
    pub(super) terminal: Entity<TerminalView>,
    pub(super) coordinator: CloseCoordinator<String>,
    pub(super) persisted: Option<PersistedCloseIntent>,
    pub(super) stop_observation: Option<PtyStopObservation>,
    pub(super) outcome_unknown: bool,
    pub(super) initiation_sequence: u64,
    pub(super) unresolved_failure: bool,
}

impl AppRoot {
    fn allocate_terminal_close_sequence(&mut self) -> Result<u64, AppRootError> {
        let sequence = self.next_terminal_close_sequence;
        self.next_terminal_close_sequence = self
            .next_terminal_close_sequence
            .checked_add(1)
            .ok_or(AppRootError::TerminalCloseState)?;
        Ok(sequence)
    }

    /// 无 writer 的内存测试根仍执行同一协调顺序，但不伪造持久回执。
    pub(super) fn close_terminal_in_memory(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        self.ensure_not_closing()?;
        if let Some(close) = self.terminal_closes.get(&id) {
            let record = close.record.clone();
            match close.coordinator.phase() {
                ClosePhase::Running(_) => return Err(AppRootError::TerminalCloseRunning),
                ClosePhase::Failed(_) => {
                    self.retry_terminal_close_in_memory(id, cx)?;
                    return Ok(record);
                }
                ClosePhase::Idle | ClosePhase::Completed => {
                    return Err(AppRootError::TerminalCloseState);
                }
            }
        }

        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let (candidate, record) = self.snapshot.without_terminal(id)?;
        let project_id = record.project_id();
        let (panel_id, terminal) = self
            .terminal_panel(project_id, id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let scope = CloseScope::Panels { project_id };
        let intent = CloseIntent::new(
            vec![CloseObject {
                id: id.to_string(),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            }],
            [0; 32],
        )
        .map_err(|_| AppRootError::TerminalCloseState)?;
        let mut coordinator = CloseCoordinator::new();
        let collect = coordinator
            .begin(scope)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let CloseEffect::Collect { attempt } = collect else {
            unreachable!("关闭协调器必须从收集开始")
        };
        coordinator
            .collected(attempt, intent)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let commit_attempt = coordinator
            .current_attempt()
            .ok_or(AppRootError::TerminalCloseState)?;
        coordinator
            .state_committed(commit_attempt)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let initiation_sequence = self.allocate_terminal_close_sequence()?;
        self.terminal_closes.insert(
            id,
            LocalTerminalClose {
                record: record.clone(),
                panel_id,
                terminal,
                coordinator,
                persisted: None,
                stop_observation: None,
                outcome_unknown: false,
                initiation_sequence,
                unresolved_failure: false,
            },
        );
        self.commit_project_snapshot(candidate);
        self.detach_closing_terminal(id, window, cx);
        cx.notify();
        self.drive_terminal_close_in_memory(id, cx)?;
        Ok(record)
    }

    pub(super) fn close_terminal_persisted(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        self.ensure_not_closing()?;
        if let Some(close) = self.terminal_closes.get(&id) {
            let record = close.record.clone();
            match close.coordinator.phase() {
                ClosePhase::Running(_) => return Err(AppRootError::TerminalCloseRunning),
                ClosePhase::Failed(_) => {
                    self.retry_terminal_close_persisted(id, cx)?;
                    return Ok(record);
                }
                ClosePhase::Idle | ClosePhase::Completed => {
                    return Err(AppRootError::TerminalCloseState);
                }
            }
        }

        self.ensure_terminal_close_recovery_allowed()?;
        let store_epoch = self
            .terminal_close_store_epoch()?
            .ok_or(AppRootError::WorkspaceEpochUnavailable)?;
        let record = self
            .snapshot
            .terminal(id)
            .cloned()
            .ok_or(slterm_workspace::ProjectSnapshotError::MissingTerminal(id))?;
        let project_id = record.project_id();
        let (panel_id, terminal) = self
            .terminal_panel(project_id, id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let (candidate, _) = self.snapshot.without_terminal(id)?;
        let close_id = uuid::Uuid::now_v7().to_string();
        let scope = CloseScope::Panels { project_id };
        let objects = vec![CloseObject {
            id: id.to_string(),
            generation: self.snapshot_generation,
            decision: CloseDecision::NotRequired,
        }];
        let fingerprint = close_fingerprint(&close_id, &scope, &store_epoch, &objects);
        let intent =
            CloseIntent::new(objects, fingerprint).map_err(|_| AppRootError::TerminalCloseState)?;
        let registration = PersistedCloseIntent {
            close_id,
            scope: scope.clone(),
            store_epoch,
            intent,
            commit_status: CloseCommitStatus::Collected,
        };
        let mut coordinator = CloseCoordinator::new();
        let collect = coordinator
            .begin(scope)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let CloseEffect::Collect { attempt } = collect else {
            unreachable!("关闭协调器必须从收集开始")
        };
        coordinator
            .collected(attempt, registration.intent.clone())
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let initiation_sequence = self.allocate_terminal_close_sequence()?;
        self.terminal_closes.insert(
            id,
            LocalTerminalClose {
                record: record.clone(),
                panel_id,
                terminal,
                coordinator,
                persisted: Some(registration),
                stop_observation: None,
                outcome_unknown: false,
                initiation_sequence,
                unresolved_failure: false,
            },
        );
        self.start_terminal_commit(id, candidate, window, cx)?;
        Ok(record)
    }

    fn retry_terminal_close_in_memory(
        &mut self,
        id: TerminalId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let close = self
            .terminal_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        close
            .coordinator
            .retry()
            .map_err(|_| AppRootError::TerminalCloseState)?;
        self.drive_terminal_close_in_memory(id, cx)
    }

    fn retry_terminal_close_persisted(
        &mut self,
        id: TerminalId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let close = self
            .terminal_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        if close.outcome_unknown {
            return Err(AppRootError::TerminalCloseOutcomeUnknown);
        }
        close
            .coordinator
            .retry()
            .map_err(|_| AppRootError::TerminalCloseState)?;
        self.drive_terminal_close_persisted(id, cx)
    }

    pub(super) fn start_terminal_commit(
        &mut self,
        id: TerminalId,
        candidate: ProjectSnapshot,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (writer, registration, attempt) = {
            let close = self
                .terminal_closes
                .get(&id)
                .ok_or(AppRootError::MissingPanel)?;
            let writer = self
                .project_writer
                .clone()
                .ok_or(AppRootError::WorkspaceEpochUnavailable)?;
            let registration = close
                .persisted
                .clone()
                .ok_or(AppRootError::TerminalCloseState)?;
            let attempt = close
                .coordinator
                .current_attempt()
                .ok_or(AppRootError::TerminalCloseState)?;
            (writer, registration, attempt)
        };
        let response = match writer.commit_close_intent(&candidate, &registration) {
            Ok(response) => response,
            Err(error) => {
                self.terminal_closes.remove(&id);
                self.resume_deferred_project_snapshot();
                return Err(AppRootError::TerminalCloseWriter(error));
            }
        };
        let window = window.window_handle();
        let executor = cx.background_executor().clone();
        let task_id = TaskId::TerminalClose(id);
        let registration_result =
            self.tasks
                .register_with(task_id, CancellationToken::new(), move || {
                    let commit = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = commit.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_terminal_commit(id, attempt, result, window, cx);
                        });
                    })
                });
        if registration_result.is_err() {
            self.fail_terminal_close_outcome_unknown(id, attempt);
        }
        Ok(())
    }

    pub(super) fn finish_terminal_commit(
        &mut self,
        id: TerminalId,
        attempt: CloseAttempt,
        result: Result<Result<PersistedCloseIntent, CloseCommitError>, std::sync::mpsc::RecvError>,
        window: AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        if let Ok(Ok(committed)) = &result {
            let Some(current) = &close.persisted else {
                return;
            };
            if committed.commit_status != CloseCommitStatus::StateCommitted
                || !same_close_fact(current, committed)
            {
                return;
            }
        }
        self.tasks.complete(TaskId::TerminalClose(id));
        match result {
            Ok(Ok(committed)) => {
                let mut completed = false;
                let updated = window.update(cx, |_, window, cx| {
                    match self.complete_terminal_state_commit(id, attempt, committed, window, cx) {
                        Ok(()) => completed = true,
                        Err(error) => {
                            log::warn!("terminal close state transition failed: {error}");
                            self.fail_terminal_close_outcome_unknown(id, attempt);
                        }
                    }
                });
                if updated.is_err() {
                    self.fail_terminal_close_outcome_unknown(id, attempt);
                } else if completed {
                    let _ = self.drive_terminal_close_persisted(id, cx);
                    cx.notify();
                }
            }
            Ok(Err(error)) => {
                if let Some(close) = self.terminal_closes.get_mut(&id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                self.terminal_closes.remove(&id);
                self.resume_deferred_project_snapshot();
                log::warn!("terminal close atomic commit failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
            }
            Err(_) => {
                log::warn!("terminal close atomic commit outcome is unknown");
                self.fail_terminal_close_outcome_unknown(id, attempt);
                super::events::notify_workspace_write_failed(cx);
            }
        }
    }

    fn complete_terminal_state_commit(
        &mut self,
        id: TerminalId,
        attempt: CloseAttempt,
        committed: PersistedCloseIntent,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), AppRootError> {
        if committed.commit_status != CloseCommitStatus::StateCommitted {
            return Err(AppRootError::TerminalCloseState);
        }
        // 持久候选提交后，从最新内存快照重算，保留在途 Exited 等其他变化。
        let (candidate, _) = self.snapshot.without_terminal(id)?;
        let close = self
            .terminal_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        close
            .coordinator
            .state_committed(attempt)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        close.persisted = Some(committed);
        self.commit_project_snapshot(candidate);
        self.detach_closing_terminal(id, window, cx);
        Ok(())
    }

    fn drive_terminal_close_in_memory(
        &mut self,
        id: TerminalId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(close) = self.terminal_closes.get(&id) else {
            return Ok(());
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observation.is_some() {
                    let attempt = close
                        .coordinator
                        .current_attempt()
                        .ok_or(AppRootError::TerminalCloseState)?;
                    self.terminal_closes
                        .get_mut(&id)
                        .expect("局部关闭刚读取必须仍存在")
                        .coordinator
                        .pty_stopped(attempt)
                        .map_err(|_| AppRootError::TerminalCloseState)?;
                    self.drive_terminal_close_in_memory(id, cx)
                } else {
                    self.start_terminal_stop(id, cx);
                    Ok(())
                }
            }
            ClosePhase::Running(CloseStage::Cleanup) => {
                let attempt = close
                    .coordinator
                    .current_attempt()
                    .ok_or(AppRootError::TerminalCloseState)?;
                self.terminal_closes
                    .get_mut(&id)
                    .expect("局部关闭刚读取必须仍存在")
                    .coordinator
                    .recovery_cleaned(attempt)
                    .map_err(|_| AppRootError::TerminalCloseState)?;
                self.destroy_closed_terminal(id, cx);
                Ok(())
            }
            ClosePhase::Running(CloseStage::CommitState)
            | ClosePhase::Idle
            | ClosePhase::Failed(_)
            | ClosePhase::Completed => Ok(()),
            ClosePhase::Running(_) => Err(AppRootError::TerminalCloseState),
        }
    }

    fn drive_terminal_close_persisted(
        &mut self,
        id: TerminalId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(close) = self.terminal_closes.get(&id) else {
            return Ok(());
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observation.is_some() {
                    self.start_terminal_advance(id, CloseCommitStatus::PtyStopped, cx);
                } else {
                    self.start_terminal_stop(id, cx);
                }
                Ok(())
            }
            ClosePhase::Running(CloseStage::Cleanup) => {
                let status = close
                    .persisted
                    .as_ref()
                    .map(|persisted| persisted.commit_status)
                    .ok_or(AppRootError::TerminalCloseState)?;
                match status {
                    CloseCommitStatus::PtyStopped => {
                        self.start_terminal_advance(id, CloseCommitStatus::CleanupCommitted, cx);
                        Ok(())
                    }
                    CloseCommitStatus::CleanupCommitted => {
                        self.start_terminal_advance(id, CloseCommitStatus::Completed, cx);
                        Ok(())
                    }
                    _ => Err(AppRootError::TerminalCloseState),
                }
            }
            ClosePhase::Running(CloseStage::CommitState)
            | ClosePhase::Idle
            | ClosePhase::Failed(_)
            | ClosePhase::Completed => Ok(()),
            ClosePhase::Running(_) => Err(AppRootError::TerminalCloseState),
        }
    }

    /// join 只在 background executor 执行；消息发送或 Panel Drop 都不代表停止完成。
    pub(super) fn start_terminal_stop(&mut self, id: TerminalId, cx: &mut Context<'_, Self>) {
        let Some(close) = self.terminal_closes.get(&id) else {
            return;
        };
        let Some(attempt) = close.coordinator.current_attempt() else {
            return;
        };
        let terminal = close.terminal.clone();
        let executor = cx.background_executor().clone();
        let task_id = TaskId::TerminalClose(id);
        let registration = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let session =
                    terminal.update(cx, |terminal, _| terminal.take_session_for_shutdown());
                let stop = executor.spawn(async move {
                    match session {
                        Some(session) => session.shutdown(),
                        None => Ok(()),
                    }
                });
                cx.spawn(async move |this, cx| {
                    let result = stop.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_terminal_stop(id, attempt, result, cx);
                    });
                })
            });
        if registration.is_err() {
            // StopPty 重复调度是幂等操作：保留原 attempt 与任务，等待原回执。
            log::debug!("terminal stop already scheduled");
        }
    }

    pub(super) fn finish_terminal_stop(
        &mut self,
        id: TerminalId,
        attempt: CloseAttempt,
        result: std::thread::Result<()>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        self.tasks.complete(TaskId::TerminalClose(id));
        let close = self
            .terminal_closes
            .get_mut(&id)
            .expect("局部关闭刚读取必须仍存在");
        match result {
            Ok(()) => close.stop_observation = Some(PtyStopObservation::Stopped),
            Err(_) => {
                // JoinHandle 的 Err 已证明线程终止，但只有显式重试才消费该事实。
                close.stop_observation = Some(PtyStopObservation::Panicked);
                close.unresolved_failure = true;
                let _ = close.coordinator.fail(attempt);
                log::warn!("{}", terminal_stop_panic_diagnostic());
                cx.notify();
                return;
            }
        }
        let persisted = self
            .terminal_closes
            .get(&id)
            .is_some_and(|close| close.persisted.is_some());
        if persisted {
            let _ = self.drive_terminal_close_persisted(id, cx);
        } else {
            let _ = self.drive_terminal_close_in_memory(id, cx);
        }
    }

    pub(super) fn start_terminal_advance(
        &mut self,
        id: TerminalId,
        next: CloseCommitStatus,
        cx: &mut Context<'_, Self>,
    ) {
        let (writer, current, attempt) = {
            let Some(close) = self.terminal_closes.get(&id) else {
                return;
            };
            let Some(attempt) = close.coordinator.current_attempt() else {
                return;
            };
            let Some(current) = close.persisted.clone() else {
                return;
            };
            let Some(writer) = self.project_writer.clone() else {
                return;
            };
            (writer, current, attempt)
        };
        let response = match writer.advance_close_intent(&current, next) {
            Ok(response) => response,
            Err(error) => {
                if let Some(close) = self.terminal_closes.get_mut(&id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("terminal close advance request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::TerminalClose(id),
            CancellationToken::new(),
            move || {
                let advance = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = advance.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_terminal_advance(id, attempt, next, result, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            self.fail_terminal_close_outcome_unknown(id, attempt);
        }
    }

    pub(super) fn finish_terminal_advance(
        &mut self,
        id: TerminalId,
        attempt: CloseAttempt,
        next: CloseCommitStatus,
        result: Result<Result<PersistedCloseIntent, CloseStoreError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        let stage_matches = matches!(
            (next, close.coordinator.phase()),
            (
                CloseCommitStatus::PtyStopped,
                ClosePhase::Running(CloseStage::StopPty)
            ) | (
                CloseCommitStatus::CleanupCommitted | CloseCommitStatus::Completed,
                ClosePhase::Running(CloseStage::Cleanup)
            )
        );
        if !stage_matches {
            return;
        }
        let Some(current) = &close.persisted else {
            return;
        };
        let expected_previous = match next {
            CloseCommitStatus::PtyStopped => CloseCommitStatus::StateCommitted,
            CloseCommitStatus::CleanupCommitted => CloseCommitStatus::PtyStopped,
            CloseCommitStatus::Completed => CloseCommitStatus::CleanupCommitted,
            _ => return,
        };
        if current.commit_status != expected_previous {
            return;
        }
        if let Ok(Ok(updated)) = &result
            && (updated.commit_status != next || !same_close_fact(current, updated))
        {
            return;
        }
        self.tasks.complete(TaskId::TerminalClose(id));
        match result {
            Ok(Ok(updated)) if updated.commit_status == next => {
                let close = self
                    .terminal_closes
                    .get_mut(&id)
                    .expect("局部关闭刚读取必须仍存在");
                close.persisted = Some(updated);
                match next {
                    CloseCommitStatus::PtyStopped => {
                        if close.coordinator.pty_stopped(attempt).is_err() {
                            self.fail_terminal_close_outcome_unknown(id, attempt);
                            return;
                        }
                    }
                    CloseCommitStatus::CleanupCommitted => {}
                    CloseCommitStatus::Completed => {
                        if close.coordinator.recovery_cleaned(attempt).is_err() {
                            self.fail_terminal_close_outcome_unknown(id, attempt);
                            return;
                        }
                        self.destroy_closed_terminal(id, cx);
                        return;
                    }
                    _ => {
                        self.fail_terminal_close_outcome_unknown(id, attempt);
                        return;
                    }
                }
                let _ = self.drive_terminal_close_persisted(id, cx);
            }
            Ok(Ok(_)) => self.fail_terminal_close_outcome_unknown(id, attempt),
            Ok(Err(error)) => {
                if let Some(close) = self.terminal_closes.get_mut(&id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("terminal close advancement failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
            }
            Err(_) => {
                log::warn!("terminal close advancement outcome is unknown");
                self.fail_terminal_close_outcome_unknown(id, attempt);
                super::events::notify_workspace_write_failed(cx);
            }
        }
    }

    fn fail_terminal_close_outcome_unknown(&mut self, id: TerminalId, attempt: CloseAttempt) {
        if let Some(close) = self.terminal_closes.get_mut(&id) {
            close.outcome_unknown = true;
            close.unresolved_failure = true;
            let _ = close.coordinator.fail(attempt);
        }
    }

    fn detach_closing_terminal(&mut self, id: TerminalId, window: &mut Window, cx: &mut App) {
        let Some(close) = self.terminal_closes.get(&id) else {
            return;
        };
        let project_id = close.record.project_id();
        let panel_id = close.panel_id;
        let terminal = close.terminal.clone();
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return;
        };
        let focus_token = self.panel_focus.get(&panel_id).map(|focus| focus.token());
        let was_focused = window
            .focused(cx)
            .is_some_and(|focused| focused == terminal.read(cx).focus_handle);
        if self.snapshot.registry().active() == Some(project_id) && was_focused {
            let next_panel = project.read(cx).panels.keys().find(|candidate| {
                **candidate != panel_id && self.panel_focus.contains_key(*candidate)
            });
            let mut candidates = Vec::new();
            if let Some(next_panel) = next_panel
                && let Some(registration) = self.panel_focus.get(next_panel)
            {
                candidates.push(registration.token());
            }
            if let Some(registration) = project.read(cx).focus_registration.as_ref() {
                candidates.push(registration.token());
            }
            candidates.push(self.focus_manager.workspace_token());
            if let Err(error) = self.focus_manager.restore_focus(&candidates, window, cx) {
                log::warn!("terminal close focus restoration failed: {error:?}");
                window.blur();
                self.focus_manager.sync_window_focus(window, cx);
            }
        }
        if let Some(token) = focus_token
            && let Err(error) = self.focus_manager.unregister(token)
        {
            log::warn!("terminal close focus unregister failed: {error:?}");
            let _ = self.focus_manager.set_visible(token, false);
            if was_focused {
                window.blur();
            }
        }
        self.panel_focus.remove(&panel_id);
        self.panel_subscriptions.remove(&panel_id);
        project.update(cx, |project, _| {
            if project.focused_panel == Some(panel_id) {
                project.focused_panel = project
                    .panels
                    .keys()
                    .find(|candidate| {
                        **candidate != panel_id && self.panel_focus.contains_key(*candidate)
                    })
                    .copied();
            }
        });
    }

    fn destroy_closed_terminal(&mut self, id: TerminalId, cx: &mut Context<'_, Self>) {
        let Some(close) = self.terminal_closes.remove(&id) else {
            return;
        };
        if let Some(project) = self.projects.get(&close.record.project_id()) {
            project.update(cx, |project, _| {
                project.panels.remove(&close.panel_id);
                if project.focused_panel == Some(close.panel_id) {
                    project.focused_panel = None;
                }
            });
        }
        cx.notify();
    }
}

pub(super) fn same_close_fact(
    current: &PersistedCloseIntent,
    receipt: &PersistedCloseIntent,
) -> bool {
    current.close_id == receipt.close_id
        && current.scope == receipt.scope
        && current.store_epoch == receipt.store_epoch
        && current.intent == receipt.intent
}

fn terminal_stop_panic_diagnostic() -> &'static str {
    "terminal PTY worker terminated abnormally"
}

pub(super) fn close_fingerprint(
    close_id: &str,
    scope: &CloseScope,
    store_epoch: &str,
    objects: &[CloseObject<String>],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"slterm-terminal-close-v1");
    hash_field(&mut hasher, close_id.as_bytes());
    match scope {
        CloseScope::Panels { project_id } => {
            hash_field(&mut hasher, b"panels");
            hash_field(&mut hasher, project_id.to_string().as_bytes());
        }
        CloseScope::Project { project_id } => {
            hash_field(&mut hasher, b"project");
            hash_field(&mut hasher, project_id.to_string().as_bytes());
        }
        CloseScope::ApplicationExit => hash_field(&mut hasher, b"application-exit"),
        CloseScope::UpdateHandoff => hash_field(&mut hasher, b"update-handoff"),
    }
    hash_field(&mut hasher, store_epoch.as_bytes());
    for object in objects {
        hash_field(&mut hasher, object.id.as_bytes());
        hasher.update(object.generation.to_be_bytes());
        let decision = match object.decision {
            CloseDecision::Save => 0,
            CloseDecision::Discard => 1,
            CloseDecision::NotRequired => 2,
        };
        hasher.update([decision]);
    }
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_be_bytes());
    hasher.update(field);
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;

    fn scope() -> CloseScope {
        CloseScope::Panels {
            project_id: slterm_workspace::ProjectId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
        }
    }

    fn objects() -> Vec<CloseObject<String>> {
        vec![CloseObject {
            id: "terminal-object".to_owned(),
            generation: 17,
            decision: CloseDecision::NotRequired,
        }]
    }

    #[test]
    fn stop_panic_diagnostic_never_echoes_payload() {
        let diagnostic = terminal_stop_panic_diagnostic();
        assert_eq!(diagnostic, "terminal PTY worker terminated abnormally");
        assert!(!diagnostic.contains("sensitive-test-panic-payload"));
    }

    #[test]
    fn fingerprint_binds_close_scope_epoch_and_object_facts() {
        let scope = scope();
        let objects = objects();
        let baseline = close_fingerprint("close-a", &scope, "epoch-a", &objects);
        assert_ne!(baseline, [0; 32]);
        assert_ne!(
            baseline,
            close_fingerprint("close-b", &scope, "epoch-a", &objects)
        );
        assert_ne!(
            baseline,
            close_fingerprint("close-a", &scope, "epoch-b", &objects)
        );
        let other_scope = CloseScope::Panels {
            project_id: slterm_workspace::ProjectId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
        };
        assert_ne!(
            baseline,
            close_fingerprint("close-a", &other_scope, "epoch-a", &objects)
        );
        let mut changed = objects.clone();
        changed[0].id = "other-terminal".to_owned();
        assert_ne!(
            baseline,
            close_fingerprint("close-a", &scope, "epoch-a", &changed)
        );
        let mut changed = objects.clone();
        changed[0].generation += 1;
        assert_ne!(
            baseline,
            close_fingerprint("close-a", &scope, "epoch-a", &changed)
        );
        let mut changed = objects;
        changed[0].decision = CloseDecision::Discard;
        assert_ne!(
            baseline,
            close_fingerprint("close-a", &scope, "epoch-a", &changed)
        );
    }
}
