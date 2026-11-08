//! 局部关闭回执身份、任务所有权与焦点作用域测试。

use super::super::task_service::TaskId;
use super::close::wait_for_close_status;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use gpui::Focusable;
use slterm_workspace::{CloseAttempt, ClosePhase, CloseStage};

#[gpui::test]
fn missing_close_receipts_never_touch_same_terminal_task(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let task = cx.spawn(async move |_, _| std::future::pending::<()>().await);
            root.tasks
                .register(
                    id,
                    super::super::task_service::CancellationToken::new(),
                    task,
                )
                .unwrap();
        });
    });
    let original = root.read_with(cx, |root, _| root.tasks.task_identity(id).unwrap());
    let attempt = CloseAttempt::new(CloseStage::Collect, 0);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_commit(
                terminal_id(TERMINAL_A),
                attempt,
                Err(std::sync::mpsc::RecvError),
                window.window_handle(),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(root.tasks.task_identity(id), Some(original));
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let payload: Box<dyn std::any::Any + Send> = Box::new("missing-close");
            root.finish_terminal_stop(terminal_id(TERMINAL_A), attempt, Err(payload), cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(root.tasks.task_identity(id), Some(original));
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_advance(
                terminal_id(TERMINAL_A),
                attempt,
                CloseCommitStatus::PtyStopped,
                Err(std::sync::mpsc::RecvError),
                cx,
            );
        });
    });

    root.read_with(cx, |root, cx| {
        assert_eq!(root.tasks.task_identity(id), Some(original));
        assert!(root.terminal_closes.is_empty());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_some());
    });
}

#[gpui::test]
fn duplicate_commit_registration_preserves_original_task_and_write_barrier(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let original = root.read_with(cx, |root, _| root.tasks.task_identity(task_id).unwrap());
    let candidate = root.read_with(cx, |root, _| {
        root.snapshot
            .without_terminal(terminal_id(TERMINAL_A))
            .unwrap()
            .0
    });

    let duplicate = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_terminal_commit(terminal_id(TERMINAL_A), candidate, window, cx)
        })
    });
    assert!(duplicate.is_ok());

    root.read_with(cx, |root, cx| {
        assert_eq!(root.tasks.task_identity(task_id), Some(original));
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_some());
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
    });
    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(
                terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        rejected,
        Err(AppRootError::ProjectSnapshotCommitInFlight)
    ));
}

#[gpui::test]
fn duplicate_advance_registration_preserves_original_task_and_stage(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_pty_stopped BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected pty status failure'); END;",
        )
        .unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_pty_stopped")
        .unwrap();
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok());
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let original = root.read_with(cx, |root, _| root.tasks.task_identity(task_id).unwrap());

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.start_terminal_advance(terminal_id(TERMINAL_A), CloseCommitStatus::PtyStopped, cx);
        });
    });

    root.read_with(cx, |root, cx| {
        assert_eq!(root.tasks.task_identity(task_id), Some(original));
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
}

#[gpui::test]
fn commit_receipt_with_wrong_identity_or_status_is_ignored(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let (original, attempt, registration) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        (
            root.tasks.task_identity(task_id).unwrap(),
            close.coordinator.current_attempt().unwrap(),
            close.persisted.clone().unwrap(),
        )
    });
    let mut wrong_id = registration.clone();
    wrong_id.close_id = "wrong-close-id".to_owned();
    wrong_id.commit_status = CloseCommitStatus::StateCommitted;
    let mut wrong_epoch = registration.clone();
    wrong_epoch.store_epoch = "wrong-epoch".to_owned();
    wrong_epoch.commit_status = CloseCommitStatus::StateCommitted;
    let mut wrong_intent = registration.clone();
    wrong_intent.intent = slterm_workspace::CloseIntent::new(
        wrong_intent.intent.objects().cloned().collect(),
        [91; 32],
    )
    .unwrap();
    wrong_intent.commit_status = CloseCommitStatus::StateCommitted;
    let mut wrong_status = registration;
    wrong_status.commit_status = CloseCommitStatus::PtyStopped;

    for invalid in [wrong_id, wrong_epoch, wrong_intent, wrong_status] {
        window.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.finish_terminal_commit(
                    terminal_id(TERMINAL_A),
                    attempt,
                    Ok(Ok(invalid)),
                    window.window_handle(),
                    cx,
                );
            });
        });
        root.read_with(cx, |root, cx| {
            assert_eq!(root.tasks.task_identity(task_id), Some(original));
            let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
            assert_eq!(close.coordinator.current_attempt(), Some(attempt));
            assert_eq!(
                close.persisted.as_ref().unwrap().commit_status,
                CloseCommitStatus::Collected
            );
            assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        });
    }
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
}

