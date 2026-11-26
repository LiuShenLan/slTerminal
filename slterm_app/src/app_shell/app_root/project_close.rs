//! 项目移除：删除意图提交后停止所属 PTY，再清理项目事实并销毁运行实体。

use gpui::{App, Context, Entity, Window};
use slterm_document::DocumentId;
use slterm_workspace::{
    CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject,
    ClosePhase, CloseScope, CloseStage, ProjectId, ProjectSnapshot, ViewKey,
};

use super::commands::AppRootError;
use super::project_view::ProjectView;
use super::side_view::workspace_view_state_registry;
use super::task_service::{CancellationToken, TaskId};
use super::terminal_close::{PtyStopObservation, close_fingerprint, same_close_fact};
use super::{AppRoot, SideViewId};
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent, ProjectCleanupError,
};

pub(super) struct LocalProjectClose {
    pub(super) project: Entity<ProjectView>,
    pub(super) coordinator: CloseCoordinator<slterm_workspace::CloseTarget>,
    pub(super) persisted: PersistedCloseIntent,
    pub(super) stop_observation: Option<PtyStopObservation>,
    pub(super) outcome_unknown: bool,
    /// Save 阶段代次失配：显式重试必须先经 Collect 重收集。
    pub(super) needs_recollect: bool,
    pub(super) window: gpui::AnyWindowHandle,
}

impl AppRoot {
    /// 同一入口既发起项目移除，也显式重试已明确的失败阶段。
    pub(crate) fn remove_project(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.remove_project_with_decisions(id, Default::default(), false, window, cx)
    }

