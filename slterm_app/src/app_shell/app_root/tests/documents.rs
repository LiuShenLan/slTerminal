//! Document 在 AppRoot 的真实打开、跨项目身份与失败原子性。

use super::close::flush_when_ready;
use super::*;
use gpui::Focusable as _;
use slterm_document::LanguageId;

include!("documents/open_file.rs");

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

fn select_editor_language_override(
    cx: &TestAppContext,
    window: &mut VisualTestContext,
    editor: &Entity<crate::app_shell::text_view::DocumentTextView>,
    language_override: Option<LanguageId>,
) {
    draw_frame(window);
    let editor_focus = editor.read_with(cx, |view, app| view.focus_handle(app));
    let selector =
        Box::leak(format!("document-language-selector-{}", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
    assert_ne!(
        window.update(|window, cx| window.focused(cx)),
        Some(editor_focus.clone()),
        "语言菜单必须取得焦点"
    );
    window.simulate_mouse_move(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        None,
        Modifiers::default(),
    );
    let down_count = match language_override {
        None => 1,
        Some(language) => {
            LanguageId::ALL
                .iter()
                .position(|candidate| *candidate == language)
                .unwrap()
                + 2
        }
    };
    let keys = format!(
        "{} enter",
        std::iter::repeat_n("down", down_count)
            .collect::<Vec<_>>()
            .join(" ")
    );
    window.simulate_keystrokes(&keys);
    draw_frame(window);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(editor_focus),
        "选择语言后必须还焦 Editor"
    );
}

fn toggle_editor_wrap(
    cx: &TestAppContext,
    window: &mut VisualTestContext,
    editor: &Entity<crate::app_shell::text_view::DocumentTextView>,
) {
    draw_frame(window);
    let selector =
        Box::leak(format!("document-wrap-toggle-{}", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
    assert!(editor.read_with(cx, |view, _| view.wrap_enabled_for_test()));
}

fn open_file_handover(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    path: &std::path::Path,
) -> Result<slterm_runtime_protocol::HandoverResponse, slterm_runtime_protocol::JsonRpcError> {
    let (completion, response) = std::sync::mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::Handover {
                    params: slterm_runtime_protocol::HandoverParams::OpenFile {
                        path: path.to_str().unwrap().to_owned(),
                    },
                    completion,
                },
                window,
                cx,
            );
        });
    });
    window.run_until_parked();
    response
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap()
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

include!("documents/external.rs");

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
fn editor_language_override_is_per_view_and_inactive_syntax_tracks_edits(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    std::fs::create_dir(&root_a).unwrap();
    let file = root_a.join("shared.txt");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, temp.path().to_str().unwrap());
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
    select_editor_language_override(cx, &mut window, &editor_a, Some(LanguageId::Python));

    activate_project(&root, &mut window, PROJECT_B);
    open_document(&root, &mut window, PROJECT_B, GROUP_B, file.clone());
    let editor_b = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_B)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    let document = editor_a.read_with(cx, |view, _| view.scroll_state_for_test().2);
    assert_eq!(
        editor_b.read_with(cx, |view, _| view.language_state_for_test()),
        (None, LanguageId::PlainText)
    );

    activate_project(&root, &mut window, PROJECT_A);
    window.simulate_input("shared ");
    assert_eq!(
        editor_text(&root, cx, PROJECT_A, document),
        "shared fn main() {}\n"
    );
    assert_eq!(
        editor_text(&root, cx, PROJECT_B, document),
        "shared fn main() {}\n"
    );
    select_editor_language_override(cx, &mut window, &editor_a, Some(LanguageId::Rust));
    for _ in 0..100 {
        if editor_a.read_with(cx, |view, _| view.syntax_is_current_for_test()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        window
            .executor()
            .advance_clock(std::time::Duration::from_millis(30));
        draw_frame(&mut window);
    }
    let syntax_status = root.read_with(cx, |root, _| {
        root.document_syntax
            .get(&(document, LanguageId::Rust))
            .map(|syntax| {
                let syntax = syntax.borrow();
                (syntax.revision_for_test(), syntax.is_resolved())
            })
    });
    assert!(
        editor_a.read_with(cx, |view, _| view.syntax_is_current_for_test()),
        "切回旧语言时必须使用已跟进最新编辑的 worker: {syntax_status:?}"
    );
    assert_eq!(
        editor_b.read_with(cx, |view, _| view.language_state_for_test()),
        (None, LanguageId::PlainText),
        "另一 ViewKey 不得继承手动语言覆盖"
    );
    select_editor_language_override(cx, &mut window, &editor_a, None);
    assert_eq!(
        editor_a.read_with(cx, |view, _| view.language_state_for_test()),
        (None, LanguageId::PlainText),
        "自动项必须清除手动覆盖"
    );
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    assert_eq!(
        std::fs::read_to_string(file.as_path()).unwrap(),
        "fn main() {}\n",
        "语言覆盖不得隐式保存或改写磁盘"
    );
}

