//! Document 在 AppRoot 的真实打开、跨项目身份与失败原子性。

use super::*;

fn open_document(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project: &str,
    group: &str,
    path: CanonicalPath,
) {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_document(project_id(project), group_id(group), path, window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
}

fn editor_text(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: &str,
    document: slterm_document::DocumentId,
) -> String {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(project)]
            .read(cx)
            .panels
            .values()
            .find(|panel| {
                panel.view_key
                    == ViewKey::Editor {
                        project_id: project_id(project),
                        document_id: document,
                    }
            })
            .and_then(|panel| panel.editor())
            .unwrap()
            .read(cx)
            .input_text_for_test(cx)
    })
}

#[gpui::test]
fn integration_document_global_identity(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = temp.path().to_path_buf();
    std::fs::create_dir(&root_a).unwrap();
    let file = root_a.join("shared.txt");
    std::fs::write(&file, "shared body").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let first = root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .find_map(|view| match view {
                ViewKey::Editor { document_id, .. } => Some(document_id),
                _ => None,
            })
            .unwrap()
    });
    assert_eq!(editor_text(&root, cx, PROJECT_A, first), "shared body");
    std::fs::write(file.as_path(), "external replacement").unwrap();

    // 同项目重复打开只还焦既有面板，不复制正文或 ViewKey。
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    let duplicate_focus = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find(|panel| {
                panel.view_key
                    == (ViewKey::Editor {
                        project_id: project_id(PROJECT_A),
                        document_id: first,
                    })
            })
            .unwrap()
            .focus_handle(cx)
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(duplicate_focus)
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_B), window, cx)
                .unwrap();
        });
    });
    open_document(&root, &mut window, PROJECT_B, GROUP_B, file);

    let second = root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_B))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .find_map(|view| match view {
                ViewKey::Editor { document_id, .. } => Some(document_id),
                _ => None,
            })
            .unwrap()
    });
    assert_eq!(second, first);
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert_eq!(editor_text(&root, cx, PROJECT_B, second), "shared body");
}

#[gpui::test]
fn failed_document_open_leaves_no_empty_panel(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_path = temp.path().join("project");
    std::fs::create_dir(&root_path).unwrap();
    let missing = CanonicalPath::new(root_path.join("missing.txt")).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_path.to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    open_document(&root, &mut window, PROJECT_A, GROUP_A, missing);

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
}

#[gpui::test]
fn invalid_document_target_rolls_back_registered_aggregate(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file.txt");
    std::fs::write(&file, "body").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    // 目标组不存在时，读盘与聚合注册已经发生，但布局、面板和聚合必须整体回滚。
    open_document(&root, &mut window, PROJECT_A, GROUP_B, file);

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
}

#[gpui::test]
fn document_open_completion_is_discarded_when_close_starts(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file.txt");
    std::fs::write(&file, "body").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_document(project_id(PROJECT_A), group_id(GROUP_A), file, window, cx)
                .unwrap();
            root.close_in_progress = true;
        });
    });
    window.run_until_parked();

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert!(root.read_with(cx, |root, _| root.pending_document_opens.is_empty()));
}

#[gpui::test]
fn document_registry_follows_shared_project_lifetime(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let project_a = temp.path().join("a");
    std::fs::create_dir(&project_a).unwrap();
    let file = project_a.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, project_a.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    add_project(&root, &mut window, PROJECT_B, temp.path().to_str().unwrap());
    open_document(&root, &mut window, PROJECT_B, GROUP_B, file);
    super::close::flush_when_ready(&runtime.client());
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);

    super::project_close::remove_project(&root, &mut window, PROJECT_A).unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert!(root.read_with(cx, |root, _| root.project_closes.is_empty()));

    super::project_close::remove_project(&root, &mut window, PROJECT_B).unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert!(root.read_with(cx, |root, _| {
        root.snapshot().registry().projects().is_empty()
    }));

    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn document_open_completion_respects_snapshot_write_barrier(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file.txt");
    std::fs::write(&file, "body").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_document(project_id(PROJECT_A), group_id(GROUP_A), file, window, cx)
                .unwrap();
            root.project_snapshot_recovery_failed = true;
        });
    });
    window.run_until_parked();

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
}
