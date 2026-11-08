//! 局部终端关闭的提交、停止、清理与重试测试。

use super::super::task_service::TaskId;
use super::close::{flush_when_ready, wait_for_close_status};
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{CloseAttempt, ClosePhase, CloseStage};

#[gpui::test]
fn local_close_defers_panel_destruction_and_blocks_window_close(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, _, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);

    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok(), "局部关闭应异步接受: {accepted:?}");
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(!root.close_completed);
    });
    let window_closed =
        window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));
    assert!(!window_closed, "局部关闭期间不得冲刷并销毁窗口");

    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_none());
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .is_some()
        );
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(weak.upgrade().is_none());
}

#[gpui::test]
fn persisted_local_close_commits_snapshot_and_completes_intent(cx: &mut TestAppContext) {
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
    let (_, _, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let request_generation = root.read_with(cx, |root, _| root.snapshot_generation);
    let database_lock = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    database_lock.execute_batch("BEGIN EXCLUSIVE").unwrap();

    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok(), "真实 writer 关闭应接受: {accepted:?}");
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Running(CloseStage::CommitState),
            "数据库写锁不得阻塞 GPUI 前台接受与状态登记"
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
    });
    assert!(
        matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "原子提交完成前不得停止 PTY"
    );
    database_lock.execute_batch("ROLLBACK").unwrap();
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .terminal(terminal_id(TERMINAL_A))
            .is_none()
    );
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    let (close_id, status, scope, epoch, fingerprint): (String, String, String, String, Vec<u8>) =
        raw.query_row(
            "SELECT close_id, commit_status, scope, store_epoch, commit_fingerprint \
             FROM close_intents",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        uuid::Uuid::parse_str(&close_id).unwrap().get_version_num(),
        7
    );
    assert_eq!(status, "completed");
    assert_eq!(scope, "panels");
    assert_eq!(epoch, runtime.metadata().store_epoch);
    assert_eq!(fingerprint.len(), 32);
    let (object_key, generation, decision): (String, i64, String) = raw
        .query_row(
            "SELECT object_key, object_generation, decision FROM close_objects WHERE close_id = ?1",
            [close_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(object_key, TERMINAL_A);
    assert_eq!(generation, request_generation as i64);
    assert_eq!(decision, "not_required");
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(!store.metadata().unwrap().unwrap().clean_shutdown);
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(weak.upgrade().is_none());
}

#[gpui::test]
fn commit_failure_rolls_back_and_retry_preserves_deferred_exited(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_close_registration BEFORE INSERT ON close_intents \
             BEGIN SELECT RAISE(ABORT, 'injected close registration failure'); END;",
        )
        .unwrap();
    let attempts = state.borrow().attempts;

    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let rejected_command = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(
                terminal_spec(PROJECT_A, GROUP_A, TERMINAL_C),
                window,
                cx,
            )
        })
    });
    assert!(
        rejected_command.is_err(),
        "初始关闭提交在途时显式快照命令必须拒绝"
    );
    assert_eq!(state.borrow().attempts, attempts, "拒绝命令不得启动 PTY");
    let exited_panel = panel_entity(&root, cx, PROJECT_A, TERMINAL_B).0;
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(exited_panel, &TerminalViewEvent::Exited, window, cx);
        });
    });
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert!(root.terminal_closes.is_empty());
        assert_eq!(
            root.snapshot()
                .terminal(terminal_id(TERMINAL_B))
                .unwrap()
                .run_intent(),
            RunIntent::Exited
        );
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
    });
    fault
        .execute_batch("DROP TRIGGER reject_close_registration")
        .unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let after_failure = store.load_project_snapshot().unwrap();
    assert!(after_failure.terminal(terminal_id(TERMINAL_A)).is_some());
    assert_eq!(
        after_failure
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Exited,
        "提交失败后必须恢复被延期的最新普通写"
    );
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0,
        "组合事务失败不得留下关闭意图或快照半提交"
    );

    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok(), "撤除故障后应可重新关闭: {retry:?}");
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    let completed = store.load_project_snapshot().unwrap();
    assert!(completed.terminal(terminal_id(TERMINAL_A)).is_none());
    assert_eq!(
        completed
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Exited
    );
    assert_eq!(
        raw.query_row("SELECT commit_status FROM close_intents", [], |row| row
            .get::<_, String>(
            0
        ),)
            .unwrap(),
        "completed"
    );
}

