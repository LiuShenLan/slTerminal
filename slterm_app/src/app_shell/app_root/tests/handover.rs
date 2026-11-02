//! 单实例移交事件与同步回执测试。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError};

use super::*;

fn dispatch_handover(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    params: HandoverParams,
) -> Result<HandoverResponse, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::Handover { params, completion },
                window,
                cx,
            );
        });
    });
    response
        .try_recv()
        .expect("AppRoot 必须同步返回 handover 回执")
}

fn assert_handover_error(error: &JsonRpcError, numeric: i32, domain: DomainErrorCode) {
    assert_eq!(error.code, numeric);
    assert_eq!(error.data.as_ref().map(|data| data.code), Some(domain));
}

fn assert_window_active(window: &mut VisualTestContext, expected: bool) {
    window.update(|window, _| assert_eq!(window.is_window_active(), expected));
}

#[gpui::test]
fn handover_focus_returns_accepted(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    window.deactivate_window();
    assert_window_active(&mut window, false);

    let response = dispatch_handover(&root, &mut window, HandoverParams::Focus).unwrap();

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn handover_focus_is_rejected_while_close_is_in_progress(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| root.close_in_progress = true);
    window.deactivate_window();
    assert_window_active(&mut window, false);

    let error = dispatch_handover(&root, &mut window, HandoverParams::Focus).unwrap_err();

    assert_handover_error(&error, -32004, DomainErrorCode::Closing);
    assert_window_active(&mut window, false);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_focus_is_rejected_after_close_completes(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| root.close_completed = true);
    window.deactivate_window();
    assert_window_active(&mut window, false);

    let error = dispatch_handover(&root, &mut window, HandoverParams::Focus).unwrap_err();

    assert_handover_error(&error, -32004, DomainErrorCode::Closing);
    assert_window_active(&mut window, false);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn unimplemented_handover_intents_are_rejected_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let attempts = state.borrow().attempts;
    window.deactivate_window();
    assert_window_active(&mut window, false);
    let intents = [
        HandoverParams::OpenProject {
            project_root: "D:/private/sk-test/project".to_owned(),
        },
        HandoverParams::OpenFile {
            path: "D:/private/sk-test/secret.txt".to_owned(),
        },
        HandoverParams::NewTerminal {
            project_root: Some("D:/private/sk-test/project".to_owned()),
            cwd: Some("D:/private/sk-test/cwd".to_owned()),
        },
    ];

    for intent in intents {
        let error = dispatch_handover(&root, &mut window, intent).unwrap_err();

        assert_handover_error(&error, -32000, DomainErrorCode::InvalidRequest);
        assert!(!error.message.contains("D:/private"));
        assert!(!error.message.contains("sk-test"));
        assert_window_active(&mut window, false);
        root.read_with(cx, |root, cx| {
            assert_eq!(root.snapshot(), &before);
            assert_eq!(root.projects.len(), 1);
            assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
            assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), Some(panel));
            assert_eq!(
                root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                    .unwrap()
                    .1
                    .entity_id(),
                terminal_entity
            );
        });
        assert_eq!(state.borrow().attempts, attempts);
    }
}

#[gpui::test]
fn shell_event_pump_replies_closing_to_handovers_after_close(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (sender, receiver) = mpsc::sync_channel(64);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx)
        })
    });
    sender.try_send(SltermShellEvent::TrayQuit).unwrap();
    let first = crate::app_shell::try_send_handover(&sender, HandoverParams::Focus).unwrap();
    let second = crate::app_shell::try_send_handover(
        &sender,
        HandoverParams::OpenProject {
            project_root: "D:/work/after-close".to_owned(),
        },
    )
    .unwrap();

    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();

    root.read_with(cx, |root, _| assert!(root.close_completed));
    for response in [first, second] {
        let reply = response
            .try_recv()
            .expect("关闭后的同批 handover 必须收到结构化回执而非断连");
        let error = reply.unwrap_err();
        assert_handover_error(&error, -32004, DomainErrorCode::Closing);
    }
}

#[gpui::test]
fn handover_completion_receiver_may_be_dropped(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let (completion, response) = mpsc::sync_channel(1);
    drop(response);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::Handover {
                    params: HandoverParams::Focus,
                    completion,
                },
                window,
                cx,
            );
        });
    });

    assert_eq!(state.borrow().attempts, 0);
}
