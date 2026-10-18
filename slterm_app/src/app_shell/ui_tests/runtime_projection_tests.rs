//! Workspace → RuntimeSnapshot 投影与 hub 入账(虚拟窗口)。
//!
//! 断言面 = 单窗口收编、pane 集合 == 树叶集合、投影不盖戳、publish 首见
//! 计数从 1 起且重复投影不烧 revision。

use super::*;
use crate::app_shell::workspace::runtime_projection::project_workspace;
use crate::runtime_api::{RuntimeTaskState, SINGLE_WINDOW_ID};

#[gpui::test]
fn workspace_projection_is_single_window_with_one_pane_per_leaf(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    let (snapshot, first_pane) = cx.update(|cx| {
        let workspace = workspace.read(cx);
        (project_workspace(workspace, cx), workspace.first_pane_id())
    });

    assert_eq!(snapshot.process_id, std::process::id());
    assert_eq!(
        snapshot.protocol_version,
        crate::runtime_api::PROTOCOL_VERSION
    );
    assert_eq!(snapshot.revision, 0, "投影方不盖 revision");
    assert_eq!(snapshot.pane_lifecycles.len(), 0);
    assert_eq!(snapshot.windows.len(), 1, "单窗口恒一元素");

    let window = &snapshot.windows[0];
    assert_eq!(window.id, SINGLE_WINDOW_ID);
    assert!(window.focused);
    assert_eq!(window.active_tab, 0);
    assert_eq!(window.focused_pane_id, first_pane.map(|id| id.0));
    assert!(!window.tabs.is_empty());

    let tab = &window.tabs[0];
    assert!(tab.active);
    assert_eq!(tab.kind, "terminal");
    assert!(tab.tab_id.is_some(), "稳定 TabId 直出协议");
    assert_eq!(tab.panes.len(), 1, "pane 集合 == 树叶集合");
    let pane = &tab.panes[0];
    assert_eq!(Some(pane.id), first_pane.map(|id| id.0));
    assert!(pane.active);
    assert_eq!(pane.task_state, RuntimeTaskState::Idle);
    assert_eq!(pane.state_change_seq, 0, "投影调用方留 0");
    assert!(matches!(
        tab.layout,
        Some(crate::runtime_api::RuntimeLayout::Pane { pane_id }) if pane_id == pane.id
    ));
}

#[gpui::test]
fn publish_stamps_the_projection_and_dedups_identical_renders(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    let (hub, snapshot) = cx.update(|cx| {
        let workspace = workspace.read(cx);
        (workspace.runtime_hub(), project_workspace(workspace, cx))
    });

    let first = hub.publish(snapshot.clone());
    assert_eq!(first.revision, 1);
    assert_eq!(first.windows[0].tabs[0].panes[0].state_change_seq, 1);

    // 完全相同的投影:不烧 revision,计数保留。
    let duplicate = hub.publish(snapshot);
    assert_eq!(duplicate.revision, 1);
    assert_eq!(duplicate.windows[0].tabs[0].panes[0].state_change_seq, 1);
}