#[gpui::test]
fn pty_stopped_failure_retains_panel_and_stale_result_cannot_cancel_retry(cx: &mut TestAppContext) {
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
    let duplicate = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(matches!(duplicate, Err(AppRootError::TerminalCloseRunning)));
    window.run_until_parked();

    assert!(
        matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ),
        "推进失败前真实 Shutdown 已发生"
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .persisted
                .as_ref()
                .unwrap()
                .commit_status,
            CloseCommitStatus::StateCommitted
        );
    });
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ),)
            .unwrap(),
        "state_committed"
    );
    fault
        .execute_batch("DROP TRIGGER reject_pty_stopped")
        .unwrap();

    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok(), "撤故障后显式重试应接受: {retry:?}");
    let (stale, current, stale_result) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        let current = close.coordinator.current_attempt().unwrap();
        let stale = CloseAttempt::new(CloseStage::StopPty, current.sequence() - 1);
        let mut stale_result = close.persisted.clone().unwrap();
        stale_result.commit_status = CloseCommitStatus::PtyStopped;
        (stale, current, stale_result)
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_advance(
                terminal_id(TERMINAL_A),
                stale,
                CloseCommitStatus::PtyStopped,
                Ok(Ok(stale_result)),
                cx,
            );
        });
    });
    root.read_with(cx, |root, cx| {
        assert!(
            root.tasks
                .is_registered(TaskId::TerminalClose(terminal_id(TERMINAL_A))),
            "stale 回执不得移除当前 attempt 的任务所有权"
        );
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(close.coordinator.current_attempt(), Some(current));
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted,
            "stale 回执不得改写当前持久阶段"
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.panel_count(project_id(PROJECT_A), cx),
            0,
            "stale 回执不得取消当前重试任务"
        );
        assert!(root.terminal_closes.is_empty());
    });
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ),)
            .unwrap(),
        "completed"
    );
}

fn cleanup_advance_failure_retries_from_persisted_stage(
    cx: &mut TestAppContext,
    rejected: CloseCommitStatus,
    expected_before_retry: CloseCommitStatus,
) {
    let (status, previous) = match rejected {
        CloseCommitStatus::CleanupCommitted => ("cleanup_committed", "pty_stopped"),
        CloseCommitStatus::Completed => ("completed", "cleanup_committed"),
        _ => unreachable!("测试只注入清理阶段"),
    };
    assert_eq!(
        previous,
        match expected_before_retry {
            CloseCommitStatus::PtyStopped => "pty_stopped",
            CloseCommitStatus::CleanupCommitted => "cleanup_committed",
            _ => unreachable!("测试只保留清理前序状态"),
        }
    );
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
            "CREATE TRIGGER reject_close_advance BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = '{status}' \
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

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .persisted
                .as_ref()
                .unwrap()
                .commit_status,
            expected_before_retry
        );
    });
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ),)
            .unwrap(),
        previous
    );
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    fault
        .execute_batch("DROP TRIGGER reject_close_advance")
        .unwrap();

    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok(), "清理阶段显式重试应接受: {retry:?}");
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
    let retry_notification = state.borrow().sessions[&panel.sequence()].try_recv();
    assert!(
        !matches!(retry_notification, Ok(Msg::Shutdown)),
        "清理重试不得再次停止 PTY，实际: {retry_notification:?}"
    );
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ),)
            .unwrap(),
        "completed"
    );
}

#[gpui::test]
fn cleanup_committed_failure_retains_panel_and_retries_cleanup(cx: &mut TestAppContext) {
    cleanup_advance_failure_retries_from_persisted_stage(
        cx,
        CloseCommitStatus::CleanupCommitted,
        CloseCommitStatus::PtyStopped,
    );
}

#[gpui::test]
fn completed_failure_retains_panel_and_retries_only_completion(cx: &mut TestAppContext) {
    cleanup_advance_failure_retries_from_persisted_stage(
        cx,
        CloseCommitStatus::Completed,
        CloseCommitStatus::CleanupCommitted,
    );
}

