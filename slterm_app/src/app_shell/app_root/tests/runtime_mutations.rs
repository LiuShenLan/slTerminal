//! Runtime 写意图：复用既有项目命令、关闭后结构化拒绝。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};
use slterm_workspace::{LogicalSize, ViewKey};

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
fn runtime_panel_mutations_reuse_layout_commands(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let project = project_id(PROJECT_A);
    let view_a = ViewKey::Terminal {
        project_id: project,
        terminal_id: terminal_id(TERMINAL_A),
    };
    let view_b = ViewKey::Terminal {
        project_id: project,
        terminal_id: terminal_id(TERMINAL_B),
    };

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::PanelActivate {
            view: view_a.clone(),
        },
    )
    .expect("激活视图必须成功");
    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group(group_id(GROUP_A)).unwrap().active(), &view_a);
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::PanelGroupSplit {
            view: view_b.clone(),
            target_group: group_id(GROUP_A),
            direction: slterm_split::SplitDirection::LeftRight,
            ratio: 0.5,
            viewport: LogicalSize {
                width: 1200.0,
                height: 800.0,
            },
        },
    )
    .expect("拆分视图必须成功");
    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids().len(), 2, "拆分后必须有两个组");
        assert!(
            layout
                .group_ids()
                .iter()
                .all(|group| layout.group(*group).is_some())
        );
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
