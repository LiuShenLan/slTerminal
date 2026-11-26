// 根外 Document 授权占位的真实重授权入口。

/// 根外授权占位必须提供真实重新授权入口：显式点击后读盘、保留 DocumentId 并替换占位。
#[gpui::test]
fn outside_document_reauthorization_replaces_placeholder(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, "disk\n").unwrap();
    let outside = dunce::canonicalize(&outside).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location =
        slterm_path::ResolvedPath::new(CanonicalPath::new(outside.clone()).unwrap(), None);
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap(),
        )
        .unwrap();
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), document, location)
        .unwrap()
        .0;
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&layout)
        .unwrap();

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.documents.session(document).is_none()));
    let button = window
        .debug_bounds("document-recovery-reauthorize")
        .expect("授权占位必须提供可点击的重新授权按钮");
    window.simulate_click(button.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.documents
                .session(document)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "disk\n",
            "重授权必须读入真实磁盘内容且保留 DocumentId"
        );
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        assert_eq!(project.panels.len(), 1, "替换必须复用同一 Panel");
        assert!(
            project
                .panels
                .values()
                .any(|panel| panel.editor().is_some()),
            "授权成功后占位必须替换为真实编辑器"
        );
    });
    assert!(
        window
            .debug_bounds("document-recovery-authorization")
            .is_none()
    );
}

/// 重授权失败必须保留占位、显示脱敏失败态，并允许用户重试。
#[gpui::test]
fn failed_outside_document_reauthorization_keeps_placeholder(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, "disk\n").unwrap();
    let outside = dunce::canonicalize(&outside).unwrap();
    std::fs::remove_file(&outside).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location =
        slterm_path::ResolvedPath::new(CanonicalPath::new(outside.clone()).unwrap(), None);
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap(),
        )
        .unwrap();
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), document, location)
        .unwrap()
        .0;
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&layout)
        .unwrap();

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    let button = window
        .debug_bounds("document-recovery-reauthorize")
        .expect("授权占位必须提供可点击的重新授权按钮");
    window.simulate_click(button.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.documents.session(document).is_none()));
    assert!(
        window
            .debug_bounds("document-recovery-authorization")
            .is_some(),
        "失败不得移除授权占位"
    );
    assert!(
        window
            .debug_bounds("document-recovery-reauthorization-failed")
            .is_some(),
        "失败必须显示明确状态"
    );
    assert!(
        window
            .debug_bounds("document-recovery-reauthorize")
            .is_some(),
        "失败后必须允许重试"
    );
}
