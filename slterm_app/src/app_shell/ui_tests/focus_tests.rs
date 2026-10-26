//! 窗口管理 facade 的壳层回落用例。
//!
//! 真实 Win32 还原/前置面(IsIconic/SW_RESTORE/SetForegroundWindow)按
//! 测试豁免走构建产物 smoke;这里断言通知回窗经 facade 落到正确目标:
//! 平台激活态变化 + tab/pane 选择。

use gpui::TestAppContext;

use super::{NewTerminalTab, open_workspace};
use crate::app_shell::SltermShellEvent;

/// toast 动作按钮也走统一回窗入口:点击确认按钮先回窗到来源 pane,
/// 确认语义本身归安全确认半程(本模块只断言回窗落点)。
#[gpui::test]
fn action_button_choice_returns_to_the_named_pane(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let first = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_id().expect("首个 pane")
    });

    // 第二个 tab 处于活跃态;动作按钮指向第一个 tab 的 pane。
    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.active_tab_index()),
        1
    );

    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.handle_shell_event(
                SltermShellEvent::NotificationChoice { pane_id: first },
                window,
                cx,
            );
        });
    });
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.active_tab_index()),
        0,
        "动作按钮必须回窗到来源 pane 所在 tab"
    );
}
