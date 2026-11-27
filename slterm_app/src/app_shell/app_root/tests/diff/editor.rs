#[gpui::test]
fn editor_git_marker_opens_shared_diff(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("marker.txt"), "one\ntwo\nthree\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "marker.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "marker"]);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("marker.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "one\nchanged\nthree\nfour\n",
                window,
                cx,
            );
        });
    });
    draw_frame(&mut window);
    draw_frame(&mut window);

    let selector =
        Box::leak(format!("document-git-marker-{}-1", editor.entity_id()).into_boxed_str());
    let bounds = window
        .debug_bounds(selector)
        .expect("Editor 必须展示来自共享 Document 差分的 Git 行标");
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(&mut window);
    draw_frame(&mut window);

    let (_panel, diff) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                PanelContent::Diff(view) => Some((panel.id, view.clone())),
                _ => None,
            })
            .expect("行标必须打开 Diff 面板")
    });
    diff.read_with(cx, |view, app| {
        assert_eq!(view.shared_identity_for_test(), document);
        assert_eq!(
            view.shared_text_for_test(app),
            "one\nchanged\nthree\nfour\n"
        );
        assert_eq!(view.head_bytes_for_test().as_ref(), b"one\ntwo\nthree\n");
        assert_eq!(view.change_state_for_test(), (2, Some(0)));
    });
}

#[gpui::test]
fn editor_git_marks_reset_with_text_state(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|_, cx| {
        editor.update(cx, |view, cx| view.set_git_marks(
                vec![
                    crate::app_shell::text_view::GitMark { line: 1, kind: crate::app_shell::text_view::GitMarkKind::Modified },
                    crate::app_shell::text_view::GitMark { line: 3, kind: crate::app_shell::text_view::GitMarkKind::Added },
                ],
                cx,
            ));
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "x", window, cx);
        });
    });

    assert!(
        editor
            .read_with(cx, |view, _| view.git_marks_for_test())
            .is_empty(),
        "正文代次变化必须立即废弃旧行号，不能等待后台差分"
    );
}

#[gpui::test]
fn editor_git_marks_coalesce_latest_text(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("marker.txt"), "one\ntwo\nthree\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "marker.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "marker"]);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("marker.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    for text in [
        "one\nfirst\nthree\n",
        "one\nsecond\nthree\n",
        "one\ntwo\nchanged\n",
    ] {
        window.update(|window, cx| {
            editor.update(cx, |view, cx| {
                let length = view.input_text_for_test().len();
                gpui::EntityInputHandler::replace_text_in_range(
                    view,
                    Some(0..length),
                    text,
                    window,
                    cx,
                );
            });
        });
    }
    draw_frame(&mut window);
    draw_frame(&mut window);

    assert_eq!(
        editor.read_with(cx, |view, _| view.git_marks_for_test()),
        vec![(2, crate::app_shell::text_view::GitMarkKind::Modified)],
        "连续编辑只能采纳最新 TextStateId 的差分行标"
    );
}

#[gpui::test]
fn diff_edit_shared_document(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("shared.md")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let (document, markdown) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                panel.markdown().map(|view| {
                    let ViewKey::Markdown { document_id, .. } = panel.view_key else {
                        unreachable!()
                    };
                    (document_id, view.clone())
                })
            })
            .unwrap()
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            let editor = ViewKey::Editor {
                project_id: project_id(PROJECT_A),
                document_id: document,
            };
            let candidate = root
                .snapshot
                .with_opened_view(project_id(PROJECT_A), group_id(GROUP_A), editor)
                .unwrap();
            root.commit_project_snapshot(candidate, cx);
            root.restore_project_panels(project_id(PROJECT_A), window, cx);
        });
    });
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let panel = open_diff(&root, &mut window, document, &binding, "shared.md");
    draw_frame(&mut window);

    let diff = diff_view(&root, panel, cx);
    let right = diff.read_with(cx, |view, _| view.right_view().clone());
    window.update(|window, cx| {
        right.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "from diff\n",
                window,
                cx,
            );
        });
    });

    assert_eq!(
        root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .unwrap()
                .text()
                .unwrap()
                .to_string()
        }),
        "from diff\n"
    );
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "from diff\n"
    );
    assert_eq!(
        markdown.read_with(cx, |view, cx| view.input_text_for_test(cx)),
        "from diff\n"
    );
}

#[gpui::test]
fn editor_diff_share_document(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    let diff = diff_view(&root, panel, cx);
    let right = diff.read_with(cx, |view, _| view.right_view().clone());
    window.update(|window, cx| {
        right.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "from diff\n",
                window,
                cx,
            );
        });
    });
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "from diff\n"
    );
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(repository.join("first.txt")).unwrap(),
        "from diff\n"
    );
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));

    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "from editor\n",
                window,
                cx,
            );
        });
    });
    assert_eq!(
        diff.read_with(cx, |view, app| view.shared_text_for_test(app)),
        "from editor\n"
    );
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("first.txt")).unwrap(),
        "from editor\n"
    );
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
    assert_eq!(
        diff.read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n"
    );
}

