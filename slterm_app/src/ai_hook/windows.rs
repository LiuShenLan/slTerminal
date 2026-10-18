//! Windows 命名管道传输。为已解析的事件供给内核进程身份。
//!
//! 平台收敛纪律(03 篇边界 8):`windows-sys` 命名管道/进程身份核验只允许
//! 出现在本文件;协议解析、事件门、仲裁器平台无关。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::local::helper_path;
use crate::ai_hook::{AiHookEvent, HOOK_EXE_ENV, PIPE_ENV, parse_envelope};

// ─── 管道服务器 ─────────────────────────────────────────────────────────

/// 管道服务线程句柄。随应用进程生命周期存活(单实例、非 per-pane);
/// `shutdown`/Drop 确定性停止并 join,测试与关窗路径都不留 detached 线程。
pub struct PipeServer {
    name: String,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl PipeServer {
    /// 停止 accept 循环并等待线程退出。
    ///
    /// accept 线程绝大多数时间阻塞在 `ConnectNamedPipe` 上,停止位本身唤不醒
    /// 它:先置停止位,再以客户端形态自连一次把阻塞的调用顶回来;线程建立
    /// 连接之后先看停止位、立即退出。自连与「实例尚未创建」存在竞态,所以
    /// 有界重试到线程真正结束——串行测试纪律要求 join 可确定性返回,不许
    /// 泄漏后台线程(0xC0000409 类调度断言的温床)。
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let Some(join) = self.join.take() else {
            return;
        };
        for _ in 0..100 {
            if join.is_finished() {
                break;
            }
            // 写打开即 helper 的客户端形态;失败说明实例正在换手,下一轮再试。
            let _ = std::fs::OpenOptions::new().write(true).open(&self.name);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = join.join();
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// GPUI 形态(唯一形态;上游 winit EventLoopProxy 通道随 legacy-shell 整支
/// 裁剪)。GPUI 有自己的事件循环,但 hook 解析与管道所有权完全一致:
/// 返回的接收端由 workspace 的钩子泵在 foreground executor 上排空,按同一
/// 稳定 pane id 合同路由。服务端线程只做阻塞 I/O 与解析,绝不碰 UI 状态。
///
/// 返回句柄必须活到应用退出:启动链持有它,关窗后 Drop 完成线程回收。
pub fn spawn_gpui_server() -> (std::sync::mpsc::Receiver<AiHookEvent>, PipeServer) {
    let (tx, rx) = std::sync::mpsc::channel();
    let server = spawn_pipe_server(move |event| tx.send(event).is_ok());
    (rx, server)
}

fn spawn_pipe_server(sink: impl Fn(AiHookEvent) -> bool + Send + 'static) -> PipeServer {
    let name = format!(r"\\.\pipe\slterm-notify-{}", std::process::id());
    // SAFETY: 单线程启动期;此刻还没有别的线程读环境变量。
    unsafe {
        std::env::set_var(PIPE_ENV, &name);
    };
    // 为 opencode 插件导出 slterm-hook.exe 的路径(尽力而为:helper 找不到时,
    // 插件就像在 slTerminal 之外一样 no-op)。正斜杠:这条路径会被插值进插件
    // 内 Bun 的 `$` shell,与安装器写配置的 `helper_command` 形态(M4.3)一致。
    if let Some(helper) = helper_path() {
        let p = helper.display().to_string().replace('\\', "/");
        unsafe {
            std::env::set_var(HOOK_EXE_ENV, &p);
        };
    }
    let stop = Arc::new(AtomicBool::new(false));
    let join = {
        let serve_name = name.clone();
        let serve_stop = Arc::clone(&stop);
        match std::thread::Builder::new()
            .name("slterm-ai-pipe".into())
            .spawn(move || serve(&serve_name, sink, &serve_stop))
        {
            Ok(join) => Some(join),
            Err(err) => {
                log::warn!("ai_hook: failed to spawn pipe server: {err}");
                None
            }
        }
    };
    PipeServer { name, stop, join }
}

/// Accept 循环。每次连接一个全新管道实例:竞态换实例的客户端会看到几个
/// 微秒的打开失败并重试(helper 重试约 100ms——在这个消息速率下是永恒)。
fn serve(name: &str, sink: impl Fn(AiHookEvent) -> bool, stop: &AtomicBool) {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE,
    };
    // PIPE_ACCESS_INBOUND 是 FILE_FLAGS_AND_ATTRIBUTES 常量,所以它的家
    // 在 FileSystem 模块而不是 Pipes。
    use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    loop {
        // 停止位在实例创建前先看一眼:shutdown 的重试自连可能恰好落在
        // 「上一实例已断开、下一实例未建」的空档,此时打开会失败;这一眼
        // 让线程在下一轮立即退出,不依赖唤醒连接成功。
        if stop.load(Ordering::SeqCst) {
            return;
        }
        // SAFETY: `wide` 以 NUL 结尾且活得比这次调用长。空安全属性 =
        // 默认 DACL,仅同用户可访问。
        let pipe = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                0,
                64 * 1024,
                0,
                std::ptr::null(),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            log::warn!("ai_hook: CreateNamedPipeW failed; AI turn events disabled");
            return;
        }

        // SAFETY: `pipe` 是本帧持有的有效句柄。
        // ERROR_PIPE_CONNECTED = 客户端先连上了;同样是成功。
        let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if connected {
            // 唤醒连接(shutdown 的自连)不带载荷:只看停止位就走,不解析
            // 空消息、也不把这当作一次真实投递。
            if stop.load(Ordering::SeqCst) {
                // SAFETY: `pipe` 是本帧持有的有效句柄。
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                return;
            }
            // 客户端身份必须在断开连接之前问:这是内核对「谁在写这条管道」
            // 的回答,载荷里自报的任何 pid 都可以伪造,这个不行。helper 此刻
            // 一定还活着(它正连着我们),所以随后的祖先链查询能命中。
            let client_pid = {
                let mut pid = 0u32;
                // SAFETY: `pipe` 是本帧持有的有效句柄;`pid` 先于读取写入。
                let ok = unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) };
                (ok != 0 && pid != 0).then_some(pid)
            };
            let mut buf = Vec::with_capacity(4096);
            let mut chunk = [0u8; 4096];
            loop {
                let mut read = 0u32;
                // SAFETY: `chunk` 活得比调用长;`read` 先写后读。
                let ok = unsafe {
                    ReadFile(
                        pipe,
                        chunk.as_mut_ptr(),
                        chunk.len() as u32,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                // ok == 0 是正常 EOF(客户端关闭即 BROKEN_PIPE)。
                if ok == 0 || read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read as usize]);
                if buf.len() > (1 << 20) {
                    break;
                }
            }
            if dispatch(&buf, client_pid, &sink) == Dispatch::SinkClosed {
                // 事件循环已消失:正在关闭。
                // SAFETY: `pipe` 仍是上面那个有效句柄。
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                return;
            }
        }
        // SAFETY: `pipe` 有效;此点之后的失败只损失这一个实例,循环会创建
        // 全新实例。
        unsafe {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
        }
    }
}

