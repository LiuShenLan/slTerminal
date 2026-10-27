//! 项目聚合、活跃项与规范布局的唯一 SQLite 存取通道。

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use rusqlite::{OptionalExtension, TransactionBehavior, params};
use slterm_path::CanonicalPath;
use slterm_workspace::{
    LayoutDtoError, PROJECT_LAYOUT_DTO_VERSION, PanelGroupId, Project, ProjectId, ProjectLayout,
    ProjectRegistry, ProjectRegistryError, ProjectRootStatus,
};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

/// 项目聚合读写失败；不向展示层暴露 SQL、路径或布局正文。
#[derive(Debug)]
pub(crate) enum ProjectStoreError {
    Project(ProjectRegistryError),
    Layout(LayoutDtoError),
    CorruptRecord,
    RevisionExhausted,
    Database(DatabaseError),
}

impl fmt::Display for ProjectStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Project(_) => "项目聚合持久记录不一致",
            Self::Layout(error) => return fmt::Display::fmt(error, formatter),
            Self::CorruptRecord => "项目聚合持久记录不一致",
            Self::RevisionExhausted => "项目布局 revision 已耗尽",
            Self::Database(_) => "项目聚合数据库操作失败",
        })
    }
}

impl std::error::Error for ProjectStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(_) | Self::CorruptRecord | Self::RevisionExhausted => None,
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
    /// 在一个事务内保存项目顺序、活跃项、组身份和规范布局。
    pub(crate) fn save_project_registry(
        &mut self,
        registry: &ProjectRegistry,
    ) -> Result<(), ProjectStoreError> {
        registry.validate()?;
        // 先在内存完成全部无损转换，避免可预见的非法值进入写事务。
        let encoded = registry
            .projects()
            .iter()
            .enumerate()
            .map(|(index, project)| EncodedProject::new(index, project))
            .collect::<Result<Vec<_>, _>>()?;
        let active = registry.active().map(|id| id.to_string());
        let dto_version = i64::try_from(PROJECT_LAYOUT_DTO_VERSION)
            .map_err(|_| ProjectStoreError::CorruptRecord)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

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
        transaction.execute(
            "INSERT INTO app_state (singleton, active_project_id) VALUES (1, ?1) \
             ON CONFLICT(singleton) DO UPDATE SET active_project_id = excluded.active_project_id",
            [active],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// 恢复全部项目；活跃项缺失时按导航顺序回退到首个项目。
    pub(crate) fn load_project_registry(&self) -> Result<ProjectRegistry, ProjectStoreError> {
        let connection = &self.database.connection;
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
        Ok(ProjectRegistry::restore(projects, active)?)
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
        ProjectRegistry, ProjectRootStatus, TerminalId, ViewKey,
    };
    use tempfile::TempDir;

    use super::*;

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
    const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
    const TERMINAL_A_SECOND: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn populated_store(temp: &TempDir) -> WorkspaceDatabase {
        let mut store = WorkspaceDatabase::open(&fixture(temp)).unwrap();
        store.save_project_registry(&sample_registry()).unwrap();
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

    #[test]
    fn project_registry_store_round_trips_ordered_projects_and_layout() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let expected = sample_registry();

        let saved = store.save_project_registry(&expected);
        assert!(saved.is_ok(), "合法项目聚合必须可保存");
        drop(store);

        let reopened = WorkspaceDatabase::open(&paths).unwrap();
        let restored = reopened.load_project_registry();
        assert!(restored.is_ok(), "已保存项目聚合必须可恢复");
        assert_eq!(restored.unwrap(), expected);
    }

    #[test]
    fn project_registry_store_updates_revision_without_recreating_retained_terminals() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let mut registry = sample_registry();
        store.save_project_registry(&registry).unwrap();
        for (terminal_id, project_id, number) in
            [("terminal-a", PROJECT_A, 0), ("terminal-b", PROJECT_B, 0)]
        {
            store
                .database
                .connection
                .execute(
                    "INSERT INTO terminals \
                     (terminal_id, project_id, number, cwd, shell, run_intent) \
                     VALUES (?1, ?2, ?3, 'D:/', 'pwsh', 'reuse')",
                    params![terminal_id, project_id, number],
                )
                .unwrap();
        }

        store.save_project_registry(&registry).unwrap();
        assert_eq!(revision(&store, PROJECT_A), 0);
        registry.rename_project(id(PROJECT_A), "renamed").unwrap();
        registry
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .insert_view(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                terminal(id(PROJECT_A), TERMINAL_A_SECOND),
            )
            .unwrap();
        registry.remove_project(id(PROJECT_B)).unwrap();
        store.save_project_registry(&registry).unwrap();

        assert_eq!(revision(&store, PROJECT_A), 1);
        assert_eq!(
            store.database.connection.query_row(
                "SELECT COUNT(*) FROM terminals WHERE terminal_id = 'terminal-a'",
                [],
                |row| row.get::<_, i64>(0),
            ),
            Ok(1)
        );
        assert_eq!(
            store.database.connection.query_row(
                "SELECT COUNT(*) FROM terminals WHERE terminal_id = 'terminal-b'",
                [],
                |row| row.get::<_, i64>(0),
            ),
            Ok(0)
        );
        assert_eq!(store.load_project_registry().unwrap(), registry);
        assert_eq!(PROJECT_LAYOUT_DTO_VERSION, 1);
    }

    #[test]
    fn project_registry_store_rejects_missing_layout_instead_of_dropping_project() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store.save_project_registry(&sample_registry()).unwrap();
        store
            .database
            .connection
            .execute(
                "DELETE FROM project_layouts WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();

        assert!(matches!(
            store.load_project_registry(),
            Err(ProjectStoreError::CorruptRecord)
        ));
    }

    #[test]
    fn project_registry_store_distinguishes_corruption_and_unsupported_layout() {
        let status_temp = tempfile::tempdir().unwrap();
        let status_store = populated_store(&status_temp);
        status_store
            .database
            .connection
            .execute(
                "UPDATE projects SET root_status = 'unknown' WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            status_store.load_project_registry(),
            Err(ProjectStoreError::CorruptRecord)
        ));

        let group_temp = tempfile::tempdir().unwrap();
        let group_store = populated_store(&group_temp);
        group_store
            .database
            .connection
            .execute(
                "DELETE FROM panel_groups WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            group_store.load_project_registry(),
            Err(ProjectStoreError::CorruptRecord)
        ));

        let version_temp = tempfile::tempdir().unwrap();
        let version_store = populated_store(&version_temp);
        version_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET dto_version = 2 WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            version_store.load_project_registry(),
            Err(ProjectStoreError::Layout(
                LayoutDtoError::UnsupportedVersion
            ))
        ));

        let json_temp = tempfile::tempdir().unwrap();
        let json_store = populated_store(&json_temp);
        json_store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET layout_json = '{}' WHERE project_id = ?1",
                [PROJECT_A],
            )
            .unwrap();
        assert!(matches!(
            json_store.load_project_registry(),
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
            invalid_store.save_project_registry(&invalid),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(invalid_store.load_project_registry().unwrap(), original);

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
        changed
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .insert_view(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                terminal(id(PROJECT_A), TERMINAL_A_SECOND),
            )
            .unwrap();
        assert!(matches!(
            revision_store.save_project_registry(&changed),
            Err(ProjectStoreError::RevisionExhausted)
        ));
        assert_eq!(revision_store.load_project_registry().unwrap(), original);

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
            version_store.save_project_registry(&changed),
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
        assert_eq!(
            store.load_project_registry().unwrap(),
            ProjectRegistry::new()
        );

        store.save_project_registry(&sample_registry()).unwrap();
        store
            .database
            .connection
            .execute("UPDATE app_state SET active_project_id = NULL", [])
            .unwrap();
        assert_eq!(
            store.load_project_registry().unwrap().active(),
            Some(id(PROJECT_A))
        );
        store
            .database
            .connection
            .execute("DELETE FROM app_state", [])
            .unwrap();
        assert_eq!(
            store.load_project_registry().unwrap().active(),
            Some(id(PROJECT_A))
        );

        store
            .save_project_registry(&ProjectRegistry::new())
            .unwrap();
        assert_eq!(
            store.load_project_registry().unwrap(),
            ProjectRegistry::new()
        );
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
            store.save_project_registry(&registry),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(
            store.load_project_registry().unwrap(),
            ProjectRegistry::new()
        );
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

        let saved = store.save_project_registry(&registry);
        assert!(saved.is_ok(), "合法默认名必须可保存");
        drop(store);
        let reopened = WorkspaceDatabase::open(&paths).unwrap();
        let restored = reopened.load_project_registry();
        assert!(restored.is_ok(), "默认名及后缀必须可重载");
        assert_eq!(restored.unwrap(), registry);
    }

    #[test]
    fn project_registry_store_swaps_roots_without_recreating_terminals() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        for (terminal_id, project_id) in [("terminal-a", PROJECT_A), ("terminal-b", PROJECT_B)] {
            store
                .database
                .connection
                .execute(
                    "INSERT INTO terminals \
                     (terminal_id, project_id, number, cwd, shell, run_intent) \
                     VALUES (?1, ?2, 0, 'D:/', 'pwsh', 'reuse')",
                    params![terminal_id, project_id],
                )
                .unwrap();
        }
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
            ProjectLayout::empty(id(PROJECT_B)),
            ProjectRootStatus::Ready,
            7,
        )
        .unwrap();
        let swapped =
            ProjectRegistry::restore(vec![project_a, project_b], Some(id(PROJECT_B))).unwrap();

        let saved = store.save_project_registry(&swapped);
        assert!(saved.is_ok(), "合法根交换必须可保存");
        assert_eq!(store.load_project_registry().unwrap(), swapped);
        assert_eq!(
            store
                .database
                .connection
                .query_row("SELECT COUNT(*) FROM terminals", [], |row| row
                    .get::<_, i64>(0)),
            Ok(2)
        );
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

        store.save_project_registry(&changed).unwrap();
        assert_eq!(revision(&store, PROJECT_A), 0);
        assert_eq!(revision(&store, PROJECT_B), 0);
        assert_eq!(store.load_project_registry().unwrap(), changed);
    }

    #[test]
    fn project_registry_store_save_rejects_missing_retained_layout() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        let mut changed = sample_registry();
        changed
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .insert_view(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                terminal(id(PROJECT_A), TERMINAL_A_SECOND),
            )
            .unwrap();
        store.save_project_registry(&changed).unwrap();
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

        assert!(matches!(
            store.save_project_registry(&changed),
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
            store.save_project_registry(&changed),
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
            store.save_project_registry(&sample_registry()),
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
        let original = store.load_project_registry().unwrap();
        let mut changed = sample_registry();
        changed
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
            store.save_project_registry(&changed),
            Err(ProjectStoreError::Project(
                ProjectRegistryError::DuplicatePanelGroup(group)
            )) if group == PanelGroupId::from_str(GROUP_A).unwrap()
        ));
        assert_eq!(store.load_project_registry().unwrap(), original);
    }

    #[test]
    fn project_registry_store_late_failure_rolls_back_prior_project_writes() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = populated_store(&temp);
        for (terminal_id, project_id) in [("terminal-a", PROJECT_A), ("terminal-b", PROJECT_B)] {
            store
                .database
                .connection
                .execute(
                    "INSERT INTO terminals \
                     (terminal_id, project_id, number, cwd, shell, run_intent) \
                     VALUES (?1, ?2, 0, 'D:/', 'pwsh', 'reuse')",
                    params![terminal_id, project_id],
                )
                .unwrap();
        }
        store
            .database
            .connection
            .execute(
                "UPDATE project_layouts SET revision = ?1 WHERE project_id = ?2",
                params![i64::MAX, PROJECT_B],
            )
            .unwrap();
        let original = store.load_project_registry().unwrap();
        let mut changed = sample_registry();
        changed.rename_project(id(PROJECT_A), "changed-a").unwrap();
        changed
            .layout_mut(id(PROJECT_A))
            .unwrap()
            .insert_view(
                PanelGroupId::from_str(GROUP_A).unwrap(),
                terminal(id(PROJECT_A), TERMINAL_A_SECOND),
            )
            .unwrap();
        changed.rename_project(id(PROJECT_B), "changed-b").unwrap();
        changed
            .layout_mut(id(PROJECT_B))
            .unwrap()
            .open_first_group(
                PanelGroupId::from_str(GROUP_B).unwrap(),
                PanelGroup::new(
                    vec![terminal(id(PROJECT_B), TERMINAL_A_SECOND)],
                    terminal(id(PROJECT_B), TERMINAL_A_SECOND),
                )
                .unwrap(),
            )
            .unwrap();

        assert!(matches!(
            store.save_project_registry(&changed),
            Err(ProjectStoreError::RevisionExhausted)
        ));
        assert_eq!(store.load_project_registry().unwrap(), original);
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
