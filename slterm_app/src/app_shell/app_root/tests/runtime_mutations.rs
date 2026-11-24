//! Runtime 写意图：复用既有项目命令、关闭后结构化拒绝。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};

use super::*;
use crate::local_control::RuntimeMutation;

fn dispatch_mutation(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    mutation: RuntimeMutation,
) -> Result<(), JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeMutation {
                    mutation,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回写意图回执")
}

#[gpui::test]
fn runtime_project_mutations_reuse_existing_commands(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    let project = project_id(PROJECT_A);

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::ProjectRename {
            project_id: project,
            name: "renamed".to_owned(),
        },
    )
    .expect("重命名必须成功");
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot()
                .registry()
                .project(project)
                .unwrap()
                .display_name(),
            "renamed"
        );
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::ProjectActivate {
            project_id: project,
        },
    )
    .expect("激活必须成功");
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().active(), Some(project));
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::ProjectReorder {
            project_id: project,
            new_index: 1,
        },
    )
    .expect("排序必须成功");
    root.read_with(cx, |root, _| {
        let order: Vec<_> = root
            .snapshot()
            .registry()
            .projects()
            .iter()
            .map(|project| project.id())
            .collect();
        assert_eq!(order, vec![project_id(PROJECT_B), project]);
    });
}

#[gpui::test]
fn runtime_project_mutations_are_rejected_while_closing(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    root.update(cx, |root, _| root.close_in_progress = true);

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::ProjectActivate {
            project_id: project_id(PROJECT_A),
        },
    )
    .unwrap_err();

    assert_eq!(error.code, -32004);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
