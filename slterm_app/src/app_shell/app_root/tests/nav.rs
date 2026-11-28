//! 导航视图：项目列表、激活与统一移除守卫。

use super::*;

/// 注入目录选择端口并保留真实项目路径解析：重定位生产入口专用。
pub(super) fn open_root_with_windows(
    cx: &mut TestAppContext,
    persistence: AppRootPersistence,
    windows: std::sync::Arc<dyn crate::platform::windows_integration::WindowsIntegration>,
) -> (Entity<AppRoot>, VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::notify::reset_in_app_notices_for_test();
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::app_root::keymap::register(cx);
        crate::app_shell::text_view::init(cx);
    });
    let mut slot = None;
    let window = cx.open_window(
        cx.update(|cx| gpui::Bounds::maximized(None, cx).size),
        |window, cx| {
            let root = cx.new(|cx| {
                let mut root = AppRoot::with_launcher(
                    PanelIdAllocator::new(1),
                    path("C:/Program Files/PowerShell/7/pwsh.exe"),
                    Box::new(FakeLauncher {
                        state: Rc::new(RefCell::new(LauncherState::default())),
                    }),
                    persistence,
                    None,
                    AppRootPorts {
                        runtime_events: None,
                        windows,
                    },
                    cx,
                );
                root.replace_agent_global_dirs_for_test(Vec::new());
                root.project_paths =
                    Arc::new(crate::platform::project_paths::SystemProjectPathResolver);
                root
            });
            let close_root = root.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                close_root
                    .update(cx, |root, cx| root.request_close(window, cx))
                    .unwrap_or(true)
            });
            root.update(cx, |root, cx| {
                root.observe_focus(window, cx);
                root.restore_active_project(window, cx);
            });
            slot = Some(root.clone());
            Root::new(root, window, cx)
        },
    );
    let window = VisualTestContext::from_window(window.into(), cx);
    (slot.unwrap(), window)
}

fn open_nav(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.update(|window, cx| {
        let _ = root.update(cx, |root, cx| {
            root.activate_side_view(SideViewId::Nav, window, cx)
        });
    });
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Nav).cloned().unwrap()
    })
}

#[gpui::test]
fn navigation_project_remove_uses_workspace_guard(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_a = temp.path().join("nav-a");
    let project_b = temp.path().join("nav-b");
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, project_a.to_str().unwrap());
    add_project(&root, &mut window, PROJECT_B, project_b.to_str().unwrap());
    let view = open_nav(&root, &mut window, cx);
    assert!(
        window.debug_bounds("nav-panel").is_some(),
        "导航视图必须真实渲染"
    );
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.nav.as_ref().map(|nav| nav.projects.len()),
            Some(2),
            "导航必须列出全部项目"
        );
    });

    let (first, second) = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            let first = root.handle_nav_remove(project_id(PROJECT_A), window, cx);
            let second = root.handle_nav_remove(project_id(PROJECT_A), window, cx);
            (first, second)
        })
    });
    assert!(first.is_ok(), "首次导航移除必须进入统一关闭链: {first:?}");
    assert!(
        matches!(
            second,
            Err(crate::app_shell::app_root::commands::AppRootError::ProjectCloseRunning)
        ),
        "关闭在途时导航移除必须被统一守卫拒绝: {second:?}"
    );
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .is_some(),
            "关闭在途时项目必须保留"
        );
    });
}

/// 导航项目行菜单是根重定位的真实入口：目录选择与真实解析走端口。
#[gpui::test]
fn nav_project_menu_relocates_to_picked_real_directory(cx: &mut TestAppContext) {
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
    let _view = open_nav(&root, &mut window, cx);

    let bounds = window.debug_bounds("nav-project-0").unwrap();
    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("nav-project-menu").is_some(),
        "右键项目行必须打开重定位菜单"
    );

    window.simulate_keystrokes("down");
    window.simulate_keystrokes("enter");
    window.run_until_parked();
    cx.run_until_parked();

    let expected = CanonicalPath::new(dunce::canonicalize(&new).unwrap()).unwrap();
    assert_eq!(
        project_root(&root, cx, project_id(PROJECT_A)),
        expected,
        "菜单入口必须把项目根改绑到选定的真实目录"
    );
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .root_status()
        }),
        slterm_workspace::ProjectRootStatus::Ready
    );
}

