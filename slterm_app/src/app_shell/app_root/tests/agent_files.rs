//! Agent 全局文件树：多根渲染、过滤与 ViewState 恢复。

use super::*;

use crate::agent_dirs::AgentGlobalDir;
use slterm_settings::AgentRootId;

fn agent_root_id() -> AgentRootId {
    AgentRootId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c").unwrap()
}

fn second_agent_root_id() -> AgentRootId {
    AgentRootId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d").unwrap()
}

fn canonical(path: std::path::PathBuf) -> CanonicalPath {
    CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap()
}

fn agent_dirs(path: &std::path::Path, second: &std::path::Path) -> Vec<AgentGlobalDir> {
    vec![
        AgentGlobalDir {
            cli_id: "claude".to_owned(),
            root_id: agent_root_id(),
            display_name: "Claude Code".to_owned(),
            path: path.to_path_buf(),
            exists: true,
        },
        AgentGlobalDir {
            cli_id: "codex".to_owned(),
            root_id: second_agent_root_id(),
            display_name: "Codex".to_owned(),
            path: second.to_path_buf(),
            exists: true,
        },
    ]
}

fn toggle_agent_files(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Option<Entity<super::super::side_view::SideView>> {
    window.update(|window, cx| {
        let _ = root.update(cx, |root, cx| {
            root.activate_side_view(SideViewId::AgentFiles, window, cx)
        });
    });
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::AgentFiles).cloned()
    })
}

fn open_agent_files(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    toggle_agent_files(root, window, cx).expect("AgentFiles 必须已打开")
}

#[gpui::test]
fn agent_files_view_state_restore(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    let agent_work = temp.path().join("agent-work");
    let agent_work_b = temp.path().join("agent-work-b");
    std::fs::create_dir_all(&agent_work_b).unwrap();
    std::fs::write(
        agent_work_b.join("codex.txt"),
        "codex
",
    )
    .unwrap();
    std::fs::create_dir_all(agent_work.join("skills")).unwrap();
    std::fs::write(agent_work.join("skills/run.md"), "skill\n").unwrap();
    std::fs::write(agent_work.join("note.txt"), "note\n").unwrap();
    // 足够多的行产生真实滚动空间，滚动 offset 不会被空内容夹紧。
    for index in 0..80 {
        std::fs::write(
            agent_work.join(format!("filler-{index:02}.txt")),
            "filler\n",
        )
        .unwrap();
    }
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();

    let skills = canonical(agent_work.join("skills"));
    let note = canonical(agent_work.join("note.txt"));
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(agent_dirs(&agent_work, &agent_work_b));
    });
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );

    let view = open_agent_files(&root, &mut window, cx);
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_activate_path(&skills, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_activate_path(&note, cx);
            view.agent_files_set_multi_selection_for_test(note.clone());
            view.agent_files_scroll_handle_for_test()
                .unwrap()
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-120.0)));
        });
    });

    let (offset, max_offset) = view.read_with(cx, |view, _| {
        let handle = view.agent_files_scroll_handle_for_test().unwrap();
        (handle.offset().y, handle.max_offset().y)
    });
    assert!(
        (-offset.as_f32() - 120.0).abs() < 1.5 && max_offset.as_f32() > 120.0,
        "内容必须可滚动且 offset 生效: max={max_offset:?} offset={offset:?}"
    );

    // 折叠第二个根制造与默认值相反的持久状态，恢复测试才有判别力。
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_toggle_root(second_agent_root_id(), cx);
        });
    });
    // 再次触发同一快捷键关闭视图并提交检查点。
    toggle_agent_files(&root, &mut window, cx);
    super::close::flush_when_ready(&runtime.client());
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let key = ViewKey::SideView {
        side_view_id: slterm_workspace::SideViewId::AgentFiles,
    };
    let stored = store
        .view_state(
            &key,
            &super::super::side_view::workspace_view_state_registry(),
        )
        .unwrap()
        .expect("AgentFiles ViewState 必须已持久化");
    assert_eq!(
        stored.state.state()["current_row"]["path"].as_str(),
        Some("note.txt")
    );
    assert_eq!(stored.state.state()["scroll"].as_f64(), Some(120.0));

    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (reopened, mut window, _) = open_root_with_persistence(cx, persistence);
    reopened.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(agent_dirs(&agent_work, &agent_work_b));
    });
    let view = open_agent_files(&reopened, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    view.read_with(cx, |view, _| {
        assert!(
            view.agent_files_expanded_for_test(agent_root_id()),
            "默认展开的根必须恢复展开"
        );
        assert!(
            !view.agent_files_expanded_for_test(second_agent_root_id()),
            "折叠的根必须保持折叠（默认展开不得覆盖持久状态）"
        );
        assert_eq!(
            view.agent_files_selected_text_for_test().as_deref(),
            Some("note.txt"),
            "当前行必须恢复"
        );
        let offset = view
            .agent_files_scroll_handle_for_test()
            .unwrap()
            .offset()
            .y;
        assert!(
            (-offset.as_f32() - 120.0).abs() < 1.5,
            "滚动位置必须恢复，actual={offset:?}"
        );
        assert!(
            view.agent_files_multi_selection_is_empty_for_test(),
            "多选属于瞬态，恢复后为空"
        );
    });
}

