#[gpui::test]
fn integration_cross_project_views(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let file = root_b.join("shared.txt");
    std::fs::write(&file, "fn main() {\n1\n2\n3\n}\n").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);

    let response = open_file_handover(&root, &mut window, file.as_path()).unwrap();
    assert!(response.is_accepted());
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.registry().active()),
        Some(project_id(PROJECT_B)),
        "OpenFile 必须激活覆盖文件的最深既有根"
    );
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor_b = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_B)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    select_editor_language_override(cx, &mut window, &editor_b, Some(LanguageId::Python));
    window.update(|_, cx| editor_b.update(cx, |view, _| view.set_scroll_for_test(2, 3)));

    activate_project(&root, &mut window, PROJECT_A);
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let editor_a = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    select_editor_language_override(cx, &mut window, &editor_a, Some(LanguageId::Rust));
    window.update(|_, cx| editor_a.update(cx, |view, _| view.set_scroll_for_test(1, 1)));
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert_eq!(
        editor_text(&root, cx, PROJECT_A, document),
        "fn main() {\n1\n2\n3\n}\n"
    );
    assert_eq!(
        editor_text(&root, cx, PROJECT_B, document),
        "fn main() {\n1\n2\n3\n}\n"
    );
    flush_when_ready(&runtime.client());

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    flush_when_ready(&runtime.client());
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let read_view = |project: &str| {
        root.read_with(cx, |root, cx| {
            let view = root.projects[&project_id(project)]
                .read(cx)
                .panels
                .values()
                .find_map(|panel| panel.editor().cloned())
                .unwrap();
            let view = view.read(cx);
            (view.scroll_state_for_test(), view.language_state_for_test())
        })
    };
    let (a_scroll, a_language) = read_view(PROJECT_A);
    assert_eq!((a_scroll.0, a_scroll.1), (1, 1));
    assert_eq!(a_language, (Some(LanguageId::Rust), LanguageId::Rust));
    activate_project(&root, &mut window, PROJECT_B);
    let (b_scroll, b_language) = read_view(PROJECT_B);
    assert_eq!((b_scroll.0, b_scroll.1), (2, 3));
    assert_eq!(b_language, (Some(LanguageId::Python), LanguageId::Python));
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn open_file_handover_failures_preserve_snapshot_and_panels(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, "outside").unwrap();
    let unsupported = root_b.join("image.png");
    std::fs::write(&unsupported, [0_u8]).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    for (path, expected) in [
        (
            root_b.join("missing.txt"),
            slterm_runtime_protocol::DomainErrorCode::NotReady,
        ),
        (
            outside,
            slterm_runtime_protocol::DomainErrorCode::Unauthorized,
        ),
        (
            unsupported,
            slterm_runtime_protocol::DomainErrorCode::InvalidRequest,
        ),
    ] {
        let error = open_file_handover(&root, &mut window, &path).unwrap_err();
        assert_eq!(error.data.unwrap().code, expected);
        assert!(!error.message.contains(path.to_str().unwrap()));
        assert_eq!(
            root.read_with(cx, |root, _| root.snapshot().clone()),
            before
        );
        assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 0);
    }
    let relative = std::path::PathBuf::from("relative.txt");
    let error = open_file_handover(&root, &mut window, &relative).unwrap_err();
    assert_eq!(
        error.data.unwrap().code,
        slterm_runtime_protocol::DomainErrorCode::InvalidRequest
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
}

#[gpui::test]
fn open_file_handover_duplicate_focuses_existing_view(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());

    assert!(
        open_file_handover(&root, &mut window, &file)
            .unwrap()
            .is_accepted()
    );
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let view = ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    let (panel, editor_focus) = root.read_with(cx, |root, cx| {
        let panel = root
            .panel_for_view(project_id(PROJECT_A), &view, cx)
            .unwrap();
        let editor = root.projects[&project_id(PROJECT_A)].read(cx).panels[&panel]
            .editor()
            .unwrap()
            .clone();
        (panel, editor.read(cx).focus_handle(cx))
    });
    window.update(|window, _| window.blur());

    assert!(
        open_file_handover(&root, &mut window, &file)
            .unwrap()
            .is_accepted()
    );

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root
            .panel_for_view(project_id(PROJECT_A), &view, cx)
            .unwrap()),
        panel
    );
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(editor_focus)
    );
}

