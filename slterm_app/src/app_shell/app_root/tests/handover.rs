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

fn new_terminal_handover(project_root: Option<&str>, cwd: Option<&str>) -> HandoverParams {
    HandoverParams::NewTerminal {
        project_root: project_root.map(str::to_owned),
        cwd: cwd.map(str::to_owned),
    }
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
fn handover_intents_are_rejected_while_close_is_in_progress(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| root.close_in_progress = true);
    window.deactivate_window();
    assert_window_active(&mut window, false);
    let intents = [
        HandoverParams::Focus,
        HandoverParams::OpenProject {
            project_root: "D:/work/b".to_owned(),
        },
        new_terminal_handover(None, None),
    ];

    for intent in intents {
        let error = dispatch_handover(&root, &mut window, intent).unwrap_err();
        assert_handover_error(&error, -32004, DomainErrorCode::Closing);
    }

    assert_window_active(&mut window, false);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_intents_are_rejected_after_close_completes(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| root.close_completed = true);
    window.deactivate_window();
    assert_window_active(&mut window, false);
    let intents = [
        HandoverParams::Focus,
        HandoverParams::OpenProject {
            project_root: "D:/work/b".to_owned(),
        },
        new_terminal_handover(None, None),
    ];

    for intent in intents {
        let error = dispatch_handover(&root, &mut window, intent).unwrap_err();
        assert_handover_error(&error, -32004, DomainErrorCode::Closing);
    }

    assert_window_active(&mut window, false);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_open_project_opens_root_without_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    window.deactivate_window();
    assert_window_active(&mut window, false);

    let response = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/work/handover".to_owned(),
        },
    )
    .unwrap();

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.projects.len(), 1);
        assert_eq!(root.snapshot().registry().projects().len(), 1);
        assert_eq!(root.snapshot().terminals().count(), 0);
        let active = root.snapshot().registry().active().unwrap();
        assert_eq!(root.panel_count(active, cx), 0);
        assert_eq!(
            root.snapshot()
                .registry()
                .project(active)
                .map(|project| project.root()),
            Some(&path("D:/work/handover"))
        );
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn handover_open_project_duplicate_root_reactivates_same_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    window.deactivate_window();

    let response = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/work/a".to_owned(),
        },
    )
    .unwrap();

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.snapshot().registry().projects().len(), 2);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), Some(panel));
        assert_eq!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .entity_id(),
            terminal_entity
        );
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_open_project_rejects_invalid_paths_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let project_entity = root.read_with(cx, |root, _| {
        root.projects[&project_id(PROJECT_A)].entity_id()
    });
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let attempts = state.borrow().attempts;
    window.deactivate_window();
    let focused_before = window.update(|window, cx| window.focused(cx));
    let invalid_roots = [
        "relative/sk-test/project",
        "C:relative/sk-test/project",
        "D:/private/sk-test/../project",
        r"\\?\D:\private\sk-test\project",
    ];

    for project_root in invalid_roots {
        let error = dispatch_handover(
            &root,
            &mut window,
            HandoverParams::OpenProject {
                project_root: project_root.to_owned(),
            },
        )
        .unwrap_err();

        assert_handover_error(&error, -32000, DomainErrorCode::InvalidRequest);
        assert!(!error.message.contains("sk-test"));
        assert!(!error.message.contains("D:/private"));
        assert_window_active(&mut window, false);
        assert_eq!(
            window.update(|window, cx| window.focused(cx)),
            focused_before
        );
        root.read_with(cx, |root, cx| {
            assert_eq!(root.snapshot(), &before);
            assert_eq!(root.projects.len(), 1);
            assert_eq!(
                root.projects[&project_id(PROJECT_A)].entity_id(),
                project_entity
            );
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
fn handover_open_project_rejects_modal_activation_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let project_entity = root.read_with(cx, |root, _| {
        root.projects[&project_id(PROJECT_A)].entity_id()
    });
    open_side_menu_for_test(&mut window);
    window.deactivate_window();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused_before = window.update(|window, cx| window.focused(cx));
    let attempts = state.borrow().attempts;

    let error = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/private/sk-test/project".to_owned(),
        },
    )
    .unwrap_err();

    assert_handover_error(&error, -32002, DomainErrorCode::Unauthorized);
    assert!(!error.message.contains("sk-test"));
    assert!(!error.message.contains("D:/private"));
    assert_window_active(&mut window, false);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.projects.len(), 1);
        assert_eq!(
            root.projects[&project_id(PROJECT_A)].entity_id(),
            project_entity
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), Some(panel));
        assert_eq!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .entity_id(),
            terminal_entity
        );
        assert!(root.side_button_menu.is_some(), "失败不得关闭既有模态");
    });
    assert_eq!(state.borrow().attempts, attempts);
}

