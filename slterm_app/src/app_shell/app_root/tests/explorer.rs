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

#[gpui::test]
fn explorer_repo_root_does_not_expand_tree_root(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    let project = repository.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("inner.txt"), "inner\n").unwrap();
    std::fs::write(repository.join("first.txt"), "changed\n").unwrap();
    let project_root = CanonicalPath::new(dunce::canonicalize(&project).unwrap()).unwrap();
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let explorer = open_explorer(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    explorer.read_with(cx, |view, _| {
        assert_eq!(
            view.explorer_repo_root(),
            Some(&repo_root),
            "必须真实读到父仓库根"
        );
        assert_eq!(
            view.explorer_tree_root(),
            Some(&project_root),
            "Git 读取不得扩大文件树根"
        );
        let rows = view.explorer_visible_paths();
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|path| path.as_path().starts_with(project_root.as_path())),
            "所有可见行必须位于项目根内"
        );
        assert!(
            rows.iter()
                .all(|path| path.as_path() != repo_root.as_path().join("first.txt")),
            "仓库根兄弟文件不得进入文件树"
        );
        assert!(view.explorer_status_entries() > 0, "仓库状态必须真实读取");
    });
}

/// 可见 Explorer 根必须登记目录监听，并把目录事件合并回投为树刷新。
#[gpui::test]
fn explorer_directory_events_refresh_the_visible_tree(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    open_explorer(&root, &mut window, cx);
    draw_frame(&mut window);
    let project_root = CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap();
    std::fs::write(project_root.as_path().join("new.txt"), "new").unwrap();

    root.read_with(cx, |root, _| {
        assert!(
            root.watcher_service
                .visible_roots()
                .iter()
                .any(|root_path| root_path == &project_root),
            "可见 Explorer 根必须登记目录监听: {:?}",
            root.watcher_service.visible_roots()
        );
        let watch = root.fs_watch.as_ref().expect("文件监听必须存在");
        watch.inject_dir_event_for_test(project_root.as_path().to_path_buf());
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| root.poll_fs_watch(window, cx));
    });
    std::thread::sleep(std::time::Duration::from_millis(210));
    window.update(|window, cx| {
        root.update(cx, |root, cx| root.poll_fs_watch(window, cx));
    });
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("explorer-row-new.txt").is_some(),
        "目录事件必须刷新可见 Explorer 树"
    );
}

/// 侧栏隐藏时暂停树根监听，重新可见恢复登记并只复核一次。
#[gpui::test]
fn explorer_watcher_follows_sidebar_visibility(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let canonical = CanonicalPath::new(dunce::canonicalize(&project_root).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    open_explorer(&root, &mut window, cx);
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.watcher_service.visible_roots(),
            vec![canonical.clone()]
        );
    });

    window.simulate_keystrokes("ctrl-shift-b");
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(
            root.watcher_service.visible_roots().is_empty(),
            "侧栏隐藏必须暂停目录监听"
        );
    });

    window.simulate_keystrokes("ctrl-shift-b");
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.watcher_service.visible_roots(),
            vec![canonical],
            "侧栏重新可见必须恢复同一根监听"
        );
    });
}

