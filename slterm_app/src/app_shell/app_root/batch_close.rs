//! 组内批量关闭：一个关闭意图原子提交全部成员，再统一停止与清理。

use std::collections::{BTreeMap, BTreeSet};

use gpui::{AnyWindowHandle, App, Context, Entity, Window};
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage, PanelGroupId, PanelId, ProjectId, ProjectRegistryError,
    ProjectSnapshot, ProjectSnapshotError, TerminalId, TerminalRecord, ViewKey,
};
use uuid::Uuid;

use crate::gpui_terminal::view::TerminalView;
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent,
};

use super::AppRoot;
use super::commands::AppRootError;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};

pub(super) struct BatchTerminalMember {
    pub(super) record: TerminalRecord,
    pub(super) panel_id: PanelId,
    pub(super) terminal: Entity<TerminalView>,
}

/// 一个组内关闭操作只有一个协调器；成员映射只负责把显式重试路由回该操作。
pub(super) struct LocalTerminalBatchClose {
    pub(super) project_id: ProjectId,
    pub(super) group_id: PanelGroupId,
    pub(super) members: BTreeMap<TerminalId, BatchTerminalMember>,
    pub(super) coordinator: CloseCoordinator<String>,
    pub(super) persisted: Option<PersistedCloseIntent>,
    pub(super) stop_observations: BTreeMap<TerminalId, PtyStopObservation>,
    pub(super) outcome_unknown: bool,
    pub(super) initiation_sequence: u64,
    pub(super) unresolved_failure: bool,
}

#[derive(Clone, Copy)]
enum GroupCloseMode {
    Others,
    All,
}

impl AppRoot {
    pub(super) fn close_other_views_from_request(
        &mut self,
        group_id: PanelGroupId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.close_group_from_request(group_id, GroupCloseMode::Others, window, cx)
    }

    pub(super) fn close_all_views_from_request(
        &mut self,
        group_id: PanelGroupId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.close_group_from_request(group_id, GroupCloseMode::All, window, cx)
    }

