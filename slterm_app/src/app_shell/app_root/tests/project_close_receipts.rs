//! 项目关闭回执身份、任务所有权与未知结果测试。

use super::super::task_service::TaskId;
use super::super::terminal_close::PtyStopObservation;
use super::close::{flush_when_ready, wait_for_close_status};
use super::project_close::remove_project;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{CloseAttempt, ClosePhase, CloseStage};

#[gpui::test]
fn project_join_panic_requires_explicit_retry_without_second_join(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    let panel_b = new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    flush_when_ready(&runtime.client());

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    let close_id = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .persisted
            .close_id
            .clone()
    });
    let committed = wait_for_close_status(&paths, &close_id, CloseCommitStatus::StateCommitted);
    let attempt = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .coordinator
            .current_attempt()
            .unwrap()
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_project_commit(
                project_id(PROJECT_B),
                attempt,
                Ok(Ok(committed)),
                window,
                cx,
            );
        });
    });
    let stop_attempt = root.read_with(cx, |root, cx| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::StopPty)
        );
        assert!(
            close
                .project
                .read(cx)
                .panels
                .values()
                .all(|panel| { panel.terminal.read(cx).session.is_none() })
        );
        close.coordinator.current_attempt().unwrap()
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_project_stop(
                project_id(PROJECT_B),
                stop_attempt,
                PtyStopObservation::Panicked,
                cx,
            );
        });
    });
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert!(!close.outcome_unknown);
    });
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    root.read_with(cx, |root, _| assert!(root.project_closes.is_empty()));
    let retry_message = state.borrow().sessions[&panel_b.sequence()].try_recv();
    assert!(matches!(
        retry_message,
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn project_unknown_commit_is_fail_closed_and_drop_cannot_delete_project(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    flush_when_ready(&runtime.client());

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    let attempt = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .coordinator
            .current_attempt()
            .unwrap()
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_project_commit(
                project_id(PROJECT_B),
                attempt,
                Err(std::sync::mpsc::RecvError),
                window,
                cx,
            );
        });
    });

    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
        assert!(
            !root
                .tasks
                .is_registered(TaskId::ProjectClose(project_id(PROJECT_B))),
            "有效断连回执只收割已结束任务"
        );
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_B))
                .is_some()
        );
    });
    assert!(matches!(
        remove_project(&root, &mut window, PROJECT_B),
        Err(AppRootError::ProjectCloseOutcomeUnknown)
    ));
    root.read_with(cx, |root, _| {
        assert!(
            !root
                .tasks
                .is_registered(TaskId::ProjectClose(project_id(PROJECT_B))),
            "未知结果不得盲目重登记"
        );
    });
    let blocked =
        window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));
    assert!(!blocked);

    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_some()
    );
}

#[gpui::test]
fn stale_project_receipt_preserves_current_attempt_and_task(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    flush_when_ready(&runtime.client());
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_project_advance BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected project advance failure'); END;",
        )
        .unwrap();

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.project_closes[&project_id(PROJECT_B)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
    });
    fault
        .execute_batch("DROP TRIGGER reject_project_advance")
        .unwrap();
    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    let (stale, current, stale_result) = root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        let current = close.coordinator.current_attempt().unwrap();
        let mut stale_result = close.persisted.clone();
        stale_result.commit_status = CloseCommitStatus::PtyStopped;
        (
            CloseAttempt::new(CloseStage::StopPty, current.sequence() - 1),
            current,
            stale_result,
        )
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_project_advance(
                project_id(PROJECT_B),
                stale,
                CloseCommitStatus::PtyStopped,
                Ok(Ok(stale_result)),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        assert_eq!(close.coordinator.current_attempt(), Some(current));
        assert_eq!(
            close.persisted.commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert!(
            root.tasks
                .is_registered(TaskId::ProjectClose(project_id(PROJECT_B)))
        );
    });
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    root.read_with(cx, |root, _| assert!(root.project_closes.is_empty()));
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_none()
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn wrong_project_commit_identity_cannot_harvest_current_task(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    flush_when_ready(&runtime.client());

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    let (attempt, wrong, identity) = root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        let mut wrong = close.persisted.clone();
        wrong.close_id = "wrong-project-close".to_owned();
        wrong.commit_status = CloseCommitStatus::StateCommitted;
        (
            close.coordinator.current_attempt().unwrap(),
            wrong,
            root.tasks
                .task_identity(TaskId::ProjectClose(project_id(PROJECT_B))),
        )
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_project_commit(project_id(PROJECT_B), attempt, Ok(Ok(wrong)), window, cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.project_closes[&project_id(PROJECT_B)]
                .coordinator
                .phase(),
            ClosePhase::Running(CloseStage::CommitState)
        );
        assert_eq!(
            root.tasks
                .task_identity(TaskId::ProjectClose(project_id(PROJECT_B))),
            identity,
            "错身份回执不得收割当前任务"
        );
    });
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    root.read_with(cx, |root, _| assert!(root.project_closes.is_empty()));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