#[gpui::test]
fn editor_html_system_open_does_not_save(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("page.html");
    std::fs::write(&file, "<p>disk</p>").unwrap();
    let file = CanonicalPath::new(file).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.simulate_input("draft ");
    select_editor_language_override(cx, &mut window, &editor, Some(LanguageId::PlainText));
    let (opened, opened_rx) = std::sync::mpsc::sync_channel(1);
    window.update(|_, cx| {
        root.update(cx, |root, _| {
            root.system_open = std::sync::Arc::new(move |path| {
                opened.try_send(path).unwrap();
            });
        });
    });

    draw_frame(&mut window);
    let selector =
        Box::leak(format!("document-system-open-{}", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    window.run_until_parked();

    assert_eq!(opened_rx.try_recv().unwrap().as_path(), file.as_path());
    assert_eq!(
        std::fs::read_to_string(file.as_path()).unwrap(),
        "<p>disk</p>"
    );
    assert!(root.read_with(cx, |root, _| {
        let document = root.snapshot().documents().next().unwrap().0;
        root.documents.session(document).unwrap().is_dirty()
    }));
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "draft <p>disk</p>"
    );
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
fn document_duplicate_view_focuses_existing(cx: &mut TestAppContext) {
    let (_temp, root, mut window, document, view, source, _target) = open_watched_markdown(cx);
    let panel = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| (panel.markdown() == Some(&view)).then_some(panel.id))
            .unwrap()
    });
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(source).unwrap(),
    );

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        2
    );
    assert_eq!(root.read_with(cx, |root, _| root.documents.len()), 1);
    let focus = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        let current = project.panels.get(&panel).unwrap();
        assert_eq!(current.markdown(), Some(&view));
        assert_eq!(
            current.view_key,
            ViewKey::Markdown {
                project_id: project_id(PROJECT_A),
                document_id: document,
            }
        );
        current.focus_handle(cx)
    });
    assert_eq!(window.update(|window, cx| window.focused(cx)), Some(focus));
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
fn editor_view_state_restore(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("scroll.txt");
    std::fs::write(&file, format!("{}\n1\n2\n3\n4\n5\n", "a".repeat(80))).unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    select_editor_language_override(cx, &mut window, &editor, Some(LanguageId::Python));
    toggle_editor_wrap(cx, &mut window, &editor);
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::set_selected_text_range(view, 0..80, window, cx);
            view.set_scroll_for_test(3, 0);
        });
    });
    flush_when_ready(&runtime.client());
    let (line, column, document, selection) =
        editor.read_with(cx, |view, _| view.scroll_state_for_test());
    assert!(line > 0);
    assert_eq!(column, 0, "软换行不得保留横向滚动");
    assert_eq!(selection, 0..80);
    let key = ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    assert_eq!(
        editor.read_with(cx, |view, _| view.language_state_for_test()),
        (Some(LanguageId::Python), LanguageId::Python)
    );
    assert!(
        root.read_with(cx, |root, _| root
            .document_syntax
            .contains_key(&(document, LanguageId::Python))),
        "选择器必须经 AppRoot 取得 Python 语法"
    );
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_view_persistence
                .editor_view_states()
                .state(&key)
                .language_override()
        }),
        Some(LanguageId::Python)
    );

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    flush_when_ready(&runtime.client());
    let persisted =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, _window, _) = open_root_with_persistence(cx, persisted);
    let restored = root.read_with(cx, |root, cx| {
        let panel = root
            .panel_for_view(project_id(PROJECT_A), &key, cx)
            .unwrap();
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .unwrap();
        let editor = panel.editor().unwrap().read(cx);
        (
            editor.scroll_state_for_test(),
            editor.language_state_for_test(),
            editor.wrap_enabled_for_test(),
        )
    });
    let (restored, restored_language, restored_wrap) = restored;
    assert_eq!(restored.0, line);
    assert_eq!(restored.1, column);
    assert_eq!(restored.3, 0..0);
    assert!(restored_wrap, "重启后必须使用真实软换行索引");
    assert!(root.read_with(cx, |root, _| {
        root.side_view_persistence
            .editor_view_states()
            .state(&key)
            .wrap()
    }));
    assert_eq!(
        restored_language,
        (Some(LanguageId::Python), LanguageId::Python)
    );
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_view_persistence
                .editor_view_states()
                .state(&key)
                .language_override()
        }),
        Some(LanguageId::Python)
    );
    assert!(
        root.read_with(cx, |root, _| root
            .document_syntax
            .contains_key(&(document, LanguageId::Python))),
        "重启后必须按覆盖安装真实 Python 语法"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn editor_wrap_toggle_before_index_ready_persists_safe_origin(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("wrap.txt");
    std::fs::write(&file, "abcdef").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    draw_frame(&mut window);
    let selector =
        Box::leak(format!("document-wrap-toggle-{}", editor.entity_id()).into_boxed_str());
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    let key = root.read_with(cx, |root, _| ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: root.snapshot().documents().next().unwrap().0,
    });
    let immediate = root.read_with(cx, |root, _| {
        root.side_view_persistence.editor_view_states().state(&key)
    });
    assert!(immediate.wrap());
    assert_eq!(immediate.scroll_line(), 0);

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    flush_when_ready(&runtime.client());
    let persisted =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, _window, _) = open_root_with_persistence(cx, persisted);
    let restored = root.read_with(cx, |root, cx| {
        let panel = root
            .panel_for_view(project_id(PROJECT_A), &key, cx)
            .unwrap();
        let editor = root.projects[&project_id(PROJECT_A)].read(cx).panels[&panel]
            .editor()
            .unwrap()
            .read(cx);
        (
            editor.wrap_enabled_for_test(),
            editor.scroll_state_for_test(),
        )
    });
    assert!(restored.0);
    assert_eq!((restored.1.0, restored.1.1), (0, 0));
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
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
