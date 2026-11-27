//! 显式放弃未完成启动材料的唯一入口：先整库归档，再按阶段删除阻断事实。

use std::fmt;
use std::path::PathBuf;

use rusqlite::TransactionBehavior;
use slterm_path::AppPaths;

use crate::persistence::database::DatabaseError;
use crate::persistence::startup::MaintenanceStage;
use crate::persistence::workspace::WorkspaceDatabase;

/// 放弃结果；归档库保留全部原始阻断事实供诊断，界面不展示路径。
#[derive(Debug)]
pub(crate) struct AbandonOutcome {
    pub(crate) archive_path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum AbandonError {
    Database(DatabaseError),
    Archive(std::io::Error),
    Store(rusqlite::Error),
    /// 领域查询失败；细节不进入展示。
    Query,
}

impl fmt::Display for AbandonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("放弃未完成启动材料失败")
    }
}

impl std::error::Error for AbandonError {}

impl From<DatabaseError> for AbandonError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for AbandonError {
    fn from(error: std::io::Error) -> Self {
        Self::Archive(error)
    }
}

impl From<rusqlite::Error> for AbandonError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error)
    }
}

fn stage_slug(stage: MaintenanceStage) -> &'static str {
    match stage {
        MaintenanceStage::Journal => "journal",
        MaintenanceStage::MaintenanceRequests => "maintenance_requests",
        MaintenanceStage::CloseIntents => "close_intents",
    }
}

/// 用户显式确认后的恢复动作：未完成状态被放弃，原库完整副本留在本地诊断目录。
pub(crate) fn abandon_maintenance_blockers(
    paths: &AppPaths,
    stage: MaintenanceStage,
) -> Result<AbandonOutcome, AbandonError> {
    let archive_dir = paths.local_root().join("recovery-abandon");
    crate::atomic_file::create_dir_current_user(&archive_dir)?;
    let archive_path = archive_dir.join(format!(
        "workspace-{}-{}.db",
        stage_slug(stage),
        uuid::Uuid::now_v7()
    ));
    let mut store = WorkspaceDatabase::open(paths).map_err(AbandonError::Database)?;
    // 先归档整库：任何删除之前的完整事实都可回溯，归档失败则零删除。
    if let Err(error) = store.backup_to_new(&archive_path) {
        crate::audit::record(crate::audit::AuditEvent::new(
            "maintenance_abandon",
            stage_slug(stage),
            crate::audit::AuditResult::Error,
        ));
        return Err(error.into());
    }
    let result = clear_stage(&mut store, stage);
    crate::audit::record(
        crate::audit::AuditEvent::new(
            "maintenance_abandon",
            stage_slug(stage),
            if result.is_ok() {
                crate::audit::AuditResult::Ok
            } else {
                crate::audit::AuditResult::Error
            },
        )
        .with_path(&archive_path),
    );
    result?;
    Ok(AbandonOutcome { archive_path })
}

