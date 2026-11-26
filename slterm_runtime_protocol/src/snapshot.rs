//! Runtime 快照资源 DTO；只含身份与元数据，不含正文或运行句柄。

use serde::{Deserialize, Serialize};

/// 快照中的项目资源身份与元数据。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectResource {
    pub project_id: String,
    pub display_name: String,
    pub root: String,
    pub active: bool,
}

/// 快照中的面板组；`views` 按持久顺序，`active_view` 为组内激活视图。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelGroupResource {
    pub project_id: String,
    pub group_id: String,
    pub views: Vec<ViewResource>,
    pub active_view: String,
}

/// 视图类型的线缆枚举；与持久 ViewKey 注册点使用同一文本。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    Terminal,
    Editor,
    Markdown,
    Diff,
    GitShow,
    Settings,
    SideView,
    AppSettingsOverlay,
}

/// 视图身份：封闭类型与规范 ViewKey JSON 文本。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ViewResource {
    pub kind: ViewKind,
    pub key: String,
}

/// 终端持久运行意图的线缆枚举。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TerminalRunIntent {
    Launch,
    Exited,
}

/// 快照中的逻辑终端；不含 PTY、输出或回滚正文。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalResource {
    pub terminal_id: String,
    pub project_id: String,
    pub number: u64,
    pub cwd: String,
    pub shell: String,
    pub title: Option<String>,
    /// 封闭运行意图；新增值必须先登记协议。
    pub run_intent: TerminalRunIntent,
    /// 当前 PTY 代次；零表示没有存活 worker（持久化 Exited）。
    pub generation: u64,
}

/// 快照中的 Document 位置元数据；不含正文。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DocumentResource {
    pub document_id: String,
    pub path: String,
}

/// 项目的分割树；`root` 为空表示空布局。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectLayoutResource {
    pub project_id: String,
    pub root: Option<LayoutNodeResource>,
}

/// 分割树节点；布尔路径即沿 first=false／second=true 的走法。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayoutNodeResource {
    Group {
        group_id: String,
    },
    Split {
        direction: crate::SplitDirectionWire,
        ratio: f32,
        first: Box<LayoutNodeResource>,
        second: Box<LayoutNodeResource>,
    },
}

/// 同一代次资源模型：项目、面板组、布局树、终端与 Document。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SnapshotResources {
    pub projects: Vec<ProjectResource>,
    pub groups: Vec<PanelGroupResource>,
    pub layouts: Vec<ProjectLayoutResource>,
    pub terminals: Vec<TerminalResource>,
    pub documents: Vec<DocumentResource>,
}

/// `runtime.snapshot` 的不可变资源快照与世代。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SnapshotResponse {
    pub store_epoch: String,
    pub stream_epoch: String,
    pub revision: u64,
    pub resources: SnapshotResources,
}