#[gpui::test]
fn agent_files_open_reuses_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("agent-project");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::write(project_root.join("note.txt"), "one\ntwo\n").unwrap();
    std::fs::write(project_root.join("other.txt"), "other\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let file =
        CanonicalPath::new(dunce::canonicalize(project_root.join("note.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(agent_dirs(&project_root, &project_root));
    });

    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    let other =
        CanonicalPath::new(dunce::canonicalize(project_root.join("other.txt")).unwrap()).unwrap();
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_open_at(&other, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.documents.len(),
            2,
            "Agent 树打开必须真实经过统一打开链"
        );
    });

    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_open_at(&file, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.documents.len(),
            2,
            "再次打开同一文件不得新建第二份 Document"
        );
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        let note_views = project
            .panels
            .values()
            .filter(|panel| panel.editor().is_some())
            .count();
        assert_eq!(note_views, 2, "两个文件各一个编辑器，note 不得重复");
    });
}

#[gpui::test]
fn agent_root_capability_union_and_revoke(cx: &mut TestAppContext) {
    use slterm_path::{PathOperation, PathPolicy, PathRequest, ResolvedPath};

    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("union-project");
    let agent_dir = temp.path().join("union-agent");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("note.txt"), "one\n").unwrap();
    let note = canonical(agent_dir.join("note.txt"));
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let writable = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "Agent".to_owned(),
        agent_dir.to_string_lossy().into_owned(),
    )
    .unwrap()
    .with_capabilities(slterm_settings::AgentRootCapabilities {
        write: true,
        terminal: false,
    });
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![writable]);
    });

    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_open_at(&note, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    let document = root.read_with(cx, |root, _| {
        assert_eq!(root.documents.len(), 1, "项目外 Agent 文件必须能打开");
        root.snapshot().documents().next().unwrap().0
    });
    let write_request = PathRequest {
        operation: PathOperation::Write,
        lexical_path: note.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    let resolved = ResolvedPath::new(note.clone(), None);
    root.read_with(cx, |root, _| {
        let grants = root.document_grants(document);
        assert!(
            PathPolicy::authorize(&write_request, &resolved, &grants, 0).is_ok(),
            "可写 Agent 根必须提供保存授权"
        );
    });
    // 关闭保存必须复用同一能力并集，而不是只从项目根派生授权。
    let close_intent = slterm_workspace::CloseIntent::new(
        vec![slterm_workspace::CloseObject {
            id: slterm_workspace::CloseTarget::Document(document),
            generation: 0,
            decision: slterm_workspace::CloseDecision::Save,
        }],
        [1; 32],
    )
    .unwrap();
    root.read_with(cx, |root, _| {
        let grants = root.document_close_grants(&close_intent);
        let grants = grants
            .get(&document)
            .expect("关闭意图中的文档必须有授权集合");
        assert!(
            PathPolicy::authorize(&write_request, &resolved, grants, 0).is_ok(),
            "批量/项目/应用关闭保存必须叠加可写 Agent 根"
        );
    });

    root.update(cx, |root, _| {
        root.replace_custom_agent_roots_for_test(Vec::new());
    });
    root.read_with(cx, |root, _| {
        let grants = root.document_grants(document);
        assert!(
            PathPolicy::authorize(&write_request, &resolved, &grants, 0).is_err(),
            "撤销可写根后保存必须失去写授权"
        );
    });
    root.read_with(cx, |root, _| {
        let grants = root.document_close_grants(&close_intent);
        let grants = grants
            .get(&document)
            .expect("关闭意图中的文档必须有授权集合");
        assert!(
            PathPolicy::authorize(&write_request, &resolved, grants, 0).is_err(),
            "撤销可写根后关闭保存同样失去授权"
        );
    });
}

#[gpui::test]
fn agent_root_path_change_resets_state_and_authorization(cx: &mut TestAppContext) {
    use slterm_path::{PathOperation, PathPolicy, PathRequest, ResolvedPath};

    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("path-project");
    let root_a_dir = temp.path().join("path-root-a");
    let root_b_dir = temp.path().join("path-root-b");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(root_a_dir.join("skills")).unwrap();
    std::fs::write(root_a_dir.join("skills/run.md"), "a\n").unwrap();
    std::fs::create_dir_all(&root_b_dir).unwrap();
    let skills_dir = canonical(root_a_dir.join("skills"));
    let skill = canonical(root_a_dir.join("skills/run.md"));
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let custom_a = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "A".to_owned(),
        root_a_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    let custom_b = slterm_settings::AgentRoot::custom(
        second_agent_root_id(),
        "B".to_owned(),
        root_b_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_a, custom_b.clone()]);
    });
    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_activate_path(&skills_dir, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_activate_path(&skill, cx);
        });
    });
    window.run_until_parked();

    window.update(|_, cx| {
        view.update(cx, |view, _| view.checkpoint_agent_files_state());
    });
    view.read_with(cx, |view, _| {
        let slterm_sidebar::SideViewState::AgentFiles(state) = &view.state else {
            panic!("必须是 AgentFiles 状态");
        };
        assert_eq!(
            state.root_expanded(agent_root_id()),
            Some(true),
            "撤销前旧根分区必须真实存在"
        );
    });

    let generation_before = root.read_with(cx, |root, _| root.current_authorization_generation());
    // 同一根 id 改路径必须走真实同步入口：旧分区与当前行一并撤销。
    let moved = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "A".to_owned(),
        root_b_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.replace_custom_agent_roots_for_test(vec![moved, custom_b.clone()]);
            root.sync_agent_files_roots(window, cx);
        });
    });
    window.run_until_parked();
    let generation_after = root.read_with(cx, |root, _| root.current_authorization_generation());
    assert!(
        generation_after > generation_before,
        "Agent 根路径变化必须推进授权代次"
    );
    view.read_with(cx, |view, _| {
        let slterm_sidebar::SideViewState::AgentFiles(state) = &view.state else {
            panic!("必须是 AgentFiles 状态");
        };
        assert_eq!(
            state.root_expanded(agent_root_id()),
            None,
            "改路径后旧根分区必须移除"
        );
        assert_eq!(
            view.agent_files_selected_text_for_test(),
            None,
            "旧根当前行不得套用"
        );
    });

    let grants = root.read_with(cx, |root, _| root.agent_root_grants_for_path(&skill));
    let request = PathRequest {
        operation: PathOperation::Write,
        lexical_path: skill.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    let resolved = ResolvedPath::new(skill.clone(), None);
    assert!(
        PathPolicy::authorize(&request, &resolved, &grants, 0).is_err(),
        "改路径后旧根授权不得套用"
    );
}

