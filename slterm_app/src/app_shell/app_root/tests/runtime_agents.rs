//! Runtime Agent 资源：持久会话身份与运行期仲裁状态派生。

use std::sync::mpsc;

use slterm_agents::AgentSession;
use slterm_runtime_protocol::{AgentResource, DomainErrorCode, JsonRpcError};

use super::*;

fn dispatch_agents(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
) -> Result<Vec<AgentResource>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::RuntimeAgents { completion }, window, cx);
        });
    });
    response
        .try_recv()
        .expect("AppRoot 必须返回 Agent 资源回执")
}

fn dispatch_mutation(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    mutation: crate::local_control::RuntimeMutation,
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
        Ok(crate::app_shell::RuntimeMutationReply::Done(result)) => result,
        Ok(crate::app_shell::RuntimeMutationReply::Git(_))
        | Ok(crate::app_shell::RuntimeMutationReply::GitOpen(_)) => {
            panic!("Agent 写意图不得返回 Git 目标")
        }
        Err(error) => Err(error),
    }
}

fn runtime_generation(root: &Entity<AppRoot>, cx: &mut TestAppContext) -> u64 {
    root.read_with(cx, |root, _| {
        root.terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .expect("测试终端必须已在 Registry 注册")
            .generation()
            .get()
    })
}

fn seed_agent(root: &Entity<AppRoot>, cx: &mut TestAppContext, panel: PanelId) {
    root.update(cx, |root, _| {
        root.agents.seed_session(
            PaneId(panel.sequence()),
            AgentSession {
                source: "claude".to_owned(),
                session_id: Some("session-a".to_owned()),
                session_file: None,
            },
        );
    });
}

/// 锚：agent 写方法必须同时匹配对外身份、稳定终端与 PTY 代次。
#[gpui::test]
fn runtime_agent_input_and_close_require_matching_identity(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    seed_agent(&root, cx, panel);
    let generation = runtime_generation(&root, cx);

    let error = dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentInput {
            agent_id: "claude:other".to_owned(),
            terminal_id: terminal_id(TERMINAL_A),
            generation,
            data: "ls".to_owned(),
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::UnknownAgent)
    );

    dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentInput {
            agent_id: "claude:session-a".to_owned(),
            terminal_id: terminal_id(TERMINAL_A),
            generation,
            data: "ls".to_owned(),
        },
    )
    .expect("匹配身份的输入必须写入");
    let mut received = None;
    {
        let mut state = state.borrow_mut();
        let receiver = state
            .sessions
            .get_mut(&panel.sequence())
            .expect("终端会话必须存在");
        while let Ok(message) = receiver.try_recv() {
            if let Msg::Input(input) = message {
                received = Some(input.into_owned());
                break;
            }
        }
    }
    assert_eq!(received.as_deref(), Some(b"ls".as_slice()));

    let error = dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentClose {
            agent_id: "claude:session-a".to_owned(),
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
        crate::local_control::RuntimeMutation::AgentClose {
            agent_id: "claude:session-a".to_owned(),
            terminal_id: terminal_id(TERMINAL_A),
            generation,
        },
    )
    .expect("匹配身份与代次的关闭必须成功");
    root.read_with(cx, |root, _| {
        assert!(
            root.snapshot().terminal(terminal_id(TERMINAL_A)).is_none(),
            "关闭后逻辑终端必须移除"
        );
    });
}

