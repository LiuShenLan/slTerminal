//! 页签溢出与完整列表的虚拟窗口测试；所有终端均使用内存会话。

use gpui::{Bounds, Pixels, point, px};

use super::*;

fn panel_terminal(root: &Entity<AppRoot>, cx: &TestAppContext, panel_id: PanelId) -> TerminalId {
    root.read_with(cx, |root, cx| {
        let project_id = root.snapshot().registry().active().unwrap();
        match root.projects[&project_id].read(cx).panels[&panel_id].view_key {
            ViewKey::Terminal { terminal_id, .. } => terminal_id,
            _ => panic!("本切片测试只创建终端视图"),
        }
    })
}

fn only_group(root: &Entity<AppRoot>, cx: &TestAppContext) -> PanelGroupId {
    root.read_with(cx, |root, _| {
        let project_id = root.snapshot().registry().active().unwrap();
        let groups = root
            .snapshot()
            .registry()
            .project(project_id)
            .unwrap()
            .layout()
            .group_ids();
        assert_eq!(groups.len(), 1);
        groups[0]
    })
}

fn tab_selector(id: TerminalId) -> &'static str {
    Box::leak(format!("app-root-tab-{id}").into_boxed_str())
}

fn tab_bar_selector(id: PanelGroupId) -> &'static str {
    Box::leak(format!("app-root-tab-bar-{id}").into_boxed_str())
}

fn click(window: &mut VisualTestContext, bounds: Bounds<Pixels>) {
    window.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::default());
}

fn assert_active_and_focused(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    project_id: ProjectId,
    group_id: PanelGroupId,
    panel_id: PanelId,
) {
    let (view, expected_focus) = root.read_with(cx, |root, cx| {
        let panel = &root.projects[&project_id].read(cx).panels[&panel_id];
        (panel.view_key.clone(), panel.focus_handle(cx))
    });
    root.read_with(cx, |root, _| {
        let group = root
            .snapshot()
            .registry()
            .project(project_id)
            .unwrap()
            .layout()
            .group(group_id)
            .unwrap();
        assert_eq!(
            group.active(),
            &view,
            "完整列表目标面板 {}",
            panel_id.sequence()
        );
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root.focused_panel(project_id, cx)),
        Some(panel_id)
    );
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(expected_focus)
    );
}

fn open_complete_list(window: &mut VisualTestContext, group_id: PanelGroupId) {
    let selector = Box::leak(format!("app-root-tab-list-{group_id}").into_boxed_str());
    let button = window.debug_bounds(selector).unwrap();
    assert!(button.size.width > px(0.0) && button.size.height > px(0.0));
    click(window, button);
    draw_frame(window);
}

#[gpui::test]
fn clicking_a_tab_reuses_its_panel_and_restores_real_focus(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/tabs");
    let panels: Vec<_> = (0..3)
        .map(|_| new_generated_terminal(&root, &mut window))
        .collect();
    let group = only_group(&root, cx);
    let terminals: Vec<_> = panels
        .iter()
        .map(|panel| panel_terminal(&root, cx, *panel))
        .collect();
    draw_frame(&mut window);

    for terminal in &terminals {
        assert!(window.debug_bounds(tab_selector(*terminal)).is_some());
    }
    let tab = window.debug_bounds(tab_selector(terminals[0])).unwrap();
    click(&mut window, tab);
    draw_frame(&mut window);

    assert_active_and_focused(&root, &mut window, cx, project, group, panels[0]);
    assert!(
        window
            .debug_bounds(terminal_selector(&terminals[0].to_string()))
            .is_some()
    );
    assert!(
        window
            .debug_bounds(terminal_selector(&terminals[1].to_string()))
            .is_none()
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project, cx)),
        3
    );
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn complete_list_activates_every_view_without_recreating_panels(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/tabs");
    let panels: Vec<_> = (0..4)
        .map(|_| new_generated_terminal(&root, &mut window))
        .collect();
    let group = only_group(&root, cx);

    for (index, panel) in panels.iter().enumerate() {
        draw_frame(&mut window);
        let focus_before = window.update(|window, cx| window.focused(cx));
        open_complete_list(&mut window, group);
        let focus_in_list = window.update(|window, cx| window.focused(cx));
        assert_ne!(focus_in_list, focus_before, "完整列表必须取得焦点");
        for _ in 0..=index {
            window.simulate_keystrokes("down");
        }
        window.simulate_keystrokes("enter");
        draw_frame(&mut window);
        assert_active_and_focused(&root, &mut window, cx, project, group, *panel);
    }

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project, cx)),
        4
    );
    assert_eq!(state.borrow().attempts, 4);
}

