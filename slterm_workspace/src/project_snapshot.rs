//! 项目聚合与其逻辑终端元数据的一致快照。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    ProjectId, ProjectRegistry, ProjectRegistryError, TerminalId, TerminalRecord, ViewKey,
};

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
