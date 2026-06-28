/// PTY spawn 命令 — ConPTY spawn + SPAWN_LOCK 串行化 + CPR 响应 + Job Object 孤儿防护
///
/// Windows 关键坑：
/// - spawn 串行化：并发 spawn 卡死 ConPTY 输出管道 → SPAWN_LOCK
/// - cwd 反斜杠：传给 ConPTY 前规范化成 \
/// - CPR 响应：openpty() 后立即写 \x1b[1;1R 到 stdin
/// - stdin drop：Windows 绝对不能 drop stdin（立即杀子进程）
/// - 孤儿进程：每个子进程放入 Job Object，JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
use crate::error::AppError;
use crate::pty::shell;
use crate::state::{AppState, PtySession};
use portable_pty::{native_pty_system, PtySize};
use std::collections::VecDeque;
use std::io::Write as _;
use std::sync::{Arc, Mutex, RwLock};
use tauri::ipc::Channel;
use uuid::Uuid;

/// PTY 输出事件 — 通过 Channel 推送到前端
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum PtyEvent {
    /// 终端输出数据（原始字节）
    Output { bytes: Vec<u8> },
    /// 子进程退出
    Exit { code: Option<i32> },
}

/// Windows Job Object 句柄 RAII 包装
///
/// 持有 `HANDLE` 以阻止 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 在 PTY 会话期间触发。
/// Drop 时调用 `CloseHandle` 释放句柄。
#[cfg(windows)]
pub struct JobHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl JobHandle {
    pub fn new(handle: windows::Win32::Foundation::HANDLE) -> Self {
        JobHandle(handle)
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        // SAFETY: CloseHandle 可从任意线程安全调用，即使句柄无效也仅返回 FALSE
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// SAFETY: HANDLE 在 Win32 中可跨线程传递；CloseHandle 可从任意线程安全调用
#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

/// spawn 参数
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    /// 前端生成的 panel ID
    pub panel_id: String,
    /// 终端列数
    pub cols: u16,
    /// 终端行数
    pub rows: u16,
    /// 工作目录（可选，默认用户主目录）
    pub cwd: Option<String>,
    /// shell 程序路径（可选，自动检测 pwsh→powershell→cmd）
    pub shell: Option<String>,
}

/// 创建 PTY 并启动 shell，返回 session_id
///
/// 输出通过 on_output Channel 持续推送到前端。
/// SPAWN_LOCK 仅保护 ConPTY 创建 + 子进程启动（Windows ConPTY 并发 spawn 卡死），
/// reader 线程启动和 sessions 插入在锁外执行。
#[tauri::command]
pub fn pty_spawn(
    state: tauri::State<'_, AppState>,
    on_output: Channel<PtyEvent>,
    request: SpawnRequest,
) -> Result<String, AppError> {
    // 串行化 spawn（仅保护 ConPTY 创建 + 子进程启动，并发 spawn 会卡死输出管道）
    let _lock = state.pty.spawn_lock.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;

    let session_id = Uuid::new_v4().to_string();

    // 选择 shell 程序（shell.rs 已配置好 CommandBuilder 参数）
    let mut cmd = shell::resolve_shell(request.shell.as_deref());

    // 设置工作目录，规范化反斜杠（Windows ConPTY 需要 \ 分隔符）
    if let Some(cwd) = &request.cwd {
        let normalized = cwd.replace('/', "\\");
        cmd.cwd(normalized);
    }

    // 传递 pane token：脚本初始化后用此值设置 SLTERM_PANE_ID，
    // 并在嵌套检测前检查 SLTERM_PANE_ID（不直接注入，避免误判跳过）

    // 创建 PTY
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: request.rows,
        cols: request.cols,
        pixel_width: 0,
        pixel_height: 0,
    })
    .map_err(|e| AppError::Pty(e.to_string()))?;

    // take_writer 只能调一次（0.9.0 破坏性变更），Arc<Mutex> 共享
    let raw_writer = pair.master.take_writer()
        .map_err(|e| AppError::Pty(e.to_string()))?;
    let writer: Arc<Mutex<Box<dyn std::io::Write + Send>>> = Arc::new(Mutex::new(raw_writer));

    // Windows: spawn 前向 stdin 写 CPR \x1b[1;1R
    // 补偿 ConPTY VtIo::StartIfNeeded() DSR 握手。
    // 提前注入确保 ConPTY 首次读取 input pipe 时即获得 CPR 应答，避免 DSR 死锁。
    // 蜂鸣+首字符消失由 reader.rs 的 strip_conpty_startup() 处理——与此无关。
    #[cfg(windows)]
    {
        let mut w = writer.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        w.write_all(b"\x1b[1;1R")?;
        w.flush()?;
    }

    // spawn 子进程
    let child = pair.slave.spawn_command(cmd)
        .map_err(|e| AppError::Pty(e.to_string()))?;

    // Windows: 将子进程放入 Job Object 防止孤儿进程
    #[cfg(windows)]
    let job_handle = {
        if let Some(pid) = child.process_id() {
            Some(add_to_job_object(pid)?)
        } else {
            None
        }
    };

    // SPAWN_LOCK 临界区结束：ConPTY 已创建完毕，后续操作（reader 线程启动、sessions 插入）无需串行化
    drop(_lock);

    // E1: 创建可替换 Channel 和 ring buffer
    let channel: Arc<RwLock<Option<Channel<PtyEvent>>>> =
        Arc::new(RwLock::new(Some(on_output)));
    let output_ring: Arc<Mutex<VecDeque<u8>>> = Arc::new(Mutex::new(VecDeque::new()));

    // P2-11: child 包装为 Arc<Mutex<>>，reader 线程通过 clone 获取真实退出码
    let child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>> =
        Arc::new(Mutex::new(child));

    // 克隆 reader，启动 reader 线程（reader.rs 首轮读取剥离 ConPTY 启动注入序列）
    let reader = pair.master.try_clone_reader()
        .map_err(|e| AppError::Pty(e.to_string()))?;
    let reader_channel = channel.clone();
    let reader_ring = output_ring.clone();
    let reader_child = child.clone();
    // P2-13: reader 线程通过此 Arc 回写真实退出码，同时也是 session 的 exit_code
    let exit_code_slot: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let reader_exit_code = exit_code_slot.clone();

    let reader_handle = std::thread::spawn(move || {
        crate::pty::reader::reader_loop(reader, reader_channel, reader_ring, reader_child, reader_exit_code);
    });

    // 保存会话
    let session = PtySession {
        master: Mutex::new(pair.master),
        child,
        writer,
        reader_handle: Some(reader_handle),
        channel,
        output_ring,
        exit_code: exit_code_slot,
        #[cfg(windows)]
        job_object: job_handle,
    };

    state
        .pty
        .sessions
        .write()
        .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?
        .insert(session_id.clone(), session);

    Ok(session_id)
}

