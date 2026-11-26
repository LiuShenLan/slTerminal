//! 项目聚合、活跃项与规范布局的唯一 SQLite 存取通道。

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use slterm_agents::AgentSession;
use slterm_document::DocumentId;
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_workspace::{
    LayoutDtoError, PROJECT_LAYOUT_DTO_VERSION, PanelGroupId, Project, ProjectId, ProjectLayout,
    ProjectRegistry, ProjectRegistryError, ProjectRootStatus, ProjectSnapshot,
    ProjectSnapshotError, RunIntent, TerminalId, TerminalMetadata, TerminalRecord,
};

#[path = "project_documents.rs"]
mod documents;

pub(super) use documents::delete_project_documents_in_transaction;
use documents::{EncodedDocument, load_document_records};

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
        let prepared = PreparedProjectSnapshot::new(snapshot)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::save_project_snapshot_in_transaction(&transaction, &prepared, None)?;
        transaction.commit()?;
        Ok(())
    }

    /// 在调用方事务内保存已完成纯校验与编码的项目快照；本函数不自行提交。
    pub(super) fn save_project_snapshot_in_transaction(
        transaction: &Connection,
        prepared: &PreparedProjectSnapshot<'_>,
        retained_project: Option<ProjectId>,
    ) -> Result<(), ProjectStoreError> {
        let snapshot = prepared.snapshot;
        let encoded = &prepared.projects;
        let active = &prepared.active;
        let terminals = &prepared.terminals;
        let documents = &prepared.documents;
        let dto_version = prepared.dto_version;

        // 启动已归一的跨记录引用使用同一规则恢复；其他损坏仍阻止保存。
        let (stored_registry, stored_terminals, stored_documents) =
            decode_project_snapshot_from(transaction)?;
        let stored_snapshot = ProjectSnapshot::recover_with_documents(
            stored_registry,
            stored_terminals,
            stored_documents,
        )?;
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
        let retained_project_id = retained_project.map(|project_id| project_id.to_string());
        for stale_id in stored_ids
            .iter()
            .filter(|id| !desired_ids.contains(id.as_str()))
        {
            // 项目关闭的初始提交只保存其他项目，目标项目留待 cleanup 级联删除。
            if retained_project_id.as_deref() == Some(stale_id.as_str()) {
                continue;
            }
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

        for project in encoded {
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
                    let stored_groups = stored_group_ids(transaction, project_id)?;
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
        let retained_terminal_ids = stored_snapshot
            .terminals()
            .filter(|terminal| Some(terminal.project_id()) == retained_project)
            .map(|terminal| terminal.id().to_string())
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
            // 保留项目下的终端同样不能在初始关闭提交中提前删除。
            if retained_terminal_ids.contains(stale_id) {
                continue;
            }
            transaction.execute("DELETE FROM terminals WHERE terminal_id = ?1", [stale_id])?;
        }
        for terminal in terminals {
            transaction.execute(
                "INSERT INTO terminals \
                 (terminal_id, project_id, number, cwd, shell, custom_title, agent_source, agent_session_id, agent_session_file, run_intent) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
                 ON CONFLICT(terminal_id) DO UPDATE SET \
                 cwd = excluded.cwd, shell = excluded.shell, \
                 custom_title = excluded.custom_title, agent_source = excluded.agent_source, \
                 agent_session_id = excluded.agent_session_id, \
                 agent_session_file = excluded.agent_session_file, run_intent = excluded.run_intent",
                params![
                    &terminal.id,
                    &terminal.project_id,
                    terminal.number,
                    &terminal.cwd,
                    &terminal.shell,
                    &terminal.custom_title,
                    &terminal.agent_source,
                    &terminal.agent_session_id,
                    &terminal.agent_session_file,
                    terminal.run_intent,
                ],
            )?;
        }
        let desired_document_ids = snapshot
            .documents()
            .map(|(id, _)| id)
            .collect::<BTreeSet<_>>();
        let retained_document_ids = retained_project
            .and_then(|project_id| stored_snapshot.registry().project(project_id))
            .map(|project| {
                project
                    .layout()
                    .view_keys()
                    .into_iter()
                    .filter_map(|view| match view {
                        slterm_workspace::ViewKey::Editor { document_id, .. }
                        | slterm_workspace::ViewKey::Markdown { document_id, .. }
                        | slterm_workspace::ViewKey::Diff { document_id, .. } => Some(document_id),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        for (stored_id, _) in stored_snapshot.documents() {
            if !desired_document_ids.contains(&stored_id)
                && !retained_document_ids.contains(&stored_id)
            {
                transaction.execute(
                    "DELETE FROM documents WHERE document_id = ?1",
                    [stored_id.to_string()],
                )?;
            }
        }
        for document in documents {
            transaction.execute(
                "INSERT INTO documents (document_id, location_json, file_identity_json) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT(document_id) DO UPDATE SET \
                 location_json = excluded.location_json, \
                 file_identity_json = excluded.file_identity_json",
                params![document.id, document.location_json, document.identity_json],
            )?;
        }
        transaction.execute(
            "INSERT INTO app_state (singleton, active_project_id) VALUES (1, ?1) \
             ON CONFLICT(singleton) DO UPDATE SET active_project_id = excluded.active_project_id",
            [active],
        )?;
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
    /// 启动恢复在同一事务读取完整布局，再确定性归一失效终端引用。
    pub(crate) fn recover_project_snapshot(
        &mut self,
    ) -> Result<ProjectSnapshot, ProjectStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (registry, terminals, documents) = decode_project_snapshot_from(&transaction)?;
        let snapshot = ProjectSnapshot::recover_with_documents(registry, terminals, documents)?;
        transaction.commit()?;
        Ok(snapshot)
    }
}

fn load_project_snapshot_from(
    connection: &rusqlite::Connection,
) -> Result<ProjectSnapshot, ProjectStoreError> {
    let (registry, terminals, documents) = decode_project_snapshot_from(connection)?;
    Ok(ProjectSnapshot::restore_with_documents(
        registry, terminals, documents,
    )?)
}

type DecodedProjectSnapshot = (
    ProjectRegistry,
    Vec<TerminalRecord>,
    Vec<(DocumentId, ResolvedPath)>,
);

fn decode_project_snapshot_from(
    connection: &rusqlite::Connection,
) -> Result<DecodedProjectSnapshot, ProjectStoreError> {
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
        // 恢复宽容解码：未知视图类型剔除、重复视图保留首个；
        // 信封、版本与其余结构损坏仍整体失败。
        let layout = ProjectLayout::from_canonical_json_lossy(&layout_json)?;
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
    let documents = load_document_records(connection)?;
    Ok((registry, terminals, documents))
}

/// 事务前完成全部输入校验与无损编码，写锁内只消费已准备数据。
pub(super) struct PreparedProjectSnapshot<'a> {
    snapshot: &'a ProjectSnapshot,
    projects: Vec<EncodedProject>,
    terminals: Vec<EncodedTerminal>,
    documents: Vec<EncodedDocument>,
    active: Option<String>,
    dto_version: i64,
}

impl<'a> PreparedProjectSnapshot<'a> {
    pub(super) fn new(snapshot: &'a ProjectSnapshot) -> Result<Self, ProjectStoreError> {
        snapshot.validate()?;
        let registry = snapshot.registry();
        let projects = registry
            .projects()
            .iter()
            .enumerate()
            .map(|(index, project)| EncodedProject::new(index, project))
            .collect::<Result<Vec<_>, _>>()?;
        let terminals = snapshot
            .terminals()
            .map(EncodedTerminal::new)
            .collect::<Result<Vec<_>, _>>()?;
        let documents = snapshot
            .documents()
            .map(|(id, location)| EncodedDocument::new(id, location))
            .collect::<Result<Vec<_>, _>>()?;
        let active = registry.active().map(|id| id.to_string());
        let dto_version = i64::try_from(PROJECT_LAYOUT_DTO_VERSION)
            .map_err(|_| ProjectStoreError::CorruptRecord)?;
        Ok(Self {
            snapshot,
            projects,
            terminals,
            documents,
            active,
            dto_version,
        })
    }
}

struct EncodedTerminal {
    id: String,
    project_id: String,
    number: i64,
    cwd: String,
    shell: String,
    custom_title: Option<String>,
    agent_source: Option<String>,
    agent_session_id: Option<String>,
    agent_session_file: Option<String>,
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
            agent_source: record.agent().map(|agent| agent.source.clone()),
            agent_session_id: record.agent().and_then(|agent| agent.session_id.clone()),
            agent_session_file: record.agent().and_then(|agent| agent.session_file.clone()),
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
        "SELECT terminal_id, project_id, number, cwd, shell, custom_title, \
         agent_source, agent_session_id, agent_session_file, run_intent \
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
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                id,
                project_id,
                number,
                cwd,
                shell,
                custom_title,
                agent_source,
                agent_session_id,
                agent_session_file,
                run_intent,
            )| {
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
                let agent = agent_source.map(|source| AgentSession {
                    source,
                    session_id: agent_session_id,
                    session_file: agent_session_file,
                });
                TerminalRecord::new(
                    id,
                    project_id,
                    number,
                    cwd,
                    shell,
                    TerminalMetadata::new(custom_title, agent),
                    run_intent,
                )
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
#[path = "project_tests.rs"]
mod project_tests;
