//! 活跃项目面板循环与关闭后的确定性还焦测试。

use super::super::{FocusNextPanel, FocusPreviousPanel};
use super::*;

fn panel_view(root: &Entity<AppRoot>, cx: &TestAppContext, panel_id: PanelId) -> ViewKey {
    root.read_with(cx, |root, cx| {
        let project_id = root.snapshot().registry().active().unwrap();
        root.projects[&project_id].read(cx).panels[&panel_id]
            .view_key
            .clone()
    })
}

fn terminal_for_panel(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    panel_id: PanelId,
) -> TerminalId {
    match panel_view(root, cx, panel_id) {
        ViewKey::Terminal { terminal_id, .. } => terminal_id,
        _ => panic!("本切片只创建终端面板"),
    }
}

fn assert_panel_focused(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    project_id: ProjectId,
    panel_id: PanelId,
) {
    let expected = root.read_with(cx, |root, cx| {
        root.projects[&project_id].read(cx).panels[&panel_id]
            .terminal
            .read(cx)
            .focus_handle
            .clone()
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root.focused_panel(project_id, cx)),
        Some(panel_id)
    );
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(expected)
    );
    let view = panel_view(root, cx, panel_id);
    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project_id)
            .unwrap()
            .layout();
        assert!(
            layout
                .group_ids()
                .into_iter()
                .any(|group_id| { layout.group(group_id).unwrap().active() == &view }),
            "聚焦面板必须同时是其组内 active"
        );
    });
}

#[gpui::test]
fn ctrl_tab_cycles_all_project_panels_without_terminal_input(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/navigation");
    let first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let neighbor = split_generated_terminal(&root, &mut window);
    let second_terminal = terminal_for_panel(&root, cx, second);
    window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.activate_terminal(second_terminal, window, cx)
            })
        })
        .unwrap();
    draw_frame(&mut window);

    for (keystroke, expected) in [
        ("ctrl-tab", neighbor),
        ("ctrl-tab", first),
        ("ctrl-shift-tab", neighbor),
        ("ctrl-shift-tab", second),
    ] {
        window.simulate_keystrokes(keystroke);
        draw_frame(&mut window);
        assert_panel_focused(&root, &mut window, cx, project, expected);
    }

    assert_eq!(state.borrow().attempts, 3, "循环不得重建 Panel 或 PTY");
    for panel in [first, second, neighbor] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ));
    }
}

#[test]
fn panel_cycle_keymap_is_global_in_terminal_and_editor_contexts() {
    let keymap = gpui::Keymap::new(CommandRegistry::panel_cycle_bindings_for_test());
    for context in ["SltermTerminal", "FileEditor"] {
        let contexts = [gpui::KeyContext::parse(context).unwrap()];
        for (keystroke, next) in [("ctrl-tab", true), ("ctrl-shift-tab", false)] {
            let input = [gpui::Keystroke::parse(keystroke).unwrap()];
            let (bindings, pending) = keymap.bindings_for_input(&input, &contexts);
            assert!(!pending);
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                bindings[0].action().as_any().is::<FocusNextPanel>(),
                next,
                "{context} 的 {keystroke} 必须命中全局面板循环"
            );
            assert_eq!(
                bindings[0].action().as_any().is::<FocusPreviousPanel>(),
                !next
            );
        }
    }
}

#[gpui::test]
fn closing_focused_panel_uses_next_previous_then_neighbor_active(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/navigation");
    let first = new_generated_terminal(&root, &mut window);
    let second = new_generated_terminal(&root, &mut window);
    let third = new_generated_terminal(&root, &mut window);
    let _neighbor = split_generated_terminal(&root, &mut window);
    let neighbor_active = new_generated_terminal(&root, &mut window);

    for (closed, expected) in [(second, third), (third, first), (first, neighbor_active)] {
        let terminal = terminal_for_panel(&root, cx, closed);
        window
            .update(|window, cx| {
                root.update(cx, |root, cx| root.activate_terminal(terminal, window, cx))
            })
            .unwrap();
        window
            .update(|window, cx| {
                root.update(cx, |root, cx| root.close_terminal(terminal, window, cx))
            })
            .unwrap();
        window.run_until_parked();
        assert_panel_focused(&root, &mut window, cx, project, expected);
    }

    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project, cx)),
        2
    );
    assert_eq!(state.borrow().attempts, 5, "关闭还焦不得重建任何终端");
}