/// 向 PTY 写入数据（来自前端键盘输入）
#[tauri::command]
pub fn pty_write(
    state: tauri::State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), AppError> {
    let sessions = state.pty.sessions.read().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
    let mut writer = session.writer.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    writer.write_all(&data)?;
    writer.flush()?;
    Ok(())
}

/// 调整 PTY 终端尺寸
///
/// 锁嵌套：sessions.read() → session.master.lock()。
/// 当前安全：外层是共享读锁（RwLock read），内层是独立 Mutex，无其他代码路径反向获取。
/// 未来重构：若新增 sessions.write() → master.lock() 或 master.lock() → sessions.write() 路径，
/// 需重新评估死锁风险，考虑将 resize 改为先 clone Arc 再操作。
#[tauri::command]
pub fn pty_resize(
    state: tauri::State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    let sessions = state.pty.sessions.read().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
    let master = session.master.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
    .map_err(|e| AppError::Pty(e.to_string()))?;
    Ok(())
}

/// 销毁 PTY 会话 — 杀子进程 → 回收 reader 线程 → 从 PtyState 移除
///
/// G1b: async + spawn_blocking。先提取 session 后释放 RwLock 写锁，
/// 再在 spawn_blocking 中执行 kill+join+drop（ClosePseudoConsole 在 pre-Win11 24H2 上永久阻塞），
/// 避免持锁阻塞导致后续命令级联卡死。
#[tauri::command]
pub async fn pty_kill(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), AppError> {
    // 提取 session 后释放写锁（锁在此 scope 结束时释放，<1ms）
    let session = {
        let mut sessions = state.pty.sessions.write().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        sessions
            .remove(&session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?
    };

    // blocking 线程中执行 kill+join+drop，不阻塞 IPC worker
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let mut session = session;
        let mut child = session.child.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        let _ = child.kill();
        drop(child);
        if let Some(handle) = session.reader_handle.take() {
            let _ = handle.join();
        }
        // session drop → master drop → ClosePseudoConsole
        Ok(())
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_kill join error: {e}")))?
}

/// PTY 重连 — 替换 Channel、回放并清空 ring buffer、检测退出状态
///
/// 前端页面切换后调用此命令将新 Channel 绑定到已有 PTY session。
/// P2-45: 回放后 drain 清空 ring buffer，避免重复数据堆积。
/// P2-42: 检测子进程退出码，若已退出则发送 Exit 事件。
#[tauri::command]
pub fn pty_reattach(
    state: tauri::State<'_, AppState>,
    session_id: String,
    on_output: Channel<PtyEvent>,
) -> Result<(), AppError> {
    let sessions = state.pty.sessions.read().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;

    // 1. 替换 Channel
    let mut ch = session.channel.write().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
    *ch = Some(on_output);

    // 2. 回放 ring buffer 并清空（P2-45: drain 同时清空，无需额外 clear）
    {
        let mut ring = session.output_ring.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        let data: Vec<u8> = ring.drain(..).collect();
        drop(ring); // 释放锁再 send
        if !data.is_empty() {
            if let Some(ref c) = *ch {
                let _ = c.send(PtyEvent::Output { bytes: data });
            }
        }
    }

    // 3. 检查退出状态（P2-42: reader 线程可能已记录退出码）
    {
        let code = *session.exit_code.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        if let Some(code) = code {
            if let Some(ref c) = *ch {
                let _ = c.send(PtyEvent::Exit { code: Some(code) });
            }
        }
    }

    Ok(())
}

/// Windows Job Object — 将子进程与父进程生命周期绑定，防止孤儿进程
///
/// 构建 Job Object 名称字符串并委托给 `create_and_assign_job` 执行 Win32 调用。
/// 返回 `JobHandle` 以在整个 PTY 会话期间持有 job handle，防止 `KILL_ON_JOB_CLOSE` 过早触发。
#[cfg(windows)]
fn add_to_job_object(pid: u32) -> Result<JobHandle, AppError> {
    use std::os::windows::ffi::OsStrExt;

    let job_name = format!("slTerminal_pty_{pid}");
    let job_name_wide: Vec<u16> = std::ffi::OsStr::new(&job_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY:
    // - `pid` 来自 `portable_pty::Child::process_id()`，是刚创建的有效子进程 ID
    // - `job_name_wide` 是本地构建的以 null 结尾的宽字符串，指针在调用期间有效
    // - 该调用紧跟在 `slave.spawn_command()` 之后，在任何可能触发 panic 的 `?` 之前完成
    // - 返回的 JobHandle 在 PtySession 存活期间持有 job handle，由 RAII Drop 负责 CloseHandle
    unsafe { create_and_assign_job(pid, &job_name_wide) }
}

/// 创建 Job Object 并设置 KILL_ON_JOB_CLOSE，将子进程分配进去
///
/// # Safety
///
/// 调用者必须保证：
///
/// - `pid` 是有效的子进程 ID。该函数在子进程创建后立即调用，进程 ID 必须仍然有效。
/// - `job_name_wide` 是一个有效的以 null 结尾的 UTF-16 宽字符串指针，函数调用期间其内存不会被释放或移动。
/// - 该函数必须在 `pty_spawn` 中、`slave.spawn_command()` 成功之后且在 `?` 传播语义可能提前返回（导致 panic 或错误传播）之前调用，
///   以确保 Win32 句柄在 `?` 传播栈展开时由 RAII (JobHandle Drop) 正确释放。
///
/// 返回的 `JobHandle` 必须在 PTY 会话存活期间持有，以防 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
/// 过早触发。`JobHandle::drop` 会调用 `CloseHandle` 释放句柄。
#[cfg(windows)]
unsafe fn create_and_assign_job(pid: u32, job_name_wide: &[u16]) -> Result<JobHandle, AppError> {
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    use windows::Win32::Foundation::CloseHandle;
    use windows::core::PCWSTR;

    // 创建 Job Object
    let job = CreateJobObjectW(None, PCWSTR::from_raw(job_name_wide.as_ptr()))
        .map_err(|e| AppError::Pty(format!("CreateJobObject failed: {e}")))?;

    // 设置 JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE（父进程退出时 OS 杀所有子进程）
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &limits as *const _ as *const _,
        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
    )
    .map_err(|e| AppError::Pty(format!("SetInformationJobObject failed: {e}")))?;

    // 打开子进程句柄并分配到 Job Object
    let process = OpenProcess(
        PROCESS_SET_QUOTA | PROCESS_TERMINATE,
        false,
        pid,
    )
    .map_err(|e| AppError::Pty(format!("OpenProcess failed: {e}")))?;

    AssignProcessToJobObject(job, process)
        .map_err(|e| AppError::Pty(format!("AssignProcessToJobObject failed: {e}")))?;

    // process 句柄仅用于 AssignProcessToJobObject，完成后立即释放
    let _ = CloseHandle(process);

    Ok(JobHandle::new(job))
}