/// 锚：agent.read/wait 必须先匹配目标终端的当前对外身份。
#[gpui::test]
fn runtime_agent_read_and_wait_require_matching_identity(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    seed_agent(&root, cx, panel);
    let generation = runtime_generation(&root, cx);

    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeRead {
                    terminal_id: terminal_id(TERMINAL_A),
                    generation,
                    agent_id: Some("claude:other".to_owned()),
                    completion,
                },
                window,
                cx,
            );
        });
    });
    let error = response.try_recv().unwrap().unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::UnknownAgent)
    );

    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeRead {
                    terminal_id: terminal_id(TERMINAL_A),
                    generation,
                    agent_id: Some("claude:session-a".to_owned()),
                    completion,
                },
                window,
                cx,
            );
        });
    });
    let worker_receipt = response
        .try_recv()
        .expect("匹配身份的读取必须入队")
        .expect("匹配身份的读取必须成功");
    let reply = {
        let mut state = state.borrow_mut();
        let receiver = state
            .sessions
            .get_mut(&panel.sequence())
            .expect("终端会话必须存在");
        loop {
            match receiver.try_recv() {
                Ok(Msg::ReadText { reply }) => break reply,
                Ok(_) => {}
                Err(TryRecvError::Empty) => panic!("worker 未收到读取命令"),
                Err(TryRecvError::Disconnected) => panic!("worker 通道不得提前断开"),
            }
        }
    };
    let expected = slterm_terminal::term::TextSnapshot {
        revision: 3,
        text: "hello".to_owned(),
    };
    reply.send(expected.clone()).expect("读取回执端必须存活");
    assert_eq!(worker_receipt.recv().unwrap(), expected);

    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeWait {
                    terminal_id: terminal_id(TERMINAL_A),
                    generation,
                    agent_id: Some("claude:other".to_owned()),
                    completion,
                },
                window,
                cx,
            );
        });
    });
    let error = response.try_recv().unwrap().unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::UnknownAgent)
    );
}

/// 锚：start/resume/fork 各建一个终端；未知 CLI 与不支持的动作明确拒绝。
#[gpui::test]
fn runtime_agent_start_resume_and_fork_create_terminals(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");

    dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentStart {
            kind: "claude".to_owned(),
            project_root: None,
            cwd: None,
        },
    )
    .expect("启动 Agent 必须成功");
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 1);
    });

    let error = dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentStart {
            kind: "unknown-cli".to_owned(),
            project_root: None,
            cwd: None,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    let error = dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentResume {
            source: "aider".to_owned(),
            session_id: Some("abc".to_owned()),
            session_file: None,
            project_root: None,
            cwd: None,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentResume {
            source: "claude".to_owned(),
            session_id: Some("abc".to_owned()),
            session_file: None,
            project_root: None,
            cwd: None,
        },
    )
    .expect("恢复支持的会话必须成功");

    dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentFork {
            source: "claude".to_owned(),
            session_id: "abc".to_owned(),
            project_root: None,
            cwd: None,
        },
    )
    .expect("分支支持的会话必须成功");
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 3);
    });

    let error = dispatch_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AgentFork {
            source: "aider".to_owned(),
            session_id: "abc".to_owned(),
            project_root: None,
            cwd: None,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );
}

#[gpui::test]
fn runtime_agents_derive_from_session_identity_and_status(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    assert!(
        dispatch_agents(&root, &mut window)
            .expect("Agent 列表必须成功")
            .is_empty(),
        "纯 Shell 不产生 Agent 关联行"
    );

    root.update(cx, |root, _| {
        root.agents.seed_session(
            PaneId(panel.sequence()),
            AgentSession {
                source: "claude".to_owned(),
                session_id: Some("session-a".to_owned()),
                session_file: None,
            },
        );
    });
    let agents = dispatch_agents(&root, &mut window).expect("Agent 列表必须成功");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_id, "claude:session-a");
    assert_eq!(agents[0].terminal_id, terminal_id(TERMINAL_A).to_string());
    assert_eq!(agents[0].project_id, project_id(PROJECT_A).to_string());
    assert_eq!(agents[0].kind, "claude");
    assert_eq!(agents[0].status, "unknown");
    assert_eq!(agents[0].native_session_id.as_deref(), Some("session-a"));

    root.update(cx, |root, _| root.close_in_progress = true);
    let error = dispatch_agents(&root, &mut window).unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
