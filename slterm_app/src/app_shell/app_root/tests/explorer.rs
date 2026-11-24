//! Explorer 真实文件树、Document 复用与视图状态测试。

use super::*;

fn open_explorer(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.simulate_keystrokes("ctrl-shift-e");
    window.run_until_parked();
    draw_frame(window);
    root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Explorer)
            .expect("Explorer 必须已打开")
            .clone()
    })
}

fn activate_explorer_row(
    explorer: &Entity<super::super::side_view::SideView>,
    path: &CanonicalPath,
    window: &mut VisualTestContext,
) {
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| view.explorer_activate_at(path, window, cx));
    });
    window.run_until_parked();
    draw_frame(window);
}

#[gpui::test]
fn explorer_open_reuses_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs_root = project_root.join("docs");
    std::fs::create_dir_all(&docs_root).unwrap();
    std::fs::write(docs_root.join("note.txt"), "one\ntwo\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let file =
        CanonicalPath::new(dunce::canonicalize(docs_root.join("note.txt")).unwrap()).unwrap();
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
                "draft\n",
                window,
                cx,
            );
        });
    });

    add_project(&root, &mut window, PROJECT_B, docs_root.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_B);
    let explorer = open_explorer(&root, &mut window, cx);
    activate_explorer_row(&explorer, &file, &mut window);

    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        1,
        "Explorer 打开必须复用另一项目已打开的同一 Document"
    );
    let editor_key = ViewKey::Editor {
        project_id: project_id(PROJECT_B),
        document_id: document,
    };
    assert!(root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_B))
            .unwrap()
            .layout()
            .view_keys()
            .contains(&editor_key)
    }));
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "draft\n",
        "复用必须保留原脏正文"
    );

    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    activate_explorer_row(&explorer, &file, &mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before,
        "同一项目重复打开只还焦既有视图"
    );
}

#[gpui::test]
fn explorer_terminal_open_rejects_invalid_cwd(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let live = project_root.join("live");
    let gone = project_root.join("gone");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::create_dir(&gone).unwrap();
    std::fs::write(project_root.join("note.txt"), "x\n").unwrap();
    let (root, mut window, state) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);

    let gone_path = CanonicalPath::new(dunce::canonicalize(&gone).unwrap()).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let attempts = state.borrow().attempts;
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_open_terminal_at(&gone_path, window, cx);
        });
    });
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before,
        "失效 cwd 不得改变项目快照"
    );
    assert_eq!(
        state.borrow().attempts,
        attempts,
        "失效 cwd 不得启动 worker 或留下空 Panel"
    );

    let live_path = CanonicalPath::new(dunce::canonicalize(&live).unwrap()).unwrap();
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_open_terminal_at(&live_path, window, cx);
        });
    });
    window.run_until_parked();
    assert_eq!(state.borrow().attempts, attempts + 1);
    root.read_with(cx, |root, _| {
        let record = root.snapshot().terminals().next().unwrap();
        assert_eq!(record.cwd(), &live_path, "终端必须落在解析后的真实目录");
    });
}

#[gpui::test]
fn explorer_inline_edit_not_committed_on_destroy(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::write(project_root.join("old.txt"), "x\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);
    let old_path =
        CanonicalPath::new(dunce::canonicalize(project_root.join("old.txt")).unwrap()).unwrap();

    window.update(|window, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_begin_inline_edit(&old_path, window, cx);
        });
    });
    let input = explorer
        .read_with(cx, |view, _| view.explorer_inline_edit_input())
        .expect("内联编辑必须有真实输入实体");
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("renamed.txt", window, cx));
    });
    window.simulate_keystrokes("ctrl-shift-n");
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(project_root.join("old.txt").exists());
    assert!(
        !project_root.join("renamed.txt").exists(),
        "销毁视图不得提交未确认的内联编辑"
    );

    let explorer = open_explorer(&root, &mut window, cx);
    assert!(
        explorer
            .read_with(cx, |view, _| view.explorer_inline_edit_input())
            .is_none(),
        "重开 Explorer 不得恢复已丢弃草稿"
    );

    window.update(|window, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_begin_inline_edit(&old_path, window, cx);
        });
    });
    let input = explorer
        .read_with(cx, |view, _| view.explorer_inline_edit_input())
        .unwrap();
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("renamed.txt", window, cx));
    });
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| view.explorer_commit_inline_edit(window, cx));
    });
    window.run_until_parked();
    assert!(!project_root.join("old.txt").exists());
    assert_eq!(
        std::fs::read_to_string(project_root.join("renamed.txt")).unwrap(),
        "x\n",
        "显式提交必须经真实文件操作完成重命名"
    );
}

