//! 有序项目集合与活跃项目的纯领域模型。

use std::collections::BTreeSet;

use slterm_path::CanonicalPath;

use crate::{PanelGroupId, ProjectId, ProjectLayout};

/// 项目根在当前工作区的可用状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectRootStatus {
    Ready,
    Inaccessible,
    RelocationRequired,
}

/// 一个项目及其独立布局；根路径不是项目身份。
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    id: ProjectId,
    root: CanonicalPath,
    display_name: String,
    layout: ProjectLayout,
    root_status: ProjectRootStatus,
    next_terminal_number: u64,
}

impl Project {
    pub const fn id(&self) -> ProjectId {
        self.id
    }

    pub fn root(&self) -> &CanonicalPath {
        &self.root
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub const fn layout(&self) -> &ProjectLayout {
        &self.layout
    }

    pub const fn root_status(&self) -> ProjectRootStatus {
        self.root_status
    }

    pub const fn next_terminal_number(&self) -> u64 {
        self.next_terminal_number
    }

    pub fn restore(
        id: ProjectId,
        root: CanonicalPath,
        display_name: String,
        layout: ProjectLayout,
        root_status: ProjectRootStatus,
        next_terminal_number: u64,
    ) -> Result<Self, ProjectRegistryError> {
        // 默认目录名及其消歧后缀可超过重命名输入上限，持久恢复不得拒读。
        if display_name.trim().is_empty() {
            return Err(ProjectRegistryError::EmptyName);
        }
        if layout.project_id() != id {
            return Err(ProjectRegistryError::LayoutProjectMismatch);
        }
        Ok(Self {
            id,
            root,
            display_name,
            layout,
            root_status,
            next_terminal_number,
        })
    }
}

/// 添加项目的结果：重复根只聚焦既有项目。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddProjectOutcome {
    Added(ProjectId),
    Existing(ProjectId),
}

/// 项目注册命令错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectRegistryError {
    DuplicateProject,
    DuplicateRoot(ProjectId),
    DuplicatePanelGroup(PanelGroupId),
    ProjectNotFound,
    EmptyName,
    NameTooLong,
    InvalidProjectIndex,
    LayoutProjectMismatch,
    TerminalNumberExhausted,
}

/// 按导航顺序保存全部项目；存在项目时恰好一个活跃项目。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectRegistry {
    projects: Vec<Project>,
    active: Option<ProjectId>,
}

