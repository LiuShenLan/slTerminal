fn wait_for_document_watch(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    source: &std::path::Path,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.fs_watch
            .as_ref()
            .is_some_and(|watch| watch.tracks_file(source))
    }) {
        assert!(std::time::Instant::now() < deadline, "watcher 未登记源文件");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn wait_for_document_relocation(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    document: slterm_document::DocumentId,
    target: &std::path::Path,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        window
            .executor()
            .advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        let relocated = root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .is_some_and(|session| session.location().canonical_path().as_path() == target)
        });
        if relocated {
            return;
        }
        let diagnostics = root.read_with(cx, |root, _| {
            (
                root.pending_document_relocations.len(),
                root.fs_watch.as_ref().map(|watch| watch.has_interest()),
                root.tasks
                    .is_registered(super::super::task_service::TaskId::DocumentExternal(
                        document,
                    )),
                root.documents
                    .session(document)
                    .map(|session| session.location().canonical_path().as_path().to_path_buf()),
                root.document_relocation_aliases.clone(),
            )
        });
        assert!(
            std::time::Instant::now() < deadline,
            "真实 watcher 改名必须到达 AppRoot: {diagnostics:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui::test]
fn editor_external_rename_keeps_view_type(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = root_a.join("nested");
    std::fs::create_dir_all(&root_b).unwrap();
    let source = root_b.join("source.txt");
    let target = root_b.join("renamed.md");
    std::fs::write(&source, "same").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    open_document(
        &root,
        &mut window,
        PROJECT_B,
        GROUP_B,
        CanonicalPath::new(&source).unwrap(),
    );
    let (document, panel, editor, original_view) = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_B)].read(cx);
        let (panel_id, panel, editor) = project
            .panels
            .iter()
            .find_map(|(id, panel)| panel.editor().map(|editor| (id, panel, editor)))
            .unwrap();
        let document = match panel.view_key {
            ViewKey::Editor { document_id, .. } => document_id,
            _ => panic!("源码文件必须安装 Editor"),
        };
        (document, *panel_id, editor.clone(), panel.view_key.clone())
    });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });
    activate_project(&root, &mut window, PROJECT_A);

    wait_for_document_watch(&root, cx, &source);
    std::fs::rename(&source, &target).unwrap();

    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.document(document).cloned()),
        root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .map(|session| session.location().clone())
        })
    );
    root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_B)].read(cx);
        let current = project.panels.get(&panel).unwrap();
        assert_eq!(current.id, panel);
        assert_eq!(current.view_key, original_view);
        assert_eq!(current.editor(), Some(&editor));
        assert_eq!(editor.read(cx).input_text_for_test(), "same");
        assert_eq!(
            editor.read(cx).language_state_for_test(),
            (None, slterm_document::LanguageId::Markdown),
            "纯改名必须更新自动语言但不改变视图身份"
        );
    });
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.registry().active()),
        Some(project_id(PROJECT_A)),
        "外部改名不得激活隐藏项目"
    );
}

#[gpui::test]
fn editor_external_rename_preserves_manual_language_override(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "same");
    select_editor_language_override(cx, &mut window, &editor, Some(LanguageId::Rust));

    std::fs::rename(&source, &target).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    assert_eq!(
        editor.read_with(cx, |view, _| view.language_state_for_test()),
        (Some(LanguageId::Rust), LanguageId::Rust)
    );
}

#[gpui::test]
fn external_rename_then_reopen_focuses_existing_editor(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "same");
    std::fs::rename(&source, &target).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&target).unwrap(),
    );

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    root.read_with(cx, |root, cx| {
        let panels = &root.projects[&project_id(PROJECT_A)].read(cx).panels;
        let panel = panels.values().next().unwrap();
        assert_eq!(panel.editor(), Some(&editor));
        assert_eq!(
            panel.view_key,
            ViewKey::Editor {
                project_id: project_id(PROJECT_A),
                document_id: document,
            }
        );
    });
}

