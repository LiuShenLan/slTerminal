//! 项目移除的持久关闭、资源回收与重试测试。

use super::close::flush_when_ready;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{ClosePhase, CloseStage, ViewState};

pub(super) fn remove_project(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project: &str,
) -> Result<CommandOutcome, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::RemoveProject,
                CommandSource::Application,
                CommandTarget::Project(project_id(project)),
                None,
                window,
                cx,
            )
        })
    })
}

/// Save 阶段代次失配必须重收集：显式重试带入的新决定生效，不复活旧意图。
#[gpui::test]
fn project_remove_save_revision_mismatch_recollects_with_new_decisions(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    let project_dir = temp.path().join("recollect-project");
    std::fs::create_dir(&project_dir).unwrap();
    let file = project_dir.join("editor.txt");
    std::fs::write(
        &file, "old
",
    )
    .unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| {
                location
                    .canonical_path()
                    .as_path()
                    .starts_with(&project_dir)
                    .then_some(id)
            })
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "first ", window, cx);
        });
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.remove_project_with_decisions(
                project_id(PROJECT_A),
                std::collections::BTreeMap::from([(
                    document,
                    slterm_workspace::CloseDecision::Save,
                )]),
                window,
                cx,
            )
            .unwrap();
        });
    });
    // 保存请求已按旧代次捕获；保存期间再次编辑使写后复核失配。
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "second ", window, cx);
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_A)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::Save),
            "代次失配必须停在 Save 失败"
        );
        assert!(close.needs_recollect, "代次失配必须标记重收集");
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.remove_project_with_decisions(
                project_id(PROJECT_A),
                std::collections::BTreeMap::from([(
                    document,
                    slterm_workspace::CloseDecision::Discard,
                )]),
                window,
                cx,
            )
            .unwrap();
        });
    });
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_A)];
        assert!(!close.needs_recollect);
        assert_ne!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::Save),
            "重收集后不得再次停在同一代次失配"
        );
    });

    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn project_remove_commits_intent_before_destroy(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel_a = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let project_b = temp.path().join("project-b");
    std::fs::create_dir(&project_b).unwrap();
    let file_b = project_b.join("editor.txt");
    std::fs::write(&file_b, "body").unwrap();
    let file_b = CanonicalPath::new(dunce::canonicalize(&file_b).unwrap()).unwrap();
    add_project(&root, &mut window, PROJECT_B, project_b.to_str().unwrap());
    let panel_b = new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    super::documents::open_document(&root, &mut window, PROJECT_B, GROUP_B, file_b);
    let editor_b = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_B)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    editor_b.update(cx, |editor, _| editor.set_scroll_for_test(2, 3));
    let document_b = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| {
                (location.canonical_path().as_path().starts_with(&project_b)).then_some(id)
            })
            .unwrap()
    });
    let editor_key = ViewKey::Editor {
        project_id: project_id(PROJECT_B),
        document_id: document_b,
    };
    flush_when_ready(&runtime.client());
    let registry = super::super::side_view::workspace_view_state_registry();
    let nav_key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let nav_state = ViewState::new(
        &nav_key,
        serde_json::json!({
            "projects": [
                {"project_id": PROJECT_A, "expanded": true, "history_expanded": false},
                {"project_id": PROJECT_B, "expanded": false, "history_expanded": true}
            ],
            "unassigned_expanded": true,
            "scroll": 12.0
        }),
        &registry,
    )
    .unwrap();
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_view_state(&nav_key, &nav_state).unwrap();
    let nav_state =
        slterm_sidebar::SideViewState::from_view_state(SideViewId::Nav, nav_state).unwrap();
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .update(cx, |view, _| view.replace_state_for_test(nav_state.clone()));
        });
    });
    let (_, _, weak_b) = panel_entity(&root, cx, PROJECT_B, TERMINAL_B);
    let focus_a = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });

    let accepted = remove_project(&root, &mut window, PROJECT_B);
    assert!(accepted.is_ok(), "项目移除应异步接受: {accepted:?}");
    let close_id = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .persisted
            .close_id
            .clone()
    });
    let blocked =
        window.update(|window, cx| root.update(cx, |root, cx| root.request_close(window, cx)));
    assert!(!blocked, "项目关闭存活时不得销毁窗口");

    window.run_until_parked();
    flush_when_ready(&runtime.client());
    window.update(|window, cx| assert_eq!(window.focused(cx), Some(focus_a)));

    root.read_with(cx, |root, cx| {
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_B))
                .is_none()
        );
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert!(!root.projects.contains_key(&project_id(PROJECT_B)));
        assert!(root.project_closes.is_empty());
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&panel_a.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(weak_b.upgrade().is_none());

    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_none()
    );
    assert_eq!(
        store
            .load_close_intent(&close_id)
            .unwrap()
            .unwrap()
            .commit_status,
        CloseCommitStatus::Completed
    );
    assert!(!store.metadata().unwrap().unwrap().clean_shutdown);
    let stored_nav = store.view_state(&nav_key, &registry).unwrap().unwrap();
    assert_eq!(
        stored_nav.state.state()["projects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        stored_nav.state.state()["projects"][0]["project_id"],
        serde_json::json!(PROJECT_A)
    );
    assert!(store.view_state(&editor_key, &registry).unwrap().is_none());
    editor_b.update(cx, |editor, _| editor.set_scroll_for_test(8, 8));
    flush_when_ready(&runtime.client());
    assert!(
        store.view_state(&editor_key, &registry).unwrap().is_none(),
        "已移除项目的迟到 Editor 检查点不得复活 ViewState"
    );
    drop(editor_b);
    flush_when_ready(&runtime.client());
    assert!(store.view_state(&editor_key, &registry).unwrap().is_none());
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    let after_drop = store.view_state(&nav_key, &registry).unwrap().unwrap();
    assert_eq!(
        after_drop.state.state()["projects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        after_drop.state.state()["projects"][0]["project_id"],
        serde_json::json!(PROJECT_A)
    );
}

#[gpui::test]
fn project_cleanup_receipt_delay_cannot_revive_editor_view_state(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let project_b = temp.path().join("project-b");
    std::fs::create_dir(&project_b).unwrap();
    let file_b = project_b.join("editor.txt");
    std::fs::write(&file_b, "body").unwrap();
    let file_b = CanonicalPath::new(dunce::canonicalize(&file_b).unwrap()).unwrap();
    add_project(&root, &mut window, PROJECT_B, project_b.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_B, GROUP_B, file_b);
    let editor_b = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_B)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    editor_b.update(cx, |editor, _| editor.set_scroll_for_test(2, 3));
    let document_b = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor_key = ViewKey::Editor {
        project_id: project_id(PROJECT_B),
        document_id: document_b,
    };
    let registry = super::super::side_view::workspace_view_state_registry();
    flush_when_ready(&runtime.client());

    remove_project(&root, &mut window, PROJECT_B).unwrap();
    let close_id = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .persisted
            .close_id
            .clone()
    });

    // 逐任务推进到清理请求已发出，但不消费其前台完成回执。
    let executor = cx.executor();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_B)]
            .persisted
            .commit_status
    }) != CloseCommitStatus::PtyStopped
    {
        assert!(std::time::Instant::now() < deadline, "项目关闭未进入清理");
        if !executor.tick() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    while store
        .load_close_intent(&close_id)
        .unwrap()
        .unwrap()
        .commit_status
        != CloseCommitStatus::CleanupCommitted
    {
        assert!(std::time::Instant::now() < deadline, "项目清理事务未提交");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.project_closes[&project_id(PROJECT_B)]
                .persisted
                .commit_status
        }),
        CloseCommitStatus::PtyStopped,
        "测试必须停在清理回执尚未处理的窗口"
    );

    editor_b.update(cx, |editor, _| editor.set_scroll_for_test(8, 8));
    flush_when_ready(&runtime.client());
    assert!(
        store.view_state(&editor_key, &registry).unwrap().is_none(),
        "StateCommitted 后的迟到检查点不得穿过清理窗口"
    );
    drop(editor_b);
    flush_when_ready(&runtime.client());
    assert!(store.view_state(&editor_key, &registry).unwrap().is_none());

    window.run_until_parked();
    assert_eq!(
        store
            .load_close_intent(&close_id)
            .unwrap()
            .unwrap()
            .commit_status,
        CloseCommitStatus::Completed
    );
    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn project_commit_failure_rolls_back_and_same_command_retries(cx: &mut TestAppContext) {
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
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_project_commit BEFORE INSERT ON close_intents \
             BEGIN SELECT RAISE(ABORT, 'injected project commit failure'); END;",
        )
        .unwrap();

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    root.read_with(cx, |root, _| {
        assert!(root.project_closes.is_empty());
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_B))
                .is_some()
        );
        assert!(root.projects.contains_key(&project_id(PROJECT_B)));
    });
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_some()
    );
    assert_eq!(
        fault
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );

    fault
        .execute_batch("DROP TRIGGER reject_project_commit")
        .unwrap();
    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_none()
    );
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn project_cleanup_failure_retries_without_rejoining_stopped_sessions(cx: &mut TestAppContext) {
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
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(&format!(
            "CREATE TRIGGER reject_project_cleanup BEFORE DELETE ON projects \
             WHEN OLD.project_id = '{PROJECT_B}' \
             BEGIN SELECT RAISE(ABORT, 'injected project cleanup failure'); END;"
        ))
        .unwrap();

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    root.read_with(cx, |root, _| {
        let close = &root.project_closes[&project_id(PROJECT_B)];
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
        assert_eq!(close.persisted.commit_status, CloseCommitStatus::PtyStopped);
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_B))
                .is_none()
        );
    });
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .project(project_id(PROJECT_B))
            .is_some()
    );

    fault
        .execute_batch("DROP TRIGGER reject_project_cleanup")
        .unwrap();
    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
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
    let retry_message = state.borrow().sessions[&panel_b.sequence()].try_recv();
    assert!(
        matches!(
            retry_message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected)
        ),
        "清理重试不得再次 join 已停止会话: {retry_message:?}"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