#[gpui::test]
fn advance_receipt_with_wrong_identity_next_or_status_is_ignored(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_pty_stopped BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected pty status failure'); END;",
        )
        .unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_pty_stopped")
        .unwrap();
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok());
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let (original, attempt, current) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        (
            root.tasks.task_identity(task_id).unwrap(),
            close.coordinator.current_attempt().unwrap(),
            close.persisted.clone().unwrap(),
        )
    });
    let mut wrong_id = current.clone();
    wrong_id.close_id = "wrong-close-id".to_owned();
    wrong_id.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_epoch = current.clone();
    wrong_epoch.store_epoch = "wrong-epoch".to_owned();
    wrong_epoch.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_intent = current.clone();
    wrong_intent.intent = slterm_workspace::CloseIntent::new(
        wrong_intent.intent.objects().cloned().collect(),
        [92; 32],
    )
    .unwrap();
    wrong_intent.commit_status = CloseCommitStatus::PtyStopped;
    let mut wrong_status = current.clone();
    wrong_status.commit_status = CloseCommitStatus::CleanupCommitted;
    let mut wrong_next = current;
    wrong_next.commit_status = CloseCommitStatus::Completed;

    for (next, invalid) in [
        (CloseCommitStatus::PtyStopped, wrong_id),
        (CloseCommitStatus::PtyStopped, wrong_epoch),
        (CloseCommitStatus::PtyStopped, wrong_intent),
        (CloseCommitStatus::PtyStopped, wrong_status),
        (CloseCommitStatus::Completed, wrong_next),
    ] {
        window.update(|_, cx| {
            root.update(cx, |root, cx| {
                root.finish_terminal_advance(
                    terminal_id(TERMINAL_A),
                    attempt,
                    next,
                    Ok(Ok(invalid)),
                    cx,
                );
            });
        });
        root.read_with(cx, |root, cx| {
            assert_eq!(root.tasks.task_identity(task_id), Some(original));
            let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
            assert_eq!(close.coordinator.current_attempt(), Some(attempt));
            assert_eq!(
                close.persisted.as_ref().unwrap().commit_status,
                CloseCommitStatus::StateCommitted
            );
            assert_eq!(
                close.coordinator.phase(),
                ClosePhase::Running(CloseStage::StopPty)
            );
            assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        });
    }
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
}

fn late_cleanup_receipt_preserves_current_task(
    cx: &mut TestAppContext,
    original: CloseCommitStatus,
    late: CloseCommitStatus,
) {
    let (rejected, previous) = match original {
        CloseCommitStatus::CleanupCommitted => ("cleanup_committed", CloseCommitStatus::PtyStopped),
        CloseCommitStatus::Completed => ("completed", CloseCommitStatus::CleanupCommitted),
        _ => unreachable!("测试只覆盖清理回执"),
    };
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(&format!(
            "CREATE TRIGGER reject_cleanup_stage BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = '{rejected}' \
             BEGIN SELECT RAISE(ABORT, 'injected cleanup failure'); END;"
        ))
        .unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_cleanup_stage")
        .unwrap();
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok());
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let (identity, attempt, mut late_receipt) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        (
            root.tasks.task_identity(task_id).unwrap(),
            close.coordinator.current_attempt().unwrap(),
            close.persisted.clone().unwrap(),
        )
    });
    late_receipt.commit_status = late;

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_advance(
                terminal_id(TERMINAL_A),
                attempt,
                late,
                Ok(Ok(late_receipt)),
                cx,
            );
        });
    });
    root.read_with(cx, |root, cx| {
        assert_eq!(root.tasks.task_identity(task_id), Some(identity));
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(close.coordinator.current_attempt(), Some(attempt));
        assert_eq!(close.persisted.as_ref().unwrap().commit_status, previous);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
}

#[gpui::test]
fn completed_cannot_skip_from_pty_stopped(cx: &mut TestAppContext) {
    late_cleanup_receipt_preserves_current_task(
        cx,
        CloseCommitStatus::CleanupCommitted,
        CloseCommitStatus::Completed,
    );
}

#[gpui::test]
fn late_cleanup_committed_cannot_replace_completed_task(cx: &mut TestAppContext) {
    late_cleanup_receipt_preserves_current_task(
        cx,
        CloseCommitStatus::Completed,
        CloseCommitStatus::CleanupCommitted,
    );
}

#[gpui::test]
fn disconnected_advance_receipt_is_fail_closed_without_reregistration(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(activated.is_ok());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_pty_stopped BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected pty status failure'); END;",
        )
        .unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    window.run_until_parked();
    fault
        .execute_batch("DROP TRIGGER reject_pty_stopped")
        .unwrap();
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok());
    let attempt = root.read_with(cx, |root, _| {
        root.terminal_closes[&terminal_id(TERMINAL_A)]
            .coordinator
            .current_attempt()
            .unwrap()
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_advance(
                terminal_id(TERMINAL_A),
                attempt,
                CloseCommitStatus::PtyStopped,
                Err(std::sync::mpsc::RecvError),
                cx,
            );
        });
    });
    root.read_with(cx, |root, cx| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert!(close.outcome_unknown);
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
    });
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        retry,
        Err(AppRootError::TerminalCloseOutcomeUnknown)
    ));
    root.read_with(cx, |root, _| {
        assert!(
            !root
                .tasks
                .is_registered(TaskId::TerminalClose(terminal_id(TERMINAL_A)))
        );
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_some());
    });
    assert!(matches!(
        state.borrow().sessions[&second_panel.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ),)
            .unwrap(),
        "state_committed"
    );
    assert!(
        !window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)))
    );
}