#[gpui::test]
fn paged_external_rename_preserves_view_and_rebinds_page_reader(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let source = project.join("large.txt");
    let target = project.join("renamed.rs");
    let mut bytes = b"paged-start\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&source, bytes).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let (document, panel, view, original_view, original_source, reader_identity) =
        root.read_with(cx, |root, cx| {
            let project = root.projects[&project_id(PROJECT_A)].read(cx);
            let (panel_id, panel, view) = project
                .panels
                .iter()
                .find_map(|(id, panel)| match &panel.content {
                    super::super::panel_host::PanelContent::Paged(view) => Some((id, panel, view)),
                    _ => None,
                })
                .unwrap();
            let document = match panel.view_key {
                ViewKey::Editor { document_id, .. } => document_id,
                _ => panic!("分页文件必须保留 Editor ViewKey"),
            };
            let session = root.documents.session(document).unwrap();
            let source = session.paged().unwrap();
            let reader_identity = Arc::as_ptr(root.paged_documents.get(&document).unwrap());
            (
                document,
                *panel_id,
                view.clone(),
                panel.view_key.clone(),
                (
                    source.source_generation(),
                    *source.fingerprint(),
                    source.index().total_bytes(),
                ),
                reader_identity,
            )
        });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });

    wait_for_document_watch(&root, cx, &source);
    std::fs::rename(&source, &target).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        let current = project.panels.get(&panel).unwrap();
        assert_eq!(current.view_key, original_view);
        assert!(
            matches!(&current.content, super::super::panel_host::PanelContent::Paged(current) if current == &view)
        );
        let source = root.documents.session(document).unwrap().paged().unwrap();
        assert_eq!(
            (
                source.source_generation(),
                *source.fingerprint(),
                source.index().total_bytes(),
            ),
            original_source,
            "纯改名不得重建分页源或推进代次"
        );
        assert_ne!(
            Arc::as_ptr(root.paged_documents.get(&document).unwrap()),
            reader_identity,
            "读取句柄必须重绑到新位置"
        );
    });
    let reader = root.read_with(cx, |root, _| root.paged_documents[&document].clone());
    assert!(reader.read_page(0).unwrap().starts_with("paged-start\n"));
}

#[gpui::test]
fn paged_external_rename_and_content_change_reload_source_and_reader(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large.txt");
    let target = temp.path().join("renamed.rs");
    let mut bytes = b"old-start\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&source, bytes).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let old_generation = root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .paged()
            .unwrap()
            .source_generation()
    });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });
    wait_for_document_watch(&root, cx, &source);

    std::fs::rename(&source, &target).unwrap();
    let mut changed = b"new-start\n".to_vec();
    changed.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, changed).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    for _ in 0..2 {
        window.refresh().unwrap();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();
    }

    assert_eq!(
        paged_painted_lines(&root, cx, PROJECT_A, document).first(),
        Some(&(0, Some("new-start".to_owned())))
    );
    assert!(root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .paged()
            .unwrap()
            .source_generation()
            > old_generation
    }));
}

#[allow(clippy::type_complexity)]
fn open_watched_editor(
    cx: &mut TestAppContext,
    content: &str,
) -> (
    tempfile::TempDir,
    Entity<AppRoot>,
    VisualTestContext,
    slterm_document::DocumentId,
    Entity<crate::app_shell::text_view::DocumentTextView>,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.txt");
    let target = temp.path().join("renamed.md");
    std::fs::write(&source, content).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let (document, editor) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                let editor = panel.editor()?.clone();
                match panel.view_key {
                    ViewKey::Editor { document_id, .. } => Some((document_id, editor)),
                    _ => None,
                }
            })
            .unwrap()
    });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });
    wait_for_document_watch(&root, cx, &source);
    (temp, root, window, document, editor, source, target)
}

#[gpui::test]
fn clean_external_rename_growth_switches_content_without_changing_panel(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old\n");
    let (panel, view_key) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                (panel.editor() == Some(&editor)).then_some((panel.id, panel.view_key.clone()))
            })
            .unwrap()
    });

    std::fs::rename(&source, &target).unwrap();
    let mut bytes = b"old\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, bytes).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    for _ in 0..2 {
        window.refresh().unwrap();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();
    }

    root.read_with(cx, |root, cx| {
        let panel_host = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        assert_eq!(panel_host.id, panel);
        assert_eq!(panel_host.view_key, view_key);
        assert!(matches!(
            panel_host.content,
            super::super::panel_host::PanelContent::Paged(_)
        ));
    });
    assert_eq!(
        paged_painted_lines(&root, cx, PROJECT_A, document).first(),
        Some(&(0, Some("old".to_owned())))
    );
}

