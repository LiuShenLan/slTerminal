//! C8 导出工作区 UI:对话框 → `SettingsPaneEvent::ExportWorkspace` → 宿主写盘。
//!
//! 用例走虚拟窗口的真实按钮与路径选择器;导出文件经 `session::load_from`
//! 回读,并与 `snapshot_for_test` 字段级对账。

use gpui::{TestAppContext, VisualTestContext};
use gpui_component::Root;

use super::{click_selector, draw_frame, open_settings_at, open_workspace};
use crate::app_shell::workspace::session_recovery::capture_window_state;
use crate::session::load_from;

/// 当前窗口通知数量(成功/失败提示的可观测面)。
fn notification_count(window: &mut VisualTestContext) -> usize {
    window.update(|window, cx| {
        window
            .root::<Root>()
            .flatten()
            .map(|root| {
                let list = root.read(cx).notification.clone();
                list.read(cx).notifications().len()
            })
            .unwrap_or(0)
    })
}

/// 点击 Advanced 页导出按钮(设置页已打开并完成首帧)。
fn click_export(window: &mut VisualTestContext) {
    window.refresh().unwrap();
    draw_frame(window);
    click_selector(window, "settings-nav-advanced");
    window.refresh().unwrap();
    draw_frame(window);
    click_selector(window, "settings-export-workspace");
}

/// 导出成功:文件存在、可回读且与整窗快照字段级相等,并给出通知。
#[gpui::test]
fn export_button_writes_the_session_snapshot(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    open_settings_at(cx, &workspace, &mut window, settings_path);
    click_export(&mut window);
    assert!(window.did_prompt_for_new_path(), "导出必须打开另存对话框");

    let output = directory.path().join("slterm-workspace.json");
    window.simulate_new_path_selection(|_| Some(output.clone()));
    window.run_until_parked();

    assert!(output.exists(), "导出必须生成目标文件");
    let exported = load_from(&output).expect("导出文件必须可回读");
    assert!(!exported.tabs.is_empty(), "导出必须包含当前标签");
    let snapshot = window.update(|window, cx| {
        workspace
            .read(cx)
            .snapshot_for_test(Some(capture_window_state(window)), cx)
    });
    assert_eq!(exported, snapshot, "导出与快照字段级相等");
    assert!(
        notification_count(&mut window) >= 1,
        "导出成功必须给出通知(含路径)"
    );
}

/// 取消另存对话框:零写入、零通知、不 panic。
#[gpui::test]
fn cancelling_the_export_dialog_writes_nothing(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    open_settings_at(cx, &workspace, &mut window, settings_path);
    click_export(&mut window);
    assert!(window.did_prompt_for_new_path());

    window.simulate_new_path_selection(|_| None);
    window.run_until_parked();

    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        0,
        "取消导出不得产生任何文件"
    );
    assert_eq!(notification_count(&mut window), 0, "取消不得发错误通知");
}

/// 不可写目标:给出可读通知且不 panic,不产出半成品文件。
#[gpui::test]
fn export_failure_reports_a_notification_without_panicking(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    open_settings_at(cx, &workspace, &mut window, settings_path);
    click_export(&mut window);

    // 父路径是普通文件:create_dir_all/write 必失败。
    let blocker = directory.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let output = blocker.join("slterm-workspace.json");
    window.simulate_new_path_selection(|_| Some(output.clone()));
    window.run_until_parked();

    assert!(!output.exists(), "失败不得产出导出文件");
    assert!(
        notification_count(&mut window) >= 1,
        "导出失败必须给出可读通知"
    );
    // 设置页仍在,窗口未因失败退出。
    assert!(
        workspace
            .read_with(cx, |workspace, _| workspace.settings_pane_for_test())
            .is_some()
    );
}
