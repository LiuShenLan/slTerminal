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
fn runtime_panel_open_settings_reuses_existing_entry(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    dispatch_mutation(&root, &mut window, RuntimeMutation::PanelOpenSettings)
        .expect("无项目时必须打开应用级设置浮层");
    root.read_with(cx, |root, _| {
        assert!(root.settings_overlay.is_some(), "应用级设置浮层必须存在");
    });

    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    dispatch_mutation(&root, &mut window, RuntimeMutation::PanelOpenSettings)
        .expect("有项目时必须打开项目设置视图");
    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout();
        let has_settings = layout.group_ids().iter().any(|group| {
            layout
                .group(*group)
                .unwrap()
                .views()
                .iter()
                .any(|view| view.kind_str() == "settings")
        });
        assert!(has_settings, "项目设置视图必须进入布局");
    });
}

/// 锚：显式 cwd 越权时不创建空 Panel、不改投活跃项目；
/// 隐藏项目内 cwd 必须作用于该项目而保持活跃项目不变。
#[gpui::test]
fn runtime_cwd_does_not_fallback(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    activate_project(&root, &mut window, PROJECT_A);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalCreate {
            project_root: None,
            cwd: Some("D:/outside/scope".to_owned()),
        },
    )
    .expect_err("越权 cwd 必须拒绝");
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Unauthorized)
    );
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot(), &before, "失败不得创建面板或改动快照");
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A)),
            "失败不得改投活跃项目"
        );
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalCreate {
            project_root: None,
            cwd: Some("D:/work/b/src".to_owned()),
        },
    )
    .expect("隐藏项目内 cwd 必须创建终端");
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot()
                .terminals()
                .any(|terminal| terminal.project_id() == project_id(PROJECT_B)),
            "新终端必须属于 cwd 最深匹配的项目"
        );
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B)),
            "合法目标选择激活该项目，而不是回退到原活跃项目"
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