    fn close_group_from_request(
        &mut self,
        group_id: PanelGroupId,
        mode: GroupCloseMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(terminal_id) = self.oldest_unresolved_terminal_close_failure() {
            return self.close_terminal(terminal_id, window, cx).map(|_| ());
        }
        if let Some(batch_id) = self.batch_for_group(group_id) {
            return self.retry_batch_close(batch_id, cx).map(|_| ());
        }
        self.ensure_terminal_close_recovery_allowed()?;
        self.ensure_project_snapshot_command_allowed()?;
        let store_epoch = self.terminal_close_store_epoch()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let group = project
            .layout()
            .group(group_id)
            .ok_or(slterm_workspace::LayoutError::GroupNotFound)?;
        let active = group.active().clone();
        let mut terminal_ids = group
            .views()
            .iter()
            .filter(|view| match mode {
                GroupCloseMode::Others => *view != &active,
                GroupCloseMode::All => true,
            })
            .map(|view| match view {
                ViewKey::Terminal { terminal_id, .. } => Ok(*terminal_id),
                _ => Err(AppRootError::MissingPanel),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if terminal_ids.is_empty() {
            return Ok(());
        }
        if matches!(mode, GroupCloseMode::Others) {
            // 保留项也必须是真实面板；不为未来 Document 构造占位宿主。
            self.panel_for_view(project_id, &active, cx)?;
        }
        terminal_ids.sort_unstable();
        terminal_ids.dedup();

        let mut members = BTreeMap::new();
        for terminal_id in &terminal_ids {
            if self.terminal_closes.contains_key(terminal_id)
                || self.terminal_close_batch_members.contains_key(terminal_id)
            {
                return Err(AppRootError::TerminalCloseRunning);
            }
            let record = self
                .snapshot
                .terminal(*terminal_id)
                .cloned()
                .ok_or(ProjectSnapshotError::MissingTerminal(*terminal_id))?;
            let (panel_id, terminal) = self
                .terminal_panel(project_id, *terminal_id, cx)
                .ok_or(AppRootError::MissingPanel)?;
            members.insert(
                *terminal_id,
                BatchTerminalMember {
                    record,
                    panel_id,
                    terminal,
                },
            );
        }
        let (candidate, _) = self.snapshot.without_terminals(&terminal_ids)?;
        let batch_id = Uuid::now_v7();
        let scope = CloseScope::Panels { project_id };
        let objects = terminal_ids
            .iter()
            .map(|terminal_id| CloseObject {
                id: terminal_id.to_string(),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        let fingerprint = store_epoch.as_ref().map_or([0; 32], |epoch| {
            close_fingerprint(&batch_id.to_string(), &scope, epoch, &objects)
        });
        let intent =
            CloseIntent::new(objects, fingerprint).map_err(|_| AppRootError::TerminalCloseState)?;
        let persisted = store_epoch.map(|store_epoch| PersistedCloseIntent {
            close_id: batch_id.to_string(),
            scope: scope.clone(),
            store_epoch,
            intent: intent.clone(),
            commit_status: CloseCommitStatus::Collected,
        });
        let mut coordinator = CloseCoordinator::new();
        let CloseEffect::Collect { attempt } = coordinator
            .begin(scope)
            .map_err(|_| AppRootError::TerminalCloseState)?
        else {
            unreachable!("批量关闭协调器必须从收集开始")
        };
        coordinator
            .collected(attempt, intent)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        let initiation_sequence = self.allocate_terminal_close_sequence()?;
        for terminal_id in &terminal_ids {
            self.terminal_close_batch_members
                .insert(*terminal_id, batch_id);
        }
        self.terminal_close_batches.insert(
            batch_id,
            LocalTerminalBatchClose {
                project_id,
                group_id,
                members,
                coordinator,
                persisted,
                stop_observations: BTreeMap::new(),
                outcome_unknown: false,
                initiation_sequence,
                unresolved_failure: false,
            },
        );
        if self.project_writer.is_some() {
            if let Err(error) = self.start_batch_commit(batch_id, candidate, window, cx) {
                self.remove_batch_registration(batch_id);
                return Err(error);
            }
        } else {
            self.complete_batch_state_commit(batch_id, None, window, cx)?;
            self.drive_batch_close_in_memory(batch_id, cx)?;
        }
        Ok(())
    }

    fn batch_for_group(&self, group_id: PanelGroupId) -> Option<Uuid> {
        self.terminal_close_batches
            .iter()
            .find(|(_, close)| close.group_id == group_id)
            .map(|(id, _)| *id)
    }

    /// 单终端入口遇到批量成员时只重试原批次，不生成第二条关闭链。
    pub(super) fn close_batch_member(
        &mut self,
        batch_id: Uuid,
        terminal_id: TerminalId,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        let record = self
            .terminal_close_batches
            .get(&batch_id)
            .and_then(|close| close.members.get(&terminal_id))
            .map(|member| member.record.clone())
            .ok_or(AppRootError::MissingPanel)?;
        self.retry_batch_close(batch_id, cx).map(|_| record)
    }

    fn retry_batch_close(
        &mut self,
        batch_id: Uuid,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return Err(AppRootError::MissingPanel);
        };
        let record = close
            .members
            .values()
            .next()
            .map(|member| member.record.clone())
            .ok_or(AppRootError::TerminalCloseState)?;
        if close.outcome_unknown {
            return Err(AppRootError::TerminalCloseOutcomeUnknown);
        }
        match close.coordinator.phase() {
            ClosePhase::Running(_) => return Err(AppRootError::TerminalCloseRunning),
            ClosePhase::Failed(_) => {}
            ClosePhase::Idle | ClosePhase::Completed => {
                return Err(AppRootError::TerminalCloseState);
            }
        }
        self.terminal_close_batches
            .get_mut(&batch_id)
            .expect("批量关闭刚读取必须仍存在")
            .coordinator
            .retry()
            .map_err(|_| AppRootError::TerminalCloseState)?;
        self.drive_batch_close(batch_id, cx)?;
        Ok(record)
    }

    pub(super) fn oldest_unresolved_batch_failure(&self) -> Option<(u64, TerminalId)> {
        self.terminal_close_batches
            .values()
            .filter(|close| close.unresolved_failure)
            .filter_map(|close| {
                close
                    .members
                    .keys()
                    .next()
                    .map(|terminal_id| (close.initiation_sequence, *terminal_id))
            })
            .min_by_key(|(sequence, _)| *sequence)
    }

    fn remove_batch_registration(&mut self, batch_id: Uuid) -> Option<LocalTerminalBatchClose> {
        let close = self.terminal_close_batches.remove(&batch_id)?;
        for terminal_id in close.members.keys() {
            self.terminal_close_batch_members.remove(terminal_id);
        }
        Some(close)
    }

    fn start_batch_commit(
        &mut self,
        batch_id: Uuid,
        candidate: ProjectSnapshot,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (writer, registration, attempt) = {
            let close = self
                .terminal_close_batches
                .get(&batch_id)
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
                self.remove_batch_registration(batch_id);
                self.resume_deferred_project_snapshot();
                return Err(AppRootError::TerminalCloseWriter(error));
            }
        };
        let window = window.window_handle();
        let executor = cx.background_executor().clone();
        let task_id = TaskId::TerminalCloseBatch(batch_id);
        let registration_result =
            self.tasks
                .register_with(task_id, CancellationToken::new(), move || {
                    let commit = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = commit.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_batch_commit(batch_id, attempt, result, window, cx);
                        });
                    })
                });
        if registration_result.is_err() {
            self.fail_batch_close_outcome_unknown(batch_id, attempt);
        }
        Ok(())
    }

    fn finish_batch_commit(
        &mut self,
        batch_id: Uuid,
        attempt: CloseAttempt,
        result: Result<Result<PersistedCloseIntent, CloseCommitError>, std::sync::mpsc::RecvError>,
        window: AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
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
        self.tasks.complete(TaskId::TerminalCloseBatch(batch_id));
        match result {
            Ok(Ok(committed)) => {
                let mut completed = false;
                let updated = window.update(cx, |_, window, cx| {
                    match self.complete_batch_state_commit(batch_id, Some(committed), window, cx) {
                        Ok(()) => completed = true,
                        Err(error) => {
                            log::warn!("batch close state transition failed: {error}");
                            self.fail_batch_close_outcome_unknown(batch_id, attempt);
                        }
                    }
                });
                if updated.is_err() {
                    self.fail_batch_close_outcome_unknown(batch_id, attempt);
                } else if completed {
                    let _ = self.drive_batch_close_persisted(batch_id, cx);
                    cx.notify();
                }
            }
            Ok(Err(error)) => {
                if let Some(close) = self.terminal_close_batches.get_mut(&batch_id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                self.remove_batch_registration(batch_id);
                self.resume_deferred_project_snapshot();
                log::warn!("batch close atomic commit failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
            }
            Err(_) => {
                log::warn!("batch close atomic commit outcome is unknown");
                self.fail_batch_close_outcome_unknown(batch_id, attempt);
                super::events::notify_workspace_write_failed(cx);
            }
        }
    }

    /// 持久提交成功后从最新快照重算，保留提交期间其他内存变化。
    fn complete_batch_state_commit(
        &mut self,
        batch_id: Uuid,
        committed: Option<PersistedCloseIntent>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), AppRootError> {
        let (project_id, terminal_ids, panel_ids) = {
            let close = self
                .terminal_close_batches
                .get(&batch_id)
                .ok_or(AppRootError::MissingPanel)?;
            (
                close.project_id,
                close.members.keys().copied().collect::<Vec<_>>(),
                close
                    .members
                    .values()
                    .map(|member| member.panel_id)
                    .collect::<Vec<_>>(),
            )
        };
        let (candidate, _) = self.snapshot.without_terminals(&terminal_ids)?;
        let divider_focus_removed =
            self.focused_split_divider_removed(project_id, &candidate, window, cx);
        let preferred_panel = self.preferred_close_panel(project_id, &panel_ids, &candidate, cx);
        let attempt = self
            .terminal_close_batches
            .get(&batch_id)
            .and_then(|close| close.coordinator.current_attempt())
            .ok_or(AppRootError::TerminalCloseState)?;
        let close = self
            .terminal_close_batches
            .get_mut(&batch_id)
            .ok_or(AppRootError::MissingPanel)?;
        close
            .coordinator
            .state_committed(attempt)
            .map_err(|_| AppRootError::TerminalCloseState)?;
        if let Some(committed) = committed {
            if committed.commit_status != CloseCommitStatus::StateCommitted {
                return Err(AppRootError::TerminalCloseState);
            }
            close.persisted = Some(committed);
        }
        self.commit_project_snapshot(candidate, cx);
        self.detach_batch_terminals(batch_id, preferred_panel, divider_focus_removed, window, cx);
        Ok(())
    }

    fn drive_batch_close(
        &mut self,
        batch_id: Uuid,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self
            .terminal_close_batches
            .get(&batch_id)
            .is_some_and(|close| close.persisted.is_some())
        {
            self.drive_batch_close_persisted(batch_id, cx)
        } else {
            self.drive_batch_close_in_memory(batch_id, cx)
        }
    }

    fn drive_batch_close_in_memory(
        &mut self,
        batch_id: Uuid,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return Ok(());
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observations.len() == close.members.len() {
                    let attempt = close
                        .coordinator
                        .current_attempt()
                        .ok_or(AppRootError::TerminalCloseState)?;
                    self.terminal_close_batches
                        .get_mut(&batch_id)
                        .expect("批量关闭刚读取必须仍存在")
                        .coordinator
                        .pty_stopped(attempt)
                        .map_err(|_| AppRootError::TerminalCloseState)?;
                    self.drive_batch_close_in_memory(batch_id, cx)
                } else if close.stop_observations.is_empty() {
                    self.start_batch_stop(batch_id, cx);
                    Ok(())
                } else {
                    Err(AppRootError::TerminalCloseState)
                }
            }
            ClosePhase::Running(CloseStage::Cleanup) => {
                let attempt = close
                    .coordinator
                    .current_attempt()
                    .ok_or(AppRootError::TerminalCloseState)?;
                self.terminal_close_batches
                    .get_mut(&batch_id)
                    .expect("批量关闭刚读取必须仍存在")
                    .coordinator
                    .recovery_cleaned(attempt)
                    .map_err(|_| AppRootError::TerminalCloseState)?;
                self.destroy_closed_batch(batch_id, cx);
                Ok(())
            }
            ClosePhase::Running(CloseStage::CommitState)
            | ClosePhase::Idle
            | ClosePhase::Failed(_)
            | ClosePhase::Completed => Ok(()),
            ClosePhase::Running(_) => Err(AppRootError::TerminalCloseState),
        }
    }

    fn drive_batch_close_persisted(
        &mut self,
        batch_id: Uuid,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return Ok(());
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observations.len() == close.members.len() {
                    self.start_batch_advance(batch_id, CloseCommitStatus::PtyStopped, cx);
                } else if close.stop_observations.is_empty() {
                    self.start_batch_stop(batch_id, cx);
                } else {
                    return Err(AppRootError::TerminalCloseState);
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
                        self.start_batch_advance(batch_id, CloseCommitStatus::CleanupCommitted, cx);
                        Ok(())
                    }
                    CloseCommitStatus::CleanupCommitted => {
                        self.start_batch_advance(batch_id, CloseCommitStatus::Completed, cx);
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

    /// 全部 session 在后台依次 join；每个成员的已消费事实只由显式重试复用。
    fn start_batch_stop(&mut self, batch_id: Uuid, cx: &mut Context<'_, Self>) {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return;
        };
        let Some(attempt) = close.coordinator.current_attempt() else {
            return;
        };
        let terminals = close
            .members
            .iter()
            .map(|(terminal_id, member)| (*terminal_id, member.terminal.clone()))
            .collect::<Vec<_>>();
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::TerminalCloseBatch(batch_id),
            CancellationToken::new(),
            move || {
                let sessions = terminals
                    .into_iter()
                    .map(|(terminal_id, terminal)| {
                        let session =
                            terminal.update(cx, |terminal, _| terminal.take_session_for_shutdown());
                        (terminal_id, session)
                    })
                    .collect::<Vec<_>>();
                let stop = executor.spawn(async move {
                    sessions
                        .into_iter()
                        .map(|(terminal_id, session)| {
                            let result = match session {
                                Some(session) => session.shutdown(),
                                None => Ok(()),
                            };
                            (terminal_id, result)
                        })
                        .collect::<Vec<_>>()
                });
                cx.spawn(async move |this, cx| {
                    let result = stop.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_batch_stop(batch_id, attempt, result, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            log::debug!("batch terminal stop already scheduled");
        }
    }

    fn finish_batch_stop(
        &mut self,
        batch_id: Uuid,
        attempt: CloseAttempt,
        result: Vec<(TerminalId, std::thread::Result<()>)>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        self.tasks.complete(TaskId::TerminalCloseBatch(batch_id));
        let close = self
            .terminal_close_batches
            .get_mut(&batch_id)
            .expect("批量关闭刚读取必须仍存在");
        let mut failed = false;
        for (terminal_id, observation) in result {
            if !close.members.contains_key(&terminal_id) {
                failed = true;
                continue;
            }
            close.stop_observations.insert(
                terminal_id,
                match observation {
                    Ok(()) => PtyStopObservation::Stopped,
                    Err(_) => {
                        failed = true;
                        PtyStopObservation::Panicked
                    }
                },
            );
        }
        if close.stop_observations.len() != close.members.len() {
            failed = true;
        }
        if failed {
            close.unresolved_failure = true;
            let _ = close.coordinator.fail(attempt);
            log::warn!("batch terminal PTY worker terminated abnormally");
            cx.notify();
            return;
        }
        let _ = self.drive_batch_close(batch_id, cx);
    }

    fn start_batch_advance(
        &mut self,
        batch_id: Uuid,
        next: CloseCommitStatus,
        cx: &mut Context<'_, Self>,
    ) {
        let (writer, current, attempt) = {
            let Some(close) = self.terminal_close_batches.get(&batch_id) else {
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
                if let Some(close) = self.terminal_close_batches.get_mut(&batch_id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("batch close advance request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::TerminalCloseBatch(batch_id),
            CancellationToken::new(),
            move || {
                let advance = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = advance.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_batch_advance(batch_id, attempt, next, result, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            self.fail_batch_close_outcome_unknown(batch_id, attempt);
        }
    }

    fn finish_batch_advance(
        &mut self,
        batch_id: Uuid,
        attempt: CloseAttempt,
        next: CloseCommitStatus,
        result: Result<Result<PersistedCloseIntent, CloseStoreError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
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
        self.tasks.complete(TaskId::TerminalCloseBatch(batch_id));
        match result {
            Ok(Ok(updated)) if updated.commit_status == next => {
                let close = self
                    .terminal_close_batches
                    .get_mut(&batch_id)
                    .expect("批量关闭刚读取必须仍存在");
                close.persisted = Some(updated);
                match next {
                    CloseCommitStatus::PtyStopped => {
                        if close.coordinator.pty_stopped(attempt).is_err() {
                            self.fail_batch_close_outcome_unknown(batch_id, attempt);
                            return;
                        }
                    }
                    CloseCommitStatus::CleanupCommitted => {}
                    CloseCommitStatus::Completed => {
                        if close.coordinator.recovery_cleaned(attempt).is_err() {
                            self.fail_batch_close_outcome_unknown(batch_id, attempt);
                            return;
                        }
                        self.destroy_closed_batch(batch_id, cx);
                        return;
                    }
                    _ => {
                        self.fail_batch_close_outcome_unknown(batch_id, attempt);
                        return;
                    }
                }
                let _ = self.drive_batch_close_persisted(batch_id, cx);
            }
            Ok(Ok(_)) => self.fail_batch_close_outcome_unknown(batch_id, attempt),
            Ok(Err(error)) => {
                if let Some(close) = self.terminal_close_batches.get_mut(&batch_id) {
                    close.unresolved_failure = true;
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("batch close advancement failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
            }
            Err(_) => {
                log::warn!("batch close advancement outcome is unknown");
                self.fail_batch_close_outcome_unknown(batch_id, attempt);
                super::events::notify_workspace_write_failed(cx);
            }
        }
    }

    fn fail_batch_close_outcome_unknown(&mut self, batch_id: Uuid, attempt: CloseAttempt) {
        if let Some(close) = self.terminal_close_batches.get_mut(&batch_id) {
            close.outcome_unknown = true;
            close.unresolved_failure = true;
            let _ = close.coordinator.fail(attempt);
        }
    }

    fn detach_batch_terminals(
        &mut self,
        batch_id: Uuid,
        preferred_panel: Option<PanelId>,
        divider_focus_removed: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(close) = self.terminal_close_batches.get(&batch_id) else {
            return;
        };
        let panels = close
            .members
            .values()
            .map(|member| (member.panel_id, member.terminal.clone()))
            .collect::<Vec<_>>();
        self.detach_terminal_panels(
            close.project_id,
            &panels,
            preferred_panel,
            divider_focus_removed,
            window,
            cx,
        );
    }

    fn destroy_closed_batch(&mut self, batch_id: Uuid, cx: &mut Context<'_, Self>) {
        let Some(close) = self.remove_batch_registration(batch_id) else {
            return;
        };
        let panel_ids = close
            .members
            .values()
            .map(|member| member.panel_id)
            .collect::<BTreeSet<_>>();
        if let Some(project) = self.projects.get(&close.project_id) {
            project.update(cx, |project, _| {
                project
                    .panels
                    .retain(|panel_id, _| !panel_ids.contains(panel_id));
                if project
                    .focused_panel
                    .is_some_and(|panel_id| panel_ids.contains(&panel_id))
                {
                    project.focused_panel = None;
                }
            });
        }
        cx.notify();
    }
}
