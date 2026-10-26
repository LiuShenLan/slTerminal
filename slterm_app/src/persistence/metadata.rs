//! 工作区运行标记与持久世代的权威存储。

use std::fmt;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

/// 一次进程运行的身份与 OS 核对材料。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunMarker {
    pub(crate) run_id: String,
    pub(crate) process_id: u32,
    pub(crate) process_created_at_ms: u64,
}

/// `workspace_meta` 的领域投影。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceMetadata {
    pub(crate) store_epoch: String,
    pub(crate) run: RunMarker,
    pub(crate) clean_shutdown: bool,
}

/// 启动结果同时保留上一运行事实与本次运行标记。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunStart {
    pub(crate) previous: Option<WorkspaceMetadata>,
    pub(crate) current: WorkspaceMetadata,
}

/// 运行标记或世代写入失败；不向展示层暴露 SQL 与路径。
#[derive(Debug)]
pub(crate) enum MetadataError {
    InvalidMetadata,
    RunConflict,
    StoreEpochConflict,
    Database(DatabaseError),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMetadata => "运行标记或持久世代无效",
            Self::RunConflict => "运行标记已变化",
            Self::StoreEpochConflict => "工作区世代已变化",
            Self::Database(error) => return fmt::Display::fmt(error, formatter),
        })
    }
}

impl std::error::Error for MetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DatabaseError> for MetadataError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for MetadataError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    /// 读取当前运行事实；更高版本库同样只允许这条只读路径。
    pub(crate) fn metadata(&self) -> Result<Option<WorkspaceMetadata>, MetadataError> {
        Ok(read_metadata(&self.database.connection)?)
    }

    /// 登记本次运行。已有库保留原世代，首次初始化才采用调用方给定的世代。
    pub(crate) fn begin_run(
        &mut self,
        initial_store_epoch: &str,
        run: RunMarker,
    ) -> Result<RunStart, MetadataError> {
        let process_created_at_ms = validate_run(&run)?;
        if initial_store_epoch.is_empty() {
            return Err(MetadataError::InvalidMetadata);
        }

        let schema_version = self.schema_version();
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = read_metadata(&transaction)?;
        let current = WorkspaceMetadata {
            store_epoch: previous.as_ref().map_or_else(
                || initial_store_epoch.to_owned(),
                |metadata| metadata.store_epoch.clone(),
            ),
            run,
            clean_shutdown: false,
        };
        if previous.is_some() {
            let changed = transaction.execute(
                "UPDATE workspace_meta \
                 SET run_id = ?1, clean_shutdown = 0, process_id = ?2, process_created_at_ms = ?3 \
                 WHERE singleton = 1",
                params![
                    &current.run.run_id,
                    i64::from(current.run.process_id),
                    process_created_at_ms
                ],
            )?;
            if changed != 1 {
                return Err(DatabaseError::StorageContract.into());
            }
        } else {
            transaction.execute(
                "INSERT INTO workspace_meta \
                 (singleton, store_epoch, run_id, clean_shutdown, schema_version, process_id, process_created_at_ms) \
                 VALUES (1, ?1, ?2, 0, ?3, ?4, ?5)",
                params![
                    &current.store_epoch,
                    &current.run.run_id,
                    i64::from(schema_version),
                    i64::from(current.run.process_id),
                    process_created_at_ms
                ],
            )?;
        }
        transaction.commit()?;
        Ok(RunStart { previous, current })
    }

    /// 只有当前运行能登记正常退出；旧运行的迟到关闭不得改新标记。
    pub(crate) fn mark_clean_shutdown(&mut self, run_id: &str) -> Result<(), MetadataError> {
        if run_id.is_empty() {
            return Err(MetadataError::InvalidMetadata);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE workspace_meta SET clean_shutdown = 1 WHERE singleton = 1 AND run_id = ?1",
            [run_id],
        )?;
        if changed != 1 {
            return Err(MetadataError::RunConflict);
        }
        transaction.commit()?;
        Ok(())
    }

    /// 显式维护提交后更换世代；当前运行与预期世代必须同时匹配。
    pub(crate) fn rotate_store_epoch(
        &mut self,
        run_id: &str,
        expected_store_epoch: &str,
        replacement_store_epoch: &str,
    ) -> Result<WorkspaceMetadata, MetadataError> {
        if run_id.is_empty()
            || expected_store_epoch.is_empty()
            || replacement_store_epoch.is_empty()
            || expected_store_epoch == replacement_store_epoch
        {
            return Err(MetadataError::InvalidMetadata);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_metadata(&transaction)?.ok_or(MetadataError::RunConflict)?;
        if current.run.run_id != run_id {
            return Err(MetadataError::RunConflict);
        }
        if current.store_epoch != expected_store_epoch {
            return Err(MetadataError::StoreEpochConflict);
        }
        let changed = transaction.execute(
            "UPDATE workspace_meta SET store_epoch = ?1 \
             WHERE singleton = 1 AND run_id = ?2 AND store_epoch = ?3",
            params![replacement_store_epoch, run_id, expected_store_epoch],
        )?;
        if changed != 1 {
            return Err(MetadataError::StoreEpochConflict);
        }
        transaction.commit()?;
        Ok(WorkspaceMetadata {
            store_epoch: replacement_store_epoch.to_owned(),
            ..current
        })
    }
}

