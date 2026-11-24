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
}