fn project_root(root: &Entity<AppRoot>, cx: &TestAppContext, id: ProjectId) -> CanonicalPath {
    root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(id)
            .expect("项目必须存在")
            .root()
            .clone()
    })
}

/// 目录选择取消：项目根与状态不变。
#[gpui::test]
fn nav_project_menu_relocation_cancel_keeps_root(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old");
    std::fs::create_dir_all(&old).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_folder_target(None);
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    add_project(&root, &mut window, PROJECT_A, old.to_str().unwrap());
    let before = project_root(&root, cx, project_id(PROJECT_A));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_project_relocation(project_id(PROJECT_A), window, cx);
        });
    });
    for _ in 0..3 {
        window.run_until_parked();
        cx.run_until_parked();
    }

    assert_eq!(project_root(&root, cx, project_id(PROJECT_A)), before);
    root.read_with(cx, |root, _| {
        assert!(root.pending_project_relocation.is_none());
        assert!(root.project_relocation_in_flight.is_none());
    });
}

/// 导航历史扫描：会话 cwd 词法归属到最深项目根。
#[gpui::test]
fn nav_history_scan_attributes_sessions_to_projects(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let home = temp.path().join("home");
    let claude = home.join(".claude").join("projects").join("D--project");
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("history-session.jsonl"),
        serde_json::json!({
            "type": "summary",
            "summary": "历史任务",
            "cwd": project_dir.to_str().unwrap(),
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    root.update(cx, |root, _| root.set_session_home_for_test(home.clone()));
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }

    let _view = open_nav(&root, &mut window, cx);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("nav-history").is_some(),
        "历史行必须出现在导航树中"
    );
    root.read_with(cx, |root, _| {
        let row = root
            .nav_history
            .iter()
            .find(|row| row.title == "历史任务")
            .expect("扫描必须发现历史会话");
        assert_eq!(row.project, Some(project_id(PROJECT_A)));
        assert_eq!(
            row.cwd.as_ref().map(|cwd| cwd.as_path()),
            Some(project_dir.as_path())
        );
        assert!(row.resume.is_some(), "可恢复会话必须带恢复命令");
    });
}

/// 历史恢复生产入口：已归属会话在目标项目建终端并注入恢复命令。
#[gpui::test]
fn nav_history_restore_opens_terminal_with_resume_command(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let home = temp.path().join("home");
    let claude = home.join(".claude").join("projects").join("D--project");
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("restore-session.jsonl"),
        serde_json::json!({
            "type": "summary",
            "summary": "恢复目标",
            "cwd": project_dir.to_str().unwrap(),
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    root.update(cx, |root, _| root.set_session_home_for_test(home.clone()));
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    let _view = open_nav(&root, &mut window, cx);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.title == "恢复目标")
            .expect("扫描必须发现历史会话")
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_nav_restore_history(index, window, cx).unwrap();
        });
    });
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        let terminals = root
            .snapshot()
            .terminals()
            .filter(|terminal| terminal.project_id() == project_id(PROJECT_A))
            .collect::<Vec<_>>();
        assert_eq!(terminals.len(), 1, "恢复必须创建终端");
        assert_eq!(
            terminals[0].cwd().as_path(),
            project_dir.as_path(),
            "历史 cwd 可用时必须沿用"
        );
        let terminal_id = terminals[0].id();
        let (_, view) = root
            .terminal_panel(project_id(PROJECT_A), terminal_id, cx)
            .expect("恢复终端面板必须存在");
        view.read_with(cx, |view, _| {
            assert!(
                view.has_pending_agent_command_for_test(),
                "恢复命令必须在提示符或超时后注入"
            );
        });
    });
}

