//! 窗口管理 facade 的壳层回落用例。
//!
//! 真实 Win32 还原/前置面(IsIconic/SW_RESTORE/SetForegroundWindow)按
//! 测试豁免走构建产物 smoke;这里断言四路回窗经 facade 落到正确目标:
//! 平台激活态变化 + tab/pane 选择。

use gpui::TestAppContext;

use super::{NewTerminalTab, open_workspace};
use crate::app_shell::SltermShellEvent;

/// legacy ATTACH(单实例移交)在单窗下 = 聚焦唯一主窗:`MuxAttach` 壳事件
/// 必须经 `windowing::focus`,把平台激活态从「开窗未激活」翻到主窗。
#[gpui::test]
fn legacy_attach_focuses_the_single_window(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let handle = window.update(|window, _| window.window_handle());
    assert_eq!(
        cx.update(|cx| cx.active_window().map(|handle| handle.window_id())),
        None,
        "前置条件:测试开窗不自动激活"
    );

    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.handle_shell_event(SltermShellEvent::MuxAttach, window, cx);
        });
    });

    assert_eq!(
        cx.update(|cx| cx.active_window().map(|handle| handle.window_id())),
        Some(handle.window_id()),
        "legacy ATTACH 必须经 facade 聚焦唯一主窗"
    );
}

/// toast 动作按钮也是四路回窗之一:点击确认按钮先回窗到来源 pane,
/// 确认语义本身归安全确认半程(本步只断言回窗落点)。
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
                SltermShellEvent::NotificationChoice {
                    pane_id: first,
                    request_id: 7,
                    choice: 0,
                },
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
