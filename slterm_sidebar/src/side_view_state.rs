//! SideView 私有 ViewState 的精确 schema。

use std::fmt;
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use slterm_settings::AgentRootId;
use slterm_workspace::{
    InvalidViewState, ProjectId, SideViewId, ViewKey, ViewKeyKind, ViewStateSchema,
};

/// 注册按完整 SideView ViewKey 分派的私有状态解码器。
pub fn side_view_state_schema() -> ViewStateSchema {
    ViewStateSchema::new(ViewKeyKind::SideView, decode_side_view_state)
}

fn decode_side_view_state(key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
    let ViewKey::SideView { side_view_id } = key else {
        return Err(InvalidViewState);
    };
    match side_view_id {
        SideViewId::Nav => decode::<NavigationState>(text),
        SideViewId::Explorer => decode::<ExplorerState>(text),
        SideViewId::Commit => decode::<CommitState>(text),
        SideViewId::AgentFiles => decode::<AgentFilesState>(text),
    }
}

fn decode<T>(text: &str) -> Result<Value, InvalidViewState>
where
    T: DeserializeOwned + Normalize + Serialize,
{
    let mut state: T = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
    state.normalize()?;
    serde_json::to_value(state).map_err(|_| InvalidViewState)
}

trait Normalize {
    fn normalize(&mut self) -> Result<(), InvalidViewState>;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationState {
    projects: Vec<NavigationProjectState>,
    unassigned_expanded: bool,
    scroll: f64,
}

impl Normalize for NavigationState {
    fn normalize(&mut self) -> Result<(), InvalidViewState> {
        canonical_unique_by(&mut self.projects, |project| project.project_id)?;
        normalize_scroll(&mut self.scroll)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationProjectState {
    #[serde(
        serialize_with = "serialize_stable_id",
        deserialize_with = "deserialize_stable_id"
    )]
    project_id: ProjectId,
    expanded: bool,
    history_expanded: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplorerState {
    projects: Vec<ExplorerProjectState>,
}

impl Normalize for ExplorerState {
    fn normalize(&mut self) -> Result<(), InvalidViewState> {
        canonical_unique_by(&mut self.projects, |project| project.project_id)?;
        for project in &mut self.projects {
            canonical_unique(&mut project.expanded)?;
            normalize_scroll(&mut project.scroll)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplorerProjectState {
    #[serde(
        serialize_with = "serialize_stable_id",
        deserialize_with = "deserialize_stable_id"
    )]
    project_id: ProjectId,
    expanded: Vec<SideViewPath>,
    #[serde(deserialize_with = "deserialize_required_option")]
    current_row: Option<SideViewPath>,
    scroll: f64,
    show_ignored: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitState {
    changes_collapsed: bool,
    unversioned_collapsed: bool,
    ignored_collapsed: bool,
    show_ignored: bool,
    scroll: f64,
}

impl Normalize for CommitState {
    fn normalize(&mut self) -> Result<(), InvalidViewState> {
        normalize_scroll(&mut self.scroll)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentFilesState {
    roots: Vec<AgentRootState>,
    #[serde(deserialize_with = "deserialize_required_option")]
    current_row: Option<AgentCurrentRow>,
    scroll: f64,
}

impl Normalize for AgentFilesState {
    fn normalize(&mut self) -> Result<(), InvalidViewState> {
        canonical_unique_by(&mut self.roots, |root| root.agent_root_id)?;
        for root in &mut self.roots {
            canonical_unique(&mut root.directories)?;
        }
        if self.current_row.as_ref().is_some_and(|current| {
            !self
                .roots
                .iter()
                .any(|root| root.agent_root_id == current.agent_root_id)
        }) {
            return Err(InvalidViewState);
        }
        normalize_scroll(&mut self.scroll)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRootState {
    #[serde(
        serialize_with = "serialize_stable_id",
        deserialize_with = "deserialize_stable_id"
    )]
    agent_root_id: AgentRootId,
    expanded: bool,
    directories: Vec<SideViewPath>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentCurrentRow {
    #[serde(
        serialize_with = "serialize_stable_id",
        deserialize_with = "deserialize_stable_id"
    )]
    agent_root_id: AgentRootId,
    #[serde(deserialize_with = "deserialize_required_option")]
    path: Option<SideViewPath>,
}

/// 视图状态内的根相对路径；空路径由根节点身份表达，不伪造为文件行。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
struct SideViewPath(String);

impl SideViewPath {
    fn new(text: String) -> Result<Self, InvalidViewState> {
        let bytes = text.as_bytes();
        let drive_prefixed = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if text.is_empty()
            || text.starts_with('/')
            || text.ends_with('/')
            || bytes.iter().any(|byte| matches!(byte, b'\\' | 0))
            || drive_prefixed
            || text
                .split('/')
                .any(|segment| matches!(segment, "" | "." | ".."))
        {
            return Err(InvalidViewState);
        }
        Ok(Self(text))
    }
}

impl<'de> Deserialize<'de> for SideViewPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

fn canonical_unique_by<T, K: Ord>(
    items: &mut [T],
    key: impl Fn(&T) -> K,
) -> Result<(), InvalidViewState> {
    items.sort_by_key(&key);
    if items.windows(2).any(|pair| key(&pair[0]) == key(&pair[1])) {
        return Err(InvalidViewState);
    }
    Ok(())
}

fn canonical_unique<T: Ord>(items: &mut [T]) -> Result<(), InvalidViewState> {
    items.sort();
    if items.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(InvalidViewState);
    }
    Ok(())
}

fn normalize_scroll(scroll: &mut f64) -> Result<(), InvalidViewState> {
    if !scroll.is_finite() || *scroll < 0.0 {
        return Err(InvalidViewState);
    }
    if *scroll == 0.0 {
        *scroll = 0.0;
    }
    Ok(())
}

fn serialize_stable_id<T, S>(id: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: fmt::Display,
    S: Serializer,
{
    serializer.serialize_str(&id.to_string())
}

fn deserialize_stable_id<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: fmt::Display,
    D: Deserializer<'de>,
{
    T::from_str(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

/// 带自定义反序列化的 Option 默认关闭缺字段隐式 None，null 仍表示无当前行。
fn deserialize_required_option<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer)
}
