//! Runtime 写命令的 UI owner 执行器(04 篇 M7.5)。
//!
//! 用虚拟窗口 + `test_session` 顶替真 PTY:命令经壳事件队列派发,断言
//! 回包形状(`action` + 提交后 `snapshot`)与终端字节落点。

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::Value;

use super::open_workspace;
use crate::ai_hook::parse_envelope;
use crate::app_shell::SltermShellEvent;
use crate::app_shell::workspace::SltermWorkspace;
use crate::runtime_api::{ApiError, RuntimeCommand, SINGLE_WINDOW_ID, dispatch_for_test};
use slterm_terminal::event_loop::Msg as EventLoopMsg;

/// 注入不拉真进程的测试会话并返回 PTY 写通道。
fn inject_test_session(
    workspace: &Entity<SltermWorkspace>,
    cx: &mut TestAppContext,
) -> std::sync::mpsc::Receiver<EventLoopMsg> {
    let view = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_view().expect("首个 pane")
    });
    view.update(cx, |view, _| {
        let (session, receiver) = crate::gpui_terminal::session::test_session();
        view.session = Some(session);
        receiver
    })
}

/// 把一条 RuntimeCommand 经壳事件泵的同一入口交给 UI owner。
fn dispatch(
    workspace: &Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    command: RuntimeCommand,
) -> Result<Value, ApiError> {
    let (dispatch, receiver) = dispatch_for_test(command);
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.handle_shell_event(SltermShellEvent::RuntimeControl(dispatch), window, cx);
        });
    });
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("UI owner 必须回执")
}

#[gpui::test]
fn pane_prompt_writes_the_bytes_and_returns_a_post_submit_running_baseline(
    cx: &mut TestAppContext,
) {
    let (workspace, mut window) = open_workspace(cx);
    let receiver = inject_test_session(&workspace, cx);

    let result = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::Prompt {
            window_id: None,
            pane_id: 1,
            text: "echo hi".into(),
            submit: true,
        },
    )
    .expect("prompt dispatch");

    match receiver.try_recv().expect("prompt 必须写进 PTY") {
        EventLoopMsg::Input(bytes) => assert_eq!(&*bytes, b"echo hi\r"),
        other => panic!("unexpected message: {other:?}"),
    }
    assert_eq!(result["action"]["window_id"], SINGLE_WINDOW_ID);
    assert_eq!(result["action"]["pane_id"], 1);
    let pane = &result["snapshot"]["windows"][0]["tabs"][0]["panes"][0];
    assert_eq!(pane["task_state"], "running", "提交同步建立 running 边沿");
    assert!(
        pane["state_change_seq"].as_u64().expect("seq") > 0,
        "CLI --wait 基线必须是非零计数"
    );
}

#[gpui::test]
fn pane_read_returns_the_real_grid_tail(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let _receiver = inject_test_session(&workspace, cx);
    let view = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_view().expect("首个 pane")
    });
    view.update(cx, |view, _| {
        let mut term = view.session.as_ref().expect("测试会话").term.lock();
        let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
        parser.advance(&mut *term, b"first line\r\nsecond line\r\n");
    });

    let result = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::ReadPane {
            window_id: None,
            pane_id: 1,
            // 测试会话是 24 行屏;请求行数覆盖整屏才能读到两行内容。
            lines: 80,
        },
    )
    .expect("read dispatch");
    let text = result["action"]["text"].as_str().expect("tail text");
    assert!(text.contains("second line"), "实际尾部: {text:?}");
    assert_eq!(result["action"]["requested_lines"], 80);
    assert_eq!(result["action"]["pane_id"], 1);
}

#[gpui::test]
fn pane_procs_reads_the_pane_shell_process_tree(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let _receiver = inject_test_session(&workspace, cx);
    let view = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_view().expect("首个 pane")
    });
    view.update(cx, |view, _| {
        view.session.as_mut().expect("测试会话").shell_pid = std::process::id();
    });

    let result = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::Procs {
            window_id: None,
            pane_id: 1,
        },
    )
    .expect("procs dispatch");
    assert_eq!(result["action"]["pane_id"], 1);
    assert_eq!(
        result["action"]["root_pid"],
        std::process::id(),
        "根进程 = PTY 直系 shell pid"
    );
}

#[gpui::test]
fn agent_start_registers_a_generation_and_agent_prompt_uses_it(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let receiver = inject_test_session(&workspace, cx);

    let started = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::AgentStart {
            window_id: None,
            pane_id: Some(1),
            name: "worker".into(),
            kind: crate::ai_agents::AgentKind::Claude,
            cwd: None,
            session_id: Some("session-1".into()),
            command: "claude".into(),
            worktree: None,
        },
    )
    .expect("agent start");
    let agent = &started["action"]["agent"];
    assert_eq!(agent["name"], "worker");
    assert_eq!(agent["generation"], 1);
    assert_eq!(agent["pane_id"], 1);
    assert!(agent["agent_id"].as_str().is_some_and(|id| !id.is_empty()));
    match receiver.try_recv().expect("启动命令必须写进 PTY") {
        EventLoopMsg::Input(bytes) => assert_eq!(&*bytes, b"claude\r"),
        other => panic!("unexpected message: {other:?}"),
    }

    let prompted = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::AgentPrompt {
            agent: "worker".into(),
            generation: Some(1),
            text: "fix the build".into(),
            submit: true,
        },
    )
    .expect("agent prompt");
    assert_eq!(prompted["action"]["agent"]["agent_id"], agent["agent_id"]);
    match receiver.try_recv().expect("prompt 必须写进 PTY") {
        EventLoopMsg::Input(bytes) => assert_eq!(&*bytes, b"fix the build\r"),
        other => panic!("unexpected message: {other:?}"),
    }

    let mismatch = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::AgentPrompt {
            agent: "worker".into(),
            generation: Some(2),
            text: "stale target".into(),
            submit: true,
        },
    )
    .expect_err("generation 不符必须失败");
    assert_eq!(mismatch.code, "agent_identity_mismatch");
}

#[gpui::test]
fn session_end_closes_the_managed_agent_and_busy_panes_block_window_close(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let _receiver = inject_test_session(&workspace, cx);
    dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::AgentStart {
            window_id: None,
            pane_id: Some(1),
            name: "worker".into(),
            kind: crate::ai_agents::AgentKind::Claude,
            cwd: None,
            session_id: Some("session-1".into()),
            command: "claude".into(),
            worktree: None,
        },
    )
    .expect("agent start");

    // 提交一旦建立 running 边沿,关窗必须显式拒绝,不静默强杀。
    dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::Prompt {
            window_id: None,
            pane_id: 1,
            text: "run".into(),
            submit: true,
        },
    )
    .expect("prompt");
    let blocked = dispatch(
        &workspace,
        &mut window,
        RuntimeCommand::CloseWindow { window_id: None },
    )
    .expect_err("忙碌 pane 必须拒绝关窗");
    assert_eq!(blocked.code, "confirmation_required");

    let end = parse_envelope(
        b"slterm-hook/1 source=claude pane=1\n\
          {\"hook_event_name\":\"SessionEnd\",\"session_id\":\"session-1\"}",
    )
    .expect("SessionEnd 信封");
    window.update(|_window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.process_agent_lifecycle_events(&[end], cx);
        });
    });
    let hub = workspace.read_with(cx, |workspace, _| workspace.runtime_hub());
    assert_eq!(
        hub.active_agent("worker", Some(1))
            .expect_err("SessionEnd 必须关闭注册项")
            .code,
        "agent_exited"
    );
    assert_eq!(SINGLE_WINDOW_ID, 1);
}
