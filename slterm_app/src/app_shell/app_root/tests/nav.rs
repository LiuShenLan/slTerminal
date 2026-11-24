//! 导航视图：项目列表、激活与统一移除守卫。

use super::*;

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
