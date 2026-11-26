//! Runtime 写意图：复用既有项目命令、关闭后结构化拒绝。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};
use slterm_workspace::ViewKey;

use super::*;
use crate::app_shell::RuntimeMutationReply;
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
    match response.try_recv().expect("AppRoot 必须返回写意图回执") {
        Ok(RuntimeMutationReply::Done(result)) => result,
        Ok(RuntimeMutationReply::Git(_)) | Ok(RuntimeMutationReply::GitOpen(_)) => {
            panic!("普通写意图不得返回 Git 目标")
        }
        Err(error) => Err(error),
    }
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

/// 当前测试终端的 PTY 代次；Runtime 输入必须精确绑定该代次。
fn terminal_generation(root: &Entity<AppRoot>, cx: &mut TestAppContext) -> u64 {
    root.read_with(cx, |root, _| {
        root.terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .expect("测试终端必须已在 Registry 注册")
            .generation()
            .get()
    })
}

/// 取 worker 收到的下一段输入；跳过安装期的非输入命令。
fn pending_terminal_input(state: &Rc<RefCell<LauncherState>>, panel: PanelId) -> Option<Vec<u8>> {
    let mut state = state.borrow_mut();
    let receiver = state
        .sessions
        .get_mut(&panel.sequence())
        .expect("终端会话必须存在");
    loop {
        match receiver.try_recv() {
            Ok(Msg::Input(input)) => return Some(input.into_owned()),
            Ok(_) => {}
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => panic!("worker 通道不得提前断开"),
        }
    }
}

/// 锚：原始输入不追加回车，execute 只追加一次回车；过期代次拒绝且零写入。
#[gpui::test]
fn runtime_terminal_input_reaches_worker_with_exact_bytes(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let generation = terminal_generation(&root, cx);

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalInput {
            terminal_id: terminal_id(TERMINAL_A),
            generation,
            data: "echo hi".to_owned(),
        },
    )
    .expect("原始输入必须写入 PTY");
    assert_eq!(
        pending_terminal_input(&state, panel).as_deref(),
        Some(b"echo hi".as_slice()),
        "input 必须原样写入，不得追加回车"
    );

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalExecute {
            terminal_id: terminal_id(TERMINAL_A),
            generation,
            command: "cargo test".to_owned(),
        },
    )
    .expect("单行执行必须写入 PTY");
    assert_eq!(
        pending_terminal_input(&state, panel).as_deref(),
        Some(b"cargo test\r".as_slice()),
        "execute 只追加一次回车"
    );
}

#[gpui::test]
fn runtime_terminal_input_rejects_stale_generation(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let generation = terminal_generation(&root, cx);

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalInput {
            terminal_id: terminal_id(TERMINAL_A),
            generation: generation + 1,
            data: "echo hi".to_owned(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, -32001);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );
    assert!(
        pending_terminal_input(&state, panel).is_none(),
        "过期代次不得写入 PTY"
    );

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalExecute {
            terminal_id: terminal_id(TERMINAL_A),
            generation: 0,
            command: "echo hi".to_owned(),
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );
    assert!(
        pending_terminal_input(&state, panel).is_none(),
        "非法代次不得写入 PTY"
    );
}

/// 锚：close 必须匹配快照代次；匹配后逻辑终端与面板一并移除。
#[gpui::test]
fn runtime_terminal_close_requires_current_generation(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let generation = terminal_generation(&root, cx);

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalClose {
            terminal_id: terminal_id(TERMINAL_A),
            generation: generation + 1,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, -32005);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::RevisionConflict)
    );
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot().terminal(terminal_id(TERMINAL_A)).is_some(),
            "代次冲突不得关闭终端"
        );
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalClose {
            terminal_id: terminal_id(TERMINAL_A),
            generation,
        },
    )
    .expect("当前代次必须允许关闭");
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot().terminal(terminal_id(TERMINAL_A)).is_none(),
            "关闭后逻辑终端必须移除"
        );
    });

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalClose {
            terminal_id: terminal_id(TERMINAL_A),
            generation: 0,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest),
        "未知终端必须明确拒绝"
    );
}

/// 锚：reconnect 只接受已退出终端且代次必须匹配；成功后代次推进到新进程。
#[gpui::test]
fn runtime_terminal_reconnect_requires_exited_terminal_and_current_generation(
    cx: &mut TestAppContext,
) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let terminal = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    window.update(|_, cx| {
        terminal.update(cx, |view, cx| view.mark_test_exited(cx));
    });
    window.run_until_parked();
    let generation = terminal_generation(&root, cx);

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalReconnect {
            terminal_id: terminal_id(TERMINAL_A),
            generation: generation + 1,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::RevisionConflict)
    );

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalReconnect {
            terminal_id: terminal_id(TERMINAL_A),
            generation,
        },
    )
    .expect("已退出终端必须可重连");
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.terminals
                .borrow()
                .target(terminal_id(TERMINAL_A))
                .unwrap()
                .generation()
                .get(),
            generation + 1,
            "重连必须分配新 PTY 代次"
        );
        assert_eq!(
            root.snapshot
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .run_intent(),
            RunIntent::Launch
        );
    });

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalReconnect {
            terminal_id: terminal_id(TERMINAL_A),
            generation: generation + 1,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady),
        "运行中终端不得重连"
    );
}

/// 锚：rename 只改自定义标题；空白、超长与未知终端零副作用。
#[gpui::test]
fn runtime_terminal_rename_updates_title_and_validates(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalRename {
            terminal_id: terminal_id(TERMINAL_A),
            title: Some("build".to_owned()),
        },
    )
    .expect("合法标题必须写入");
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot()
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .custom_title(),
            Some("build")
        );
    });

    dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalRename {
            terminal_id: terminal_id(TERMINAL_A),
            title: None,
        },
    )
    .expect("清除标题必须成功");
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot()
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .custom_title(),
            None
        );
    });

    for title in [Some("  ".to_owned()), Some("界".repeat(129))] {
        let error = dispatch_mutation(
            &root,
            &mut window,
            RuntimeMutation::TerminalRename {
                terminal_id: terminal_id(TERMINAL_A),
                title,
            },
        )
        .unwrap_err();
        assert_eq!(
            error.data.as_ref().map(|data| data.code),
            Some(DomainErrorCode::InvalidRequest)
        );
    }

    let error = dispatch_mutation(
        &root,
        &mut window,
        RuntimeMutation::TerminalRename {
            terminal_id: terminal_id(TERMINAL_B),
            title: Some("ghost".to_owned()),
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );
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