fn scan_unowned_session(
    cx: &mut TestAppContext,
) -> (Entity<AppRoot>, VisualTestContext, tempfile::TempDir) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let project_dir = temp.path().join("project");
    let unowned = temp.path().join("unowned");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::create_dir_all(&unowned).unwrap();
    let home = temp.path().join("home");
    let claude = home.join(".claude").join("projects").join("D--unowned");
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("unowned-session.jsonl"),
        serde_json::json!({
            "type": "summary",
            "summary": "未归属任务",
            "cwd": unowned.to_str().unwrap(),
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    root.update(cx, |root, _| root.set_session_home_for_test(home.clone()));
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    let _ = open_nav(&root, &mut window, cx);
    (root, window, temp)
}

/// 真实导航删除入口：来源哈希、隔离副本、持久批次与导航隐藏一次闭合。
#[gpui::test]
fn nav_history_delete_quarantines_and_hides_session(cx: &mut TestAppContext) {
    let (root, mut window, paths, _home, source, _runtime) = quarantine_fixture(cx);
    let _view = open_nav(&root, &mut window, cx);
    draw_frame(&mut window);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.source_path == source)
            .expect("扫描必须发现待删除会话")
    });

    delete_via_history_menu(&root, &mut window, cx, index);

    assert!(
        root.read_with(cx, |root, _| !root.quarantine.records.is_empty()),
        "删除入口必须形成持久隔离批次"
    );
    assert!(!source.exists(), "删除必须移走来源");
    let records = WorkspaceDatabase::open(&paths)
        .unwrap()
        .quarantine_records()
        .unwrap();
    assert_eq!(records.len(), 1, "删除必须写入持久批次记录");
    assert_eq!(
        records[0].status,
        crate::history_quarantine::BatchStatus::Completed
    );
    assert!(
        root.read_with(cx, |root, _| root
            .nav_history
            .iter()
            .all(|row| row.source_path != source)),
        "已隔离会话必须从导航隐藏"
    );
    assert!(
        window.debug_bounds("nav-quarantine-open").is_some(),
        "存在批次时必须显示已删除会话入口"
    );
    assert!(
        std::fs::read_dir(paths.local_root().join("history-quarantine"))
            .unwrap()
            .next()
            .is_some(),
        "隔离根必须保留清单与副本"
    );
    let _ = paths;
}

/// 隔离区撤销：真实副本恢复来源，导航不再隐藏，持久状态推进为已撤销。
#[gpui::test]
fn nav_quarantine_undo_restores_deleted_session(cx: &mut TestAppContext) {
    let (root, mut window, paths, _home, source, _runtime) = quarantine_fixture(cx);
    let _view = open_nav(&root, &mut window, cx);
    draw_frame(&mut window);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.source_path == source)
            .expect("扫描必须发现待删除会话")
    });
    invoke_delete(&root, &mut window, cx, index);
    open_quarantine(&root, &mut window, cx);
    assert!(
        selector_exists(&mut window, "nav-quarantine-undo-0"),
        "可撤销批次必须渲染撤销入口"
    );
    click_selector(&mut window, "nav-quarantine-undo-0");
    settle(&root, &mut window, cx);

    assert!(source.exists(), "撤销必须恢复来源");
    assert!(
        root.read_with(cx, |root, _| root
            .nav_history
            .iter()
            .any(|row| row.source_path == source)),
        "撤销后会话必须重新出现在导航"
    );
    let records = WorkspaceDatabase::open(&paths)
        .unwrap()
        .quarantine_records()
        .unwrap();
    assert_eq!(
        records[0].status,
        crate::history_quarantine::BatchStatus::Undone
    );
}

