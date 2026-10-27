//! 项目聚合、活跃项与规范布局的唯一 SQLite 存取通道。

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use rusqlite::{OptionalExtension, TransactionBehavior, params};
use slterm_path::CanonicalPath;
use slterm_workspace::{
    LayoutDtoError, PROJECT_LAYOUT_DTO_VERSION, PanelGroupId, Project, ProjectId, ProjectLayout,
    ProjectRegistry, ProjectRegistryError, ProjectRootStatus, ProjectSnapshot,
    ProjectSnapshotError, RunIntent, TerminalId, TerminalRecord,
};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

/// 项目聚合读写失败；不向展示层暴露 SQL、路径或布局正文。
#[derive(Debug)]
pub(crate) enum ProjectStoreError {
    Project(ProjectRegistryError),
    Snapshot(ProjectSnapshotError),
    Layout(LayoutDtoError),
    CorruptRecord,
    RevisionExhausted,
    TerminalNumberRegression,
    TerminalNumberReuse,
    Database(DatabaseError),
}

impl fmt::Display for ProjectStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Project(_) => "项目聚合持久记录不一致",
            Self::Snapshot(_) => "项目快照持久记录不一致",
            Self::Layout(error) => return fmt::Display::fmt(error, formatter),
            Self::CorruptRecord => "项目聚合持久记录不一致",
            Self::RevisionExhausted => "项目布局 revision 已耗尽",
            Self::TerminalNumberRegression => "项目终端编号计数器不得回退",
            Self::TerminalNumberReuse => "逻辑终端编号不得复用",
            Self::Database(_) => "项目聚合数据库操作失败",
        })
    }
}

impl std::error::Error for ProjectStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(_)
            | Self::CorruptRecord
            | Self::RevisionExhausted
            | Self::TerminalNumberRegression
            | Self::TerminalNumberReuse => None,
            Self::Snapshot(error) => Some(error),
            Self::Layout(error) => Some(error),
            Self::Database(error) => Some(error),
        }
    }
}

impl From<ProjectRegistryError> for ProjectStoreError {
    fn from(error: ProjectRegistryError) -> Self {
        Self::Project(error)
    }
}