#[gpui::test]
fn handover_open_project_restores_hidden_launch_and_focuses_active_terminal(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );
    let attempts = state.borrow().attempts;
    window.deactivate_window();

    let response = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/work/b".to_owned(),
        },
    )
    .unwrap();

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    assert_eq!(state.borrow().attempts, attempts + 1);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_B, TERMINAL_B);
    let (terminal_focus, project_focus) = root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 1);
        let terminal = root
            .terminal_panel(project_id(PROJECT_B), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1;
        (
            terminal.read(cx).focus_handle.clone(),
            root.projects[&project_id(PROJECT_B)]
                .read(cx)
                .focus_handle
                .clone(),
        )
    });
    window.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(terminal_focus.clone()));
        assert_ne!(window.focused(cx), Some(project_focus));
    });

    let duplicate = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/work/b".to_owned(),
        },
    )
    .unwrap();

    window.run_until_parked();
    assert!(duplicate.is_accepted());
    assert_eq!(state.borrow().attempts, attempts + 1);
    assert_eq!(
        panel_entity(&root, cx, PROJECT_B, TERMINAL_B).1,
        terminal_entity
    );
    window.update(|window, cx| assert_eq!(window.focused(cx), Some(terminal_focus)));
}

#[gpui::test]
fn handover_open_project_focus_precheck_failure_does_not_restore_hidden_panels(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    let stale_panel = window.update(|_, cx| {
        root.update(cx, |root, cx| {
            assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
            let stale_panel = root.panel_ids.allocate().unwrap();
            let stale_handle = cx.focus_handle();
            let parent = root.focus_manager.workspace_token();
            let registration = root
                .focus_manager
                .register(&stale_handle, parent, Some(project_id(PROJECT_B)), false)
                .unwrap();
            let token = registration.token();
            root.focus_manager.unregister(token).unwrap();
            root.panel_focus.insert(stale_panel, registration);
            root.projects[&project_id(PROJECT_B)].update(cx, |project, _| {
                project.focused_panel = Some(stale_panel);
            });
            stale_panel
        })
    });
    window.update(|window, cx| {
        let active_focus = root.read(cx).projects[&project_id(PROJECT_A)]
            .read(cx)
            .focus_handle
            .clone();
        active_focus.focus(window, cx);
    });
    window.deactivate_window();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused_before = window.update(|window, cx| window.focused(cx));
    let attempts = state.borrow().attempts;

    let error = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenProject {
            project_root: "D:/work/b".to_owned(),
        },
    )
    .unwrap_err();

    assert_eq!(
        state.borrow().attempts,
        attempts,
        "默认焦点预检失败不得先懒启动隐藏项目"
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(
            root.focused_panel(project_id(PROJECT_B), cx),
            Some(stale_panel)
        );
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    assert_window_active(&mut window, false);
    assert_handover_error(&error, -32001, DomainErrorCode::NotReady);
    assert_eq!(error.message, "open project is not ready");
    assert!(!error.message.contains("D:/work/b"));
}

