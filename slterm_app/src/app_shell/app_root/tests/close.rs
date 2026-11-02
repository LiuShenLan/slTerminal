//! 关闭协调与事件泵生命周期测试。

use super::*;
use crate::persistence::workspace_writer::{WorkspaceWriterClient, WorkspaceWriterError};

fn flush_when_ready(client: &WorkspaceWriterClient) {
    // 负载裕度，非断言/语义放宽：等待真实 Flush 结果，故障前置条件不变。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        match client.flush() {
            Ok(response) => {
                response
                    .recv_timeout(Duration::from_secs(5))
                    .expect("writer flush did not respond before the deadline");
                return;
            }
            Err(WorkspaceWriterError::QueueFull) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("flush failed: {error}"),
        }
    }
    panic!("writer did not drain before the deadline");
}

/// 调用方必须持有数据库写锁；强制写线程进入故障写入后确认普通队列已满。
fn fill_writer_queue(client: &WorkspaceWriterClient) {
    client
        .save_sidebar_state(&slterm_sidebar::SidebarState::default())
        .unwrap();
    let _writer_blocked = client.flush().unwrap();
    let mut queue_full = false;
    for _ in 0..=64 {
        match client.save_sidebar_state(&slterm_sidebar::SidebarState::default()) {
            Ok(()) => {}
            Err(WorkspaceWriterError::QueueFull) => {
                queue_full = true;
                break;
            }
            Err(error) => panic!("unexpected writer error: {error}"),
        }
    }
    assert!(queue_full, "故障前置条件必须填满普通写队列");
}

/// 以同一 ViewKey 的递增值填充队列，返回最后一个实际入队的状态。
fn fill_view_state_queue(
    client: &WorkspaceWriterClient,
    key: &ViewKey,
    registry: &slterm_workspace::ViewStateRegistry,
) -> slterm_workspace::ViewState {
    let state = |scroll: f64| {
        slterm_workspace::ViewState::new(
            key,
            serde_json::json!({"projects": [], "unassigned_expanded": true, "scroll": scroll}),
            registry,
        )
        .unwrap()
    };
    client.save_view_state(key, &state(0.0)).unwrap();
    let _writer_blocked = client.flush().unwrap();
    let mut queue_full = false;
    let mut last_accepted = None;
    for attempt in 1..=65 {
        let candidate = state(attempt as f64);
        match client.save_view_state(key, &candidate) {
            Ok(()) => last_accepted = Some(candidate),
            Err(WorkspaceWriterError::QueueFull) => {
                queue_full = true;
                break;
            }
            Err(error) => panic!("unexpected writer error: {error}"),
        }
    }
    assert!(queue_full, "故障前置条件必须填满普通写队列");
    last_accepted.expect("至少一个 ViewState 必须进入普通队列")
}

/// 最后入队值落盘即证明其后的普通命令也已排空，且未触碰独立终态通道。
fn wait_for_view_state(
    paths: &AppPaths,
    key: &ViewKey,
    registry: &slterm_workspace::ViewStateRegistry,
    expected: &slterm_workspace::ViewState,
) {
    // 负载裕度，非断言/语义放宽：以真实磁盘状态判定队列排水完成。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let store = WorkspaceDatabase::open(paths).unwrap();
        let recovered = store.view_state(key, registry).unwrap();
        if recovered.is_some_and(|stored| stored.state.state() == expected.state()) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "last queued ViewState was not persisted before the deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn tray_quit_until_closed(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) {
    // 负载裕度，非断言/语义放宽：只通过真实重试等待写线程排水，不做外部 flush。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        window.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
            });
        });
        window.run_until_parked();
        if root.read_with(cx, |root, _| root.close_completed) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("TrayQuit retries did not close without an external flush");
}

#[gpui::test]
fn shell_event_pump_releases_receiver_on_app_root_drop(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx)
        })
    });
    let weak = root.downgrade();

    window.update(|window, _| window.remove_window());
    drop(root);
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();

    assert!(
        sender.send(SltermShellEvent::TrayQuit).is_err(),
        "AppRoot 销毁后不得保留事件接收端"
    );
}

#[gpui::test]
fn close_flush_success_closes_window_and_marks_clean_shutdown(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let weak = root.downgrade();

    assert!(!window.simulate_close());
    root.read_with(cx, |root, _| assert!(root.close_in_progress));
    window.run_until_parked();
    drop(root);
    cx.update(|_| {});
    weak.assert_released();
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let metadata = WorkspaceDatabase::open(&paths)
        .unwrap()
        .metadata()
        .unwrap()
        .unwrap();
    assert!(metadata.clean_shutdown);
}

