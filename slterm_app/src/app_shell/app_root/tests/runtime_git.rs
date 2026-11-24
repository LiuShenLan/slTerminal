//! Runtime Git 状态目标：前台只解析项目与仓库入口。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};

use super::*;

fn dispatch_git_status(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project_id: Option<ProjectId>,
) -> Result<crate::app_shell::RuntimeGitTarget, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeGitStatus {
                    project_id,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回 Git 目标回执")
}

#[gpui::test]
fn runtime_git_target_resolves_project_root_and_rejects_unknown(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);

    let Err(error) = dispatch_git_status(&root, &mut window, None) else {
        panic!("无活跃项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let target =
        dispatch_git_status(&root, &mut window, None).expect("活跃项目必须解析出 Git 目标");
    assert_eq!(target.project_id, project_id(PROJECT_A));
    assert_eq!(target.root, path("D:/work/a"));

    let target = dispatch_git_status(&root, &mut window, Some(project_id(PROJECT_A)))
        .expect("显式项目必须解析");
    assert_eq!(target.project_id, project_id(PROJECT_A));

    let Err(error) = dispatch_git_status(&root, &mut window, Some(project_id(PROJECT_B))) else {
        panic!("未知项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let Err(error) = dispatch_git_status(&root, &mut window, None) else {
        panic!("关闭中必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
