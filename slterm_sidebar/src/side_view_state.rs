//! SideView 私有 ViewState 的精确 schema。

use std::fmt;
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use slterm_settings::AgentRootId;
use slterm_workspace::{
    InvalidViewState, ProjectId, SideViewId, ViewKey, ViewKeyKind, ViewState, ViewStateError,
    ViewStateRegistry, ViewStateSchema,
};

/// 注册按完整 SideView ViewKey 分派的私有状态解码器。
pub fn side_view_state_schema() -> ViewStateSchema {
    ViewStateSchema::new(ViewKeyKind::SideView, decode_side_view_state)
}

/// SideView 运行态与持久状态共用的强类型事实。
#[derive(Clone, Debug)]
pub enum SideViewState {
    Nav(NavigationState),
    Explorer(ExplorerState),
    Commit(CommitState),
    AgentFiles(AgentFilesState),
}

impl SideViewState {
    pub fn new(id: SideViewId) -> Self {
        match id {
            SideViewId::Nav => Self::Nav(NavigationState::default()),
            SideViewId::Explorer => Self::Explorer(ExplorerState::default()),
            SideViewId::Commit => Self::Commit(CommitState::default()),
            SideViewId::AgentFiles => Self::AgentFiles(AgentFilesState::default()),
        }
    }

    pub const fn id(&self) -> SideViewId {
        match self {
            Self::Nav(_) => SideViewId::Nav,
            Self::Explorer(_) => SideViewId::Explorer,
            Self::Commit(_) => SideViewId::Commit,
            Self::AgentFiles(_) => SideViewId::AgentFiles,
        }
    }

    /// 编码只经已注册 schema，禁止绕过精确字段校验。
    pub fn to_view_state(&self, registry: &ViewStateRegistry) -> Result<ViewState, ViewStateError> {
        let value = match self {
            Self::Nav(state) => serde_json::to_value(state),
            Self::Explorer(state) => serde_json::to_value(state),
            Self::Commit(state) => serde_json::to_value(state),
            Self::AgentFiles(state) => serde_json::to_value(state),
        }
        .map_err(|_| ViewStateError::Encode)?;
        ViewState::new(
            &ViewKey::SideView {
                side_view_id: self.id(),
            },
            value,
            registry,
        )
    }

    /// 已恢复的 ViewState 转回强类型运行态，并校验视图身份。
    pub fn from_view_state(id: SideViewId, state: ViewState) -> Result<Self, InvalidViewState> {
        state
            .scope(&ViewKey::SideView { side_view_id: id })
            .map_err(|_| InvalidViewState)?;
        let value = state.into_state();
        match id {
            SideViewId::Nav => serde_json::from_value(value).map(Self::Nav),
            SideViewId::Explorer => serde_json::from_value(value).map(Self::Explorer),
            SideViewId::Commit => serde_json::from_value(value).map(Self::Commit),
            SideViewId::AgentFiles => serde_json::from_value(value).map(Self::AgentFiles),
        }
        .map_err(|_| InvalidViewState)
    }

    /// 删除指定项目分区；无项目分区的视图与缺失身份均返回未变化。
    pub fn remove_project(&mut self, project_id: ProjectId) -> bool {
        match self {
            Self::Nav(state) => {
                let previous = state.projects.len();
                state
                    .projects
                    .retain(|project| project.project_id != project_id);
                state.projects.len() != previous
            }
            Self::Explorer(state) => {
                let previous = state.projects.len();
                state
                    .projects
                    .retain(|project| project.project_id != project_id);
                state.projects.len() != previous
            }
            Self::Commit(_) | Self::AgentFiles(_) => false,
        }
    }