#[gpui::test]
fn external_conversion_closes_language_selector_and_focuses_new_panel(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old\n");
    let panel = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| (panel.editor() == Some(&editor)).then_some(panel.id))
            .unwrap()
    });
    draw_frame(&mut window);
    let selector =
        Box::leak(format!("document-language-selector-{}", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.language_selector_menu.is_some() && root.focus_manager.modal_is_active()
    }));

    std::fs::rename(&source, &target).unwrap();
    let mut bytes = b"outside\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, bytes).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    let new_focus = root.read_with(cx, |root, cx| {
        assert!(root.language_selector_menu.is_none());
        assert!(!root.focus_manager.modal_is_active());
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        assert!(matches!(
            panel.content,
            super::super::panel_host::PanelContent::Paged(_)
        ));
        panel.focus_handle(cx)
    });
    assert_eq!(window.update(|window, cx| window.focused(cx)), Some(new_focus));
}

#[gpui::test]
fn external_conversion_focus_failure_rolls_back_registry_snapshot_and_panel(
    cx: &mut TestAppContext,
) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old\n");
    let (panel, before) = root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                (panel.editor() == Some(&editor)).then_some((panel.id, panel.view_key.clone()))
            })
            .unwrap()
            .0;
        (panel, root.snapshot().clone())
    });
    root.update(cx, |root, _| {
        root.focus_manager.fail_next_request_focus_for_test();
    });

    std::fs::rename(&source, &target).unwrap();
    let mut bytes = b"outside\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, bytes).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.last_document_relocation_targets.get(&document).is_some_and(|last| {
            last.as_path() == target
                && !root.tasks.is_registered(
                    super::super::task_service::TaskId::DocumentExternal(document),
                )
        })
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "未执行外部形态转换");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    root.read_with(cx, |root, cx| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.text().unwrap().to_string(), "old\n");
        assert_eq!(session.location().canonical_path().as_path(), source);
        assert_eq!(root.snapshot(), &before);
        let current = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        assert_eq!(current.editor(), Some(&editor));
        assert_eq!(editor.read(cx).input_text_for_test(), "old\n");
    });
    select_editor_language_override(cx, &mut window, &editor, Some(LanguageId::Rust));
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_view_persistence
                .editor_view_states()
                .state(&ViewKey::Editor {
                    project_id: project_id(PROJECT_A),
                    document_id: document,
                })
                .language_override()
        }),
        Some(LanguageId::Rust),
        "转换失败不得让存活旧视图的租约失效"
    );
}

#[gpui::test]
fn paged_external_rename_shrink_restores_editor_without_changing_panel(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large.txt");
    let target = temp.path().join("small.txt");
    std::fs::write(
        &source,
        vec![b'x'; slterm_document::EDITABLE_LIMIT_BYTES as usize + 1],
    )
    .unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, CanonicalPath::new(&source).unwrap());
    let (document, panel, view_key) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                super::super::panel_host::PanelContent::Paged(_) => Some((
                    root.snapshot().documents().next().unwrap().0,
                    panel.id,
                    panel.view_key.clone(),
                )),
                _ => None,
            })
            .unwrap()
    });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });
    wait_for_document_watch(&root, cx, &source);

    std::fs::rename(&source, &target).unwrap();
    std::fs::write(&target, "small\n").unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        assert_eq!(panel.view_key, view_key);
        let editor = panel.editor().unwrap();
        assert_eq!(editor.read(cx).input_text_for_test(), "small\n");
    });
}

#[gpui::test]
fn editor_consecutive_external_renames_coalesce_to_latest_target(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "same");
    let intermediate = source.with_file_name("intermediate.txt");

    std::fs::rename(&source, &intermediate).unwrap();
    std::fs::rename(&intermediate, &target).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "same");
    assert!(root.read_with(cx, |root, _| {
        !root.document_relocation_aliases.contains_key(&source)
            && !root.document_relocation_aliases.contains_key(&intermediate)
    }));
}

