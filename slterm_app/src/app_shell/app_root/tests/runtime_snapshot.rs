//! Runtime 快照事件：前台同一时刻资源与关闭后的结构化拒绝。

use std::str::FromStr as _;
use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError, SnapshotResources};
use slterm_workspace::TerminalId;

use super::*;

fn dispatch_snapshot(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
) -> Result<SnapshotResources, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::RuntimeSnapshot { completion }, window, cx);
        });
    });
    response.try_recv().expect("AppRoot 必须返回快照回执")
}

#[gpui::test]
fn runtime_snapshot_returns_workspace_resources_and_identity(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    let snapshot = dispatch_snapshot(&root, &mut window).expect("快照必须成功");

    assert_eq!(snapshot.projects.len(), 1);
    assert_eq!(
        snapshot.projects[0].project_id,
        project_id(PROJECT_A).to_string()
    );
    assert_eq!(snapshot.projects[0].display_name, "a");
    assert_eq!(snapshot.projects[0].root, "D:/work/a");
    assert!(snapshot.projects[0].active);
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(snapshot.groups[0].group_id, group_id(GROUP_A).to_string());
    assert_eq!(snapshot.layouts.len(), 1);
    assert_eq!(
        snapshot.layouts[0].root,
        Some(slterm_runtime_protocol::LayoutNodeResource::Group {
            group_id: group_id(GROUP_A).to_string(),
        }),
        "快照必须暴露可推导 resize 路径的布局树"
    );
    assert_eq!(snapshot.groups[0].views.len(), 1);
    assert_eq!(
        snapshot.groups[0].views[0].kind,
        slterm_runtime_protocol::ViewKind::Terminal
    );
    assert!(
        snapshot.groups[0].views[0].key.contains("\"terminal\""),
        "视图 key 必须是规范 ViewKey JSON: {}",
        snapshot.groups[0].views[0].key
    );
    assert_eq!(
        snapshot.groups[0].active_view,
        snapshot.groups[0].views[0].key
    );
    assert_eq!(snapshot.terminals.len(), 1);
    let terminal = TerminalId::from_str(TERMINAL_A).unwrap();
    assert_eq!(snapshot.terminals[0].terminal_id, terminal.to_string());
    assert_eq!(
        snapshot.terminals[0].project_id,
        project_id(PROJECT_A).to_string()
    );
    assert_eq!(
        snapshot.terminals[0].run_intent,
        slterm_runtime_protocol::TerminalRunIntent::Launch
    );
    assert_eq!(snapshot.terminals[0].cwd, "D:/work/a");
    assert_eq!(
        snapshot.terminals[0].generation, 1,
        "快照必须暴露当前 PTY 代次供写方法定位"
    );
}

#[gpui::test]
fn runtime_snapshot_is_rejected_while_close_is_in_progress(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    root.update(cx, |root, _| root.close_in_progress = true);

    let error = dispatch_snapshot(&root, &mut window).unwrap_err();

    assert_eq!(error.code, -32004);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}

#[gpui::test]
fn runtime_snapshot_is_rejected_after_close_completes(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    root.update(cx, |root, _| root.close_completed = true);

    let error = dispatch_snapshot(&root, &mut window).unwrap_err();

    assert_eq!(error.code, -32004);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