fn clear_stage(store: &mut WorkspaceDatabase, stage: MaintenanceStage) -> Result<(), AbandonError> {
    // 先收集未完成身份，再开事务；事务期间不再读领域视图。
    let journal_ids = if stage == MaintenanceStage::Journal {
        store
            .unfinished_journals()
            .map_err(|_| AbandonError::Query)?
            .into_iter()
            .map(|journal| journal.journal_id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let request_ids = if stage == MaintenanceStage::MaintenanceRequests {
        store
            .unfinished_maintenance_requests()
            .map_err(|_| AbandonError::Query)?
            .into_iter()
            .map(|request| request.request_id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let close_ids = if stage == MaintenanceStage::CloseIntents {
        store
            .unfinished_close_intents()
            .map_err(|_| AbandonError::Query)?
            .into_iter()
            .map(|intent| intent.close_id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let transaction = store
        .database
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    match stage {
        MaintenanceStage::Journal => {
            for journal_id in journal_ids {
                // Journal 关联的 Operation 仍保留可查询记录，只解除指向被放弃的 Journal。
                transaction.execute(
                    "UPDATE runtime_operations SET journal_id = NULL WHERE journal_id = ?1",
                    [&journal_id],
                )?;
                transaction.execute(
                    "DELETE FROM journal_steps WHERE journal_id = ?1",
                    [&journal_id],
                )?;
                transaction.execute(
                    "DELETE FROM managed_journals WHERE journal_id = ?1",
                    [&journal_id],
                )?;
            }
        }
        MaintenanceStage::MaintenanceRequests => {
            for request_id in request_ids {
                transaction.execute(
                    "DELETE FROM maintenance_requests WHERE request_id = ?1",
                    [&request_id],
                )?;
            }
            transaction.execute("DELETE FROM recovery_points", [])?;
        }
        MaintenanceStage::CloseIntents => {
            for close_id in close_ids {
                transaction
                    .execute("DELETE FROM close_objects WHERE close_id = ?1", [&close_id])?;
                transaction
                    .execute("DELETE FROM close_intents WHERE close_id = ?1", [&close_id])?;
            }
        }
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use slterm_workspace::{CloseIntent, CloseScope};
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::close::{CloseCommitStatus, PersistedCloseIntent};
    use crate::persistence::journal::{JournalRegistration, JournalStepRegistration};
    use crate::persistence::maintenance_request::{
        MaintenanceRequest, MaintenanceRequestStatus, RecoveryPointRecord, RecoveryPointStatus,
    };
    use crate::persistence::metadata::RunMarker;

    const EPOCH: &str = "epoch-abandon";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn store(paths: &AppPaths) -> WorkspaceDatabase {
        let mut store = WorkspaceDatabase::open(paths).unwrap();
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: "run-abandon".into(),
                    process_id: 1,
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
        store
    }

    fn close_intent(id: &str) -> PersistedCloseIntent {
        PersistedCloseIntent {
            close_id: id.into(),
            scope: CloseScope::ApplicationExit,
            store_epoch: EPOCH.into(),
            intent: CloseIntent::new(Vec::new(), [4; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        }
    }

    #[test]
    fn close_intent_abandon_archives_whole_database_and_keeps_completed_history() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&paths);
        store
            .register_close_intent(&close_intent("close-unfinished"))
            .unwrap();
        let mut done = close_intent("close-done");
        store.register_close_intent(&done).unwrap();
        for next in [
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            CloseCommitStatus::Completed,
        ] {
            done = store
                .advance_close_intent(
                    &done.close_id,
                    done.commit_status,
                    next,
                    done.intent.fingerprint(),
                    EPOCH,
                )
                .unwrap();
        }
        drop(store);

        let outcome = abandon_maintenance_blockers(&paths, MaintenanceStage::CloseIntents).unwrap();

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.unfinished_close_intents().unwrap().is_empty());
        let remaining: i64 = store
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1, "Completed 关闭历史不得删除");
        let archived = rusqlite::Connection::open(&outcome.archive_path).unwrap();
        let archived_count: i64 = archived
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(archived_count, 2, "归档必须保留删除前的完整事实");
    }

    #[test]
    fn maintenance_request_abandon_clears_requests_and_recovery_points() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&paths);
        store
            .register_maintenance_request(&MaintenanceRequest {
                request_id: "request-a".into(),
                kind: "backup.restore".into(),
                status: MaintenanceRequestStatus::Prepared,
                staged_fingerprint: [1; 32],
                store_epoch: EPOCH.into(),
                staged_path: "maintenance/request-a/staged.bin".into(),
            })
            .unwrap();
        store
            .record_recovery_point(&RecoveryPointRecord {
                recovery_point_id: "request-a".into(),
                object_key: "settings/settings.json".into(),
                status: RecoveryPointStatus::Prepared,
                before_fingerprint: None,
                after_fingerprint: None,
                dpapi_path: "maintenance/request-a.point".into(),
            })
            .unwrap();
        drop(store);

        abandon_maintenance_blockers(&paths, MaintenanceStage::MaintenanceRequests).unwrap();

        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.unfinished_maintenance_requests().unwrap().is_empty());
        assert!(store.recovery_points("request-a").unwrap().is_empty());
    }

    #[test]
    fn journal_abandon_removes_unfinished_journal_and_steps() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&paths);
        store
            .register_journal(JournalRegistration {
                operation_id: None,
                journal_id: "journal-a".into(),
                kind: "test".into(),
                store_epoch: EPOCH.into(),
                before_fingerprint: Some([1; 32]),
                steps: vec![JournalStepRegistration {
                    step_key: "step-a".into(),
                    kind: "replace".into(),
                    target: json!({"version": 1}),
                    authorization_generation: 1,
                    before_fingerprint: Some([2; 32]),
                }],
            })
            .unwrap();
        drop(store);

        abandon_maintenance_blockers(&paths, MaintenanceStage::Journal).unwrap();

        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.unfinished_journals().unwrap().is_empty());
        let steps: i64 = store
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM journal_steps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(steps, 0);
    }
}
