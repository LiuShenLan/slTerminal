#[gpui::test]
fn diff_restore_uses_authorization_placeholder(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(temp.path().join("outside.txt")).unwrap(),
        None,
    );
    persist_diff_layout(&paths, &repository, document, location, &binding);

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.documents.session(document).is_none()));
    assert_diff_recovery_placeholder(&root, &mut window, cx, "document-recovery-authorization");
}

#[gpui::test]
fn diff_restore_uses_replaced_placeholder(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let file = repository.join("replaced.txt");
    std::fs::write(&file, "old clean").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    crate::atomic_file::write(&file, b"new clean").unwrap();
    let document = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    persist_diff_layout(&paths, &repository, document, location, &binding);

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.documents.session(document).is_none()));
    assert_diff_recovery_placeholder(&root, &mut window, cx, "document-recovery-replaced");
}

#[gpui::test]
fn diff_restore_uses_paged_placeholder(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let file = repository.join("large.txt");
    std::fs::write(
        &file,
        vec![b'x'; slterm_document::EDITABLE_LIMIT_BYTES as usize + 1],
    )
    .unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let document = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    persist_diff_layout(&paths, &repository, document, location, &binding);

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().paged().is_some()
    }));
    assert_diff_recovery_placeholder(&root, &mut window, cx, "document-recovery-paged");
}

#[gpui::test]
fn diff_view_state_restore_clears_transients(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);
    let diff = diff_view(&root, panel, cx);
    let expected = crate::app_shell::text_view::DiffViewState::new(
        slterm_document::DiffBinding::new(binding.clone(), "first.txt".parse().unwrap()),
    )
    .with_mode(crate::app_shell::text_view::DiffViewMode::Inline)
    .with_sync_scroll(false)
    .with_column_width(720)
    .with_collapsed(Some(crate::app_shell::text_view::DiffCollapsedSide::Head))
    .with_swapped(true)
    .with_whitespace(slterm_document::DiffWhitespaceMode::All)
    .with_collapse_unchanged(true)
    .with_source(crate::app_shell::text_view::EditorViewState::default().with_wrap(true));
    window.update(|window, cx| {
        diff.update(cx, |view, cx| {
            view.set_view_state_for_test(expected.clone(), window, cx)
        });
    });
    let key = ViewKey::Diff {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    let registry = super::super::side_view::workspace_view_state_registry();
    let mut with_transient = serde_json::to_value(&expected).unwrap();
    with_transient["search"] = serde_json::json!("transient");
    assert!(slterm_workspace::ViewState::new(&key, with_transient, &registry).is_err());
    super::close::flush_when_ready(&runtime.client());

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    std::fs::write(repository.join("first.txt"), b"later head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "later"]);

    let persisted =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persisted);
    draw_frame(&mut window);
    let restored_panel = root.read_with(cx, |root, cx| {
        root.panel_for_view(project_id(PROJECT_A), &key, cx)
            .unwrap()
    });
    let restored = diff_view(&root, restored_panel, cx);
    restored.read_with(cx, |view, app| {
        assert_eq!(view.view_state_for_test(), expected);
        assert_eq!(view.shared_text_for_test(app), "later head\n");
        assert_eq!(view.transients_for_test(app), (0, 0..0));
        assert_eq!(view.head_bytes_for_test().as_ref(), b"fixed first\n");
    });
    let focused = window.update(|window, cx| {
        let focus = restored.read(cx).focus_handle(cx);
        window.focused(cx).is_some_and(|actual| actual == focus)
    });
    assert!(!focused);
}
