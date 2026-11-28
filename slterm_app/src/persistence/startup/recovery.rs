//! 领域启动恢复：布局归一先于快照对账，数据库双损保留现场后重建。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use slterm_path::AppPaths;
use slterm_workspace::ProjectSnapshot;

use crate::persistence::database::{DatabaseAccess, DatabaseError};
use crate::persistence::project::ProjectStoreError;
use crate::persistence::workspace::WorkspaceDatabase;

/// 快照恢复器只消费已经完成布局归一的同一份项目快照。
pub(crate) trait SnapshotRecoverer {
    fn recover(&mut self, layout: &ProjectSnapshot) -> Result<(), SnapshotRecoveryError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotRecoveryError {
    Failed,
}

#[derive(Debug)]
pub(crate) enum StartupRecoveryError {
    Project(ProjectStoreError),
    Snapshot(SnapshotRecoveryError),
}

impl fmt::Display for StartupRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Project(_) => "工作区布局恢复失败",
            Self::Snapshot(_) => "工作区快照恢复失败",
        })
    }
}

impl std::error::Error for StartupRecoveryError {}

impl From<ProjectStoreError> for StartupRecoveryError {
    fn from(error: ProjectStoreError) -> Self {
        Self::Project(error)
    }
}

impl From<SnapshotRecoveryError> for StartupRecoveryError {
    fn from(error: SnapshotRecoveryError) -> Self {
        Self::Snapshot(error)
    }
}

pub(crate) struct StartupRecovery;

impl StartupRecovery {
    /// 先恢复并归一布局；没有真实快照恢复器时不执行任何快照动作。
    pub(crate) fn recover(
        store: &mut WorkspaceDatabase,
        snapshots: Option<&mut dyn SnapshotRecoverer>,
    ) -> Result<ProjectSnapshot, StartupRecoveryError> {
        let layout = store.recover_project_snapshot()?;
        if let Some(recoverer) = snapshots {
            recoverer.recover(&layout)?;
        }
        Ok(layout)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceOpenDisposition {
    Clean,
    RestoredFromBackup,
    Isolated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceRecoveryNotice {
    RestoredFromBackup,
    Isolated,
}

#[derive(Debug)]
pub(crate) enum WorkspaceOpenError {
    Database(DatabaseError),
    Isolation(io::Error),
}

impl fmt::Display for WorkspaceOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database(_) => "工作区数据库打开失败",
            Self::Isolation(_) => "工作区损坏现场隔离失败",
        })
    }
}

impl std::error::Error for WorkspaceOpenError {}

impl From<DatabaseError> for WorkspaceOpenError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<io::Error> for WorkspaceOpenError {
    fn from(error: io::Error) -> Self {
        Self::Isolation(error)
    }
}

pub(crate) struct RecoveredWorkspace {
    pub(crate) store: WorkspaceDatabase,
    pub(crate) disposition: WorkspaceOpenDisposition,
    quarantine_path: Option<PathBuf>,
}

impl RecoveredWorkspace {
    pub(crate) fn notice(&self) -> Option<WorkspaceRecoveryNotice> {
        match self.disposition {
            WorkspaceOpenDisposition::Clean => None,
            WorkspaceOpenDisposition::RestoredFromBackup => {
                Some(WorkspaceRecoveryNotice::RestoredFromBackup)
            }
            WorkspaceOpenDisposition::Isolated => Some(WorkspaceRecoveryNotice::Isolated),
        }
    }

    pub(crate) fn quarantine_path(&self) -> Option<&Path> {
        self.quarantine_path.as_deref()
    }
}

