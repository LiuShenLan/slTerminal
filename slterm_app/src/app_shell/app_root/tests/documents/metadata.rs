/// 行尾切换与手动只读经 Editor 按钮可达，并写入共享 Document。
#[gpui::test]
fn editor_line_ending_and_manual_read_only_are_live(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("metadata.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    draw_frame(&mut window);
    let entity = editor.entity_id();
    for selector in [
        format!("document-line-ending-{entity}"),
        format!("document-read-only-{entity}"),
    ] {
        let selector: &'static str = Box::leak(selector.into_boxed_str());
        assert!(
            window.debug_bounds(selector).is_some(),
            "Editor 底栏必须提供 {selector}"
        );
    }

    root.update(cx, |root, cx| root.toggle_document_line_ending(document, cx));
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.encoding().line_ending, slterm_document::LineEnding::Lf);
        assert!(session.is_dirty(), "行尾切换必须进入 Document 脏态");
    });
    root.update(cx, |root, cx| root.toggle_document_line_ending(document, cx));
    root.read_with(cx, |root, _| {
        assert!(!root.documents.session(document).unwrap().is_dirty());
    });

    root.update(cx, |root, cx| {
        root.toggle_document_manual_read_only(document, cx)
    });
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert!(session.is_manually_read_only());
        assert!(session.is_read_only());
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, Some(0..0), "x", window, cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.documents
                .session(document)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "old",
            "手动只读必须拒绝编辑"
        );
    });
    root.update(cx, |root, cx| {
        root.toggle_document_manual_read_only(document, cx)
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, Some(0..0), "x", window, cx);
        });
    });
    root.read_with(cx, |root, _| {
        assert!(
            root.documents
                .session(document)
                .unwrap()
                .text()
                .unwrap()
                .to_string()
                .starts_with('x')
        );
    });
}
