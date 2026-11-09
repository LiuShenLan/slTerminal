//! 关闭意图提交边界的真实子进程崩溃恢复测试。

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use slterm_path::AppPaths;
    use slterm_workspace::{CloseIntent, CloseScope};

    use crate::app_shell::WorkspaceWriterRuntime;
    use crate::persistence::close::{CloseCommitStatus, PersistedCloseIntent};
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::startup::{
        JournalRecovererRegistry, MaintenanceOutcome, MaintenanceRunner, MaintenanceStage,
    };
    use crate::persistence::workspace::WorkspaceDatabase;

    const ROOT_ENV: &str = "SLTERM_TEST_CRASH_ROOT";
    const TARGET_ENV: &str = "SLTERM_TEST_CRASH_TARGET";
    const CHILD_TEST: &str = "app_shell::crash_recovery::tests::crash_child_close_commit_boundary";

    fn target_status(name: &str) -> CloseCommitStatus {
        match name {
            "state_committed" => CloseCommitStatus::StateCommitted,
            "pty_stopped" => CloseCommitStatus::PtyStopped,
            "cleanup_committed" => CloseCommitStatus::CleanupCommitted,
            "completed" => CloseCommitStatus::Completed,
            _ => panic!("未知崩溃目标: {name}"),
        }
    }

    /// 环境变量缺失时只是父测试的普通同进程占位，不产生任何写盘。
    #[test]
    fn crash_child_close_commit_boundary() {
        let Some(root) = std::env::var_os(ROOT_ENV) else {
            return;
        };
        let target = target_status(&std::env::var(TARGET_ENV).unwrap());
        let paths = AppPaths::new(
            std::path::PathBuf::from(&root).join("config"),
            std::path::PathBuf::from(&root).join("local"),
        )
        .unwrap();
        let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let registration = PersistedCloseIntent {
            close_id: "crash-close".to_owned(),
            scope: CloseScope::ApplicationExit,
            store_epoch: runtime.metadata().store_epoch.clone(),
            intent: CloseIntent::new(Vec::new(), [41; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        };
        let mut current = client
            .commit_close_intent(&slterm_workspace::ProjectSnapshot::default(), &registration)
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        for next in [
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            CloseCommitStatus::Completed,
        ] {
            if current.commit_status == target {
                // 已收到真实 SQLite 提交回执；跳过全部析构与正常退出链。
                std::process::exit(0);
            }
            current = client
                .advance_close_intent(&current, next)
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
        }
        assert_eq!(current.commit_status, target);
        std::process::exit(0);
    }

    #[test]
    fn close_commit_crash_reopens_with_exact_persistent_stage() {
        for target in [
            "state_committed",
            "pty_stopped",
            "cleanup_committed",
            "completed",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--test",
                    CHILD_TEST,
                    "--exact",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env(ROOT_ENV, temp.path())
                .env(TARGET_ENV, target)
                .status()
                .unwrap();
            assert!(status.success(), "崩溃子进程在 {target} 非正常退出");

            let mut store = WorkspaceDatabase::open(&paths).unwrap();
            let persisted = store.load_close_intent("crash-close").unwrap().unwrap();
            assert_eq!(persisted.commit_status, target_status(target));
            let run = store
                .begin_run(
                    &uuid::Uuid::now_v7().to_string(),
                    RunMarker {
                        run_id: uuid::Uuid::now_v7().to_string(),
                        process_id: std::process::id(),
                        process_created_at_ms:
                            crate::platform::update_installation::current_process_created_at_ms()
                                .unwrap(),
                    },
                )
                .unwrap();
            assert!(!run.previous.unwrap().clean_shutdown);
            let maintenance = MaintenanceRunner::new(
                &mut store,
                &run.current.store_epoch,
                &JournalRecovererRegistry::default(),
            )
            .run()
            .unwrap();
            if target == "completed" {
                assert_eq!(maintenance, MaintenanceOutcome::Ready);
            } else {
                assert_eq!(
                    maintenance,
                    MaintenanceOutcome::NeedsAttention {
                        stage: MaintenanceStage::CloseIntents
                    }
                );
            }
        }
    }
}
