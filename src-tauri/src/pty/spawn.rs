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
#[cfg(not(windows))]
use portable_pty::native_pty_system;
use portable_pty::PtySize;
use std::collections::VecDeque;
use std::io::Write as _;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tauri::ipc::Channel;
use uuid::Uuid;

// ─── ConPTY flag 常量（绕过 portable-pty 直接调 Win32 API）───
//
// portable-pty 0.9.0 硬编码 flags=0x7（INHERIT_CURSOR|RESIZE_QUIRK|WIN32_INPUT_MODE），
// 不暴露 CreatePseudoConsole dwFlags 参数。此处绕过 openpty()，直接调用 windows crate
// 的 CreatePseudoConsole，按 OS build 动态决定是否启用 PASSTHROUGH_MODE (0x8)。

#[cfg(windows)]
pub mod conpty_custom {
    use anyhow::{bail, ensure, Error};
    use filedescriptor::{FileDescriptor, Pipe};
    use portable_pty::{Child, ChildKiller, MasterPty, PtySize};
    use std::io::{Read, Write};
    use std::mem;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::{Arc, Mutex};
    use windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, HPCON, COORD,
        PSEUDOCONSOLE_INHERIT_CURSOR,
    };
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
        PROCESS_CREATION_FLAGS, LPPROC_THREAD_ATTRIBUTE_LIST, STARTF_USESTDHANDLES,
        STARTUPINFOEXW,
    };
    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::core::{PCWSTR, PWSTR};

    // ─── flag 常量（windows crate 仅定义 PSEUDOCONSOLE_INHERIT_CURSOR）───
    const FLAG_RESIZE_QUIRK: u32 = 0x2;
    const FLAG_WIN32_INPUT_MODE: u32 = 0x4;
    const FLAG_PASSTHROUGH_MODE: u32 = 0x8;

    const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x00000400;

    /// 计算 ConPTY flags：基础 0x7，Win11 22H2+ (build>=22621) 追加 PASSTHROUGH_MODE
    pub fn compute_conpty_flags(build_number: u32) -> u32 {
        let mut flags =
            PSEUDOCONSOLE_INHERIT_CURSOR | FLAG_RESIZE_QUIRK | FLAG_WIN32_INPUT_MODE;
        if build_number >= 22621 {
            flags |= FLAG_PASSTHROUGH_MODE;
        }
        flags
    }

    /// 将 UTF-8 字符串编码为以 null 结尾的 UTF-16LE 向量
    fn to_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 构建 CreateProcessW 命令行（程序路径 + 参数的单行字符串）
    fn build_cmdline(program: &str, args: &[String]) -> Vec<u16> {
        let mut s = String::new();
        // 程序路径含空格时加引号
        let quote = program.contains(' ');
        if quote { s.push('"'); }
        s.push_str(program);
        if quote { s.push('"'); }
        for arg in args {
            s.push(' ');
            if arg.contains(' ') || arg.contains('\t') {
                s.push('"');
                s.push_str(arg);
                s.push('"');
            } else {
                s.push_str(arg);
            }
        }
        to_wide_null(&s)
    }

    /// 构造环境块（继承当前进程环境 + 追加/覆盖 extra_envs）
    fn build_env_block(extra_envs: &[(String, String)]) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        let mut base: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        // 读取当前进程环境变量
        for (k, v) in std::env::vars() {
            base.insert(k, v);
        }
        for (k, v) in extra_envs {
            base.insert(k.clone(), v.clone());
        }
        // 构造 null 分隔的宽字符串块：KEY=VALUE\0KEY=VALUE\0\0
        let mut buf: Vec<u16> = Vec::new();
        for (k, v) in &base {
            let entry = format!("{}={}", k, v);
            buf.extend(std::ffi::OsStr::new(&entry).encode_wide());
            buf.push(0);
        }
        buf.push(0);
        buf
    }

    /// 轻量 STARTUPINFOEXW 属性列表 wrapper
    struct AttrList {
        data: Vec<u8>,
    }

    impl AttrList {
        fn with_capacity(num_attributes: u32) -> Result<Self, Error> {
            let mut bytes_required: usize = 0;
            unsafe {
                let _ = InitializeProcThreadAttributeList(None, num_attributes, Some(0), &mut bytes_required);
            };
            let mut data = vec![0u8; bytes_required];
            let res = unsafe {
                InitializeProcThreadAttributeList(
                    Some(LPPROC_THREAD_ATTRIBUTE_LIST(data.as_mut_ptr() as *mut _)),
                    num_attributes,
                    Some(0),
                    &mut bytes_required,
                )
            };
            if res.is_err() {
                bail!("InitializeProcThreadAttributeList 失败");
            }
            Ok(Self { data })
        }

        fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
            LPPROC_THREAD_ATTRIBUTE_LIST(self.data.as_mut_ptr() as *mut _)
        }

        /// 将 HPCON 附加到属性列表，使子进程连接到此伪控制台
        fn set_pty(&mut self, hpc: HPCON) -> Result<(), Error> {
            let res = unsafe {
                UpdateProcThreadAttribute(
                    self.as_mut_ptr(),
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                    Some(hpc.0 as *const std::ffi::c_void), // HPCON 值（非指针）作为 lpvalue
                    mem::size_of::<HPCON>(),
                    None,
                    None,
                )
            };
            ensure!(
                res.is_ok(),
                "UpdateProcThreadAttribute 失败"
            );
            Ok(())
        }
    }

    impl Drop for AttrList {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.as_mut_ptr());
            }
        }
    }

    // ─── ConPtyMaster: 实现 portable_pty::MasterPty 的自定义类型 ───

    struct ConPtyInner {
        hpc: HPCON,
        readable: FileDescriptor,
        writable: Option<FileDescriptor>,
        size: PtySize,
    }

    /// 自定义 MasterPty 实现，持有直接通过 Win32 API 创建的 HPCON
    pub struct ConPtyMaster {
        inner: Arc<Mutex<ConPtyInner>>,
    }

    // Downcast trait（便携式 pty::MasterPty 要求）由 mopa blanket impl 自动实现，无需手动

    impl MasterPty for ConPtyMaster {
        fn resize(&self, size: PtySize) -> Result<(), Error> {
            let mut inner = self.inner.lock().unwrap();
            // 检查 HPCON 有效性（初始化或已关闭时为 INVALID_HANDLE_VALUE）
            if inner.hpc.is_invalid() {
                inner.size = size;
                return Ok(());
            }
            let coord = COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            };
            unsafe { ResizePseudoConsole(inner.hpc, coord)? };
            inner.size = size;
            Ok(())
        }

        fn get_size(&self) -> Result<PtySize, Error> {
            Ok(self.inner.lock().unwrap().size)
        }

        fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, Error> {
            Ok(Box::new(self.inner.lock().unwrap().readable.try_clone()?))
        }

        fn take_writer(&self) -> Result<Box<dyn Write + Send>, Error> {
            Ok(Box::new(
                self.inner
                    .lock()
                    .unwrap()
                    .writable
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("writer 已被取走（仅允许 take 一次）"))?,
            ))
        }
    }

    /// 子进程句柄 RAII wrapper
    pub struct RawChild {
        proc_handle: Mutex<OwnedHandle>,
        pid: u32,
    }

    impl std::fmt::Debug for RawChild {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RawChild").field("pid", &self.pid).finish()
        }
    }

    // Downcast trait（便携式 pty::Child 要求）由 mopa blanket impl 自动实现

    impl ChildKiller for RawChild {
        fn kill(&mut self) -> std::io::Result<()> {
            use windows::Win32::System::Threading::TerminateProcess;
            let proc = self.proc_handle.lock().unwrap();
            unsafe {
                TerminateProcess(HANDLE(proc.as_raw_handle()), 1)
                    .map_err(std::io::Error::other)?;
            }
            Ok(())
        }

        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            // ChildKiller 通过互斥锁共享进程句柄，无需克隆
            // 此方法仅用于 trait 兼容，不应被调用
            unimplemented!("RawChild 不支持 clone_killer")
        }
    }

    impl Child for RawChild {
        fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
            use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

            let proc = self.proc_handle.lock().unwrap();
            unsafe {
                let result = WaitForSingleObject(
                    HANDLE(proc.as_raw_handle()),
                    0,
                );
                // WAIT_TIMEOUT = 0x0000_0102, WAIT_OBJECT_0 = 0x0000_0000
                if result.0 == 0x0000_0102 {
                    return Ok(None);
                }
                let mut exit_code: u32 = 0;
                GetExitCodeProcess(HANDLE(proc.as_raw_handle()), &mut exit_code)
                    .map_err(std::io::Error::other)?;
                Ok(Some(portable_pty::ExitStatus::with_exit_code(exit_code)))
            }
        }

        fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
            use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
            const INFINITE: u32 = 0xFFFF_FFFF;

            let proc = self.proc_handle.lock().unwrap();
            unsafe {
                WaitForSingleObject(HANDLE(proc.as_raw_handle()), INFINITE);
                let mut exit_code: u32 = 0;
                GetExitCodeProcess(HANDLE(proc.as_raw_handle()), &mut exit_code)
                    .map_err(std::io::Error::other)?;
                Ok(portable_pty::ExitStatus::with_exit_code(exit_code))
            }
        }

        fn process_id(&self) -> Option<u32> {
            Some(self.pid)
        }

        fn as_raw_handle(&self) -> Option<std::os::windows::raw::HANDLE> {
            let proc = self.proc_handle.lock().unwrap();
            Some(proc.as_raw_handle() as std::os::windows::raw::HANDLE)
        }
    }

    impl Drop for ConPtyInner {
        fn drop(&mut self) {
            // ConPTY 关闭前确保 writer 已 drop（stops child stdin → EOF 传递）
            drop(self.writable.take());
            if !self.hpc.is_invalid() {
                unsafe { ClosePseudoConsole(self.hpc) };
            }
        }
    }

    /// 创建 ConPTY 管道对 + MasterPty 包装
    ///
    /// 返回 (HPCON, ConPtyMaster) 供后续 spawn 和 session 注册。
    /// HPCON 单独返回是因为 spawn_conpty_child 需要直接引用它
    /// （ProcThreadAttributeList::set_pty 需要 HPCON 值）。
    pub fn create_conpty_pair(
        cols: u16,
        rows: u16,
        build_number: u32,
    ) -> Result<(HPCON, ConPtyMaster), Error> {
        let stdin_pipe = Pipe::new()?;
        let stdout_pipe = Pipe::new()?;

        let flags = compute_conpty_flags(build_number);
        let size = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        let hpc = unsafe {
            CreatePseudoConsole(
                size,
                HANDLE(stdin_pipe.read.as_raw_handle()),
                HANDLE(stdout_pipe.write.as_raw_handle()),
                flags,
            )
        }?;

        let master = ConPtyMaster {
            inner: Arc::new(Mutex::new(ConPtyInner {
                hpc,
                readable: stdout_pipe.read,
                writable: Some(stdin_pipe.write),
                size: PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                },
            })),
        };

        Ok((hpc, master))
    }

    /// 使用自定义 HPCON 启动子进程
    ///
    /// shell_info: 从 shell::resolve_shell_info() 获取的 shell 程序信息
    /// extra_envs: 额外环境变量（COLORTERM, TERM, TERM_PROGRAM 等）
    /// cwd: 工作目录（可选）
    pub fn spawn_conpty_child(
        hpc: HPCON,
        shell_info: &super::super::shell::ShellInfo,
        extra_envs: &[(String, String)],
        cwd: Option<&str>,
    ) -> Result<RawChild, Error> {
        let app_name = to_wide_null(&shell_info.program);
        let mut cmd_line = build_cmdline(&shell_info.program, &shell_info.args);
        let env_block = build_env_block(extra_envs);

        let mut si: STARTUPINFOEXW = unsafe { mem::zeroed() };
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;

        let mut attrs = AttrList::with_capacity(1)?;
        attrs.set_pty(hpc)?;
        si.lpAttributeList = attrs.as_mut_ptr();

        let cwd_wide: Option<Vec<u16>> = cwd.map(|c| to_wide_null(&c.replace('/', "\\")));

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };
        let res = unsafe {
            CreateProcessW(
                PCWSTR::from_raw(app_name.as_ptr()),
                Some(PWSTR::from_raw(cmd_line.as_mut_ptr())),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT | PROCESS_CREATION_FLAGS(CREATE_UNICODE_ENVIRONMENT),
                Some(env_block.as_ptr() as *const std::ffi::c_void),
                cwd_wide
                    .as_ref()
                    .map_or(PCWSTR::null(), |c| PCWSTR::from_raw(c.as_ptr())),
                &si.StartupInfo,
                &mut pi,
            )
        };

        if res.is_err() {
            let err = std::io::Error::last_os_error();
            bail!(
                "CreateProcessW `{}` 失败: {}",
                shell_info.program,
                err
            );
        }

        // 关闭子进程的主线程句柄（不需要），进程句柄由 RawChild 持有
        unsafe {
            let _ = CloseHandle(HANDLE(pi.hThread.0));
        }

        Ok(RawChild {
            proc_handle: Mutex::new(unsafe { OwnedHandle::from_raw_handle(pi.hProcess.0 as _) }),
            pid: pi.dwProcessId,
        })
    }

    // ─── 测试 ───
    #[cfg(test)]
    mod tests {
        use super::*;

        // T1: compute_conpty_flags（4 条）
        #[test]
        fn flags_win10_returns_0x7() {
            assert_eq!(compute_conpty_flags(19041), 0x7);
        }

        #[test]
        fn flags_win11_21h2_returns_0x7() {
            assert_eq!(compute_conpty_flags(22000), 0x7);
        }

        #[test]
        fn flags_win11_22h2_returns_0xf() {
            assert_eq!(compute_conpty_flags(22621), 0xF);
        }

        #[test]
        fn flags_win11_24h2_returns_0xf() {
            assert_eq!(compute_conpty_flags(26100), 0xF);
        }

        // T3: 常量值验证（4 条）
        #[test]
        fn flag_inherit_cursor_is_0x1() {
            assert_eq!(PSEUDOCONSOLE_INHERIT_CURSOR, 0x1);
        }

        #[test]
        fn flag_resize_quirk_is_0x2() {
            assert_eq!(FLAG_RESIZE_QUIRK, 0x2);
        }

        #[test]
        fn flag_win32_input_mode_is_0x4() {
            assert_eq!(FLAG_WIN32_INPUT_MODE, 0x4);
        }

        #[test]
        fn flag_passthrough_mode_is_0x8() {
            assert_eq!(FLAG_PASSTHROUGH_MODE, 0x8);
        }

        // T2: ConPtyMaster MasterPty trait（4 条，依赖实际 Pipe 创建）
        #[test]
        fn master_get_size_initial() {
            let (_hpc, master) = create_conpty_pair(80, 24, 26100).unwrap();
            assert_eq!(master.get_size().unwrap().cols, 80);
            assert_eq!(master.get_size().unwrap().rows, 24);
        }

        #[test]
        fn master_take_writer_first_succeeds() {
            let (_hpc, master) = create_conpty_pair(80, 24, 26100).unwrap();
            let writer = master.take_writer();
            assert!(writer.is_ok());
        }

        #[test]
        fn master_take_writer_second_fails() {
            let (_hpc, master) = create_conpty_pair(80, 24, 26100).unwrap();
            assert!(master.take_writer().is_ok(), "第一次 take_writer 应成功");
            assert!(master.take_writer().is_err(), "第二次 take_writer 应失败");
        }

        #[test]
        fn master_try_clone_reader_succeeds() {
            let (_hpc, master) = create_conpty_pair(80, 24, 26100).unwrap();
            let reader = master.try_clone_reader();
            assert!(reader.is_ok());
        }

        // T4: ProcThreadAttributeList 生命周期（2 条）
        #[test]
        fn attr_list_create_and_drop() {
            let list = AttrList::with_capacity(1);
            assert!(list.is_ok());
            // Drop 时不应 panic
        }

        #[test]
        fn attr_list_as_mut_ptr_non_null() {
            let mut list = AttrList::with_capacity(1).unwrap();
            let ptr = list.as_mut_ptr().0;
            assert!(!ptr.is_null());
        }
    }
}

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

    // 解析 shell 程序（不依赖 portable-pty CommandBuilder，直接获取结构化信息）
    let shell_info = shell::resolve_shell_info(request.shell.as_deref());

    // 注入终端能力环境变量——Claude Code 依赖此宣告启用 True Color
    let extra_envs: Vec<(String, String)> = vec![
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM".into(), "xterm-256color".into()),
        ("TERM_PROGRAM".into(), "slTerminal".into()),
    ];

    // 创建 PTY 并获取 master +（Windows 独有）HPCON 用于子进程 spawn
    // Windows: 绕过 portable-pty openpty，直接调 Win32 CreatePseudoConsole 控制 flags
    #[cfg(windows)]
    let (conpty_hpc, conpty_master) = {
        let build = super::build::get_windows_build_number().unwrap_or(0);
        conpty_custom::create_conpty_pair(request.cols, request.rows, build)
            .map_err(|e| AppError::Pty(e.to_string()))?
    };
    #[cfg(windows)]
    let master: Box<dyn portable_pty::MasterPty + Send> = Box::new(conpty_master);

    /// 非 Windows: 使用 portable-pty 原生 openpty
    #[cfg(not(windows))]
    let (master, slave) = {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: request.rows,
            cols: request.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::Pty(e.to_string()))?;
        (pair.master, pair.slave)
    };

    // take_writer 只能调一次（0.9.0 破坏性变更），Arc<Mutex> 共享
    let raw_writer = master.take_writer()
        .map_err(|e| AppError::Pty(e.to_string()))?;
    let writer: Arc<Mutex<Box<dyn std::io::Write + Send>>> = Arc::new(Mutex::new(raw_writer));

    // Windows: spawn 前向 stdin 写 CPR \x1b[1;1R
    // 补偿 ConPTY VtIo::StartIfNeeded() DSR 握手。
    #[cfg(windows)]
    {
        let mut w = writer.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        w.write_all(b"\x1b[1;1R")?;
        w.flush()?;
    }

    // spawn 子进程
    #[cfg(windows)]
    let child: Box<dyn portable_pty::Child + Send> = Box::new(
        conpty_custom::spawn_conpty_child(
            conpty_hpc,
            &shell_info,
            &extra_envs,
            request.cwd.as_deref(),
        )
        .map_err(|e| AppError::Pty(e.to_string()))?
    );
    #[cfg(not(windows))]
    let child: Box<dyn portable_pty::Child + Send> = {
        let mut cmd = shell::resolve_shell(request.shell.as_deref());
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM", "xterm-256color");
        cmd.env("TERM_PROGRAM", "slTerminal");
        if let Some(cwd) = &request.cwd {
            cmd.cwd(cwd.replace('/', "\\"));
        }
        slave.spawn_command(cmd)
            .map_err(|e| AppError::Pty(e.to_string()))?
    };

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
    let reader = master.try_clone_reader()
        .map_err(|e| AppError::Pty(e.to_string()))?;
    let reader_channel = channel.clone();
    let reader_ring = output_ring.clone();
    let reader_child = child.clone();
    // P2-13: reader 线程通过此 Arc 回写真实退出码，同时也是 session 的 exit_code
    let exit_code_slot: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let reader_exit_code = exit_code_slot.clone();

    // DA1 注入防重复标志
    let da1_injected = Arc::new(AtomicBool::new(false));

    let writer_reader = writer.clone();
    let da1_injected_reader = da1_injected.clone();

    let reader_handle = std::thread::spawn(move || {
        crate::pty::reader::reader_loop(
            reader, reader_channel, reader_ring, reader_child, reader_exit_code,
            writer_reader, da1_injected_reader,
        );
    });

    // 保存会话
    let session = PtySession {
        master: Mutex::new(master),
        child,
        writer,
        reader_handle: Some(reader_handle),
        channel,
        output_ring,
        exit_code: exit_code_slot,
        da1_injected,
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
