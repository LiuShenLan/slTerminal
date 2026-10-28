//! 项目聚合与其逻辑终端元数据的一致快照。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use slterm_path::CanonicalPath;
use slterm_split::SplitDirection;

use crate::{
    LayoutError, LogicalSize, PanelGroup, PanelGroupId, ProjectId, ProjectRegistry,
    ProjectRegistryError, RunIntent, TerminalId, TerminalRecord, TerminalRecordError, ViewKey,
};

/// 在既有组末尾或空项目首组打开逻辑终端的输入。
#[derive(Clone, Debug)]
pub struct NewTerminalSpec {
    pub project_id: ProjectId,
    pub terminal_id: TerminalId,
    /// 非空布局中的目标组；空布局中作为首组身份。
    pub target_group: PanelGroupId,
    pub shell: CanonicalPath,
    pub custom_title: Option<String>,
}

/// 从目标组拆分出带新逻辑终端的新组。
#[derive(Clone, Debug)]
pub struct SplitTerminalSpec {
    pub new_terminal: NewTerminalSpec,
    pub new_group_id: PanelGroupId,
    pub direction: SplitDirection,
    pub ratio: f32,
    pub viewport: LogicalSize,
}

/// 快照命令的布局、记录或一致性错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectCommandError {
    Registry(ProjectRegistryError),
    Layout(LayoutError),
    Record(TerminalRecordError),
    Snapshot(ProjectSnapshotError),
}

impl fmt::Display for ProjectCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("项目快照命令失败")
    }
}

impl std::error::Error for ProjectCommandError {}

impl From<ProjectRegistryError> for ProjectCommandError {
    fn from(error: ProjectRegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<LayoutError> for ProjectCommandError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<TerminalRecordError> for ProjectCommandError {
    fn from(error: TerminalRecordError) -> Self {
        Self::Record(error)
    }
}

impl From<ProjectSnapshotError> for ProjectCommandError {
    fn from(error: ProjectSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

/// 项目快照的跨记录约束错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectSnapshotError {
    Project(ProjectRegistryError),
    DuplicateTerminal(TerminalId),
    TerminalProjectNotFound(TerminalId),
    TerminalProjectMismatch(TerminalId),
    MissingTerminal(TerminalId),
    OrphanTerminal(TerminalId),
    DuplicateTerminalNumber { project_id: ProjectId, number: u64 },
    TerminalNumberBeyondCounter(TerminalId),
}

impl fmt::Display for ProjectSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("项目快照与逻辑终端记录不一致")
    }
}

impl std::error::Error for ProjectSnapshotError {}

impl From<ProjectRegistryError> for ProjectSnapshotError {
    fn from(error: ProjectRegistryError) -> Self {
        Self::Project(error)
    }
}

/// 同一事务保存与恢复的项目聚合和逻辑终端集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSnapshot {
    registry: ProjectRegistry,
    terminals: BTreeMap<TerminalId, TerminalRecord>,
}

