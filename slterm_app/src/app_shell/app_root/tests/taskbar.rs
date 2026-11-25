//! 任务栏进度只跟随活跃项目的聚焦终端。

use super::*;
use crate::gpui_terminal::view::TerminalViewEvent;
use crate::platform::windows_integration::FakeWindowsIntegration;
use crate::taskbar::TaskProgress;

fn emit_progress(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    panel: PanelId,
    progress: TaskProgress,
) {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(
                panel,
                &TerminalViewEvent::ProgressChanged(progress),
                window,
                cx,
            );
        });
    });
}

/// 后台终端进度不抢任务栏；聚焦终端进度经 Windows 集成端口投递。
#[gpui::test]
fn taskbar_progress_follows_the_focused_terminal(cx: &mut TestAppContext) {
    crate::notify::reset_windows_integration_for_test();
    let windows = std::sync::Arc::new(FakeWindowsIntegration::default());
    crate::notify::install_windows_integration(windows.clone());

    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);

    emit_progress(&root, &mut window, first, TaskProgress::Value(40));
    assert!(
        windows.take_taskbar_progress().is_empty(),
        "非聚焦终端的进度不得投递"
    );

    emit_progress(&root, &mut window, second, TaskProgress::Value(40));
    assert_eq!(
        windows.take_taskbar_progress(),
        vec![TaskProgress::Value(40)],
        "聚焦终端的真实进度必须投递"
    );

    // 同 pane 同形态去重。
    emit_progress(&root, &mut window, second, TaskProgress::Value(40));
    assert!(windows.take_taskbar_progress().is_empty());

    crate::notify::reset_windows_integration_for_test();
}
