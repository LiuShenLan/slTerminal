//! 管道端到端集成用例(出口,真机、串行)。
//!
//! 链路全程不 mock:真命名管道服务线程 → `GetNamedPipeClientProcessId` →
//! `parse_envelope` → 进程内 mpsc → `AgentHub` 的 pane 归属与仲裁器。
//! 客户端按 `slterm_hook::send_local` 的同一形态打开管道写入(helper 进程
//! 的唯一传输动作),载荷是真实的 claude hook JSON。
//!
//! 运行纪律:管道名按进程 id 固定,用例必须串行(`--test-threads=1`);
//! 进程内再加一把锁,防默认并行执行时两个服务线程抢同名管道。

use std::io::Write as _;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use slterm_split::PaneId;

use crate::agent_host::AgentHub;
use crate::ai_agents::{AgentStatus, AgentStatusSource};
use crate::ai_hook::{AiHookEvent, AiHookKind, PIPE_ENV, spawn_gpui_server};

/// 进程内串行锁(生产纪律之外的兜底;真机管道同名)。
static E2E_LOCK: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    E2E_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// helper 形态客户端:命名管道按文件路径写打开,竞态换实例时短重试
/// (与 `slterm_hook` 的 send_local 20×5ms 同形态,窗口放大到 1s)。
fn write_envelope(pipe: &str, message: &[u8]) {
    for _ in 0..200 {
        match std::fs::OpenOptions::new().write(true).open(pipe) {
            Ok(mut file) => {
                file.write_all(message).expect("写命名管道");
                return;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("命名管道在 1s 内未接受客户端连接");
}

/// 服务线程可确定性停止:接收端先关,shutdown 仍能唤醒 accept 阻塞并
/// join(幂等;用例能跑完即证明没有 detached 线程挂住进程)。
#[test]
fn pipe_server_shutdown_joins_the_accept_thread() {
    let _serial = serial();
    let (receiver, mut server) = spawn_gpui_server();
    drop(receiver);
    let started = std::time::Instant::now();
    server.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "无客户端时 shutdown 应立即返回"
    );
    // 幂等:join 已取走,再次调用只置停止位。
    server.shutdown();
}

/// 防复发:客户端连上后保持句柄、不写不关,serve 线程阻塞在 `ReadFile`;
/// shutdown 必须取消该同步 I/O 并在 2s 内返回(不能依赖客户端配合关闭)。
#[test]
fn pipe_server_shutdown_cancels_a_client_blocking_readfile() {
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};

    let _serial = serial();
    let (_receiver, mut server) = spawn_gpui_server();
    let pipe = std::env::var(PIPE_ENV).expect("启动管道服务必须发布管道名");
    let wide: Vec<u16> = pipe.encode_utf16().chain(Some(0)).collect();

    // 以 helper 同形态的客户端连接并故意保持句柄:不写、不关,服务线程
    // 处理完连接后阻塞在 ReadFile 上。
    let mut client = INVALID_HANDLE_VALUE;
    for _ in 0..200 {
        // SAFETY: `wide` 以 NUL 结尾且活得比这次调用长;空安全属性 =
        // 默认 DACL,仅同用户可访问。
        client = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if client != INVALID_HANDLE_VALUE {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_ne!(
        client, INVALID_HANDLE_VALUE,
        "客户端必须在 1s 内连上命名管道"
    );

    // 负载裕度(非断言放宽):连接建立后 serve 线程才从 ConnectNamedPipe
    // 返回并进入 ReadFile;这里只等调度落位,确保用例覆盖的是 ReadFile 取消。
    std::thread::sleep(Duration::from_millis(100));

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let shutdown = std::thread::spawn(move || {
        server.shutdown();
        let _ = done_tx.send(());
    });
    let returned = done_rx.recv_timeout(Duration::from_secs(2));
    // 先关客户端句柄再断言:即使取消路径回归、shutdown 超时,ReadFile 也能
    // 读到 EOF 让 serve 线程退出,用例失败但不留挂起线程或泄漏句柄。
    // SAFETY: `client` 是上面成功打开的有效句柄,且只关一次。
    unsafe {
        CloseHandle(client);
    }
    assert!(returned.is_ok(), "shutdown 未能在 2s 内取消阻塞的 ReadFile");
    shutdown.join().expect("shutdown 线程不得 panic");
}

/// 起服务 → helper 形态写一条 PromptSubmit → 事件通道收取 → pane 归属仲裁。
#[test]
fn real_pipe_prompt_submit_reaches_the_owning_pane_host() {
    let _serial = serial();
    let (receiver, mut server) = spawn_gpui_server();
    let pipe = std::env::var(PIPE_ENV).expect("启动管道服务必须发布管道名");

    let mut hub = AgentHub::new();
    hub.register(PaneId(7));
    hub.register(PaneId(8));
    let payload = r#"{"hook_event_name":"UserPromptSubmit","session_id":"pipe-e2e-prompt"}"#;
    write_envelope(
        &pipe,
        format!("slterm-hook/1 source=claude pane=7\n{payload}").as_bytes(),
    );

    let event = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("事件必须经真管道抵达宿主通道");
    assert_eq!(event.pane, Some(7));
    assert_eq!(event.source, "claude");
    assert_eq!(hub.dispatch(vec![event], None), 1);
    let owner = hub.host(PaneId(7)).expect("pane 7 宿主");
    assert_eq!(owner.status(), AgentStatus::Working);
    assert_eq!(owner.source(), AgentStatusSource::Hook);
    // 信封 pane 是严格路由合同:另一个 pane 不得收到任何状态。
    assert_eq!(hub.host(PaneId(8)).unwrap().status(), AgentStatus::Unknown);

    server.shutdown();
}

/// 坏载荷(别家 runner 的 camelCase 形态)在宿主侧解析被拒,绝不投递;
/// 紧随其后的合法 TurnDone 只更新本 pane 的生命周期。
#[test]
fn real_pipe_foreign_payload_is_dropped_and_turn_done_is_owned_by_pane() {
    let _serial = serial();
    let (receiver, mut server) = spawn_gpui_server();
    let pipe = std::env::var(PIPE_ENV).expect("启动管道服务必须发布管道名");

    let mut hub = AgentHub::new();
    hub.register(PaneId(3));
    hub.register(PaneId(4));
    // 串台门失效形态:若宿主侧不拒收,它会是先到的那条事件。
    write_envelope(
        &pipe,
        b"slterm-hook/1 source=claude pane=3\n\
          {\"hookEventName\":\"Stop\",\"sessionId\":\"foreign\"}",
    );
    let payload = r#"{"hook_event_name":"Stop","session_id":"pipe-e2e-answer","last_assistant_message":"原样回答"}"#;
    write_envelope(
        &pipe,
        format!("slterm-hook/1 source=claude pane=3\n{payload}").as_bytes(),
    );

    let event: AiHookEvent = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("合法事件必须抵达宿主通道");
    assert_eq!(event.kind, AiHookKind::TurnDone);
    assert_eq!(event.pane, Some(3));
    assert_eq!(hub.dispatch(vec![event], None), 1);

    let owner = hub.host(PaneId(3)).expect("pane 3 宿主");
    assert_eq!(owner.status(), AgentStatus::Done);
    assert_eq!(hub.host(PaneId(4)).unwrap().status(), AgentStatus::Unknown);

    server.shutdown();
}
