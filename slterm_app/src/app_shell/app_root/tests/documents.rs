//! Document 在 AppRoot 的真实打开、跨项目身份与失败原子性。

use super::*;

pub(super) fn open_document(
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
            .input_text_for_test()
    })
}

fn paged_painted_lines(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: &str,
    document: slterm_document::DocumentId,
) -> Vec<(usize, Option<String>)> {
    root.read_with(cx, |root, cx| {
        let project_view = root.projects[&project_id(project)].read(cx);
        let panel = project_view
            .panels
            .values()
            .find(|panel| {
                panel.view_key
                    == (ViewKey::Editor {
                        project_id: project_id(project),
                        document_id: document,
                    })
            })
            .unwrap();
        match &panel.content {
            crate::app_shell::app_root::panel_host::PanelContent::Paged(view) => {
                view.read(cx).painted_lines_for_test()
            }
            _ => panic!("大文件必须安装 PagedTextView"),
        }
    })
}

fn write_recovered_snapshot(
    paths: &AppPaths,
    document: slterm_document::DocumentId,
    location: slterm_path::ResolvedPath,
    draft: &str,
) {
    let disk = std::fs::read_to_string(location.canonical_path().as_path()).unwrap();
    let saved = slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
    let current = slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
    let mut documents = slterm_document::DocumentRegistry::new();
    documents
        .open(
            document,
            location,
            ropey::Rope::from_str(&disk),
            saved,
            Arc::from(disk.as_bytes()),
            slterm_document::DocumentMetadata::default(),
        )
        .unwrap();
    documents
        .apply(
            slterm_document::EditTransaction {
                document_id: document,
                expected_text_revision: 0,
                edits: vec![slterm_document::TextEdit::new(0..disk.len(), draft)],
                origin: slterm_document::EditOrigin::UserInput,
                merge_policy: slterm_document::MergePolicy::Disallowed,
            },
            current,
            std::time::Instant::now(),
        )
        .unwrap();
    let snapshot = crate::persistence::document_snapshot::envelope::DocumentSnapshot::capture(
        documents.session(document).unwrap(),
        "run-test",
    )
    .unwrap();
    crate::persistence::document_snapshot::store::DocumentSnapshotStore::new(paths)
        .write(&snapshot, &BTreeSet::from([document]))
        .unwrap();
}

#[gpui::test]
fn outside_document_restart_requires_authorization(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(temp.path().join("outside.txt")).unwrap(),
        None,
    );
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
    let (panel_count, is_recovery) = root.read_with(cx, |root, cx| {
        let panels = &root.projects[&project_id(PROJECT_A)].read(cx).panels;
        (
            panels.len(),
            panels.values().all(|panel| {
                matches!(
                    &panel.content,
                    super::super::panel_host::PanelContent::Recovery(_)
                )
            }),
        )
    });
    assert_eq!(panel_count, 1);
    assert!(is_recovery);
    assert!(
        window
            .debug_bounds("document-recovery-authorization")
            .is_some()
    );
    assert!(window.update(|window, cx| window.focused(cx)).is_none());
}

#[gpui::test]
fn paged_document_restart_has_explicit_placeholder(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("large.txt");
    std::fs::write(
        &file,
        vec![b'x'; slterm_document::EDITABLE_LIMIT_BYTES as usize + 1],
    )
    .unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
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

    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().paged().is_some()
    }));
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert!(window.debug_bounds("document-recovery-paged").is_some());
}

#[gpui::test]
fn startup_reconciles_layout_before_snapshot(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("file.txt");
    std::fs::write(&file, "disk\n").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let saved = slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
    let current = slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
    let mut documents = slterm_document::DocumentRegistry::new();
    documents
        .open(
            document,
            location.clone(),
            ropey::Rope::from_str("disk\n"),
            saved,
            Arc::from(b"disk\n".as_slice()),
            slterm_document::DocumentMetadata::default(),
        )
        .unwrap();
    documents
        .apply(
            slterm_document::EditTransaction {
                document_id: document,
                expected_text_revision: 0,
                edits: vec![slterm_document::TextEdit::new(0..5, "draft\n")],
                origin: slterm_document::EditOrigin::UserInput,
                merge_policy: slterm_document::MergePolicy::Disallowed,
            },
            current,
            std::time::Instant::now(),
        )
        .unwrap();
    let snapshot = crate::persistence::document_snapshot::envelope::DocumentSnapshot::capture(
        documents.session(document).unwrap(),
        "run-test",
    )
    .unwrap();
    crate::persistence::document_snapshot::store::DocumentSnapshotStore::new(&paths)
        .write(&snapshot, &BTreeSet::from([document]))
        .unwrap();
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

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        layout
    );
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "draft\n");
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.restore_project_panels(project_id(PROJECT_A), window, cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "draft\n");
}

