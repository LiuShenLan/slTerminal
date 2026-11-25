//! 全量验收：重复创建与销毁的真实运行根资源回收。

use std::time::Duration;

use super::*;

/// 锚：重复创建与销毁运行根不残留写任务，工作区库始终可重开。
#[gpui::test]
fn acceptance_root_recreate_leaves_no_pending_writes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    for _ in 0..3 {
        let (events, _events_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let persistence =
            AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
        let (root, mut window, _) = open_root_with_persistence(cx, persistence);
        window.update(|window, cx| {
            root.update(cx, |root, cx| root.observe_window_geometry(window, cx));
            window.bounds_changed(cx);
        });
        // 先移除窗口再丢弃句柄才真正销毁 AppRoot。
        window.update(|window, _| window.remove_window());
        drop(root);
        cx.update(|_| {});
        window.executor().advance_clock(Duration::from_millis(600));
        window.run_until_parked();
        runtime
            .shutdown(ControlPlaneCleanup::Succeeded)
            .expect("每一轮销毁后 writer 必须可正常关闭");
    }

    let observer = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        observer
            .window_geometry(&crate::platform::machine::machine_key())
            .unwrap()
            .is_none(),
        "销毁后的去抖不得跨运行写入几何"
    );
}
