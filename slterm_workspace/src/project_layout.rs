//! 项目布局与面板组的纯领域模型。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;
use slterm_split::{DIVIDER_GAP, RemoveOutcome, SplitDirection, SplitTree, split_lengths};

use crate::{PanelGroupId, ProjectId, ViewKey, ViewKeyError};

const PROJECT_LAYOUT_VERSION: u64 = 1;
const PROJECT_LAYOUT_KIND: &str = "project_layout";

const MIN_GROUP_WIDTH: f32 = 120.0;
const MIN_GROUP_HEIGHT: f32 = 80.0;

/// 布局构造或变更错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// 面板组不能为空。
    EmptyViews,
    /// 同一逻辑视图不能在项目布局中重复出现。
    DuplicateView,
    /// 稳定面板组身份已存在。
    DuplicateGroup,
    /// 布局树叶与组记录未一一对应。
    TreeGroupMismatch,
    /// 目标面板组不存在。
    GroupNotFound,
    /// 目标路径不是既有分割节点。
    SplitNotFound,
    /// 目标逻辑视图不存在于当前布局或指定组。
    ViewNotFound,
    /// 目标组内索引超出本次移动的合法范围。
    InvalidViewIndex,
    /// 合并的源组与目标组相同。
    SameGroup,
    /// 移动拆分会使源组与落点同时失去必要内容。
    SplitWouldEmptySource,
    /// 空态布局已经包含面板组。
    LayoutNotEmpty,
    /// 尺寸或比例不是有效有限几何值。
    InvalidGeometry,
    /// 新几何无法容纳面板组的最小逻辑尺寸。
    InsufficientSpace,
    /// 视图必须属于当前项目。
    ProjectMismatch,
    /// 激活视图必须属于该面板组。
    ActiveNotMember,
}

/// ProjectLayout 持久 DTO 的编码或恢复错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutDtoError {
    /// JSON、字段集合或身份文本不符合当前结构。
    InvalidStructure,
    /// 外层类型名不是项目布局。
    UnknownKind,
    /// 外层 DTO 版本不受支持。
    UnsupportedVersion,
    /// 布局引用或领域不变量非法。
    InvalidLayout(LayoutError),
    /// 内嵌 ViewKey DTO 非法。
    ViewKey(ViewKeyError),
    /// 规范 JSON 编码失败。
    Encode,
}

impl From<LayoutError> for LayoutDtoError {
    fn from(error: LayoutError) -> Self {
        Self::InvalidLayout(error)
    }
}

impl From<ViewKeyError> for LayoutDtoError {
    fn from(error: ViewKeyError) -> Self {
        Self::ViewKey(error)
    }
}

impl fmt::Display for LayoutDtoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "ProjectLayout 持久结构非法",
            Self::UnknownKind => "未知 ProjectLayout 类型",
            Self::UnsupportedVersion => "不支持的 ProjectLayout DTO 版本",
            Self::InvalidLayout(_) => "ProjectLayout 领域校验失败",
            Self::ViewKey(_) => "ProjectLayout 包含非法 ViewKey",
            Self::Encode => "ProjectLayout 规范 JSON 编码失败",
        })
    }
}

impl std::error::Error for LayoutDtoError {}

/// 一个稳定面板组内的有序逻辑视图与激活关系。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelGroup {
    views: Vec<ViewKey>,
    active: ViewKey,
}

impl PanelGroup {
    /// 从非空有序视图与激活视图构造面板组。
    pub fn new(views: Vec<ViewKey>, active: ViewKey) -> Result<Self, LayoutError> {
        if views.is_empty() {
            return Err(LayoutError::EmptyViews);
        }
        if views
            .iter()
            .enumerate()
            .any(|(index, view)| views[..index].contains(view))
        {
            return Err(LayoutError::DuplicateView);
        }
        if !views.contains(&active) {
            return Err(LayoutError::ActiveNotMember);
        }
        Ok(Self { views, active })
    }

    /// 按持久顺序返回组内视图。
    pub fn views(&self) -> &[ViewKey] {
        &self.views
    }

    /// 返回组内激活视图。
    pub fn active(&self) -> &ViewKey {
        &self.active
    }