#[gpui::test]
fn join_panic_requires_explicit_retry_before_cleanup(cx: &mut TestAppContext) {
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
    let stop_attempt = root.read_with(cx, |root, cx| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::StopPty)
        );
        assert!(
            close.terminal.read(cx).session.is_none(),
            "注入 join panic 前必须已移交并消费原 session"
        );
        close.coordinator.current_attempt().unwrap()
    });

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let panic_payload: Box<dyn std::any::Any + Send> =
                Box::new("sensitive-test-panic-payload");
            root.finish_terminal_stop(
                terminal_id(TERMINAL_A),
                stop_attempt,
                Err(panic_payload),
                cx,
            );
        });
    });

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty),
            "首次 join panic 不得自动进入 Cleanup"
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
    });
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        raw.query_row(
            "SELECT commit_status FROM close_intents WHERE close_id = ?1",
            [close_id.clone()],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "state_committed"
    );
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(retry.is_ok(), "只有显式重试才消费异常终止事实: {retry:?}");
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
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

#[gpui::test]
fn committed_close_rebases_onto_exited_and_later_writes_do_not_restore_terminal(
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
    split_terminal(&root, &mut window);
    flush_when_ready(&runtime.client());

    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let exited_panel = panel_entity(&root, cx, PROJECT_A, TERMINAL_B).0;
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(exited_panel, &TerminalViewEvent::Exited, window, cx);
        });
    });
    window.run_until_parked();
    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_B), window, cx)
        })
    });
    assert!(activated.is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    let snapshot = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert!(snapshot.terminal(terminal_id(TERMINAL_A)).is_none());
    assert_eq!(
        snapshot
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Exited,
        "旧关闭候选与后续普通写都不得覆盖最新 Exited 或复活关闭终端"
    );
}

#[gpui::test]
fn writer_without_current_epoch_rejects_close_without_side_effects(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });

    assert!(matches!(
        rejected,
        Err(AppRootError::WorkspaceEpochUnavailable)
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.terminal_closes.is_empty());
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_some());
    });
    writer.flush().unwrap();
    assert!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_project_snapshot()
            .unwrap()
            .terminal(terminal_id(TERMINAL_A))
            .is_some()
    );
}

#[gpui::test]
fn disconnected_commit_receipt_is_fail_closed_and_never_blindly_retried(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let attempt = root.read_with(cx, |root, _| {
        root.terminal_closes[&terminal_id(TERMINAL_A)]
            .coordinator
            .current_attempt()
            .unwrap()
    });
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

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
        assert!(root.terminal_closes[&terminal_id(TERMINAL_A)].outcome_unknown);
    });
    let retry = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(matches!(
        retry,
        Err(AppRootError::TerminalCloseOutcomeUnknown)
    ));
    let attempts = state.borrow().attempts;
    let command = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(
                terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        command,
        Err(AppRootError::ProjectSnapshotCommitInFlight)
    ));
    assert_eq!(state.borrow().attempts, attempts);
    assert!(
        !window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)))
    );
}

#[gpui::test]
fn stale_commit_receipt_preserves_current_task_and_attempt(cx: &mut TestAppContext) {
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
    let current = root.read_with(cx, |root, _| {
        root.terminal_closes[&terminal_id(TERMINAL_A)]
            .coordinator
            .current_attempt()
            .unwrap()
    });
    let stale = CloseAttempt::new(CloseStage::Collect, current.sequence() - 1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_terminal_commit(
                terminal_id(TERMINAL_A),
                stale,
                Err(std::sync::mpsc::RecvError),
                window.window_handle(),
                cx,
            );
        });
    });

    root.read_with(cx, |root, cx| {
        assert!(
            root.tasks
                .is_registered(TaskId::TerminalClose(terminal_id(TERMINAL_A)))
        );
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(close.coordinator.current_attempt(), Some(current));
        assert!(!close.outcome_unknown);
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::Collected
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
}

#[gpui::test]
fn stale_stop_receipt_preserves_current_task_and_attempt(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let current = root.read_with(cx, |root, _| {
        root.terminal_closes[&terminal_id(TERMINAL_A)]
            .coordinator
            .current_attempt()
            .unwrap()
    });
    let stale = CloseAttempt::new(CloseStage::CommitState, current.sequence() - 1);
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let payload: Box<dyn std::any::Any + Send> = Box::new("stale-stop");
            root.finish_terminal_stop(terminal_id(TERMINAL_A), stale, Err(payload), cx);
        });
    });

    root.read_with(cx, |root, cx| {
        assert!(
            root.tasks
                .is_registered(TaskId::TerminalClose(terminal_id(TERMINAL_A)))
        );
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        assert_eq!(close.coordinator.current_attempt(), Some(current));
        assert!(close.stop_observation.is_none());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_closes.is_empty());
    });
}

