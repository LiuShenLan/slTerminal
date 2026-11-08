//! 应用退出回执身份、持久前序、任务所有权与多会话回收测试。

use super::super::task_service::TaskId;
use super::close::flush_when_ready;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{CloseAttempt, ClosePhase, CloseStage};

fn application_advance_failure_retries(
    cx: &mut TestAppContext,
    rejected: CloseCommitStatus,
    previous: CloseCommitStatus,
    failed_stage: CloseStage,
) {
    let rejected_name = match rejected {
        CloseCommitStatus::PtyStopped => "pty_stopped",
        CloseCommitStatus::CleanupCommitted => "cleanup_committed",
        CloseCommitStatus::Completed => "completed",
        _ => unreachable!("测试只覆盖停止后的持久推进"),
    };
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(&format!(
            "CREATE TRIGGER reject_application_advance BEFORE UPDATE OF commit_status \
             ON close_intents WHEN NEW.commit_status = '{rejected_name}' \
             BEGIN SELECT RAISE(ABORT, 'injected application advance failure'); END;"
        ))
        .unwrap();

    assert!(!window.simulate_close());
    let close_id = root.read_with(cx, |root, _| {
        root.application_close
            .as_ref()
            .unwrap()
            .persisted
            .close_id
            .clone()
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert_eq!(close.coordinator.phase(), ClosePhase::Failed(failed_stage));
        assert_eq!(close.persisted.commit_status, previous);
        assert!(!close.outcome_unknown);
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    let command = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::NewTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(matches!(command, Err(AppRootError::Closing)));
    let (completion, handover) = std::sync::mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::Handover {
                    params: slterm_runtime_protocol::HandoverParams::Focus,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    let handover = handover.try_recv().unwrap().unwrap_err();
    assert_eq!(handover.code, -32004);
    assert_eq!(
        handover.data.as_ref().map(|data| data.code),
        Some(slterm_runtime_protocol::DomainErrorCode::Closing)
    );
    fault
        .execute_batch("DROP TRIGGER reject_application_advance")
        .unwrap();

    assert!(!window.simulate_close());
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert!(root.close_completed);
        assert_eq!(
            root.application_close.as_ref().unwrap().coordinator.phase(),
            ClosePhase::Running(CloseStage::StopServices)
        );
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    assert_eq!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_close_intent(&close_id)
            .unwrap()
            .unwrap()
            .commit_status,
        CloseCommitStatus::Completed
    );
}

#[gpui::test]
fn application_pty_stopped_failure_retries_from_state_committed(cx: &mut TestAppContext) {
    application_advance_failure_retries(
        cx,
        CloseCommitStatus::PtyStopped,
        CloseCommitStatus::StateCommitted,
        CloseStage::StopPty,
    );
}

#[gpui::test]
fn application_cleanup_failure_retries_from_pty_stopped(cx: &mut TestAppContext) {
    application_advance_failure_retries(
        cx,
        CloseCommitStatus::CleanupCommitted,
        CloseCommitStatus::PtyStopped,
        CloseStage::Cleanup,
    );
}

#[gpui::test]
fn application_completed_failure_retries_from_cleanup_committed(cx: &mut TestAppContext) {
    application_advance_failure_retries(
        cx,
        CloseCommitStatus::Completed,
        CloseCommitStatus::CleanupCommitted,
        CloseStage::Cleanup,
    );
}

#[gpui::test]
fn application_disconnected_commit_is_fail_closed_without_reregistration(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    assert!(!window.simulate_close());
    let attempt = root.read_with(cx, |root, _| {
        root.application_close
            .as_ref()
            .unwrap()
            .coordinator
            .current_attempt()
            .unwrap()
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_application_commit(attempt, Err(std::sync::mpsc::RecvError), cx);
        });
    });
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
        assert!(!root.tasks.is_registered(TaskId::CloseFlush));
    });
    assert!(!window.simulate_close());
    root.read_with(cx, |root, _| {
        assert!(root.application_close.as_ref().unwrap().outcome_unknown);
        assert!(!root.tasks.is_registered(TaskId::CloseFlush));
        assert!(!root.close_completed);
    });
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn application_duplicate_advance_is_fail_closed_after_original_receipt(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_application_advance BEFORE UPDATE OF commit_status \
             ON close_intents WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected application advance failure'); END;",
        )
        .unwrap();
    assert!(!window.simulate_close());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_application_advance")
        .unwrap();

    assert!(!window.simulate_close());
    let original = root.read_with(cx, |root, _| {
        root.tasks.task_identity(TaskId::CloseFlush).unwrap()
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.start_application_advance(CloseCommitStatus::PtyStopped, cx);
        });
    });
    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(root.tasks.task_identity(TaskId::CloseFlush), Some(original));
    });
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert!(!root.close_completed);
    });
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn application_advance_receipt_validates_identity_attempt_next_and_previous(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_application_advance BEFORE UPDATE OF commit_status \
             ON close_intents WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected application advance failure'); END;",
        )
        .unwrap();
    assert!(!window.simulate_close());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_application_advance")
        .unwrap();
    assert!(!window.simulate_close());

    let (original, attempt, current) = root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        (
            root.tasks.task_identity(TaskId::CloseFlush).unwrap(),
            close.coordinator.current_attempt().unwrap(),
            close.persisted.clone(),
        )
    });
    let mut wrong_id = current.clone();
    wrong_id.close_id = "wrong-application-close".to_owned();
    wrong_id.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_epoch = current.clone();
    wrong_epoch.store_epoch = "wrong-epoch".to_owned();
    wrong_epoch.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_intent = current.clone();
    wrong_intent.intent = slterm_workspace::CloseIntent::new(
        wrong_intent.intent.objects().cloned().collect(),
        [93; 32],
    )
    .unwrap();
    wrong_intent.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_status = current.clone();
    wrong_status.commit_status = CloseCommitStatus::CleanupCommitted;
    let mut wrong_next = current.clone();
    wrong_next.commit_status = CloseCommitStatus::Completed;
    let mut valid = current;
    valid.commit_status = CloseCommitStatus::PtyStopped;
    let stale = CloseAttempt::new(CloseStage::StopPty, attempt.sequence() - 1);

    for (receipt_attempt, next, invalid) in [
        (attempt, CloseCommitStatus::PtyStopped, wrong_id),
        (attempt, CloseCommitStatus::PtyStopped, wrong_epoch),
        (attempt, CloseCommitStatus::PtyStopped, wrong_intent),
        (attempt, CloseCommitStatus::PtyStopped, wrong_status),
        (attempt, CloseCommitStatus::Completed, wrong_next),
        (stale, CloseCommitStatus::PtyStopped, valid),
    ] {
        window.update(|_, cx| {
            root.update(cx, |root, cx| {
                root.finish_application_advance(receipt_attempt, next, Ok(Ok(invalid)), cx);
            });
        });
        root.read_with(cx, |root, _| {
            let close = root.application_close.as_ref().unwrap();
            assert_eq!(close.coordinator.current_attempt(), Some(attempt));
            assert_eq!(
                close.persisted.commit_status,
                CloseCommitStatus::StateCommitted
            );
            assert_eq!(root.tasks.task_identity(TaskId::CloseFlush), Some(original));
        });
    }
    window.run_until_parked();
    root.read_with(cx, |root, _| assert!(root.close_completed));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn application_commit_receipt_with_wrong_identity_is_ignored(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    assert!(!window.simulate_close());
    let (original, attempt, wrong) = root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        let mut wrong = close.persisted.clone();
        wrong.close_id = "wrong-application-close".to_owned();
        wrong.commit_status = CloseCommitStatus::StateCommitted;
        (
            root.tasks.task_identity(TaskId::CloseFlush).unwrap(),
            close.coordinator.current_attempt().unwrap(),
            wrong,
        )
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_application_commit(attempt, Ok(Ok(wrong)), cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.application_close.as_ref().unwrap().coordinator.phase(),
            ClosePhase::Running(CloseStage::CommitState)
        );
        assert_eq!(root.tasks.task_identity(TaskId::CloseFlush), Some(original));
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| assert!(root.close_completed));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn application_global_receipt_validates_identity_attempt_and_disconnect(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let window_handle = window.update(|window, _| window.window_handle());
    let attempt = window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let mut coordinator = slterm_workspace::CloseCoordinator::new();
            coordinator
                .begin(slterm_workspace::CloseScope::ApplicationExit)
                .unwrap();
            let attempt = coordinator.current_attempt().unwrap();
            let intent = slterm_workspace::CloseIntent::new(Vec::new(), [7; 32]).unwrap();
            coordinator.collected(attempt, intent.clone()).unwrap();
            coordinator
                .state_committed(coordinator.current_attempt().unwrap())
                .unwrap();
            coordinator
                .pty_stopped(coordinator.current_attempt().unwrap())
                .unwrap();
            coordinator
                .recovery_cleaned(coordinator.current_attempt().unwrap())
                .unwrap();
            let attempt = coordinator.current_attempt().unwrap();
            root.application_close = Some(super::super::application_close::ApplicationClose {
                coordinator,
                persisted: crate::persistence::close::PersistedCloseIntent {
                    close_id: "application-global-receipt".to_owned(),
                    scope: slterm_workspace::CloseScope::ApplicationExit,
                    store_epoch: "test-epoch".to_owned(),
                    intent,
                    commit_status: CloseCommitStatus::Completed,
                },
                stop_observation: Some(super::super::terminal_close::PtyStopObservation::Stopped),
                outcome_unknown: false,
                window: window_handle,
            });
            root.close_in_progress = true;
            let task = cx.spawn(async move |_, _| std::future::pending::<()>().await);
            root.tasks
                .register(
                    TaskId::CloseFlush,
                    super::super::task_service::CancellationToken::new(),
                    task,
                )
                .unwrap();
            attempt
        })
    });
    let original = root.read_with(cx, |root, _| {
        root.tasks.task_identity(TaskId::CloseFlush).unwrap()
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_application_global(
                "wrong-application-close".to_owned(),
                attempt,
                Ok(Ok(crate::app_shell::FinalShutdownOutcome::Completed)),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(root.tasks.task_identity(TaskId::CloseFlush), Some(original));
        assert!(!root.close_completed);
    });
    let stale = CloseAttempt::new(CloseStage::StopServices, attempt.sequence() - 1);
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_application_global(
                "application-global-receipt".to_owned(),
                stale,
                Ok(Ok(crate::app_shell::FinalShutdownOutcome::Completed)),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(root.tasks.task_identity(TaskId::CloseFlush), Some(original));
        assert!(!root.close_completed);
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_application_global(
                "application-global-receipt".to_owned(),
                attempt,
                Err(std::sync::mpsc::RecvError),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopServices)
        );
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
        assert!(!root.tasks.is_registered(TaskId::CloseFlush));
    });
}