#[gpui::test]
fn close_flush_failure_keeps_window_and_allows_retry(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_project_write BEFORE INSERT ON projects \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let weak = root.downgrade();
    let report = runtime.client().flush().unwrap().recv().unwrap();
    assert_eq!(report.failed, 1, "故障注入必须使项目写失败");

    assert!(!window.simulate_close());
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("app-root-content").is_some(),
        "flush 失败必须保留窗口"
    );
    root.read_with(cx, |root, _| assert!(!root.close_in_progress));
    fault
        .execute_batch("DROP TRIGGER reject_project_write")
        .unwrap();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    drop(root);
    cx.update(|_| {});
    weak.assert_released();
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    let metadata = WorkspaceDatabase::open(&paths)
        .unwrap()
        .metadata()
        .unwrap()
        .unwrap();
    assert!(metadata.clean_shutdown);
}

#[gpui::test]
fn closing_rejects_new_commands(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    root.update(cx, |root, _| root.close_in_progress = true);

    let result = window.update(|window, cx| {
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

    assert!(matches!(result, Err(AppRootError::Closing)));
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn close_flush_submits_rejected_final_snapshot(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let client = runtime.client();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(client.clone()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    fill_writer_queue(&client);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let expected = root.read_with(cx, |root, _| {
        assert!(root.pending_final_project_snapshot.is_some());
        root.snapshot().clone()
    });
    let weak = root.downgrade();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    root.read_with(cx, |root, _| assert!(!root.close_in_progress));
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-empty").is_some());

    fault.execute_batch("ROLLBACK").unwrap();
    flush_when_ready(&client);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    drop(root);
    cx.update(|_| {});
    weak.assert_released();
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), expected);
    let registry = super::super::side_view::side_view_state_registry();
    assert!(
        store
            .view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &registry,
            )
            .unwrap()
            .is_some(),
        "SideView 私有终态必须随关窗 flush 提交"
    );
}

#[gpui::test]
fn close_queue_full_retries_without_external_flush(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let client = runtime.client();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(client.clone()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    fill_writer_queue(&client);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.adjust_sidebar_width(83.0, window, cx).unwrap();
        });
    });
    let expected = root.read_with(cx, |root, _| root.snapshot().clone());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    root.read_with(cx, |root, _| assert!(!root.close_in_progress));
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-content").is_some());

    fault.execute_batch("ROLLBACK").unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.adjust_sidebar_width(111.0, window, cx).unwrap();
        });
    });
    let expected_sidebar = root.read_with(cx, |root, _| root.side_views.state().clone());
    let registry = super::super::side_view::side_view_state_registry();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let updated = slterm_workspace::ViewState::new(
        &key,
        serde_json::json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &registry,
    )
    .unwrap();
    let updated = slterm_sidebar::SideViewState::from_view_state(SideViewId::Nav, updated).unwrap();
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .update(cx, |view, _| view.replace_state_for_test(updated.clone()));
        });
    });
    tray_quit_until_closed(&root, &mut window, cx);
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), expected);
    assert_eq!(
        store.sidebar_state().unwrap().unwrap().state,
        expected_sidebar,
        "重试不得让容量一通道中的旧侧栏终态覆盖新编辑"
    );
    let stored = store.view_state(&key, &registry).unwrap().unwrap();
    assert_eq!(
        stored.state.state()["scroll"],
        serde_json::json!(84.0),
        "重试不得让旧 SideView 终态覆盖关闭失败后的修改"
    );
}

#[gpui::test]
fn close_retry_reconciles_side_view_with_project_recovery_barrier(cx: &mut TestAppContext) {
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
    let client = runtime.client();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(client.clone()), None);
    assert!(persistence.project_snapshot_recovery_failed());
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    let registry = super::super::side_view::side_view_state_registry();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let queued = fill_view_state_queue(&client, &key, &registry);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    root.read_with(cx, |root, _| {
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
    });
    fault.execute_batch("ROLLBACK").unwrap();
    wait_for_view_state(&paths, &key, &registry, &queued);

    let updated = slterm_workspace::ViewState::new(
        &key,
        serde_json::json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &registry,
    )
    .unwrap();
    let updated = slterm_sidebar::SideViewState::from_view_state(SideViewId::Nav, updated).unwrap();
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .update(cx, |view, _| view.replace_state_for_test(updated.clone()));
        });
    });
    tray_quit_until_closed(&root, &mut window, cx);
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let store = WorkspaceDatabase::open(&paths).unwrap();
    let stored = store.view_state(&key, &registry).unwrap().unwrap();
    assert_eq!(
        stored.state.state()["scroll"],
        serde_json::json!(84.0),
        "普通入队成功不得让终态通道中的旧 SideView 状态覆盖新状态"
    );
}

