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

/// 空工作区仍返回合法空快照，不伪造项目或终端。
#[gpui::test]
fn runtime_snapshot_empty_workspace_has_no_resources(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);

    let snapshot = dispatch_snapshot(&root, &mut window).expect("空工作区必须成功");

    assert!(snapshot.projects.is_empty());
    assert!(snapshot.groups.is_empty());
    assert!(snapshot.layouts.is_empty());
    assert!(snapshot.terminals.is_empty());
    assert!(snapshot.documents.is_empty());
}

/// 多项目、多组与隐藏项目全部进入快照，活跃标记来自项目注册表。
#[gpui::test]
fn runtime_snapshot_covers_hidden_projects_and_split_groups(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_C, TERMINAL_C);

    let snapshot = dispatch_snapshot(&root, &mut window).expect("多项目快照必须成功");

    assert_eq!(snapshot.projects.len(), 2);
    assert_eq!(
        snapshot
            .projects
            .iter()
            .filter(|project| project.active)
            .map(|project| project.project_id.as_str())
            .collect::<Vec<_>>(),
        [project_id(PROJECT_B).to_string()],
        "只有当前项目标记为活跃"
    );
    assert_eq!(snapshot.layouts.len(), 2);
    let project_a_layout = snapshot
        .layouts
        .iter()
        .find(|layout| layout.project_id == project_id(PROJECT_A).to_string())
        .expect("隐藏项目布局仍必须存在");
    assert!(
        matches!(
            project_a_layout.root,
            Some(slterm_runtime_protocol::LayoutNodeResource::Split { .. })
        ),
        "拆分后的隐藏项目必须保留分割树"
    );
    assert_eq!(snapshot.groups.len(), 3, "两个项目共三个面板组");
    assert_eq!(snapshot.terminals.len(), 3);
    for terminal in &snapshot.terminals {
        assert_eq!(terminal.generation, 1, "存活 worker 携带当前代次");
    }
}

/// 打开的真实 Document 进入快照资源。
#[gpui::test]
fn runtime_snapshot_lists_open_documents(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());

    let snapshot = dispatch_snapshot(&root, &mut window).expect("Document 快照必须成功");

    assert_eq!(snapshot.documents.len(), 1);
    assert_eq!(
        snapshot.documents[0].path,
        file.as_path().to_str().unwrap(),
        "Document 路径必须是规范文本"
    );
    assert_eq!(
        snapshot.documents[0].document_id,
        root.read_with(cx, |root, _| root.snapshot.documents().next().unwrap().0)
            .to_string()
    );
}

/// 已退出且无 worker 的终端代次为零，不猜测旧进程。
#[gpui::test]
fn runtime_snapshot_reports_zero_generation_for_exited_terminals(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let terminal = TerminalId::from_str(TERMINAL_A).unwrap();
    let target = root.read_with(cx, |root, _| {
        root.terminals.borrow().target(terminal).unwrap()
    });
    let session = root.read_with(cx, |root, _| {
        root.terminals.borrow_mut().remove(target).unwrap()
    });
    session.shutdown().unwrap();
    root.update(cx, |root, _| {
        root.snapshot = root
            .snapshot
            .with_terminal_run_intent(terminal, slterm_workspace::RunIntent::Exited)
            .unwrap();
    });

    let snapshot = dispatch_snapshot(&root, &mut window).expect("退出终端快照必须成功");

    assert_eq!(snapshot.terminals.len(), 1);
    assert_eq!(
        snapshot.terminals[0].run_intent,
        slterm_runtime_protocol::TerminalRunIntent::Exited
    );
    assert_eq!(
        snapshot.terminals[0].generation, 0,
        "无 worker 代次必须为零"
    );
}

/// 非 Unicode 路径不得以有损文本进入快照。
#[test]
fn snapshot_path_encoding_rejects_non_unicode_paths() {
    use std::os::windows::ffi::OsStringExt as _;

    let wide = [b'D' as u16, b':' as u16, b'\\' as u16, 0xD800];
    let path = CanonicalPath::new(std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &wide,
    )))
    .expect("绝对路径形状必须成立");
    let error = crate::app_shell::app_root::runtime_snapshot::path_text(&path).unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady),
        "编码失败必须返回封闭 NotReady"
    );
}
