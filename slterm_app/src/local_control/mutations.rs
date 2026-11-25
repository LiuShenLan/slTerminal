//! Runtime 写意图的封闭集合：server 解析参数后交给应用壳执行。
//!
//! 意图只携带稳定身份与已校验参数；路径、授权、事务与关闭策略
//! 全部由 AppRoot 复用既有命令目录处理。

use slterm_runtime_protocol::{
    CloseDocumentsPolicy, CloseHooksPolicy, GitDeleteExpected, GitRollbackExpected,
};
use slterm_split::SplitDirection;
use slterm_workspace::{LogicalSize, PanelGroupId, ProjectId, TerminalId, ViewKey};

/// 当前可执行的写意图；新增方法必须先接入真实命令再登记。
///
/// 前缀表达资源域；Terminal/Git 等方法族接入后变体集合将自然分化。
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeMutation {
    ProjectActivate {
        project_id: ProjectId,
    },
    ProjectRename {
        project_id: ProjectId,
        name: String,
    },
    ProjectReorder {
        project_id: ProjectId,
        new_index: usize,
    },
    ProjectRemove {
        project_id: ProjectId,
    },
    PanelGroupSplit {
        view: ViewKey,
        target_group: PanelGroupId,
        direction: SplitDirection,
        ratio: f32,
        viewport: LogicalSize,
    },
    PanelGroupMerge {
        source_group: PanelGroupId,
        target_group: PanelGroupId,
    },
    PanelGroupResize {
        path: Vec<bool>,
        ratio: f32,
        viewport: LogicalSize,
    },
    PanelActivate {
        view: ViewKey,
    },
    PanelMove {
        view: ViewKey,
        target_group: PanelGroupId,
        target_index: usize,
    },
    PanelOpenSettings,
    PanelOpenFile {
        path: String,
    },
    PanelSave {
        view: ViewKey,
    },
    PanelClose {
        view: ViewKey,
        documents: Option<CloseDocumentsPolicy>,
    },
    AppFocus,
    AppClose {
        documents: Option<CloseDocumentsPolicy>,
        hooks: Option<CloseHooksPolicy>,
    },
    TerminalCreate {
        project_root: Option<String>,
        cwd: Option<String>,
    },
    TerminalInput {
        terminal_id: TerminalId,
        generation: u64,
        data: String,
    },
    TerminalExecute {
        terminal_id: TerminalId,
        generation: u64,
        command: String,
    },
    TerminalClose {
        terminal_id: TerminalId,
        generation: u64,
    },
    TerminalReconnect {
        terminal_id: TerminalId,
        generation: u64,
    },
    TerminalRename {
        terminal_id: TerminalId,
        title: Option<String>,
    },
    AgentInput {
        agent_id: String,
        terminal_id: TerminalId,
        generation: u64,
        data: String,
    },
    AgentClose {
        agent_id: String,
        terminal_id: TerminalId,
        generation: u64,
    },
    AgentStart {
        kind: String,
        project_root: Option<String>,
        cwd: Option<String>,
    },
    AgentResume {
        source: String,
        session_id: Option<String>,
        session_file: Option<String>,
        project_root: Option<String>,
        cwd: Option<String>,
    },
    AgentFork {
        source: String,
        session_id: String,
        project_root: Option<String>,
        cwd: Option<String>,
    },
    GitRollback {
        project_id: Option<ProjectId>,
        path: String,
        expected: GitRollbackExpected,
    },
    GitDelete {
        project_id: Option<ProjectId>,
        path: String,
        expected: GitDeleteExpected,
        permanent: bool,
    },
    GitOpenDiff {
        project_id: Option<ProjectId>,
        path: String,
    },
    GitOpenDeleted {
        project_id: Option<ProjectId>,
        path: String,
    },
}

