//! Runtime v2 资源快照：从前台唯一项目快照派生只读 DTO。

use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{
    DocumentResource, DomainErrorCode, JsonRpcError, PanelGroupResource, ProjectResource,
    SnapshotResources, TerminalResource, ViewResource,
};
use slterm_workspace::RunIntent;

use super::AppRoot;
use crate::app_shell::control_error;

impl AppRoot {
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

fn snapshot_encoding_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::NotReady,
        -32001,
        "snapshot encoding failed",
    )
}