    fn remove_at(&mut self, index: usize) -> ViewKey {
        let removed = self.views.remove(index);
        if self.active == removed && !self.views.is_empty() {
            self.active = self.views[index.min(self.views.len() - 1)].clone();
        }
        removed
    }
}

/// 布局操作使用的逻辑像素尺寸，与屏幕坐标无关。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

impl LogicalSize {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// 单个项目的稳定面板组树；不持有运行面板实例或焦点。
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectLayout {
    project_id: ProjectId,
    tree: Option<SplitTree<PanelGroupId>>,
    groups: BTreeMap<PanelGroupId, PanelGroup>,
}

impl ProjectLayout {
    /// 从首个非空面板组构造项目布局。
    pub fn new(
        project_id: ProjectId,
        group_id: PanelGroupId,
        group: PanelGroup,
    ) -> Result<Self, LayoutError> {
        let mut layout = Self::empty(project_id);
        layout.open_first_group(group_id, group)?;
        Ok(layout)
    }

    /// 构造不包含面板组的项目空态。
    pub fn empty(project_id: ProjectId) -> Self {
        Self {
            project_id,
            tree: None,
            groups: BTreeMap::new(),
        }
    }

    /// 从独立版本的规范 JSON 恢复项目布局。
    pub fn from_canonical_json(text: &str) -> Result<Self, LayoutDtoError> {
        let envelope: ProjectLayoutEnvelope =
            serde_json::from_str(text).map_err(|_| LayoutDtoError::InvalidStructure)?;
        if envelope.version != PROJECT_LAYOUT_VERSION {
            return Err(LayoutDtoError::UnsupportedVersion);
        }
        if envelope.kind != PROJECT_LAYOUT_KIND {
            return Err(LayoutDtoError::UnknownKind);
        }
        let dto: ProjectLayoutDto =
            serde_json::from_str(text).map_err(|_| LayoutDtoError::InvalidStructure)?;
        dto.into_domain()
    }

    /// 返回布局所属项目。
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// 按树的深度优先顺序返回稳定组身份。
    pub fn group_ids(&self) -> Vec<PanelGroupId> {
        self.tree.as_ref().map_or_else(Vec::new, SplitTree::leaves)
    }

    /// 按稳定身份查询面板组。
    pub fn group(&self, group_id: PanelGroupId) -> Option<&PanelGroup> {
        self.groups.get(&group_id)
    }

    /// 序列化为独立版本、确定键序且可逐字节重放的 JSON。
    pub fn to_canonical_json(&self) -> Result<String, LayoutDtoError> {
        let mut groups = Vec::with_capacity(self.groups.len());
        for group_id in self.group_ids() {
            let group = &self.groups[&group_id];
            let views = group
                .views()
                .iter()
                .map(ViewKey::to_value)
                .collect::<Result<Vec<_>, _>>()?;
            groups.push(object_value(BTreeMap::from([
                ("active", group.active().to_value()?),
                ("group_id", Value::String(group_id.to_string())),
                ("views", Value::Array(views)),
            ])));
        }
        let tree = self.tree.as_ref().map_or(Ok(Value::Null), tree_to_value)?;
        let object = BTreeMap::from([
            ("groups", Value::Array(groups)),
            ("kind", Value::String(PROJECT_LAYOUT_KIND.to_owned())),
            ("project_id", Value::String(self.project_id.to_string())),
            ("tree", tree),
            ("version", Value::from(PROJECT_LAYOUT_VERSION)),
        ]);
        serde_json::to_string(&object_value(object)).map_err(|_| LayoutDtoError::Encode)
    }

    /// 返回只读泛型树，供应用层计算展示几何。
    pub fn tree(&self) -> Option<&SplitTree<PanelGroupId>> {
        self.tree.as_ref()
    }

    /// 按树与组内顺序返回全部逻辑视图。
    pub fn view_keys(&self) -> Vec<ViewKey> {
        self.group_ids()
            .into_iter()
            .flat_map(|group_id| self.groups[&group_id].views().iter().cloned())
            .collect()
    }

    /// 在无组空态中建立首个带内容的面板组。
    pub fn open_first_group(
        &mut self,
        group_id: PanelGroupId,
        group: PanelGroup,
    ) -> Result<(), LayoutError> {
        if self.tree.is_some() || !self.groups.is_empty() {
            return Err(LayoutError::LayoutNotEmpty);
        }
        if group
            .views()
            .iter()
            .any(|view| view.project_id() != Some(self.project_id))
        {
            return Err(LayoutError::ProjectMismatch);
        }
        self.tree = Some(SplitTree::leaf(group_id));
        self.groups.insert(group_id, group);
        Ok(())
    }

