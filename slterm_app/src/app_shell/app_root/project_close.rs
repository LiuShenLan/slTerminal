//! 项目移除：删除意图提交后停止所属 PTY，再清理项目事实并销毁运行实体。

use gpui::{App, Context, Entity, Window};
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage, ProjectId, ProjectSnapshot,
};

use super::commands::AppRootError;
use super::project_view::ProjectView;
use super::side_view::side_view_state_registry;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};
use super::{AppRoot, SideViewId};
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent, ProjectCleanupError,
};

pub(super) struct LocalProjectClose {
    pub(super) project: Entity<ProjectView>,
    pub(super) coordinator: CloseCoordinator<String>,
    pub(super) persisted: PersistedCloseIntent,
    pub(super) stop_observation: Option<PtyStopObservation>,
    pub(super) outcome_unknown: bool,
}

impl AppRoot {
    /// 同一入口既发起项目移除，也显式重试已明确的失败阶段。
    pub(crate) fn remove_project(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(close) = self.project_closes.get(&id) {
            return match close.coordinator.phase() {
                ClosePhase::Running(_) => Err(AppRootError::ProjectCloseRunning),
                ClosePhase::Failed(_) => {
                    self.retry_project_close(id, cx)?;
                    Ok(())
                }
                ClosePhase::Idle | ClosePhase::Completed => Err(AppRootError::ProjectCloseState),
            };
        }
        if !self.project_closes.is_empty()
            || self
                .terminal_closes
                .values()
                .any(|close| close.record.project_id() == id)
        {
            return Err(AppRootError::ProjectCloseRunning);
        }
        self.ensure_terminal_close_recovery_allowed()?;
        self.ensure_project_snapshot_command_allowed()?;
        let store_epoch = self
            .terminal_close_store_epoch()?
            .ok_or(AppRootError::WorkspaceEpochUnavailable)?;
        let project = self
            .projects
            .get(&id)
            .cloned()
            .ok_or(AppRootError::MissingPanel)?;
        if self.snapshot.registry().active() == Some(id) {
            self.focus_manager.ensure_activation_allowed()?;
        }
        self.sync_focus(window, cx);
        let (candidate, _) = self.snapshot.without_project(id)?;
        let scope = CloseScope::Project { project_id: id };
        let objects = self
            .snapshot
            .terminals()
            .filter(|terminal| terminal.project_id() == id)
            .map(|terminal| CloseObject {
                id: terminal.id().to_string(),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        let close_id = uuid::Uuid::now_v7().to_string();
        let fingerprint = close_fingerprint(&close_id, &scope, &store_epoch, &objects);
        let intent =
            CloseIntent::new(objects, fingerprint).map_err(|_| AppRootError::ProjectCloseState)?;
        let registration = PersistedCloseIntent {
            close_id,
            scope: scope.clone(),
            store_epoch,
            intent,
            commit_status: CloseCommitStatus::Collected,
        };
        let mut coordinator = CloseCoordinator::new();
        let CloseEffect::Collect { attempt } = coordinator
            .begin(scope)
            .map_err(|_| AppRootError::ProjectCloseState)?
        else {
            unreachable!("项目关闭协调器必须从收集开始")
        };
        coordinator
            .collected(attempt, registration.intent.clone())
            .map_err(|_| AppRootError::ProjectCloseState)?;
        self.project_closes.insert(
            id,
            LocalProjectClose {
                project,
                coordinator,
                persisted: registration,
                stop_observation: None,
                outcome_unknown: false,
            },
        );
        self.start_project_commit(id, candidate, window, cx)?;
        Ok(())
    }

    fn retry_project_close(
        &mut self,
        id: ProjectId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let close = self
            .project_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        if close.outcome_unknown {
            return Err(AppRootError::ProjectCloseOutcomeUnknown);
        }
        close
            .coordinator
            .retry()
            .map_err(|_| AppRootError::ProjectCloseState)?;
        self.drive_project_close(id, cx)
    }

    fn start_project_commit(
        &mut self,
        id: ProjectId,
        candidate: ProjectSnapshot,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (writer, registration, attempt) = {
            let close = self
                .project_closes
                .get(&id)
                .ok_or(AppRootError::MissingPanel)?;
            let writer = self
                .project_writer
                .clone()
                .ok_or(AppRootError::WorkspaceEpochUnavailable)?;
            (
                writer,
                close.persisted.clone(),
                close
                    .coordinator
                    .current_attempt()
                    .ok_or(AppRootError::ProjectCloseState)?,
            )
        };
        let response = match writer.commit_project_close_intent(&candidate, &registration) {
            Ok(response) => response,
            Err(error) => {
                self.project_closes.remove(&id);
                self.resume_deferred_project_snapshot();
                return Err(AppRootError::ProjectCloseWriter(error));
            }
        };
        let window = window.window_handle();
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::ProjectClose(id),
            CancellationToken::new(),
            move || {
                let commit = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = commit.await;
                    let completion = this.clone();
                    let updated = window.downcast::<gpui_component::Root>().map(|window| {
                        window.update(cx, |_, window, cx| {
                            completion.update(cx, |root, cx| {
                                root.finish_project_commit(id, attempt, result, window, cx);
                            })
                        })
                    });
                    if !matches!(updated, Some(Ok(Ok(())))) {
                        let _ = this.update(cx, |root, _cx| {
                            root.fail_project_close_outcome_unknown(id, attempt);
                        });
                    }
                })
            },
        );
        if registration.is_err() {
            self.fail_project_close_outcome_unknown(id, attempt);
        }
        Ok(())
    }

    pub(super) fn finish_project_commit(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        result: Result<Result<PersistedCloseIntent, CloseCommitError>, std::sync::mpsc::RecvError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        if let Ok(Ok(committed)) = &result
            && (committed.commit_status != CloseCommitStatus::StateCommitted
                || !same_close_fact(&close.persisted, committed))
        {
            return;
        }
        self.tasks.complete(TaskId::ProjectClose(id));
        match result {
            Ok(Ok(committed)) => {
                match self.complete_project_state_commit(id, attempt, committed, window, cx) {
                    Ok(()) => {
                        let _ = self.drive_project_close(id, cx);
                        cx.notify();
                    }
                    Err(error) => {
                        log::warn!("project close state transition failed: {error}");
                        self.fail_project_close_outcome_unknown(id, attempt);
                    }
                }
            }
            Ok(Err(error)) => {
                if let Some(close) = self.project_closes.get_mut(&id) {
                    let _ = close.coordinator.fail(attempt);
                }
                self.project_closes.remove(&id);
                self.resume_deferred_project_snapshot();
                log::warn!("project close atomic commit failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                cx.notify();
            }
            Err(_) => {
                log::warn!("project close atomic commit outcome is unknown");
                self.fail_project_close_outcome_unknown(id, attempt);
                cx.notify();
            }
        }
    }

    fn complete_project_state_commit(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        committed: PersistedCloseIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if committed.commit_status != CloseCommitStatus::StateCommitted {
            return Err(AppRootError::ProjectCloseState);
        }
        let (candidate, _) = self.snapshot.without_project(id)?;
        let was_active = self.snapshot.registry().active() == Some(id);
        let next_active = candidate.registry().active();
        let close = self
            .project_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        close
            .coordinator
            .state_committed(attempt)
            .map_err(|_| AppRootError::ProjectCloseState)?;
        close.persisted = committed;
        self.commit_project_snapshot(candidate, cx);
        if was_active && let Some(next_active) = next_active {
            self.restore_project_panels(next_active, window, cx);
        }
        self.detach_closing_project(id, was_active, next_active, window, cx);
        Ok(())
    }

    fn detach_closing_project(
        &mut self,
        id: ProjectId,
        was_active: bool,
        next_active: Option<ProjectId>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        let project = close.project.clone();
        let panel_ids = project.read(cx).panels.keys().copied().collect::<Vec<_>>();
        self.projects.remove(&id);
        if was_active {
            self.focus_manager.restore_active_project(next_active);
            self.sync_focus_visibility(next_active, cx);
            let mut candidates = next_active
                .and_then(|active| self.project_default_focus(active, cx).ok())
                .into_iter()
                .collect::<Vec<_>>();
            candidates.push(self.focus_manager.workspace_token());
            if let Err(error) = self.focus_manager.restore_focus(&candidates, window, cx) {
                log::warn!("project close focus restoration failed: {error:?}");
                window.blur();
                self.focus_manager.sync_window_focus(window, cx);
            }
        }
        for panel_id in panel_ids {
            if let Some(registration) = self.panel_focus.remove(&panel_id)
                && let Err(error) = self.focus_manager.unregister(registration.token())
            {
                log::warn!("project panel focus unregister failed: {error:?}");
                let _ = self.focus_manager.set_visible(registration.token(), false);
            }
            self.panel_subscriptions.remove(&panel_id);
        }
        let focus_registration = project.update(cx, |project, _| project.focus_registration.take());
        if let Some(registration) = focus_registration
            && let Err(error) = self.focus_manager.unregister(registration.token())
        {
            log::warn!("project focus unregister failed: {error:?}");
            let _ = self.focus_manager.set_visible(registration.token(), false);
        }
        self.sync_project_visibility(cx);
    }

    fn drive_project_close(
        &mut self,
        id: ProjectId,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(close) = self.project_closes.get(&id) else {
            return Ok(());
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observation.is_some() {
                    self.start_project_advance(id, CloseCommitStatus::PtyStopped, cx);
                } else {
                    self.start_project_stop(id, cx);
                }
                Ok(())
            }
            ClosePhase::Running(CloseStage::Cleanup) => {
                match close.persisted.commit_status {
                    CloseCommitStatus::PtyStopped => self.start_project_cleanup(id, cx),
                    CloseCommitStatus::CleanupCommitted => {
                        self.start_project_advance(id, CloseCommitStatus::Completed, cx)
                    }
                    _ => return Err(AppRootError::ProjectCloseState),
                }
                Ok(())
            }
            ClosePhase::Running(CloseStage::CommitState)
            | ClosePhase::Idle
            | ClosePhase::Failed(_)
            | ClosePhase::Completed => Ok(()),
            ClosePhase::Running(_) => Err(AppRootError::ProjectCloseState),
        }
    }

    fn start_project_stop(&mut self, id: ProjectId, cx: &mut Context<'_, Self>) {
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        let Some(attempt) = close.coordinator.current_attempt() else {
            return;
        };
        let project = close.project.clone();
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::ProjectClose(id),
            CancellationToken::new(),
            move || {
                let sessions = project.update(cx, |project, cx| {
                    project
                        .panels
                        .values()
                        .map(|panel| {
                            panel
                                .terminal
                                .update(cx, |terminal, _| terminal.take_session_for_shutdown())
                        })
                        .collect::<Vec<_>>()
                });
                let stop = executor.spawn(async move {
                    let mut observation = PtyStopObservation::Stopped;
                    for session in sessions.into_iter().flatten() {
                        if session.shutdown().is_err() {
                            observation = PtyStopObservation::Panicked;
                        }
                    }
                    observation
                });
                cx.spawn(async move |this, cx| {
                    let observation = stop.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_project_stop(id, attempt, observation, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            // 重复停止调度保留原任务与原回执，不消费另一份会话。
            log::debug!("project stop already scheduled");
        }
    }

    pub(super) fn finish_project_stop(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        observation: PtyStopObservation,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        self.tasks.complete(TaskId::ProjectClose(id));
        let close = self
            .project_closes
            .get_mut(&id)
            .expect("项目关闭刚读取必须仍存在");
        close.stop_observation = Some(observation);
        if observation == PtyStopObservation::Panicked {
            let _ = close.coordinator.fail(attempt);
            log::warn!("project PTY worker terminated abnormally");
            cx.notify();
            return;
        }
        let _ = self.drive_project_close(id, cx);
    }

    fn start_project_advance(
        &mut self,
        id: ProjectId,
        next: CloseCommitStatus,
        cx: &mut Context<'_, Self>,
    ) {
        debug_assert!(matches!(
            next,
            CloseCommitStatus::PtyStopped | CloseCommitStatus::Completed
        ));
        let (writer, current, attempt) = {
            let Some(close) = self.project_closes.get(&id) else {
                return;
            };
            let Some(attempt) = close.coordinator.current_attempt() else {
                return;
            };
            let Some(writer) = self.project_writer.clone() else {
                return;
            };
            (writer, close.persisted.clone(), attempt)
        };
        let response = match writer.advance_close_intent(&current, next) {
            Ok(response) => response,
            Err(error) => {
                if let Some(close) = self.project_closes.get_mut(&id) {
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("project close advance request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::ProjectClose(id),
            CancellationToken::new(),
            move || {
                let advance = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = advance.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_project_advance(id, attempt, next, result, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            self.fail_project_close_outcome_unknown(id, attempt);
        }
    }

    pub(super) fn finish_project_advance(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        next: CloseCommitStatus,
        result: Result<Result<PersistedCloseIntent, CloseStoreError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let expected_previous = match next {
            CloseCommitStatus::PtyStopped => CloseCommitStatus::StateCommitted,
            CloseCommitStatus::Completed => CloseCommitStatus::CleanupCommitted,
            _ => return,
        };
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt)
            || close.persisted.commit_status != expected_previous
        {
            return;
        }
        if let Ok(Ok(updated)) = &result
            && (updated.commit_status != next || !same_close_fact(&close.persisted, updated))
        {
            return;
        }
        self.tasks.complete(TaskId::ProjectClose(id));
        match result {
            Ok(Ok(updated)) => {
                let close = self
                    .project_closes
                    .get_mut(&id)
                    .expect("项目关闭刚读取必须仍存在");
                let transition = if next == CloseCommitStatus::PtyStopped {
                    close.coordinator.pty_stopped(attempt)
                } else {
                    close.coordinator.recovery_cleaned(attempt)
                };
                if transition.is_err() {
                    self.fail_project_close_outcome_unknown(id, attempt);
                    return;
                }
                close.persisted = updated;
                if next == CloseCommitStatus::Completed {
                    self.destroy_closed_project(id, cx);
                } else {
                    let _ = self.drive_project_close(id, cx);
                }
            }
            Ok(Err(error)) => {
                if let Some(close) = self.project_closes.get_mut(&id) {
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("project close advancement failed: {error}");
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => self.fail_project_close_outcome_unknown(id, attempt),
        }
        cx.notify();
    }

    fn start_project_cleanup(&mut self, id: ProjectId, cx: &mut Context<'_, Self>) {
        let (writer, current, attempt) = {
            let Some(close) = self.project_closes.get(&id) else {
                return;
            };
            let Some(attempt) = close.coordinator.current_attempt() else {
                return;
            };
            let Some(writer) = self.project_writer.clone() else {
                return;
            };
            (writer, close.persisted.clone(), attempt)
        };
        let response = match writer.cleanup_project_close(&current, side_view_state_registry()) {
            Ok(response) => response,
            Err(error) => {
                if let Some(close) = self.project_closes.get_mut(&id) {
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("project cleanup request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::ProjectClose(id),
            CancellationToken::new(),
            move || {
                let cleanup = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = cleanup.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_project_cleanup(id, attempt, result, cx);
                    });
                })
            },
        );
        if registration.is_err() {
            self.fail_project_close_outcome_unknown(id, attempt);
        }
    }

    pub(super) fn finish_project_cleanup(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        result: Result<
            Result<PersistedCloseIntent, ProjectCleanupError>,
            std::sync::mpsc::RecvError,
        >,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.project_closes.get(&id) else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt)
            || close.persisted.commit_status != CloseCommitStatus::PtyStopped
        {
            return;
        }
        if let Ok(Ok(updated)) = &result
            && (updated.commit_status != CloseCommitStatus::CleanupCommitted
                || !same_close_fact(&close.persisted, updated))
        {
            return;
        }
        self.tasks.complete(TaskId::ProjectClose(id));
        match result {
            Ok(Ok(updated)) => {
                let close = self
                    .project_closes
                    .get_mut(&id)
                    .expect("项目关闭刚读取必须仍存在");
                close.persisted = updated;
                for side_view_id in [SideViewId::Nav, SideViewId::Explorer] {
                    if let Some(view) = self.side_views.instance(side_view_id) {
                        view.update(cx, |view, _| view.remove_project(id));
                    }
                }
                let _ = self.drive_project_close(id, cx);
            }
            Ok(Err(error)) => {
                if let Some(close) = self.project_closes.get_mut(&id) {
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("project cleanup failed: {error}");
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => self.fail_project_close_outcome_unknown(id, attempt),
        }
        cx.notify();
    }

    fn fail_project_close_outcome_unknown(&mut self, id: ProjectId, attempt: CloseAttempt) {
        if let Some(close) = self.project_closes.get_mut(&id) {
            close.outcome_unknown = true;
            let _ = close.coordinator.fail(attempt);
        }
    }

    fn destroy_closed_project(&mut self, id: ProjectId, cx: &mut Context<'_, Self>) {
        self.project_closes.remove(&id);
        self.resume_deferred_project_snapshot();
        cx.notify();
    }
}
