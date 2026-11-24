//! Runtime 终端读取：前台校验代次后把 worker 文本快照命令入队。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};
use slterm_terminal::term::TextSnapshot;

use super::*;

fn dispatch_read(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    terminal_id: TerminalId,
    generation: u64,
) -> Result<mpsc::Receiver<TextSnapshot>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeRead {
                    terminal_id,
                    generation,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回读取回执")
}

/// 从测试 worker 通道取出读取命令的应答端；跳过安装期的其他命令。
fn take_read_reply(
    state: &Rc<RefCell<LauncherState>>,
    panel: PanelId,
) -> mpsc::SyncSender<TextSnapshot> {
    let mut state = state.borrow_mut();
    let receiver = state
        .sessions
        .get_mut(&panel.sequence())
        .expect("终端会话必须存在");
    loop {
        match receiver.try_recv() {
            Ok(Msg::ReadText { reply }) => return reply,
            Ok(_) => {}
            Err(TryRecvError::Empty) => panic!("worker 未收到读取命令"),
            Err(TryRecvError::Disconnected) => panic!("worker 通道不得提前断开"),
        }
    }
}

#[gpui::test]
fn runtime_read_queues_worker_snapshot_on_matching_generation(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let generation = root.read_with(cx, |root, _| {
        root.terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .expect("测试终端必须已在 Registry 注册")
            .generation()
            .get()
    });

    let receiver = dispatch_read(&root, &mut window, terminal_id(TERMINAL_A), generation)
        .expect("匹配代次的读取必须入队");
    let reply = take_read_reply(&state, panel);
    let expected = TextSnapshot {
        revision: 7,
        text: "hello\nworld".to_owned(),
    };
    reply.send(expected.clone()).expect("读取回执端必须存活");
    assert_eq!(receiver.recv().expect("worker 快照必须回传"), expected);
}

#[gpui::test]
fn runtime_read_rejects_stale_generation_and_close(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let generation = root.read_with(cx, |root, _| {
        root.terminals
            .borrow()
            .target(terminal_id(TERMINAL_A))
            .expect("测试终端必须已在 Registry 注册")
            .generation()
            .get()
    });

    let error = dispatch_read(&root, &mut window, terminal_id(TERMINAL_A), generation + 1)
        .expect_err("过期代次必须拒绝");
    assert_eq!(error.code, -32001);
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );
    assert!(
        no_pending_read(&state, panel),
        "拒绝的读取不得向 worker 发送命令"
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let error = dispatch_read(&root, &mut window, terminal_id(TERMINAL_A), generation)
        .expect_err("关闭中必须拒绝");
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}

/// 通道中不得残留读取命令；非读取命令只记录并跳过。
fn no_pending_read(state: &Rc<RefCell<LauncherState>>, panel: PanelId) -> bool {
    let mut state = state.borrow_mut();
    let Some(receiver) = state.sessions.get_mut(&panel.sequence()) else {
        return true;
    };
    loop {
        match receiver.try_recv() {
            Ok(Msg::ReadText { .. }) => return false,
            Ok(_) => {}
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return true,
        }
    }
}
