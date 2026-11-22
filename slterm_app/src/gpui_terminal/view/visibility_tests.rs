//! 隐藏终端的输出合并与恢复呈现。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::str::FromStr as _;
use std::sync::Arc;

use gpui::TestAppContext;
use slterm_workspace::TerminalId;

use super::*;

#[gpui::test]
fn terminal_hidden_output_backpressure(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_terminal::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let terminals = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let terminal_id = TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e08").unwrap();
    let notifications = Rc::new(Cell::new(0));
    let mut event_tx_slot = None;
    let (terminal, window) = cx.add_window_view(|window, cx| {
        let mut view = TerminalView::restore_error(
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
            "测试禁用真实 PTY".to_owned(),
            window,
            cx,
        );
        let (session, _commands, events, event_tx) = session::test_session_with_events();
        let target = terminals
            .borrow_mut()
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))
            .unwrap();
        view.frame = Some(Arc::new(
            terminals.borrow().capture_frame(target, 0).unwrap(),
        ));
        view.binding = Some(TerminalBinding::new(&terminals, target));
        view._session_task = Some(crate::gpui_terminal::session_pump::attach(
            target, events, cx,
        ));
        view.set_output_visible(false, cx);
        event_tx_slot = Some(event_tx);
        view
    });
    let observed = Rc::clone(&notifications);
    let _subscription =
        window.update(|_, cx| cx.observe(&terminal, move |_, _| observed.set(observed.get() + 1)));
    window.run_until_parked();
    notifications.set(0);

    let event_tx = event_tx_slot.unwrap();
    let target = terminals.borrow().target(terminal_id).unwrap();
    for (revision, text) in [(1, "alpha"), (2, "beta"), (3, "gamma")] {
        let binding = TerminalBinding::new(&terminals, target);
        binding
            .feed_test_bytes(format!("{text}\r\n").as_bytes())
            .unwrap();
        let frame = terminals.borrow().capture_frame(target, revision).unwrap();
        event_tx.send(slterm_terminal::event::Event::Frame(Arc::new(frame)));
    }
    window.run_until_parked();
    window.update(|_, cx| {
        terminal.update(cx, |view, _| {
            let frame = view.frame.as_ref().unwrap();
            assert_eq!(frame.revision(), 3, "隐藏期间只保留最新帧");
            let text: String = frame
                .render()
                .segments
                .iter()
                .flat_map(|segment| &segment.cells)
                .map(|cell| cell.text.as_str())
                .collect();
            for expected in ["alpha", "beta", "gamma"] {
                assert!(text.contains(expected), "隐藏输出不得丢失 {expected}");
            }
        });
    });
    assert_eq!(notifications.get(), 0, "隐藏输出不得触发重绘");

    window.update(|_, cx| {
        terminal.update(cx, |view, cx| view.set_output_visible(true, cx));
    });
    assert_eq!(notifications.get(), 1, "恢复可见必须主动补最新帧");

    drop(_subscription);
    let worker = terminals.borrow_mut().remove(target).unwrap();
    worker.shutdown().unwrap();
    drop(terminal);
    window.update(|window, _| window.remove_window());
    window.run_until_parked();
}