/// 仅当前版本物理损坏进入副本恢复；高版本、异库和存储合同错误保持原分流。
pub(crate) fn open_workspace_with_recovery(
    paths: &AppPaths,
) -> Result<RecoveredWorkspace, WorkspaceOpenError> {
    // 先完成上次崩溃遗留的隔离改名；只有改名全部落定后才允许打开空工作区。
    let interrupted_isolation = complete_interrupted_isolation(paths)?;
    match WorkspaceDatabase::open(paths) {
        Ok(store) => {
            if let Some(quarantine_path) = interrupted_isolation {
                finalize_isolation(&quarantine_path)?;
                return Ok(RecoveredWorkspace {
                    store,
                    disposition: WorkspaceOpenDisposition::Isolated,
                    quarantine_path: Some(quarantine_path),
                });
            }
            return Ok(RecoveredWorkspace {
                store,
                disposition: WorkspaceOpenDisposition::Clean,
                quarantine_path: None,
            });
        }
        Err(DatabaseError::Corrupt) => {}
        Err(error) => return Err(WorkspaceOpenError::Database(error)),
    }

    let backup_path = paths.workspace_disaster_copy_path();
    let backup = if backup_path.try_exists()? {
        WorkspaceDatabase::open_path(&backup_path).ok()
    } else {
        None
    };
    if let Some(backup) = backup
        && backup.access() == DatabaseAccess::ReadWrite
    {
        let temporary = paths
            .local_root()
            .join(format!(".workspace-restored-{}.db", uuid::Uuid::now_v7()));
        let result = (|| {
            backup.backup_to_new(&temporary)?;
            let quarantine_path = preserve_files(paths)?;
            crate::atomic_file::replace(&temporary, &paths.workspace_db_path())?;
            Ok(RecoveredWorkspace {
                store: WorkspaceDatabase::open(paths)?,
                disposition: WorkspaceOpenDisposition::RestoredFromBackup,
                quarantine_path: Some(quarantine_path),
            })
        })();
        if result.is_err() {
            remove_temporary_database(&temporary);
        }
        return result;
    }

    let quarantine_path = quarantine_files(paths)?;
    let store = WorkspaceDatabase::open(paths)?;
    finalize_isolation(&quarantine_path)?;
    Ok(RecoveredWorkspace {
        store,
        disposition: WorkspaceOpenDisposition::Isolated,
        quarantine_path: Some(quarantine_path),
    })
}

/// 隔离改名进行中的持久标记；完成改名与空库打开后才移除。
const ISOLATION_MARKER: &str = ".incomplete";

fn workspace_sources(paths: &AppPaths) -> Vec<PathBuf> {
    let mut sources = vec![paths.workspace_db_path()];
    sources.extend(
        ["-wal", "-shm"]
            .into_iter()
            .map(|suffix| sidecar_path(&paths.workspace_db_path(), suffix)),
    );
    let backup = paths.workspace_disaster_copy_path();
    sources.push(backup.clone());
    sources.extend(
        ["-wal", "-shm"]
            .into_iter()
            .map(|suffix| sidecar_path(&backup, suffix)),
    );
    sources
}

/// 完成上次崩溃遗留的隔离改名；返回仍需收尾的隔离目录。
///
/// 标记存在即代表双损隔离已开始。此时只向前完成改名：任一文件同时存在于
/// 原位与隔离目录视为矛盾，明确失败；绝不把中间态交给「首次使用」空库路径。
fn complete_interrupted_isolation(paths: &AppPaths) -> Result<Option<PathBuf>, WorkspaceOpenError> {
    let root = paths.local_root().join("quarantine");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(WorkspaceOpenError::Isolation(error)),
    };
    let mut incomplete = Vec::new();
    for entry in entries {
        let directory = entry?.path();
        if directory.join(ISOLATION_MARKER).is_file() {
            incomplete.push(directory);
        }
    }
    incomplete.sort();
    for directory in &incomplete {
        for source in workspace_sources(paths) {
            let file_name = source
                .file_name()
                .ok_or_else(|| WorkspaceOpenError::Isolation(io::Error::other("隔离文件名无效")))?;
            let quarantined = directory.join(file_name);
            let source_present = source.try_exists()?;
            let quarantined_present = quarantined.try_exists()?;
            match (source_present, quarantined_present) {
                (true, true) => {
                    return Err(WorkspaceOpenError::Isolation(io::Error::other(
                        "隔离中间态同时存在原位与副本",
                    )));
                }
                (true, false) => std::fs::rename(&source, &quarantined)?,
                (false, _) => {}
            }
        }
    }
    Ok(incomplete.pop())
}

fn finalize_isolation(directory: &Path) -> Result<(), WorkspaceOpenError> {
    match std::fs::remove_file(directory.join(ISOLATION_MARKER)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(WorkspaceOpenError::Isolation(error)),
    }
}

fn quarantine_directory(paths: &AppPaths) -> Result<PathBuf, WorkspaceOpenError> {
    let directory = paths
        .local_root()
        .join("quarantine")
        .join(format!("workspace-{}", uuid::Uuid::now_v7()));
    if let Some(parent) = directory.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&directory)?;
    Ok(directory)
}