#[gpui::test]
fn open_file_handover_close_settles_active_and_pending_requests(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let files = [project.join("a.txt"), project.join("b.txt")];
    for file in &files {
        std::fs::write(file, "body").unwrap();
    }
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let mut responses = Vec::new();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            for file in &files {
                let (completion, response) = std::sync::mpsc::sync_channel(1);
                root.handle_shell_event(
                    SltermShellEvent::Handover {
                        params: slterm_runtime_protocol::HandoverParams::OpenFile {
                            path: file.to_str().unwrap().to_owned(),
                        },
                        completion,
                    },
                    window,
                    cx,
                );
                responses.push(response);
            }
            root.close_in_progress = true;
        });
    });
    window.run_until_parked();

    for response in responses {
        let error = response
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap()
            .unwrap_err();
        assert_eq!(
            error.data.unwrap().code,
            slterm_runtime_protocol::DomainErrorCode::Closing
        );
        assert_eq!(
            response.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected),
            "唯一回执后不得残留 sender 或迟到二次回执"
        );
    }
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
fn open_file_handover_empty_project_fifo_reuses_committed_first_group(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let files = [project.join("a.txt"), project.join("b.txt")];
    for file in &files {
        std::fs::write(file, "body").unwrap();
    }
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let mut responses = Vec::new();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            for file in &files {
                let (completion, response) = std::sync::mpsc::sync_channel(1);
                root.handle_shell_event(
                    SltermShellEvent::Handover {
                        params: slterm_runtime_protocol::HandoverParams::OpenFile {
                            path: file.to_str().unwrap().to_owned(),
                        },
                        completion,
                    },
                    window,
                    cx,
                );
                responses.push(response);
            }
        });
    });
    window.run_until_parked();

    for response in responses {
        let result = response
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert!(result.unwrap().is_accepted(), "同空项目排队打开都必须成功");
    }
    root.read_with(cx, |root, cx| {
        let project = root
            .snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap();
        assert_eq!(project.layout().group_ids().len(), 1);
        assert_eq!(project.layout().view_keys().len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.documents.len(), 2);
    });
}

#[gpui::test]
fn open_file_handover_existing_hidden_view_synchronizes_terminal_visibility(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let file = root_b.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    assert!(
        open_file_handover(&root, &mut window, &file)
            .unwrap()
            .is_accepted()
    );
    activate_project(&root, &mut window, PROJECT_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    assert!(output_visible(&root, cx, PROJECT_A, TERMINAL_A));
    assert!(!output_visible(&root, cx, PROJECT_B, TERMINAL_B));

    assert!(
        open_file_handover(&root, &mut window, &file)
            .unwrap()
            .is_accepted()
    );

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.registry().active()),
        Some(project_id(PROJECT_B))
    );
    assert!(!output_visible(&root, cx, PROJECT_A, TERMINAL_A));
    assert!(output_visible(&root, cx, PROJECT_B, TERMINAL_B));
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
}

#[gpui::test]
fn open_file_handover_failure_preserves_shared_document_and_existing_editor(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let file = root_b.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    window.update(|_, cx| {
        root.update(cx, |root, _| {
            root.focus_manager
                .fail_terminal_request_focus_after_activation_for_test();
        });
    });

    let error = open_file_handover(&root, &mut window, file.as_path()).unwrap_err();

    assert_eq!(
        error.data.unwrap().code,
        slterm_runtime_protocol::DomainErrorCode::NotReady
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "shared");
    assert_eq!(
        root.read_with(cx, |root, cx| {
            root.projects[&project_id(PROJECT_A)]
                .read(cx)
                .panels
                .values()
                .find_map(|panel| panel.editor().cloned())
                .unwrap()
                .entity_id()
        }),
        editor.entity_id()
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );
}

#[gpui::test]
fn open_file_handover_modal_activation_rolls_back_hidden_project(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let file = root_b.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    open_side_menu_for_test(&mut window);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused = window.update(|window, cx| window.focused(cx));

    let error = open_file_handover(&root, &mut window, &file).unwrap_err();

    assert_eq!(
        error.data.unwrap().code,
        slterm_runtime_protocol::DomainErrorCode::Unauthorized
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );
    assert_eq!(window.update(|window, cx| window.focused(cx)), focused);
}

#[gpui::test]
fn open_file_handover_late_focus_failure_restores_project_and_focus(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let file = root_b.join("shared.txt");
    std::fs::write(&file, "shared").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, _, terminal) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let terminal_focus = terminal
        .read_with(cx, |terminal, cx| terminal.focus_handle(cx))
        .unwrap();
    window.update(|window, cx| terminal_focus.focus(window, cx));
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    window.update(|_, cx| {
        root.update(cx, |root, _| {
            root.focus_manager
                .fail_terminal_request_focus_after_activation_for_test();
        });
    });

    let error = open_file_handover(&root, &mut window, &file).unwrap_err();

    assert_eq!(
        error.data.unwrap().code,
        slterm_runtime_protocol::DomainErrorCode::NotReady
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.focus_manager.state_for_test().0),
        Some(project_id(PROJECT_A))
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(terminal_focus)
    );
}

/// 同一真实文件的不同词法路径（硬链接）必须复用同一 Document 与面板。
#[gpui::test]
fn path_file_identity_deduplicates_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("file.txt");
    let alias = project_root.join("alias.txt");
    std::fs::write(&file, "shared\n").unwrap();
    std::fs::hard_link(&file, &alias).unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );

    super::documents::open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
    );
    super::documents::open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(dunce::canonicalize(&alias).unwrap()).unwrap(),
    );

    root.read_with(cx, |root, cx| {
        assert_eq!(root.documents.len(), 1, "文件身份必须唯一");
        assert_eq!(
            root.panel_count(project_id(PROJECT_A), cx),
            1,
            "同一真实文件的别名不得创建第二个面板"
        );
    });
}