impl ProjectSnapshot {
    pub fn restore(
        registry: ProjectRegistry,
        terminals: Vec<TerminalRecord>,
    ) -> Result<Self, ProjectSnapshotError> {
        let mut terminal_map = BTreeMap::new();
        for record in terminals {
            let id = record.id();
            if terminal_map.insert(id, record).is_some() {
                return Err(ProjectSnapshotError::DuplicateTerminal(id));
            }
        }
        let snapshot = Self {
            registry,
            terminals: terminal_map,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// 生成新增逻辑终端的候选快照；任何失败都不修改原快照。
    pub fn with_new_terminal(
        &self,
        spec: NewTerminalSpec,
    ) -> Result<(Self, TerminalRecord), ProjectCommandError> {
        self.validate()?;
        let mut candidate = self.clone();
        let (record, view) = candidate.prepare_record(&spec)?;
        let layout = candidate
            .registry_mut()
            .layout_mut(spec.project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if layout.tree().is_none() {
            let group = PanelGroup::new(vec![view.clone()], view)?;
            layout.open_first_group(spec.target_group, group)?;
        } else {
            layout.insert_view(spec.target_group, view)?;
        }
        candidate.add_terminal(record.clone())?;
        candidate.validate()?;
        Ok((candidate, record))
    }

    /// 生成拆分逻辑终端的候选快照；新组从创建起即包含新视图。
    pub fn with_split_terminal(
        &self,
        spec: SplitTerminalSpec,
    ) -> Result<(Self, TerminalRecord), ProjectCommandError> {
        self.validate()?;
        let mut candidate = self.clone();
        let (record, view) = candidate.prepare_record(&spec.new_terminal)?;
        let group = PanelGroup::new(vec![view.clone()], view)?;
        candidate
            .registry_mut()
            .layout_mut(spec.new_terminal.project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .split(
                spec.new_terminal.target_group,
                spec.new_group_id,
                group,
                spec.direction,
                spec.ratio,
                spec.viewport,
            )?;
        candidate.add_terminal(record.clone())?;
        candidate.validate()?;
        Ok((candidate, record))
    }

    /// 生成关闭逻辑终端的候选快照；最后关闭后保留项目空态。
    pub fn without_terminal(
        &self,
        id: TerminalId,
    ) -> Result<(Self, TerminalRecord), ProjectCommandError> {
        self.validate()?;
        let record = self
            .terminal(id)
            .cloned()
            .ok_or(ProjectSnapshotError::MissingTerminal(id))?;
        let view = ViewKey::Terminal {
            project_id: record.project_id(),
            terminal_id: id,
        };
        let mut candidate = self.clone();
        candidate
            .registry_mut()
            .layout_mut(record.project_id())
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .close_view(&view)?;
        let removed = candidate.remove_terminal(id)?;
        candidate.validate()?;
        Ok((candidate, removed))
    }

    /// 生成激活逻辑终端的候选快照，只改变布局激活关系。
    pub fn with_active_terminal(&self, id: TerminalId) -> Result<Self, ProjectCommandError> {
        self.validate()?;
        let record = self
            .terminal(id)
            .ok_or(ProjectSnapshotError::MissingTerminal(id))?;
        let view = ViewKey::Terminal {
            project_id: record.project_id(),
            terminal_id: id,
        };
        let mut candidate = self.clone();
        let layout = candidate
            .registry_mut()
            .layout_mut(record.project_id())
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let group_id = layout
            .group_ids()
            .into_iter()
            .find(|group_id| {
                layout
                    .group(*group_id)
                    .is_some_and(|group| group.views().contains(&view))
            })
            .ok_or(LayoutError::ViewNotFound)?;
        layout.activate_view(group_id, &view)?;
        candidate.validate()?;
        Ok(candidate)
    }

    /// 生成更新逻辑终端运行意图的候选快照，不改变布局。
    pub fn with_terminal_run_intent(
        &self,
        id: TerminalId,
        run_intent: RunIntent,
    ) -> Result<Self, ProjectCommandError> {
        self.validate()?;
        if self.terminal(id).is_none() {
            return Err(ProjectSnapshotError::MissingTerminal(id).into());
        }
        let mut candidate = self.clone();
        candidate.set_run_intent(id, run_intent)?;
        candidate.validate()?;
        Ok(candidate)
    }

    fn prepare_record(
        &mut self,
        spec: &NewTerminalSpec,
    ) -> Result<(TerminalRecord, ViewKey), ProjectCommandError> {
        if self.terminal(spec.terminal_id).is_some() {
            return Err(ProjectSnapshotError::DuplicateTerminal(spec.terminal_id).into());
        }
        let root = self
            .registry()
            .project(spec.project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .root()
            .clone();
        let number = self
            .registry_mut()
            .allocate_terminal_number(spec.project_id)?;
        let record = TerminalRecord::new(
            spec.terminal_id,
            spec.project_id,
            number,
            root,
            spec.shell.clone(),
            spec.custom_title.clone(),
            RunIntent::Launch,
        )?;
        let view = ViewKey::Terminal {
            project_id: spec.project_id,
            terminal_id: spec.terminal_id,
        };
        Ok((record, view))
    }

    /// 校验项目聚合、布局引用、终端归属与编号约束。
    pub fn validate(&self) -> Result<(), ProjectSnapshotError> {
        self.registry.validate()?;

        let mut referenced = BTreeSet::new();
        for project in self.registry.projects() {
            for view in project.layout().view_keys() {
                if let ViewKey::Terminal {
                    project_id,
                    terminal_id,
                } = view
                {
                    let record = self
                        .terminals
                        .get(&terminal_id)
                        .ok_or(ProjectSnapshotError::MissingTerminal(terminal_id))?;
                    if record.project_id() != project_id {
                        return Err(ProjectSnapshotError::TerminalProjectMismatch(terminal_id));
                    }
                    referenced.insert(terminal_id);
                }
            }
        }

        let mut numbers = BTreeSet::new();
        for record in self.terminals.values() {
            let id = record.id();
            let project = self
                .registry
                .project(record.project_id())
                .ok_or(ProjectSnapshotError::TerminalProjectNotFound(id))?;
            if !referenced.contains(&id) {
                return Err(ProjectSnapshotError::OrphanTerminal(id));
            }
            if !numbers.insert((record.project_id(), record.number())) {
                return Err(ProjectSnapshotError::DuplicateTerminalNumber {
                    project_id: record.project_id(),
                    number: record.number(),
                });
            }
            if record.number() >= project.next_terminal_number() {
                return Err(ProjectSnapshotError::TerminalNumberBeyondCounter(id));
            }
        }
        Ok(())
    }

    /// 原子加入逻辑终端；任何一致性校验失败都保持原快照。
    pub fn add_terminal(&mut self, record: TerminalRecord) -> Result<(), ProjectSnapshotError> {
        let id = record.id();
        if self.terminals.contains_key(&id) {
            return Err(ProjectSnapshotError::DuplicateTerminal(id));
        }
        self.terminals.insert(id, record);
        if let Err(error) = self.validate() {
            self.terminals.remove(&id);
            return Err(error);
        }
        Ok(())
    }

    /// 原子移除逻辑终端；布局仍引用或记录缺失时保持原快照。
    pub fn remove_terminal(
        &mut self,
        id: TerminalId,
    ) -> Result<TerminalRecord, ProjectSnapshotError> {
        let record = self
            .terminals
            .remove(&id)
            .ok_or(ProjectSnapshotError::MissingTerminal(id))?;
        if let Err(error) = self.validate() {
            self.terminals.insert(id, record);
            return Err(error);
        }
        Ok(record)
    }

    /// 更新既有逻辑终端的启动意图。
    pub fn set_run_intent(
        &mut self,
        id: TerminalId,
        run_intent: crate::RunIntent,
    ) -> Result<(), ProjectSnapshotError> {
        let record = self
            .terminals
            .get_mut(&id)
            .ok_or(ProjectSnapshotError::MissingTerminal(id))?;
        record.set_run_intent(run_intent);
        Ok(())
    }

    pub const fn registry(&self) -> &ProjectRegistry {
        &self.registry
    }

    /// 可变修改后必须重新执行快照校验。
    pub fn registry_mut(&mut self) -> &mut ProjectRegistry {
        &mut self.registry
    }

    pub fn terminal(&self, id: TerminalId) -> Option<&TerminalRecord> {
        self.terminals.get(&id)
    }

    pub fn terminals(&self) -> impl Iterator<Item = &TerminalRecord> {
        self.terminals.values()
    }
}
