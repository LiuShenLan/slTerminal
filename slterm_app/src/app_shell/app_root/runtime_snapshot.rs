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
use super::focus_manager::FocusError;
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
    pub(super) fn handle_runtime_mutation(
        &mut self,
        mutation: RuntimeMutation,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), JsonRpcError> {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return Err(closing_error());
        }
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
            RuntimeMutation::ProjectRemove { project_id } => self
                .execute_command(
                    CommandId::RemoveProject,
                    CommandSource::Application,
                    CommandTarget::Project(project_id),
                    None,
                    window,
                    cx,
                )
                .map(|_| ())
                .map_err(runtime_mutation_error),
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
        }
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

/// 请求的对外 Agent 身份与目标终端当前关联不一致。
fn unknown_agent_error() -> JsonRpcError {
    control_error(DomainErrorCode::UnknownAgent, -32000, "unknown agent")
}

/// 写意图错误映射为封闭领域码；不携带路径、SQL 或正文。
fn runtime_mutation_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_error(),
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
