//! 应用退出的局部关闭段：登记意图、停止全部会话、提交清理完成。

use gpui::{Context, Window};
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage,
};

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};
use crate::app_shell::FinalShutdownOutcome;
use crate::app_shell::global_close::{GlobalCloseError, GlobalCloseRequestError};
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

/// 恢复失败时不登记关闭意图，仅跟踪最终 flush 后的全局回执代次。
pub(super) struct RecoveryGlobalClose {
    pub(super) close_id: String,
    pub(super) attempt: u64,
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
            let window_handle = window.window_handle();
            let recovery = self
                .recovery_global_close
                .get_or_insert_with(|| RecoveryGlobalClose {
                    close_id: uuid::Uuid::now_v7().to_string(),
                    attempt: 0,
                    outcome_unknown: false,
                    window: window_handle,
                });
            if recovery.outcome_unknown {
                return false;
            }
            self.close_in_progress = true;
            return self.start_close_flush(writer, window_handle, cx);
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

    pub(super) fn start_application_global_close(&mut self, cx: &mut Context<'_, Self>) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        if close.coordinator.phase() != ClosePhase::Running(CloseStage::StopServices) {
            return;
        }
        let attempt = close
            .coordinator
            .current_attempt()
            .expect("全局服务阶段必须有 attempt");
        let close_id = close.persisted.close_id.clone();
        let Some(client) = self.global_close.clone() else {
            return;
        };
        let owner = crate::windowing::focus::window_hwnd(cx);
        let response = match client.request(owner) {
            Ok(response) => response,
            Err(GlobalCloseRequestError::Full) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.close_in_progress = false;
                log::warn!("application global close request queue full");
                return;
            }
            Err(GlobalCloseRequestError::Disconnected) => {
                log::warn!("application global close worker disconnected");
                self.fail_application_close_unknown(attempt);
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let completion = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = completion.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_application_global(close_id, attempt, result, cx);
                        });
                    })
                });
        if registration.is_err() {
            self.fail_application_close_unknown(attempt);
        }
    }

    pub(super) fn start_recovery_global_close(&mut self, cx: &mut Context<'_, Self>) {
        let Some(client) = self.global_close.clone() else {
            return;
        };
        let (close_id, attempt) = {
            let Some(recovery) = self.recovery_global_close.as_mut() else {
                return;
            };
            if recovery.outcome_unknown {
                return;
            }
            let Some(attempt) = recovery.attempt.checked_add(1) else {
                recovery.outcome_unknown = true;
                self.close_in_progress = false;
                return;
            };
            recovery.attempt = attempt;
            (recovery.close_id.clone(), attempt)
        };
        let owner = crate::windowing::focus::window_hwnd(cx);
        let response = match client.request(owner) {
            Ok(response) => response,
            Err(GlobalCloseRequestError::Full) => {
                self.close_in_progress = false;
                log::warn!("recovery global close request queue full");
                return;
            }
            Err(GlobalCloseRequestError::Disconnected) => {
                if let Some(recovery) = self.recovery_global_close.as_mut() {
                    recovery.outcome_unknown = true;
                }
                self.close_in_progress = false;
                log::warn!("recovery global close worker disconnected");
                return;
            }
        };
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let completion = executor.spawn(async move { response.recv() });
                    cx.spawn(async move |this, cx| {
                        let result = completion.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_recovery_global(close_id, attempt, result, cx);
                        });
                    })
                });
        if registration.is_err() {
            if let Some(recovery) = self.recovery_global_close.as_mut() {
                recovery.outcome_unknown = true;
            }
            self.close_in_progress = false;
        }
    }

    fn finish_recovery_global(
        &mut self,
        close_id: String,
        attempt: u64,
        result: Result<Result<FinalShutdownOutcome, GlobalCloseError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(recovery) = self.recovery_global_close.as_ref() else {
            return;
        };
        if recovery.close_id != close_id || recovery.attempt != attempt || recovery.outcome_unknown
        {
            return;
        }
        self.tasks.complete(TaskId::CloseFlush);
        match result {
            Ok(Ok(_)) => {
                let window = recovery.window;
                self.close_in_progress = false;
                self.close_completed = true;
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
            Ok(Err(GlobalCloseError::ControlPlaneCleanupFailed)) => {
                self.close_in_progress = false;
                log::warn!("recovery control plane cleanup failed");
            }
            Ok(Err(GlobalCloseError::Writer(error))) => {
                if let Some(recovery) = self.recovery_global_close.as_mut() {
                    recovery.outcome_unknown = true;
                }
                self.close_in_progress = false;
                log::warn!("recovery global writer shutdown failed: {error}");
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => {
                if let Some(recovery) = self.recovery_global_close.as_mut() {
                    recovery.outcome_unknown = true;
                }
                self.close_in_progress = false;
                super::events::notify_workspace_write_failed(cx);
            }
        }
        cx.notify();
    }

    pub(super) fn finish_application_global(
        &mut self,
        close_id: String,
        attempt: CloseAttempt,
        result: Result<Result<FinalShutdownOutcome, GlobalCloseError>, std::sync::mpsc::RecvError>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        if close.persisted.close_id != close_id
            || close.coordinator.current_attempt() != Some(attempt)
            || close.coordinator.phase() != ClosePhase::Running(CloseStage::StopServices)
        {
            return;
        }
        self.tasks.complete(TaskId::CloseFlush);
        match result {
            Ok(Ok(outcome)) => {
                let transitioned = {
                    let close = self.application_close.as_mut().expect("关闭记录必须存在");
                    if close.coordinator.services_stopped(attempt).is_err() {
                        false
                    } else {
                        let clean_attempt = close
                            .coordinator
                            .current_attempt()
                            .expect("正常退出提交阶段必须有 attempt");
                        match outcome {
                            FinalShutdownOutcome::Completed => close
                                .coordinator
                                .clean_shutdown_committed(clean_attempt)
                                .is_ok(),
                            FinalShutdownOutcome::Abandoned => {
                                if close.coordinator.fail(clean_attempt).is_err() {
                                    false
                                } else {
                                    close.coordinator.abandon().is_ok()
                                }
                            }
                        }
                    }
                };
                if !transitioned {
                    let attempt = self
                        .application_close
                        .as_ref()
                        .and_then(|close| close.coordinator.current_attempt())
                        .unwrap_or(attempt);
                    self.fail_application_close_unknown(attempt);
                    return;
                }
                let window = self
                    .application_close
                    .as_ref()
                    .expect("关闭记录必须存在")
                    .window;
                self.close_in_progress = false;
                self.close_completed = true;
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
            Ok(Err(GlobalCloseError::ControlPlaneCleanupFailed)) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.close_in_progress = false;
                log::warn!("application control plane cleanup failed");
            }
            Ok(Err(GlobalCloseError::Writer(error))) => {
                log::warn!("application global writer shutdown failed: {error}");
                self.fail_application_close_unknown(attempt);
                super::events::notify_workspace_write_failed(cx);
            }
            Err(_) => {
                self.fail_application_close_unknown(attempt);
                super::events::notify_workspace_write_failed(cx);
            }
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