#[gpui::test]
fn editor_external_rename_foldback_drops_stale_target_alias(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, _editor, source, target) =
        open_watched_editor(cx, "same");
    std::fs::rename(&source, &target).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    std::fs::rename(&target, &source).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        !root.pending_document_relocations.contains_key(&document)
            && !root.active_document_relocations.contains_key(&document)
            && root
                .last_document_relocation_targets
                .get(&document)
                .is_some_and(|last| last.as_path() == source)
            && !root.tasks.is_registered(
                super::super::task_service::TaskId::DocumentExternal(document),
            )
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "折返后仍保留旧目标状态");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(root.read_with(cx, |root, _| {
        !root.document_relocation_aliases.contains_key(&target)
    }));

    std::fs::write(&target, "unrelated").unwrap();
    let unrelated = target.with_file_name("unrelated.txt");
    std::fs::rename(&target, &unrelated).unwrap();
    for _ in 0..4 {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let state = root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        (
            session.location().canonical_path().as_path().to_path_buf(),
            session.external_conflict().map(|conflict| conflict.kind()),
            session.is_read_revoked(),
            root.pending_document_relocations.clone(),
            root.last_document_relocation_targets.clone(),
            root.document_relocation_aliases.clone(),
        )
    });
    assert!(
        state.0 == source && state.1.is_none() && !state.2,
        "折返后状态: {state:?}"
    );
}

#[gpui::test]
fn newer_pending_relocation_does_not_retain_intermediate_alias(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, _editor, source, target) =
        open_watched_editor(cx, "same");
    let latest = target.with_file_name("latest.txt");
    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    std::fs::rename(&source, &target).unwrap();
    std::fs::rename(&target, &latest).unwrap();
    let target_path = CanonicalPath::new(&target).unwrap();
    let latest_path = CanonicalPath::new(&latest).unwrap();
    root.update(cx, |root, _| {
        root.active_document_relocations
            .insert(document, target_path.clone());
        root.last_document_relocation_targets
            .insert(document, target_path.clone());
        root.pending_document_relocations
            .insert(document, latest_path.clone());
        root.document_relocation_aliases.insert(target.clone(), document);
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_document_external(
                document,
                target_path.clone(),
                false,
                Ok(Some(
                    slterm_document::DocumentExternalChange::ReadRevoked {
                        location: slterm_path::ResolvedPath::new(target_path, None),
                        request,
                    }
                    .into(),
                )),
                window,
                cx,
            );
        });
    });

    assert!(root.read_with(cx, |root, _| {
        !root.document_relocation_aliases.contains_key(&target)
            && root.active_document_relocations.get(&document) == Some(&latest_path)
            && root.last_document_relocation_targets.get(&document) == Some(&latest_path)
    }));
}

#[gpui::test]
fn foldback_refresh_does_not_retain_intermediate_alias(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, _editor, source, target) =
        open_watched_editor(cx, "same");
    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    std::fs::rename(&source, &target).unwrap();
    std::fs::rename(&target, &source).unwrap();
    let source_path = CanonicalPath::new(&source).unwrap();
    let target_path = CanonicalPath::new(&target).unwrap();
    root.update(cx, |root, _| {
        root.active_document_relocations
            .insert(document, target_path.clone());
        root.last_document_relocation_targets
            .insert(document, target_path.clone());
        root.pending_document_relocations
            .insert(document, source_path.clone());
        root.pending_document_refreshes.insert(document);
        root.document_relocation_aliases.insert(target.clone(), document);
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_document_external(
                document,
                target_path.clone(),
                false,
                Ok(Some(
                    slterm_document::DocumentExternalChange::ReadRevoked {
                        location: slterm_path::ResolvedPath::new(target_path, None),
                        request,
                    }
                    .into(),
                )),
                window,
                cx,
            );
        });
    });

    assert!(root.read_with(cx, |root, _| {
        !root.document_relocation_aliases.contains_key(&target)
            && root.active_document_relocations.get(&document) == Some(&source_path)
            && !root.last_document_relocation_targets.contains_key(&document)
    }));
}