#[gpui::test]
fn replaced_clean_document_has_placeholder_beside_new_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("file.txt");
    std::fs::write(&file, "old clean").unwrap();
    let old_location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    crate::atomic_file::write(&file, b"new clean").unwrap();
    let new_location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let old =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let new =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap();
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap(),
        )
        .unwrap();
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), old, old_location)
        .unwrap()
        .0
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), new, new_location)
        .unwrap()
        .0;
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&layout)
        .unwrap();

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(root.read_with(cx, |root, _| root.documents.session(old).is_none()));
    assert_eq!(editor_text(&root, cx, PROJECT_A, new), "new clean");
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        2
    );
    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            let candidate = root
                .snapshot()
                .with_active_view(&ViewKey::Editor {
                    project_id: project_id(PROJECT_A),
                    document_id: old,
                })
                .unwrap();
            root.commit_project_snapshot(candidate, cx);
        });
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("document-recovery-replaced").is_some());

    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.close_all_views_from_request(group_id(GROUP_A), BTreeMap::new(), _window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| root.snapshot().documents().next().is_none()));
}

#[gpui::test]
fn binary_new_source_keeps_old_replaced_placeholder(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("file.txt");
    std::fs::write(&file, "old clean").unwrap();
    let old_location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    crate::atomic_file::write(&file, b"replacement").unwrap();
    let new_location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let old =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let new =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    write_recovered_snapshot(&paths, new, new_location.clone(), "new draft");
    std::fs::write(&file, [0, 1, 2]).unwrap();
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap(),
        )
        .unwrap();
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), old, old_location)
        .unwrap()
        .0
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), new, new_location)
        .unwrap()
        .0;
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&layout)
        .unwrap();

    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert_eq!(editor_text(&root, cx, PROJECT_A, new), "new draft");
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        2
    );
    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            let candidate = root
                .snapshot()
                .with_active_view(&ViewKey::Editor {
                    project_id: project_id(PROJECT_A),
                    document_id: old,
                })
                .unwrap();
            root.commit_project_snapshot(candidate, cx);
        });
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("document-recovery-replaced").is_some());
}

#[gpui::test]
fn shared_document_restore_preserves_project_grant_union(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = temp.path().join("b");
    std::fs::create_dir(&root_a).unwrap();
    std::fs::create_dir(&root_b).unwrap();
    let file = root_a.join("shared.txt");
    std::fs::write(&file, "disk").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let document =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let location = slterm_path::ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let mut projects = slterm_workspace::ProjectRegistry::new();
    for (project, root) in [(PROJECT_A, &root_a), (PROJECT_B, &root_b)] {
        projects
            .add_project(
                project_id(project),
                CanonicalPath::new(dunce::canonicalize(root).unwrap()).unwrap(),
            )
            .unwrap();
    }
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(
            project_id(PROJECT_A),
            group_id(GROUP_A),
            document,
            location.clone(),
        )
        .unwrap()
        .0
        .with_opened_document(project_id(PROJECT_B), group_id(GROUP_B), document, location)
        .unwrap()
        .0;
    WorkspaceDatabase::open(&paths)
        .unwrap()
        .save_project_snapshot(&layout)
        .unwrap();
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_B), window, cx)
                .unwrap();
        });
    });
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            let revision = root.documents.session(document).unwrap().text_revision();
            let end = root
                .documents
                .session(document)
                .unwrap()
                .text()
                .unwrap()
                .len();
            root.documents
                .apply(
                    slterm_document::EditTransaction {
                        document_id: document,
                        expected_text_revision: revision,
                        edits: vec![slterm_document::TextEdit::new(0..end, "changed")],
                        origin: slterm_document::EditOrigin::UserInput,
                        merge_policy: slterm_document::MergePolicy::Disallowed,
                    },
                    slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
                    std::time::Instant::now(),
                )
                .unwrap();
            root.save_document(document, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(std::fs::read_to_string(file).unwrap(), "changed");
}

#[gpui::test]
fn inflight_open_does_not_suspend_existing_source_editing_or_saving(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.txt");
    let second = temp.path().join("second.txt");
    std::fs::write(&first, "old").unwrap();
    std::fs::write(&second, "second").unwrap();
    let first_path = CanonicalPath::new(&first).unwrap();
    let second_path = CanonicalPath::new(&second).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, first_path);
    let document = root.read_with(cx, |root, _| {
        root.snapshot
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
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_document(
                project_id(PROJECT_A),
                group_id(GROUP_B),
                second_path,
                window,
                cx,
            )
            .unwrap();
        });
    });

    window.simulate_input("draft ");
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, _| root
            .documents
            .session(document)
            .unwrap()
            .text()
            .unwrap()
            .to_string()),
        "draft old"
    );
    window.simulate_keystrokes("ctrl-s");
    window.run_until_parked();
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "draft old");
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_saving()
    }));
}

