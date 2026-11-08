//! 项目关闭预检、空集合与焦点边界测试。

use super::close::flush_when_ready;
use super::project_close::remove_project;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::ProjectRegistry;

#[gpui::test]
fn project_close_rejects_concurrent_removal_without_side_effects(cx: &mut TestAppContext) {
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
    add_project(&root, &mut window, PROJECT_C, "D:/work/c");
    flush_when_ready(&runtime.client());

    assert!(remove_project(&root, &mut window, PROJECT_A).is_ok());
    let generation = root.read_with(cx, |root, _| root.snapshot_generation);
    assert!(matches!(
        remove_project(&root, &mut window, PROJECT_B),
        Err(AppRootError::ProjectCloseRunning)
    ));
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot_generation, generation);
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_B))
                .is_some()
        );
        assert_eq!(root.project_closes.len(), 1);
    });
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    assert!(matches!(
        state.borrow().sessions[&panel_b.sequence()].try_recv(),
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn empty_project_close_completes_without_close_objects(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    flush_when_ready(&runtime.client());

    assert!(remove_project(&root, &mut window, PROJECT_A).is_ok());
    let close_id = root.read_with(cx, |root, _| {
        root.project_closes[&project_id(PROJECT_A)]
            .persisted
            .close_id
            .clone()
    });
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    root.read_with(cx, |root, _| {
        assert!(root.project_closes.is_empty());
        assert!(root.snapshot().registry().projects().is_empty());
        assert_eq!(root.snapshot().registry().active(), None);
    });
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let completed = store.load_close_intent(&close_id).unwrap().unwrap();
    assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
    assert_eq!(completed.intent.objects().count(), 0);
    assert!(
        store
            .load_project_snapshot()
            .unwrap()
            .registry()
            .projects()
            .is_empty()
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn removing_active_project_restores_next_hidden_launch(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    registry
        .add_project(project_id(PROJECT_B), path("D:/work/b"))
        .unwrap();
    let mut snapshot = ProjectSnapshot::restore(registry, Vec::new()).unwrap();
    let (candidate, _) = snapshot
        .with_new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    snapshot = candidate;
    let (snapshot, _) = snapshot
        .with_new_terminal(terminal_spec(PROJECT_B, GROUP_B, TERMINAL_B))
        .unwrap();
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_project_snapshot(&snapshot).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);

    assert!(remove_project(&root, &mut window, PROJECT_B).is_ok());
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
    });
    assert_eq!(
        state.borrow().attempts,
        2,
        "下一项目的隐藏 Launch 必须按既有恢复语义启动"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