#[gpui::test]
fn agent_root_delete_preserves_document_and_pty(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("delete-project");
    let agent_dir = temp.path().join("delete-agent");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("note.txt"), "one\n").unwrap();
    let note = canonical(agent_dir.join("note.txt"));
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let custom = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "Agent".to_owned(),
        agent_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom]);
    });
    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_open_at(&note, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(root.documents.len(), 1);
        assert!(
            root.terminals
                .borrow()
                .target(terminal_id(TERMINAL_A))
                .is_some(),
            "删除根前终端必须存活"
        );
    });

    root.update(cx, |root, _| {
        root.replace_custom_agent_roots_for_test(Vec::new());
    });
    root.update(cx, |root, cx| root.sync_agent_files_roots_after_load(cx));
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(root.documents.len(), 1, "删除根不得删除 Document");
        assert!(
            root.terminals
                .borrow()
                .target(terminal_id(TERMINAL_A))
                .is_some(),
            "删除根不得强杀既有 PTY"
        );
    });
    view.read_with(cx, |view, _| {
        assert!(
            !view.agent_files_expanded_for_test(agent_root_id()),
            "删除根后运行态不得保留"
        );
        assert_eq!(view.agent_files_selected_text_for_test(), None);
        let slterm_sidebar::SideViewState::AgentFiles(state) = &view.state else {
            panic!("必须是 AgentFiles 状态");
        };
        assert_eq!(state.root_expanded(agent_root_id()), None);
    });
}

