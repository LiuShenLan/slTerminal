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
use std::io::Write as _;
use std::sync::{Arc, Mutex};
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

/// 终端尺寸信息（Phase 1 前端 DTO 保留，后续使用）
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PtySizeDto {
    pub rows: u16,
    pub cols: u16,
}

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
/// spawn 全程握 SPAWN_LOCK 串行化（Windows ConPTY 并发 spawn 卡死）。
#[tauri::command]
pub fn pty_spawn(
    state: tauri::State<'_, AppState>,
    on_output: Channel<PtyEvent>,
    request: SpawnRequest,
) -> Result<String, AppError> {
    // 串行化 spawn（Windows ConPTY 关键）
    let _lock = state.pty.spawn_lock.lock().unwrap();

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
    })?;

    // take_writer 只能调一次（0.9.0 破坏性变更），Arc<Mutex> 共享
    let raw_writer = pair.master.take_writer()?;
    let writer: Arc<Mutex<Box<dyn std::io::Write + Send>>> = Arc::new(Mutex::new(raw_writer));

    // Windows: openpty() 后立即向 stdin 写 CPR \x1b[1;1R
    // 补偿 ConPTY VtIo::StartIfNeeded() DSR 握手
    #[cfg(windows)]
    {
        let mut w = writer.lock().unwrap();
        w.write_all(b"\x1b[1;1R")?;
        w.flush()?;
    }

    // spawn 子进程
    let child = pair.slave.spawn_command(cmd)?;

    // Windows: 将子进程放入 Job Object 防止孤儿进程
    #[cfg(windows)]
    {
        if let Some(pid) = child.process_id() {
            add_to_job_object(pid)?;
        }
    }

    // 克隆 reader，启动 reader 线程
    let reader = pair.master.try_clone_reader()?;

    let reader_handle = std::thread::spawn(move || {
        crate::pty::reader::reader_loop(reader, on_output);
    });

    // 保存会话
    let session = PtySession {
        master: Mutex::new(pair.master),
        child: Mutex::new(child),
        writer,
        reader_handle: Some(reader_handle),
    };

    state
        .pty
        .sessions
        .write()
        .unwrap()
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
    let sessions = state.pty.sessions.read().unwrap();
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
    let mut writer = session.writer.lock().unwrap();
    writer.write_all(&data)?;
    writer.flush()?;
    Ok(())
}

/// 调整 PTY 终端尺寸
#[tauri::command]
pub fn pty_resize(
    state: tauri::State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    let sessions = state.pty.sessions.read().unwrap();
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
    let master = session.master.lock().unwrap();
    master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    Ok(())
}

/// 销毁 PTY 会话 — 杀子进程 → 回收 reader 线程 → 从 PtyState 移除
#[tauri::command]
pub fn pty_kill(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), AppError> {
    let mut sessions = state.pty.sessions.write().unwrap();
    let mut session = sessions
        .remove(&session_id)
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;

    // 杀子进程
    let mut child = session.child.lock().unwrap();
    let _ = child.kill();

    // 等待 reader 线程退出
    if let Some(handle) = session.reader_handle.take() {
        let _ = handle.join();
    }

    Ok(())
}

/// Windows Job Object — 将子进程与父进程生命周期绑定，防止孤儿进程
#[cfg(windows)]
fn add_to_job_object(pid: u32) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    use windows::core::PCWSTR;

    unsafe {
        // 创建 Job Object
        let job_name = format!("slTerminal_pty_{pid}");
        let job_name_wide: Vec<u16> = std::ffi::OsStr::new(&job_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let job = CreateJobObjectW(None, PCWSTR::from_raw(job_name_wide.as_ptr()))
            .map_err(|e| AppError::Pty(format!("CreateJobObject 失败: {e}")))?;

        // 设置 JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE（父进程退出时 OS 杀所有子进程）
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|e| AppError::Pty(format!("SetInformationJobObject 失败: {e}")))?;

        // 打开子进程句柄并分配到 Job Object
        let process = OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE,
            false,
            pid,
        )
        .map_err(|e| AppError::Pty(format!("OpenProcess 失败: {e}")))?;

        AssignProcessToJobObject(job, process)
            .map_err(|e| AppError::Pty(format!("AssignProcessToJobObject 失败: {e}")))?;
    }
    Ok(())
}