#[test]
fn closing_focus_fallback_cannot_bypass_modal_scope() {
    let source = include_str!("../terminal_close.rs");
    let detach = source
        .split("fn detach_closing_terminal")
        .nth(1)
        .unwrap()
        .split("fn destroy_closed_terminal")
        .next()
        .unwrap();
    assert!(
        !detach.contains("workspace_focus.focus"),
        "关闭还焦不得绕过 FocusManager 模态作用域裸送 workspace 焦点"
    );
}

#[gpui::test]
fn modal_opened_during_async_commit_retains_focus_scope(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let closing_panel = panel_entity(&root, cx, PROJECT_A, TERMINAL_A).0;
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_side_button_menu(SideViewId::Nav, None, window, cx)
                .unwrap();
        });
    });
    let terminal_focus = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| terminal_focus.focus(window, cx));
    window.run_until_parked();

    window.update(|window, cx| {
        let focused = window.focused(cx).unwrap();
        let menu = root.read_with(cx, |root, _| {
            root.side_button_menu
                .as_ref()
                .unwrap()
                .menu
                .focus_handle(cx)
        });
        assert_eq!(focused, menu, "异步关闭不得把焦点送到模态外");
    });
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(!root.panel_focus.contains_key(&closing_panel));
    });
}

#[gpui::test]
fn duplicate_stop_scheduling_waits_for_original_receipt(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let (close_id, commit_attempt) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        (
            close.persisted.as_ref().unwrap().close_id.clone(),
            close.coordinator.current_attempt().unwrap(),
        )
    });
    let committed = wait_for_close_status(&paths, &close_id, CloseCommitStatus::StateCommitted);
    let window_handle = window.update(|window, _| window.window_handle());
    cx.update(|cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_commit(
                terminal_id(TERMINAL_A),
                commit_attempt,
                Ok(Ok(committed)),
                window_handle,
                cx,
            );
        });
    });
    let task_id = TaskId::TerminalClose(terminal_id(TERMINAL_A));
    let (attempt, identity) = root.read_with(cx, |root, cx| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::StopPty)
        );
        assert!(close.terminal.read(cx).session.is_none());
        (
            close.coordinator.current_attempt().unwrap(),
            root.tasks.task_identity(task_id).unwrap(),
        )
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.start_terminal_stop(terminal_id(TERMINAL_A), cx)
        });
    });

    root.read_with(cx, |root, cx| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::StopPty),
            "重复调度必须等待原回执，不得转入 Failed"
        );
        assert_eq!(close.coordinator.current_attempt(), Some(attempt));
        assert!(!close.outcome_unknown);
        assert_eq!(root.tasks.task_identity(task_id), Some(identity));
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        raw.query_row(
            "SELECT commit_status FROM close_intents WHERE close_id = ?1",
            [close_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "completed"
    );
}