#[gpui::test]
fn global_close_rejects_local_terminal_close_without_stopping_pty(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    root.update(cx, |root, _| root.close_in_progress = true);

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });

    assert!(matches!(rejected, Err(AppRootError::Closing)));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.terminal_closes.is_empty());
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[gpui::test]
fn commit_inflight_rejects_another_local_close_before_any_side_effect(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = split_terminal(&root, &mut window);
    let before = root.read_with(cx, |root, _| {
        (root.snapshot().clone(), root.snapshot_generation)
    });

    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let direct = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_B), window, cx)
        })
    });
    assert!(matches!(
        direct,
        Err(AppRootError::ProjectSnapshotCommitInFlight)
    ));
    let focused = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_focused_terminal_from_request(window, cx)
        })
    });
    assert!(matches!(
        focused,
        Err(AppRootError::ProjectSnapshotCommitInFlight)
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!((root.snapshot().clone(), root.snapshot_generation), before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.terminal_closes.len(), 1);
    });
    for panel in [first_panel, second_panel] {
        assert!(
            matches!(
                state.borrow().sessions[&panel.sequence()].try_recv(),
                Err(TryRecvError::Empty)
            ),
            "并发关闭拒绝前不得停止任何 PTY"
        );
    }

    window.run_until_parked();
    flush_when_ready(&runtime.client());
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1,
        "被拒绝的并发关闭不得发出第二个写请求"
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_some());
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_B), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
    });
}

#[gpui::test]
fn drop_with_unknown_commit_does_not_restore_closed_terminal(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    flush_when_ready(&runtime.client());
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());
    let (close_id, attempt) = root.read_with(cx, |root, _| {
        let close = &root.terminal_closes[&terminal_id(TERMINAL_A)];
        (
            close.persisted.as_ref().unwrap().close_id.clone(),
            close.coordinator.current_attempt().unwrap(),
        )
    });
    let committed = wait_for_close_status(&paths, &close_id, CloseCommitStatus::StateCommitted);
    assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
    let committed_snapshot = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert!(
        committed_snapshot
            .terminal(terminal_id(TERMINAL_A))
            .is_none()
    );
    assert_eq!(
        committed_snapshot
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Launch
    );

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
    let exited_panel = panel_entity(&root, cx, PROJECT_A, TERMINAL_B).0;
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(exited_panel, &TerminalViewEvent::Exited, window, cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert!(root.pending_final_project_snapshot.is_some());
        assert!(
            root.pending_final_project_snapshot
                .as_ref()
                .unwrap()
                .terminal(terminal_id(TERMINAL_A))
                .is_some(),
            "延期快照必须包含待关闭终端，才能检验 Drop 屏障"
        );
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
    });
    let weak = root.downgrade();
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_drop_project_write BEFORE INSERT ON projects \
             BEGIN SELECT RAISE(ABORT, 'injected drop write'); END;",
        )
        .unwrap();
    window.update(|window, _| window.remove_window());
    drop(root);
    cx.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();
    let drop_flush = runtime.client().flush().unwrap().recv().unwrap();
    assert_eq!(
        drop_flush.failed, 0,
        "CommitState 屏障期间 Drop 不得发送项目终态写"
    );
    fault
        .execute_batch("DROP TRIGGER reject_drop_project_write")
        .unwrap();

    let after_drop = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert!(
        after_drop.terminal(terminal_id(TERMINAL_A)).is_none(),
        "未知提交结果下 Drop 不得用延期旧快照复活关闭终端"
    );
    assert_eq!(
        after_drop
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Launch,
        "写屏障未解除时延期 Exited 不得绕过原子提交事实"
    );
}

