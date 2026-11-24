//! Runtime v2 资源快照：从前台唯一项目快照派生只读 DTO。

use gpui::{Context, Window};
use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{
    DocumentResource, DomainErrorCode, JsonRpcError, PanelGroupResource, ProjectResource,
    SnapshotResources, TerminalResource, ViewResource,
};
use slterm_workspace::RunIntent;

use super::AppRoot;
use super::command_registry::{CommandId, CommandParameters, CommandSource, CommandTarget};
use super::commands::AppRootError;
use super::focus_manager::FocusError;
use crate::app_shell::control_error;
use crate::local_control::RuntimeMutation;

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
        }
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

fn path_text(path: &CanonicalPath) -> Result<String, JsonRpcError> {
    path.as_path()
        .to_str()
        .map(str::to_owned)
        .ok_or_else(snapshot_encoding_error)
}

fn closing_error() -> JsonRpcError {
    control_error(DomainErrorCode::Closing, -32004, "closing")
}

/// 写意图错误映射为封闭领域码；不携带路径、SQL 或正文。
fn runtime_mutation_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_error(),
        AppRootError::Focus(FocusError::ModalActive) => {
            control_error(DomainErrorCode::Unauthorized, -32002, "modal active")
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