/// 永久删除必须经显式确认：点击删除只进入确认面，确认后才移除材料与记录。
#[gpui::test]
fn nav_quarantine_permanent_delete_requires_confirmation(cx: &mut TestAppContext) {
    let (root, mut window, paths, _home, source, _runtime) = quarantine_fixture(cx);
    let _view = open_nav(&root, &mut window, cx);
    draw_frame(&mut window);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.source_path == source)
            .expect("扫描必须发现待删除会话")
    });
    invoke_delete(&root, &mut window, cx, index);
    // 模拟撤销窗口已过：到期后永久删除入口才允许出现。
    root.update(cx, |root, cx| {
        root.quarantine.records[0].completed_at =
            Some(std::time::SystemTime::now() - std::time::Duration::from_secs(10));
        root.sync_nav_quarantine(cx);
    });
    open_quarantine(&root, &mut window, cx);
    assert!(
        selector_exists(&mut window, "nav-quarantine-delete-0"),
        "到期批次必须渲染永久删除入口"
    );
    click_selector(&mut window, "nav-quarantine-delete-0");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("nav-quarantine-confirm").is_some(),
        "永久删除必须先显示确认面"
    );
    assert_eq!(quarantine_file_count(&paths), 2, "确认前不得移除清单或副本");

    click_selector(&mut window, "nav-quarantine-delete-confirm");
    settle(&root, &mut window, cx);

    assert_eq!(quarantine_file_count(&paths), 0, "确认后必须移除隔离材料");
    assert!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .quarantine_records()
            .unwrap()
            .is_empty(),
        "确认后必须移除持久批次记录"
    );
}

/// 导出入口：后台选择目标并原子写入 ZIP；导出不清除隔离材料。
#[gpui::test]
fn nav_quarantine_export_writes_zip_without_clearing_material(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let target = temp.path().join("quarantine-export.zip");
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(target.clone()));
    let (root, mut window) = open_root_with_windows(cx, persistence, windows);
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let home = temp.path().join("home");
    let claude = home.join(".claude").join("projects").join("D--project");
    std::fs::create_dir_all(&claude).unwrap();
    let source = claude.join("export-session.jsonl");
    std::fs::write(
        &source,
        serde_json::json!({
            "type": "summary",
            "summary": "导出目标",
            "cwd": project_dir.to_str().unwrap(),
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    root.update(cx, |root, _| root.set_session_home_for_test(home.clone()));
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    settle(&root, &mut window, cx);
    let _view = open_nav(&root, &mut window, cx);
    draw_frame(&mut window);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.source_path == source)
            .expect("扫描必须发现导出会话")
    });
    invoke_delete(&root, &mut window, cx, index);
    open_quarantine(&root, &mut window, cx);
    assert!(
        selector_exists(&mut window, "nav-quarantine-export-0"),
        "隔离批次必须渲染导出入口"
    );
    click_selector(&mut window, "nav-quarantine-export-0");
    settle(&root, &mut window, cx);

    let bytes = std::fs::read(&target).expect("导出目标必须存在");
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(
        quarantine_file_count(&paths),
        2,
        "导出本身不得清除清单或副本"
    );
}

fn quarantine_fixture(
    cx: &mut TestAppContext,
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    AppPaths,
    tempfile::TempDir,
    std::path::PathBuf,
    WorkspaceWriterRuntime,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _launcher) = open_root_with_persistence(cx, persistence);
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let home = temp.path().join("home");
    let claude = home.join(".claude").join("projects").join("D--project");
    std::fs::create_dir_all(&claude).unwrap();
    let source = claude.join("delete-session.jsonl");
    std::fs::write(
        &source,
        serde_json::json!({
            "type": "summary",
            "summary": "删除目标",
            "cwd": project_dir.to_str().unwrap(),
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    root.update(cx, |root, _| root.set_session_home_for_test(home.clone()));
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    settle(&root, &mut window, cx);
    root.read_with(cx, |root, _| {
        assert!(root.quarantine_root.is_some(), "生产恢复必须提供隔离根")
    });
    (root, window, paths, temp, source, runtime)
}

fn quarantine_file_count(paths: &AppPaths) -> usize {
    let root = paths.local_root().join("history-quarantine");
    match std::fs::read_dir(root) {
        Ok(entries) => entries.flatten().count(),
        Err(_) => 0,
    }
}

fn selector_exists(window: &mut VisualTestContext, selector: &'static str) -> bool {
    draw_frame(window);
    window.debug_bounds(selector).is_some()
}

/// 导航历史单行菜单：右键行后经真实菜单入口删除。
fn delete_via_history_menu(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    index: usize,
) {
    draw_frame(window);
    let selector: &'static str = Box::leak(format!("nav-history-row-{index}").into_boxed_str());
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少历史行 {selector}"));
    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw_frame(window);
    assert!(
        window.debug_bounds("nav-history-menu").is_some(),
        "历史行右键必须打开单行菜单"
    );
    window.simulate_keystrokes("down");
    window.simulate_keystrokes("enter");
    settle(root, window, cx);
}

/// 导航删除的生产入口：先证明删除控件已渲染，再调用同一 AppRoot 命令。
fn invoke_delete(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    index: usize,
) {
    let selector: &'static str = Box::leak(format!("nav-history-row-{index}").into_boxed_str());
    assert!(selector_exists(window, selector), "导航行必须渲染");
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_nav_delete_history(index, window, cx).unwrap();
        });
    });
    settle(root, window, cx);
}

