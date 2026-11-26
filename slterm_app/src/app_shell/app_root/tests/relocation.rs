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
    window.update(|_, cx| root.update(cx, |root, cx| root.relocate_project(id, new_root, cx)))
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
    assert!(
        matches!(blocked, Err(AppRootError::HooksDraftPending)),
        "项目专属未提交草稿必须拒绝移除: {blocked:?}"
    );
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot
                .registry()
                .project(project_id(PROJECT_A))
                .is_some(),
            "拒绝后项目关系必须保持"
        );
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
