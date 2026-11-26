//! Runtime v2 资源快照：从前台唯一项目快照派生只读 DTO。

use gpui::{App, Context, Entity, Window};
use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{
    AgentResource, DocumentResource, DomainErrorCode, JsonRpcError, PanelGroupResource,
    ProjectResource, SnapshotResources, TerminalResource, ViewResource,
};
use slterm_workspace::{RunIntent, ViewKey};

use crate::ai_agents::AgentStatus;
use crate::gpui_terminal::PaneId;

use super::AppRoot;
use super::command_registry::{CommandId, CommandParameters, CommandSource, CommandTarget};
use super::commands::AppRootError;
use super::events::open_file_handover_error;
use super::focus_manager::FocusError;
use crate::app_shell::RuntimeMutationReply;
use crate::app_shell::control_error;
use crate::local_control::RuntimeMutation;

/// Runtime 访问终端的运行要求：写输入必须 Launch；只读画面允许已退出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeTerminalAccess {
    Running,
    Display,
}

impl AppRoot {
    /// 发布轻量资源事件；关闭开始后不再回填过期事件。
    pub(super) fn publish_runtime_event(&self, kind: &str, resource: String) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return;
        }
        let Some(publisher) = &self.runtime_events else {
            return;
        };
        let _ = publisher.publish(slterm_runtime_protocol::RuntimeEvent {
            revision: 0,
            kind: kind.to_owned(),
            resource,
            generation: self.snapshot_generation,
        });
    }

    /// 执行 Runtime 写意图；参数校验、授权、事务与关闭策略全部复用既有命令。
    ///
    /// Git 破坏写只在前台解析项目与写授权；阻塞执行由调用方在后台完成。
    pub(super) fn handle_runtime_mutation(
        &mut self,
        mutation: RuntimeMutation,
        completion: &std::sync::mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> RuntimeMutationDispatch {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return RuntimeMutationDispatch::Reply(Err(closing_error()));
        }
        match mutation {
            RuntimeMutation::GitRollback { .. } | RuntimeMutation::GitDelete { .. } => {
                RuntimeMutationDispatch::Reply(self.resolve_runtime_git_write(mutation))
            }
            RuntimeMutation::GitOpenDiff { .. } | RuntimeMutation::GitOpenDeleted { .. } => {
                RuntimeMutationDispatch::Reply(self.resolve_runtime_git_open(mutation))
            }
            RuntimeMutation::PanelOpenFile { path } => {
                self.start_runtime_panel_open(path, completion, window, cx)
            }
            RuntimeMutation::PanelSave { view } => {
                self.start_runtime_panel_save(view, completion, cx)
            }
            mutation => RuntimeMutationDispatch::Reply(
                self.execute_runtime_mutation(mutation, window, cx)
                    .map(|()| RuntimeMutationReply::Done(Ok(()))),
            ),
        }
    }

    /// 打开文件到文本面板：准备失败立即回执，真正打开完成后回执。
    fn start_runtime_panel_open(
        &mut self,
        path: String,
        completion: &std::sync::mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> RuntimeMutationDispatch {
        let path = match CanonicalPath::new(path) {
            Ok(path) => path,
            Err(_) => return RuntimeMutationDispatch::Reply(Err(invalid_request_error())),
        };
        let request = match self.prepare_document_handover(path, window, cx) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeMutationDispatch::Reply(Err(open_file_handover_error(error)));
            }
        };
        let mut completion = RuntimeMutationCompletion::new(completion.clone());
        self.queue_document_open(
            request,
            Some(Box::new(move |result, _window, _cx| {
                let reply = match result {
                    Ok(()) => RuntimeMutationReply::Done(Ok(())),
                    Err(error) => RuntimeMutationReply::Done(Err(open_file_handover_error(error))),
                };
                completion.send(reply);
            })),
            window,
            cx,
        );
        RuntimeMutationDispatch::Deferred
    }

    /// 保存视图绑定的 Document：回执由唯一保存链在落定后发送。
    fn start_runtime_panel_save(
        &mut self,
        view: ViewKey,
        completion: &std::sync::mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
        cx: &mut Context<'_, Self>,
    ) -> RuntimeMutationDispatch {
        let document_id = match &view {
            ViewKey::Editor { document_id, .. } | ViewKey::Markdown { document_id, .. } => {
                *document_id
            }
            _ => return RuntimeMutationDispatch::Reply(Err(invalid_request_error())),
        };
        let view_exists = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .any(|project| project.layout().view_keys().contains(&view));
        let document_open = self.snapshot.documents().any(|(id, _)| id == document_id);
        if !view_exists || !document_open {
            return RuntimeMutationDispatch::Reply(Err(invalid_request_error()));
        }
        self.runtime_save_waiters
            .entry(document_id)
            .or_default()
            .push(completion.clone());
        self.save_document(document_id, cx);
        RuntimeMutationDispatch::Deferred
    }

    /// 显式关闭策略只作用于「最后引用在关闭集合内」的脏文档；
    /// 集合外仍有引用的共享文档不产生决定，避免整单被拒。
    fn last_reference_decisions(
        &self,
        closing: &[slterm_workspace::ViewKey],
        document_ids: impl Iterator<Item = slterm_document::DocumentId>,
        decision: slterm_workspace::CloseDecision,
    ) -> std::collections::BTreeMap<slterm_document::DocumentId, slterm_workspace::CloseDecision>
    {
        let outside = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .filter(|view| !closing.iter().any(|candidate| candidate == view))
            .filter_map(|view| view_document_id(&view))
            .collect::<std::collections::BTreeSet<_>>();
        document_ids
            .filter(|id| !outside.contains(id))
            .filter(|id| {
                self.documents
                    .session(*id)
                    .is_some_and(|session| session.is_dirty())
            })
            .map(|id| (id, decision))
            .collect()
    }

    /// 非交互项目移除：Hooks 草稿与项目范围 Document 都需要显式决定。
    fn start_runtime_project_remove(
        &mut self,
        project_id: slterm_workspace::ProjectId,
        documents: Option<slterm_runtime_protocol::CloseDocumentsPolicy>,
        hooks: Option<slterm_runtime_protocol::CloseHooksPolicy>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return Err(closing_error());
        }
        let views = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.layout().view_keys())
            .ok_or_else(invalid_request_error)?;
        let discard_hooks = match hooks {
            Some(slterm_runtime_protocol::CloseHooksPolicy::Discard) => true,
            None if self.hook_drafts.project_has_drafts(project_id) => {
                return Err(decision_required_error());
            }
            None => false,
        };
        let mut decisions = std::collections::BTreeMap::new();
        match documents {
            Some(policy) => {
                let decision = match policy {
                    slterm_runtime_protocol::CloseDocumentsPolicy::Save => {
                        slterm_workspace::CloseDecision::Save
                    }
                    slterm_runtime_protocol::CloseDocumentsPolicy::Discard => {
                        slterm_workspace::CloseDecision::Discard
                    }
                };
                decisions.extend(self.last_reference_decisions(
                    &views,
                    views.iter().filter_map(view_document_id),
                    decision,
                ));
            }
            None => match self.collect_document_close_with_autosave(
                &views,
                &std::collections::BTreeMap::new(),
                cx,
            ) {
                Ok(_) => {}
                Err(slterm_workspace::DocumentCloseError::MissingDecision(_)) => {
                    return Err(decision_required_error());
                }
                Err(_) => return Err(invalid_request_error()),
            },
        }
        self.remove_project_with_decisions(project_id, decisions, discard_hooks, window, cx)
            .map_err(runtime_mutation_error)?;
        // 决定齐备且关闭已登记后才丢弃草稿；失败路径保持现场。
        if discard_hooks {
            self.discard_project_hooks_drafts_for_close(project_id, cx);
        }
        Ok(())
    }

    /// 单面板非交互关闭：只关闭目标视图，脏 Document 需要显式决定。
    fn start_runtime_panel_close(
        &mut self,
        view: ViewKey,
        documents: Option<slterm_runtime_protocol::CloseDocumentsPolicy>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return Err(closing_error());
        }
        let project_id = view.project_id().ok_or_else(invalid_request_error)?;
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or_else(invalid_request_error)?;
        if self.snapshot.registry().active() != Some(project_id) {
            return Err(not_ready_error("project not active"));
        }
        if !project.layout().view_keys().contains(&view) {
            return Err(invalid_request_error());
        }
        let document_ids = match &view {
            ViewKey::Editor { document_id, .. }
            | ViewKey::Markdown { document_id, .. }
            | ViewKey::Diff { document_id, .. } => vec![*document_id],
            _ => Vec::new(),
        };
        let mut decisions = std::collections::BTreeMap::new();
        match documents {
            Some(policy) => {
                let decision = match policy {
                    slterm_runtime_protocol::CloseDocumentsPolicy::Save => {
                        slterm_workspace::CloseDecision::Save
                    }
                    slterm_runtime_protocol::CloseDocumentsPolicy::Discard => {
                        slterm_workspace::CloseDecision::Discard
                    }
                };
                decisions.extend(self.last_reference_decisions(
                    std::slice::from_ref(&view),
                    document_ids.iter().copied(),
                    decision,
                ));
            }
            None => {
                match self.collect_document_close_with_autosave(
                    std::slice::from_ref(&view),
                    &std::collections::BTreeMap::new(),
                    cx,
                ) {
                    Ok(_) => {}
                    Err(slterm_workspace::DocumentCloseError::MissingDecision(_)) => {
                        return Err(decision_required_error());
                    }
                    Err(_) => return Err(invalid_request_error()),
                }
            }
        }
        self.close_view_from_request(view, decisions, window, cx)
            .map_err(runtime_mutation_error)
    }

    /// 非交互应用关闭：策略齐备才登记关闭，缺少决定只返回结构化错误。
    fn start_runtime_application_close(
        &mut self,
        documents: Option<slterm_runtime_protocol::CloseDocumentsPolicy>,
        hooks: Option<slterm_runtime_protocol::CloseHooksPolicy>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return Err(closing_error());
        }
        let discard_hooks = match hooks {
            Some(slterm_runtime_protocol::CloseHooksPolicy::Discard) => true,
            None if self.hook_drafts.dirty_drafts().next().is_some() => {
                return Err(decision_required_error());
            }
            None => false,
        };
        let decisions = match documents {
            Some(policy) => {
                let decision = match policy {
                    slterm_runtime_protocol::CloseDocumentsPolicy::Save => {
                        slterm_workspace::CloseDecision::Save
                    }
                    slterm_runtime_protocol::CloseDocumentsPolicy::Discard => {
                        slterm_workspace::CloseDecision::Discard
                    }
                };
                self.documents
                    .ids()
                    .filter(|id| {
                        self.documents
                            .session(*id)
                            .is_some_and(|session| session.is_dirty())
                    })
                    .map(|id| (id, decision))
                    .collect()
            }
            None => std::collections::BTreeMap::new(),
        };
        match self.start_application_close_with_decisions(decisions, false, window, cx) {
            Ok(
                super::application_close::ApplicationCloseStart::Completed
                | super::application_close::ApplicationCloseStart::Started,
            ) => {
                // 关闭已登记后才丢弃草稿；拒绝或等待路径保持现场。
                if discard_hooks {
                    self.discard_all_hooks_drafts_for_close(cx);
                }
                Ok(())
            }
            Ok(
                super::application_close::ApplicationCloseStart::Deferred
                | super::application_close::ApplicationCloseStart::Rejected,
            ) => Err(not_ready_error("application close not ready")),
            Err(slterm_workspace::DocumentCloseError::MissingDecision(_)) => {
                Err(decision_required_error())
            }
            Err(_) => Err(not_ready_error("application close not ready")),
        }
    }

    /// Git 打开目标：校验路径形状并解析项目；仓库绑定与打开链都在后台发起。
    fn resolve_runtime_git_open(
        &self,
        mutation: RuntimeMutation,
    ) -> Result<RuntimeMutationReply, JsonRpcError> {
        let (project_id, path) = match &mutation {
            RuntimeMutation::GitOpenDiff {
                project_id, path, ..
            }
            | RuntimeMutation::GitOpenDeleted {
                project_id, path, ..
            } => (*project_id, path.as_str()),
            _ => return Err(invalid_request_error()),
        };
        if path.trim().is_empty() || !valid_repo_relative_path(path) {
            return Err(invalid_request_error());
        }
        let target = self.runtime_git_target(project_id)?;
        Ok(RuntimeMutationReply::GitOpen(
            crate::app_shell::GitOpenTarget {
                project_id: target.project_id,
                root: target.root,
                git: target.git,
            },
        ))
    }

    /// Runtime Git 打开：已捕获的仓库绑定在前台复用 Commit 的同一打开链。
    pub(super) fn handle_runtime_git_open(
        &mut self,
        project_id: slterm_workspace::ProjectId,
        binding: Box<slterm_document::RepositoryBinding>,
        path: slterm_path::CanonicalPath,
        kind: crate::app_shell::RuntimeGitOpenKind,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return Err(closing_error());
        }
        match kind {
            crate::app_shell::RuntimeGitOpenKind::Diff => {
                self.open_commit_diff_path(project_id, *binding, path, window, cx)
            }
            crate::app_shell::RuntimeGitOpenKind::Deleted => {
                self.open_commit_gitshow_path(project_id, *binding, path, window, cx)
            }
        }
        .map_err(runtime_mutation_error)
    }

    /// Git 破坏写目标：校验路径形状并解析项目；不做任何阻塞读取。
    fn resolve_runtime_git_write(
        &self,
        mutation: RuntimeMutation,
    ) -> Result<RuntimeMutationReply, JsonRpcError> {
        let (project_id, path) = match &mutation {
            RuntimeMutation::GitRollback {
                project_id, path, ..
            }
            | RuntimeMutation::GitDelete {
                project_id, path, ..
            } => (*project_id, path.as_str()),
            _ => return Err(invalid_request_error()),
        };
        if path.trim().is_empty() || !valid_repo_relative_path(path) {
            return Err(invalid_request_error());
        }
        let target = self.runtime_git_target(project_id)?;
        Ok(RuntimeMutationReply::Git(
            crate::app_shell::GitWriteTarget {
                project_id: target.project_id,
                root: target.root,
                git: target.git,
                file_operations: std::sync::Arc::clone(&self.file_operations),
            },
        ))
    }

    fn execute_runtime_mutation(
        &mut self,
        mutation: RuntimeMutation,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        match mutation {
            RuntimeMutation::ProjectActivate { project_id } => self
                .activate_project(project_id, window, cx)
                .map_err(runtime_mutation_error),
            RuntimeMutation::ProjectRename { project_id, name } => self
                .execute_command(
                    CommandId::RenameProject,
                    CommandSource::Application,
                    CommandTarget::Project(project_id),
                    Some(CommandParameters::RenameProject { name }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::ProjectReorder {
                project_id,
                new_index,
            } => self
                .execute_command(
                    CommandId::MoveProject,
                    CommandSource::Application,
                    CommandTarget::Project(project_id),
                    Some(CommandParameters::MoveProject { new_index }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::ProjectRemove {
                project_id,
                documents,
                hooks,
            } => self.start_runtime_project_remove(project_id, documents, hooks, window, cx),
            RuntimeMutation::PanelGroupSplit {
                view,
                target_group,
                direction,
                ratio,
                viewport,
            } => self
                .execute_command(
                    CommandId::SplitView,
                    CommandSource::Application,
                    CommandTarget::Focused,
                    Some(CommandParameters::SplitView {
                        view,
                        target_group,
                        direction,
                        ratio,
                        viewport,
                    }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::PanelGroupMerge {
                source_group,
                target_group,
            } => self
                .execute_command(
                    CommandId::MergeGroups,
                    CommandSource::Application,
                    CommandTarget::Focused,
                    Some(CommandParameters::MergeGroups {
                        source_group,
                        target_group,
                    }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::PanelGroupResize {
                path,
                ratio,
                viewport,
            } => self
                .execute_command(
                    CommandId::ResizeSplit,
                    CommandSource::Application,
                    CommandTarget::Focused,
                    Some(CommandParameters::ResizeSplit {
                        path,
                        ratio,
                        viewport,
                    }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::PanelActivate { view } => self
                .activate_view(view, window, cx)
                .map_err(runtime_mutation_error),
            RuntimeMutation::PanelMove {
                view,
                target_group,
                target_index,
            } => self
                .execute_command(
                    CommandId::MoveView,
                    CommandSource::Application,
                    CommandTarget::Focused,
                    Some(CommandParameters::MoveView {
                        view,
                        target_group,
                        target_index,
                    }),
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
            RuntimeMutation::PanelOpenSettings => self
                .open_settings_from_request(window, cx)
                .map_err(runtime_mutation_error),
            RuntimeMutation::TerminalCreate { project_root, cwd } => {
                let project_root = project_root
                    .map(CanonicalPath::new)
                    .transpose()
                    .map_err(|_| invalid_request_error())?;
                let cwd = cwd
                    .map(CanonicalPath::new)
                    .transpose()
                    .map_err(|_| invalid_request_error())?;
                self.new_terminal_from_request(self.shell.clone(), project_root, cwd, window, cx)
                    .map(|_| ())
                    .map_err(runtime_mutation_error)
            }
            RuntimeMutation::TerminalInput {
                terminal_id,
                generation,
                data,
            } => {
                let terminal = self.runtime_terminal_view(
                    terminal_id,
                    generation,
                    RuntimeTerminalAccess::Running,
                    cx,
                )?;
                terminal
                    .update(cx, |view, cx| view.runtime_input(data.into_bytes(), cx))
                    .map_err(|_| not_ready_error("terminal not ready"))
            }
            RuntimeMutation::TerminalExecute {
                terminal_id,
                generation,
                command,
            } => {
                let terminal = self.runtime_terminal_view(
                    terminal_id,
                    generation,
                    RuntimeTerminalAccess::Running,
                    cx,
                )?;
                // execute 是显式执行：只追加一次回车，不模拟键盘事件。
                let mut bytes = command.into_bytes();
                bytes.push(b'\r');
                terminal
                    .update(cx, |view, cx| view.runtime_input(bytes, cx))
                    .map_err(|_| not_ready_error("terminal not ready"))
            }
            RuntimeMutation::TerminalClose {
                terminal_id,
                generation,
            } => {
                self.snapshot
                    .terminal(terminal_id)
                    .ok_or_else(invalid_request_error)?;
                if self.runtime_terminal_generation(terminal_id) != generation {
                    return Err(revision_conflict_error());
                }
                self.close_terminal(terminal_id, window, cx)
                    .map(|_| ())
                    .map_err(runtime_mutation_error)
            }
            RuntimeMutation::TerminalReconnect {
                terminal_id,
                generation,
            } => {
                self.snapshot
                    .terminal(terminal_id)
                    .ok_or_else(invalid_request_error)?;
                if self.runtime_terminal_generation(terminal_id) != generation {
                    return Err(revision_conflict_error());
                }
                self.reconnect_terminal(terminal_id, cx)
                    .map(|_| ())
                    .map_err(runtime_mutation_error)
            }
            RuntimeMutation::TerminalRename { terminal_id, title } => {
                self.snapshot
                    .terminal(terminal_id)
                    .ok_or_else(invalid_request_error)?;
                self.rename_terminal(terminal_id, title, cx)
                    .map(|_| ())
                    .map_err(runtime_mutation_error)
            }
            RuntimeMutation::AgentInput {
                agent_id,
                terminal_id,
                generation,
                data,
            } => {
                let terminal = self.runtime_terminal_view(
                    terminal_id,
                    generation,
                    RuntimeTerminalAccess::Running,
                    cx,
                )?;
                self.ensure_runtime_agent(&agent_id, terminal_id, cx)?;
                terminal
                    .update(cx, |view, cx| view.runtime_input(data.into_bytes(), cx))
                    .map_err(|_| not_ready_error("terminal not ready"))
            }
            RuntimeMutation::AgentClose {
                agent_id,
                terminal_id,
                generation,
            } => {
                self.snapshot
                    .terminal(terminal_id)
                    .ok_or_else(invalid_request_error)?;
                if self.runtime_terminal_generation(terminal_id) != generation {
                    return Err(revision_conflict_error());
                }
                self.ensure_runtime_agent(&agent_id, terminal_id, cx)?;
                self.close_terminal(terminal_id, window, cx)
                    .map(|_| ())
                    .map_err(runtime_mutation_error)
            }
            RuntimeMutation::AgentStart {
                kind,
                project_root,
                cwd,
            } => {
                let kind =
                    crate::ai_agents::AgentKind::parse(&kind).ok_or_else(invalid_request_error)?;
                self.start_agent_terminal(
                    kind.catalog().builtin_command.to_owned(),
                    project_root,
                    cwd,
                    window,
                    cx,
                )
            }
            RuntimeMutation::AgentResume {
                source,
                session_id,
                session_file,
                project_root,
                cwd,
            } => {
                let session = slterm_agents::AgentSession {
                    source,
                    session_id,
                    session_file,
                };
                let command = session
                    .resume_command()
                    .ok_or_else(agent_command_unsupported)?;
                self.start_agent_terminal(command, project_root, cwd, window, cx)
            }
            RuntimeMutation::AgentFork {
                source,
                session_id,
                project_root,
                cwd,
            } => {
                let kind = crate::ai_agents::AgentKind::parse(&source)
                    .ok_or_else(invalid_request_error)?;
                let command = kind
                    .fork_command(&session_id)
                    .ok_or_else(agent_command_unsupported)?;
                self.start_agent_terminal(command, project_root, cwd, window, cx)
            }
            RuntimeMutation::PanelClose { view, documents } => {
                self.start_runtime_panel_close(view, documents, window, cx)
            }
            RuntimeMutation::AppFocus => {
                crate::windowing::focus::focus_window(window);
                cx.notify();
                Ok(())
            }
            RuntimeMutation::AppClose { documents, hooks } => {
                self.start_runtime_application_close(documents, hooks, window, cx)
            }
            // Git 破坏写与打开由 handle_runtime_mutation 分流到目标解析，不到达这里。
            RuntimeMutation::GitRollback { .. }
            | RuntimeMutation::GitDelete { .. }
            | RuntimeMutation::GitOpenDiff { .. }
            | RuntimeMutation::GitOpenDeleted { .. }
            | RuntimeMutation::PanelOpenFile { .. }
            | RuntimeMutation::PanelSave { .. } => Err(invalid_request_error()),
        }
    }

    /// 新建终端后把 AgentCatalog 构造的命令排入首个真实提示符注入。
    fn start_agent_terminal(
        &mut self,
        command: String,
        project_root: Option<String>,
        cwd: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        let project_root = project_root
            .map(CanonicalPath::new)
            .transpose()
            .map_err(|_| invalid_request_error())?;
        let cwd = cwd
            .map(CanonicalPath::new)
            .transpose()
            .map_err(|_| invalid_request_error())?;
        let panel_id = self
            .new_terminal_from_request(self.shell.clone(), project_root, cwd, window, cx)
            .map_err(runtime_mutation_error)?;
        let terminal_id = self
            .terminal_identity(panel_id, cx)
            .ok_or_else(|| not_ready_error("terminal panel missing"))?;
        let project_id = self
            .snapshot
            .terminal(terminal_id)
            .map(slterm_workspace::TerminalRecord::project_id)
            .ok_or_else(|| not_ready_error("terminal record missing"))?;
        let (_, view) = self
            .terminal_panel(project_id, terminal_id, cx)
            .ok_or_else(|| not_ready_error("terminal panel missing"))?;
        view.update(cx, |view, cx| view.schedule_agent_command(command, cx));
        Ok(())
    }

    /// 写方法必须证明目标终端当前的对外 Agent 身份与请求一致。
    fn ensure_runtime_agent(
        &self,
        agent_id: &str,
        terminal_id: slterm_workspace::TerminalId,
        cx: &App,
    ) -> Result<(), JsonRpcError> {
        if self.runtime_agent_id(terminal_id, cx).as_deref() == Some(agent_id) {
            Ok(())
        } else {
            Err(unknown_agent_error())
        }
    }

    /// 目标终端的对外 Agent 身份；纯 Shell 或无关联时为 None。
    fn runtime_agent_id(
        &self,
        terminal_id: slterm_workspace::TerminalId,
        cx: &App,
    ) -> Option<String> {
        let terminal_id = terminal_id.to_string();
        self.runtime_agents(cx)
            .ok()?
            .into_iter()
            .find(|agent| agent.terminal_id == terminal_id)
            .map(|agent| agent.agent_id)
    }

    /// 当前 Registry 代次；没有存活 worker 的持久化记录为零。
    fn runtime_terminal_generation(&self, terminal_id: slterm_workspace::TerminalId) -> u64 {
        self.terminals
            .borrow()
            .target(terminal_id)
            .map(|target| target.generation().get())
            .unwrap_or(0)
    }

    /// Runtime 只读文本快照：校验稳定身份与代次后把 worker 命令入队。
    ///
    /// 等待由调用方在后台完成，前台不锁实时网格；面板已退出仍可读最后画面。
    pub(super) fn handle_runtime_read(
        &self,
        terminal_id: slterm_workspace::TerminalId,
        generation: u64,
        agent_id: Option<&str>,
        cx: &App,
    ) -> Result<std::sync::mpsc::Receiver<slterm_terminal::term::TextSnapshot>, JsonRpcError> {
        let terminal = self.runtime_terminal_view(
            terminal_id,
            generation,
            RuntimeTerminalAccess::Display,
            cx,
        )?;
        if let Some(agent_id) = agent_id {
            self.ensure_runtime_agent(agent_id, terminal_id, cx)?;
        }
        terminal
            .read(cx)
            .runtime_text_snapshot()
            .map_err(|_| not_ready_error("terminal not ready"))
    }

    /// Runtime 终端等待：返回最近发布帧的 revision；前台不阻塞等待变化。
    pub(super) fn handle_runtime_wait(
        &self,
        terminal_id: slterm_workspace::TerminalId,
        generation: u64,
        agent_id: Option<&str>,
        cx: &App,
    ) -> Result<u64, JsonRpcError> {
        let terminal = self.runtime_terminal_view(
            terminal_id,
            generation,
            RuntimeTerminalAccess::Display,
            cx,
        )?;
        if let Some(agent_id) = agent_id {
            self.ensure_runtime_agent(agent_id, terminal_id, cx)?;
        }
        Ok(terminal.read(cx).runtime_revision())
    }

    /// 按完整目标（稳定身份 + PTY 代次）定位仍存活的终端视图。
    ///
    /// 写输入要求 Launch 意图；只读画面允许已退出终端。代次必须与请求一致
    /// 且面板仍存活，任一不满足都明确失败，不猜测重连后的新进程。
    fn runtime_terminal_view(
        &self,
        terminal_id: slterm_workspace::TerminalId,
        generation: u64,
        access: RuntimeTerminalAccess,
        cx: &App,
    ) -> Result<Entity<crate::gpui_terminal::view::TerminalView>, JsonRpcError> {
        let record = self
            .snapshot
            .terminal(terminal_id)
            .ok_or_else(invalid_request_error)?;
        if access == RuntimeTerminalAccess::Running && record.run_intent() != RunIntent::Launch {
            return Err(not_ready_error("terminal not running"));
        }
        let target = self
            .terminals
            .borrow()
            .target(terminal_id)
            .ok_or_else(|| not_ready_error("terminal target missing"))?;
        if target.generation().get() != generation {
            return Err(not_ready_error("terminal generation mismatch"));
        }
        let project_id = record.project_id();
        let view = ViewKey::Terminal {
            project_id,
            terminal_id,
        };
        let project = self
            .projects
            .get(&project_id)
            .ok_or_else(|| not_ready_error("project view missing"))?;
        project
            .read(cx)
            .panels
            .values()
            .find(|host| host.view_key == view)
            .and_then(|host| host.terminal().cloned())
            .ok_or_else(|| not_ready_error("terminal panel missing"))
    }

    /// Runtime Git 读取目标：只解析项目与仓库入口，不在前台执行 git。
    pub(super) fn runtime_git_target(
        &self,
        project_id: Option<slterm_workspace::ProjectId>,
    ) -> Result<crate::app_shell::RuntimeGitTarget, JsonRpcError> {
        let project_id = match project_id {
            Some(project_id) => project_id,
            None => self
                .active_project()
                .map_err(|_| not_ready_error("no active project"))?,
        };
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or_else(invalid_request_error)?;
        Ok(crate::app_shell::RuntimeGitTarget {
            project_id,
            root: project.root().clone(),
            git: std::sync::Arc::clone(&self.git),
        })
    }

    /// Runtime Git diff 目标：项目解析复用状态入口，路径必须是无 `..` 的仓库相对路径。
    pub(super) fn runtime_git_diff_target(
        &self,
        project_id: Option<slterm_workspace::ProjectId>,
        path: String,
    ) -> Result<crate::app_shell::RuntimeGitDiffTarget, JsonRpcError> {
        if path.trim().is_empty() || !valid_repo_relative_path(&path) {
            return Err(invalid_request_error());
        }
        let base = self.runtime_git_target(project_id)?;
        Ok(crate::app_shell::RuntimeGitDiffTarget {
            project_id: base.project_id,
            root: base.root,
            path,
            git: base.git,
        })
    }

    /// Runtime Agent 资源：持久会话身份与运行期仲裁状态派生，不另存映射。
    ///
    /// 纯 Shell 不产生关联行；同一原生会话 ID 可出现在多个终端关联中。
    pub(super) fn runtime_agents(&self, cx: &App) -> Result<Vec<AgentResource>, JsonRpcError> {
        let mut agents = Vec::new();
        for terminal in self.snapshot.terminals() {
            let Some((panel_id, _)) = self.terminal_panel(terminal.project_id(), terminal.id(), cx)
            else {
                continue;
            };
            let pane = PaneId(panel_id.sequence());
            let session = self
                .agents
                .session_agent(pane)
                .or_else(|| terminal.agent().cloned());
            let Some(session) = session else {
                continue;
            };
            let status = self
                .agents
                .host(pane)
                .map(|host| host.status())
                .unwrap_or_default();
            let agent_id = match &session.session_id {
                Some(native) => format!("{}:{native}", session.source),
                None => format!("{}@{}", session.source, terminal.id()),
            };
            agents.push(AgentResource {
                agent_id,
                terminal_id: terminal.id().to_string(),
                project_id: terminal.project_id().to_string(),
                kind: session.source.clone(),
                status: agent_status_text(status),
                native_session_id: session.session_id.clone(),
            });
        }
        Ok(agents)
    }

    /// 在 GPUI 前台按当前时刻生成同一份资源快照。
    ///
    /// 非 Unicode 路径与规范 ViewKey 编码失败明确报错，不把截断内容伪装成完整快照。
    pub(super) fn runtime_snapshot_resources(&self) -> Result<SnapshotResources, JsonRpcError> {
        let registry = self.snapshot.registry();
        let active = registry.active();
        let mut projects = Vec::with_capacity(registry.projects().len());
        let mut groups = Vec::new();
        for project in registry.projects() {
            let project_id = project.id().to_string();
            projects.push(ProjectResource {
                project_id: project_id.clone(),
                display_name: project.display_name().to_owned(),
                root: path_text(project.root())?,
                active: active == Some(project.id()),
            });
            let layout = project.layout();
            for group_id in layout.group_ids() {
                let group = layout.group(group_id).ok_or_else(snapshot_encoding_error)?;
                let mut views = Vec::with_capacity(group.views().len());
                for view in group.views() {
                    let key = view
                        .to_canonical_json()
                        .map_err(|_| snapshot_encoding_error())?;
                    views.push(ViewResource {
                        kind: view.kind_str().to_owned(),
                        key,
                    });
                }
                groups.push(PanelGroupResource {
                    project_id: project_id.clone(),
                    group_id: group_id.to_string(),
                    views,
                    active_view: group
                        .active()
                        .to_canonical_json()
                        .map_err(|_| snapshot_encoding_error())?,
                });
            }
        }

        let mut terminals = Vec::new();
        for terminal in self.snapshot.terminals() {
            // 代次以 Registry 为准；没有存活 worker 的持久化记录为零。
            let generation = self
                .terminals
                .borrow()
                .target(terminal.id())
                .map(|target| target.generation().get())
                .unwrap_or(0);
            terminals.push(TerminalResource {
                terminal_id: terminal.id().to_string(),
                project_id: terminal.project_id().to_string(),
                number: terminal.number(),
                cwd: path_text(terminal.cwd())?,
                shell: path_text(terminal.shell())?,
                title: terminal.custom_title().map(str::to_owned),
                run_intent: match terminal.run_intent() {
                    RunIntent::Launch => "launch".to_owned(),
                    RunIntent::Exited => "exited".to_owned(),
                },
                generation,
            });
        }

        let mut documents = Vec::new();
        for (document_id, location) in self.snapshot.documents() {
            documents.push(DocumentResource {
                document_id: document_id.to_string(),
                path: path_text(location.canonical_path())?,
            });
        }

        Ok(SnapshotResources {
            projects,
            groups,
            terminals,
            documents,
        })
    }
}

/// 文本视图绑定的 Document；其他视图没有可保存内容。
fn view_document_id(view: &ViewKey) -> Option<slterm_document::DocumentId> {
    match view {
        ViewKey::Editor { document_id, .. }
        | ViewKey::Markdown { document_id, .. }
        | ViewKey::Diff { document_id, .. } => Some(*document_id),
        _ => None,
    }
}

fn agent_status_text(status: AgentStatus) -> String {
    match status {
        AgentStatus::Idle => "idle",
        AgentStatus::Working => "working",
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Unknown => "unknown",
    }
    .to_owned()
}

fn path_text(path: &CanonicalPath) -> Result<String, JsonRpcError> {
    path.as_path()
        .to_str()
        .map(str::to_owned)
        .ok_or_else(snapshot_encoding_error)
}

/// 写意图处理结果：立即回执，或已移交异步完成回调。
pub(super) enum RuntimeMutationDispatch {
    Reply(Result<RuntimeMutationReply, JsonRpcError>),
    Deferred,
}

/// 异步写意图完成回执守卫：正常发送后不重复；释放时未发送即回执 Closing。
struct RuntimeMutationCompletion {
    completion: Option<std::sync::mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>>,
}

impl RuntimeMutationCompletion {
    fn new(
        completion: std::sync::mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
    ) -> Self {
        Self {
            completion: Some(completion),
        }
    }

    fn send(&mut self, reply: RuntimeMutationReply) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.try_send(Ok(reply));
        }
    }
}

impl Drop for RuntimeMutationCompletion {
    fn drop(&mut self) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.try_send(Err(closing_error()));
        }
    }
}

fn closing_error() -> JsonRpcError {
    control_error(DomainErrorCode::Closing, -32004, "closing")
}

fn invalid_request_error() -> JsonRpcError {
    control_error(DomainErrorCode::InvalidRequest, -32000, "invalid request")
}

fn not_ready_error(message: &'static str) -> JsonRpcError {
    control_error(DomainErrorCode::NotReady, -32001, message)
}

/// 目标代次与快照不一致：客户端必须重新快照后再写。
fn revision_conflict_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::RevisionConflict,
        -32005,
        "revision conflict",
    )
}

/// 非交互关闭缺少显式处置决定：调用方必须使用新 mutation_id 重试。
fn decision_required_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::DecisionRequired,
        -32006,
        "decision required",
    )
}

/// 请求的对外 Agent 身份与目标终端当前关联不一致。
fn unknown_agent_error() -> JsonRpcError {
    control_error(DomainErrorCode::UnknownAgent, -32000, "unknown agent")
}

/// 仓库相对路径：非空、无 NUL、不以分隔符开头且不含 `.`/`..` 组件。
fn valid_repo_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains(['\0', '\\']) {
        return false;
    }
    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

/// AgentCatalog 未声明该动作或不接受该会话身份。
fn agent_command_unsupported() -> JsonRpcError {
    control_error(
        DomainErrorCode::InvalidRequest,
        -32000,
        "agent command unsupported",
    )
}

/// 写意图错误映射为封闭领域码；不携带路径、SQL 或正文。
fn runtime_mutation_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_error(),
        AppRootError::HooksDraftPending => decision_required_error(),
        AppRootError::Focus(FocusError::ModalActive) => {
            control_error(DomainErrorCode::Unauthorized, -32002, "modal active")
        }
        AppRootError::NewTerminalTargetUnauthorized | AppRootError::OpenFileTargetUnauthorized => {
            control_error(DomainErrorCode::Unauthorized, -32002, "target unauthorized")
        }
        AppRootError::UnexpectedParameters
        | AppRootError::Registry(_)
        | AppRootError::Project(_)
        | AppRootError::Snapshot(_)
        | AppRootError::Layout(_) => control_error(
            DomainErrorCode::InvalidRequest,
            -32000,
            "invalid project request",
        ),
        _ => control_error(DomainErrorCode::NotReady, -32001, "mutation not ready"),
    }
}

fn snapshot_encoding_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::NotReady,
        -32001,
        "snapshot encoding failed",
    )
}
