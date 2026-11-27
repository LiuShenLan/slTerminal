//! 应用内通知保留面：桌面开关与前台聚焦不隐藏进程级错误。

use super::*;
use crate::notify::{AlertSeverity, Notification};

/// 桌面通知关闭且窗口聚焦时，错误仍进入应用内保留面，关闭只由用户操作。
#[gpui::test]
fn process_errors_stay_in_app_until_dismissed(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    window.update(|window, cx| {
        let focus = root.read(cx).workspace_focus.clone();
        focus.focus(window, cx);
    });
    cx.update(|cx| {
        cx.set_global(crate::app_shell::config::Settings {
            desktop_notifications: false,
            ..crate::app_shell::config::Settings::default()
        });
        crate::notify::deliver_process_alert(
            &Notification::ProcessAlert {
                text: "in-app-test-error".to_owned(),
                severity: AlertSeverity::Error,
            },
            cx,
        );
    });
    root.update(cx, |root, cx| root.poll_in_app_notices(cx));
    draw_frame(&mut window);
    let ids = root.read_with(cx, |root, _| {
        root.in_app_notices
            .iter()
            .filter(|notice| notice.text == "in-app-test-error")
            .map(|notice| notice.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(ids.len(), 1, "错误必须进入应用内保留面");
    let banner = window
        .debug_bounds("in-app-notice")
        .expect("应用内错误必须渲染");
    assert!(
        window.debug_bounds("in-app-notice-dismiss").is_some(),
        "应用内错误必须提供显式关闭"
    );

    window.simulate_click(
        banner.origin + gpui::point(gpui::px(4.0), gpui::px(4.0)),
        Modifiers::default(),
    );
    draw_frame(&mut window);
    let after_click = root.read_with(cx, |root, _| root.in_app_notices.clone());
    assert!(
        after_click
            .iter()
            .any(|notice| notice.text == "in-app-test-error"),
        "点击通知本身不得清除应用内错误: {after_click:?}"
    );

    let dismiss = window
        .debug_bounds("in-app-notice-dismiss")
        .expect("关闭入口必须仍在");
    window.simulate_click(dismiss.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.in_app_notices.is_empty()),
        "显式关闭后错误才移除"
    );
    assert!(window.debug_bounds("in-app-notice").is_none());
}