#[gpui::test]
fn dismissing_complete_list_preserves_active_and_restores_real_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/tabs");
    let panels: Vec<_> = (0..3)
        .map(|_| new_generated_terminal(&root, &mut window))
        .collect();
    let group = only_group(&root, cx);
    draw_frame(&mut window);
    open_complete_list(&mut window, group);
    assert!(window.debug_bounds("app-root-tab-list-menu").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-tab-list-menu").is_none());
    assert_active_and_focused(
        &root,
        &mut window,
        cx,
        project,
        group,
        *panels.last().unwrap(),
    );
}

#[gpui::test]
fn complete_list_batch_actions_close_other_and_all_views(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/tabs");
    let panels: Vec<_> = (0..3)
        .map(|_| new_generated_terminal(&root, &mut window))
        .collect();
    let group = only_group(&root, cx);
    draw_frame(&mut window);

    open_complete_list(&mut window, group);
    for _ in 0..4 {
        window.simulate_keystrokes("down");
    }
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project, cx)),
        1
    );
    assert_active_and_focused(&root, &mut window, cx, project, group, panels[2]);

    open_complete_list(&mut window, group);
    for _ in 0..2 {
        window.simulate_keystrokes("down");
    }
    window.simulate_keystrokes("enter");
    draw_frame(&mut window);

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project, cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout()
            .group_ids()
            .is_empty()
    }));
    assert!(window.debug_bounds("app-root-tab-list-menu").is_none());
}

#[gpui::test]
fn narrow_tab_bar_scrolls_horizontally_without_losing_views(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        AppRootPersistence::empty(None),
        Some(gpui::size(px(320.0), px(320.0))),
    );
    open_generated_project(&root, &mut window, "D:/work/tabs");
    let panels: Vec<_> = (0..5)
        .map(|_| new_generated_terminal(&root, &mut window))
        .collect();
    let group = only_group(&root, cx);
    let first = panel_terminal(&root, cx, panels[0]);
    let last = panel_terminal(&root, cx, *panels.last().unwrap());
    window
        .update(|window, cx| root.update(cx, |root, cx| root.activate_terminal(first, window, cx)))
        .unwrap();
    draw_frame(&mut window);
    draw_frame(&mut window);

    let bar = window.debug_bounds(tab_bar_selector(group)).unwrap();
    let before = window.debug_bounds(tab_selector(last)).unwrap();
    assert!(
        before.origin.x + before.size.width > bar.origin.x + bar.size.width,
        "窄窗口必须产生横向溢出"
    );
    window.simulate_event(gpui::ScrollWheelEvent {
        position: bar.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(-120.0), px(0.0))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    draw_frame(&mut window);

    let after = window.debug_bounds(tab_selector(last)).unwrap();
    assert!(
        after.origin.x + after.size.width < before.origin.x + before.size.width,
        "横向滚动必须移动完整页签序列"
    );
    for panel in &panels {
        let terminal = panel_terminal(&root, cx, *panel);
        assert!(window.debug_bounds(tab_selector(terminal)).is_some());
    }

    window
        .update(|window, cx| root.update(cx, |root, cx| root.activate_terminal(last, window, cx)))
        .unwrap();
    draw_frame(&mut window);
    draw_frame(&mut window);
    let revealed = window.debug_bounds(tab_selector(last)).unwrap();
    assert!(revealed.origin.x >= bar.origin.x);
    assert!(revealed.origin.x + revealed.size.width <= bar.origin.x + bar.size.width);
    assert_active_and_focused(
        &root,
        &mut window,
        cx,
        root.read_with(cx, |root, _| root.snapshot().registry().active().unwrap()),
        group,
        *panels.last().unwrap(),
    );
}