#[gpui::test]
fn clean_external_rename_and_edit_reload_shared_editor(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old");

    std::fs::rename(&source, &target).unwrap();
    std::fs::write(&target, "outside").unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "outside");
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.document(document).cloned()),
        root.read_with(cx, |root, _| {
            root.documents.session(document).map(|session| session.location().clone())
        })
    );
}

#[gpui::test]
fn clean_external_edit_without_rename_reloads_editor(cx: &mut TestAppContext) {
    let (_temp, _root, window, _document, editor, source, _target) =
        open_watched_editor(cx, "old");

    std::fs::write(&source, "outside").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while editor.read_with(cx, |view, _| view.input_text_for_test()) != "outside" {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "普通外部修改未重载 Editor");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui::test]
fn dirty_external_delete_without_rename_preserves_text_and_conflict(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, _target) =
        open_watched_editor(cx, "old");
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });

    std::fs::remove_file(&source).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_some_and(|conflict| conflict.kind() == slterm_document::ExternalConflictKind::Deleted)
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "普通外部删除未建立 Deleted 冲突");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "local old");
}

#[gpui::test]
fn dirty_external_rename_and_edit_preserve_local_text_and_conflict(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old");
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });

    std::fs::rename(&source, &target).unwrap();
    std::fs::write(&target, "outside").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_some()
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "脏文档未建立共享冲突");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "local old");
    assert_eq!(root.read_with(cx, |root, _| root.documents.session(document).unwrap().text().unwrap().to_string()), "local old");
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.documents.session(document).unwrap().location().canonical_path().as_path().to_path_buf()
        }),
        source,
        "冲突不得重绑 Registry"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot.document(document).unwrap().canonical_path().as_path().to_path_buf()),
        source,
        "冲突不得提交位置快照候选"
    );
}

#[gpui::test]
fn dirty_external_edit_after_rename_conflict_refreshes_alias(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old");
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });
    std::fs::rename(&source, &target).unwrap();
    std::fs::write(&target, "outside").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_some()
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "改名修改未建立冲突");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    std::fs::write(&target, "latest").unwrap();
    while !root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .and_then(|conflict| conflict.disk_baseline())
            .is_some_and(|baseline| baseline.as_ref() == b"latest")
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(
            std::time::Instant::now() < deadline,
            "冲突后的别名修改未刷新磁盘快照"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "local old");
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().location().canonical_path().as_path() == source
    }));
}

#[gpui::test]
fn external_rename_waits_for_inflight_save_marker(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, editor, source, target) =
        open_watched_editor(cx, "old");
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });
    let save = root.update(cx, |root, _| root.documents.begin_save(document, 0).unwrap());
    let save_generation = save.save_generation();

    std::fs::rename(&source, &target).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| root.pending_document_relocations.contains_key(&document)) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "保存中改名未进入待处理队列");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().location().canonical_path().as_path() == source
            && !root.tasks.is_registered(super::super::task_service::TaskId::DocumentExternal(document))
    }));

    root.update(cx, |root, _| {
        root.documents.fail_save(document, save_generation).unwrap();
    });
    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    assert_eq!(editor.read_with(cx, |view, _| view.input_text_for_test()), "local old");
}

#[gpui::test]
fn pending_refresh_after_rename_reads_target_not_old_source(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, _editor, source, target) =
        open_watched_editor(cx, "same");
    let save = root.update(cx, |root, _| root.documents.begin_save(document, 0).unwrap());
    let save_generation = save.save_generation();
    std::fs::rename(&source, &target).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| root.pending_document_relocations.contains_key(&document)) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "保存中改名未进入待处理队列");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    root.update(cx, |root, _| {
        // 延迟到达的旧位置普通事件与 pending 改名合并后仍必须读取新目标。
        root.pending_document_refreshes.insert(document);
        root.documents.fail_save(document, save_generation).unwrap();
    });

    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    assert!(root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_none()
    }));
}