fn open_quarantine(root: &Entity<AppRoot>, window: &mut VisualTestContext, cx: &TestAppContext) {
    assert!(
        selector_exists(window, "nav-quarantine-open"),
        "存在批次时必须渲染已删除会话入口"
    );
    click_selector(window, "nav-quarantine-open");
    settle(root, window, cx);
}

/// 隔离对话框按真实左键命中控件，覆盖按钮路由而不是直达处理器。
fn click_selector(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少界面元素 {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
}

fn settle(root: &Entity<AppRoot>, window: &mut VisualTestContext, cx: &TestAppContext) {
    for _ in 0..10 {
        window.run_until_parked();
        cx.run_until_parked();
        draw_frame(window);
    }
    let _ = root;
}

/// 未归属会话必须先显示目标根确认，不得静默落到活跃项目。
#[gpui::test]
fn unowned_history_requires_explicit_confirmation(cx: &mut TestAppContext) {
    let (root, mut window, _temp) = scan_unowned_session(cx);
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.title == "未归属任务")
            .expect("扫描必须发现未归属会话")
    });
    let projects_before = root.read_with(cx, |root, _| root.snapshot().registry().projects().len());
    let terminals_before = root.read_with(cx, |root, _| root.snapshot().terminals().count());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_nav_restore_history(index, window, cx).unwrap();
        });
    });
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.nav_restore_confirm.is_some()),
        "未归属会话必须打开确认面"
    );
    assert!(
        window.debug_bounds("nav-restore-confirm").is_some(),
        "确认面必须真实绘制"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().registry().projects().len()),
        projects_before,
        "确认前不得创建项目"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().terminals().count()),
        terminals_before,
        "确认前不得创建终端"
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.cancel_nav_restore_confirm(window, cx).unwrap();
        });
    });
    assert!(root.read_with(cx, |root, _| root.nav_restore_confirm.is_none()));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().registry().projects().len()),
        projects_before,
        "取消不得创建项目"
    );
}

/// 确认后才创建项目与终端，并注入恢复命令。
#[gpui::test]
fn unowned_history_confirm_creates_project_and_terminal(cx: &mut TestAppContext) {
    let (root, mut window, temp) = scan_unowned_session(cx);
    let unowned = temp.path().join("unowned");
    let index = root.read_with(cx, |root, _| {
        root.nav_history
            .iter()
            .position(|row| row.title == "未归属任务")
            .expect("扫描必须发现未归属会话")
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_nav_restore_history(index, window, cx).unwrap();
        });
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.accept_nav_restore_confirm(window, cx).unwrap();
        });
    });
    for _ in 0..8 {
        window.run_until_parked();
        cx.run_until_parked();
    }

    root.read_with(cx, |root, cx| {
        let project = root
            .snapshot()
            .registry()
            .projects()
            .iter()
            .find(|project| project.root().as_path() == unowned.as_path())
            .expect("确认后必须创建目标根项目");
        let terminal = root
            .snapshot()
            .terminals()
            .find(|terminal| terminal.project_id() == project.id())
            .expect("确认后必须创建终端");
        assert_eq!(terminal.cwd().as_path(), unowned.as_path());
        let (_, view) = root
            .terminal_panel(project.id(), terminal.id(), cx)
            .expect("恢复终端面板必须存在");
        view.read_with(cx, |view, _| {
            assert!(
                view.has_pending_agent_command_for_test(),
                "未归属恢复同样必须注入恢复命令"
            );
        });
    });
}
