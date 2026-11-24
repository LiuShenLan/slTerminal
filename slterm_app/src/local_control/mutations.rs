//! Runtime 写意图的封闭集合：server 解析参数后交给应用壳执行。
//!
//! 意图只携带稳定身份与已校验参数；路径、授权、事务与关闭策略
//! 全部由 AppRoot 复用既有命令目录处理。

use slterm_split::SplitDirection;
use slterm_workspace::{LogicalSize, PanelGroupId, ProjectId, ViewKey};

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