/// 单击只单选；目录双击只切换展开；多选时 Enter 不打开文件。
#[gpui::test]
fn explorer_click_and_enter_follow_selection_contract(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs = project_root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("note.txt"), "note\n").unwrap();
    std::fs::write(project_root.join("other.txt"), "other\n").unwrap();
    let canonical =
        |path: std::path::PathBuf| CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
    let docs = canonical(docs);
    let (root, mut window, state) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);
    draw_frame(&mut window);

    // 单击目录只改为单选，不展开。
    let bounds = window
        .debug_bounds("explorer-row-docs")
        .expect("目录行必须渲染");
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(&mut window);
    explorer.read_with(cx, |view, _| {
        assert_eq!(view.explorer_selected(), Some(&docs), "单击必须选中目录");
        assert!(!view.explorer_is_expanded(&docs), "单击不得展开目录");
    });

    // 目录双击只切换展开，不得启动终端。
    window.simulate_event(gpui::MouseDownEvent {
        position: bounds.center(),
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    window.run_until_parked();
    draw_frame(&mut window);
    explorer.read_with(cx, |view, _| {
        assert!(view.explorer_is_expanded(&docs), "双击目录必须展开");
    });
    assert_eq!(state.borrow().attempts, 0, "双击目录不得启动终端");
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().terminals().count()),
        0,
        "双击目录不得留下终端"
    );

    // 多选后 Enter 不打开文件。
    let note_row = window
        .debug_bounds("explorer-row-docs/note.txt")
        .expect("展开后文件行必须渲染");
    window.simulate_click(note_row.center(), Modifiers::default());
    let other_row = window
        .debug_bounds("explorer-row-other.txt")
        .expect("根文件行必须渲染");
    window.simulate_click(
        other_row.center(),
        Modifiers {
            control: true,
            ..Default::default()
        },
    );
    draw_frame(&mut window);
    explorer.read_with(cx, |view, _| {
        assert_eq!(
            view.explorer_multi_selection()
                .map(|selected| selected.len()),
            Some(1),
            "Ctrl 单击必须形成多选"
        );
    });
    window.dispatch_action(super::super::explorer::ExplorerOpenSelected);
    window.run_until_parked();
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        0,
        "多选时 Enter 不得打开文件"
    );

    // 行菜单是显式打开入口：右键后点击“打开”必须走统一文件路由。
    window.simulate_mouse_down(note_row.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(note_row.center(), MouseButton::Right, Modifiers::default());
    draw_frame(&mut window);
    let open = window
        .debug_bounds("explorer-menu-open")
        .expect("右键必须打开行菜单");
    window.simulate_click(open.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        1,
        "行菜单“打开”必须打开文件"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().terminals().count()),
        0,
        "打开文件不得创建终端"
    );
}

/// 双击目录不再隐式启动终端；终端入口只在行菜单中。
#[gpui::test]
fn explorer_directory_double_click_does_not_open_terminal(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs = project_root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    let (root, mut window, state) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);
    draw_frame(&mut window);
    let bounds = window
        .debug_bounds("explorer-row-docs")
        .expect("目录行必须渲染");
    window.simulate_event(gpui::MouseDownEvent {
        position: bounds.center(),
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    window.run_until_parked();
    draw_frame(&mut window);
    explorer.read_with(cx, |view, _| {
        assert!(view.explorer_is_expanded(
            &CanonicalPath::new(dunce::canonicalize(&docs).unwrap()).unwrap()
        ));
    });
    assert_eq!(state.borrow().attempts, 0);
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().terminals().count()),
        0
    );
}

struct ExplorerTestRecycleBin {
    dir: std::path::PathBuf,
}

impl crate::app_shell::file_operation_service::RecycleBin for ExplorerTestRecycleBin {
    fn send_to_recycle_bin(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let name = path.file_name().unwrap();
        std::fs::rename(path, self.dir.join(name))
    }
}

fn execute_file_operations_directly(
    root: &Entity<AppRoot>,
    cx: &mut TestAppContext,
    recycle: std::path::PathBuf,
) {
    root.update(cx, |root, _| {
        root.replace_file_operations_for_test(
            crate::app_shell::file_operation_service::FileOperationService::with_recycle_bin_for_test(
                std::sync::Arc::new(ExplorerTestRecycleBin { dir: recycle }),
            ),
        );
    });
}

fn wait_explorer(window: &mut VisualTestContext) {
    for _ in 0..20 {
        window.run_until_parked();
        draw_frame(window);
    }
}

fn open_row_menu(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("树行必须渲染: {selector}"));
    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw_frame(window);
}

