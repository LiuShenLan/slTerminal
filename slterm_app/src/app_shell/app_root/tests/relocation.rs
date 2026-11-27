//! 项目根重定位：冲突与 Hooks 草稿守卫、历史归属重算、既有绑定保持。

use std::str::FromStr as _;

use slterm_agents::AgentKind;
use slterm_document::RepoPath;
use slterm_workspace::ProjectRegistryError;

use super::super::diff::DiffOpenRequest;
use super::super::git_show::GitShowOpenRequest;
use super::super::navigation::deepest_project_for_cwd;
use super::super::panel_host::PanelContent;
use super::*;
use super::nav::open_root_with_windows;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer, HooksDraftOwner};

fn project_root(root: &Entity<AppRoot>, cx: &TestAppContext, id: ProjectId) -> CanonicalPath {
    root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(id)
            .expect("项目必须存在")
            .root()
            .clone()
    })
}

fn relocate(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: ProjectId,
    new_root: CanonicalPath,
) -> Result<(), AppRootError> {
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.relocate_project_from_request(id, new_root, cx)
        })
    })
}

fn attribute(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    cwd: &CanonicalPath,
) -> Option<ProjectId> {
    root.read_with(cx, |root, _| {
        deepest_project_for_cwd(
            root.snapshot
                .registry()
                .projects()
                .iter()
                .map(|project| (project.id(), project.root())),
            cwd,
        )
    })
}

fn draft_key(panel: PanelId, layer: HooksDraftLayer) -> HooksDraftKey {
    HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
        agent: AgentKind::Claude,
        layer,
    }
}

fn clean_draft() -> HooksDraft {
    let text = r#"{"Stop":[]}"#;
    HooksDraft::loaded(
        None,
        text.to_owned(),
        crate::ai_hook::draft::parse_hooks_json(text).unwrap(),
    )
}

/// 冲突与 Project／Local 未提交草稿必须先解决；干净草稿与 User 草稿不阻塞；
/// 重定位后历史按最新词法根重算，既有终端 cwd 保持。
#[gpui::test]
fn navigation_relocation_guards_and_reassigns_history(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "C:/work/old");
    new_generated_terminal(&root, &mut window);
    add_project(&root, &mut window, PROJECT_B, "C:/other");
    assert_eq!(state.borrow().attempts, 1);

    let old_root = path("C:/work/old");
    let new_root = path("C:/work/new");
    let new_cwd = path("C:/work/new/sub");
    let old_cwd = path("C:/work/old/sub");
    assert_eq!(attribute(&root, cx, &new_cwd), None);
    assert_eq!(attribute(&root, cx, &old_cwd), Some(project_id(PROJECT_A)));

    let conflict = relocate(&root, &mut window, project_id(PROJECT_A), path("C:/other"));
    assert!(
        matches!(
            conflict,
            Err(AppRootError::Registry(ProjectRegistryError::DuplicateRoot(id)))
                if id == project_id(PROJECT_B)
        ),
        "新根与既有项目冲突必须拒绝: {conflict:?}"
    );
    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), old_root);

    let panel = PanelIdAllocator::new(7).allocate().unwrap();
    let local_key = draft_key(panel, HooksDraftLayer::Local(project_id(PROJECT_A)));
    root.update(cx, |root, _| {
        let mut dirty = clean_draft();
        dirty.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        root.hook_drafts.insert(local_key, dirty);
    });
    let blocked = relocate(&root, &mut window, project_id(PROJECT_A), new_root.clone());
    assert!(
        matches!(blocked, Err(AppRootError::HooksDraftPending)),
        "未提交的 Project／Local 草稿必须拒绝: {blocked:?}"
    );
    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), old_root);

    root.update(cx, |root, _| {
        root.hook_drafts.clear_project(project_id(PROJECT_A));
        root.hook_drafts.insert(
            draft_key(panel, HooksDraftLayer::Project(project_id(PROJECT_A))),
            clean_draft(),
        );
        root.hook_drafts.insert(local_key, clean_draft());
        root.hook_drafts
            .insert(draft_key(panel, HooksDraftLayer::User), clean_draft());
    });
    relocate(&root, &mut window, project_id(PROJECT_A), new_root.clone()).unwrap();
    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), new_root);

    assert_eq!(attribute(&root, cx, &new_cwd), Some(project_id(PROJECT_A)));
    assert_eq!(
        attribute(&root, cx, &old_cwd),
        None,
        "旧根历史不得继续归属原项目"
    );
    root.read_with(cx, |root, _| {
        let terminal = root
            .snapshot
            .terminals()
            .find(|terminal| terminal.project_id() == project_id(PROJECT_A))
            .expect("既有终端必须保留");
        assert_eq!(terminal.cwd(), &old_root, "根重定位不得改写既有终端 cwd");
    });
    assert_eq!(state.borrow().attempts, 1, "重定位不得重建 PTY");
}

