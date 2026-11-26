/// 未知编码打开失败必须提供显式编码重开；选中后建立真实可编辑 Document。
#[gpui::test]
fn unknown_encoding_prompts_for_explicit_reopen(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    // GBK 编码的“中文\n”，不是合法 UTF-8。
    let file = project.join("gbk.txt");
    std::fs::write(&file, [0xd6, 0xd0, 0xce, 0xc4, b'\n']).unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());

    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);

    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.encoding_prompt.is_some()),
        "未知编码必须打开编码选择面"
    );
    assert!(
        root.read_with(cx, |root, _| root.documents.is_empty()),
        "解码失败不得建立可保存正文"
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0,
        "编码选择前不得留下空面板"
    );
    assert!(window.debug_bounds("encoding-prompt-dialog").is_some());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.resolve_encoding_prompt(slterm_document::DocumentEncoding::Gbk, window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.encoding_prompt.is_none()));
    let document = root.read_with(cx, |root, _| {
        root.snapshot().documents().next().unwrap().0
    });
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.text().unwrap().to_string(), "中文\n");
        assert_eq!(
            session.encoding().encoding,
            slterm_document::DocumentEncoding::Gbk
        );
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "中文\n");
}

/// 显式编码仍不合法时保留选择面且不建立面板；取消不改变布局。
#[gpui::test]
fn wrong_encoding_keeps_prompt_and_cancel_leaves_layout_unchanged(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("gbk.txt");
    std::fs::write(&file, [0xd6, 0xd0, 0xce, 0xc4, b'\n']).unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    draw_frame(&mut window);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            // 奇数长度字节流不是合法 UTF-16 LE，重开必然失败。
            root.resolve_encoding_prompt(
                slterm_document::DocumentEncoding::Utf16Le,
                window,
                cx,
            )
            .unwrap();
        });
    });
    window.run_until_parked();

    assert!(
        root.read_with(cx, |root, _| root.encoding_prompt.is_some()),
        "错误编码必须回到编码选择面，而不是静默失败"
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert_eq!(root.read_with(cx, |root, _| root.snapshot().clone()), before);

    window.update(|window, cx| {
        root.update(cx, |root, cx| root.cancel_encoding_prompt(window, cx).unwrap());
    });
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.encoding_prompt.is_none()));
    assert_eq!(root.read_with(cx, |root, _| root.snapshot().clone()), before);
}

/// 二进制内容不是编码歧义，不得弹出编码选择面。
#[gpui::test]
fn binary_content_is_rejected_without_encoding_prompt(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("binary.bin.txt");
    std::fs::write(&file, b"text\0binary").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());

    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);

    assert!(root.read_with(cx, |root, _| root.encoding_prompt.is_none()));
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
}

/// 超限 GBK 文件同样经显式编码进入分页只读视图。
#[gpui::test]
fn oversized_gbk_file_reopens_as_paged_through_explicit_encoding(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("large-gbk.txt");
    let unit = [0xd6, 0xd0, 0xce, 0xc4, b'\n'];
    let mut bytes = Vec::new();
    while (bytes.len() as u64) <= slterm_document::EDITABLE_LIMIT_BYTES {
        bytes.extend_from_slice(&unit);
    }
    std::fs::write(&file, bytes).unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());

    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.encoding_prompt.is_some()));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.resolve_encoding_prompt(slterm_document::DocumentEncoding::Gbk, window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();

    let document = root.read_with(cx, |root, _| {
        root.snapshot().documents().next().unwrap().0
    });
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert!(session.paged().is_some(), "超限 GBK 必须建立分页正文");
        assert_eq!(
            session.encoding().encoding,
            slterm_document::DocumentEncoding::Gbk
        );
        assert!(session.is_read_only());
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
}
