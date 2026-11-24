#[gpui::test]
fn diff_uses_shared_document_from_commit(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                panel.editor().map(|_| match panel.view_key {
                    ViewKey::Editor { document_id, .. } => document_id,
                    _ => unreachable!(),
                })
            })
            .unwrap()
    });
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "shared draft\n",
                window,
                cx,
            );
        });
    });

    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);

    diff_view(&root, panel, cx).read_with(cx, |view, app| {
        assert_eq!(view.shared_identity_for_test(), document);
        assert_eq!(view.shared_text_for_test(app), "shared draft\n");
        assert_eq!(view.head_bytes_for_test().as_ref(), b"fixed first\n");
    });

    let reopened = open_diff_at(&root, &mut window, document, &binding, "first.txt", Some(0));
    draw_frame(&mut window);
    assert_eq!(reopened, panel);
    assert_eq!(
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.change_state_for_test()),
        (1, Some(0)),
        "重复入口必须定位请求所在的差异块"
    );
}

#[gpui::test]
fn diff_change_navigation_cycles_through_latest_diff(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("nav.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "nav.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "navigation"]);
    let binding = current_binding(&repository);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("nav.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let (document, editor) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                panel.editor().map(|editor| {
                    let ViewKey::Editor { document_id, .. } = panel.view_key else {
                        unreachable!()
                    };
                    (document_id, editor.clone())
                })
            })
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "ONE\ntwo\nthree\nfour\nFIVE\n",
                window,
                cx,
            );
        });
    });
    let panel = open_diff(&root, &mut window, document, &binding, "nav.txt");
    draw_frame(&mut window);
    draw_frame(&mut window);
    let diff = diff_view(&root, panel, cx);
    assert_eq!(
        diff.read_with(cx, |view, _| view.change_state_for_test()),
        (2, None)
    );
    for expected in [Some(0), Some(1), Some(0)] {
        window.update(|_, cx| {
            diff.update(cx, |view, cx| view.next_change(cx));
        });
        assert_eq!(
            diff.read_with(cx, |view, _| view.change_state_for_test().1),
            expected
        );
    }
    window.update(|_, cx| {
        diff.update(cx, |view, cx| view.previous_change(cx));
    });
    assert_eq!(
        diff.read_with(cx, |view, _| view.change_state_for_test().1),
        Some(1)
    );
}

#[gpui::test]
fn diff_bom_matches_decoded_document(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("bom.txt"), b"\xef\xbb\xbfsame\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "bom.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "bom"]);
    let binding = current_binding(&repository);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("bom.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "bom.txt");
    draw_frame(&mut window);
    draw_frame(&mut window);
    assert_eq!(
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.change_state_for_test()),
        (0, None),
        "HEAD 的 UTF-8 BOM 必须与工作区解码语义一致"
    );
}

#[gpui::test]
fn diff_head_failure_still_reveals_requested_worktree_line(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("broken.txt"), "one\ntwo\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "broken.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "broken"]);
    let binding = current_binding(&repository);
    let output = super::gitshow::git(&repository, &["rev-parse", "HEAD:broken.txt"]);
    let object = std::str::from_utf8(&output.stdout).unwrap().trim();
    std::fs::remove_file(
        repository
            .join(".git/objects")
            .join(&object[..2])
            .join(&object[2..]),
    )
    .unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("broken.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff_at(
        &root,
        &mut window,
        document,
        &binding,
        "broken.txt",
        Some(1),
    );
    draw_frame(&mut window);
    let diff = diff_view(&root, panel, cx);
    assert_eq!(
        diff.read_with(cx, |view, app| view.transients_for_test(app).0),
        1,
        "HEAD 不可用时仍须降级定位共享工作区"
    );
}

#[gpui::test]
fn failed_diff_open_does_not_capture_binding(cx: &mut TestAppContext) {
    let (_temp, repository, first_binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_diff(
                DiffOpenRequest {
                    project_id: project_id(PROJECT_A),
                    target_group: Some(group_id(GROUP_B)),
                    document_id: document,
                    binding: first_binding,
                    path: RepoPath::from_str("first.txt").unwrap(),
                    focus: true,
                    target_line: None,
                },
                window,
                cx,
            )
        })
    });
    assert!(rejected.is_err());

    std::fs::write(repository.join("first.txt"), b"second head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "second"]);
    let second_binding = current_binding(&repository);
    let panel = open_diff(&root, &mut window, document, &second_binding, "first.txt");
    draw_frame(&mut window);

    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"second head\n",
        "失败请求不得把未建立面板的固定绑定留给后续打开"
    );
}