/// 根路径变化即授权撤销：授权代次必须推进，旧计划与在途保存失效。
#[gpui::test]
fn project_relocation_advances_the_authorization_generation(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let old_root = temp.path().join("old");
    let new_root = temp.path().join("new");
    std::fs::create_dir(&old_root).unwrap();
    std::fs::create_dir(&new_root).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, old_root.to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.current_authorization_generation());
    let new_root = CanonicalPath::new(dunce::canonicalize(&new_root).unwrap()).unwrap();
    relocate(&root, &mut window, project_id(PROJECT_A), new_root.clone()).unwrap();
    let after = root.read_with(cx, |root, _| root.current_authorization_generation());
    assert!(
        after > before,
        "重定位后授权代次必须推进: {before} -> {after}"
    );
}

/// 重命名与排序不改动任何路径，不得推进授权代次或打断在途保存。
#[gpui::test]
fn project_rename_and_reorder_keep_the_authorization_generation(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("a");
    let root_b = temp.path().join("b");
    std::fs::create_dir(&root_a).unwrap();
    std::fs::create_dir(&root_b).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, root_b.to_str().unwrap());
    let before = root.read_with(cx, |root, _| root.current_authorization_generation());

    root.update(cx, |root, cx| {
        root.rename_project_from_request(project_id(PROJECT_A), "Renamed".to_owned(), cx)
            .unwrap();
        root.move_project_from_request(project_id(PROJECT_A), 1, cx)
            .unwrap();
    });

    let after = root.read_with(cx, |root, _| root.current_authorization_generation());
    assert_eq!(
        after, before,
        "重命名与排序不得推进授权代次: {before} -> {after}"
    );
}

/// 项目移除同样要求绑定该项目的未提交 Hooks 草稿先解决；干净草稿与 User 草稿不阻塞。
#[gpui::test]
fn project_remove_requires_hooks_drafts_resolved(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "C:/work/remove");
    let panel = PanelIdAllocator::new(8).allocate().unwrap();
    let local_key = draft_key(panel, HooksDraftLayer::Local(project_id(PROJECT_A)));
    root.update(cx, |root, _| {
        let mut dirty = clean_draft();
        dirty.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        root.hook_drafts.insert(local_key, dirty);
    });
    let blocked = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.remove_project(project_id(PROJECT_A), window, cx)
        })
    });
    assert!(blocked.is_ok(), "项目专属未提交草稿必须先进入决定面");
    assert!(root.read_with(cx, |root, _| root.close_decision.is_some()));
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot
                .registry()
                .project(project_id(PROJECT_A))
                .is_some(),
            "决定面打开时项目关系必须保持"
        );
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.cancel_close_decision(window, cx).unwrap()
        });
    });

    root.update(cx, |root, _| {
        root.hook_drafts.clear_project(project_id(PROJECT_A));
        root.hook_drafts.insert(local_key, clean_draft());
        root.hook_drafts
            .insert(draft_key(panel, HooksDraftLayer::User), clean_draft());
    });
    let attempted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.remove_project(project_id(PROJECT_A), window, cx)
        })
    });
    assert!(
        !matches!(attempted, Err(AppRootError::HooksDraftPending)),
        "干净草稿与 User 草稿不得阻塞项目移除: {attempted:?}"
    );
}