#[gpui::test]
fn external_rename_rechecks_latest_project_grants_before_commit(cx: &mut TestAppContext) {
    use crate::platform::document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver};

    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = temp.path().join("b");
    std::fs::create_dir_all(&root_a).unwrap();
    std::fs::create_dir_all(&root_b).unwrap();
    let source = root_a.join("source.txt");
    let target = root_a.join("renamed.txt");
    std::fs::write(&source, "same").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    open_document(&root, &mut window, PROJECT_A, GROUP_A, CanonicalPath::new(&source).unwrap());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    root.update(cx, |root, cx| {
        let location = root
            .documents
            .session(document)
            .unwrap()
            .location()
            .clone();
        let candidate = root
            .snapshot
            .with_opened_document(project_id(PROJECT_B), group_id(GROUP_B), document, location)
            .unwrap()
            .0;
        root.commit_project_snapshot(candidate, cx);
    });
    let request = root.read_with(cx, |root, _| root.documents.external_request(document, root.current_authorization_generation()).unwrap());
    std::fs::rename(&source, &target).unwrap();
    let target_location = SystemDocumentPathResolver
        .resolve(&CanonicalPath::new(&target).unwrap())
        .unwrap();

    root.update(cx, |root, cx| {
        let candidate = root
            .snapshot
            .without_project(project_id(PROJECT_A))
            .unwrap()
            .0;
        root.commit_project_snapshot(candidate, cx);
    });
    let change = root.update(cx, |root, _| {
        root.reauthorize_external_change(
            document,
            slterm_document::DocumentExternalChange::Relocated {
                request,
                location: target_location,
            },
        )
    });

    assert!(matches!(
        change,
        slterm_document::DocumentExternalChange::ReadRevoked { request, .. } if request.document_id() == document
    ));
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.documents.session(document).unwrap().location().canonical_path().as_path().to_path_buf()
        }),
        source
    );
}

#[gpui::test]
fn read_revoke_for_superseded_relocation_target_is_ignored(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, _editor, source, target) =
        open_watched_editor(cx, "same");
    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    let location = request.location().clone();
    let target = CanonicalPath::new(&target).unwrap();
    root.update(cx, |root, _| {
        root.pending_document_relocations
            .insert(document, target.clone());
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.apply_document_external_result(
                document,
                CanonicalPath::new(&source).unwrap(),
                false,
                slterm_document::DocumentExternalChange::ReadRevoked { request, location }
                    .into(),
                window,
                cx,
            );
        });
    });

    assert!(root.read_with(cx, |root, _| {
        !root.documents.session(document).unwrap().is_read_revoked()
            && root.pending_document_relocations.get(&document) == Some(&target)
    }));
}

fn open_watched_markdown(
    cx: &mut TestAppContext,
) -> (
    tempfile::TempDir,
    Entity<AppRoot>,
    VisualTestContext,
    slterm_document::DocumentId,
    Entity<crate::app_shell::markdown::MarkdownView>,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let source = project.join("note.md");
    let target = project.join("renamed.md");
    std::fs::write(&source, "old\n").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&source).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&source).unwrap()).unwrap()),
    );
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(&project).unwrap()).unwrap(),
        )
        .unwrap();
    let project_snapshot = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), document, location)
        .unwrap()
        .0;
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&project_snapshot)
        .unwrap();

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    window.run_until_parked();
    let view = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                super::super::panel_host::PanelContent::Markdown(view) => Some(view.view.clone()),
                _ => None,
            })
            .unwrap()
    });
    let (_sender, receiver) = std::sync::mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx);
        });
    });
    wait_for_document_watch(&root, cx, &source);
    (temp, root, window, document, view, source, target)
}

fn edit_markdown_draft(
    view: &Entity<crate::app_shell::markdown::MarkdownView>,
    window: &mut VisualTestContext,
) {
    let input = view.read_with(window, |view, _| view.source_for_test());
    window.update(|window, cx| {
        input.update(cx, |input, cx| {
            gpui::EntityInputHandler::replace_text_in_range(
                input,
                Some(0..0),
                "local ",
                window,
                cx,
            );
        });
    });
}

#[gpui::test]
fn markdown_shared_document_autosave(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, view, source, _target) = open_watched_markdown(cx);
    cx.update(|cx| {
        cx.set_global(crate::app_shell::config::Settings {
            document_autosave: true,
            ..crate::app_shell::config::Settings::default()
        });
    });
    edit_markdown_draft(&view, &mut window);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "old\n");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while root.read_with(cx, |root, _| root.documents.session(document).unwrap().is_dirty()) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "Markdown 共享 Document 未自动保存");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "local old\n");
}