#[gpui::test]
fn close_terminal_releases_instance_and_keeps_empty_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, _, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot().terminals().count(), 0);
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .is_some()
        );
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .layout()
                .view_keys()
                .is_empty()
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(weak.upgrade().is_none());
}

#[gpui::test]
fn closing_last_panel_releases_actual_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let terminal = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    let terminal_focus = terminal.read_with(cx, |terminal, _| terminal.focus_handle.clone());

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    window.update(|window, cx| {
        let focused = window.focused(cx);
        assert!(focused.is_some(), "空项目必须保留可用焦点落点");
        assert_ne!(focused, Some(terminal_focus), "不得留住已销毁面板焦点");
    });
}

#[gpui::test]
fn closing_remembered_panel_preserves_actual_project_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    let project_focus = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| project_focus.focus(window, cx));
    window.run_until_parked();

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_B), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    window.update(|window, _| {
        assert!(
            project_focus.is_focused(window),
            "关闭非真实焦点面板不得抢走项目容器焦点"
        );
    });
}

#[gpui::test]
fn close_focused_terminal_closes_split_panel_and_updates_focus(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    let first = new_generated_terminal(&root, &mut window);
    let split = split_generated_terminal(&root, &mut window);

    let closed = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(closed.is_ok(), "关闭聚焦终端失败: {closed:?}");
    window.run_until_parked();
    let CommandOutcome::TerminalClosed(closed) = closed.unwrap() else {
        unreachable!("关闭命令必须返回逻辑记录");
    };

    root.read_with(cx, |root, cx| {
        assert!(root.snapshot().terminal(closed.id()).is_none());
        assert_eq!(root.snapshot().terminals().count(), 1);
        assert_eq!(root.focused_panel(project, cx), Some(first));
        assert_eq!(root.panel_count(project, cx), 1);
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids().len(), 1);
    });
    assert!(matches!(
        state.borrow().sessions[&split.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn close_terminal_shortcut_uses_app_root_command(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    split_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("ctrl-shift-w");
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 1);
        assert_eq!(root.snapshot().registry().active(), Some(project));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn focused_close_uses_actual_focus_instead_of_stale_projection(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = split_terminal(&root, &mut window);
    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(activated.is_ok());
    let second_focus = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| {
        root.update(cx, |root, _| root.focus_subscriptions = None);
        second_focus.focus(window, cx);
    });
    window.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(second_focus.clone()));
        assert_eq!(
            root.read_with(cx, |root, cx| root.focused_panel(project_id(PROJECT_A), cx)),
            Some(first_panel),
            "测试必须保持投影 A 与真实焦点 B 不同步"
        );
    });

    let closed = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_focused_terminal_from_request(window, cx)
        })
    });
    assert_eq!(closed.unwrap().id(), terminal_id(TERMINAL_B));
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_some());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_none());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert!(
        matches!(
            state.borrow().sessions[&first_panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "不得误关投影中的 A"
    );
    assert!(matches!(
        state.borrow().sessions[&second_panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
}

fn assert_epoch_rejection_has_no_side_effects(cx: &mut TestAppContext, focused: bool) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = split_terminal(&root, &mut window);
    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(activated.is_ok());
    let second_focus = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| {
        root.update(cx, |root, _| root.focus_subscriptions = None);
        second_focus.focus(window, cx);
    });
    writer.flush().unwrap();
    let before_snapshot = root.read_with(cx, |root, _| root.snapshot().clone());
    let before_disk = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert_eq!(before_snapshot, before_disk);
    let (before_generation, before_pending) = root.read_with(cx, |root, _| {
        (
            root.snapshot_generation,
            root.pending_final_project_snapshot.clone(),
        )
    });

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            if focused {
                root.close_focused_terminal_from_request(window, cx)
            } else {
                root.close_terminal(terminal_id(TERMINAL_B), window, cx)
            }
        })
    });
    assert!(matches!(
        rejected,
        Err(AppRootError::WorkspaceEpochUnavailable)
    ));

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before_snapshot);
        assert_eq!(root.snapshot_generation, before_generation);
        assert_eq!(root.pending_final_project_snapshot, before_pending);
        assert_eq!(
            root.focused_panel(project_id(PROJECT_A), cx),
            Some(first_panel),
            "世代拒绝不得改变焦点投影"
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert!(root.terminal_closes.is_empty());
        for id in [TERMINAL_A, TERMINAL_B] {
            assert!(
                root.terminal_panel(project_id(PROJECT_A), terminal_id(id), cx)
                    .unwrap()
                    .1
                    .read(cx)
                    .session
                    .is_some()
            );
        }
    });
    for panel in [first_panel, second_panel] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ));
    }
    writer.flush().unwrap();
    assert_eq!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_project_snapshot()
            .unwrap(),
        before_disk,
        "世代拒绝不得产生普通快照写"
    );
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[gpui::test]
fn direct_close_epoch_rejection_preserves_focus_snapshot_and_writes(cx: &mut TestAppContext) {
    assert_epoch_rejection_has_no_side_effects(cx, false);
}