/// 根重定位不改绑既有 Diff 与 GitShow 的仓库绑定与固定 HEAD 路径。
#[gpui::test]
fn repository_binding_survives_project_relocation(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let diff = window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.open_diff(
                    DiffOpenRequest {
                        project_id: project_id(PROJECT_A),
                        target_group: Some(group_id(GROUP_A)),
                        document_id: document,
                        binding: binding.clone(),
                        path: RepoPath::from_str("first.txt").unwrap(),
                        focus: true,
                        target_line: None,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    let gitshow = window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.open_gitshow(
                    GitShowOpenRequest {
                        project_id: project_id(PROJECT_A),
                        target_group: Some(group_id(GROUP_A)),
                        binding: binding.clone(),
                        path: RepoPath::from_str("first.txt").unwrap(),
                        focus: true,
                        activate_project: false,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    draw_frame(&mut window);

    let new_root = temp.path().join("relocated");
    std::fs::create_dir(&new_root).unwrap();
    let new_root = CanonicalPath::new(dunce::canonicalize(&new_root).unwrap()).unwrap();
    relocate(&root, &mut window, project_id(PROJECT_A), new_root.clone()).unwrap();
    draw_frame(&mut window);

    root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        let PanelContent::Diff(view) = &project.panels[&diff].content else {
            panic!("Diff 面板必须保留");
        };
        let fixed = view
            .read(cx)
            .view_state_for_test()
            .binding()
            .expect("Diff 固定绑定必须有效");
        assert_eq!(
            fixed.repository_binding(),
            &binding,
            "Diff 绑定不得随项目根重定位改绑"
        );
        let PanelContent::GitShow(_) = &project.panels[&gitshow].content else {
            panic!("GitShow 面板必须保留");
        };
        let ViewKey::GitShow {
            repository_binding, ..
        } = &project.panels[&gitshow].view_key
        else {
            panic!("GitShow 面板必须持有固定仓库绑定");
        };
        assert_eq!(
            repository_binding.as_ref(),
            &binding,
            "GitShow 绑定不得随项目根重定位改绑"
        );
    });
}

/// 真实目录端到端：重定位后既有 Document 位置不变，新根内文件可按新根授权打开。
#[gpui::test]
fn integration_project_relocation(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let old_root = temp.path().join("old");
    let new_root = temp.path().join("new");
    std::fs::create_dir(&old_root).unwrap();
    std::fs::create_dir(&new_root).unwrap();
    let old_file = old_root.join("old.txt");
    let new_file = new_root.join("new.txt");
    std::fs::write(&old_file, "old\n").unwrap();
    std::fs::write(&new_file, "new\n").unwrap();

    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, old_root.to_str().unwrap());
    let old_file = CanonicalPath::new(dunce::canonicalize(&old_file).unwrap()).unwrap();
    let new_file = CanonicalPath::new(dunce::canonicalize(&new_file).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, old_file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);

    let resolved = CanonicalPath::new(dunce::canonicalize(&new_root).unwrap()).unwrap();
    relocate(&root, &mut window, project_id(PROJECT_A), resolved.clone()).unwrap();
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .root(),
            &resolved
        );
        assert_eq!(
            root.snapshot
                .documents()
                .find(|(id, _)| *id == document)
                .map(|(_, location)| location.canonical_path().as_path()),
            Some(old_file.as_path()),
            "既有 Document 位置不得随项目根重定位变化"
        );
    });

    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, new_file);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot().documents().count(),
            2,
            "新根内文件必须能按重定位后的项目根授权打开"
        );
    });
}

/// 重定位生产入口遇到脏 Hooks 草稿：决定面丢弃后才改根。
#[gpui::test]
fn relocation_entry_discards_hooks_drafts_through_the_decision_surface(cx: &mut TestAppContext) {
    use crate::app_shell::app_root::close_decision::CloseDecisionChoice;
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old");
    let new = temp.path().join("new");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_folder_target(Some(new.clone()));
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    add_project(&root, &mut window, PROJECT_A, old.to_str().unwrap());
    let panel = PanelIdAllocator::new(7).allocate().unwrap();
    root.update(cx, |root, _| {
        let mut dirty = clean_draft();
        dirty.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        root.hook_drafts.insert(
            draft_key(panel, HooksDraftLayer::Local(project_id(PROJECT_A))),
            dirty,
        );
    });
    let old_root = project_root(&root, cx, project_id(PROJECT_A));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_project_relocation(project_id(PROJECT_A), window, cx);
        });
    });
    for _ in 0..3 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    assert!(
        root.read_with(cx, |root, _| root.close_decision.is_some()),
        "脏草稿必须先打开决定面"
    );
    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), old_root);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.resolve_close_decision(CloseDecisionChoice::Discard, window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
    let expected = CanonicalPath::new(dunce::canonicalize(&new).unwrap()).unwrap();
    assert_eq!(
        project_root(&root, cx, project_id(PROJECT_A)),
        expected,
        "丢弃草稿后必须完成重定位"
    );
    root.read_with(cx, |root, _| {
        assert!(
            !root.hook_drafts.project_has_drafts(project_id(PROJECT_A)),
            "重定位前草稿必须已丢弃"
        );
    });
}

/// 决定面取消：根关联与草稿都不变。
#[gpui::test]
fn relocation_entry_cancel_keeps_root_and_drafts(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old");
    let new = temp.path().join("new");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_folder_target(Some(new.clone()));
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    add_project(&root, &mut window, PROJECT_A, old.to_str().unwrap());
    let panel = PanelIdAllocator::new(7).allocate().unwrap();
    root.update(cx, |root, _| {
        let mut dirty = clean_draft();
        dirty.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        root.hook_drafts.insert(
            draft_key(panel, HooksDraftLayer::Local(project_id(PROJECT_A))),
            dirty,
        );
    });
    let old_root = project_root(&root, cx, project_id(PROJECT_A));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_project_relocation(project_id(PROJECT_A), window, cx);
        });
    });
    for _ in 0..3 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.cancel_close_decision(window, cx).unwrap();
        });
    });

    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), old_root);
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.project_has_drafts(project_id(PROJECT_A)));
        assert!(root.pending_project_relocation.is_none());
    });
}