/// 单条消息的处理结果(单元测试面;真机端到端链路见 `e2e_tests`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dispatch {
    Delivered,
    /// 超过 1MB 载荷上限:读完即丢,不解析截断 JSON。
    Oversized,
    /// 信封解析失败(坏协议头/未知 source/串台门闭合)。
    Unparsed,
    /// sink 拒收(事件循环已关)。
    SinkClosed,
}

/// 单条消息的处理内核,与管道 I/O 解耦:解析 + 内核身份注入 + 投递。
/// `client_pid` 只能来自内核对当前连接的回答(`GetNamedPipeClientProcessId`,
/// 断开前读),载荷自报的身份一律被它覆盖。
fn dispatch(buf: &[u8], client_pid: Option<u32>, sink: &impl Fn(AiHookEvent) -> bool) -> Dispatch {
    if buf.len() > (1 << 20) {
        return Dispatch::Oversized;
    }
    let Some(mut event) = parse_envelope(buf) else {
        return Dispatch::Unparsed;
    };
    event.client_pid = client_pid;
    // agent 的进程身份:helper 的父进程往往是执行 hook 命令的 shell,agent
    // 在更上一层,所以要沿祖先链找。用它区分嵌套子代理,见
    // `AiHookEvent::agent_pid`。
    event.agent_pid = client_pid
        .and_then(crate::process_tree::nearest_agent_ancestor)
        .map(|(pid, _)| pid);
    log::debug!("ai_hook: {event:?}");
    if sink(event) {
        Dispatch::Delivered
    } else {
        Dispatch::SinkClosed
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use std::cell::RefCell;

    fn envelope(body: &str) -> Vec<u8> {
        format!("slterm-hook/1 source=claude pane=3\n{body}").into_bytes()
    }

    /// 内核身份覆盖载荷自报:载荷里写的任何 pid 字段都到不了事件上,
    /// 事件携带的只能是服务端从内核问到的那个。
    #[test]
    fn kernel_client_pid_overrides_any_payload_claim() {
        let buf = envelope(
            r#"{"hook_event_name":"Stop","session_id":"s","client_pid":1,"pid":1,"helper_pid":1}"#,
        );
        let delivered = RefCell::new(Vec::new());
        let outcome = dispatch(&buf, Some(4242), &|event| {
            delivered.borrow_mut().push(event);
            true
        });
        assert_eq!(outcome, Dispatch::Delivered);
        let events = delivered.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].client_pid, Some(4242));
        // 4242 此刻不是活进程,祖先链查不到东西:agent_pid 必须是 None,
        // 绝不能拿载荷自报值填空。
        assert_eq!(events[0].agent_pid, None);
        // pane 路由字段来自信封头,不受身份注入影响。
        assert_eq!(events[0].pane, Some(3));
    }

    #[test]
    fn oversized_messages_are_dropped_without_parsing() {
        let mut buf = envelope(r#"{"hook_event_name":"Stop","session_id":"s"}"#);
        buf.resize((1 << 20) + 1, b' ');
        let outcome = dispatch(&buf, Some(4242), &|_| panic!("超限载荷绝不投递"));
        assert_eq!(outcome, Dispatch::Oversized);
    }

    #[test]
    fn unparsable_envelopes_never_reach_the_sink() {
        // 坏协议头(两侧字面量互钉:helper 侧 envelope 测试钉同一头)。
        let bad_header = b"slterm-hook/9 source=claude pane=3\n{\"hook_event_name\":\"Stop\"}";
        assert_eq!(
            dispatch(bad_header, None, &|_| panic!()),
            Dispatch::Unparsed
        );
        // 串台门:别家 runner 的 camelCase 载荷即便闸门失效也在这里被拦。
        let foreign = envelope(r#"{"hookEventName":"Stop","sessionId":"foreign","cwd":"D:/x"}"#);
        assert_eq!(dispatch(&foreign, None, &|_| panic!()), Dispatch::Unparsed);
        // 内核身份缺失(None)不阻塞解析:client_pid 拿不到证据时宁可放行,
        // 也不能因一次竞态丢掉真实的完成通知。
        let good = envelope(r#"{"hook_event_name":"Stop","session_id":"s"}"#);
        let outcome = dispatch(&good, None, &|event| {
            assert_eq!(event.client_pid, None);
            true
        });
        assert_eq!(outcome, Dispatch::Delivered);
    }

    #[test]
    fn sink_closed_is_reported_so_the_server_shuts_down() {
        let good = envelope(r#"{"hook_event_name":"Stop","session_id":"s"}"#);
        assert_eq!(dispatch(&good, None, &|_| false), Dispatch::SinkClosed);
    }
}
