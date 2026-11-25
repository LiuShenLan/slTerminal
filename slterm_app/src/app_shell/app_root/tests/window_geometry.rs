//! 窗口几何观察、去抖与按机器持久化。

use std::time::{Duration, Instant};

use super::*;

/// 窗口边界观察注册后经去抖按机器写入工作区库。
#[gpui::test]
fn window_geometry_persists_active_machine_bounds(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _events_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    window.update(|window, cx| {
        root.update(cx, |root, cx| root.observe_window_geometry(window, cx));
        window.bounds_changed(cx);
    });
    assert!(
        root.read_with(cx, |root, _| root.window_geometry.is_some()),
        "窗口边界变化必须记录当前几何"
    );
    // 去抖到期前不得入队。
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        observer
            .window_geometry(&crate::platform::machine::machine_key())
            .unwrap()
            .is_none(),
        "去抖到期前不得落盘"
    );

    window.executor().advance_clock(Duration::from_millis(600));
    window.run_until_parked();
    let report = runtime
        .client()
        .flush()
        .expect("flush 必须可提交")
        .recv_timeout(Duration::from_secs(5))
        .expect("flush 必须有回执");
    assert_eq!(report.failed, 0, "窗口几何写盘不得失败");

    let geometry = observer
        .window_geometry(&crate::platform::machine::machine_key())
        .unwrap()
        .expect("去抖到期后必须落盘");
    assert!(geometry.width > 0.0 && geometry.height > 0.0);

    drop(root);
    cx.update(|_| {});
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

/// 去抖任务在 AppRoot 销毁后不残留写入。
#[gpui::test]
fn window_geometry_task_stops_with_the_root(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _events_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    window.update(|window, cx| {
        root.update(cx, |root, cx| root.observe_window_geometry(window, cx));
        window.bounds_changed(cx);
    });

    // 窗口持有根实体；先移除窗口再丢弃句柄才真正销毁 AppRoot。
    window.update(|window, _| window.remove_window());
    drop(root);
    cx.update(|_| {});
    window.executor().advance_clock(Duration::from_millis(600));
    window.run_until_parked();

    let deadline = Instant::now() + Duration::from_millis(700);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    while Instant::now() < deadline {
        assert!(
            observer
                .window_geometry(&crate::platform::machine::machine_key())
                .unwrap()
                .is_none(),
            "AppRoot 销毁后不得再写入窗口几何"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