    /// 带显式 Document 处置的项目移除；保存阶段先于删除意图提交。
    pub(super) fn remove_project_with_decisions(
        &mut self,
        id: ProjectId,
        document_decisions: std::collections::BTreeMap<DocumentId, CloseDecision>,
        allow_hooks_discard: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(close) = self.project_closes.get(&id) {
            return match close.coordinator.phase() {
                ClosePhase::Running(_) => Err(AppRootError::ProjectCloseRunning),
                ClosePhase::Failed(_) => {
                    self.retry_project_close(id, &document_decisions, cx)?;
                    Ok(())
                }
                ClosePhase::Idle | ClosePhase::Completed => Err(AppRootError::ProjectCloseState),
            };
        }
        // Runtime 显式丢弃策略在收集决定与预检全部通过后由调用方落地；
        // 交互路径没有决定，脏草稿保持阻断。
        if self.hook_drafts.project_has_drafts(id) && !allow_hooks_discard {
            return Err(AppRootError::HooksDraftPending);
        }
        if !self.project_closes.is_empty()
            || self
                .terminal_closes
                .values()
                .any(|close| close.record.project_id() == id)
            || self
                .terminal_close_batches
                .values()
                .any(|close| close.project_id == id)
        {
            return Err(AppRootError::ProjectCloseRunning);
        }
        self.ensure_project_snapshot_recovery_allowed()?;
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
        let mut objects = self
            .snapshot
            .terminals()
            .filter(|terminal| terminal.project_id() == id)
            .map(|terminal| CloseObject {
                id: slterm_workspace::CloseTarget::Terminal(terminal.id()),
                generation: self.snapshot_generation,
                decision: CloseDecision::NotRequired,
            })
            .collect::<Vec<_>>();
        let project_views = self
            .snapshot
            .registry()
            .project(id)
            .map(|project| project.layout().view_keys())
            .unwrap_or_default();
        objects.extend(self.collect_document_close_with_autosave(
            &project_views,
            &document_decisions,
            cx,
        )?);
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
                needs_recollect: false,
                window: window.window_handle(),
            },
        );
        if self
            .project_closes
            .get(&id)
            .is_some_and(|close| close.coordinator.phase() == ClosePhase::Running(CloseStage::Save))
        {
            self.start_project_save(id, cx);
            return Ok(());
        }
        self.start_project_commit(id, candidate, window.window_handle(), cx)?;
        Ok(())
    }

    /// 项目移除的保存阶段：真实写盘与代次校验先于删除意图提交。
    fn start_project_save(&mut self, id: ProjectId, cx: &mut Context<'_, Self>) {
        let task_id = TaskId::ProjectClose(id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let Some((attempt, intent)) = self.project_closes.get(&id).and_then(|close| {
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
            0,
        );
        let pending = prepared
            .requests
            .iter()
            .map(|request| (request.document_id(), request.save_generation()))
            .collect::<Vec<_>>();
        let executor = cx.background_executor().clone();
        let registration = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let save = executor.spawn(async move {
                    let writes = crate::app_shell::document_close::execute_document_save_writes(
                        prepared.requests,
                        &grants,
                        0,
                    );
                    (prepared.failures, writes)
                });
                cx.spawn(async move |this, cx| {
                    let (failures, writes) = save.await;
                    let _ = this.update(cx, |root, cx| {
                        root.finish_project_save(id, attempt, failures, writes, cx);
                    });
                })
            });
        if registration.is_err() {
            for (document_id, generation) in pending {
                let _ = self.documents.fail_save(document_id, generation);
            }
            self.fail_project_close_outcome_unknown(id, attempt);
        }
    }

    fn finish_project_save(
        &mut self,
        id: ProjectId,
        attempt: CloseAttempt,
        failures: Vec<crate::app_shell::document_close::DocumentSaveFailure>,
        writes: Vec<crate::app_shell::document_close::DocumentSaveWriteOutcome>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(close) = self.project_closes.get(&id) else {
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
        self.tasks.complete(TaskId::ProjectClose(id));
        if let Err(failures) = result {
            let needs_recollect = failures
                .iter()
                .any(|failure| failure.is_revision_mismatch());
            if let Some(close) = self.project_closes.get_mut(&id) {
                close.needs_recollect = needs_recollect;
                let _ = close.coordinator.fail(attempt);
            }
            log::warn!("项目移除前的 Document 保存失败: {} 项", failures.len());
            super::events::notify_document_save_error(
                &crate::text_document::SaveError::Io(std::io::Error::other(
                    "项目移除前的文档保存失败",
                )),
                cx,
            );
            cx.notify();
            return;
        }
        let Some(close) = self.project_closes.get_mut(&id) else {
            return;
        };
        if close.coordinator.save_succeeded(attempt).is_err() {
            self.fail_project_close_outcome_unknown(id, attempt);
            return;
        }
        let Some(window) = self.project_closes.get(&id).map(|close| close.window) else {
            return;
        };
        let Ok((candidate, _)) = self.snapshot.without_project(id) else {
            return;
        };
        if let Err(error) = self.start_project_commit(id, candidate, window, cx) {
            log::warn!("项目移除提交无法开始: {error}");
        }
    }

    fn retry_project_close(
        &mut self,
        id: ProjectId,
        document_decisions: &std::collections::BTreeMap<DocumentId, CloseDecision>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (needs_recollect, outcome_unknown) = {
            let close = self
                .project_closes
                .get(&id)
                .ok_or(AppRootError::MissingPanel)?;
            (close.needs_recollect, close.outcome_unknown)
        };
        if outcome_unknown {
            return Err(AppRootError::ProjectCloseOutcomeUnknown);
        }
        if needs_recollect {
            self.recollect_project_close(id, document_decisions, cx)?;
        } else {
            self.project_closes
                .get_mut(&id)
                .ok_or(AppRootError::MissingPanel)?
                .coordinator
                .retry()
                .map_err(|_| AppRootError::ProjectCloseState)?;
        }
        self.drive_project_close(id, cx)
    }

    /// Save 阶段代次失配：从最新 Document 代次与新决定重建意图并重新提交。
    fn recollect_project_close(
        &mut self,
        id: ProjectId,
        document_decisions: &std::collections::BTreeMap<DocumentId, CloseDecision>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (previous, scope, store_epoch, close_id) = {
            let close = self
                .project_closes
                .get(&id)
                .ok_or(AppRootError::MissingPanel)?;
            (
                close
                    .coordinator
                    .intent()
                    .cloned()
                    .ok_or(AppRootError::ProjectCloseState)?,
                close
                    .coordinator
                    .scope()
                    .cloned()
                    .ok_or(AppRootError::ProjectCloseState)?,
                close.persisted.store_epoch.clone(),
                close.persisted.close_id.clone(),
            )
        };
        let closing = self
            .snapshot
            .registry()
            .project(id)
            .map(|project| project.layout().view_keys())
            .unwrap_or_default();
        let all_views = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .collect::<Vec<_>>();
        // 仍有效的原决定保留；已变干净或存在项目外引用的对象不再携带旧决定。
        let mut decisions = document_decisions.clone();
        for object in previous.objects() {
            let slterm_workspace::CloseTarget::Document(target_document) = object.id else {
                continue;
            };
            if decisions.contains_key(&target_document)
                || !matches!(
                    object.decision,
                    CloseDecision::Save | CloseDecision::Discard
                )
            {
                continue;
            }
            let has_external_reference = all_views.iter().any(|view| match view {
                ViewKey::Editor { document_id, .. }
                | ViewKey::Markdown { document_id, .. }
                | ViewKey::Diff { document_id, .. } => {
                    *document_id == target_document && !closing.contains(view)
                }
                _ => false,
            });
            if !has_external_reference
                && self
                    .documents
                    .session(target_document)
                    .is_some_and(|session| session.is_dirty())
            {
                decisions.insert(target_document, object.decision);
            }
        }
        let mut objects = self.collect_document_close_with_autosave(&closing, &decisions, cx)?;
        objects.extend(
            previous
                .objects()
                .filter(|object| matches!(object.id, slterm_workspace::CloseTarget::Terminal(_)))
                .cloned(),
        );
        let fingerprint = close_fingerprint(&close_id, &scope, &store_epoch, &objects);
        let intent =
            CloseIntent::new(objects, fingerprint).map_err(|_| AppRootError::ProjectCloseState)?;
        let close = self
            .project_closes
            .get_mut(&id)
            .ok_or(AppRootError::MissingPanel)?;
        let CloseEffect::Collect { attempt } = close
            .coordinator
            .recollect()
            .map_err(|_| AppRootError::ProjectCloseState)?
        else {
            return Err(AppRootError::ProjectCloseState);
        };
        close
            .coordinator
            .collected(attempt, intent.clone())
            .map_err(|_| AppRootError::ProjectCloseState)?;
        close.persisted.intent = intent;
        close.needs_recollect = false;
        Ok(())
    }

    fn start_project_commit(
        &mut self,
        id: ProjectId,
        candidate: ProjectSnapshot,
        window: gpui::AnyWindowHandle,
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
        self.side_view_persistence
            .editor_view_states()
            .remove_project(id);
        // 项目已从快照移除：项目专属 Hooks 草稿与文档随同一提交清理，User 草稿保留。
        let hooks_keys = self.hook_drafts.clear_project(id);
        self.forget_hooks_draft_keys(&hooks_keys);
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
            self.tasks.cancel(TaskId::DiffHead(panel_id));
            self.remove_editor_git(panel_id);
            self.tasks
                .remove_view_refresh(super::task_service::ViewTaskOwner::Panel(panel_id));
            self.forget_panel_runtime(panel_id, cx);
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
            ClosePhase::Running(CloseStage::Save) => {
                self.start_project_save(id, cx);
                Ok(())
            }
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
        let registry = std::rc::Rc::clone(&self.terminals);
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::ProjectClose(id),
            CancellationToken::new(),
            move || {
                let terminal_ids = project.update(cx, |project, _| {
                    project
                        .panels
                        .values()
                        .filter_map(|panel| match &panel.view_key {
                            slterm_workspace::ViewKey::Terminal { terminal_id, .. } => {
                                Some(*terminal_id)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                });
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
        let response = match writer.cleanup_project_close(&current, workspace_view_state_registry())
        {
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