#[gpui::test]
fn handover_new_terminal_without_paths_uses_active_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    window.deactivate_window();

    let response = dispatch_handover(&root, &mut window, new_terminal_handover(None, None))
        .expect("活跃项目必须接受无路径新终端");

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B))
        );
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 1);
        let record = root
            .snapshot()
            .terminals()
            .find(|record| record.project_id() == project_id(PROJECT_B))
            .unwrap();
        assert_eq!(record.cwd(), &path("D:/work/b"));
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_new_terminal_explicit_root_selects_hidden_project_and_focuses_terminal(
    cx: &mut TestAppContext,
) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    window.deactivate_window();

    let response = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/a"), Some("D:/work/a/nested")),
    )
    .expect("显式根必须选择隐藏项目");

    window.run_until_parked();
    assert!(response.is_accepted());
    assert_window_active(&mut window, true);
    let new_terminal = root.read_with(cx, |root, _| {
        root.snapshot()
            .terminals()
            .find(|record| {
                record.project_id() == project_id(PROJECT_A)
                    && record.cwd() == &path("D:/work/a/nested")
            })
            .map(TerminalRecord::id)
            .unwrap()
    });
    let terminal_focus = root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        root.terminal_panel(project_id(PROJECT_A), new_terminal, cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| assert_eq!(window.focused(cx), Some(terminal_focus)));
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn handover_new_terminal_unknown_root_is_unauthorized_without_project_creation(
    cx: &mut TestAppContext,
) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused_before = window.update(|window, cx| window.focused(cx));
    window.deactivate_window();

    let error = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(
            Some("D:/private/sk-test/unknown"),
            Some("D:/private/sk-test/unknown/cwd"),
        ),
    )
    .unwrap_err();

    assert_handover_error(&error, -32002, DomainErrorCode::Unauthorized);
    assert!(!error.message.contains("sk-test"));
    assert!(!error.message.contains("D:/private"));
    assert_window_active(&mut window, false);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.projects.len(), 1);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn handover_new_terminal_cwd_selects_deepest_covering_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work");
    add_project(&root, &mut window, PROJECT_B, "D:/work/a");
    activate_project(&root, &mut window, PROJECT_A);

    let response = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(None, Some("D:/work/a/nested")),
    )
    .expect("cwd 必须选择最深覆盖项目");

    window.run_until_parked();
    assert!(response.is_accepted());
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B))
        );
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn handover_new_terminal_rejects_uncovered_and_cross_root_cwd_without_fallback(
    cx: &mut TestAppContext,
) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused_before = window.update(|window, cx| window.focused(cx));
    window.deactivate_window();
    let requests = [
        new_terminal_handover(None, Some("D:/work/uncovered")),
        new_terminal_handover(Some("D:/work/a"), Some("D:/work/b/nested")),
    ];

    for request in requests {
        let error = dispatch_handover(&root, &mut window, request).unwrap_err();

        assert_handover_error(&error, -32002, DomainErrorCode::Unauthorized);
        assert_window_active(&mut window, false);
        assert_eq!(
            window.update(|window, cx| window.focused(cx)),
            focused_before
        );
        root.read_with(cx, |root, cx| {
            assert_eq!(root.snapshot(), &before);
            assert_eq!(
                root.snapshot().registry().active(),
                Some(project_id(PROJECT_B))
            );
            assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
            assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        });
        assert_eq!(state.borrow().attempts, 0);
    }
}

#[gpui::test]
fn handover_new_terminal_rejects_invalid_path_shapes_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    window.deactivate_window();
    let requests = [
        new_terminal_handover(Some("relative/sk-test"), None),
        new_terminal_handover(None, Some("D:/private/sk-test/../cwd")),
    ];

    for request in requests {
        let error = dispatch_handover(&root, &mut window, request).unwrap_err();

        assert_handover_error(&error, -32000, DomainErrorCode::InvalidRequest);
        assert_eq!(error.message, "invalid new terminal path");
        assert!(!error.message.contains("sk-test"));
        assert!(!error.message.contains("D:/private"));
        assert_window_active(&mut window, false);
        root.read_with(cx, |root, cx| {
            assert_eq!(root.snapshot(), &before);
            assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        });
        assert_eq!(state.borrow().attempts, 0);
    }
}

#[gpui::test]
fn handover_new_terminal_rejects_modal_activation_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    open_side_menu_for_test(&mut window);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let focused_before = window.update(|window, cx| window.focused(cx));
    window.deactivate_window();

    let error =
        dispatch_handover(&root, &mut window, new_terminal_handover(None, None)).unwrap_err();

    assert_handover_error(&error, -32002, DomainErrorCode::Unauthorized);
    assert_window_active(&mut window, false);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.side_button_menu.is_some());
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn handover_new_terminal_launch_failure_rolls_back_hidden_target(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    let (_, original_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    window.update(|window, cx| {
        let terminal = root
            .read(cx)
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1;
        let focus = terminal.read(cx).focus_handle.clone();
        focus.focus(window, cx);
    });
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let (focused_panel, focus_registrations, subscriptions) = root.read_with(cx, |root, cx| {
        (
            root.focused_panel(project_id(PROJECT_A), cx),
            root.panel_focus.len(),
            root.panel_subscriptions.len(),
        )
    });
    let focused_before = window.update(|window, cx| window.focused(cx));
    let attempts = state.borrow().attempts;
    state.borrow_mut().fail_next = true;
    window.deactivate_window();

    let error = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/b"), Some("D:/work/b/nested")),
    )
    .unwrap_err();

    assert_handover_error(&error, -32001, DomainErrorCode::NotReady);
    assert!(!error.message.contains("D:/work/b"));
    assert_window_active(&mut window, false);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), focused_panel);
        assert_eq!(root.panel_focus.len(), focus_registrations);
        assert_eq!(root.panel_subscriptions.len(), subscriptions);
        assert_eq!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .entity_id(),
            original_entity
        );
    });
    assert_eq!(state.borrow().attempts, attempts + 1);
}