    /// 在既有组末尾打开视图并激活它。
    pub fn insert_view(
        &mut self,
        group_id: PanelGroupId,
        view: ViewKey,
    ) -> Result<(), LayoutError> {
        if !self.groups.contains_key(&group_id) {
            return Err(LayoutError::GroupNotFound);
        }
        if view.project_id() != Some(self.project_id) {
            return Err(LayoutError::ProjectMismatch);
        }
        if self
            .groups
            .values()
            .any(|group| group.views().contains(&view))
        {
            return Err(LayoutError::DuplicateView);
        }
        let group = self
            .groups
            .get_mut(&group_id)
            .ok_or(LayoutError::GroupNotFound)?;
        group.views.push(view.clone());
        group.active = view;
        Ok(())
    }

    /// 激活组内已有视图。
    pub fn activate_view(
        &mut self,
        group_id: PanelGroupId,
        view: &ViewKey,
    ) -> Result<(), LayoutError> {
        let group = self
            .groups
            .get_mut(&group_id)
            .ok_or(LayoutError::GroupNotFound)?;
        if !group.views.contains(view) {
            return Err(LayoutError::ViewNotFound);
        }
        group.active = view.clone();
        Ok(())
    }

    /// 关闭逻辑视图；组变空时同时移除组。
    pub fn close_view(&mut self, view: &ViewKey) -> Result<(), LayoutError> {
        let (group_id, index) = self.locate_view(view).ok_or(LayoutError::ViewNotFound)?;
        let group = self
            .groups
            .get_mut(&group_id)
            .ok_or(LayoutError::ViewNotFound)?;
        group.remove_at(index);
        if group.views.is_empty() {
            self.remove_group(group_id);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), LayoutError> {
        let group_ids = self.group_ids();
        let unique_ids: BTreeSet<_> = group_ids.iter().copied().collect();
        if unique_ids.len() != group_ids.len() {
            return Err(LayoutError::DuplicateGroup);
        }
        if group_ids.len() != self.groups.len()
            || group_ids
                .iter()
                .any(|group_id| !self.groups.contains_key(group_id))
        {
            return Err(LayoutError::TreeGroupMismatch);
        }
        let mut views: Vec<&ViewKey> = Vec::new();
        for group in self.groups.values() {
            for view in group.views() {
                if view.project_id() != Some(self.project_id) {
                    return Err(LayoutError::ProjectMismatch);
                }
                if views.contains(&view) {
                    return Err(LayoutError::DuplicateView);
                }
                views.push(view);
            }
        }
        Ok(())
    }

    fn locate_view(&self, view: &ViewKey) -> Option<(PanelGroupId, usize)> {
        self.groups.iter().find_map(|(group_id, group)| {
            group
                .views
                .iter()
                .position(|candidate| candidate == view)
                .map(|index| (*group_id, index))
        })
    }

    fn remove_group(&mut self, group_id: PanelGroupId) -> PanelGroup {
        let group = self
            .groups
            .remove(&group_id)
            .expect("面板组树与组内容必须一致");
        let outcome = self.tree.as_mut().map(|tree| tree.remove_leaf(group_id));
        match outcome {
            Some(RemoveOutcome::WasRoot) => self.tree = None,
            Some(RemoveOutcome::Collapsed(_)) => {}
            Some(RemoveOutcome::NotFound) | None => {
                debug_assert!(false, "面板组树与组内容必须一致")
            }
        }
        group
    }

    /// 调整既有分割节点的提交比例，不改变任何视图身份。
    pub fn resize_split(
        &mut self,
        path: &[bool],
        ratio: f32,
        viewport: LogicalSize,
    ) -> Result<(), LayoutError> {
        if !valid_ratio(ratio) || !valid_size(viewport) {
            return Err(LayoutError::InvalidGeometry);
        }
        let mut candidate = self.tree.clone().ok_or(LayoutError::SplitNotFound)?;
        let node = candidate.node_mut(path).ok_or(LayoutError::SplitNotFound)?;
        match node {
            SplitTree::Split { ratio: current, .. } => *current = ratio,
            SplitTree::Leaf(_) => return Err(LayoutError::SplitNotFound),
        }
        if !tree_fits(&candidate, viewport) {
            return Err(LayoutError::InsufficientSpace);
        }
        self.tree = Some(candidate);
        Ok(())
    }

    /// 移动视图到同项目目标组的最终索引并激活它。
    pub fn move_view(
        &mut self,
        view: &ViewKey,
        target_group: PanelGroupId,
        target_index: usize,
    ) -> Result<(), LayoutError> {
        let target_len = self
            .groups
            .get(&target_group)
            .ok_or(LayoutError::GroupNotFound)?
            .views
            .len();
        let (source_group, source_index) =
            self.locate_view(view).ok_or(LayoutError::ViewNotFound)?;
        if source_group == target_group {
            if target_index >= target_len {
                return Err(LayoutError::InvalidViewIndex);
            }
            let group = self
                .groups
                .get_mut(&target_group)
                .ok_or(LayoutError::GroupNotFound)?;
            let moved = group.remove_at(source_index);
            group.views.insert(target_index, moved.clone());
            group.active = moved;
            return Ok(());
        }
        if target_index > target_len {
            return Err(LayoutError::InvalidViewIndex);
        }
        let (moved, source_empty) = {
            let group = self
                .groups
                .get_mut(&source_group)
                .ok_or(LayoutError::ViewNotFound)?;
            let moved = group.remove_at(source_index);
            (moved, group.views.is_empty())
        };
        if source_empty {
            self.remove_group(source_group);
        }
        let target = self
            .groups
            .get_mut(&target_group)
            .ok_or(LayoutError::GroupNotFound)?;
        target.views.insert(target_index, moved.clone());
        target.active = moved;
        Ok(())
    }

    /// 将源组视图按原顺序追加到目标组并移除源组。
    pub fn merge_groups(
        &mut self,
        source_group: PanelGroupId,
        target_group: PanelGroupId,
    ) -> Result<(), LayoutError> {
        if source_group == target_group {
            return Err(LayoutError::SameGroup);
        }
        if !self.groups.contains_key(&source_group) || !self.groups.contains_key(&target_group) {
            return Err(LayoutError::GroupNotFound);
        }
        let mut source = self.remove_group(source_group);
        let target = self
            .groups
            .get_mut(&target_group)
            .ok_or(LayoutError::GroupNotFound)?;
        target.views.append(&mut source.views);
        Ok(())
    }

    /// 将既有视图移动到目标边缘形成的新组，按根 viewport 校验源组塌缩后的最终空间。
    pub fn split_view(
        &mut self,
        view: &ViewKey,
        target_group: PanelGroupId,
        new_group_id: PanelGroupId,
        direction: SplitDirection,
        ratio: f32,
        viewport: LogicalSize,
    ) -> Result<(), LayoutError> {
        if !valid_ratio(ratio) || !valid_size(viewport) {
            return Err(LayoutError::InvalidGeometry);
        }
        if !self.groups.contains_key(&target_group) {
            return Err(LayoutError::GroupNotFound);
        }
        if self.groups.contains_key(&new_group_id) {
            return Err(LayoutError::DuplicateGroup);
        }
        let (source_group, source_index) =
            self.locate_view(view).ok_or(LayoutError::ViewNotFound)?;
        if source_group == target_group && self.groups[&source_group].views.len() == 1 {
            return Err(LayoutError::SplitWouldEmptySource);
        }

        let mut candidate = self.clone();
        let split = candidate
            .tree
            .as_mut()
            .ok_or(LayoutError::GroupNotFound)?
            .split_leaf(target_group, new_group_id, direction, ratio);
        if !split {
            return Err(LayoutError::GroupNotFound);
        }
        let (moved, source_empty) = {
            let group = candidate
                .groups
                .get_mut(&source_group)
                .ok_or(LayoutError::ViewNotFound)?;
            let moved = group.remove_at(source_index);
            (moved, group.views.is_empty())
        };
        if source_empty {
            candidate.remove_group(source_group);
        }
        candidate.groups.insert(
            new_group_id,
            PanelGroup {
                views: vec![moved.clone()],
                active: moved,
            },
        );
        let tree = candidate.tree.as_ref().ok_or(LayoutError::GroupNotFound)?;
        if !split_groups_fit(tree, target_group, new_group_id, viewport) {
            return Err(LayoutError::InsufficientSpace);
        }
        *self = candidate;
        Ok(())
    }

    /// 在目标组边缘拆分出带内容的新组，按根 viewport 校验最终空间。
    pub fn split(
        &mut self,
        target_group: PanelGroupId,
        new_group_id: PanelGroupId,
        new_group: PanelGroup,
        direction: SplitDirection,
        ratio: f32,
        viewport: LogicalSize,
    ) -> Result<(), LayoutError> {
        if !valid_ratio(ratio) || !valid_size(viewport) {
            return Err(LayoutError::InvalidGeometry);
        }
        if !self.groups.contains_key(&target_group) {
            return Err(LayoutError::GroupNotFound);
        }
        if self.groups.contains_key(&new_group_id) {
            return Err(LayoutError::DuplicateGroup);
        }
        if new_group
            .views()
            .iter()
            .any(|view| view.project_id() != Some(self.project_id))
        {
            return Err(LayoutError::ProjectMismatch);
        }
        if new_group.views().iter().any(|new_view| {
            self.groups
                .values()
                .any(|group| group.views().contains(new_view))
        }) {
            return Err(LayoutError::DuplicateView);
        }

        let mut candidate = self.tree.clone().ok_or(LayoutError::GroupNotFound)?;
        if !candidate.split_leaf(target_group, new_group_id, direction, ratio) {
            return Err(LayoutError::GroupNotFound);
        }
        if !split_groups_fit(&candidate, target_group, new_group_id, viewport) {
            return Err(LayoutError::InsufficientSpace);
        }
        self.tree = Some(candidate);
        self.groups.insert(new_group_id, new_group);
        Ok(())
    }
}

#[derive(Deserialize)]
struct ProjectLayoutEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectLayoutDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    project_id: String,
    groups: Vec<PanelGroupDto>,
    #[serde(default, deserialize_with = "deserialize_present")]
    tree: Option<Option<SplitTreeDto>>,
}

impl ProjectLayoutDto {
    fn into_domain(self) -> Result<ProjectLayout, LayoutDtoError> {
        let project_id = self
            .project_id
            .parse()
            .map_err(|_| LayoutDtoError::InvalidStructure)?;
        let tree = self.tree.ok_or(LayoutDtoError::InvalidStructure)?;
        let tree = tree
            .map(|tree| {
                let mut group_ids = BTreeSet::new();
                tree.into_domain(&mut group_ids)
            })
            .transpose()?;
        let mut groups = BTreeMap::new();
        for group in self.groups {
            let (group_id, group) = group.into_domain()?;
            if groups.insert(group_id, group).is_some() {
                return Err(LayoutError::DuplicateGroup.into());
            }
        }
        let layout = ProjectLayout {
            project_id,
            tree,
            groups,
        };
        layout.validate()?;
        Ok(layout)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelGroupDto {
    group_id: String,
    active: Box<RawValue>,
    views: Vec<Box<RawValue>>,
}

impl PanelGroupDto {
    fn into_domain(self) -> Result<(PanelGroupId, PanelGroup), LayoutDtoError> {
        let group_id = self
            .group_id
            .parse()
            .map_err(|_| LayoutDtoError::InvalidStructure)?;
        let active = ViewKey::from_canonical_json(self.active.get())?;
        let views = self
            .views
            .into_iter()
            .map(|view| ViewKey::from_canonical_json(view.get()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((group_id, PanelGroup::new(views, active)?))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SplitTreeDto {
    Group {
        group_id: String,
    },
    Split {
        direction: SplitDirectionDto,
        ratio: f32,
        first: Box<SplitTreeDto>,
        second: Box<SplitTreeDto>,
    },
}

impl SplitTreeDto {
    fn into_domain(
        self,
        group_ids: &mut BTreeSet<PanelGroupId>,
    ) -> Result<SplitTree<PanelGroupId>, LayoutDtoError> {
        match self {
            Self::Group { group_id } => {
                let group_id = group_id
                    .parse()
                    .map_err(|_| LayoutDtoError::InvalidStructure)?;
                if !group_ids.insert(group_id) {
                    return Err(LayoutError::DuplicateGroup.into());
                }
                Ok(SplitTree::leaf(group_id))
            }
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if !valid_ratio(ratio) {
                    return Err(LayoutError::InvalidGeometry.into());
                }
                let first = first.into_domain(group_ids)?;
                let second = second.into_domain(group_ids)?;
                Ok(SplitTree::Split {
                    direction: direction.into(),
                    ratio,
                    preview_ratio: None,
                    dragging: false,
                    first: Box::new(first),
                    second: Box::new(second),
                })
            }
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SplitDirectionDto {
    LeftRight,
    TopBottom,
}

impl From<SplitDirectionDto> for SplitDirection {
    fn from(direction: SplitDirectionDto) -> Self {
        match direction {
            SplitDirectionDto::LeftRight => Self::LeftRight,
            SplitDirectionDto::TopBottom => Self::TopBottom,
        }
    }
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn object_value(object: BTreeMap<&'static str, Value>) -> Value {
    Value::Object(
        object
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn tree_to_value(tree: &SplitTree<PanelGroupId>) -> Result<Value, LayoutDtoError> {
    match tree {
        SplitTree::Leaf(group_id) => Ok(object_value(BTreeMap::from([
            ("group_id", Value::String(group_id.to_string())),
            ("kind", Value::String("group".to_owned())),
        ]))),
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
            ..
        } => {
            if !valid_ratio(*ratio) {
                return Err(LayoutError::InvalidGeometry.into());
            }
            let direction = match direction {
                SplitDirection::LeftRight => "left_right",
                SplitDirection::TopBottom => "top_bottom",
            };
            Ok(object_value(BTreeMap::from([
                ("direction", Value::String(direction.to_owned())),
                ("first", tree_to_value(first)?),
                ("kind", Value::String("split".to_owned())),
                ("ratio", Value::from(*ratio)),
                ("second", tree_to_value(second)?),
            ])))
        }
    }
}

fn valid_ratio(ratio: f32) -> bool {
    ratio.is_finite() && ratio > 0.0 && ratio < 1.0
}

fn valid_size(size: LogicalSize) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

fn child_sizes(
    direction: SplitDirection,
    ratio: f32,
    size: LogicalSize,
) -> (LogicalSize, LogicalSize) {
    let total = match direction {
        SplitDirection::LeftRight => size.width,
        SplitDirection::TopBottom => size.height,
    };
    let (first, second) = split_lengths(total, ratio, DIVIDER_GAP, 0.0);
    match direction {
        SplitDirection::LeftRight => (
            LogicalSize::new(first, size.height),
            LogicalSize::new(second, size.height),
        ),
        SplitDirection::TopBottom => (
            LogicalSize::new(size.width, first),
            LogicalSize::new(size.width, second),
        ),
    }
}

fn group_size_is_sufficient(size: LogicalSize) -> bool {
    size.width >= MIN_GROUP_WIDTH && size.height >= MIN_GROUP_HEIGHT
}

fn group_size(
    tree: &SplitTree<PanelGroupId>,
    target: PanelGroupId,
    size: LogicalSize,
) -> Option<LogicalSize> {
    match tree {
        SplitTree::Leaf(group_id) => (*group_id == target).then_some(size),
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
            ..
        } => {
            if !valid_ratio(*ratio) {
                return None;
            }
            let (first_size, second_size) = child_sizes(*direction, *ratio, size);
            group_size(first, target, first_size)
                .or_else(|| group_size(second, target, second_size))
        }
    }
}

fn split_groups_fit(
    tree: &SplitTree<PanelGroupId>,
    target_group: PanelGroupId,
    new_group: PanelGroupId,
    viewport: LogicalSize,
) -> bool {
    [target_group, new_group]
        .into_iter()
        .all(|group_id| group_size(tree, group_id, viewport).is_some_and(group_size_is_sufficient))
}

fn tree_fits<T>(tree: &SplitTree<T>, size: LogicalSize) -> bool {
    match tree {
        SplitTree::Leaf(_) => group_size_is_sufficient(size),
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
            ..
        } => {
            if !valid_ratio(*ratio) {
                return false;
            }
            let (first_size, second_size) = child_sizes(*direction, *ratio, size);
            tree_fits(first, first_size) && tree_fits(second, second_size)
        }
    }
}