#[gpui::test]
fn focused_close_epoch_rejection_preserves_focus_snapshot_and_writes(cx: &mut TestAppContext) {
    assert_epoch_rejection_has_no_side_effects(cx, true);
}

fn assert_recovery_failure_rejects_close_without_side_effects(
    cx: &mut TestAppContext,
    focused: bool,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let original = terminal_recovery_snapshot();
    save_terminal_recovery_snapshot(&paths, &original);
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    let valid_layout = raw
        .query_row(
            "SELECT layout_json FROM project_layouts WHERE project_id = ?1",
            [PROJECT_A],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    raw.execute(
        "UPDATE project_layouts SET layout_json = 'not-json' WHERE project_id = ?1",
        [PROJECT_A],
    )
    .unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(runtime.client()));
    assert!(persistence.project_snapshot_recovery_failed());
    raw.execute(
        "UPDATE project_layouts SET layout_json = ?2 WHERE project_id = ?1",
        rusqlite::params![PROJECT_A, valid_layout],
    )
    .unwrap();
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_C, "D:/work/explicit");
    let panel = new_terminal(&root, &mut window, PROJECT_C, GROUP_C, TERMINAL_C);
    flush_when_ready(&runtime.client());
    let before_disk = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert_eq!(before_disk, original);
    let (before_snapshot, before_generation, before_pending, before_focused_panel) = root
        .read_with(cx, |root, cx| {
            (
                root.snapshot().clone(),
                root.snapshot_generation,
                root.pending_final_project_snapshot.clone(),
                root.focused_panel(project_id(PROJECT_C), cx),
            )
        });
    let before_actual_focus = window.update(|window, cx| window.focused(cx));

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            if focused {
                root.execute_command(
                    CommandId::CloseActiveTerminal,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                )
                .map(|outcome| match outcome {
                    CommandOutcome::TerminalClosed(record) => record,
                    _ => panic!("关闭命令必须返回终端记录"),
                })
            } else {
                root.close_terminal(terminal_id(TERMINAL_C), window, cx)
            }
        })
    });
    assert!(
        matches!(rejected, Err(AppRootError::ProjectSnapshotRecoveryFailed)),
        "恢复失败必须使用独立错误: {rejected:?}"
    );
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before_snapshot);
        assert_eq!(root.snapshot_generation, before_generation);
        assert_eq!(root.pending_final_project_snapshot, before_pending);
        assert_eq!(
            root.focused_panel(project_id(PROJECT_C), cx),
            before_focused_panel
        );
        assert_eq!(root.panel_count(project_id(PROJECT_C), cx), 1);
        assert!(root.terminal_closes.is_empty());
        assert!(
            root.terminal_panel(project_id(PROJECT_C), terminal_id(TERMINAL_C), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
    });
    window.update(|window, cx| assert_eq!(window.focused(cx), before_actual_focus));
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    flush_when_ready(&runtime.client());
    assert_eq!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_project_snapshot()
            .unwrap(),
        before_disk,
        "恢复屏障必须同时阻止普通写与局部关闭组合提交"
    );
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn direct_close_recovery_failure_preserves_snapshot_focus_and_session(cx: &mut TestAppContext) {
    assert_recovery_failure_rejects_close_without_side_effects(cx, false);
}

