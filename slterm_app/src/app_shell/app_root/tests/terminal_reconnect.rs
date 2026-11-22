//! 已退出终端的重连、失败保留与代次回收。

use super::*;

#[gpui::test]
fn terminal_reconnect_retries_without_replacing_the_panel(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel_id = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let terminal = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    window.update(|_, cx| {
        terminal.update(cx, |view, cx| view.mark_test_exited(cx));
    });
    window.run_until_parked();
    let old_target = root.read_with(cx, |root, _| {
        root.terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .unwrap()
    });
    let old_commands = state
        .borrow_mut()
        .sessions
        .remove(&panel_id.sequence())
        .unwrap();

    state.borrow_mut().fail_next = true;
    let failed = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::ReconnectTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(failed.is_err());
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.terminals.borrow().target(terminal_id(TERMINAL_A)),
            Some(old_target)
        );
        assert_eq!(
            root.snapshot
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .run_intent(),
            RunIntent::Exited
        );
    });
    terminal.read_with(cx, |view, _| assert!(view.has_error()));

    let reconnected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::ReconnectTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(reconnected.is_ok());
    window.run_until_parked();
    assert!(matches!(old_commands.try_recv(), Ok(Msg::Shutdown)));
    root.read_with(cx, |root, _| {
        let target = root
            .terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .unwrap();
        assert_eq!(target.generation().get(), 2);
        assert!(matches!(
            root.terminals
                .borrow()
                .send(old_target, Msg::Input(Cow::Owned(b"stale".to_vec()))),
            Err(crate::gpui_terminal::registry::RegistryError::StaleGeneration)
        ));
        assert_eq!(
            root.snapshot
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .run_intent(),
            RunIntent::Launch
        );
    });
    terminal.read_with(cx, |view, _| {
        assert!(!view.is_exited());
        assert!(!view.has_error());
        assert!(view.has_reconnect_separator());
    });
}