#[gpui::test]
fn agent_files_filter_is_not_authorization(cx: &mut TestAppContext) {
    use slterm_path::{PathOperation, PathPolicy, PathRequest, ResolvedPath};
    use slterm_sidebar::DirectoryStatus;

    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("filter-project");
    let alpha_dir = temp.path().join("filter-alpha");
    let beta_dir = temp.path().join("filter-beta");
    let gamma_dir = temp.path().join("filter-gamma");
    for dir in [&project_root, &alpha_dir, &beta_dir, &gamma_dir] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(alpha_dir.join("a.txt"), "a\n").unwrap();
    std::fs::write(beta_dir.join("b.txt"), "b\n").unwrap();
    std::fs::write(gamma_dir.join("c.txt"), "c\n").unwrap();
    let beta_file = canonical(beta_dir.join("b.txt"));
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let alpha = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "Alpha".to_owned(),
        alpha_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    let beta = slterm_settings::AgentRoot::custom(
        second_agent_root_id(),
        "Beta".to_owned(),
        beta_dir.to_string_lossy().into_owned(),
    )
    .unwrap()
    .with_capabilities(slterm_settings::AgentRootCapabilities {
        write: true,
        terminal: false,
    });
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![alpha.clone(), beta.clone()]);
    });
    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_set_filter("alpha", cx);
        });
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.agent_files_visible_roots_for_test()),
        vec!["Alpha".to_owned()],
        "过滤只显示名称匹配的根"
    );

    // 过滤生效后新增的隐藏根不得被读取（不发起目录请求）。
    let gamma = slterm_settings::AgentRoot::custom(
        AgentRootId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e").unwrap(),
        "Gamma".to_owned(),
        gamma_dir.to_string_lossy().into_owned(),
    )
    .unwrap();
    root.update(cx, |root, _| {
        root.replace_custom_agent_roots_for_test(vec![alpha, beta, gamma]);
    });
    root.update(cx, |root, cx| root.sync_agent_files_roots_after_load(cx));
    window.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.agent_files_root_status_for_test(
                AgentRootId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e").unwrap()
            ),
            Some(DirectoryStatus::NotRequested),
            "被过滤根不得发起目录读取"
        );
    });

    // 过滤不是授权：隐藏的可写根仍提供保存写授权。
    let grants = root.read_with(cx, |root, _| root.agent_root_grants_for_path(&beta_file));
    let request = PathRequest {
        operation: PathOperation::Write,
        lexical_path: beta_file.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    let resolved = ResolvedPath::new(beta_file.clone(), None);
    assert!(
        PathPolicy::authorize(&request, &resolved, &grants, 0).is_ok(),
        "显示过滤不得改变授权"
    );
}

#[gpui::test]
fn integration_path_capability_union(cx: &mut TestAppContext) {
    use slterm_path::{PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};

    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("union-cross-project");
    let agent_dir = temp.path().join("union-cross-agent");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("note.txt"), "one\n").unwrap();
    let note = canonical(agent_dir.join("note.txt"));
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let writable = slterm_settings::AgentRoot::custom(
        agent_root_id(),
        "Agent".to_owned(),
        agent_dir.to_string_lossy().into_owned(),
    )
    .unwrap()
    .with_capabilities(slterm_settings::AgentRootCapabilities {
        write: true,
        terminal: false,
    });
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![writable]);
    });
    let view = open_agent_files(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_open_at(&note, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let resolved = ResolvedPath::new(note.clone(), None);
    let request = |operation| PathRequest {
        operation,
        lexical_path: note.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    let read_only_repo = [PathGrant::repo_root(canonical(agent_dir.clone()))];
    assert!(
        PathPolicy::authorize(&request(PathOperation::Read), &resolved, &read_only_repo, 0).is_ok(),
        "只读仓库根必须提供读取"
    );
    assert!(
        PathPolicy::authorize(
            &request(PathOperation::Write),
            &resolved,
            &read_only_repo,
            0
        )
        .is_err(),
        "只读仓库根不得提供写入"
    );
    root.read_with(cx, |root, _| {
        let grants = root.document_grants(document);
        assert!(
            PathPolicy::authorize(&request(PathOperation::Read), &resolved, &grants, 0).is_ok()
        );
        assert!(
            PathPolicy::authorize(&request(PathOperation::Write), &resolved, &grants, 0).is_ok(),
            "可写 Agent 根与项目根并集必须覆盖保存"
        );
    });
}