fn validate_run(run: &RunMarker) -> Result<i64, MetadataError> {
    if run.run_id.is_empty() {
        return Err(MetadataError::InvalidMetadata);
    }
    i64::try_from(run.process_created_at_ms).map_err(|_| MetadataError::InvalidMetadata)
}

fn read_metadata(connection: &Connection) -> Result<Option<WorkspaceMetadata>, DatabaseError> {
    connection
        .query_row(
            "SELECT store_epoch, run_id, clean_shutdown, process_id, process_created_at_ms \
             FROM workspace_meta WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .map(decode_metadata)
        .transpose()
}

fn decode_metadata(
    raw: (String, String, i64, i64, i64),
) -> Result<WorkspaceMetadata, DatabaseError> {
    let (store_epoch, run_id, clean_shutdown, process_id, process_created_at_ms) = raw;
    if store_epoch.is_empty() || run_id.is_empty() {
        return Err(DatabaseError::StorageContract);
    }
    let clean_shutdown = match clean_shutdown {
        0 => false,
        1 => true,
        _ => return Err(DatabaseError::StorageContract),
    };
    Ok(WorkspaceMetadata {
        store_epoch,
        run: RunMarker {
            run_id,
            process_id: u32::try_from(process_id).map_err(|_| DatabaseError::StorageContract)?,
            process_created_at_ms: u64::try_from(process_created_at_ms)
                .map_err(|_| DatabaseError::StorageContract)?,
        },
        clean_shutdown,
    })
}

#[cfg(test)]
mod metadata_tests {
    use std::path::PathBuf;
    use std::process::Command;

    use rusqlite::Connection;
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::database::DatabaseAccess;

    const CRASH_ROOT: &str = "SLTERM_TEST_WORKSPACE_WAL_CRASH_ROOT";
    const CRASH_EXIT_CODE: i32 = 86;
    const CRASH_TEST: &str = "persistence::metadata::metadata_tests::workspace_wal_recovers_committed_run_after_process_exit";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn run(run_id: &str, process_id: u32) -> RunMarker {
        RunMarker {
            run_id: run_id.to_owned(),
            process_id,
            process_created_at_ms: 456,
        }
    }

    fn metadata(store_epoch: &str, run: RunMarker, clean_shutdown: bool) -> WorkspaceMetadata {
        WorkspaceMetadata {
            store_epoch: store_epoch.to_owned(),
            run,
            clean_shutdown,
        }
    }

    /// 正常重启只更换 run_id；上一正常与异常运行事实均可观察。
    #[test]
    fn run_markers_survive_reopen_and_normal_restart_preserves_epoch() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(store.metadata().unwrap(), None);

        let first = store.begin_run("epoch-a", run("run-a", 11)).unwrap();
        assert_eq!(first.previous, None);
        assert_eq!(first.current, metadata("epoch-a", run("run-a", 11), false));
        drop(store);

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            store.metadata().unwrap(),
            Some(metadata("epoch-a", run("run-a", 11), false))
        );
        store.mark_clean_shutdown("run-a").unwrap();
        assert_eq!(
            store.metadata().unwrap(),
            Some(metadata("epoch-a", run("run-a", 11), true))
        );

        let second = store.begin_run("epoch-b", run("run-b", 22)).unwrap();
        assert_eq!(second.current.store_epoch, "epoch-a");
        assert_eq!(second.current.run, run("run-b", 22));
        assert!(!second.current.clean_shutdown);
        assert_eq!(
            second.previous,
            Some(metadata("epoch-a", run("run-a", 11), true))
        );
        drop(store);

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let third = store.begin_run("epoch-c", run("run-c", 33)).unwrap();
        assert_eq!(third.current.store_epoch, "epoch-a");
        assert_eq!(
            third.previous,
            Some(metadata("epoch-a", run("run-b", 22), false))
        );
    }

    /// 旧运行、旧世代与同值更换均不得改写当前元数据。
    #[test]
    fn stale_run_and_epoch_preconditions_leave_metadata_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run("epoch-old", run("run-current", 11))
            .unwrap();
        let before = store.metadata().unwrap().unwrap();

        assert!(matches!(
            store.mark_clean_shutdown("run-stale"),
            Err(MetadataError::RunConflict)
        ));
        assert!(matches!(
            store.rotate_store_epoch("run-stale", "epoch-old", "epoch-new"),
            Err(MetadataError::RunConflict)
        ));
        assert!(matches!(
            store.rotate_store_epoch("run-current", "epoch-stale", "epoch-new"),
            Err(MetadataError::StoreEpochConflict)
        ));
        assert!(matches!(
            store.rotate_store_epoch("run-current", "epoch-old", "epoch-old"),
            Err(MetadataError::InvalidMetadata)
        ));
        assert_eq!(store.metadata().unwrap(), Some(before));

        let rotated = store
            .rotate_store_epoch("run-current", "epoch-old", "epoch-new")
            .unwrap();
        assert_eq!(rotated.store_epoch, "epoch-new");
        assert_eq!(rotated.run, run("run-current", 11));
        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(store.metadata().unwrap(), Some(rotated));
    }

    /// 空身份与无法无损进入 SQLite INTEGER 的时间不得部分登记。
    #[test]
    fn invalid_run_metadata_is_rejected_without_a_partial_row() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();

        for (epoch, run_id) in [("", "run-a"), ("epoch-a", "")] {
            assert!(matches!(
                store.begin_run(epoch, run(run_id, 1)),
                Err(MetadataError::InvalidMetadata)
            ));
        }
        assert!(matches!(
            store.begin_run(
                "epoch-a",
                RunMarker {
                    run_id: "run-a".into(),
                    process_id: 1,
                    process_created_at_ms: u64::MAX,
                },
            ),
            Err(MetadataError::InvalidMetadata)
        ));
        assert!(matches!(
            store.mark_clean_shutdown(""),
            Err(MetadataError::InvalidMetadata)
        ));
        assert!(matches!(
            store.rotate_store_epoch("", "epoch-a", "epoch-b"),
            Err(MetadataError::InvalidMetadata)
        ));
        assert!(matches!(
            store.rotate_store_epoch("run-a", "", "epoch-b"),
            Err(MetadataError::InvalidMetadata)
        ));
        assert!(matches!(
            store.rotate_store_epoch("run-a", "epoch-a", ""),
            Err(MetadataError::InvalidMetadata)
        ));
        assert_eq!(store.metadata().unwrap(), None);
    }

    /// 更高版本只允许读取运行事实，所有写入口都失败且不改代次。
    #[test]
    fn higher_version_metadata_is_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store.begin_run("epoch-a", run("run-a", 11)).unwrap();
        drop(store);

        let connection = Connection::open(paths.workspace_db_path()).unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        drop(connection);

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(store.access(), DatabaseAccess::ReadOnlyHigherVersion);
        assert_eq!(
            store.metadata().unwrap(),
            Some(metadata("epoch-a", run("run-a", 11), false))
        );
        assert!(store.begin_run("epoch-b", run("run-b", 22)).is_err());
        assert!(store.mark_clean_shutdown("run-a").is_err());
        assert!(
            store
                .rotate_store_epoch("run-a", "epoch-a", "epoch-b")
                .is_err()
        );
        assert_eq!(
            store.metadata().unwrap(),
            Some(metadata("epoch-a", run("run-a", 11), false))
        );
    }

    /// 子进程在事务提交后不析构直接退出；父进程必须从 WAL 读回已提交标记。
    #[test]
    fn workspace_wal_recovers_committed_run_after_process_exit() {
        if let Some(root) = std::env::var_os(CRASH_ROOT) {
            let root = PathBuf::from(root);
            let paths = AppPaths::new(root.join("config"), root.join("local")).unwrap();
            let mut store = WorkspaceDatabase::open(&paths).unwrap();
            store.begin_run("epoch-wal", run("run-wal", 44)).unwrap();
            std::process::exit(CRASH_EXIT_CODE);
        }

        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(CRASH_TEST)
            .arg("--test-threads=1")
            .env(CRASH_ROOT, temp.path())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(CRASH_EXIT_CODE),
            "崩溃子进程未到达提交后退出点\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let wal = paths.workspace_db_path().with_extension("db-wal");
        assert!(wal.exists(), "异常退出后必须保留待恢复 WAL");

        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            store.metadata().unwrap(),
            Some(metadata("epoch-wal", run("run-wal", 44), false))
        );
    }
}