#[gpui::test]
fn focused_close_recovery_failure_preserves_snapshot_focus_and_session(cx: &mut TestAppContext) {
    assert_recovery_failure_rejects_close_without_side_effects(cx, true);
}

fn close_active_command(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
) -> Result<TerminalRecord, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
            .map(|outcome| match outcome {
                CommandOutcome::TerminalClosed(record) => record,
                _ => panic!("关闭命令必须返回终端记录"),
            })
        })
    })
}

fn reject_completed_transition(paths: &AppPaths) -> rusqlite::Connection {
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_close_completed BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'completed' \
             BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END;",
        )
        .unwrap();
    fault
}

#[gpui::test]
fn close_active_command_retries_detached_failure_instead_of_focused_peer(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(activated.is_ok());
    flush_when_ready(&runtime.client());
    let fault = reject_completed_transition(&paths);

    let accepted = close_active_command(&root, &mut window).unwrap();
    assert_eq!(accepted.id(), terminal_id(TERMINAL_A));
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
    });
    let second_focus = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| {
        assert_eq!(
            window.focused(cx),
            Some(second_focus),
            "StateCommitted 后真实焦点必须已经还到 B"
        );
    });
    fault
        .execute_batch("DROP TRIGGER reject_close_completed")
        .unwrap();

    let retried = close_active_command(&root, &mut window).unwrap();
    assert_eq!(
        retried.id(),
        terminal_id(TERMINAL_A),
        "A 已失败时不得把真实焦点上的 B 当作新关闭目标"
    );
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert!(root.terminal_closes.is_empty());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_some());
    });
    assert!(matches!(
        state.borrow().sessions[&first_panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&second_panel.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[gpui::test]
fn close_active_command_retries_last_detached_failure_without_focus(cx: &mut TestAppContext) {
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
    let fault = reject_completed_transition(&paths);

    close_active_command(&root, &mut window).unwrap();
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.terminal_closes[&terminal_id(TERMINAL_A)]
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), None);
    });
    fault
        .execute_batch("DROP TRIGGER reject_close_completed")
        .unwrap();

    let retried = close_active_command(&root, &mut window).unwrap();
    assert_eq!(retried.id(), terminal_id(TERMINAL_A));
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert!(root.terminal_closes.is_empty());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
}

#[gpui::test]
fn close_active_command_retries_multiple_failures_in_initiation_order(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let third_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    flush_when_ready(&runtime.client());
    let fault = reject_completed_transition(&paths);

    for id in [TERMINAL_B, TERMINAL_A] {
        let accepted = window.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.close_terminal(terminal_id(id), window, cx)
            })
        });
        assert!(accepted.is_ok());
        window.run_until_parked();
    }
    root.read_with(cx, |root, _| {
        for id in [TERMINAL_A, TERMINAL_B] {
            assert_eq!(
                root.terminal_closes[&terminal_id(id)].coordinator.phase(),
                ClosePhase::Failed(CloseStage::Cleanup)
            );
        }
    });
    fault
        .execute_batch("DROP TRIGGER reject_close_completed")
        .unwrap();

    let first_retry = close_active_command(&root, &mut window).unwrap();
    assert_eq!(
        first_retry.id(),
        terminal_id(TERMINAL_B),
        "B 先发起关闭，不得按 TerminalId 顺序先重试 A"
    );
    let during_retry = close_active_command(&root, &mut window);
    assert!(
        matches!(during_retry, Err(AppRootError::TerminalCloseRunning)),
        "最早失败关闭的重试回执未完成前不得推进 A 或新关 C: {during_retry:?}"
    );
    window.run_until_parked();
    let second_retry = close_active_command(&root, &mut window).unwrap();
    assert_eq!(second_retry.id(), terminal_id(TERMINAL_A));
    window.run_until_parked();
    root.read_with(cx, |root, cx| {
        assert!(root.terminal_closes.is_empty());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_C)).is_some());
    });
    assert!(matches!(
        state.borrow().sessions[&first_panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&second_panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&third_panel.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));

    let new_close = close_active_command(&root, &mut window).unwrap();
    assert_eq!(
        new_close.id(),
        terminal_id(TERMINAL_C),
        "全部失败重试完成后才允许新的 focused close"
    );
    window.run_until_parked();
}
