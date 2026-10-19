//! 设置页键位编辑:录制/清除/恢复默认写回与保留键红线。
//!
//! 全部用例走虚拟窗口里的真实按钮与按键路径;设置文件注入 tempdir,
//! 磁盘断言直接经 `keyboard_bindings::load_keybind_pairs_at` 回读。

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{click_selector, draw_frame, open_settings_at, open_workspace};
use crate::app_shell::workspace::SltermWorkspace;
use crate::app_shell::workspace::keyboard_bindings;

/// 打开设置页并切到交互页(键位编辑组已绘制)。
fn open_interaction_page(
    cx: &mut TestAppContext,
    settings: Option<&str>,
) -> (
    Entity<SltermWorkspace>,
    VisualTestContext,
    std::path::PathBuf,
    tempfile::TempDir,
) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    if let Some(settings) = settings {
        std::fs::write(&settings_path, settings).unwrap();
    }
    open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    window.refresh().unwrap();
    draw_frame(&mut window);
    click_selector(&mut window, "settings-nav-interaction");
    window.refresh().unwrap();
    draw_frame(&mut window);
    (workspace, window, settings_path, directory)
}

/// 录制新组合:旧用户行被新组合替换,布局持久化到 `keybind` 段。
#[gpui::test]
fn recording_replaces_the_old_combo_in_the_keybind_segment(cx: &mut TestAppContext) {
    let (workspace, mut window, settings_path, _directory) = open_interaction_page(
        cx,
        Some(r#"{"keybind":[{"combo":"ctrl+s","action":"save_file"}]}"#),
    );

    click_selector(&mut window, "settings-keybind-record-save_file");
    window.simulate_keystrokes("ctrl-alt-r");
    window.run_until_parked();

    let pairs = keyboard_bindings::load_keybind_pairs_at(&settings_path);
    assert_eq!(
        pairs,
        [("ctrl-alt-r".to_owned(), "save_file".to_owned())],
        "新组合必须落盘且旧组合被替换"
    );
    cx.update(|cx| {
        let pane = workspace
            .read(cx)
            .settings_pane_for_test()
            .expect("设置页必须仍打开");
        let pane = pane.read(cx);
        assert_eq!(pane.keybind_recording_for_test(), None);
        assert!(pane.keybind_error_for_test().is_none());
        assert_eq!(
            pane.keybind_pairs_for_test(),
            [("ctrl-alt-r".to_owned(), "save_file".to_owned())]
        );
    });
}

/// 清除:动作行移除,默认组合落 `none` 精确收回(旧键透传终端)。
#[gpui::test]
fn clearing_an_action_writes_the_unbind_marker(cx: &mut TestAppContext) {
    let (_workspace, mut window, settings_path, _directory) = open_interaction_page(
        cx,
        Some(r#"{"keybind":[{"combo":"ctrl+alt+r","action":"save_file"}]}"#),
    );

    click_selector(&mut window, "settings-keybind-clear-save_file");
    window.run_until_parked();

    let pairs = keyboard_bindings::load_keybind_pairs_at(&settings_path);
    assert_eq!(
        pairs,
        [("ctrl+s".to_owned(), "none".to_owned())],
        "清除必须撤自定义行并把默认键精确收回"
    );
}

/// 恢复默认:撤掉 `none` 标记,生效组合回到静态默认;其余动作不受影响。
#[gpui::test]
fn resetting_an_action_restores_the_static_default(cx: &mut TestAppContext) {
    let (_workspace, mut window, settings_path, _directory) = open_interaction_page(
        cx,
        Some(concat!(
            r#"{"keybind":[{"combo":"ctrl+s","action":"none"},"#,
            r#"{"combo":"ctrl+alt+x","action":"toggle_sidebar"}]}"#
        )),
    );

    click_selector(&mut window, "settings-keybind-reset-save_file");
    window.run_until_parked();

    let pairs = keyboard_bindings::load_keybind_pairs_at(&settings_path);
    assert_eq!(
        pairs,
        [("ctrl+alt+x".to_owned(), "toggle_sidebar".to_owned())],
        "恢复默认只撤本动作标记,不得误删其他动作"
    );
    assert_eq!(
        keyboard_bindings::effective_combos(&pairs, "save_file"),
        ["ctrl+s".to_owned()],
        "恢复默认后展示与生效组合回到静态默认"
    );
}

/// 保留键(Ctrl+C)录制被拒绝:可读错误、零写盘、仍停留在录制态。
#[gpui::test]
fn recording_a_reserved_combo_is_rejected_without_writing(cx: &mut TestAppContext) {
    let (workspace, mut window, settings_path, _directory) = open_interaction_page(cx, None);

    click_selector(&mut window, "settings-keybind-record-save_file");
    window.simulate_keystrokes("ctrl-c");
    window.run_until_parked();

    assert!(!settings_path.exists(), "保留键被拒绝时不得写设置文件");
    cx.update(|cx| {
        let pane = workspace
            .read(cx)
            .settings_pane_for_test()
            .expect("设置页必须仍打开");
        let pane = pane.read(cx);
        assert!(
            pane.keybind_recording_for_test().is_some(),
            "拒绝后应留在录制态供重试"
        );
        let error = pane.keybind_error_for_test().expect("保留键必须有可读错误");
        assert!(
            error.contains("ctrl-c") || error.contains("Ctrl+C"),
            "错误文案必须包含被拒组合: {error}"
        );
        assert!(pane.keybind_pairs_for_test().is_empty());
    });
}

/// Esc 取消录制:零写盘、录制态与错误同时清空。
#[gpui::test]
fn escape_cancels_recording_without_writing(cx: &mut TestAppContext) {
    let (workspace, mut window, settings_path, _directory) = open_interaction_page(cx, None);

    click_selector(&mut window, "settings-keybind-record-save_file");
    window.simulate_keystrokes("escape");
    window.run_until_parked();

    assert!(!settings_path.exists(), "取消录制不得写设置文件");
    cx.update(|cx| {
        let pane = workspace
            .read(cx)
            .settings_pane_for_test()
            .expect("设置页必须仍打开");
        let pane = pane.read(cx);
        assert_eq!(pane.keybind_recording_for_test(), None);
        assert!(pane.keybind_error_for_test().is_none());
        assert!(pane.keybind_pairs_for_test().is_empty());
    });
}