impl RuntimeMutation {
    /// 线缆方法名；同时作为 Operation 的方法身份。
    pub(crate) fn method(&self) -> &'static str {
        match self {
            Self::ProjectActivate { .. } => "project.activate",
            Self::ProjectRename { .. } => "project.rename",
            Self::ProjectReorder { .. } => "project.reorder",
            Self::ProjectRemove { .. } => "project.remove",
            Self::PanelGroupSplit { .. } => "panel_group.split",
            Self::PanelGroupMerge { .. } => "panel_group.merge",
            Self::PanelGroupResize { .. } => "panel_group.resize",
            Self::PanelActivate { .. } => "panel.activate",
            Self::PanelMove { .. } => "panel.move",
            Self::PanelOpenSettings => "panel.open_settings",
            Self::PanelOpenFile { .. } => "panel.open_file",
            Self::PanelSave { .. } => "panel.save",
            Self::PanelClose { .. } => "panel.close",
            Self::AppFocus => "app.focus",
            Self::AppClose { .. } => "app.close",
            Self::TerminalCreate { .. } => "terminal.create",
            Self::TerminalInput { .. } => "terminal.input",
            Self::TerminalExecute { .. } => "terminal.execute",
            Self::TerminalClose { .. } => "terminal.close",
            Self::TerminalReconnect { .. } => "terminal.reconnect",
            Self::TerminalRename { .. } => "terminal.rename",
            Self::AgentInput { .. } => "agent.input",
            Self::AgentClose { .. } => "agent.close",
            Self::AgentStart { .. } => "agent.start",
            Self::AgentResume { .. } => "agent.resume",
            Self::AgentFork { .. } => "agent.fork",
            Self::GitRollback { .. } => "git.rollback",
            Self::GitDelete { .. } => "git.delete",
            Self::GitOpenDiff { .. } => "git.open_diff",
            Self::GitOpenDeleted { .. } => "git.open_deleted",
        }
    }

    /// 规范参数值：只含稳定身份与已校验输入，用于计算幂等散列。
    ///
    /// 视图身份必须可无损编码；非 Unicode 路径等失败明确拒绝。
    pub(crate) fn params_value(&self) -> Result<serde_json::Value, ()> {
        match self {
            Self::ProjectActivate { project_id } => {
                Ok(serde_json::json!({"project_id": project_id.to_string()}))
            }
            Self::ProjectRename { project_id, name } => {
                Ok(serde_json::json!({"project_id": project_id.to_string(), "name": name}))
            }
            Self::ProjectReorder {
                project_id,
                new_index,
            } => Ok(serde_json::json!({
                "project_id": project_id.to_string(),
                "new_index": new_index,
            })),
            Self::ProjectRemove { project_id } => {
                Ok(serde_json::json!({"project_id": project_id.to_string()}))
            }
            Self::PanelGroupSplit {
                view,
                target_group,
                direction,
                ratio,
                viewport,
            } => Ok(serde_json::json!({
                "view": view.to_canonical_json().map_err(|_| ())?,
                "target_group": target_group.to_string(),
                "direction": split_direction_text(*direction),
                "ratio": ratio,
                "viewport": viewport_value(*viewport),
            })),
            Self::PanelGroupMerge {
                source_group,
                target_group,
            } => Ok(serde_json::json!({
                "source_group": source_group.to_string(),
                "target_group": target_group.to_string(),
            })),
            Self::PanelGroupResize {
                path,
                ratio,
                viewport,
            } => Ok(serde_json::json!({
                "path": path,
                "ratio": ratio,
                "viewport": viewport_value(*viewport),
            })),
            Self::PanelActivate { view } => {
                Ok(serde_json::json!({"view": view.to_canonical_json().map_err(|_| ())?}))
            }
            Self::PanelMove {
                view,
                target_group,
                target_index,
            } => Ok(serde_json::json!({
                "view": view.to_canonical_json().map_err(|_| ())?,
                "target_group": target_group.to_string(),
                "target_index": target_index,
            })),
            Self::PanelOpenSettings => Ok(serde_json::json!({})),
            Self::PanelOpenFile { path } => Ok(serde_json::json!({"path": path})),
            Self::PanelSave { view } => Ok(serde_json::json!({
                "view": view.to_canonical_json().map_err(|_| ())?,
            })),
            Self::PanelClose { view, documents } => Ok(serde_json::json!({
                "view": view.to_canonical_json().map_err(|_| ())?,
                "documents": documents,
            })),
            Self::AppFocus => Ok(serde_json::json!({})),
            Self::AppClose { documents, hooks } => Ok(serde_json::json!({
                "documents": documents,
                "hooks": hooks,
            })),
            Self::TerminalCreate { project_root, cwd } => Ok(serde_json::json!({
                "project_root": project_root,
                "cwd": cwd,
            })),
            Self::TerminalInput {
                terminal_id,
                generation,
                data,
            } => Ok(serde_json::json!({
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
                "data": data,
            })),
            Self::TerminalExecute {
                terminal_id,
                generation,
                command,
            } => Ok(serde_json::json!({
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
                "command": command,
            })),
            Self::TerminalClose {
                terminal_id,
                generation,
            } => Ok(serde_json::json!({
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
            })),
            Self::TerminalReconnect {
                terminal_id,
                generation,
            } => Ok(serde_json::json!({
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
            })),
            Self::TerminalRename { terminal_id, title } => Ok(serde_json::json!({
                "terminal_id": terminal_id.to_string(),
                "title": title,
            })),
            Self::AgentInput {
                agent_id,
                terminal_id,
                generation,
                data,
            } => Ok(serde_json::json!({
                "agent_id": agent_id,
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
                "data": data,
            })),
            Self::AgentClose {
                agent_id,
                terminal_id,
                generation,
            } => Ok(serde_json::json!({
                "agent_id": agent_id,
                "terminal_id": terminal_id.to_string(),
                "generation": generation,
            })),
            Self::AgentStart {
                kind,
                project_root,
                cwd,
            } => Ok(serde_json::json!({
                "kind": kind,
                "project_root": project_root,
                "cwd": cwd,
            })),
            Self::AgentResume {
                source,
                session_id,
                session_file,
                project_root,
                cwd,
            } => Ok(serde_json::json!({
                "source": source,
                "session_id": session_id,
                "session_file": session_file,
                "project_root": project_root,
                "cwd": cwd,
            })),
            Self::AgentFork {
                source,
                session_id,
                project_root,
                cwd,
            } => Ok(serde_json::json!({
                "source": source,
                "session_id": session_id,
                "project_root": project_root,
                "cwd": cwd,
            })),
            Self::GitRollback {
                project_id,
                path,
                expected,
            } => Ok(serde_json::json!({
                "project_id": project_id.map(|id| id.to_string()),
                "path": path,
                "expected": expected,
            })),
            Self::GitDelete {
                project_id,
                path,
                expected,
                permanent,
            } => Ok(serde_json::json!({
                "project_id": project_id.map(|id| id.to_string()),
                "path": path,
                "expected": expected,
                "permanent": permanent,
            })),
            Self::GitOpenDiff { project_id, path } | Self::GitOpenDeleted { project_id, path } => {
                Ok(serde_json::json!({
                    "project_id": project_id.map(|id| id.to_string()),
                    "path": path,
                }))
            }
        }
    }
}

fn split_direction_text(direction: SplitDirection) -> &'static str {
    match direction {
        SplitDirection::LeftRight => "left_right",
        SplitDirection::TopBottom => "top_bottom",
    }
}

fn viewport_value(viewport: LogicalSize) -> serde_json::Value {
    serde_json::json!({"width": viewport.width, "height": viewport.height})
}

/// server 投递给派发器的写请求。
pub(crate) struct RuntimeMutationRequest {
    pub(crate) client_id: String,
    pub(crate) mutation_id: String,
    pub(crate) expected_revision: Option<u64>,
    pub(crate) mutation: RuntimeMutation,
}
