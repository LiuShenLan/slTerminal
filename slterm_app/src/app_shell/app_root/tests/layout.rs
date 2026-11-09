//! 布局命令的虚拟窗口接线测试；所有终端均使用内存会话。

use slterm_workspace::LayoutError;

use super::*;

fn execute_layout(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: CommandId,
    parameters: CommandParameters,
) -> Result<CommandOutcome, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                id,
                CommandSource::Application,
                CommandTarget::Focused,
                Some(parameters),
                window,
                cx,
            )
        })
    })
}

fn panel_view(root: &Entity<AppRoot>, cx: &TestAppContext, panel_id: PanelId) -> ViewKey {
    root.read_with(cx, |root, cx| {
        let project_id = root.snapshot().registry().active().unwrap();
        root.projects[&project_id].read(cx).panels[&panel_id]
            .view_key
            .clone()
    })
}

fn group_containing(root: &Entity<AppRoot>, cx: &TestAppContext, view: &ViewKey) -> PanelGroupId {
    root.read_with(cx, |root, _| {
        let project_id = root.snapshot().registry().active().unwrap();
        let layout = root
            .snapshot()
            .registry()
            .project(project_id)
            .unwrap()
            .layout();
        layout
            .group_ids()
            .into_iter()
            .find(|group_id| layout.group(*group_id).unwrap().views().contains(view))
            .unwrap()
    })
}

fn assert_actual_panel_focus(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    project_id: ProjectId,
    panel_id: PanelId,
) {
    let expected = root.read_with(cx, |root, cx| {
        root.projects[&project_id].read(cx).panels[&panel_id].focus_handle(cx)
    });
    let actual = window.update(|window, cx| window.focused(cx));
    assert_eq!(actual, Some(expected), "真实 GPUI 焦点必须落在目标面板");
}