#[gpui::test]
fn handover_new_terminal_hidden_layout_fallback_restores_and_uses_existing_group(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    let attempts = state.borrow().attempts;
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );

    let response = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/b"), Some("D:/work/b/nested")),
    )
    .expect("隐藏项目不得因面板未恢复而拒绝新终端");

    window.run_until_parked();
    assert!(response.is_accepted());
    let new_terminal = root.read_with(cx, |root, _| {
        root.snapshot()
            .terminals()
            .find(|record| record.cwd() == &path("D:/work/b/nested"))
            .map(TerminalRecord::id)
            .unwrap()
    });
    let terminal_focus = root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B))
        );
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 2);
        let project = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_B))
            .unwrap();
        let group = project.layout().group(group_id(GROUP_B)).unwrap();
        assert_eq!(
            group.views(),
            &[
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_B),
                    terminal_id: terminal_id(TERMINAL_B),
                },
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_B),
                    terminal_id: new_terminal,
                },
            ]
        );
        assert_eq!(
            group.active(),
            &ViewKey::Terminal {
                project_id: project_id(PROJECT_B),
                terminal_id: new_terminal,
            }
        );
        root.terminal_panel(project_id(PROJECT_B), new_terminal, cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| assert_eq!(window.focused(cx), Some(terminal_focus)));
    assert_window_active(&mut window, true);
    assert_eq!(state.borrow().attempts, attempts + 2);
}

#[derive(Clone, Copy)]
enum HiddenFocusFailure {
    TokenPrecheck,
    ProjectActivation,
    TerminalRequestFocus,
}

fn assert_hidden_focus_failure_rolls_back_launch(
    cx: &mut TestAppContext,
    failure: HiddenFocusFailure,
) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, original_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    activate_project(&root, &mut window, PROJECT_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let (focus_state, registrations, subscriptions) = root.read_with(cx, |root, _| {
        (
            root.focus_manager.state_for_test(),
            root.panel_focus.len(),
            root.panel_subscriptions.len(),
        )
    });
    let focused_before = window.update(|window, cx| window.focused(cx));
    let sessions_before = state
        .borrow()
        .sessions
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let attempts = state.borrow().attempts;
    window.deactivate_window();
    window.update(|_, cx| {
        root.update(cx, |root, _| match failure {
            HiddenFocusFailure::TokenPrecheck => root
                .focus_manager
                .fail_next_project_activation_precheck_for_test(),
            HiddenFocusFailure::ProjectActivation => root
                .focus_manager
                .fail_next_project_activation_after_scope_change_for_test(),
            HiddenFocusFailure::TerminalRequestFocus => {
                root.focus_manager
                    .fail_terminal_request_focus_after_activation_for_test();
            }
        });
    });

    let error = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/b"), None),
    )
    .unwrap_err();

    assert_handover_error(&error, -32001, DomainErrorCode::NotReady);
    assert_window_active(&mut window, false);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        focused_before
    );
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.focus_manager.state_for_test(), focus_state);
        assert_eq!(root.projects.len(), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(root.panel_focus.len(), registrations);
        assert_eq!(root.panel_subscriptions.len(), subscriptions);
        assert_eq!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .entity_id(),
            original_entity
        );
    });
    assert_eq!(
        state.borrow().attempts,
        attempts + 1,
        "故障必须注入 launcher 成功之后"
    );
    let sessions = state.borrow();
    let launched = sessions
        .sessions
        .iter()
        .filter(|(sequence, _)| !sessions_before.contains(sequence))
        .collect::<Vec<_>>();
    assert_eq!(launched.len(), 1, "只应产生一个新测试会话");
    for (_, receiver) in launched {
        assert!(
            matches!(receiver.try_recv(), Ok(Msg::Shutdown)),
            "失败回滚必须关闭已启动会话"
        );
    }
}

#[gpui::test]
fn handover_new_terminal_token_precheck_failure_rolls_back_launched_terminal(
    cx: &mut TestAppContext,
) {
    assert_hidden_focus_failure_rolls_back_launch(cx, HiddenFocusFailure::TokenPrecheck);
}

#[gpui::test]
fn handover_new_terminal_project_activation_failure_rolls_back_launched_terminal(
    cx: &mut TestAppContext,
) {
    assert_hidden_focus_failure_rolls_back_launch(cx, HiddenFocusFailure::ProjectActivation);
}

#[gpui::test]
fn handover_new_terminal_terminal_focus_failure_rolls_back_launched_terminal(
    cx: &mut TestAppContext,
) {
    assert_hidden_focus_failure_rolls_back_launch(cx, HiddenFocusFailure::TerminalRequestFocus);
}