impl From<ProjectSnapshotError> for ProjectStoreError {
    fn from(error: ProjectSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<LayoutDtoError> for ProjectStoreError {
    fn from(error: LayoutDtoError) -> Self {
        Self::Layout(error)
    }
}

impl From<DatabaseError> for ProjectStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for ProjectStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    /// 在一个事务内保存项目顺序、活跃项、规范布局与逻辑终端元数据。
    pub(crate) fn save_project_snapshot(
        &mut self,
        snapshot: &ProjectSnapshot,
    ) -> Result<(), ProjectStoreError> {
        snapshot.validate()?;
        let registry = snapshot.registry();
        // 先在内存完成全部无损转换，避免可预见的非法值进入写事务。
        let encoded = registry
            .projects()
            .iter()
            .enumerate()
            .map(|(index, project)| EncodedProject::new(index, project))
            .collect::<Result<Vec<_>, _>>()?;
        let active = registry.active().map(|id| id.to_string());
        let terminals = snapshot
            .terminals()
            .map(EncodedTerminal::new)
            .collect::<Result<Vec<_>, _>>()?;
        let dto_version = i64::try_from(PROJECT_LAYOUT_DTO_VERSION)
            .map_err(|_| ProjectStoreError::CorruptRecord)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        // 已存快照必须先完整自洽，禁止借新快照静默修复损坏。
        let stored_snapshot = load_project_snapshot_from(&transaction)?;
        for terminal in snapshot.terminals() {
            if let Some(stored) = stored_snapshot.terminal(terminal.id()) {
                if stored.project_id() != terminal.project_id()
                    || stored.number() != terminal.number()
                {
                    return Err(ProjectStoreError::CorruptRecord);
                }
            } else {
                let high_watermark = stored_snapshot
                    .registry()
                    .project(terminal.project_id())
                    .map_or(0, |project| project.next_terminal_number());
                if terminal.number() < high_watermark {
                    return Err(ProjectStoreError::TerminalNumberReuse);
                }
            }
        }

        let mut statement = transaction.prepare("SELECT project_id FROM projects")?;
        let stored_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?;
        drop(statement);
        let desired_ids = encoded
            .iter()
            .map(|project| project.id.as_str())
            .collect::<BTreeSet<_>>();
        for stale_id in stored_ids
            .iter()
            .filter(|id| !desired_ids.contains(id.as_str()))
        {
            transaction.execute("DELETE FROM projects WHERE project_id = ?1", [stale_id])?;
        }

        // 合法根均为绝对路径；事务内先换成不可能与之冲突的临时值，支持项目间交换根。
        for project in encoded
            .iter()
            .filter(|project| stored_ids.contains(project.id.as_str()))
        {
            transaction.execute(
                "UPDATE projects SET root = ?2 WHERE project_id = ?1",
                params![
                    &project.id,
                    format!("slterm-pending-project-root:{}", project.id)
                ],
            )?;
        }

        for project in &encoded {
            let stored_next_number = transaction
                .query_row(
                    "SELECT next_terminal_number FROM projects WHERE project_id = ?1",
                    [&project.id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if stored_next_number.is_some_and(|stored| project.next_terminal_number < stored) {
                return Err(ProjectStoreError::TerminalNumberRegression);
            }
            let stored_layout = transaction
                .query_row(
                    "SELECT dto_version, layout_json, revision FROM project_layouts \
                     WHERE project_id = ?1",
                    [&project.id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?;
            let revision = match stored_layout {
                Some((version, json, revision)) => {
                    let version =
                        u64::try_from(version).map_err(|_| ProjectStoreError::CorruptRecord)?;
                    if version != PROJECT_LAYOUT_DTO_VERSION {
                        return Err(LayoutDtoError::UnsupportedVersion.into());
                    }
                    // 保留项目的旧记录必须完整且自洽，保存不得借目标快照修复损坏。
                    let stored = ProjectLayout::from_canonical_json(&json)?;
                    let project_id = ProjectId::from_str(&project.id)
                        .map_err(|_| ProjectStoreError::CorruptRecord)?;
                    if stored.project_id() != project_id {
                        return Err(ProjectRegistryError::LayoutProjectMismatch.into());
                    }
                    let stored_groups = stored_group_ids(&transaction, project_id)?;
                    if stored_groups != stored.group_ids().into_iter().collect() {
                        return Err(ProjectStoreError::CorruptRecord);
                    }
                    if json == project.layout_json {
                        revision
                    } else {
                        revision
                            .checked_add(1)
                            .ok_or(ProjectStoreError::RevisionExhausted)?
                    }
                }
                None if stored_ids.contains(project.id.as_str()) => {
                    return Err(ProjectStoreError::CorruptRecord);
                }
                None => 0,
            };
            transaction.execute(
                "INSERT INTO projects \
                 (project_id, root, display_name, sort_order, root_status, next_terminal_number) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(project_id) DO UPDATE SET \
                 root = excluded.root, display_name = excluded.display_name, \
                 sort_order = excluded.sort_order, root_status = excluded.root_status, \
                 next_terminal_number = excluded.next_terminal_number",
                params![
                    &project.id,
                    &project.root,
                    &project.display_name,
                    project.sort_order,
                    project.root_status,
                    project.next_terminal_number,
                ],
            )?;
            transaction.execute(
                "INSERT INTO project_layouts (project_id, dto_version, layout_json, revision) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(project_id) DO UPDATE SET \
                 dto_version = excluded.dto_version, layout_json = excluded.layout_json, \
                 revision = excluded.revision",
                params![&project.id, dto_version, &project.layout_json, revision],
            )?;
            transaction.execute(
                "DELETE FROM panel_groups WHERE project_id = ?1",
                [&project.id],
            )?;
            for group_id in &project.group_ids {
                transaction.execute(
                    "INSERT INTO panel_groups (panel_group_id, project_id) VALUES (?1, ?2)",
                    params![group_id, &project.id],
                )?;
            }
        }
        let desired_terminal_ids = terminals
            .iter()
            .map(|terminal| terminal.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut statement = transaction.prepare("SELECT terminal_id FROM terminals")?;
        let stored_terminal_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?;
        drop(statement);
        for stale_id in stored_terminal_ids
            .iter()
            .filter(|id| !desired_terminal_ids.contains(id.as_str()))
        {
            transaction.execute("DELETE FROM terminals WHERE terminal_id = ?1", [stale_id])?;
        }
        for terminal in &terminals {
            transaction.execute(
                "INSERT INTO terminals \
                 (terminal_id, project_id, number, cwd, shell, custom_title, run_intent) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(terminal_id) DO UPDATE SET \
                 cwd = excluded.cwd, shell = excluded.shell, \
                 custom_title = excluded.custom_title, run_intent = excluded.run_intent",
                params![
                    &terminal.id,
                    &terminal.project_id,
                    terminal.number,
                    &terminal.cwd,
                    &terminal.shell,
                    &terminal.custom_title,
                    terminal.run_intent,
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO app_state (singleton, active_project_id) VALUES (1, ?1) \
             ON CONFLICT(singleton) DO UPDATE SET active_project_id = excluded.active_project_id",
            [active],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// 在同一 Immediate 事务恢复项目、布局与逻辑终端，不混合并发提交世代。
    pub(crate) fn load_project_snapshot(&mut self) -> Result<ProjectSnapshot, ProjectStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_project_snapshot_from(&transaction)?;
        transaction.commit()?;
        Ok(snapshot)
    }
}

fn load_project_snapshot_from(
    connection: &rusqlite::Connection,
) -> Result<ProjectSnapshot, ProjectStoreError> {
    let mut statement = connection.prepare(
        "SELECT p.project_id, p.root, p.display_name, p.root_status, \
         p.next_terminal_number, l.dto_version, l.layout_json \
         FROM projects p \
         LEFT JOIN project_layouts l ON l.project_id = p.project_id \
         ORDER BY p.sort_order, p.project_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut projects = Vec::with_capacity(rows.len());
    for (id, root, name, status, next_number, version, layout_json) in rows {
        let id = ProjectId::from_str(&id).map_err(|_| ProjectStoreError::CorruptRecord)?;
        let version = version.ok_or(ProjectStoreError::CorruptRecord)?;
        let layout_json = layout_json.ok_or(ProjectStoreError::CorruptRecord)?;
        let version = u64::try_from(version).map_err(|_| ProjectStoreError::CorruptRecord)?;
        if version != PROJECT_LAYOUT_DTO_VERSION {
            return Err(LayoutDtoError::UnsupportedVersion.into());
        }
        let layout = ProjectLayout::from_canonical_json(&layout_json)?;
        let stored_groups = stored_group_ids(connection, id)?;
        let layout_groups = layout.group_ids().into_iter().collect::<BTreeSet<_>>();
        if stored_groups != layout_groups {
            return Err(ProjectStoreError::CorruptRecord);
        }
        let root = CanonicalPath::new(root).map_err(|_| ProjectStoreError::CorruptRecord)?;
        let next_number =
            u64::try_from(next_number).map_err(|_| ProjectStoreError::CorruptRecord)?;
        projects.push(Project::restore(
            id,
            root,
            name,
            layout,
            decode_root_status(&status)?,
            next_number,
        )?);
    }
    let active = connection
        .query_row(
            "SELECT active_project_id FROM app_state WHERE singleton = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .map(|id| ProjectId::from_str(&id).map_err(|_| ProjectStoreError::CorruptRecord))
        .transpose()?
        .or_else(|| projects.first().map(Project::id));
    let registry = ProjectRegistry::restore(projects, active)?;
    let terminals = load_terminal_records(connection)?;
    Ok(ProjectSnapshot::restore(registry, terminals)?)
}

struct EncodedTerminal {
    id: String,
    project_id: String,
    number: i64,
    cwd: String,
    shell: String,
    custom_title: Option<String>,
    run_intent: &'static str,
}

impl EncodedTerminal {
    fn new(record: &TerminalRecord) -> Result<Self, ProjectStoreError> {
        Ok(Self {
            id: record.id().to_string(),
            project_id: record.project_id().to_string(),
            number: i64::try_from(record.number()).map_err(|_| ProjectStoreError::CorruptRecord)?,
            cwd: record
                .cwd()
                .as_path()
                .to_str()
                .ok_or(ProjectStoreError::CorruptRecord)?
                .to_owned(),
            shell: record
                .shell()
                .as_path()
                .to_str()
                .ok_or(ProjectStoreError::CorruptRecord)?
                .to_owned(),
            custom_title: record.custom_title().map(str::to_owned),
            run_intent: match record.run_intent() {
                RunIntent::Launch => "launch",
                RunIntent::Exited => "exited",
            },
        })
    }
}

struct EncodedProject {
    id: String,
    root: String,
    display_name: String,
    sort_order: i64,
    root_status: &'static str,
    next_terminal_number: i64,
    layout_json: String,
    group_ids: Vec<String>,
}

impl EncodedProject {
    fn new(index: usize, project: &Project) -> Result<Self, ProjectStoreError> {
        Ok(Self {
            id: project.id().to_string(),
            root: project
                .root()
                .as_path()
                .to_str()
                .ok_or(ProjectStoreError::CorruptRecord)?
                .to_owned(),
            display_name: project.display_name().to_owned(),
            sort_order: i64::try_from(index).map_err(|_| ProjectStoreError::CorruptRecord)?,
            root_status: encode_root_status(project.root_status()),
            next_terminal_number: i64::try_from(project.next_terminal_number())
                .map_err(|_| ProjectStoreError::CorruptRecord)?,
            layout_json: project.layout().to_canonical_json()?,
            group_ids: project
                .layout()
                .group_ids()
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        })
    }
}

fn load_terminal_records(
    connection: &rusqlite::Connection,
) -> Result<Vec<TerminalRecord>, ProjectStoreError> {
    let mut statement = connection.prepare(
        "SELECT terminal_id, project_id, number, cwd, shell, custom_title, run_intent \
         FROM terminals ORDER BY terminal_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(id, project_id, number, cwd, shell, custom_title, run_intent)| {
                let id = TerminalId::from_str(&id).map_err(|_| ProjectStoreError::CorruptRecord)?;
                let project_id = ProjectId::from_str(&project_id)
                    .map_err(|_| ProjectStoreError::CorruptRecord)?;
                let number = u64::try_from(number).map_err(|_| ProjectStoreError::CorruptRecord)?;
                let cwd = CanonicalPath::new(cwd).map_err(|_| ProjectStoreError::CorruptRecord)?;
                let shell =
                    CanonicalPath::new(shell).map_err(|_| ProjectStoreError::CorruptRecord)?;
                let run_intent = match run_intent.as_str() {
                    "launch" => RunIntent::Launch,
                    "exited" => RunIntent::Exited,
                    _ => return Err(ProjectStoreError::CorruptRecord),
                };
                TerminalRecord::new(id, project_id, number, cwd, shell, custom_title, run_intent)
                    .map_err(|_| ProjectStoreError::CorruptRecord)
            },
        )
        .collect()
}

fn stored_group_ids(
    connection: &rusqlite::Connection,
    project_id: ProjectId,
) -> Result<BTreeSet<PanelGroupId>, ProjectStoreError> {
    let mut statement = connection.prepare(
        "SELECT panel_group_id FROM panel_groups WHERE project_id = ?1 ORDER BY panel_group_id",
    )?;
    let ids = statement
        .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
        .map(|id| {
            let id = id?;
            PanelGroupId::from_str(&id).map_err(|_| ProjectStoreError::CorruptRecord)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(ids)
}

const fn encode_root_status(status: ProjectRootStatus) -> &'static str {
    match status {
        ProjectRootStatus::Ready => "ready",
        ProjectRootStatus::Inaccessible => "inaccessible",
        ProjectRootStatus::RelocationRequired => "relocation_required",
    }
}

fn decode_root_status(status: &str) -> Result<ProjectRootStatus, ProjectStoreError> {
    match status {
        "ready" => Ok(ProjectRootStatus::Ready),
        "inaccessible" => Ok(ProjectRootStatus::Inaccessible),
        "relocation_required" => Ok(ProjectRootStatus::RelocationRequired),
        _ => Err(ProjectStoreError::CorruptRecord),
    }
}

#[cfg(test)]
mod project_tests {
    use std::str::FromStr;

    use rusqlite::params;
    use slterm_path::{AppPaths, CanonicalPath};
    use slterm_workspace::{
        PROJECT_LAYOUT_DTO_VERSION, PanelGroup, PanelGroupId, Project, ProjectId, ProjectLayout,
        ProjectRegistry, ProjectRootStatus, ProjectSnapshot, RunIntent, TerminalId, TerminalRecord,
        ViewKey,
    };
    use tempfile::TempDir;

    use super::*;

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
    const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
    const TERMINAL_A_SECOND: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";
    const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2c";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn populated_store(temp: &TempDir) -> WorkspaceDatabase {
        let mut store = WorkspaceDatabase::open(&fixture(temp)).unwrap();
        save_registry(&mut store, &sample_registry()).unwrap();
        store
    }

    fn id(text: &str) -> ProjectId {
        ProjectId::from_str(text).unwrap()
    }

    fn root(text: &str) -> CanonicalPath {
        CanonicalPath::new(text).unwrap()
    }

    fn terminal(project_id: ProjectId, text: &str) -> ViewKey {
        ViewKey::Terminal {
            project_id,
            terminal_id: TerminalId::from_str(text).unwrap(),
        }
    }

    fn nonempty_layout() -> ProjectLayout {
        let project_id = id(PROJECT_A);
        let view = terminal(project_id, TERMINAL_A);
        ProjectLayout::new(
            project_id,
            PanelGroupId::from_str(GROUP_A).unwrap(),
            PanelGroup::new(vec![view.clone()], view).unwrap(),
        )
        .unwrap()
    }

    fn sample_registry() -> ProjectRegistry {
        let project_a = Project::restore(
            id(PROJECT_A),
            root("D:/work/a"),
            "same".to_owned(),
            nonempty_layout(),
            ProjectRootStatus::Inaccessible,
            41,
        )
        .unwrap();
        let project_b = Project::restore(
            id(PROJECT_B),
            root("D:/work/b"),
            "same".to_owned(),
            ProjectLayout::empty(id(PROJECT_B)),
            ProjectRootStatus::RelocationRequired,
            7,
        )
        .unwrap();
        ProjectRegistry::restore(vec![project_a, project_b], Some(id(PROJECT_B))).unwrap()
    }

    fn registry_with_two_terminals() -> ProjectRegistry {
        let mut registry = sample_registry();
        let view = terminal(id(PROJECT_B), TERMINAL_B);
        registry
            .layout_mut(id(PROJECT_B))
            .unwrap()
            .open_first_group(
                PanelGroupId::from_str(GROUP_B).unwrap(),
                PanelGroup::new(vec![view.clone()], view).unwrap(),
            )
            .unwrap();
        registry
    }

    fn snapshot(registry: ProjectRegistry) -> ProjectSnapshot {
        let mut records = Vec::new();
        for project in registry.projects() {
            let mut number = 0;
            for view in project.layout().view_keys() {
                if let ViewKey::Terminal {
                    project_id,
                    terminal_id,
                } = view
                {
                    records.push(
                        TerminalRecord::new(
                            terminal_id,
                            project_id,
                            number,
                            root("D:/work/terminal"),
                            root("C:/shell/pwsh.exe"),
                            None,
                            RunIntent::Launch,
                        )
                        .unwrap(),
                    );
                    number += 1;
                }
            }
        }
        ProjectSnapshot::restore(registry, records).unwrap()
    }

    fn record(project: &str, terminal: &str, number: u64) -> TerminalRecord {
        TerminalRecord::new(
            TerminalId::from_str(terminal).unwrap(),
            id(project),
            number,
            root("D:/work/terminal"),
            root("C:/shell/pwsh.exe"),
            None,
            RunIntent::Launch,
        )
        .unwrap()
    }

    fn insert_allocated_terminal(
        registry: &mut ProjectRegistry,
        project: &str,
        group: &str,
        terminal_text: &str,
    ) -> TerminalRecord {
        let project_id = id(project);
        let number = registry.allocate_terminal_number(project_id).unwrap();
        registry
            .layout_mut(project_id)
            .unwrap()
            .insert_view(
                PanelGroupId::from_str(group).unwrap(),
                terminal(project_id, terminal_text),
            )
            .unwrap();
        record(project, terminal_text, number)
    }

    fn versioned_snapshot(version: &str) -> ProjectSnapshot {
        let mut projects = Vec::new();
        let mut records = Vec::new();
        for index in 0..24_u8 {
            let project_id = id(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d00{index:02x}"));
            let terminal_id =
                TerminalId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b{index:02x}"))
                    .unwrap();
            let view = ViewKey::Terminal {
                project_id,
                terminal_id,
            };
            let layout = ProjectLayout::new(
                project_id,
                PanelGroupId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a{index:02x}"))
                    .unwrap(),
                PanelGroup::new(vec![view.clone()], view).unwrap(),
            )
            .unwrap();
            projects.push(
                Project::restore(
                    project_id,
                    root(&format!("D:/work/{index}")),
                    version.to_owned(),
                    layout,
                    ProjectRootStatus::Ready,
                    1,
                )
                .unwrap(),
            );
            records.push(
                TerminalRecord::new(
                    terminal_id,
                    project_id,
                    0,
                    root("D:/work/terminal"),
                    root("C:/shell/pwsh.exe"),
                    Some(version.to_owned()),
                    RunIntent::Launch,
                )
                .unwrap(),
            );
        }
        let active = projects.first().map(Project::id);
        ProjectSnapshot::restore(ProjectRegistry::restore(projects, active).unwrap(), records)
            .unwrap()
    }

    fn save_registry(
        store: &mut WorkspaceDatabase,
        registry: &ProjectRegistry,
    ) -> Result<(), ProjectStoreError> {
        store.save_project_snapshot(&snapshot(registry.clone()))
    }

    fn load_registry(store: &mut WorkspaceDatabase) -> Result<ProjectRegistry, ProjectStoreError> {
        Ok(store.load_project_snapshot()?.registry().clone())
    }

    fn revision(store: &WorkspaceDatabase, project_id: &str) -> i64 {
        store
            .database
            .connection
            .query_row(
                "SELECT revision FROM project_layouts WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn terminal_rowid(store: &WorkspaceDatabase, terminal_id: &str) -> i64 {
        store
            .database
            .connection
            .query_row(
                "SELECT rowid FROM terminals WHERE terminal_id = ?1",
                [terminal_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn project_snapshot_store_round_trips_terminal_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let record = TerminalRecord::new(
            TerminalId::from_str(TERMINAL_A).unwrap(),
            id(PROJECT_A),
            40,
            root("D:/work/terminal"),
            root("C:/Program Files/PowerShell/7/pwsh.exe"),
            Some("构建终端".to_owned()),
            RunIntent::Exited,
        )
        .unwrap();
        let expected = ProjectSnapshot::restore(sample_registry(), vec![record]).unwrap();

        assert!(store.save_project_snapshot(&expected).is_ok());
        drop(store);

        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(reopened.load_project_snapshot().unwrap(), expected);
    }

    #[test]
    fn project_snapshot_store_load_returns_only_committed_generations() {
        use rusqlite::trace::{TraceEvent, TraceEventCodes};

        enum WriteEvent {
            Blocked,
            Finished,
        }

        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let first = versioned_snapshot("first");
        let second = versioned_snapshot("second");
        let mut writer = WorkspaceDatabase::open(&paths).unwrap();
        writer.save_project_snapshot(&first).unwrap();

        let reader_paths = paths.clone();
        let (paused_sender, paused_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut store = WorkspaceDatabase::open(&reader_paths).unwrap();
            store
                .database
                .connection
                .trace_v2(
                    TraceEventCodes::SQLITE_TRACE_STMT,
                    Some(move |event: TraceEvent<'_>| {
                        if let TraceEvent::Stmt(statement, _) = event
                            && statement.sql().contains("FROM terminals")
                        {
                            paused_sender.send(()).unwrap();
                            release_receiver.recv().unwrap();
                        }
                    }),
                )
                .unwrap();
            store.load_project_snapshot().unwrap()
        });
        paused_receiver.recv().unwrap();

        let (write_event_sender, write_event_receiver) = std::sync::mpsc::channel();
        let blocked_sender = write_event_sender.clone();
        writer
            .database
            .connection
            .busy_handler(Some(move |_| {
                blocked_sender.send(WriteEvent::Blocked).unwrap();
                false
            }))
            .unwrap();
        let writer_snapshot = second.clone();
        let writer_thread = std::thread::spawn(move || {
            let result = writer.save_project_snapshot(&writer_snapshot);
            write_event_sender.send(WriteEvent::Finished).unwrap();
            (writer, result)
        });

        // 读事务持锁时写方先报告 blocked；无读事务时写方先提交，形成确定的跨查询交错。
        let first_write_event = write_event_receiver.recv().unwrap();
        release_sender.send(()).unwrap();
        let loaded = reader.join().unwrap();
        let (mut writer, first_write) = writer_thread.join().unwrap();
        match first_write_event {
            WriteEvent::Blocked => {
                assert!(first_write.is_err());
                assert_eq!(loaded, first);
                writer
                    .database
                    .connection
                    .busy_handler(None::<fn(i32) -> bool>)
                    .unwrap();
                writer.save_project_snapshot(&second).unwrap();
                let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
                assert_eq!(reopened.load_project_snapshot().unwrap(), second);
            }
            WriteEvent::Finished => {
                first_write.unwrap();
                assert!(
                    loaded == first || loaded == second,
                    "恢复不得混合两个已提交世代"
                );
            }
        }
    }

    #[test]
    fn project_snapshot_store_rejects_terminal_identity_reassignment() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        let renumbered = ProjectSnapshot::restore(
            sample_registry(),
            vec![
                TerminalRecord::new(
                    TerminalId::from_str(TERMINAL_A).unwrap(),
                    id(PROJECT_A),
                    1,
                    root("D:/work/terminal"),
                    root("C:/shell/pwsh.exe"),
                    None,
                    RunIntent::Launch,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        assert!(matches!(
            store.save_project_snapshot(&renumbered),
            Err(ProjectStoreError::CorruptRecord)
        ));

        let mut moved_registry = sample_registry();
        moved_registry
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .close_view(&terminal(id(PROJECT_A), TERMINAL_A))
            .unwrap();
        let moved_view = terminal(id(PROJECT_B), TERMINAL_A);
        moved_registry
            .layout_mut(id(PROJECT_B))
            .unwrap()
            .open_first_group(
                PanelGroupId::from_str(GROUP_B).unwrap(),
                PanelGroup::new(vec![moved_view.clone()], moved_view).unwrap(),
            )
            .unwrap();
        let moved = ProjectSnapshot::restore(
            moved_registry,
            vec![
                TerminalRecord::new(
                    TerminalId::from_str(TERMINAL_A).unwrap(),
                    id(PROJECT_B),
                    0,
                    root("D:/work/terminal"),
                    root("C:/shell/pwsh.exe"),
                    None,
                    RunIntent::Launch,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        assert!(matches!(
            store.save_project_snapshot(&moved),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
    }

    #[test]
    fn project_snapshot_store_rejects_terminal_number_counter_regression() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        let project_a = Project::restore(
            id(PROJECT_A),
            root("D:/work/a"),
            "changed".to_owned(),
            nonempty_layout(),
            ProjectRootStatus::Ready,
            40,
        )
        .unwrap();
        let changed_registry = ProjectRegistry::restore(
            vec![
                project_a,
                original.registry().project(id(PROJECT_B)).unwrap().clone(),
            ],
            Some(id(PROJECT_B)),
        )
        .unwrap();
        let changed =
            ProjectSnapshot::restore(changed_registry, original.terminals().cloned().collect())
                .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::TerminalNumberRegression)
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
    }

    #[test]
    fn project_snapshot_store_rejects_new_terminal_number_reuse() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        let mut closed_registry = sample_registry();
        closed_registry
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .close_view(&terminal(id(PROJECT_A), TERMINAL_A))
            .unwrap();
        let mut recycled_registry = closed_registry.clone();
        let recycled_view = terminal(id(PROJECT_A), TERMINAL_A_SECOND);
        recycled_registry
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .open_first_group(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                PanelGroup::new(vec![recycled_view.clone()], recycled_view).unwrap(),
            )
            .unwrap();
        let recycled = ProjectSnapshot::restore(
            recycled_registry,
            vec![
                TerminalRecord::new(
                    TerminalId::from_str(TERMINAL_A_SECOND).unwrap(),
                    id(PROJECT_A),
                    0,
                    root("D:/work/terminal"),
                    root("C:/shell/pwsh.exe"),
                    None,
                    RunIntent::Launch,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let closed = ProjectSnapshot::restore(closed_registry, Vec::new()).unwrap();

        assert!(matches!(
            store.save_project_snapshot(&recycled),
            Err(ProjectStoreError::TerminalNumberReuse)
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
        store.save_project_snapshot(&closed).unwrap();
        assert!(matches!(
            store.save_project_snapshot(&recycled),
            Err(ProjectStoreError::TerminalNumberReuse)
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), closed);
    }

    #[test]
    fn project_snapshot_store_save_rejects_corrupt_retained_terminal() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        store
            .database
            .connection
            .execute(
                "UPDATE terminals SET run_intent = 'unknown' WHERE terminal_id = ?1",
                [TERMINAL_A],
            )
            .unwrap();
        let mut changed = original.clone();
        changed
            .registry_mut()
            .rename_project(id(PROJECT_A), "changed")
            .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(
            store.database.connection.query_row(
                "SELECT display_name FROM projects WHERE project_id = ?1",
                [PROJECT_A],
                |row| row.get::<_, String>(0),
            ),
            Ok("same".to_owned())
        );
    }

    #[test]
    fn project_snapshot_store_save_rejects_orphan_retained_terminal() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        store
            .database
            .connection
            .execute(
                "INSERT INTO terminals \
                 (terminal_id, project_id, number, cwd, shell, run_intent) \
                 VALUES (?1, ?2, 1, 'D:/work/terminal', 'C:/shell/pwsh.exe', 'launch')",
                [TERMINAL_A_SECOND, PROJECT_A],
            )
            .unwrap();
        let mut changed = original.clone();
        changed
            .registry_mut()
            .rename_project(id(PROJECT_A), "changed")
            .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::Snapshot(
                ProjectSnapshotError::OrphanTerminal(terminal_id)
            )) if terminal_id == TerminalId::from_str(TERMINAL_A_SECOND).unwrap()
        ));
        assert_eq!(
            store.database.connection.query_row(
                "SELECT display_name FROM projects WHERE project_id = ?1",
                [PROJECT_A],
                |row| row.get::<_, String>(0),
            ),
            Ok("same".to_owned())
        );
        assert_eq!(
            store.database.connection.query_row(
                "SELECT COUNT(*) FROM terminals WHERE terminal_id = ?1",
                [TERMINAL_A_SECOND],
                |row| row.get::<_, i64>(0),
            ),
            Ok(1)
        );
    }

    #[test]
    fn project_snapshot_store_terminal_failure_rolls_back_project_changes() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = snapshot(sample_registry());
        store.save_project_snapshot(&original).unwrap();
        let fault = rusqlite::Connection::open(store.database.connection.path().unwrap()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_terminal_write BEFORE UPDATE ON terminals \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();
        let changed_record = TerminalRecord::new(
            TerminalId::from_str(TERMINAL_A).unwrap(),
            id(PROJECT_A),
            0,
            root("D:/work/terminal"),
            root("C:/shell/pwsh.exe"),
            Some("changed".to_owned()),
            RunIntent::Exited,
        )
        .unwrap();
        let mut changed_registry = sample_registry();
        changed_registry
            .rename_project(id(PROJECT_B), "changed-b")
            .unwrap();
        let changed = ProjectSnapshot::restore(changed_registry, vec![changed_record]).unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::Database(_))
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
    }

    #[test]
    fn project_registry_store_round_trips_ordered_projects_and_layout() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let expected = sample_registry();

        let saved = save_registry(&mut store, &expected);
        assert!(saved.is_ok(), "合法项目聚合必须可保存");
        drop(store);

        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        let restored = load_registry(&mut reopened);
        assert!(restored.is_ok(), "已保存项目聚合必须可恢复");
        assert_eq!(restored.unwrap(), expected);
    }

    #[test]
    fn project_registry_store_updates_revision_without_recreating_retained_terminals() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let mut registry = registry_with_two_terminals();
        save_registry(&mut store, &registry).unwrap();
        let terminal_a_rowid = terminal_rowid(&store, TERMINAL_A);
        let terminal_b_rowid = terminal_rowid(&store, TERMINAL_B);

        save_registry(&mut store, &registry).unwrap();
        assert_eq!(revision(&store, PROJECT_A), 0);
        assert_eq!(terminal_rowid(&store, TERMINAL_A), terminal_a_rowid);
        assert_eq!(terminal_rowid(&store, TERMINAL_B), terminal_b_rowid);
        registry.rename_project(id(PROJECT_A), "renamed").unwrap();
        let second =
            insert_allocated_terminal(&mut registry, PROJECT_A, GROUP_A, TERMINAL_A_SECOND);
        registry.remove_project(id(PROJECT_B)).unwrap();
        let changed = ProjectSnapshot::restore(
            registry.clone(),
            vec![record(PROJECT_A, TERMINAL_A, 0), second],
        )
        .unwrap();
        store.save_project_snapshot(&changed).unwrap();

        assert_eq!(revision(&store, PROJECT_A), 1);
        assert_eq!(terminal_rowid(&store, TERMINAL_A), terminal_a_rowid);
        assert_eq!(
            store.database.connection.query_row(
                "SELECT COUNT(*) FROM terminals WHERE terminal_id = ?1",
                [TERMINAL_B],
                |row| row.get::<_, i64>(0),
            ),
            Ok(0)
        );
        assert_eq!(load_registry(&mut store).unwrap(), registry);
        assert_eq!(PROJECT_LAYOUT_DTO_VERSION, 1);
    }

    #[test]
    fn project_registry_store_rejects_missing_layout_instead_of_dropping_project() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        save_registry(&mut store, &sample_registry()).unwrap();
        store
            .database
            .connection
            .execute(
                "DELETE FROM project_layouts WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();

        assert!(matches!(
            load_registry(&mut store),
            Err(ProjectStoreError::CorruptRecord)
        ));
    }

    #[test]
    fn project_registry_store_distinguishes_corruption_and_unsupported_layout() {
        let status_temp = tempfile::tempdir().unwrap();
        let mut status_store = populated_store(&status_temp);
        status_store
            .database
            .connection
            .execute(
                "UPDATE projects SET root_status = 'unknown' WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            load_registry(&mut status_store),
            Err(ProjectStoreError::CorruptRecord)
        ));

        let group_temp = tempfile::tempdir().unwrap();
        let mut group_store = populated_store(&group_temp);
        group_store
            .database
            .connection
            .execute(
                "DELETE FROM panel_groups WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            load_registry(&mut group_store),
            Err(ProjectStoreError::CorruptRecord)
        ));

        let version_temp = tempfile::tempdir().unwrap();
        let mut version_store = populated_store(&version_temp);
        version_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET dto_version = 2 WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            load_registry(&mut version_store),
            Err(ProjectStoreError::Layout(
                LayoutDtoError::UnsupportedVersion
            ))
        ));

        let json_temp = tempfile::tempdir().unwrap();
        let mut json_store = populated_store(&json_temp);
        json_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET layout_json = '{}' WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            load_registry(&mut json_store),
            Err(ProjectStoreError::Layout(LayoutDtoError::InvalidStructure))
        ));
    }

    #[test]
    fn project_registry_store_save_failures_preserve_committed_snapshot() {
        let invalid_temp = tempfile::tempdir().unwrap();
        let mut invalid_store = populated_store(&invalid_temp);
        let original = sample_registry();
        let invalid_a = Project::restore(
            id(PROJECT_A),
            root("D:/work/a"),
            "changed".to_owned(),
            nonempty_layout(),
            ProjectRootStatus::Ready,
            u64::MAX,
        )
        .unwrap();
        let invalid = ProjectRegistry::restore(
            vec![invalid_a, original.project(id(PROJECT_B)).unwrap().clone()],
            Some(id(PROJECT_A)),
        )
        .unwrap();
        assert!(matches!(
            save_registry(&mut invalid_store, &invalid),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(load_registry(&mut invalid_store).unwrap(), original);

        let revision_temp = tempfile::tempdir().unwrap();
        let mut revision_store = populated_store(&revision_temp);
        revision_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET revision = ?1 WHERE project_id = ?2",
                params![i64::MAX, PROJECT_A],
            )
            .unwrap();
        let mut changed = sample_registry();
        changed.rename_project(id(PROJECT_A), "changed").unwrap();
        let second = insert_allocated_terminal(&mut changed, PROJECT_A, GROUP_A, TERMINAL_A_SECOND);
        let changed =
            ProjectSnapshot::restore(changed, vec![record(PROJECT_A, TERMINAL_A, 0), second])
                .unwrap();
        assert!(matches!(
            revision_store.save_project_snapshot(&changed),
            Err(ProjectStoreError::RevisionExhausted)
        ));
        assert_eq!(load_registry(&mut revision_store).unwrap(), original);

        let version_temp = tempfile::tempdir().unwrap();
        let mut version_store = populated_store(&version_temp);
        version_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET dto_version = 2 WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        let changed = sample_registry();
        assert!(matches!(
            save_registry(&mut version_store, &changed),
            Err(ProjectStoreError::Layout(
                LayoutDtoError::UnsupportedVersion
            ))
        ));
        assert_eq!(
            version_store.database.connection.query_row(
                "SELECT display_name FROM projects WHERE project_id = ?1",
                [PROJECT_A],
                |row| row.get::<_, String>(0),
            ),
            Ok("same".to_owned())
        );
    }

    #[test]
    fn project_registry_store_empty_and_missing_active_use_deterministic_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        assert_eq!(load_registry(&mut store).unwrap(), ProjectRegistry::new());

        save_registry(&mut store, &sample_registry()).unwrap();
        store
            .database
            .connection
            .execute("UPDATE app_state SET active_project_id = NULL", [])
            .unwrap();
        assert_eq!(
            load_registry(&mut store).unwrap().active(),
            Some(id(PROJECT_A))
        );
        store
            .database
            .connection
            .execute("DELETE FROM app_state", [])
            .unwrap();
        assert_eq!(
            load_registry(&mut store).unwrap().active(),
            Some(id(PROJECT_A))
        );

        save_registry(&mut store, &ProjectRegistry::new()).unwrap();
        assert_eq!(load_registry(&mut store).unwrap(), ProjectRegistry::new());
        assert_eq!(
            store
                .database
                .connection
                .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                    .get::<_, i64>(0),),
            Ok(0)
        );
    }

    /// 非 Unicode 项目根不得经替换字符进入持久库。
    #[test]
    fn project_registry_store_rejects_non_unicode_root_without_writing() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let mut path = OsString::from("D:/");
        path.push(OsString::from_wide(&[0xd800]));
        let project = Project::restore(
            id(PROJECT_A),
            CanonicalPath::new(PathBuf::from(path)).unwrap(),
            "project".to_owned(),
            ProjectLayout::empty(id(PROJECT_A)),
            ProjectRootStatus::Ready,
            0,
        )
        .unwrap();
        let registry = ProjectRegistry::restore(vec![project], Some(id(PROJECT_A))).unwrap();

        assert!(matches!(
            save_registry(&mut store, &registry),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(load_registry(&mut store).unwrap(), ProjectRegistry::new());
    }

    /// 非 Unicode 终端路径不得经替换字符进入持久库或留下项目半快照。
    #[test]
    fn project_snapshot_store_rejects_non_unicode_terminal_paths_without_writing() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        for invalid_cwd in [true, false] {
            let mut path = OsString::from("D:/");
            path.push(OsString::from_wide(&[0xd800]));
            let invalid = CanonicalPath::new(PathBuf::from(path)).unwrap();
            let record = TerminalRecord::new(
                TerminalId::from_str(TERMINAL_A).unwrap(),
                id(PROJECT_A),
                0,
                if invalid_cwd {
                    invalid.clone()
                } else {
                    root("D:/work/terminal")
                },
                if invalid_cwd {
                    root("C:/shell/pwsh.exe")
                } else {
                    invalid
                },
                None,
                RunIntent::Launch,
            )
            .unwrap();
            let snapshot = ProjectSnapshot::restore(sample_registry(), vec![record]).unwrap();

            assert!(matches!(
                store.save_project_snapshot(&snapshot),
                Err(ProjectStoreError::CorruptRecord)
            ));
            assert_eq!(
                store.load_project_snapshot().unwrap(),
                ProjectSnapshot::default()
            );
        }
    }

    #[test]
    fn project_registry_store_round_trips_long_default_names_and_suffixes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let directory = "界".repeat(81);
        let mut registry = ProjectRegistry::new();
        registry
            .add_project(id(PROJECT_A), root(&format!("D:/work/{directory}")))
            .unwrap();
        registry
            .add_project(id(PROJECT_B), root(&format!("D:/else/{directory}")))
            .unwrap();

        let saved = save_registry(&mut store, &registry);
        assert!(saved.is_ok(), "合法默认名必须可保存");
        drop(store);
        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        let restored = load_registry(&mut reopened);
        assert!(restored.is_ok(), "默认名及后缀必须可重载");
        assert_eq!(restored.unwrap(), registry);
    }

    #[test]
    fn project_registry_store_swaps_roots_without_recreating_terminals() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = registry_with_two_terminals();
        save_registry(&mut store, &original).unwrap();
        let terminal_a_rowid = terminal_rowid(&store, TERMINAL_A);
        let terminal_b_rowid = terminal_rowid(&store, TERMINAL_B);
        let project_a = Project::restore(
            id(PROJECT_A),
            root("D:/work/b"),
            "a".to_owned(),
            nonempty_layout(),
            ProjectRootStatus::Ready,
            41,
        )
        .unwrap();
        let project_b = Project::restore(
            id(PROJECT_B),
            root("D:/work/a"),
            "b".to_owned(),
            original.project(id(PROJECT_B)).unwrap().layout().clone(),
            ProjectRootStatus::Ready,
            7,
        )
        .unwrap();
        let swapped =
            ProjectRegistry::restore(vec![project_a, project_b], Some(id(PROJECT_B))).unwrap();

        let saved = save_registry(&mut store, &swapped);
        assert!(saved.is_ok(), "合法根交换必须可保存");
        assert_eq!(load_registry(&mut store).unwrap(), swapped);
        assert_eq!(terminal_rowid(&store, TERMINAL_A), terminal_a_rowid);
        assert_eq!(terminal_rowid(&store, TERMINAL_B), terminal_b_rowid);
    }

    #[test]
    fn project_registry_store_metadata_only_changes_keep_layout_revisions() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        let mut changed = sample_registry();
        changed.rename_project(id(PROJECT_A), "renamed").unwrap();
        changed
            .relocate_project(id(PROJECT_A), root("D:/new/a"))
            .unwrap();
        changed
            .set_root_status(id(PROJECT_B), ProjectRootStatus::Ready)
            .unwrap();
        assert_eq!(changed.allocate_terminal_number(id(PROJECT_A)), Ok(41));
        changed.move_project(id(PROJECT_B), 0).unwrap();
        changed.activate_project(id(PROJECT_A)).unwrap();

        save_registry(&mut store, &changed).unwrap();
        assert_eq!(revision(&store, PROJECT_A), 0);
        assert_eq!(revision(&store, PROJECT_B), 0);
        assert_eq!(load_registry(&mut store).unwrap(), changed);
    }

    #[test]
    fn project_registry_store_save_rejects_missing_retained_layout() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        let mut changed = sample_registry();
        let second = insert_allocated_terminal(&mut changed, PROJECT_A, GROUP_A, TERMINAL_A_SECOND);
        let changed_snapshot = ProjectSnapshot::restore(
            changed.clone(),
            vec![record(PROJECT_A, TERMINAL_A, 0), second.clone()],
        )
        .unwrap();
        store.save_project_snapshot(&changed_snapshot).unwrap();
        assert_eq!(revision(&store, PROJECT_A), 1);
        store
            .database
            .connection
            .execute(
                "DELETE FROM project_layouts WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        changed.rename_project(id(PROJECT_B), "changed-b").unwrap();
        let changed_snapshot =
            ProjectSnapshot::restore(changed, vec![record(PROJECT_A, TERMINAL_A, 0), second])
                .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed_snapshot),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(
            store.database.connection.query_row(
                "SELECT COUNT(*) FROM project_layouts WHERE project_id = ?1",
                [PROJECT_A],
                |row| row.get::<_, i64>(0),
            ),
            Ok(0)
        );
        assert_eq!(
            store.database.connection.query_row(
                "SELECT display_name FROM projects WHERE project_id = ?1",
                [PROJECT_B],
                |row| row.get::<_, String>(0),
            ),
            Ok("same".to_owned())
        );
    }

    #[test]
    fn project_registry_store_save_rejects_retained_group_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        store
            .database
            .connection
            .execute(
                "DELETE FROM panel_groups WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        let mut changed = sample_registry();
        changed.rename_project(id(PROJECT_B), "changed-b").unwrap();

        assert!(matches!(
            save_registry(&mut store, &changed),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(
            store.database.connection.query_row(
                "SELECT display_name FROM projects WHERE project_id = ?1",
                [PROJECT_B],
                |row| row.get::<_, String>(0),
            ),
            Ok("same".to_owned())
        );
    }

    #[test]
    fn project_registry_store_save_rejects_malformed_retained_layout() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET layout_json = '{}' WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();

        assert!(matches!(
            save_registry(&mut store, &sample_registry()),
            Err(ProjectStoreError::Layout(LayoutDtoError::InvalidStructure))
        ));
        assert_eq!(
            store.database.connection.query_row(
                "SELECT layout_json FROM project_layouts WHERE project_id = ?1",
                [PROJECT_A],
                |row| row.get::<_, String>(0),
            ),
            Ok("{}".to_owned())
        );
    }

    #[test]
    fn project_registry_store_rejects_cross_project_group_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        let original = load_registry(&mut store).unwrap();
        let mut changed = snapshot(sample_registry());
        changed
            .registry_mut()
            .layout_mut(id(PROJECT_B))
            .unwrap()
            .open_first_group(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                PanelGroup::new(
                    vec![terminal(id(PROJECT_B), TERMINAL_A_SECOND)],
                    terminal(id(PROJECT_B), TERMINAL_A_SECOND),
                )
                .unwrap(),
            )
            .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::Snapshot(ProjectSnapshotError::Project(
                ProjectRegistryError::DuplicatePanelGroup(group)
            ))) if group == PanelGroupId::from_str(GROUP_A).unwrap()
        ));
        assert_eq!(load_registry(&mut store).unwrap(), original);
    }

    #[test]
    fn project_registry_store_late_failure_rolls_back_prior_project_writes() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        save_registry(&mut store, &registry_with_two_terminals()).unwrap();
        store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET revision = ?1 WHERE project_id = ?2",
                params![i64::MAX, PROJECT_B],
            )
            .unwrap();
        let original = load_registry(&mut store).unwrap();
        let mut changed = registry_with_two_terminals();
        changed.rename_project(id(PROJECT_A), "changed-a").unwrap();
        let second = insert_allocated_terminal(&mut changed, PROJECT_A, GROUP_A, TERMINAL_A_SECOND);
        changed.rename_project(id(PROJECT_B), "changed-b").unwrap();
        changed
            .layout_mut(id(PROJECT_B))
            .unwrap()
            .close_view(&terminal(id(PROJECT_B), TERMINAL_B))
            .unwrap();
        let changed =
            ProjectSnapshot::restore(changed, vec![record(PROJECT_A, TERMINAL_A, 0), second])
                .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&changed),
            Err(ProjectStoreError::RevisionExhausted)
        ));
        assert_eq!(load_registry(&mut store).unwrap(), original);
        assert_eq!(
            store
                .database
                .connection
                .query_row("SELECT COUNT(*) FROM terminals", [], |row| row
                    .get::<_, i64>(0)),
            Ok(2)
        );
    }
}
