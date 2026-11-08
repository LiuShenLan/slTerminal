//! 终端会话所有权移交与视图 Drop 兜底边界。

use std::sync::Arc;
use std::sync::mpsc::TryRecvError;

use gpui::TestAppContext;

use super::*;

/// 取走的是原会话而非重建副本；视图释放在所有权移出后不得补发第二次
/// Shutdown。取出的会话仍能独立完成显式关闭。
#[gpui::test]
fn take_session_for_shutdown_transfers_ownership_without_drop_notification(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_terminal::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let mut receiver_slot = None;
    let mut original_term_slot = None;
    let (terminal, window) = cx.add_window_view(|window, cx| {
        let mut view = TerminalView::new(
            PaneId(1),
            (80, 24),
            TerminalLaunch::Local {
                cwd: None,
                shell: Some(slterm_terminal::tty::Shell::new(
                    "slterm-test-missing-shell-executable".into(),
                    Vec::new(),
                )),
                shell_name: None,
                explicit_command: false,
            },
            window,
            cx,
        );
        let (session, receiver) = session::test_session();
        original_term_slot = Some(Arc::clone(&session.term));
        view.install_test_session(session);
        receiver_slot = Some(receiver);
        view
    });
    let original_term = original_term_slot.expect("测试应保存原会话身份");
    let receiver = receiver_slot.expect("测试应保留 Shutdown 接收端");
    let weak = terminal.downgrade();

    let taken = terminal.update(window, |view, _| {
        let taken = view
            .take_session_for_shutdown()
            .expect("首次取走应返回原会话");
        assert!(view.session.is_none(), "取走后视图不得保留会话");
        assert!(
            view.take_session_for_shutdown().is_none(),
            "重复取走必须返回 None"
        );
        taken
    });
    assert!(
        Arc::ptr_eq(&original_term, &taken.term),
        "返回值必须就是视图原持有的会话"
    );

    drop(terminal);
    window.update(|window, _| window.remove_window());
    window.run_until_parked();
    assert!(weak.upgrade().is_none(), "测试必须先确认视图已 Drop");
    assert!(
        matches!(receiver.try_recv(), Err(TryRecvError::Empty)),
        "会话已移出，视图 Drop 不得再发 Shutdown"
    );

    taken.shutdown().expect("移出的无 PTY 会话应关闭成功");
    assert!(matches!(receiver.try_recv(), Ok(Msg::Shutdown)));
}