    /// 删除指定 AgentRoot 分区，并清除仍指向该根的当前行。
    pub fn remove_agent_root(&mut self, agent_root_id: AgentRootId) -> bool {
        let Self::AgentFiles(state) = self else {
            return false;
        };
        let previous = state.roots.len();
        state
            .roots
            .retain(|root| root.agent_root_id != agent_root_id);
        let removed = state.roots.len() != previous;
        if removed
            && state
                .current_row
                .as_ref()
                .is_some_and(|current| current.agent_root_id == agent_root_id)
        {
            state.current_row = None;
        }
        removed
    }
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationState {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerState {
    projects: Vec<ExplorerProjectState>,
}

impl ExplorerState {
    /// 按项目身份读取持久分区；缺失表示该项目尚无持久展开事实。
    pub fn project(&self, project_id: ProjectId) -> Option<&ExplorerProjectState> {
        self.projects
            .iter()
            .find(|project| project.project_id == project_id)
    }

    /// 按需创建分区；插入后保持稳定身份排序，编码字节与恢复顺序一致。
    pub fn project_mut(&mut self, project_id: ProjectId) -> &mut ExplorerProjectState {
        let index = match self
            .projects
            .binary_search_by_key(&project_id, |project| project.project_id)
        {
            Ok(index) => index,
            Err(index) => {
                self.projects
                    .insert(index, ExplorerProjectState::new(project_id));
                index
            }
        };
        &mut self.projects[index]
    }
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

/// Explorer 单项目持久分区；字段私有，运行态只经方法读写。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerProjectState {
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

impl ExplorerProjectState {
    fn new(project_id: ProjectId) -> Self {
        Self {
            project_id,
            expanded: Vec::new(),
            current_row: None,
            scroll: 0.0,
            show_ignored: false,
        }
    }

    pub fn expanded(&self) -> impl Iterator<Item = &SideViewPath> {
        self.expanded.iter()
    }

    pub fn current_row(&self) -> Option<&SideViewPath> {
        self.current_row.as_ref()
    }

    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    pub fn show_ignored(&self) -> bool {
        self.show_ignored
    }

    /// 展开集合是树快照；重复路径按最后一次状态去重并保持有序。
    pub fn set_expanded(&mut self, expanded: Vec<SideViewPath>) {
        let mut expanded = expanded;
        expanded.sort();
        expanded.dedup();
        self.expanded = expanded;
    }

    pub fn set_current_row(&mut self, row: Option<SideViewPath>) {
        self.current_row = row;
    }

    pub fn set_scroll(&mut self, scroll: f64) {
        self.scroll = scroll;
    }

    pub fn set_show_ignored(&mut self, show_ignored: bool) {
        self.show_ignored = show_ignored;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitState {
    changes_collapsed: bool,
    unversioned_collapsed: bool,
    ignored_collapsed: bool,
    show_ignored: bool,
    scroll: f64,
}

impl CommitState {
    pub fn changes_collapsed(&self) -> bool {
        self.changes_collapsed
    }

    pub fn unversioned_collapsed(&self) -> bool {
        self.unversioned_collapsed
    }

    pub fn ignored_collapsed(&self) -> bool {
        self.ignored_collapsed
    }

    pub fn show_ignored(&self) -> bool {
        self.show_ignored
    }

    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    pub fn set_changes_collapsed(&mut self, value: bool) {
        self.changes_collapsed = value;
    }

    pub fn set_unversioned_collapsed(&mut self, value: bool) {
        self.unversioned_collapsed = value;
    }

    pub fn set_ignored_collapsed(&mut self, value: bool) {
        self.ignored_collapsed = value;
    }

    pub fn set_show_ignored(&mut self, value: bool) {
        self.show_ignored = value;
    }

    pub fn set_scroll(&mut self, value: f64) {
        self.scroll = value;
    }
}

impl Normalize for CommitState {
    fn normalize(&mut self) -> Result<(), InvalidViewState> {
        normalize_scroll(&mut self.scroll)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentFilesState {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
pub struct SideViewPath(String);

impl SideViewPath {
    pub fn new(text: String) -> Result<Self, InvalidViewState> {
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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SideViewPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
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