#[gpui::test]
fn move_view_reuses_panel_and_focuses_moved_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let third = split_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let third_view = panel_view(&root, cx, third);
    let source = group_containing(&root, cx, &second_view);
    let target = group_containing(&root, cx, &third_view);

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MoveView,
        CommandParameters::MoveView {
            view: second_view.clone(),
            target_group: target,
            target_index: 0,
        },
    );

    assert!(
        matches!(result, Ok(CommandOutcome::Completed)),
        "{result:?}"
    );
    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group(source).unwrap().views().len(), 1);
        assert_eq!(layout.group(target).unwrap().views()[0], second_view);
        assert_eq!(layout.group(target).unwrap().active(), &second_view);
        assert_eq!(root.focused_panel(project, cx), Some(second));
        assert_eq!(root.snapshot().terminals().count(), 3);
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, second);
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn merge_groups_preserves_instances_and_focuses_actual_active(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let third = split_generated_terminal(&root, &mut window);
    let first_view = panel_view(&root, cx, first);
    let source = group_containing(&root, cx, &first_view);
    let third_view = panel_view(&root, cx, third);
    let target = group_containing(&root, cx, &third_view);

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MergeGroups,
        CommandParameters::MergeGroups {
            source_group: source,
            target_group: target,
        },
    );

    assert!(
        matches!(result, Ok(CommandOutcome::Completed)),
        "{result:?}"
    );
    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids(), vec![target]);
        assert_eq!(layout.group(target).unwrap().active(), &third_view);
        assert_eq!(layout.group(target).unwrap().views().len(), 3);
        assert!(root.split_dividers.is_empty(), "合并后不得残留分隔线焦点");
        assert_eq!(root.focused_panel(project, cx), Some(third));
        assert!(
            root.projects[&project]
                .read(cx)
                .panels
                .contains_key(&second)
        );
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, third);
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn split_existing_view_moves_panel_without_starting_another_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let source = group_containing(&root, cx, &second_view);

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::SplitView,
        CommandParameters::SplitView {
            view: second_view.clone(),
            target_group: source,
            direction: SplitDirection::TopBottom,
            ratio: 0.5,
            viewport: LogicalSize::new(1200.0, 800.0),
        },
    );

    assert!(
        matches!(result, Ok(CommandOutcome::Completed)),
        "{result:?}"
    );
    let target = group_containing(&root, cx, &second_view);
    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids().len(), 2);
        assert_eq!(root.split_dividers.len(), 1);
        assert_eq!(layout.group(source).unwrap().views().len(), 1);
        assert_ne!(target, source);
        assert_eq!(layout.group(target).unwrap().active(), &second_view);
        assert_eq!(root.focused_panel(project, cx), Some(second));
        assert_eq!(root.snapshot().terminals().count(), 2);
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, second);
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn resize_split_changes_only_geometry(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = split_generated_terminal(&root, &mut window);

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::ResizeSplit,
        CommandParameters::ResizeSplit {
            path: Vec::new(),
            ratio: 0.75,
            viewport: LogicalSize::new(1200.0, 800.0),
        },
    );

    assert!(
        matches!(result, Ok(CommandOutcome::Completed)),
        "{result:?}"
    );
    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        match layout.tree().unwrap() {
            SplitTree::Split { ratio, .. } => assert_eq!(*ratio, 0.75),
            SplitTree::Leaf(_) => panic!("拆分后必须保留根分割节点"),
        }
        assert_eq!(root.focused_panel(project, cx), Some(second));
        assert_eq!(root.snapshot().terminals().count(), 2);
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, second);
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn layout_menu_moves_active_view_to_next_group(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let _third = split_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let ViewKey::Terminal { terminal_id, .. } = &second_view else {
        panic!("测试面板必须是终端")
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(*terminal_id, window, cx).unwrap();
        });
    });
    draw_frame(&mut window);

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-layout-menu").is_some());
    window.simulate_keystrokes("down");
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-layout-menu").is_none());
    let target = group_containing(&root, cx, &second_view);
    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group(target).unwrap().active(), &second_view);
        assert_eq!(layout.group(target).unwrap().views().len(), 2);
        assert_eq!(root.focused_panel(project, cx), Some(second));
    });
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn layout_menu_splits_and_merges_existing_view(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let source = group_containing(&root, cx, &second_view);
    draw_frame(&mut window);

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-layout-menu").is_some());
    window.simulate_keystrokes("down");
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    let split_group = group_containing(&root, cx, &second_view);
    assert_ne!(split_group, source);
    assert_eq!(state.borrow().attempts, 2);
    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);
    for _ in 0..2 {
        window.simulate_keystrokes("down");
    }
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids(), vec![source]);
        assert_eq!(layout.group(source).unwrap().views().len(), 2);
        assert_eq!(root.snapshot().terminals().count(), 2);
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, first);
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn split_divider_keyboard_resize_preserves_focus(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let _second = split_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);
    for _ in 0..3 {
        window.simulate_keystrokes("down");
    }
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    let divider = root.read_with(cx, |root, _| {
        root.split_dividers[&(project, Vec::new())].handle.clone()
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(divider.clone())
    );

    window.simulate_keystrokes("up");
    draw_frame(&mut window);

    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        match layout.tree().unwrap() {
            SplitTree::Split { ratio, .. } => assert_eq!(*ratio, 0.48),
            SplitTree::Leaf(_) => panic!("拆分后必须保留根分割节点"),
        }
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(divider)
    );
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn layout_request_focus_failure_does_not_commit_candidate(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let third = split_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let third_view = panel_view(&root, cx, third);
    let target = group_containing(&root, cx, &third_view);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| {
        let token = root.panel_focus[&second].token();
        root.focus_manager.set_visible(token, false).unwrap();
    });

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MoveView,
        CommandParameters::MoveView {
            view: second_view,
            target_group: target,
            target_index: 0,
        },
    );

    assert!(matches!(
        result,
        Err(AppRootError::Focus(FocusError::Unavailable))
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.focused_panel(project, cx), Some(third));
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, third);
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn single_view_group_does_not_open_empty_layout_menu(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    open_generated_project(&root, &mut window, "D:/work/layout");
    new_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-layout-menu").is_none());
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn layout_menu_shortcut_is_scoped_to_project_content(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let panel = new_generated_terminal(&root, &mut window);
    new_generated_terminal(&root, &mut window);
    let view = panel_view(&root, cx, panel);
    draw_frame(&mut window);
    let nav_bounds = window.debug_bounds("app-root-side-view-nav").unwrap();
    window.simulate_mouse_down(nav_bounds.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(nav_bounds.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let nav_focus = root.read_with(cx, |root, cx| {
        root.side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .read(cx)
            .focus_handle
            .clone()
            .unwrap()
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(nav_focus)
    );

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-layout-menu").is_none());

    let ViewKey::Terminal { terminal_id, .. } = view else {
        panic!("测试面板必须是终端")
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id, window, cx).unwrap();
        });
    });
    draw_frame(&mut window);
    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-layout-menu").is_some());
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().registry().active()),
        Some(project)
    );
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn closing_terminal_with_focused_collapsing_divider_restores_remaining_panel(
    cx: &mut TestAppContext,
) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let first = new_generated_terminal(&root, &mut window);
    let _second = split_generated_terminal(&root, &mut window);
    draw_frame(&mut window);
    let result = execute_layout(
        &root,
        &mut window,
        CommandId::FocusSplitDivider,
        CommandParameters::FocusSplitDivider { path: Vec::new() },
    );
    assert!(
        matches!(result, Ok(CommandOutcome::Completed)),
        "{result:?}"
    );
    let divider = root.read_with(cx, |root, _| {
        root.split_dividers[&(project, Vec::new())].handle.clone()
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(divider)
    );

    let close = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(close.is_ok(), "关闭终端失败: {close:?}");
    window.run_until_parked();
    draw_frame(&mut window);

    root.read_with(cx, |root, cx| {
        assert_eq!(root.projects[&project].read(cx).panels.len(), 1);
        assert_eq!(root.focused_panel(project, cx), Some(first));
        assert!(root.split_dividers.is_empty());
    });
    assert_actual_panel_focus(&root, &mut window, cx, project, first);
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn layout_focus_failure_does_not_commit_candidate(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let _first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let third = split_generated_terminal(&root, &mut window);
    let second_view = panel_view(&root, cx, second);
    let third_view = panel_view(&root, cx, third);
    let target = group_containing(&root, cx, &third_view);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    root.update(cx, |root, _| {
        root.panel_focus.remove(&second);
    });

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MoveView,
        CommandParameters::MoveView {
            view: second_view,
            target_group: target,
            target_index: 0,
        },
    );

    assert!(matches!(result, Err(AppRootError::MissingPanel)));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.focused_panel(project, cx), Some(third));
    });
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn modal_layout_command_is_rejected_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let first = new_generated_terminal(&root, &mut window);
    let view = panel_view(&root, cx, first);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    open_side_menu_for_test(&mut window);

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MoveView,
        CommandParameters::MoveView {
            view,
            target_group: group_id(GROUP_C),
            target_index: 0,
        },
    );

    assert!(matches!(
        result,
        Err(AppRootError::Focus(FocusError::ModalActive))
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.focused_panel(project, cx), Some(first));
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn invalid_layout_target_preserves_snapshot_instances_and_focus(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/layout");
    let first = new_generated_terminal(&root, &mut window);
    let view = panel_view(&root, cx, first);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    let result = execute_layout(
        &root,
        &mut window,
        CommandId::MoveView,
        CommandParameters::MoveView {
            view,
            target_group: group_id(GROUP_C),
            target_index: 0,
        },
    );

    assert!(matches!(
        result,
        Err(AppRootError::Layout(LayoutError::GroupNotFound))
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.focused_panel(project, cx), Some(first));
        assert_eq!(root.projects[&project].read(cx).panels.len(), 1);
    });
    assert_eq!(state.borrow().attempts, 1);
}