impl ProjectRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn restore(
        projects: Vec<Project>,
        active: Option<ProjectId>,
    ) -> Result<Self, ProjectRegistryError> {
        let registry = Self { projects, active };
        registry.validate()?;
        Ok(registry)
    }

    /// 校验跨项目身份、组身份与活跃关系；布局可变入口返回后须重新校验。
    pub fn validate(&self) -> Result<(), ProjectRegistryError> {
        let mut panel_groups = BTreeSet::new();
        for (index, project) in self.projects.iter().enumerate() {
            if self.projects[..index]
                .iter()
                .any(|existing| existing.id == project.id)
            {
                return Err(ProjectRegistryError::DuplicateProject);
            }
            if let Some(existing) = self.projects[..index]
                .iter()
                .find(|existing| existing.root == project.root)
            {
                return Err(ProjectRegistryError::DuplicateRoot(existing.id));
            }
            for group_id in project.layout.group_ids() {
                if !panel_groups.insert(group_id) {
                    return Err(ProjectRegistryError::DuplicatePanelGroup(group_id));
                }
            }
        }
        let active_is_valid = match self.active {
            Some(id) => self.projects.iter().any(|project| project.id == id),
            None => self.projects.is_empty(),
        };
        if !active_is_valid {
            return Err(ProjectRegistryError::ProjectNotFound);
        }
        Ok(())
    }

    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn layout_mut(&mut self, id: ProjectId) -> Option<&mut ProjectLayout> {
        self.projects
            .iter_mut()
            .find(|project| project.id == id)
            .map(|project| &mut project.layout)
    }

    pub const fn active(&self) -> Option<ProjectId> {
        self.active
    }

    pub fn activate_project(&mut self, id: ProjectId) -> Result<(), ProjectRegistryError> {
        if !self.projects.iter().any(|project| project.id == id) {
            return Err(ProjectRegistryError::ProjectNotFound);
        }
        self.active = Some(id);
        Ok(())
    }

    pub fn set_root_status(
        &mut self,
        id: ProjectId,
        status: ProjectRootStatus,
    ) -> Result<(), ProjectRegistryError> {
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        project.root_status = status;
        Ok(())
    }

    pub fn allocate_terminal_number(&mut self, id: ProjectId) -> Result<u64, ProjectRegistryError> {
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let number = project.next_terminal_number;
        project.next_terminal_number = number
            .checked_add(1)
            .ok_or(ProjectRegistryError::TerminalNumberExhausted)?;
        Ok(number)
    }

    pub fn remove_project(&mut self, id: ProjectId) -> Result<Project, ProjectRegistryError> {
        let index = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let removed = self.projects.remove(index);
        if self.active == Some(id) {
            self.active = self
                .projects
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|previous| self.projects.get(previous))
                })
                .map(|project| project.id);
        }
        Ok(removed)
    }

    pub fn rename_project(
        &mut self,
        id: ProjectId,
        name: &str,
    ) -> Result<(), ProjectRegistryError> {
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let name = name.trim();
        validate_display_name(name)?;
        project.display_name = name.to_owned();
        Ok(())
    }

    pub fn move_project(
        &mut self,
        id: ProjectId,
        new_index: usize,
    ) -> Result<(), ProjectRegistryError> {
        let old_index = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if new_index >= self.projects.len() {
            return Err(ProjectRegistryError::InvalidProjectIndex);
        }
        if old_index != new_index {
            let project = self.projects.remove(old_index);
            self.projects.insert(new_index, project);
        }
        Ok(())
    }

    pub fn relocate_project(
        &mut self,
        id: ProjectId,
        new_root: CanonicalPath,
    ) -> Result<(), ProjectRegistryError> {
        let index = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if self.projects[index].root == new_root {
            self.projects[index].root_status = ProjectRootStatus::Ready;
            return Ok(());
        }
        if let Some(existing) = self
            .projects
            .iter()
            .find(|project| project.id != id && project.root == new_root)
        {
            return Err(ProjectRegistryError::DuplicateRoot(existing.id));
        }
        self.projects[index].root = new_root;
        self.projects[index].root_status = ProjectRootStatus::Ready;
        Ok(())
    }

    pub fn add_project(
        &mut self,
        id: ProjectId,
        root: CanonicalPath,
    ) -> Result<AddProjectOutcome, ProjectRegistryError> {
        if let Some(existing) = self.projects.iter().find(|project| project.id == id) {
            if existing.root != root {
                return Err(ProjectRegistryError::DuplicateProject);
            }
            self.active = Some(existing.id);
            return Ok(AddProjectOutcome::Existing(existing.id));
        }
        if let Some(existing) = self.projects.iter().find(|project| project.root == root) {
            self.active = Some(existing.id);
            return Ok(AddProjectOutcome::Existing(existing.id));
        }
        let base_name = root
            .as_path()
            .file_name()
            .unwrap_or_else(|| root.as_path().as_os_str())
            .to_string_lossy()
            .into_owned();
        let mut display_name = base_name.clone();
        let mut suffix = 2_u32;
        while self
            .projects
            .iter()
            .any(|project| project.display_name == display_name)
        {
            display_name = format!("{base_name} ({suffix})");
            suffix = suffix.checked_add(1).expect("项目重名序号耗尽");
        }
        self.projects.push(Project {
            id,
            root,
            display_name,
            layout: ProjectLayout::empty(id),
            root_status: ProjectRootStatus::Ready,
            next_terminal_number: 0,
        });
        self.active = Some(id);
        Ok(AddProjectOutcome::Added(id))
    }
}

fn validate_display_name(name: &str) -> Result<(), ProjectRegistryError> {
    if name.trim().is_empty() {
        return Err(ProjectRegistryError::EmptyName);
    }
    if name.chars().count() > 80 {
        return Err(ProjectRegistryError::NameTooLong);
    }
    Ok(())
}
