//! Runtime 写意图的封闭集合：server 解析参数后交给应用壳执行。
//!
//! 意图只携带稳定身份与已校验参数；路径、授权、事务与关闭策略
//! 全部由 AppRoot 复用既有命令目录处理。

use slterm_workspace::ProjectId;

/// 当前可执行的写意图；新增方法必须先接入真实命令再登记。
///
/// 前缀表达资源域；Panel/Terminal 等方法族接入后变体集合将自然分化。
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl RuntimeMutation {
    /// 线缆方法名；同时作为 Operation 的方法身份。
    pub(crate) fn method(&self) -> &'static str {
        match self {
            Self::ProjectActivate { .. } => "project.activate",
            Self::ProjectRename { .. } => "project.rename",
            Self::ProjectReorder { .. } => "project.reorder",
            Self::ProjectRemove { .. } => "project.remove",
        }
    }

    /// 规范参数值：只含稳定身份与已校验输入，用于计算幂等散列。
    pub(crate) fn params_value(&self) -> serde_json::Value {
        match self {
            Self::ProjectActivate { project_id } => {
                serde_json::json!({"project_id": project_id.to_string()})
            }
            Self::ProjectRename { project_id, name } => {
                serde_json::json!({"project_id": project_id.to_string(), "name": name})
            }
            Self::ProjectReorder {
                project_id,
                new_index,
            } => serde_json::json!({
                "project_id": project_id.to_string(),
                "new_index": new_index,
            }),
            Self::ProjectRemove { project_id } => {
                serde_json::json!({"project_id": project_id.to_string()})
            }
        }
    }
}

/// server 投递给派发器的写请求。
pub(crate) struct RuntimeMutationRequest {
    pub(crate) client_id: String,
    pub(crate) mutation_id: String,
    pub(crate) expected_revision: Option<u64>,
    pub(crate) mutation: RuntimeMutation,
}
