//! 应用退出的局部关闭段：登记意图、停止全部会话、提交清理完成。

use gpui::{App, Context, Window};
use slterm_document::DocumentId;
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage, CloseTarget,
};

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};
use crate::app_shell::FinalShutdownOutcome;
use crate::app_shell::global_close::{GlobalCloseError, GlobalCloseRequestError};
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent,
};

/// 应用关闭启动结果：同步完成、异步推进、等待在途工作或未开始。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApplicationCloseStart {
    Completed,
    Started,
    Deferred,
    Rejected,
}

pub(super) struct ApplicationClose {
    pub(super) coordinator: CloseCoordinator<CloseTarget>,
    pub(super) persisted: PersistedCloseIntent,
    pub(super) stop_observation: Option<PtyStopObservation>,
    pub(super) outcome_unknown: bool,
    pub(super) needs_recollect: bool,
    pub(super) window: gpui::AnyWindowHandle,
    /// 恢复失败会话不写关闭意图，只在本进程内推进同一关闭状态机。
    pub(super) persist_intent: bool,
    /// Runtime `app.close` 的回执：保存与最终冲刷落定后才答复。
    pub(super) runtime_completion: Option<super::runtime_snapshot::RuntimeMutationCompletion>,
}

impl AppRoot {
    pub(super) fn start_application_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        // 交互式关闭没有 Hooks 决定面：有脏草稿时保持窗口，要求先保存或丢弃。
        if self.hook_drafts.dirty_drafts().next().is_some() {
            super::events::notify_hooks_draft_close_pending(cx);
            return false;
        }
        match self.start_application_close_with_decisions(Default::default(), true, window, cx) {
            Ok(ApplicationCloseStart::Completed) => true,
            Ok(ApplicationCloseStart::Started | ApplicationCloseStart::Deferred) => false,
            Ok(ApplicationCloseStart::Rejected) => false,
            Err(error) => {
                log::warn!("application close requires document decisions: {error}");
                let count = self
                    .documents
                    .ids()
                    .filter(|id| {
                        self.documents
                            .session(*id)
                            .is_some_and(|session| session.is_dirty())
                    })
                    .count();
                super::events::notify_application_close_decision_required(count, cx);
                false
            }
        }
    }

    /// 带显式 Document 决定的应用关闭；缺决定只返回错误，不弹任何交互。
    pub(super) fn start_application_close_with_decisions(
        &mut self,
        decisions: std::collections::BTreeMap<DocumentId, CloseDecision>,
        defer_on_inflight: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<ApplicationCloseStart, slterm_workspace::DocumentCloseError> {
        if !self.terminal_closes.is_empty()
            || !self.terminal_close_batches.is_empty()
            || !self.project_closes.is_empty()
        {
            return Ok(ApplicationCloseStart::Rejected);
        }
        if self.application_close.is_some() {
            self.retry_application_close(cx);
            return Ok(ApplicationCloseStart::Rejected);
        }
        if self.close_in_progress {
            return Ok(ApplicationCloseStart::Rejected);
        }
        // 在途文档保存期间无法稳定判定决定齐备性：UI 路径延后，Runtime 明确拒绝。
        if self.has_inflight_document_work() {
            if !defer_on_inflight {
                return Ok(ApplicationCloseStart::Rejected);
            }
            self.pending_application_close = Some(window.window_handle());
            self.pending_application_close_decisions = decisions;
            self.close_in_progress = true;
            return Ok(ApplicationCloseStart::Deferred);
        }
        // 决定齐备性先于任何完成路径：没有 writer 也不得丢弃脏正文。
        let documents = self.collect_application_documents(decisions, cx)?;
        let Some(writer) = self.project_writer.clone() else {
            // 无 writer 无法持久关闭意图，也不得假装已保存：需要保存时明确拒绝。
            if documents
                .iter()
                .any(|object| object.decision == CloseDecision::Save)
            {
                return Ok(ApplicationCloseStart::Rejected);
            }
            self.close_in_progress = true;
            self.close_completed = true;
            // 无 writer 的完成路径没有最终持久提交，不得保留安装预约。
            self.cancel_update_install();
            window.remove_window();
            return Ok(ApplicationCloseStart::Completed);
        };
        let persist_intent = !self.project_snapshot_recovery_failed;
        let store_epoch = match self.store_epoch.clone().filter(|epoch| !epoch.is_empty()) {
            Some(epoch) => epoch,
            // 恢复失败会话不写关闭意图，空世代只用于进程内指纹。
            None if !persist_intent => String::new(),
            None => {
                log::warn!("application close rejected without current store epoch");
                return Ok(ApplicationCloseStart::Rejected);
            }
        };
        let scope = CloseScope::ApplicationExit;
        let mut objects = self
            .snapshot
            .terminals()
            .map(|terminal| CloseObject {
                id: CloseTarget::Terminal(terminal.id()),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        objects.extend(documents);
        let close_id = uuid::Uuid::now_v7().to_string();
        let fingerprint = close_fingerprint(&close_id, &scope, &store_epoch, &objects);
        let intent = match CloseIntent::new(objects, fingerprint) {
            Ok(intent) => intent,
            Err(_) => return Ok(ApplicationCloseStart::Rejected),
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
            return Ok(ApplicationCloseStart::Rejected);
        }
        self.application_close = Some(ApplicationClose {
            coordinator,
            persisted: registration,
            stop_observation: None,
            outcome_unknown: false,
            needs_recollect: false,
            window: window.window_handle(),
            persist_intent,
            runtime_completion: None,
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
            return Ok(ApplicationCloseStart::Rejected);
        }
        if self
            .application_close
            .as_ref()
            .is_some_and(|close| close.coordinator.phase() == ClosePhase::Running(CloseStage::Save))
        {
            self.start_application_save(cx);
        } else if persist_intent {
            self.start_application_commit(writer, snapshot, cx);
        } else {
            self.advance_ephemeral_commit(cx);
        }
        Ok(ApplicationCloseStart::Started)
    }

    /// Runtime 关闭回执成功：只在保存与最终冲刷全部落定后调用。
    pub(super) fn finish_runtime_close_success(&mut self) {
        let Some(mut completion) = self
            .application_close
            .as_mut()
            .and_then(|close| close.runtime_completion.take())
        else {
            return;
        };
        completion.send(crate::app_shell::RuntimeMutationReply::Done(Ok(())));
    }

    /// Runtime 关闭回执已知失败：窗口与未保存正文仍保持现场。
    pub(super) fn finish_runtime_close_failure(&mut self) {
        let Some(mut completion) = self
            .application_close
            .as_mut()
            .and_then(|close| close.runtime_completion.take())
        else {
            return;
        };
        completion.send_error(super::runtime_snapshot::not_ready_error(
            "application close failed",
        ));
    }

    /// Runtime 关闭结果不明：放弃回执，让 Operation 进入可查询的 outcome_unknown。
    pub(super) fn abandon_runtime_close(&mut self) {
        if let Some(completion) = self
            .application_close
            .as_mut()
            .and_then(|close| close.runtime_completion.take())
        {
            completion.abandon();
        }
    }

    fn has_inflight_document_work(&self) -> bool {
        self.documents.ids().any(|id| {
            self.tasks.is_registered(TaskId::DocumentSave(id))
                || self.tasks.is_registered(TaskId::DocumentExternal(id))
        }) || !self.pending_document_relocations.is_empty()
            || !self.pending_document_refreshes.is_empty()
    }

    pub(super) fn resume_pending_application_close(&mut self, cx: &mut Context<'_, Self>) {
        let Some(window) = self.pending_application_close else {
            return;
        };
        if self.has_inflight_document_work() {
            return;
        }
        self.pending_application_close = None;
        self.close_in_progress = false;
        let decisions = std::mem::take(&mut self.pending_application_close_decisions);
        let root = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = root.update(cx, |root, cx| {
                    let _ =
                        root.start_application_close_with_decisions(decisions, true, window, cx);
                });
            });
        });
    }

    fn collect_application_documents(
        &mut self,
        mut decisions: std::collections::BTreeMap<DocumentId, CloseDecision>,
        cx: &App,
    ) -> Result<Vec<CloseObject<CloseTarget>>, slterm_workspace::DocumentCloseError> {
        decisions.retain(|id, _| {
            self.documents
                .session(*id)
                .is_some_and(|session| session.is_dirty())
        });
        self.sync_document_autosave_enabled(cx);
        for id in self.document_autosave.on_exit(&self.documents) {
            decisions.entry(id).or_insert(CloseDecision::Save);
        }
        let closing = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .collect::<Vec<_>>();
        slterm_workspace::collect_document_close(
            &self.snapshot,
            &self.documents,
            &closing,
            &decisions,
        )
    }

    fn recollect_application_close(&mut self, cx: &mut Context<'_, Self>) -> bool {
        let Some((attempt, close_id, store_epoch, old_intent)) =
            self.application_close.as_mut().and_then(|close| {
                let CloseEffect::Collect { attempt } = close.coordinator.recollect().ok()? else {
                    unreachable!()
                };
                Some((
                    attempt,
                    close.persisted.close_id.clone(),
                    close.persisted.store_epoch.clone(),
                    close.coordinator.intent()?.clone(),
                ))
            })
        else {
            return false;
        };
        let mut objects = self
            .snapshot
            .terminals()
            .map(|terminal| CloseObject {
                id: CloseTarget::Terminal(terminal.id()),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        let decisions = old_intent
            .objects()
            .filter_map(|object| match object.id {
                CloseTarget::Document(id)
                    if matches!(
                        object.decision,
                        CloseDecision::Save | CloseDecision::Discard
                    ) =>
                {
                    Some((id, object.decision))
                }
                _ => None,
            })
            .collect();
        let documents = match self.collect_application_documents(decisions, cx) {
            Ok(documents) => documents,
            Err(error) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                log::warn!("application close recollect requires document decisions: {error}");
                let count = self
                    .documents
                    .ids()
                    .filter(|id| {
                        self.documents
                            .session(*id)
                            .is_some_and(|session| session.is_dirty())
                    })
                    .count();
                super::events::notify_application_close_decision_required(count, cx);
                return false;
            }
        };
        objects.extend(documents);
        let fingerprint = close_fingerprint(
            &close_id,
            &CloseScope::ApplicationExit,
            &store_epoch,
            &objects,
        );
        let intent = match CloseIntent::new(objects, fingerprint) {
            Ok(intent) => intent,
            Err(_) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                return false;
            }
        };
        let Some(close) = self.application_close.as_mut() else {
            return false;
        };
        if close
            .coordinator
            .collected(attempt, intent.clone())
            .is_err()
        {
            let _ = close.coordinator.fail(attempt);
            return false;
        }
        close.persisted.intent = intent;
        close.needs_recollect = false;
        true
    }

    fn retry_application_close(&mut self, cx: &mut Context<'_, Self>) {
        if self
            .application_close
            .as_ref()
            .is_some_and(|close| close.needs_recollect)
            && !self.recollect_application_close(cx)
        {
            self.finish_runtime_close_failure();
            self.application_close = None;
            self.close_in_progress = false;
            self.side_view_persistence
                .resume_checkpoint_for_close_retry(&self.side_views, cx);
            return;
        }
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
                self.finish_runtime_close_failure();
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
                self.finish_runtime_close_failure();
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
            ClosePhase::Running(CloseStage::Save) => self.start_application_save(cx),
            ClosePhase::Running(CloseStage::CommitState) if !close.persist_intent => {
                self.advance_ephemeral_commit(cx);
            }
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

    /// 恢复失败会话没有持久关闭意图；CommitState 直接在内存推进到 StopPty。
    fn advance_ephemeral_commit(&mut self, cx: &mut Context<'_, Self>) {
        let transition = {
            let Some(close) = self.application_close.as_mut() else {
                return;
            };
            if close.persist_intent
                || close.coordinator.phase() != ClosePhase::Running(CloseStage::CommitState)
            {
                return;
            }
            let attempt = close
                .coordinator
                .current_attempt()
                .expect("提交阶段必须有 attempt");
            if close.coordinator.state_committed(attempt).is_err() {
                Err(attempt)
            } else {
                close.persisted.commit_status = CloseCommitStatus::StateCommitted;
                Ok(())
            }
        };
        if let Err(attempt) = transition {
            self.fail_application_close_unknown(attempt);
            return;
        }
        self.close_in_progress = true;
        self.drive_application_close(cx);
        cx.notify();
    }

    fn start_application_save(&mut self, cx: &mut Context<'_, Self>) {
        if self.tasks.is_registered(TaskId::CloseFlush) {
            return;
        }
        let Some((attempt, intent)) = self.application_close.as_ref().and_then(|close| {
            Some((
                close.coordinator.current_attempt()?,
                close.coordinator.intent()?.clone(),
            ))
        }) else {
            return;
        };
        let grants = self.document_close_grants(&intent);
        let prepared = crate::app_shell::document_close::prepare_document_save_stage(
            &mut self.documents,
            &intent,
            &grants,
            crate::app_shell::authorization::current_generation(),
        );
        let pending = prepared
            .requests
            .iter()
            .map(|request| (request.document_id(), request.save_generation()))
            .collect::<Vec<_>>();
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let save = executor.spawn(async move {
                        let writes = crate::app_shell::document_close::execute_document_save_writes(
                            prepared.requests,
                            &grants,
                            crate::app_shell::authorization::current_generation(),
                        );
                        (prepared.failures, writes)
                    });
                    cx.spawn(async move |this, cx| {
                        let (failures, writes) = save.await;
                        let _ = this.update(cx, |root, cx| {
                            root.finish_application_save(attempt, failures, writes, cx);
                        });
                    })
                });
        if registration.is_err() {
            for (id, generation) in pending {
                let _ = self.documents.fail_save(id, generation);
            }
            self.fail_application_close_unknown(attempt);
        }
    }

    fn finish_application_save(
        &mut self,
        attempt: CloseAttempt,
        failures: Vec<crate::app_shell::document_close::DocumentSaveFailure>,
        writes: Vec<crate::app_shell::document_close::DocumentSaveWriteOutcome>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.application_close.as_ref() else {
            return;
        };
        if close.coordinator.current_attempt() != Some(attempt) {
            return;
        }
        let intent = close.coordinator.intent().cloned();
        let result = crate::app_shell::document_close::complete_document_save_stage(
            &mut self.documents,
            failures,
            writes,
        )
        .and_then(|()| {
            intent.as_ref().map_or(Ok(()), |intent| {
                crate::app_shell::document_close::validate_document_close_generations(
                    &self.documents,
                    intent,
                )
            })
        });
        self.tasks.complete(TaskId::CloseFlush);
        if let Err(failures) = result {
            if let Some(close) = self.application_close.as_mut() {
                close.needs_recollect |= failures
                    .iter()
                    .any(|failure| failure.is_revision_mismatch());
                let _ = close.coordinator.fail(attempt);
            }
            self.finish_runtime_close_failure();
            self.close_in_progress = false;
            self.side_view_persistence
                .resume_checkpoint_for_close_retry(&self.side_views, cx);
            log::warn!("应用退出前的 Document 保存失败: {} 项", failures.len());
            super::events::notify_document_save_error(
                &crate::text_document::SaveError::Io(std::io::Error::other(
                    "应用退出前的文档保存失败",
                )),
                cx,
            );
            cx.notify();
            return;
        }
        let Some(close) = self.application_close.as_mut() else {
            return;
        };
        let persist_intent = close.persist_intent;
        if close.coordinator.save_succeeded(attempt).is_err() {
            self.fail_application_close_unknown(attempt);
            return;
        }
        if !persist_intent {
            self.advance_ephemeral_commit(cx);
            return;
        }
        let Some(writer) = self.project_writer.clone() else {
            self.fail_application_close_unknown(attempt);
            return;
        };
        let snapshot = self.snapshot.clone();
        self.start_application_commit(writer, snapshot, cx);
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
        let registry = std::rc::Rc::clone(&self.terminals);
        let executor = cx.background_executor().clone();
        let registration =
            self.tasks
                .register_with(TaskId::CloseFlush, CancellationToken::new(), move || {
                    let terminal_ids = projects
                        .into_iter()
                        .flat_map(|project| {
                            project.update(cx, |project, _| {
                                project
                                    .panels
                                    .values()
                                    .filter_map(|panel| match &panel.view_key {
                                        slterm_workspace::ViewKey::Terminal {
                                            terminal_id, ..
                                        } => Some(*terminal_id),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect::<Vec<_>>();
                    let sessions = terminal_ids
                        .into_iter()
                        .filter_map(|id| super::commands::take_terminal_worker(&registry, id))
                        .collect::<Vec<_>>();
                    let stop = executor.spawn(async move {
                        let mut observation = PtyStopObservation::Stopped;
                        for session in sessions.into_iter() {
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
            self.finish_runtime_close_failure();
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
        let Some((attempt, persist_intent)) = self.application_close.as_ref().map(|close| {
            (
                close
                    .coordinator
                    .current_attempt()
                    .expect("推进阶段必须有 attempt"),
                close.persist_intent,
            )
        }) else {
            return;
        };
        if !persist_intent {
            let transition = {
                let close = self.application_close.as_mut().expect("关闭记录必须存在");
                match next {
                    CloseCommitStatus::PtyStopped => close.coordinator.pty_stopped(attempt).is_ok(),
                    CloseCommitStatus::CleanupCommitted => true,
                    CloseCommitStatus::Completed => {
                        close.coordinator.recovery_cleaned(attempt).is_ok()
                    }
                    _ => return,
                }
            };
            if !transition {
                self.fail_application_close_unknown(attempt);
                return;
            }
            let close = self.application_close.as_mut().expect("关闭记录必须存在");
            close.persisted.commit_status = next;
            self.close_in_progress = true;
            self.drive_application_close(cx);
            cx.notify();
            return;
        }
        let current = self
            .application_close
            .as_ref()
            .expect("关闭记录必须存在")
            .persisted
            .clone();
        let Some(writer) = self.project_writer.clone() else {
            return;
        };
        let response = match writer.advance_close_intent(&current, next) {
            Ok(response) => response,
            Err(error) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.finish_runtime_close_failure();
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
                self.finish_runtime_close_failure();
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
                self.finish_runtime_close_failure();
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
                if outcome == crate::app_shell::FinalShutdownOutcome::Completed {
                    self.finish_update_install();
                } else {
                    self.cancel_update_install();
                }
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
            Ok(Err(GlobalCloseError::ControlPlaneCleanupFailed)) => {
                if let Some(close) = self.application_close.as_mut() {
                    let _ = close.coordinator.fail(attempt);
                }
                self.finish_runtime_close_failure();
                self.close_in_progress = false;
                log::warn!("application control plane cleanup failed");
            }
            Ok(Err(GlobalCloseError::Writer(error))) => {
                log::warn!("application global writer shutdown failed: {error}");
                self.finish_runtime_close_failure();
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
        self.abandon_runtime_close();
        if let Some(close) = self.application_close.as_mut() {
            close.outcome_unknown = true;
            let _ = close.coordinator.fail(attempt);
        }
        self.close_in_progress = false;
    }
}
