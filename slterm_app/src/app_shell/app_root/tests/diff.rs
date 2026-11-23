use std::str::FromStr as _;

use gpui::{Entity, Focusable as _, TestAppContext};
use slterm_document::{DocumentId, RepoPath, RepositoryBinding};

use super::super::diff::{DiffOpenRequest, DiffView};
use super::super::panel_host::PanelContent;
use super::*;

fn open_diff(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    document_id: DocumentId,
    binding: &RepositoryBinding,
    path: &str,
) -> PanelId {
    open_diff_at(root, window, document_id, binding, path, None)
}

fn open_diff_at(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    document_id: DocumentId,
    binding: &RepositoryBinding,
    path: &str,
    target_line: Option<usize>,
) -> PanelId {
    window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.open_diff(
                    DiffOpenRequest {
                        project_id: project_id(PROJECT_A),
                        target_group: Some(group_id(GROUP_A)),
                        document_id,
                        binding: binding.clone(),
                        path: RepoPath::from_str(path).unwrap(),
                        focus: true,
                        target_line,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap()
}

fn current_binding(repository: &std::path::Path) -> RepositoryBinding {
    let root = CanonicalPath::new(dunce::canonicalize(repository).unwrap()).unwrap();
    crate::git::GitFacade::default()
        .repository_binding(&root, &[slterm_path::PathGrant::repo_root(root.clone())], 0)
        .unwrap()
}

fn persist_diff_layout(
    paths: &AppPaths,
    repository: &std::path::Path,
    document: DocumentId,
    location: slterm_path::ResolvedPath,
    binding: &RepositoryBinding,
) {
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(repository).unwrap()).unwrap(),
        )
        .unwrap();
    let key = ViewKey::Diff {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), document, location)
        .unwrap()
        .0
        .with_opened_view(project_id(PROJECT_A), group_id(GROUP_A), key.clone())
        .unwrap();
    let state = crate::app_shell::text_view::DiffViewState::new(slterm_document::DiffBinding::new(
        binding.clone(),
        "first.txt".parse().unwrap(),
    ));
    let registry = super::super::side_view::workspace_view_state_registry();
    let state =
        slterm_workspace::ViewState::new(&key, serde_json::to_value(state).unwrap(), &registry)
            .unwrap();
    let mut database = WorkspaceDatabase::open(paths).unwrap();
    database.save_project_snapshot(&layout).unwrap();
    database.save_view_state(&key, &state).unwrap();
}

fn assert_diff_recovery_placeholder(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    selector: &'static str,
) {
    let (panel_count, recovery_count) = root.read_with(cx, |root, cx| {
        let panels = &root.projects[&project_id(PROJECT_A)].read(cx).panels;
        (
            panels.len(),
            panels
                .values()
                .filter(|panel| matches!(&panel.content, PanelContent::Recovery(_)))
                .count(),
        )
    });
    assert_eq!((panel_count, recovery_count), (2, 2));
    assert!(window.debug_bounds(selector).is_some());
}

fn diff_view(root: &Entity<AppRoot>, panel: PanelId, cx: &TestAppContext) -> Entity<DiffView> {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .and_then(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .unwrap()
    })
}

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
        editor.update(cx, |view, cx| view.set_git_marks(vec![1, 3], cx));
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
        vec![2],
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
fn diff_hidden_head_refresh_coalesced(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n"
    );

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    for (label, text) in [("second", "second head\n"), ("third", "third head\n")] {
        std::fs::write(repository.join("first.txt"), text).unwrap();
        super::gitshow::git(&repository, &["add", "--", "first.txt"]);
        super::gitshow::git(&repository, &["commit", "--quiet", "-m", label]);
        window.update(|_, cx| {
            root.update(cx, |root, cx| root.sync_project_visibility(cx));
        });
    }
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n",
        "隐藏期间不得刷新固定 HEAD"
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
            root.sync_project_visibility(cx);
        });
    });
    draw_frame(&mut window);

    let head =
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.head_bytes_for_test().to_vec());
    let generation = root.read_with(cx, |root, _| {
        root.tasks.view_refresh_generation_for_test(
            super::super::task_service::ViewTaskOwner::Panel(panel),
        )
    });
    assert_eq!(
        (head, generation),
        (b"third head\n".to_vec(), Some(2)),
        "重新可见只合并复核一次，并采用原仓库最新 HEAD"
    );

    let editor = ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(editor, window, cx).unwrap();
        });
    });
    std::fs::write(repository.join("first.txt"), "fourth head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "fourth"]);
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.sync_project_visibility(cx));
    });
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"third head\n",
        "同组非 active 的 Diff 同样隐藏"
    );

    let diff_key = ViewKey::Diff {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(diff_key, window, cx).unwrap();
        });
    });
    draw_frame(&mut window);
    let head =
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.head_bytes_for_test().to_vec());
    let generation = root.read_with(cx, |root, _| {
        root.tasks.view_refresh_generation_for_test(
            super::super::task_service::ViewTaskOwner::Panel(panel),
        )
    });
    assert_eq!((head, generation), (b"fourth head\n".to_vec(), Some(3)));
    let refreshed = diff_view(&root, panel, cx)
        .read_with(cx, |view, _| view.view_state_for_test().binding().unwrap());
    assert_eq!(
        refreshed.repository_binding(),
        &current_binding(&repository),
        "复核成功必须推进唯一 ViewState 中的提交身份"
    );
}