#[gpui::test]
fn close_retry_persists_side_view_changes_after_failure(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_view_state_write BEFORE INSERT ON view_states \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert!(!root.close_in_progress);
        assert!(!root.close_completed);
    });
    fault
        .execute_batch("DROP TRIGGER reject_view_state_write")
        .unwrap();

    let registry = super::super::side_view::side_view_state_registry();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let updated = slterm_workspace::ViewState::new(
        &key,
        serde_json::json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &registry,
    )
    .unwrap();
    let updated = slterm_sidebar::SideViewState::from_view_state(SideViewId::Nav, updated).unwrap();
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .update(cx, |view, _| view.replace_state_for_test(updated.clone()));
        });
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| assert!(root.close_completed));
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let store = WorkspaceDatabase::open(&paths).unwrap();
    let stored = store.view_state(&key, &registry).unwrap().unwrap();
    assert_eq!(stored.state.state()["scroll"], serde_json::json!(84.0));
}

#[gpui::test]
fn close_without_sidebar_edit_preserves_recovery_barrier(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .save_sidebar_state(&slterm_sidebar::SidebarState::default())
        .unwrap();
    drop(store);
    let raw_store = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    raw_store
        .execute(
            "UPDATE app_state SET sidebar_json = 'not-json' WHERE singleton = 1",
            [],
        )
        .unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| assert!(root.close_completed));
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let preserved = raw_store
        .query_row(
            "SELECT sidebar_json FROM app_state WHERE singleton = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(preserved, "not-json", "未编辑时不得回写恢复失败的默认态");
}

#[gpui::test]
fn close_without_sidebar_record_or_edit_preserves_absence(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::TrayQuit, window, cx)
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| assert!(root.close_completed));
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        store.sidebar_state().unwrap().is_none(),
        "无记录且未编辑时关闭不得创建默认侧栏记录"
    );
}

#[gpui::test]
fn close_without_writer_stops_remaining_shell_events(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx)
        })
    });

    let _ = sender.send(SltermShellEvent::TrayQuit);
    let _ = sender.send(SltermShellEvent::OpenDirectories(vec![
        "D:/work/after-close".to_owned(),
    ]));
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert!(root.snapshot().registry().projects().is_empty());
        assert!(root.close_completed);
    });
}

#[gpui::test]
fn closing_rejects_mouse_terminal_activation(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    root.update(cx, |root, _| root.close_in_progress = true);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_B), window, cx)
        })
    });

    assert!(matches!(result, Err(AppRootError::Closing)));
    root.read_with(cx, |root, _| {
        let active = root
            .snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group(group_id(GROUP_A))
            .unwrap()
            .active();
        assert_eq!(
            active,
            &ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_A),
            }
        );
    });
}

#[gpui::test]
fn closing_cancels_and_rejects_sidebar_drag(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-width-divider")
        .unwrap();
    let start = divider.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(
        gpui::point(start.x + gpui::px(40.0), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    root.read_with(cx, |root, _| assert!(root.sidebar_drag.is_some()));
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();

    window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));
    window.simulate_mouse_move(
        gpui::point(start.x + gpui::px(80.0), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );

    root.read_with(cx, |root, _| {
        assert!(root.sidebar_drag.is_none());
        assert_eq!(root.side_views.state().width(), 290);
    });
    fault.execute_batch("ROLLBACK").unwrap();
    window.run_until_parked();
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn close_stopped_writer_keeps_window(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    draw_frame(&mut window);

    let closed =
        window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));

    assert!(!closed);
    root.read_with(cx, |root, _| assert!(!root.close_in_progress));
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-empty").is_some());
}

#[gpui::test]
fn close_flush_reconciles_exited_generation(cx: &mut TestAppContext) {
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
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let weak = root.downgrade();
    window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));
    window.update(|_, cx| {
        let terminal = root.read_with(cx, |root, cx| {
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
        });
        terminal.update(cx, |_, cx| cx.emit(TerminalViewEvent::Exited));
    });
    fault.execute_batch("ROLLBACK").unwrap();

    window.run_until_parked();
    drop(root);
    cx.update(|_| {});
    weak.assert_released();
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    let snapshot = WorkspaceDatabase::open(&paths)
        .unwrap()
        .load_project_snapshot()
        .unwrap();
    assert_eq!(
        snapshot
            .terminal(terminal_id(TERMINAL_A))
            .unwrap()
            .run_intent(),
        RunIntent::Exited,
        "旧代次 flush 成功后必须重新提交 Exited 快照"
    );
}
