//! 组内批量关闭的原子提交、空组塌缩与失败重试测试。

use super::close::flush_when_ready;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{ClosePhase, CloseStage};

fn execute_batch(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: CommandId,
    group: PanelGroupId,
) -> Result<CommandOutcome, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                id,
                CommandSource::Menu,
                CommandTarget::Focused,
                Some(CommandParameters::CloseGroup { group_id: group }),
                window,
                cx,
            )
        })
    })
}

fn close_active(
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

fn group_views(root: &Entity<AppRoot>, cx: &TestAppContext, group: PanelGroupId) -> Vec<ViewKey> {
    root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .and_then(|project| project.layout().group(group))
            .map(|group| group.views().to_vec())
            .unwrap_or_default()
    })
}

#[gpui::test]
fn integration_batch_close(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let kept = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let neighbor = window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.split_terminal_with_identity(
                    SplitTerminalSpec {
                        new_terminal: terminal_spec(PROJECT_A, GROUP_A, TERMINAL_C),
                        new_group_id: group_id(GROUP_B),
                        direction: SplitDirection::LeftRight,
                        ratio: 0.5,
                        viewport: LogicalSize::new(1200.0, 800.0),
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.activate_terminal(terminal_id(TERMINAL_B), window, cx)
            })
        })
        .unwrap();
    let first_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let kept_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_B);
    let neighbor_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_C);
    let kept_focus = window.update(|window, cx| window.focused(cx));

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseOtherViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    window.run_until_parked();

    assert_eq!(
        group_views(&root, cx, group_id(GROUP_A)),
        vec![ViewKey::Terminal {
            project_id: project_id(PROJECT_A),
            terminal_id: terminal_id(TERMINAL_B),
        }]
    );
    assert_eq!(
        group_views(&root, cx, group_id(GROUP_B)),
        vec![ViewKey::Terminal {
            project_id: project_id(PROJECT_A),
            terminal_id: terminal_id(TERMINAL_C),
        }]
    );
    assert!(first_entity.2.upgrade().is_none());
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_B).1,
        kept_entity.1
    );
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_C).1,
        neighbor_entity.1
    );
    assert_eq!(first_entity.0, first);
    assert_eq!(kept_entity.0, kept);
    assert_eq!(neighbor_entity.0, neighbor);
    assert!(matches!(
        state.borrow().sessions[&first.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(
        state.borrow().sessions[&neighbor.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(window.update(|window, cx| window.focused(cx)), kept_focus);

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    window.run_until_parked();

    assert!(group_views(&root, cx, group_id(GROUP_A)).is_empty());
    assert_eq!(group_views(&root, cx, group_id(GROUP_B)).len(), 1);
    assert_eq!(
        root.read_with(cx, |root, _| root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group_ids()),
        vec![group_id(GROUP_B)]
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&neighbor.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(root.read_with(cx, |root, _| root.terminal_closes.len()), 0);
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn batch_commit_failure_preserves_every_panel_and_pty(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_close BEFORE INSERT ON close_intents \
             BEGIN SELECT RAISE(ABORT, 'injected batch close failure'); END;",
        )
        .unwrap();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.terminal_close_batches.len(), 1);
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::CommitState)
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.snapshot(), &before);
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
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ));
    }
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
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
    fault
        .execute_batch("DROP TRIGGER reject_batch_close")
        .unwrap();
    assert_eq!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_project_snapshot()
            .unwrap(),
        before
    );
    assert_eq!(
        fault
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .is_ok()
    );
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn batch_stop_failure_retries_original_batch_without_rejoin_or_new_target(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let kept = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_pty_stopped BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected batch pty failure'); END;",
        )
        .unwrap();

    execute_batch(
        &root,
        &mut window,
        CommandId::CloseOtherViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 3);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_C)).is_some());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_none());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_none());
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert!(close.unresolved_failure);
    });
    let close_id = root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .unwrap()
            .persisted
            .as_ref()
            .unwrap()
            .close_id
            .clone()
    });
    let object_keys = fault
        .prepare("SELECT object_key FROM close_objects WHERE close_id = ?1 ORDER BY object_key")
        .unwrap()
        .query_map([close_id], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        object_keys,
        vec![TERMINAL_A.to_owned(), TERMINAL_B.to_owned()]
    );
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    fault
        .execute_batch("DROP TRIGGER reject_batch_pty_stopped")
        .unwrap();

    let retried = close_active(&root, &mut window).unwrap();
    assert_eq!(retried.id(), terminal_id(TERMINAL_A));
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_C)).is_some());
    });
    for panel in [first, second] {
        assert!(
            !matches!(
                state.borrow().sessions[&panel.sequence()].try_recv(),
                Ok(Msg::Shutdown)
            ),
            "显式重试不得再次 join 已停止的成员"
        );
    }
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn batch_cleanup_failure_retries_without_rejoining_members(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_cleanup BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'cleanup_committed' \
             BEGIN SELECT RAISE(ABORT, 'injected batch cleanup failure'); END;",
        )
        .unwrap();

    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::PtyStopped
        );
    });
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    fault
        .execute_batch("DROP TRIGGER reject_batch_cleanup")
        .unwrap();

    assert_eq!(
        close_active(&root, &mut window).unwrap().id(),
        terminal_id(TERMINAL_A)
    );
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
    });
    for panel in [first, second] {
        assert!(!matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