fn click_visible(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("控件必须渲染: {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
}

/// 新建、重命名、删除全部经行菜单与统一文件操作执行体。
#[gpui::test]
fn explorer_menu_creates_renames_and_deletes_through_shared_executor(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs = project_root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    let recycle = temp.path().join("recycle");
    let (root, mut window, _) = open_root(cx);
    execute_file_operations_directly(&root, cx, recycle.clone());
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let explorer = open_explorer(&root, &mut window, cx);
    wait_explorer(&mut window);

    open_row_menu(&mut window, "explorer-row-docs");
    click_visible(&mut window, "explorer-menu-new-file");
    let input = explorer
        .read_with(cx, |view, _| view.explorer_menu_input_for_test())
        .expect("新建文件必须出现输入框");
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("new.txt", window, cx));
    });
    click_visible(&mut window, "explorer-menu-input-save");
    wait_explorer(&mut window);
    assert!(docs.join("new.txt").exists(), "新建文件必须落盘");
    let docs_canonical = CanonicalPath::new(dunce::canonicalize(&docs).unwrap()).unwrap();
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| {
            view.explorer_activate_at(&docs_canonical, window, cx);
        });
    });
    wait_explorer(&mut window);

    open_row_menu(&mut window, "explorer-row-docs/new.txt");
    click_visible(&mut window, "explorer-menu-rename");
    let input = explorer
        .read_with(cx, |view, _| view.explorer_menu_input_for_test())
        .expect("重命名必须出现输入框");
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("renamed.txt", window, cx));
    });
    click_visible(&mut window, "explorer-menu-input-save");
    wait_explorer(&mut window);
    assert!(!docs.join("new.txt").exists());
    assert!(docs.join("renamed.txt").exists(), "重命名必须落盘");

    open_row_menu(&mut window, "explorer-row-docs/renamed.txt");
    click_visible(&mut window, "explorer-menu-delete");
    assert!(
        window
            .debug_bounds("explorer-menu-delete-confirm")
            .is_some(),
        "删除必须先确认"
    );
    click_visible(&mut window, "explorer-menu-delete-confirm");
    wait_explorer(&mut window);
    assert!(!docs.join("renamed.txt").exists(), "删除必须移除工作区文件");
    assert!(
        recycle.join("renamed.txt").exists(),
        "删除必须优先进入回收站"
    );
}

/// 复制与粘贴复用应用会话文件剪贴板与 PasteService。
#[gpui::test]
fn explorer_menu_copy_paste_uses_the_shared_service(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs = project_root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(project_root.join("source.txt"), "copy me\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let _explorer = open_explorer(&root, &mut window, cx);
    wait_explorer(&mut window);

    open_row_menu(&mut window, "explorer-row-source.txt");
    click_visible(&mut window, "explorer-menu-copy");
    assert!(
        root.read_with(cx, |root, _| root.file_clipboard.is_some()),
        "复制必须写入应用会话文件剪贴板"
    );
    open_row_menu(&mut window, "explorer-row-docs");
    click_visible(&mut window, "explorer-menu-paste");
    wait_explorer(&mut window);
    assert_eq!(
        std::fs::read_to_string(docs.join("source.txt")).unwrap(),
        "copy me\n",
        "粘贴必须复制真实文件"
    );
}

/// Git 状态字母与忽略开关必须消费真实状态投影。
#[gpui::test]
fn explorer_git_status_and_ignored_filter_are_rendered(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join(".gitignore"), "ignored.txt\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", ".gitignore"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "ignore"]);
    std::fs::write(repository.join("ignored.txt"), "ignored\n").unwrap();
    std::fs::write(repository.join("first.txt"), "changed\n").unwrap();
    let modified =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    let ignored =
        CanonicalPath::new(dunce::canonicalize(repository.join("ignored.txt")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let explorer = open_explorer(&root, &mut window, cx);
    wait_explorer(&mut window);
    explorer.read_with(cx, |view, _| {
        assert_eq!(
            view.explorer_status_char_for_test(&modified),
            Some('M'),
            "修改行必须显示 Git 状态字母"
        );
        assert_eq!(view.explorer_status_char_for_test(&ignored), Some('I'));
    });
    assert!(
        window.debug_bounds("explorer-row-ignored.txt").is_none(),
        "忽略开关关闭时必须隐藏忽略条目"
    );
    assert!(
        window.debug_bounds("explorer-status-M").is_some(),
        "Git 状态字母必须真实渲染"
    );
    click_visible(&mut window, "explorer-show-ignored");
    wait_explorer(&mut window);
    assert!(
        window.debug_bounds("explorer-row-ignored.txt").is_some(),
        "打开忽略开关后必须显示忽略条目"
    );
}

/// 文件树键盘入口经统一目录动作派发：F2 从前端键位打开内联重命名。
#[gpui::test]
fn explorer_f2_action_starts_inline_rename(cx: &mut TestAppContext) {
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
    let row = window
        .debug_bounds("explorer-row-old.txt")
        .expect("文件行必须渲染");
    window.simulate_click(row.center(), Modifiers::default());
    draw_frame(&mut window);
    window.simulate_keystrokes("f2");
    draw_frame(&mut window);
    assert!(
        explorer
            .read_with(cx, |view, _| view.explorer_inline_edit_input())
            .is_some(),
        "F2 必须经目录动作打开内联重命名"
    );
}