#[gpui::test]
fn handover_new_terminal_syncs_original_active_focus_before_hidden_candidate(
    cx: &mut TestAppContext,
) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    activate_project(&root, &mut window, PROJECT_A);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
                .unwrap();
        });
    });
    let second_focus = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
            .unwrap()
            .1
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| second_focus.focus(window, cx));
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.snapshot = root
                .snapshot
                .with_active_terminal(terminal_id(TERMINAL_A))
                .unwrap();
            let first_panel = root
                .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .0;
            root.projects[&project_id(PROJECT_A)].update(cx, |project, _| {
                project.focused_panel = Some(first_panel);
            });
        });
    });
    root.read_with(cx, |root, _| {
        let project = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap();
        assert_eq!(
            project.layout().group(group_id(GROUP_A)).unwrap().active(),
            &ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_A),
            }
        );
    });

    let response = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/b"), None),
    )
    .unwrap();

    window.run_until_parked();
    assert!(response.is_accepted());
    root.read_with(cx, |root, cx| {
        let project = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap();
        assert_eq!(
            project.layout().group(group_id(GROUP_A)).unwrap().active(),
            &ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_C),
            },
            "隐藏候选必须克隆已同步的原活跃项目焦点"
        );
        assert_eq!(
            root.focused_panel(project_id(PROJECT_A), cx),
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
                .map(|(panel, _)| panel)
        );
    });
    activate_project(&root, &mut window, PROJECT_A);
    window.run_until_parked();
    window.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(second_focus));
    });
}

#[gpui::test]
fn handover_new_terminal_empty_workspace_unknown_root_is_unauthorized(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    window.deactivate_window();

    let error = dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/unknown"), None),
    )
    .unwrap_err();

    assert_handover_error(&error, -32002, DomainErrorCode::Unauthorized);
    assert_window_active(&mut window, false);
    root.read_with(cx, |root, _| {
        assert!(root.projects.is_empty());
        assert!(root.snapshot().registry().projects().is_empty());
        assert_eq!(root.snapshot().terminals().count(), 0);
    });
    assert_eq!(state.borrow().attempts, 0);
    assert!(state.borrow().sessions.is_empty());
}

#[gpui::test]
fn handover_new_terminal_multi_group_uses_remembered_non_first_group(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");

    dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/a"), None),
    )
    .unwrap();

    root.read_with(cx, |root, _| {
        let new_terminal = root
            .snapshot()
            .terminals()
            .find(|record| record.project_id() == project_id(PROJECT_A) && record.number() == 2)
            .map(TerminalRecord::id)
            .unwrap();
        let project = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap();
        assert_eq!(
            project.layout().group(group_id(GROUP_A)).unwrap().views(),
            &[ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_A),
            }]
        );
        assert_eq!(
            project.layout().group(group_id(GROUP_B)).unwrap().views(),
            &[
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_A),
                    terminal_id: terminal_id(TERMINAL_B),
                },
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_A),
                    terminal_id: new_terminal,
                },
            ]
        );
    });
}

#[gpui::test]
fn handover_new_terminal_multi_group_without_focus_falls_back_to_first_group(
    cx: &mut TestAppContext,
) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.projects[&project_id(PROJECT_A)].update(cx, |project, _| {
                project.focused_panel = None;
            });
        });
    });

    dispatch_handover(
        &root,
        &mut window,
        new_terminal_handover(Some("D:/work/a"), None),
    )
    .unwrap();

    root.read_with(cx, |root, _| {
        let new_terminal = root
            .snapshot()
            .terminals()
            .find(|record| record.project_id() == project_id(PROJECT_A) && record.number() == 2)
            .map(TerminalRecord::id)
            .unwrap();
        let project = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap();
        assert_eq!(
            project.layout().group(group_id(GROUP_A)).unwrap().views(),
            &[
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_A),
                    terminal_id: terminal_id(TERMINAL_A),
                },
                ViewKey::Terminal {
                    project_id: project_id(PROJECT_A),
                    terminal_id: new_terminal,
                },
            ]
        );
        assert_eq!(
            project.layout().group(group_id(GROUP_B)).unwrap().views(),
            &[ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_B),
            }]
        );
    });
}

#[gpui::test]
fn handover_open_file_remains_invalid_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let attempts = state.borrow().attempts;
    window.deactivate_window();
    assert_window_active(&mut window, false);
    let intents = [HandoverParams::OpenFile {
        path: "D:/private/sk-test/secret.txt".to_owned(),
    }];

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