fn explorer_project(temp: &tempfile::TempDir) -> (std::path::PathBuf, CanonicalPath) {
    let project_root = temp.path().join("project");
    let docs = project_root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("note.txt"),
        "note
",
    )
    .unwrap();
    std::fs::write(
        project_root.join("other.txt"),
        "other
",
    )
    .unwrap();
    // 足够多的行才能形成真实滚动偏移。
    for index in 0..60 {
        std::fs::write(
            project_root.join(format!("filler-{index:02}.txt")),
            "x
",
        )
        .unwrap();
    }
    let canonical =
        |path: std::path::PathBuf| CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
    (project_root.clone(), canonical(project_root))
}

#[gpui::test]
fn explorer_view_state_restore(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let (project_root, _root_path) = explorer_project(&temp);
    let canonical =
        |path: std::path::PathBuf| CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
    let docs = canonical(project_root.join("docs"));
    let note = canonical(project_root.join("docs/note.txt"));
    let other = canonical(project_root.join("other.txt"));
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);
    activate_explorer_row(&explorer, &docs, &mut window);
    window.update(|_, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_select_at(&note, cx);
            view.explorer_set_show_ignored(true, cx);
            let handle = view.explorer_scroll_handle().unwrap();
            // GPUI 向下滚动 offset 为负值。
            handle.set_offset(gpui::point(gpui::px(0.0), gpui::px(-40.0)));
            assert_eq!(
                handle.offset().y,
                gpui::px(-40.0),
                "闭包内 set_offset 必须生效"
            );
        });
    });

    let (before_close, max_offset) = explorer.read_with(cx, |view, _| {
        let handle = view.explorer_scroll_handle().unwrap();
        (handle.offset().y.as_f32(), handle.max_offset().y.as_f32())
    });
    assert!(
        (-before_close - 40.0).abs() < 1.0,
        "closed_before offset={before_close} max={max_offset}"
    );
    window.simulate_keystrokes("ctrl-shift-n");
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let key = ViewKey::SideView {
        side_view_id: slterm_workspace::SideViewId::Explorer,
    };
    let stored = store
        .view_state(
            &key,
            &super::super::side_view::workspace_view_state_registry(),
        )
        .unwrap()
        .expect("Explorer ViewState 必须已持久化");
    assert_eq!(
        stored.state.state()["projects"][0]["scroll"].as_f64(),
        Some(40.0),
        "关闭检查点必须写入当前滚动"
    );
    let explorer = open_explorer(&root, &mut window, cx);
    window.run_until_parked();
    explorer.read_with(cx, |view, _| {
        assert!(view.explorer_is_expanded(&docs), "展开目录必须恢复");
        assert_eq!(view.explorer_selected(), Some(&note), "当前行必须恢复");
        assert!(view.explorer_show_ignored(), "忽略开关必须恢复");
        assert!(
            (-view.explorer_scroll_handle().unwrap().offset().y.as_f32() - 40.0).abs() < 1.0,
            "滚动位置必须恢复"
        );
        assert!(view.explorer_filter().is_empty(), "过滤必须清除");
        assert!(
            view.explorer_multi_selection().unwrap().is_empty(),
            "多选必须清除"
        );
    });

    // 瞬态不跨重开：在新实例设置后再次重开必须回到默认。
    window.update(|_, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_set_filter("note", cx);
            view.explorer_toggle_multi_selection(&other, cx);
        });
    });
    window.simulate_keystrokes("ctrl-shift-n");
    window.run_until_parked();
    let explorer = open_explorer(&root, &mut window, cx);
    window.run_until_parked();
    explorer.read_with(cx, |view, _| {
        assert!(view.explorer_filter().is_empty());
        assert!(view.explorer_multi_selection().unwrap().is_empty());
        assert!(view.explorer_is_expanded(&docs), "瞬态变化不得丢失持久展开");
        assert_eq!(view.explorer_selected(), Some(&note));
    });

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    super::close::flush_when_ready(&runtime.client());

    let persisted =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persisted);
    draw_frame(&mut window);
    let explorer = open_explorer(&root, &mut window, cx);
    window.run_until_parked();
    explorer.read_with(cx, |view, _| {
        assert!(view.explorer_is_expanded(&docs), "重启后展开目录必须恢复");
        assert_eq!(
            view.explorer_selected(),
            Some(&note),
            "重启后当前行必须恢复"
        );
        assert!(view.explorer_show_ignored());
        assert!((-view.explorer_scroll_handle().unwrap().offset().y.as_f32() - 40.0).abs() < 1.0);
    });
}