#[gpui::test]
fn document_shared_across_projects(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    add_project(&root, &mut window, PROJECT_B, outside.to_str().unwrap());
    let panel = open_diff_in(
        &root,
        &mut window,
        DiffOpenRequest {
            project_id: project_id(PROJECT_B),
            target_group: Some(group_id(GROUP_B)),
            document_id: document,
            binding: binding.clone(),
            path: RepoPath::from_str("first.txt").unwrap(),
            focus: true,
            target_line: None,
        },
    );
    let diff = diff_view_in(&root, panel, PROJECT_B, cx);
    let right = diff.read_with(cx, |view, _| view.right_view().clone());
    window.update(|window, cx| {
        right.update(cx, |view, cx| {
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
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "shared draft\n"
    );

    activate_project(&root, &mut window, PROJECT_A);
    window.simulate_keystrokes("ctrl-z");
    assert_eq!(
        diff.read_with(cx, |view, app| view.shared_text_for_test(app)),
        "fixed first\n"
    );
    activate_project(&root, &mut window, PROJECT_B);
    window.simulate_keystrokes("ctrl-y");
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "shared draft\n"
    );

    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    let target = request.location().canonical_path().clone();
    let conflict = slterm_document::DocumentConflict::modified(
        request.location().clone(),
        std::sync::Arc::from(b"outside\n".as_slice()),
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.apply_document_external_result(
                document,
                target,
                true,
                slterm_document::DocumentExternalChange::Conflict { request, conflict }.into(),
                window,
                cx,
            );
        });
    });

    assert_eq!(
        root.read_with(cx, |root, _| {
            let session = root.documents.session(document).unwrap();
            (
                session.external_conflict().map(|conflict| conflict.kind()),
                session
                    .external_conflict()
                    .and_then(|conflict| conflict.disk_baseline())
                    .map(|baseline| baseline.to_vec()),
            )
        }),
        (
            Some(slterm_document::ExternalConflictKind::Modified),
            Some(b"outside\n".to_vec())
        )
    );
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "shared draft\n"
    );
    assert_eq!(
        diff.read_with(cx, |view, app| view.shared_text_for_test(app)),
        "shared draft\n"
    );
}

#[gpui::test]
fn integration_git_document(cx: &mut TestAppContext) {
    let (temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("marker.txt"), "one\ntwo\nthree\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "marker.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "marker"]);
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("marker.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
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
                "one\nchanged\nthree\nfour\n",
                window,
                cx,
            );
        });
    });
    draw_frame(&mut window);
    draw_frame(&mut window);
    let selector =
        Box::leak(format!("document-git-marker-{}-1", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(&mut window);
    let binding = current_binding(&repository);

    add_project(&root, &mut window, PROJECT_B, outside.to_str().unwrap());
    let panel = open_diff_in(
        &root,
        &mut window,
        DiffOpenRequest {
            project_id: project_id(PROJECT_B),
            target_group: Some(group_id(GROUP_B)),
            document_id: document,
            binding: binding.clone(),
            path: RepoPath::from_str("marker.txt").unwrap(),
            focus: true,
            target_line: None,
        },
    );
    let diff = diff_view_in(&root, panel, PROJECT_B, cx);
    let right = diff.read_with(cx, |view, _| view.right_view().clone());
    window.update(|window, cx| {
        right.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "integration save\n",
                window,
                cx,
            );
        });
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(repository.join("marker.txt")).unwrap(),
        "integration save\n"
    );
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "integration save\n"
    );

    activate_project(&root, &mut window, PROJECT_A);
    let editor_key = ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(editor_key, window, cx).unwrap();
        });
    });
    window.simulate_keystrokes("ctrl-z");
    assert_eq!(
        diff.read_with(cx, |view, app| view.shared_text_for_test(app)),
        "one\nchanged\nthree\nfour\n"
    );
    activate_project(&root, &mut window, PROJECT_B);
    window.simulate_keystrokes("ctrl-y");
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "integration save\n"
    );

    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    let target = request.location().canonical_path().clone();
    let conflict = slterm_document::DocumentConflict::modified(
        request.location().clone(),
        std::sync::Arc::from(b"outside\n".as_slice()),
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.apply_document_external_result(
                document,
                target,
                true,
                slterm_document::DocumentExternalChange::Conflict { request, conflict }.into(),
                window,
                cx,
            );
        });
    });
    assert!(root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_some()
    }));
    assert_eq!(
        diff.read_with(cx, |view, app| view.shared_text_for_test(app)),
        "integration save\n"
    );
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
}

