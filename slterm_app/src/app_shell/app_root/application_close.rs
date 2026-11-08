//! 应用退出的局部关闭段：登记意图、停止全部会话、提交清理完成。

use gpui::{Context, Window};
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage,
};

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent,
};

pub(super) struct ApplicationClose {
    pub(super) coordinator: CloseCoordinator<String>,
    pub(super) persisted: PersistedCloseIntent,
    pub(super) stop_observation: Option<PtyStopObservation>,
    pub(super) outcome_unknown: bool,
    pub(super) window: gpui::AnyWindowHandle,
}

impl AppRoot {
    pub(super) fn start_application_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        if !self.terminal_closes.is_empty() || !self.project_closes.is_empty() {
            return false;
        }
        if self.application_close.is_some() {
            self.retry_application_close(cx);
            return false;
        }
        if self.close_in_progress {
            return false;
        }
        let Some(writer) = self.project_writer.clone() else {
            self.close_in_progress = true;
            self.close_completed = true;
            window.remove_window();
            return true;
        };
        if self.project_snapshot_recovery_failed {
            self.close_in_progress = true;
            return self.start_close_flush(writer, window.window_handle(), cx);
        }
        let Some(store_epoch) = self.store_epoch.clone().filter(|epoch| !epoch.is_empty()) else {
            log::warn!("application close rejected without current store epoch");
            return false;
        };
        let scope = CloseScope::ApplicationExit;
        let objects = self
            .snapshot
            .terminals()
            .map(|terminal| CloseObject {
                id: terminal.id().to_string(),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        let close_id = uuid::Uuid::now_v7().to_string();
        let fingerprint = close_fingerprint(&close_id, &scope, &store_epoch, &objects);
        let intent = match CloseIntent::new(objects, fingerprint) {
            Ok(intent) => intent,
            Err(_) => return false,
        };
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
            .expect("空闲应用关闭协调器必须能开始")
        else {
            unreachable!()
        };
        if coordinator
            .collected(attempt, registration.intent.clone())
            .is_err()
        {
            return false;
        }
        self.application_close = Some(ApplicationClose {
            coordinator,
            persisted: registration,
            stop_observation: None,
            outcome_unknown: false,
            window: window.window_handle(),
        });
        self.close_in_progress = true;
        self.sidebar_drag = None;
        let snapshot = self.snapshot.clone();
        if let Err(error) =
            self.side_view_persistence
                .submit_final_states(&self.side_views, &snapshot, cx)
        {
            self.application_close = None;
            self.close_in_progress = false;
            self.side_view_persistence
                .resume_checkpoint_for_close_retry(&self.side_views, cx);
            log::warn!("application close final-state handoff failed: {error}");
            super::events::notify_workspace_write_failed(cx);
            return false;
        }
        self.start_application_commit(writer, snapshot, cx);
        false
    }

    fn retry_application_close(&mut self, cx: &mut Context<'_, Self>) {
        let Some(close) = self.application_close.as_mut() else {
            return;
        };
        if close.outcome_unknown {
            return;
        }
        if matches!(close.coordinator.phase(), ClosePhase::Running(_)) {
            let resume = !self.close_in_progress;
            self.close_in_progress = true;
            if resume {
                self.drive_application_close(cx);
            }
            return;
        }
        if matches!(close.coordinator.phase(), ClosePhase::Failed(_))
            && close.coordinator.retry().is_err()
        {
            return;
        }
        self.close_in_progress = true;
        self.drive_application_close(cx);
    }

    fn start_application_commit(
        &mut self,
        writer: crate::persistence::workspace_writer::WorkspaceWriterClient,
        snapshot: slterm_workspace::ProjectSnapshot,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        let registration = close.persisted.clone();
        let attempt = close
            .coordinator
            .current_attempt()
            .expect("提交阶段必须有 attempt");
        let response = match writer.commit_close_intent(&snapshot, &registration) {
            Ok(response) => response,
            Err(error) => {
                self.application_close = None;
                self.close_in_progress = false;
                self.side_view_persistence
                    .resume_checkpoint_for_close_retry(&self.side_views, cx);
                log::warn!("application close commit request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let commit = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = commit.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_application_commit(attempt, result, cx);
                        });
                    })
                });
        if registration.is_err() {
            self.fail_application_close_unknown(attempt);
        }
    }

    pub(super) fn finish_application_commit(
        &mut self,
        attempt: CloseAttempt,
        result: Result<Result<PersistedCloseIntent, CloseCommitError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
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
        self.tasks.complete(TaskId::CloseFlush);
        match result {
            Ok(Ok(committed)) => {
                let close = self.application_close.as_mut().expect("关闭记录必须存在");
                if close.coordinator.state_committed(attempt).is_err() {
                    self.fail_application_close_unknown(attempt);
                    return;
                }
                close.persisted = committed;
                self.drive_application_close(cx);
            }
            Ok(Err(error)) => {
                self.application_close = None;
                self.close_in_progress = false;
                self.side_view_persistence
                    .resume_checkpoint_for_close_retry(&self.side_views, cx);
                log::warn!("application close commit failed: {error}");
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => self.fail_application_close_unknown(attempt),
        }
        cx.notify();
    }

    fn drive_application_close(&mut self, cx: &mut Context<'_, Self>) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        match close.coordinator.phase() {
            ClosePhase::Running(CloseStage::StopPty) => {
                if close.stop_observation.is_some() {
                    self.start_application_advance(CloseCommitStatus::PtyStopped, cx);
                } else {
                    self.start_application_stop(cx);
                }
            }
            ClosePhase::Running(CloseStage::Cleanup) => match close.persisted.commit_status {
                CloseCommitStatus::PtyStopped => {
                    self.start_application_advance(CloseCommitStatus::CleanupCommitted, cx)
                }
                CloseCommitStatus::CleanupCommitted => {
                    self.start_application_advance(CloseCommitStatus::Completed, cx)
                }
                _ => self.fail_application_close_unknown(
                    close
                        .coordinator
                        .current_attempt()
                        .expect("清理阶段必须有 attempt"),
                ),
            },
            ClosePhase::Running(CloseStage::StopServices) => {
                let window = close.window;
                if let Some(writer) = self.project_writer.clone() {
                    self.start_close_flush(writer, window, cx);
                }
            }
            _ => {}
        }
    }

    fn start_application_stop(&mut self, cx: &mut Context<'_, Self>) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        let attempt = close
            .coordinator
            .current_attempt()
            .expect("停止阶段必须有 attempt");
        let projects = self.projects.values().cloned().collect::<Vec<_>>();
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let sessions = projects
                        .into_iter()
                        .flat_map(|project| {
                            project.update(cx, |project, cx| {
                                project
                                    .panels
                                    .values()
                                    .map(|panel| {
                                        panel.terminal.update(cx, |terminal, _| {
                                            terminal.take_session_for_shutdown()
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect::<Vec<_>>();
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
                            root.finish_application_stop(attempt, observation, cx);
                        });
                    })
                });
        if registration.is_err() {
            log::debug!("application stop already scheduled");
        }
    }

    pub(super) fn finish_application_stop(
        &mut self,
        attempt: CloseAttempt,
        observation: PtyStopObservation,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        self.tasks.complete(TaskId::CloseFlush);
        let close = self.application_close.as_mut().expect("关闭记录必须存在");
        close.stop_observation = Some(observation);
        if observation == PtyStopObservation::Panicked {
            let _ = close.coordinator.fail(attempt);
            self.close_in_progress = false;
            log::warn!("application PTY worker terminated abnormally");
            cx.notify();
            return;
        }
        self.drive_application_close(cx);
    }

    pub(super) fn start_application_advance(
        &mut self,
        next: CloseCommitStatus,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        let attempt = close
            .coordinator
            .current_attempt()
            .expect("推进阶段必须有 attempt");
        let current = close.persisted.clone();
        let Some(writer) = self.project_writer.clone() else {
            return;
        };
        let response = match writer.advance_close_intent(&current, next) {
            Ok(response) => response,
            Err(error) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.close_in_progress = false;
                log::warn!("application close advance request failed: {error}");
                super::events::notify_workspace_write_failed(cx);
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let advance = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = advance.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_application_advance(attempt, next, result, cx);
                        });
                    })
                });
        if registration.is_err() {
            self.fail_application_close_unknown(attempt);
        }
    }

    pub(super) fn finish_application_advance(
        &mut self,
        attempt: CloseAttempt,
        next: CloseCommitStatus,
        result: Result<Result<PersistedCloseIntent, CloseStoreError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let expected_previous = match next {
            CloseCommitStatus::PtyStopped => CloseCommitStatus::StateCommitted,
            CloseCommitStatus::CleanupCommitted => CloseCommitStatus::PtyStopped,
            CloseCommitStatus::Completed => CloseCommitStatus::CleanupCommitted,
            _ => return,
        };
        let Some(close) = self.application_close.as_ref() else {
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
        self.tasks.complete(TaskId::CloseFlush);
        match result {
            Ok(Ok(updated)) => {
                let close = self.application_close.as_mut().expect("关闭记录必须存在");
                let transition = match next {
                    CloseCommitStatus::PtyStopped => close.coordinator.pty_stopped(attempt),
                    CloseCommitStatus::CleanupCommitted => {
                        close.persisted = updated.clone();
                        self.drive_application_close(cx);
                        cx.notify();
                        return;
                    }
                    CloseCommitStatus::Completed => close.coordinator.recovery_cleaned(attempt),
                    _ => unreachable!(),
                };
                if transition.is_err() {
                    self.fail_application_close_unknown(attempt);
                    return;
                }
                close.persisted = updated;
                self.drive_application_close(cx);
            }
            Ok(Err(error)) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.close_in_progress = false;
                log::warn!("application close advancement failed: {error}");
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => self.fail_application_close_unknown(attempt),
        }
        cx.notify();
    }

    fn fail_application_close_unknown(&mut self, attempt: CloseAttempt) {
        if let Some(close) = self.application_close.as_mut() {
            close.outcome_unknown = true;
            let _ = close.coordinator.fail(attempt);
        }
        self.close_in_progress = false;
    }
}