/// 有效副本恢复前复制原现场；替换失败时原损坏库仍留在原路径。
fn preserve_files(paths: &AppPaths) -> Result<PathBuf, WorkspaceOpenError> {
    let directory = quarantine_directory(paths)?;
    let mut sources = vec![paths.workspace_db_path()];
    sources.extend(
        ["-wal", "-shm"]
            .into_iter()
            .map(|suffix| sidecar_path(&paths.workspace_db_path(), suffix)),
    );
    for source in sources {
        match std::fs::metadata(&source) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(WorkspaceOpenError::Isolation(error)),
        }
        let file_name = source
            .file_name()
            .ok_or_else(|| WorkspaceOpenError::Isolation(io::Error::other("隔离文件名无效")))?;
        let destination = directory.join(file_name);
        std::fs::copy(&source, &destination)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(destination)?
            .sync_all()?;
    }
    Ok(directory)
}

fn remove_temporary_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(sidecar_path(path, suffix));
    }
}

/// 同一本地卷内改名保留原始字节；任一步失败均不得继续建立空工作区。
fn quarantine_files(paths: &AppPaths) -> Result<PathBuf, WorkspaceOpenError> {
    let directory = quarantine_directory(paths)?;
    // 标记先于任何改名落定：崩溃后重启必须继续完成隔离，而不是初始化空库。
    let marker = directory.join(ISOLATION_MARKER);
    std::fs::write(&marker, b"")?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&marker)?
        .sync_all()?;
    for source in workspace_sources(paths) {
        match std::fs::metadata(&source) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(WorkspaceOpenError::Isolation(error)),
        }
        let file_name = source
            .file_name()
            .ok_or_else(|| WorkspaceOpenError::Isolation(io::Error::other("隔离文件名无效")))?;
        std::fs::rename(&source, directory.join(file_name))?;
    }
    Ok(directory)
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    use slterm_path::{AppPaths, CanonicalPath};
    use slterm_workspace::{
        PanelGroup, PanelGroupId, Project, ProjectId, ProjectLayout, ProjectRegistry,
        ProjectRootStatus, ProjectSnapshot, RunIntent, TerminalId, TerminalMetadata,
        TerminalRecord, ViewKey,
    };

    use super::{
        SnapshotRecoverer, StartupRecovery, WorkspaceOpenDisposition, open_workspace_with_recovery,
    };
    use crate::persistence::workspace::WorkspaceDatabase;

    const PROJECT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const GROUP: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
    const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";

    fn paths(temp: &tempfile::TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn terminal_id(text: &str) -> TerminalId {
        TerminalId::from_str(text).unwrap()
    }

    fn record(text: &str, number: u64) -> TerminalRecord {
        TerminalRecord::new(
            terminal_id(text),
            ProjectId::from_str(PROJECT).unwrap(),
            number,
            CanonicalPath::new("D:/work").unwrap(),
            CanonicalPath::new("C:/shell/pwsh.exe").unwrap(),
            TerminalMetadata::new(None, None),
            RunIntent::Launch,
        )
        .unwrap()
    }

    fn snapshot() -> ProjectSnapshot {
        let project_id = ProjectId::from_str(PROJECT).unwrap();
        let views = [TERMINAL_A, TERMINAL_B]
            .into_iter()
            .map(|terminal| ViewKey::Terminal {
                project_id,
                terminal_id: terminal_id(terminal),
            })
            .collect::<Vec<_>>();
        let layout = ProjectLayout::new(
            project_id,
            PanelGroupId::from_str(GROUP).unwrap(),
            PanelGroup::new(views.clone(), views[0].clone()).unwrap(),
        )
        .unwrap();
        let registry = ProjectRegistry::restore(
            vec![
                Project::restore(
                    project_id,
                    CanonicalPath::new("D:/work").unwrap(),
                    "work".to_owned(),
                    layout,
                    ProjectRootStatus::Ready,
                    2,
                )
                .unwrap(),
            ],
            Some(project_id),
        )
        .unwrap();
        ProjectSnapshot::restore(registry, vec![record(TERMINAL_A, 0), record(TERMINAL_B, 1)])
            .unwrap()
    }

    struct RecordingSnapshots {
        observed: Vec<ProjectSnapshot>,
        fail: bool,
    }

    impl SnapshotRecoverer for RecordingSnapshots {
        fn recover(
            &mut self,
            layout: &ProjectSnapshot,
        ) -> Result<(), super::SnapshotRecoveryError> {
            self.observed.push(layout.clone());
            if self.fail {
                Err(super::SnapshotRecoveryError::Failed)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn workspace_startup_layout_before_snapshots() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&paths(&temp)).unwrap();
        store.save_project_snapshot(&snapshot()).unwrap();
        store
            .database
            .connection
            .execute("DELETE FROM terminals WHERE terminal_id = ?1", [TERMINAL_A])
            .unwrap();
        let mut snapshots = RecordingSnapshots {
            observed: Vec::new(),
            fail: false,
        };

        let recovered = StartupRecovery::recover(&mut store, Some(&mut snapshots)).unwrap();

        assert_eq!(recovered.terminals().count(), 1);
        assert_eq!(
            recovered.registry().projects()[0].layout().view_keys(),
            vec![ViewKey::Terminal {
                project_id: ProjectId::from_str(PROJECT).unwrap(),
                terminal_id: terminal_id(TERMINAL_B),
            }]
        );
        assert_eq!(snapshots.observed, vec![recovered]);
    }

    #[test]
    fn snapshot_recovery_error_is_not_reported_as_ready() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&paths(&temp)).unwrap();
        store.save_project_snapshot(&snapshot()).unwrap();
        let mut snapshots = RecordingSnapshots {
            observed: Vec::new(),
            fail: true,
        };

        let result = StartupRecovery::recover(&mut store, Some(&mut snapshots));

        assert!(matches!(
            result,
            Err(super::StartupRecoveryError::Snapshot(
                super::SnapshotRecoveryError::Failed
            ))
        ));
        assert_eq!(snapshots.observed.len(), 1);
    }

    #[test]
    fn workspace_valid_disaster_copy_is_restored_before_isolation() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let expected = snapshot();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store.save_project_snapshot(&expected).unwrap();
        store
            .backup_to_new(&paths.workspace_disaster_copy_path())
            .unwrap();
        drop(store);
        let corrupt = b"corrupt-main-workspace";
        std::fs::write(paths.workspace_db_path(), corrupt).unwrap();

        let mut opened = open_workspace_with_recovery(&paths).unwrap();

        assert_eq!(
            opened.disposition,
            WorkspaceOpenDisposition::RestoredFromBackup
        );
        assert!(matches!(
            opened.notice(),
            Some(super::WorkspaceRecoveryNotice::RestoredFromBackup)
        ));
        assert_eq!(opened.store.load_project_snapshot().unwrap(), expected);
        assert!(opened.quarantine_path().is_some());
        assert_eq!(
            std::fs::read(opened.quarantine_path().unwrap().join("workspace.db")).unwrap(),
            corrupt
        );
        assert!(paths.workspace_disaster_copy_path().exists());
    }

    /// 主库损坏但灾难副本带有效 WAL 时，必须先经副本恢复而不是隔离为双损。
    #[test]
    fn workspace_valid_wal_backup_is_restored_after_main_corruption() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;
        connection
            .pragma_update(None, "journal_mode", "wal")
            .unwrap();
        connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        connection
            .execute(
                "INSERT INTO workspace_meta \
                 (singleton, store_epoch, run_id, clean_shutdown, schema_version, process_id, process_created_at_ms) \
                 VALUES (1, 'epoch-wal', 'run-wal', 1, 2, 7, 8)",
                [],
            )
            .unwrap();
        // 连接保持打开时复制主库与 WAL：副本必须包含尚未 checkpoint 的提交。
        let backup = paths.workspace_disaster_copy_path();
        let wal = |path: &Path| PathBuf::from(format!("{}-wal", path.display()));
        std::fs::copy(paths.workspace_db_path(), &backup).unwrap();
        if wal(&paths.workspace_db_path()).exists() {
            std::fs::copy(wal(&paths.workspace_db_path()), wal(&backup)).unwrap();
        }
        drop(store);
        let corrupt = b"corrupt-main-workspace";
        std::fs::write(paths.workspace_db_path(), corrupt).unwrap();

        let opened = open_workspace_with_recovery(&paths).unwrap();

        assert_eq!(
            opened.disposition,
            WorkspaceOpenDisposition::RestoredFromBackup
        );
        assert_eq!(
            opened.store.metadata().unwrap().unwrap().store_epoch,
            "epoch-wal",
            "副本 WAL 中的提交必须被恢复"
        );
        assert_eq!(
            std::fs::read(opened.quarantine_path().unwrap().join("workspace.db")).unwrap(),
            corrupt,
            "损坏现场必须完整保留"
        );
    }

    /// 双损隔离改名中途失败后保留原位与标记；解除冲突后可重试完成。
    #[test]
    fn interrupted_isolation_rename_failure_is_retryable() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        let quarantine = paths
            .local_root()
            .join("quarantine")
            .join("workspace-retry");
        std::fs::create_dir_all(quarantine.join("workspace.db")).unwrap();
        std::fs::write(quarantine.join(super::ISOLATION_MARKER), b"").unwrap();
        let main = b"corrupt-main-workspace";
        std::fs::write(paths.workspace_db_path(), main).unwrap();

        assert!(
            open_workspace_with_recovery(&paths).is_err(),
            "原位与隔离目录冲突必须阻断"
        );
        assert_eq!(
            std::fs::read(paths.workspace_db_path()).unwrap(),
            main,
            "失败不得丢失原位材料"
        );
        assert!(quarantine.join(super::ISOLATION_MARKER).exists());

        std::fs::remove_dir(quarantine.join("workspace.db")).unwrap();
        let mut opened = open_workspace_with_recovery(&paths).unwrap();
        assert_eq!(opened.disposition, WorkspaceOpenDisposition::Isolated);
        assert!(!quarantine.join(super::ISOLATION_MARKER).exists());
        assert_eq!(
            std::fs::read(quarantine.join("workspace.db")).unwrap(),
            main
        );
        assert_eq!(
            opened.store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
    }

    /// 隔离改名中途崩溃后，重启先完成改名再建空库，不把中间态当首次使用。
    #[test]
    fn interrupted_isolation_is_completed_before_opening_the_empty_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        let main = b"corrupt-main-workspace";
        let backup = b"corrupt-backup-workspace";
        let quarantine = paths
            .local_root()
            .join("quarantine")
            .join("workspace-interrupted");
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::write(quarantine.join(super::ISOLATION_MARKER), b"").unwrap();
        // 模拟只完成了主库改名。
        std::fs::write(quarantine.join("workspace.db"), main).unwrap();
        std::fs::write(paths.workspace_disaster_copy_path(), backup).unwrap();

        let mut opened = open_workspace_with_recovery(&paths).unwrap();

        assert_eq!(opened.disposition, WorkspaceOpenDisposition::Isolated);
        assert!(
            !quarantine.join(super::ISOLATION_MARKER).exists(),
            "完成隔离后必须移除标记"
        );
        assert!(
            paths.workspace_db_path().exists(),
            "完成后应创建新的空工作区库"
        );
        assert!(!paths.workspace_disaster_copy_path().exists());
        assert_eq!(
            std::fs::read(quarantine.join("workspace.db")).unwrap(),
            main
        );
        assert_eq!(
            std::fs::read(quarantine.join("workspace.backup.db")).unwrap(),
            backup
        );
        assert_eq!(
            opened.store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
    }

    /// 原位与隔离副本同时存在是矛盾中间态，必须阻断而不是静默覆盖。
    #[test]
    fn interrupted_isolation_with_duplicate_files_blocks_startup() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        let quarantine = paths
            .local_root()
            .join("quarantine")
            .join("workspace-duplicate");
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::write(quarantine.join(super::ISOLATION_MARKER), b"").unwrap();
        std::fs::write(quarantine.join("workspace.db"), b"copy").unwrap();
        std::fs::write(paths.workspace_db_path(), b"source").unwrap();

        assert!(open_workspace_with_recovery(&paths).is_err());
        assert_eq!(std::fs::read(paths.workspace_db_path()).unwrap(), b"source");
    }

    #[test]
    fn workspace_missing_disaster_copy_is_not_a_valid_empty_backup() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        std::fs::write(paths.workspace_db_path(), b"corrupt-main").unwrap();

        let opened = open_workspace_with_recovery(&paths).unwrap();

        assert_eq!(opened.disposition, WorkspaceOpenDisposition::Isolated);
        assert!(!paths.workspace_disaster_copy_path().exists());
    }

    #[test]
    fn workspace_unrecoverable_isolated() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        let main = b"corrupt-main-workspace";
        let backup = b"corrupt-backup-workspace";
        std::fs::write(paths.workspace_db_path(), main).unwrap();
        std::fs::write(paths.workspace_disaster_copy_path(), backup).unwrap();

        let mut opened = open_workspace_with_recovery(&paths).unwrap();

        assert_eq!(opened.disposition, WorkspaceOpenDisposition::Isolated);
        assert!(matches!(
            opened.notice(),
            Some(super::WorkspaceRecoveryNotice::Isolated)
        ));
        assert_eq!(
            opened.store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
        assert_eq!(
            std::fs::read(opened.quarantine_path().unwrap().join("workspace.db")).unwrap(),
            main
        );
        assert_eq!(
            std::fs::read(
                opened
                    .quarantine_path()
                    .unwrap()
                    .join("workspace.backup.db")
            )
            .unwrap(),
            backup
        );
        drop(opened);
        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            reopened.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
    }
}