#[gpui::test]
fn diff_save_uses_capability_union(cx: &mut TestAppContext) {
    let (temp, repository, _binding) = super::gitshow::repository();
    let work = repository.join("work");
    std::fs::create_dir(&work).unwrap();
    std::fs::write(work.join("first.txt"), "fixed first\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "work/first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "worktree"]);
    let binding = current_binding(&repository);
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let (root, mut window, _) = open_root(cx);
    // 项目写能力只覆盖仓库子目录；HEAD 只能经只读 RepoRoot 授权读取。
    add_project(&root, &mut window, PROJECT_B, work.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(work.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_B, GROUP_B, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    open_diff_in(
        &root,
        &mut window,
        DiffOpenRequest {
            project_id: project_id(PROJECT_B),
            target_group: Some(group_id(GROUP_B)),
            document_id: document,
            binding: binding.clone(),
            path: RepoPath::from_str("work/first.txt").unwrap(),
            focus: true,
            target_line: None,
        },
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_other_views_from_request(group_id(GROUP_B), BTreeMap::new(), window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
    add_project(&root, &mut window, PROJECT_A, outside.to_str().unwrap());
    let panel = open_diff_in(
        &root,
        &mut window,
        DiffOpenRequest {
            project_id: project_id(PROJECT_A),
            target_group: Some(group_id(GROUP_A)),
            document_id: document,
            binding: binding.clone(),
            path: RepoPath::from_str("work/first.txt").unwrap(),
            focus: true,
            target_line: None,
        },
    );
    assert!(root.read_with(cx, |root, _| {
        root.snapshot().registry().projects().iter().all(|project| {
            project
                .layout()
                .view_keys()
                .iter()
                .all(|view| matches!(view, ViewKey::Diff { .. }))
        })
    }));

    let diff = diff_view(&root, panel, cx);
    let right = diff.read_with(cx, |view, _| view.right_view().clone());
    window.update(|window, cx| {
        right.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "union save\n",
                window,
                cx,
            );
        });
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(work.join("first.txt")).unwrap(),
        "union save\n"
    );
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
}


/// 行标保留新增／修改／删除语义，而不是压平成单一强调色行号。
#[gpui::test]
fn diff_marks_classify_insert_replace_and_delete(cx: &mut TestAppContext) {
    use crate::app_shell::text_view::GitMarkKind;
    let _ = cx;
    let text_state = slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
    let commit = <slterm_document::CommitId as std::str::FromStr>::from_str("0123456789abcdef0123456789abcdef01234567")
        .unwrap();
    let classify = |head: &str, worktree: &str| {
        let input = slterm_document::DiffInput::git(
            std::sync::Arc::from(head.as_bytes()),
            std::sync::Arc::from(worktree.as_bytes()),
            slterm_document::DiffWhitespaceMode::Exact,
            text_state,
            commit.clone(),
            1,
        );
        let diff = slterm_document::compute_diff(&input).unwrap();
        crate::app_shell::app_root::diff::diff_change_marks(&diff)
    };

    let marks = classify("a\nb\nc\n", "a\nB\nc\nd\n");
    assert!(marks.contains(&(1, GitMarkKind::Modified)), "{marks:?}");
    assert!(marks.contains(&(3, GitMarkKind::Added)), "{marks:?}");

    let marks = classify("a\nb\nc\n", "a\nc\n");
    assert!(
        marks.iter().any(|(_, kind)| *kind == GitMarkKind::Deleted),
        "纯删除必须保留删除语义: {marks:?}"
    );
}

/// 越界旧行号不得进入软换行定位或触发 panic。
#[gpui::test]
fn out_of_range_git_marks_are_ignored_during_painting(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    editor.update(cx, |view, cx| {
        view.set_git_marks(
            vec![crate::app_shell::text_view::GitMark {
                line: usize::MAX - 1,
                kind: crate::app_shell::text_view::GitMarkKind::Added,
            }],
            cx,
        );
    });
    draw_frame(&mut window);
}

/// 行标显隐不得改变正文宽度或触发换行重排：gutter 槽位恒定。
#[gpui::test]
fn git_gutter_keeps_text_width_stable_when_marks_change(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    draw_frame(&mut window);
    let without_marks = editor.read_with(cx, |view, _| view.content_mask_width_for_test());

    editor.update(cx, |view, cx| {
        view.set_git_marks(
            vec![crate::app_shell::text_view::GitMark {
                line: 0,
                kind: crate::app_shell::text_view::GitMarkKind::Modified,
            }],
            cx,
        );
    });
    draw_frame(&mut window);
    let with_marks = editor.read_with(cx, |view, _| view.content_mask_width_for_test());

    editor.update(cx, |view, cx| view.set_git_marks(Vec::new(), cx));
    draw_frame(&mut window);
    let cleared = editor.read_with(cx, |view, _| view.content_mask_width_for_test());

    assert!(without_marks.is_some(), "前置：编辑器必须已绘制");
    assert_eq!(without_marks, with_marks, "行标出现不得改变正文宽度");
    assert_eq!(without_marks, cleared, "行标清空不得改变正文宽度");
}
