//! 项目聚合与其逻辑终端元数据的一致快照。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use slterm_document::DocumentId;
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_split::SplitDirection;

use crate::{
    LayoutError, LogicalSize, PanelGroup, PanelGroupId, Project, ProjectId, ProjectRegistry,
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
    /// 缺省使用项目根；显式目录必须位于项目根内。
    pub cwd: Option<CanonicalPath>,
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
    TerminalCwdOutsideProject,
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
    MissingDocument(DocumentId),
    DuplicateDocument(DocumentId),
    OrphanDocument(DocumentId),
    DocumentLocationConflict(DocumentId),
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
    documents: BTreeMap<DocumentId, ResolvedPath>,
}

impl ProjectSnapshot {
    pub fn restore(
        registry: ProjectRegistry,
        terminals: Vec<TerminalRecord>,
    ) -> Result<Self, ProjectSnapshotError> {
        Self::restore_with_documents(registry, terminals, Vec::new())
    }

    pub fn restore_with_documents(
        registry: ProjectRegistry,
        terminals: Vec<TerminalRecord>,
        documents: Vec<(DocumentId, ResolvedPath)>,
    ) -> Result<Self, ProjectSnapshotError> {
        let mut terminal_map = BTreeMap::new();
        for record in terminals {
            let id = record.id();
            if terminal_map.insert(id, record).is_some() {
                return Err(ProjectSnapshotError::DuplicateTerminal(id));
            }
        }
        let mut document_map = BTreeMap::new();
        for (id, location) in documents {
            if document_map.insert(id, location).is_some() {
                return Err(ProjectSnapshotError::DuplicateDocument(id));
            }
        }
        let snapshot = Self {
            registry,
            terminals: terminal_map,
            documents: document_map,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// 启动恢复只归一跨记录引用；身份、编号与其他结构损坏仍然报错。
    pub fn recover(
        registry: ProjectRegistry,
        terminals: Vec<TerminalRecord>,
    ) -> Result<Self, ProjectSnapshotError> {
        Self::recover_with_documents(registry, terminals, Vec::new())
    }

    pub fn recover_with_documents(
        registry: ProjectRegistry,
        terminals: Vec<TerminalRecord>,
        documents: Vec<(DocumentId, ResolvedPath)>,
    ) -> Result<Self, ProjectSnapshotError> {
        registry.validate()?;
        let mut terminal_map = BTreeMap::new();
        for record in terminals {
            let id = record.id();
            if terminal_map.insert(id, record).is_some() {
                return Err(ProjectSnapshotError::DuplicateTerminal(id));
            }
        }
        let mut document_map = BTreeMap::new();
        for (id, location) in documents {
            if document_map.insert(id, location).is_some() {
                return Err(ProjectSnapshotError::DuplicateDocument(id));
            }
        }
        let mut recovered = Self {
            registry,
            terminals: terminal_map,
            documents: document_map,
        };
        let mut detached_views = Vec::new();
        for project in recovered.registry.projects() {
            for view in project.layout().view_keys() {
                let detached = match &view {
                    ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    } => !recovered
                        .terminals
                        .get(terminal_id)
                        .is_some_and(|record| record.project_id() == *project_id),
                    ViewKey::Editor { document_id, .. }
                    | ViewKey::Markdown { document_id, .. }
                    | ViewKey::Diff { document_id, .. } => {
                        !recovered.documents.contains_key(document_id)
                    }
                    _ => false,
                };
                if detached {
                    detached_views.push(view);
                }
            }
        }
        for view in detached_views {
            let project_id = view.project_id().expect("终端视图必须属于项目");
            recovered
                .registry
                .layout_mut(project_id)
                .expect("项目布局必须属于既有项目")
                .close_view(&view)
                .expect("待归一视图必须存在于原布局");
        }
        let referenced = recovered
            .registry
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .filter_map(|view| match view {
                ViewKey::Terminal { terminal_id, .. } => Some(terminal_id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        recovered
            .terminals
            .retain(|terminal_id, _| referenced.contains(terminal_id));
        recovered.retain_referenced_documents();
        recovered.validate()?;
        Ok(recovered)
    }

    /// 在同一候选中登记 Document 元数据并打开 Editor 视图。
    pub fn with_opened_document(
        &self,
        project_id: ProjectId,
        target_group: PanelGroupId,
        document_id: DocumentId,
        location: ResolvedPath,
    ) -> Result<(Self, ViewKey), ProjectCommandError> {
        self.validate()?;
        if self
            .documents
            .get(&document_id)
            .is_some_and(|existing| existing != &location)
        {
            return Err(ProjectSnapshotError::DocumentLocationConflict(document_id).into());
        }
        let view = ViewKey::Editor {
            project_id,
            document_id,
        };
        let mut candidate = self.clone();
        candidate.documents.insert(document_id, location);
        let layout = candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if let Some(group_id) = layout.group_ids().into_iter().find(|group_id| {
            layout
                .group(*group_id)
                .is_some_and(|group| group.views().contains(&view))
        }) {
            layout.activate_view(group_id, &view)?;
        } else if layout.tree().is_none() {
            let group = PanelGroup::new(vec![view.clone()], view.clone())?;
            layout.open_first_group(target_group, group)?;
        } else {
            layout.insert_view(target_group, view.clone())?;
        }
        candidate.validate()?;
        Ok((candidate, view))
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

    /// 生成移除项目的候选快照；项目拥有的逻辑终端一并移除。
    pub fn without_project(&self, id: ProjectId) -> Result<(Self, Project), ProjectCommandError> {
        self.validate()?;
        let mut candidate = self.clone();
        let removed = candidate.registry_mut().remove_project(id)?;
        candidate
            .terminals
            .retain(|_, record| record.project_id() != id);
        candidate.retain_referenced_documents();
        candidate.validate()?;
        Ok((candidate, removed))
    }

    /// 生成关闭逻辑终端的候选快照；最后关闭后保留项目空态。
    pub fn without_terminal(
        &self,
        id: TerminalId,
    ) -> Result<(Self, TerminalRecord), ProjectCommandError> {
        let (candidate, mut removed) = self.without_terminals(&[id])?;
        Ok((
            candidate,
            removed.pop().expect("单终端关闭必须返回一个记录"),
        ))
    }

    /// 在同一候选中关闭全部逻辑终端；任何失败都不修改原快照。
    pub fn without_terminals(
        &self,
        ids: &[TerminalId],
    ) -> Result<(Self, Vec<TerminalRecord>), ProjectCommandError> {
        self.validate()?;
        let mut candidate = self.clone();
        let mut removed = Vec::with_capacity(ids.len());
        for id in ids {
            let record = candidate
                .terminal(*id)
                .cloned()
                .ok_or(ProjectSnapshotError::MissingTerminal(*id))?;
            let view = ViewKey::Terminal {
                project_id: record.project_id(),
                terminal_id: *id,
            };
            candidate
                .registry_mut()
                .layout_mut(record.project_id())
                .ok_or(ProjectRegistryError::ProjectNotFound)?
                .close_view(&view)?;
            removed.push(candidate.remove_terminal(*id)?);
        }
        candidate.validate()?;
        Ok((candidate, removed))
    }

    /// 生成激活逻辑终端的候选快照，只改变布局激活关系。
    pub fn with_active_terminal(&self, id: TerminalId) -> Result<Self, ProjectCommandError> {
        let record = self
            .terminal(id)
            .ok_or(ProjectSnapshotError::MissingTerminal(id))?;
        self.with_active_view(&ViewKey::Terminal {
            project_id: record.project_id(),
            terminal_id: id,
        })
    }

    /// 生成激活既有逻辑视图的候选快照，只改变其所属组的激活关系。
    pub fn with_active_view(&self, view: &ViewKey) -> Result<Self, ProjectCommandError> {
        self.validate()?;
        let project_id = view.project_id().ok_or(LayoutError::ProjectMismatch)?;
        let mut candidate = self.clone();
        let layout = candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let group_id = layout
            .group_ids()
            .into_iter()
            .find(|group_id| {
                layout
                    .group(*group_id)
                    .is_some_and(|group| group.views().contains(view))
            })
            .ok_or(LayoutError::ViewNotFound)?;
        layout.activate_view(group_id, view)?;
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
        let cwd = spec.cwd.clone().unwrap_or_else(|| root.clone());
        if !cwd.as_path().starts_with(root.as_path()) {
            return Err(ProjectCommandError::TerminalCwdOutsideProject);
        }
        let number = self
            .registry_mut()
            .allocate_terminal_number(spec.project_id)?;
        let record = TerminalRecord::new(
            spec.terminal_id,
            spec.project_id,
            number,
            cwd,
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
        let mut referenced_documents = BTreeSet::new();
        for project in self.registry.projects() {
            for view in project.layout().view_keys() {
                match view {
                    ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    } => {
                        let record = self
                            .terminals
                            .get(&terminal_id)
                            .ok_or(ProjectSnapshotError::MissingTerminal(terminal_id))?;
                        if record.project_id() != project_id {
                            return Err(ProjectSnapshotError::TerminalProjectMismatch(terminal_id));
                        }
                        referenced.insert(terminal_id);
                    }
                    ViewKey::Editor { document_id, .. }
                    | ViewKey::Markdown { document_id, .. }
                    | ViewKey::Diff { document_id, .. } => {
                        if !self.documents.contains_key(&document_id) {
                            return Err(ProjectSnapshotError::MissingDocument(document_id));
                        }
                        referenced_documents.insert(document_id);
                    }
                    _ => {}
                }
            }
        }
        for id in self.documents.keys() {
            if !referenced_documents.contains(id) {
                return Err(ProjectSnapshotError::OrphanDocument(*id));
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

    fn retain_referenced_documents(&mut self) {
        let referenced = self
            .registry
            .projects()
            .iter()
            .flat_map(|project| project.layout().view_keys())
            .filter_map(|view| match view {
                ViewKey::Editor { document_id, .. }
                | ViewKey::Markdown { document_id, .. }
                | ViewKey::Diff { document_id, .. } => Some(document_id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        self.documents.retain(|id, _| referenced.contains(id));
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

    pub fn document(&self, id: DocumentId) -> Option<&ResolvedPath> {
        self.documents.get(&id)
    }

    pub fn documents(&self) -> impl Iterator<Item = (DocumentId, &ResolvedPath)> {
        self.documents.iter().map(|(id, location)| (*id, location))
    }

    pub fn terminals(&self) -> impl Iterator<Item = &TerminalRecord> {
        self.terminals.values()
    }
}