#[gpui::test]
fn source_save_failure_preserves_draft_and_allows_retry(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("save.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(&file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    window.simulate_input("draft ");

    std::fs::write(&file, "external").unwrap();
    window.simulate_keystrokes("ctrl-s");
    window.run_until_parked();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "external");
    assert!(root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        session.is_dirty() && !session.is_saving()
    }));

    std::fs::write(&file, "old").unwrap();
    window.simulate_keystrokes("ctrl-s");
    window.run_until_parked();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "draft old");
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
}

#[gpui::test]
fn save_during_inflight_save_coalesces_the_latest_snapshot(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("save.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(&file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path);
    let document = root.read_with(cx, |root, _| {
        root.snapshot
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
            gpui::EntityInputHandler::replace_text_in_range(view, None, "first ", window, cx);
        });
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_saving()
    }));
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "second ", window, cx);
        });
    });
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_saving()
    }));
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "first second old");
    assert!(!root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        session.is_dirty() || session.is_saving()
    }));
}

#[gpui::test]
fn shared_source_edit_and_save_use_one_document_session(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    std::fs::create_dir(&root_a).unwrap();
    let file = root_a.join("shared.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, temp.path().to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_A);
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| {
        root.snapshot
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
    activate_project(&root, &mut window, PROJECT_B);
    open_document(&root, &mut window, PROJECT_B, GROUP_B, file.clone());
    activate_project(&root, &mut window, PROJECT_A);
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());

    window.simulate_input("shared ");
    window.run_until_parked();

    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "shared old");
    assert_eq!(editor_text(&root, cx, PROJECT_B, document), "shared old");
    assert_eq!(
        root.read_with(cx, |root, _| root
            .documents
            .session(document)
            .unwrap()
            .text()
            .unwrap()
            .to_string()),
        "shared old"
    );
    for _ in 0..16 {
        if root.read_with(cx, |root, _| {
            root.documents.session(document).unwrap().text().unwrap() == "old"
        }) {
            break;
        }
        window.simulate_keystrokes("ctrl-z");
    }
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "old");
    assert_eq!(editor_text(&root, cx, PROJECT_B, document), "old");
    for _ in 0..16 {
        if root.read_with(cx, |root, _| {
            root.documents.session(document).unwrap().text().unwrap() == "shared old"
        }) {
            break;
        }
        window.simulate_keystrokes("ctrl-y");
    }
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "shared old");
    assert_eq!(editor_text(&root, cx, PROJECT_B, document), "shared old");
    window.simulate_keystrokes("ctrl-s");
    window.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(file.as_path()).unwrap(),
        "shared old"
    );
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
}

#[gpui::test]
fn large_file_paged_views_share_a_real_source_across_projects(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("large.txt");
    let mut bytes = b"paged-start\n".to_vec();
    bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
    std::fs::write(&file, bytes).unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );

    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());

    let document = root.read_with(cx, |root, _| {
        root.snapshot
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
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().text().is_none()
    }));
    for _ in 0..2 {
        window.refresh().unwrap();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();
    }
    let painted = paged_painted_lines(&root, cx, PROJECT_A, document);
    assert_eq!(painted.first(), Some(&(0, Some("paged-start".to_owned()))));
    let cached_pages = root.read_with(cx, |root, _| root.page_cache.borrow().len());

    add_project(&root, &mut window, PROJECT_B, temp.path().to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_B);
    open_document(&root, &mut window, PROJECT_B, GROUP_B, file);
    for _ in 0..2 {
        window.refresh().unwrap();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();
    }

    let painted = paged_painted_lines(&root, cx, PROJECT_B, document);
    assert_eq!(painted.first(), Some(&(0, Some("paged-start".to_owned()))));
    assert_eq!(
        root.read_with(cx, |root, _| {
            (
                root.paged_documents.len(),
                root.page_cache.borrow().len(),
                root.documents.len(),
            )
        }),
        (1, cached_pages, 1)
    );

    let closing_view = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_B)].read(cx);
        project
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                crate::app_shell::app_root::panel_host::PanelContent::Paged(view) => {
                    Some(view.clone())
                }
                _ => None,
            })
            .unwrap()
    });
    super::close::flush_when_ready(&runtime.client());
    super::project_close::remove_project(&root, &mut window, PROJECT_A).unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    assert_eq!(root.read_with(cx, |root, _| root.paged_documents.len()), 1);
    super::project_close::remove_project(&root, &mut window, PROJECT_B).unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    assert!(root.read_with(cx, |root, _| {
        root.documents.is_empty()
            && root.paged_documents.is_empty()
            && root.page_cache.borrow().is_empty()
    }));
    assert!(closing_view.read_with(cx, |view, _| view.is_closed_for_test()));

    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
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