#[gpui::test]
fn application_exit_stops_visible_and_hidden_project_sessions(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel_a = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    let panel_b = new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    flush_when_ready(&runtime.client());

    assert!(!window.simulate_close());
    let close_id = root.read_with(cx, |root, _| {
        root.application_close
            .as_ref()
            .unwrap()
            .persisted
            .close_id
            .clone()
    });
    window.run_until_parked();

    for panel in [panel_a, panel_b] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    root.read_with(cx, |root, _| assert!(root.close_completed));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    let completed = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_close_intent(&close_id)
        .unwrap()
        .unwrap();
    assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
    assert_eq!(completed.intent.objects().count(), 2);
}

#[gpui::test]
fn application_disconnected_global_worker_is_fail_closed(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let client = crate::app_shell::global_close::GlobalCloseClient::disconnected_for_test();
    let (root, mut window, _state) = open_root_with_persistence_and_global(cx, persistence, client);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    flush_when_ready(&runtime.client());

    assert!(!window.simulate_close());
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopServices)
        );
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
        assert!(!root.tasks.is_registered(TaskId::CloseFlush));
    });
    assert!(!window.simulate_close());
    root.read_with(cx, |root, _| {
        let close = root.application_close.as_ref().unwrap();
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopServices)
        );
        assert!(!root.close_completed);
    });
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn recovery_disconnected_global_worker_is_fail_closed(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut original = ProjectSnapshot::default();
    original
        .registry_mut()
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &original);
    rusqlite::Connection::open(paths.workspace_db_path())
        .unwrap()
        .execute(
            "UPDATE project_layouts SET layout_json = 'not-json' WHERE project_id = ?1",
            [PROJECT_A],
        )
        .unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let client = crate::app_shell::global_close::GlobalCloseClient::disconnected_for_test();
    let (root, mut window, _state) = open_root_with_persistence_and_global(cx, persistence, client);

    assert!(!window.simulate_close());
    window.run_until_parked();
    let attempt = root.read_with(cx, |root, _| {
        let recovery = root.recovery_global_close.as_ref().unwrap();
        assert!(recovery.outcome_unknown);
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
        recovery.attempt
    });
    assert!(!window.simulate_close());
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.recovery_global_close.as_ref().unwrap().attempt,
            attempt
        );
        assert!(!root.close_completed);
    });
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn application_recovery_failure_flushes_without_close_intent(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut original = ProjectSnapshot::default();
    original
        .registry_mut()
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &original);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute(
            "UPDATE project_layouts SET layout_json = 'not-json' WHERE project_id = ?1",
            [PROJECT_A],
        )
        .unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    assert!(persistence.project_snapshot_recovery_failed());
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);

    assert!(!window.simulate_close());
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert!(root.close_completed);
        assert!(root.application_close.is_none());
    });
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    assert_eq!(
        fault
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

#[gpui::test]
fn application_recovery_failure_uses_global_close_without_intent(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut original = ProjectSnapshot::default();
    original
        .registry_mut()
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &original);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute(
            "UPDATE project_layouts SET layout_json = 'not-json' WHERE project_id = ?1",
            [PROJECT_A],
        )
        .unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    assert!(persistence.project_snapshot_recovery_failed());
    let global = crate::app_shell::global_close::GlobalCloseRuntime::start(runtime, None).unwrap();
    let (root, mut window, _state) =
        open_root_with_persistence_and_global(cx, persistence, global.client());

    assert!(!window.simulate_close());
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert!(root.close_completed);
        assert!(root.application_close.is_none());
        assert!(root.recovery_global_close.is_some());
    });
    assert!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .metadata()
            .unwrap()
            .unwrap()
            .clean_shutdown
    );
    assert_eq!(
        fault
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        global.join().unwrap(),
        crate::app_shell::global_close::GlobalCloseSummary::Completed
    );
}