#[gpui::test]
fn markdown_mode_toggle_uses_shared_draft_without_disk_reload(cx: &mut TestAppContext) {
    let (_temp, _root, mut window, _document, view, source, _target) = open_watched_markdown(cx);
    window.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_mode_for_test(window, cx));
    });
    edit_markdown_draft(&view, &mut window);
    window.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_mode_for_test(window, cx));
    });
    window.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.rendered_text_for_test(), "local old");
    });
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "old\n");
}

#[gpui::test]
fn dirty_markdown_external_growth_preserves_draft_and_panel(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, view, source, target) = open_watched_markdown(cx);
    let panel = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                matches!(&panel.content, super::super::panel_host::PanelContent::Markdown(current) if current.view == view)
                    .then_some(panel.id)
            })
            .unwrap()
    });
    edit_markdown_draft(&view, &mut window);
    assert!(root.read_with(cx, |root, _| root.documents.session(document).unwrap().is_dirty()));

    std::fs::rename(&source, &target).unwrap();
    let mut bytes = b"outside\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, bytes).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .external_conflict()
            .is_some()
    }) {
        window.executor().advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "Markdown 脏草稿未建立共享冲突");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(
        view.read_with(cx, |view, cx| view.input_text_for_test(cx)),
        "local old\n"
    );
    assert!(root.read_with(cx, |root, cx| {
        matches!(
            root.projects[&project_id(PROJECT_A)].read(cx).panels[&panel].content,
            super::super::panel_host::PanelContent::Markdown(ref current) if current.view == view
        )
    }));
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .unwrap()
                .location()
                .canonical_path()
                .as_path()
                .to_path_buf()
        }),
        source
    );
}

#[gpui::test]
fn clean_markdown_external_growth_switches_to_paged_without_changing_view(
    cx: &mut TestAppContext,
) {
    let (_temp, root, mut window, document, _view, source, target) = open_watched_markdown(cx);
    let (panel, view_key) = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| {
                matches!(panel.content, super::super::panel_host::PanelContent::Markdown(_))
                    .then_some((panel.id, panel.view_key.clone()))
            })
            .unwrap()
    });

    std::fs::rename(&source, &target).unwrap();
    let mut bytes = b"outside\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&target, bytes).unwrap();
    wait_for_document_relocation(&root, &mut window, cx, document, &target);

    root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        assert_eq!(panel.view_key, view_key);
        assert!(matches!(
            panel.content,
            super::super::panel_host::PanelContent::Paged(_)
        ));
        assert!(root.documents.session(document).unwrap().paged().is_some());
    });
}

#[gpui::test]
fn markdown_shared_save_defers_external_relocation_without_losing_text(
    cx: &mut TestAppContext,
) {
    use crate::platform::document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver};

    let (_temp, root, mut window, document, view, source, target) = open_watched_markdown(cx);
    edit_markdown_draft(&view, &mut window);
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    assert!(root.read_with(cx, |root, _| root.documents.session(document).unwrap().is_saving()));
    let request = root.read_with(cx, |root, _| {
        root.documents.external_request(document, root.current_authorization_generation()).unwrap()
    });
    std::fs::rename(&source, &target).unwrap();
    let location = SystemDocumentPathResolver
        .resolve(&CanonicalPath::new(&target).unwrap())
        .unwrap();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.apply_document_external_result(
                document,
                CanonicalPath::new(&target).unwrap(),
                false,
                crate::app_shell::document_external::DocumentExternalResult {
                    change: slterm_document::DocumentExternalChange::Relocated {
                        request,
                        location,
                    },
                    paged: None,
                },
                window,
                cx,
            );
        });
    });

    assert!(root.read_with(cx, |root, _| {
        root.pending_document_relocations.contains_key(&document)
            && root.documents.session(document).unwrap().location().canonical_path().as_path() == source
    }));
    wait_for_document_relocation(&root, &mut window, cx, document, &target);
    assert_eq!(
        view.read_with(cx, |view, cx| view.input_text_for_test(cx)),
        "local old\n"
    );
    assert!(root.read_with(cx, |root, _| root.documents.session(document).unwrap().is_dirty()));
}