#[gpui::test]
fn diff_stale_head_completion_preserves_current_refresh(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    let diff = diff_view(&root, panel, cx);
    draw_frame(&mut window);

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    std::fs::write(repository.join("first.txt"), "second head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "second"]);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    let owner = super::super::task_service::ViewTaskOwner::Panel(panel);
    let stale = root.read_with(cx, |root, _| {
        root.tasks
            .current_view_refresh_ticket_for_test(owner)
            .unwrap()
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_B), window, cx)
                .unwrap();
        });
    });
    std::fs::write(repository.join("first.txt"), "third head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "third"]);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    let (current, task_identity) = root.read_with(cx, |root, _| {
        (
            root.tasks
                .current_view_refresh_ticket_for_test(owner)
                .unwrap(),
            root.tasks
                .task_identity(super::super::task_service::TaskId::DiffHead(panel))
                .unwrap(),
        )
    });
    assert_ne!(stale.generation(), current.generation());

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_diff_head(
                panel,
                diff.downgrade(),
                stale,
                Err(crate::git::GitError::Cancelled),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        assert!(
            root.tasks
                .is_registered(super::super::task_service::TaskId::DiffHead(panel)),
            "stale 回执不得移除新任务"
        );
        assert_eq!(
            root.tasks
                .task_identity(super::super::task_service::TaskId::DiffHead(panel)),
            Some(task_identity)
        );
        assert_eq!(
            root.tasks.current_view_refresh_ticket_for_test(owner),
            Some(current)
        );
    });
    assert_eq!(
        diff.read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n",
        "stale 错误不得覆盖当前固定正文"
    );

    draw_frame(&mut window);
    assert_eq!(
        diff.read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"third head\n"
    );
}

#[gpui::test]
fn diff_close_cancels_refresh_and_releases_ticket(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_all_views_from_request(group_id(GROUP_A), BTreeMap::new(), window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert!(
            !root
                .tasks
                .is_registered(super::super::task_service::TaskId::DiffHead(panel))
        );
        assert_eq!(
            root.tasks.view_refresh_generation_for_test(
                super::super::task_service::ViewTaskOwner::Panel(panel),
            ),
            None,
            "Panel 销毁必须释放可见性代次"
        );
    });
}

#[gpui::test]
fn diff_does_not_rebind_after_relocation(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    let moved = _temp.path().join("moved");
    std::fs::rename(&repository, &moved).unwrap();
    std::fs::create_dir(&repository).unwrap();
    super::gitshow::git(&repository, &["init", "--quiet"]);
    super::gitshow::git(&repository, &["config", "core.autocrlf", "false"]);
    super::gitshow::git(&repository, &["config", "user.name", "slTerminal Test"]);
    super::gitshow::git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("first.txt"), "imposter head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "imposter"]);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    draw_frame(&mut window);

    let diff = diff_view(&root, panel, cx);
    let (head, current_binding) = diff.read_with(cx, |view, _| {
        (
            view.head_bytes_option_for_test(),
            view.view_state_for_test().binding().unwrap(),
        )
    });
    assert_eq!(
        head.as_deref(),
        Some(b"fixed first\n".as_slice()),
        "刷新失败不得丢弃原固定快照或采用重定位路径的新仓库"
    );
    assert_eq!(current_binding.repository_binding().head(), binding.head());
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

#[gpui::test]
fn diff_open_respects_snapshot_commit_barrier(cx: &mut TestAppContext) {
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
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::close::flush_when_ready(&runtime.client());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let database_lock = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    database_lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_diff(
                DiffOpenRequest {
                    project_id: project_id(PROJECT_A),
                    target_group: Some(group_id(GROUP_A)),
                    document_id: document,
                    binding,
                    path: RepoPath::from_str("first.txt").unwrap(),
                    focus: true,
                    target_line: None,
                },
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        rejected,
        Err(super::super::commands::AppRootError::ProjectSnapshotCommitInFlight)
    ));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    database_lock.execute_batch("ROLLBACK").unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
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
