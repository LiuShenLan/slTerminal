//! View 解绑与 Registry 显式关闭边界。

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr as _;
use std::sync::mpsc::TryRecvError;

use gpui::TestAppContext;
use slterm_workspace::TerminalId;

use super::*;

/// View Drop 只解除订阅，不停止 Registry 持有的 worker；显式退休后才可关闭。
#[gpui::test]
fn view_drop_detaches_without_stopping_the_registry_worker(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_terminal::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let terminals = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let terminal_id = TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e05").unwrap();
    let mut receiver_slot = None;
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
        let (session, receiver) = session::test_session();
        view.install_test_worker(&terminals, terminal_id, session);
        receiver_slot = Some(receiver);
        view
    });
    let target = terminals.borrow().target(terminal_id).unwrap();
    let receiver = receiver_slot.unwrap();
    let weak = terminal.downgrade();

    drop(terminal);
    window.update(|window, _| window.remove_window());
    window.run_until_parked();
    assert!(weak.upgrade().is_none(), "测试必须先确认视图已 Drop");
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

    terminals
        .borrow()
        .send(target, Msg::Input(Cow::Owned(b"still-owned".to_vec())))
        .expect("View Drop 不得停止 Registry worker");
    assert!(matches!(receiver.try_recv(), Ok(Msg::Input(_))));

    let worker = terminals.borrow_mut().remove(target).unwrap();
    worker.shutdown().expect("退休 worker 应显式关闭");
    assert!(matches!(receiver.try_recv(), Ok(Msg::Shutdown)));
}

/// 进程退出只退休输入；订阅必须继续交付退出后的滚动与选区帧。
#[gpui::test]
fn process_exit_keeps_delivering_display_frames(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_terminal::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let terminals = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let terminal_id = TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e07").unwrap();
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
        event_tx.send(slterm_terminal::event::Event::Exit);
        event_tx_slot = Some(event_tx);
        view
    });

    window.run_until_parked();
    window.update(|_, cx| {
        terminal.update(cx, |view, _| {
            assert!(view.is_exited());
            assert_eq!(view.frame.as_ref().unwrap().revision(), 0);
        });
    });

    let target = terminals.borrow().target(terminal_id).unwrap();
    let frame = terminals.borrow().capture_frame(target, 1).unwrap();
    event_tx_slot
        .take()
        .unwrap()
        .send(slterm_terminal::event::Event::Frame(Arc::new(frame)));
    window.run_until_parked();
    window.update(|_, cx| {
        terminal.update(cx, |view, _| {
            assert_eq!(view.frame.as_ref().unwrap().revision(), 1);
            assert!(matches!(
                view.binding
                    .as_ref()
                    .unwrap()
                    .send(Msg::Input(Cow::Owned(b"late".to_vec()))),
                Err(crate::gpui_terminal::registry::RegistryError::ProcessExited)
            ));
        });
    });

    let worker = terminals.borrow_mut().remove(target).unwrap();
    worker.shutdown().unwrap();
    drop(terminal);
    window.update(|window, _| window.remove_window());
    window.run_until_parked();
}
