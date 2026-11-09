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
    match WorkspaceDatabase::open(paths) {
        Ok(store) => {
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
    Ok(RecoveredWorkspace {
        store: WorkspaceDatabase::open(paths)?,
        disposition: WorkspaceOpenDisposition::Isolated,
        quarantine_path: Some(quarantine_path),
    })
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
    for source in sources {
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
    use std::str::FromStr;

    use slterm_path::{AppPaths, CanonicalPath};
    use slterm_workspace::{
        PanelGroup, PanelGroupId, Project, ProjectId, ProjectLayout, ProjectRegistry,
        ProjectRootStatus, ProjectSnapshot, RunIntent, TerminalId, TerminalRecord, ViewKey,
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
            None,
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
