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
use crate::state::{self as app_state, AppState, PtySession, PtyState};
#[cfg(not(windows))]
use portable_pty::native_pty_system;
use portable_pty::PtySize;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::ipc::Channel;
use uuid::Uuid;

/// BE-01: PTY 会话总数上限——防止会话无上限堆积耗尽 ConPTY/进程句柄
const MAX_PTY_SESSIONS: usize = 32;

// ─── ConPTY flag 常量（绕过 portable-pty 直接调 Win32 API）───
//
// portable-pty 0.9.0 硬编码 flags=0x7（INHERIT_CURSOR|RESIZE_QUIRK|WIN32_INPUT_MODE），
// 不暴露 CreatePseudoConsole dwFlags 参数。此处绕过 openpty()，直接调用 windows crate
// 的 CreatePseudoConsole，完全控制 flags（当前固定 0x7，见 compute_conpty_flags 注释）。

#[cfg(windows)]
pub mod conpty_custom {
    use crate::pty::conpty_api::{resolve_conpty_api, CONPTY_WIN11_MIN_BUILD};
    use anyhow::{bail, ensure, Error};
    use filedescriptor::{FileDescriptor, Pipe};
    use portable_pty::{Child, ChildKiller, MasterPty, PtySize};
    use std::io::{Read, Write};
    use std::mem;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::{Arc, Mutex};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::System::Console::{COORD, HPCON, PSEUDOCONSOLE_INHERIT_CURSOR};
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };

    // ─── flag 常量（windows crate 仅定义 PSEUDOCONSOLE_INHERIT_CURSOR）───
    const FLAG_RESIZE_QUIRK: u32 = 0x2;
    const FLAG_WIN32_INPUT_MODE: u32 = 0x4;

    const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x00000400;

    /// 计算 ConPTY flags（三态）：
    ///
    /// - **捆绑新 conhost**（仅 Win10 尝试，见 conpty_api ADR-0005）：恒 0x7——
    ///   新版完整支持 0x4（修复 microsoft/terminal#376 的 PR #4856 即在新版）
    /// - 系统 conhost + Win11（build >= 21376）：0x7
    /// - 系统 conhost + Win10（build < 21376）：0x3——**回退路径**。0x3 未修复滚轮
    ///   （0x3/0x7 均实测失效，根因是老 conhost 不转发鼠标 VT 序列，#376），仅因
    ///   键盘/IME 已实测正常而保留，防回退场景无谓启用 0x4。
    ///
    /// 阈值 21376 与前端 xterm 钳制（ADR-0004，XTERM_CONPTY_MIN_BUILD）同源——
    /// 同为 xterm.js 的 ConPTY 兼容分界，Win10/Win11 分叉共用。
    ///
    /// **勿启用 PASSTHROUGH_MODE (0x8)**：0x8 下 claude 等全屏 TUI（v2.1.89+ 默认
    /// alt buffer + mouse tracking）的鼠标滚轮完全失效。2026-07 在 Win11 build 26200
    /// 真实 app 双向实测：0xF 时 xterm 的 SGR wheel report（`\x1b[<64/65;x;yM`）完整写入
    /// ConPTY stdin 但 claude 无反应，去掉 0x8 后滚轮恢复，输出流畅度无肉眼可见退化。
    /// 疑似机制：passthrough 下 conhost 不解析子进程输出、不跟踪 DECSET 1000/1002/1006
    /// mouse mode（microsoft/terminal#376、PR #9970）——但**最小复现实验失败**：node 直接
    /// 子进程（DECSET 1002/1003/1006 + alt buffer + 60fps 负载）在 0xF 下 stdin 的 SGR
    /// report 仍原样透传，阻断条件仅真实 claude 场景（pwsh→claude 进程树 + kitty 协议）
    /// 复现。因此**验证本函数改动必须实测真实 claude 滚轮**，勿以最小实验/单测绿为依据。
    pub fn compute_conpty_flags(build_number: u32, bundled: bool) -> u32 {
        let base = PSEUDOCONSOLE_INHERIT_CURSOR | FLAG_RESIZE_QUIRK;
        if bundled || build_number >= CONPTY_WIN11_MIN_BUILD {
            base | FLAG_WIN32_INPUT_MODE
        } else {
            base // 系统老 conhost 回退：去 WIN32_INPUT_MODE
        }
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
        if quote {
            s.push('"');
        }
        s.push_str(program);
        if quote {
            s.push('"');
        }
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

    /// 构造环境块（继承当前进程环境 + 追加/覆盖 extra_envs）。
    /// 用 Vec 保持插入顺序，extra_envs 覆盖同名字段——确保环境块确定性。
    fn build_env_block(extra_envs: &[(String, String)]) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        // 用 Vec 保持确定性顺序（HashMap 迭代顺序非确定性）
        let mut env: Vec<(String, String)> = std::env::vars().collect();
        for (k, v) in extra_envs {
            // 覆盖已有键或追加到末尾
            if let Some(existing) = env.iter_mut().find(|(ek, _)| ek == k) {
                existing.1 = v.clone();
            } else {
                env.push((k.clone(), v.clone()));
            }
        }
        // 构造 null 分隔的宽字符串块：KEY=VALUE\0KEY=VALUE\0\0
        let mut buf: Vec<u16> = Vec::new();
        for (k, v) in &env {
            let entry = format!("{}={}", k, v);
            buf.extend(std::ffi::OsStr::new(&entry).encode_wide());
            buf.push(0);
        }
        buf.push(0);
        buf
    }

    /// 构造 CreateProcessW 的 cwd 宽字符串（纯函数）
    /// Windows 坑：CreateProcessW 对 `/` 行为异常——先将 `/` 规范化成 `\` 再编码 UTF-16LE
    fn build_cwd_wide(cwd: &str) -> Vec<u16> {
        to_wide_null(&cwd.replace('/', "\\"))
    }

    /// 轻量 STARTUPINFOEXW 属性列表 wrapper
    struct AttrList {
        data: Vec<u8>,
    }

    impl AttrList {
        fn with_capacity(num_attributes: u32) -> Result<Self, Error> {
            let mut bytes_required: usize = 0;
            unsafe {
                let _ = InitializeProcThreadAttributeList(
                    None,
                    num_attributes,
                    Some(0),
                    &mut bytes_required,
                );
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
            ensure!(res.is_ok(), "UpdateProcThreadAttribute 失败");
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
        /// 进程级 ConPTY API 单例（系统或捆绑，见 conpty_api）
        api: &'static crate::pty::conpty_api::ConptyApi,
    }

    /// 自定义 MasterPty 实现，持有直接通过 Win32 API 创建的 HPCON
    pub struct ConPtyMaster {
        inner: Arc<Mutex<ConPtyInner>>,
    }

    // Downcast trait（便携式 pty::MasterPty 要求）由 mopa blanket impl 自动实现，无需手动

    impl MasterPty for ConPtyMaster {
        fn resize(&self, size: PtySize) -> Result<(), Error> {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| anyhow::anyhow!("ConPtyInner lock poisoned: {e}"))?;
            // 检查 HPCON 有效性（初始化或已关闭时为 INVALID_HANDLE_VALUE）
            if inner.hpc.is_invalid() {
                inner.size = size;
                return Ok(());
            }
            let coord = COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            };
            // SAFETY: ResizePseudoConsole 是 Win32 ConPTY API；hpc 由 create 创建，coord 基于已验证的 PtySize
            unsafe { inner.api.resize(inner.hpc, coord)? };
            inner.size = size;
            Ok(())
        }

        fn get_size(&self) -> Result<PtySize, Error> {
            Ok(self
                .inner
                .lock()
                .map_err(|e| anyhow::anyhow!("ConPtyInner lock poisoned: {e}"))?
                .size)
        }

        fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, Error> {
            Ok(Box::new(
                self.inner
                    .lock()
                    .map_err(|e| anyhow::anyhow!("ConPtyInner lock poisoned: {e}"))?
                    .readable
                    .try_clone()?,
            ))
        }

        fn take_writer(&self) -> Result<Box<dyn Write + Send>, Error> {
            Ok(Box::new(
                self.inner
                    .lock()
                    .map_err(|e| anyhow::anyhow!("ConPtyInner lock poisoned: {e}"))?
                    .writable
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("writer 已被取走（仅允许 take 一次）"))?,
            ))
        }
    }

    /// raw 句柄的 Send 包装——仅用于 PeekNamedPipe 非阻塞查询（BE-05 微批续读）。
    /// 句柄所有权由 reader（FileDescriptor）持有，本包装只引用同一内核对象，
    /// 不负责关闭；跨线程仅查询不释放，故 Send 安全。
    struct SendRawHandle(std::os::windows::io::RawHandle);
    // SAFETY: 句柄生命周期由 reader 端 FileDescriptor 保证（克隆句柄引用同一
    // 内核对象），本方法仅做 PeekNamedPipe 查询（同步非阻塞），无所有权、
    // 无关闭语义——跨线程传递安全。
    unsafe impl Send for SendRawHandle {}
    unsafe impl Sync for SendRawHandle {}

    impl SendRawHandle {
        /// 非阻塞查询管道可读字节数（微批续读决策用）
        ///
        /// 用 PeekNamedPipe（同步查询，与后续 read 同一管道状态视角，零竞态）
        /// 替代旧 WaitForSingleObject 信号检查——匿名管道读端的信号在数据
        /// 被读走后存在 reset 延迟（经典竞态）：误报「有数据」→ 微批续读的
        /// 阻塞 read 空等 → reader 线程卡死 → 该终端永久无输出（E2E 全部终端
        /// 文本为空 + win10 黑屏根因）。Peek 返回当前可读字节数，为 0 即不续读，
        /// 不存在信号时序窗口。对端关闭时 Peek 返回 0，由后续 read Ok(0) EOF 兜底。
        fn pending_bytes(&self) -> u32 {
            use windows::Win32::System::Pipes::PeekNamedPipe;
            let mut avail: u32 = 0;
            unsafe {
                // PeekNamedPipe 失败（句柄无效等异常）→ 视为无数据（0），
                // 不续读——由主循环 read 走 Err/EOF 分支兜底，不卡死
                let _ = PeekNamedPipe(HANDLE(self.0), None, 0, None, Some(&mut avail), None);
            }
            avail
        }
    }

    /// BE-05: 克隆 ConPTY 输出读端并构造微批续读检查器（PtyReaderInput）
    ///
    /// - reader: 输出管道读端（阻塞 read，供 reader_loop 主循环与微批续读）
    /// - pending: 非阻塞「管道是否有可读数据」检测——PeekNamedPipe 查询当前可读
    ///   字节数，> 0 才有数据（对端关闭时 Peek 返回 0，由后续阻塞 read 返回
    ///   Ok(0) EOF 兜底）。供 reader_loop 微批续读决策（BE-05：
    ///   「读到即续读」非定时器，无数据时不空等）。
    ///
    /// 须在 conpty_master 装箱（Box<dyn MasterPty>）前调用——内部字段对上层不可见。
    pub fn clone_reader_with_pending_check(
        master: &ConPtyMaster,
    ) -> Result<crate::pty::reader::PtyReaderInput, Error> {
        let inner = master
            .inner
            .lock()
            .map_err(|e| anyhow::anyhow!("ConPtyInner lock poisoned: {e}"))?;
        let read_end = inner.readable.try_clone()?;
        // raw 指针需 Send 包装才能跨线程（HANDLE 未实现 Send）；经方法调用捕获
        // 整个包装（路径捕获 handle.0 会退化为捕获 raw 指针本身）
        let handle = SendRawHandle(read_end.as_raw_handle());
        let pending: Box<dyn Fn() -> bool + Send> = Box::new(move || handle.pending_bytes() > 0);
        Ok(crate::pty::reader::PtyReaderInput::new(
            Box::new(read_end),
            pending,
        ))
    }

    /// 子进程句柄 RAII wrapper。
    /// proc_handle 用 Arc 共享，支持 clone_killer 无锁复制。
    pub struct RawChild {
        proc_handle: Arc<Mutex<OwnedHandle>>,
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
            let proc = self.proc_handle.lock().map_err(|e| {
                std::io::Error::other(format!("RawChild proc_handle lock poisoned: {e}"))
            })?;
            // SAFETY: TerminateProcess 是 Win32 API；HANDLE 来自 CreateProcessW 创建的有效子进程句柄
            unsafe {
                TerminateProcess(HANDLE(proc.as_raw_handle()), 1).map_err(std::io::Error::other)?;
            }
            Ok(())
        }

        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            // 通过 Arc 共享进程句柄，无需克隆 HANDLE 或加锁
            // 此方法仅用于 trait 兼容
            Box::new(RawChild {
                proc_handle: Arc::clone(&self.proc_handle),
                pid: self.pid,
            })
        }
    }

    impl Child for RawChild {
        fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
            use windows::Win32::Foundation::WAIT_OBJECT_0;
            use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

            let proc = self.proc_handle.lock().map_err(|e| {
                std::io::Error::other(format!("RawChild proc_handle lock poisoned: {e}"))
            })?;
            // SAFETY: WaitForSingleObject 和 GetExitCodeProcess 是 Win32 API；HANDLE 来自 CreateProcessW 创建的有效子进程句柄
            unsafe {
                let result = WaitForSingleObject(HANDLE(proc.as_raw_handle()), 0);
                // WAIT_TIMEOUT = 0x0000_0102: 子进程仍在运行
                if result.0 == 0x0000_0102 {
                    return Ok(None);
                }
                // WAIT_OBJECT_0 = 0x0000_0000: 子进程已退出
                if result != WAIT_OBJECT_0 {
                    return Err(std::io::Error::other(format!(
                        "WaitForSingleObject 失败: 返回值 0x{:08X}",
                        result.0
                    )));
                }
                let mut exit_code: u32 = 0;
                GetExitCodeProcess(HANDLE(proc.as_raw_handle()), &mut exit_code)
                    .map_err(std::io::Error::other)?;
                Ok(Some(portable_pty::ExitStatus::with_exit_code(exit_code)))
            }
        }

        fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
            use windows::Win32::Foundation::WAIT_OBJECT_0;
            use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
            const INFINITE: u32 = 0xFFFF_FFFF;

            let proc = self.proc_handle.lock().map_err(|e| {
                std::io::Error::other(format!("RawChild proc_handle lock poisoned: {e}"))
            })?;
            // SAFETY: WaitForSingleObject 和 GetExitCodeProcess 是 Win32 API；HANDLE 来自 CreateProcessW 创建的有效子进程句柄
            unsafe {
                let result = WaitForSingleObject(HANDLE(proc.as_raw_handle()), INFINITE);
                if result != WAIT_OBJECT_0 {
                    return Err(std::io::Error::other(format!(
                        "WaitForSingleObject 失败: 返回值 0x{:08X}",
                        result.0
                    )));
                }
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
            // 锁中毒时返回 None（句柄已不可靠）
            let proc = self.proc_handle.lock().ok()?;
            Some(proc.as_raw_handle() as std::os::windows::raw::HANDLE)
        }
    }

    impl Drop for ConPtyInner {
        fn drop(&mut self) {
            // ConPTY 关闭前确保 writer 已 drop（stops child stdin → EOF 传递）
            drop(self.writable.take());
            if !self.hpc.is_invalid() {
                // SAFETY: ClosePseudoConsole 是 Win32 ConPTY 清理 API；hpc 由 create 创建，仅调用一次
                unsafe { self.api.close(self.hpc) };
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

        // Win10 尝试捆绑新 conhost（ADR-0005），失败静默回退系统；Win11 恒系统
        let api = resolve_conpty_api(build_number);
        let flags = compute_conpty_flags(build_number, api.is_bundled());
        let size = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        // SAFETY: CreatePseudoConsole 是 Win32 ConPTY 创建 API；管道句柄来自 filedescriptor::Pipe，
        // 在其生命周期内有效；api 为进程级单例引用
        let hpc = unsafe {
            api.create(
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
                api,
            })),
        };

        Ok((hpc, master))
    }

    /// 使用自定义 HPCON 启动子进程
    ///
    /// shell_info: 从 shell::resolve_shell_info() 获取的 shell 程序信息
    /// extra_envs: 额外环境变量（COLORTERM, TERM, TERM_PROGRAM 等）
    /// cwd: 工作目录（可选）
    ///
    /// 可纯化部分（命令行/环境块/cwd 宽字符串构造）已抽为独立纯函数
    /// （build_cmdline / build_env_block / build_cwd_wide），由单元测试覆盖；
    /// 纯 Win32 调用部分（AttrList::set_pty → CreateProcessW 组合）由
    /// pty_spawn_custom_conpty 集成测试（tests/pty_integration_tests.rs）+ CI 守卫（Windows runner）验证。
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

        let cwd_wide: Option<Vec<u16>> = cwd.map(build_cwd_wide);

        // SAFETY: CreateProcessW 是 Win32 进程创建 API；app_name/cmd_line/cwd/env_block 均在栈上保持存活；
        // lpAttributeList 由 AttrList 管理生命周期；pi 是未初始化的 PROCESS_INFORMATION 输出参数
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
            bail!("CreateProcessW `{}` 失败: {}", shell_info.program, err);
        }

        // 关闭子进程的主线程句柄（不需要），进程句柄由 RawChild 持有
        unsafe {
            let _ = CloseHandle(HANDLE(pi.hThread.0));
        }

        Ok(RawChild {
            proc_handle: Arc::new(Mutex::new(unsafe {
                OwnedHandle::from_raw_handle(pi.hProcess.0 as _)
            })),
            pid: pi.dwProcessId,
        })
    }

    // ─── 测试 ───
    #[cfg(test)]
    mod conpty_custom_tests {
        use super::*;

        // T1: compute_conpty_flags（7 条）——三态：捆绑恒 0x7（新 conhost 完整支持
        // 0x4）；系统按 build 分叉（Win10 0x3 为回退路径——0x3/0x7 均实测滚轮失效，
        // 根因在老 conhost 不转发鼠标，见 conpty_api ADR-0005）；回归守卫：任何组合
        // 都不启用 PASSTHROUGH_MODE 0x8（passthrough 会吞 terminal→child 的 SGR mouse report）
        #[test]
        fn flags_win10_19041_system_returns_0x3() {
            assert_eq!(compute_conpty_flags(19041, false), 0x3);
        }

        #[test]
        fn flags_below_threshold_21375_system_returns_0x3() {
            assert_eq!(compute_conpty_flags(21375, false), 0x3);
        }

        #[test]
        fn flags_threshold_21376_system_returns_0x7() {
            assert_eq!(compute_conpty_flags(21376, false), 0x7);
        }

        #[test]
        fn flags_win11_21h2_system_returns_0x7() {
            assert_eq!(compute_conpty_flags(22000, false), 0x7);
        }

        #[test]
        fn flags_win11_22h2_system_returns_0x7() {
            assert_eq!(compute_conpty_flags(22621, false), 0x7);
        }

        #[test]
        fn flags_win11_24h2_system_returns_0x7() {
            assert_eq!(compute_conpty_flags(26100, false), 0x7);
        }

        #[test]
        fn flags_win10_bundled_returns_0x7() {
            assert_eq!(compute_conpty_flags(19041, true), 0x7);
        }

        // T3: 常量值验证（3 条）
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

        // ─── T5: build_env_block 环境变量注入测试 ───
        // 验证 pty_spawn 注入的三个终端能力环境变量正确编码到 ConPTY 环境块

        #[test]
        fn build_env_block_terminal_env_vars_included() {
            let extra = vec![
                ("COLORTERM".into(), "truecolor".into()),
                ("TERM".into(), "xterm-256color".into()),
                ("TERM_PROGRAM".into(), "slTerminal".into()),
            ];
            let block = build_env_block(&extra);
            let text = String::from_utf16_lossy(&block);
            assert!(
                text.contains("COLORTERM=truecolor"),
                "环境块应包含 COLORTERM=truecolor，实际内容: {text}"
            );
            assert!(
                text.contains("TERM=xterm-256color"),
                "环境块应包含 TERM=xterm-256color，实际内容: {text}"
            );
            assert!(
                text.contains("TERM_PROGRAM=slTerminal"),
                "环境块应包含 TERM_PROGRAM=slTerminal，实际内容: {text}"
            );
        }

        #[test]
        fn build_env_block_extra_overrides_existing() {
            // 取一个已知存在的 Windows 环境变量做覆盖测试
            let extra = vec![("COMPUTERNAME".into(), "TEST_OVERRIDE_VALUE".into())];
            let block = build_env_block(&extra);
            let text = String::from_utf16_lossy(&block);
            assert!(
                text.contains("COMPUTERNAME=TEST_OVERRIDE_VALUE"),
                "extra_env 应覆盖已有环境变量 COMPUTERNAME，实际内容: {text}"
            );
            // 确保只有一条 COMPUTERNAME（被覆盖而非追加）
            let count = text.matches("COMPUTERNAME=").count();
            assert_eq!(
                count, 1,
                "COMPUTERNAME 在环境块中应仅出现一次（被覆盖），实际出现 {count} 次"
            );
        }

        #[test]
        fn build_env_block_preserves_inherited_vars() {
            let extra = vec![("SLTERM_TEST_DUMMY".into(), "dummy_value".into())];
            let block = build_env_block(&extra);
            let text = String::from_utf16_lossy(&block);
            // Windows 必定存在的系统变量（环境块键名保留原始大小写，如 SystemRoot=——
            // 键名匹配不区分大小写，统一转大写后比对）
            let upper = text.to_ascii_uppercase();
            assert!(
                upper.contains("SYSTEMROOT="),
                "应保留继承的系统变量 SYSTEMROOT，实际内容: {text}"
            );
            assert!(
                text.contains("SLTERM_TEST_DUMMY=dummy_value"),
                "应追加新变量 SLTERM_TEST_DUMMY"
            );
        }

        #[test]
        fn build_env_block_double_null_terminated() {
            let extra = vec![("A".into(), "B".into())];
            let block = build_env_block(&extra);
            // 环境块应以双 null 结尾（\0\0）
            let len = block.len();
            assert!(len >= 2, "环境块长度应 ≥ 2");
            assert_eq!(block[len - 1], 0, "环境块最后一个 wchar 应为 null");
            assert_eq!(
                block[len - 2],
                0,
                "环境块倒数第二个 wchar 应为 null（双 null 终止）"
            );
        }

        // ─── T6: to_wide_null / cwd 反斜杠规范化测试 ───
        // 验证 spawn_conpty_child 中 cwd.replace('/', '\\') 的编码正确性

        #[test]
        fn cwd_forward_slash_to_backslash_encoding() {
            // 调用真实 build_cwd_wide（PTY-08 抽取的 cwd 宽字符串构造纯函数）：
            // 正斜杠→反斜杠 + UTF-16LE 编码 + null 终止
            let wide = build_cwd_wide("C:/Users/test/project");
            // 验证 null 终止
            assert!(wide.ends_with(&[0]), "应以 null 结尾");
            // 去掉尾部 null 后解码验证
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(
                text, "C:\\Users\\test\\project",
                "反斜杠路径应正确编码为 UTF-16LE"
            );
        }

        #[test]
        fn cwd_no_trailing_slash_unchanged() {
            // 不含正斜杠的路径无需转换（build_cwd_wide 原样编码）
            let wide = build_cwd_wide("C:\\Users\\test");
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(text, "C:\\Users\\test", "纯反斜杠路径应保持不变");
        }

        #[test]
        fn cwd_mixed_slashes_normalized() {
            // 混合斜杠：只有 / 转为 \（build_cwd_wide 内部规范化）
            let wide = build_cwd_wide("C:/Users\\test/project\\sub");
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(
                text, "C:\\Users\\test\\project\\sub",
                "混合斜杠应将 / 统一为 \\"
            );
        }

        #[test]
        fn to_wide_null_empty_string() {
            let wide = to_wide_null("");
            assert_eq!(wide, vec![0], "空字符串编码为仅含 null 终止符");
        }

        #[test]
        fn to_wide_null_ascii() {
            let wide = to_wide_null("hello");
            assert_eq!(wide.len(), 6, "5 字符 + 1 null = 6 个 u16");
            assert_eq!(wide[0], b'h' as u16);
            assert_eq!(wide[4], b'o' as u16);
            assert_eq!(wide[5], 0, "应以 null 结尾");
        }

        // ─── T7: build_cmdline 引号处理测试（PTY-07）───
        // 程序路径/参数含空格或制表符时须加引号，否则 CreateProcessW 会错误拆分参数

        /// 程序路径含空格 → 路径整体加引号
        #[test]
        fn build_cmdline_quotes_program_with_space() {
            let wide = build_cmdline("C:\\Program Files\\PowerShell\\7\\pwsh.exe", &[]);
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(
                text, "\"C:\\Program Files\\PowerShell\\7\\pwsh.exe\"",
                "含空格程序路径应整体加引号"
            );
        }

        /// 参数含空格 → 该参数加引号，无空格程序路径不加
        #[test]
        fn build_cmdline_quotes_arg_with_space() {
            let wide = build_cmdline("pwsh.exe", &["-Command".into(), "echo hello world".into()]);
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(
                text, "pwsh.exe -Command \"echo hello world\"",
                "含空格参数应加引号，无空格参数不加"
            );
        }

        /// 参数含制表符 → 该参数加引号（tab 同为参数分隔符）
        #[test]
        fn build_cmdline_quotes_arg_with_tab() {
            let wide = build_cmdline("cmd.exe", &["echo\thello".into()]);
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(text, "cmd.exe \"echo\thello\"", "含制表符参数应加引号");
        }

        /// 无空格路径与参数 → 全部不加引号
        #[test]
        fn build_cmdline_no_quotes_without_space() {
            let wide = build_cmdline(
                "C:\\Windows\\System32\\cmd.exe",
                &["/c".into(), "echo".into()],
            );
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(
                text, "C:\\Windows\\System32\\cmd.exe /c echo",
                "无空格时不应加任何引号"
            );
        }

        /// 空 args → 仅程序路径（无尾随空格）
        #[test]
        fn build_cmdline_empty_args() {
            let wide = build_cmdline("cmd.exe", &[]);
            let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);
            assert_eq!(text, "cmd.exe", "空 args 时应仅含程序路径");
        }

        // ─── T8: ConPtyMaster::resize HPCON invalid 分支测试（PTY-09）───
        // HPCON 已关闭（invalid）后 resize 应静默更新 size，不调 Win32 API（不 panic/不报错）

        /// 构造 invalid HPCON 状态 → resize 静默成功且 size 更新
        #[test]
        fn master_resize_invalid_hpc_silently_updates_size() {
            let pipe = Pipe::new().unwrap();
            let master = ConPtyMaster {
                inner: Arc::new(Mutex::new(ConPtyInner {
                    // windows 0.61 的 HPCON 为 isize 包装（非指针），invalid = -1（INVALID_HANDLE_VALUE 低位）
                    hpc: HPCON(INVALID_HANDLE_VALUE.0 as isize),
                    readable: pipe.read,
                    writable: Some(pipe.write),
                    size: PtySize {
                        rows: 24,
                        cols: 80,
                        pixel_width: 0,
                        pixel_height: 0,
                    },
                    // 26100（Win11）→ 系统路径；本测试走 invalid HPCON 分支，不调用 api
                    api: resolve_conpty_api(26100),
                })),
            };

            // resize 静默成功（不调 ResizePseudoConsole）
            master
                .resize(PtySize {
                    rows: 30,
                    cols: 100,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .unwrap();

            // size 已更新
            let size = master.get_size().unwrap();
            assert_eq!(size.rows, 30, "resize 后 rows 应更新");
            assert_eq!(size.cols, 100, "resize 后 cols 应更新");

            // 多次 resize 仍静默成功，且 size 持续更新
            master
                .resize(PtySize {
                    rows: 40,
                    cols: 120,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .unwrap();
            let size = master.get_size().unwrap();
            assert_eq!(size.rows, 40, "多次 resize 后 rows 应持续更新");
            assert_eq!(size.cols, 120, "多次 resize 后 cols 应持续更新");
        }

        // ─── T9: SendRawHandle::pending_bytes（BE-05 微批续读——S06 信号竞态修复回归）───
        // 旧实现 WaitForSingleObject(handle, 0)：数据被读走后信号 reset 存在延迟，
        // 误报「有数据」→ 微批续读的阻塞 read 空等 → reader 线程卡死 → 该终端
        // 永久无输出（E2E 全部终端文本为空 + win10 黑屏根因）。PeekNamedPipe
        // 同步查询可读字节数，无信号时序窗口。本用例锁死新语义。

        #[cfg(windows)]
        #[test]
        fn pending_bytes_reflects_pipe_data_availability() {
            use std::io::{Read, Write};
            use std::os::windows::io::AsRawHandle;

            let mut pipe = Pipe::new().unwrap();
            let handle = SendRawHandle(pipe.read.as_raw_handle());

            // 空管道 → 0
            assert_eq!(handle.pending_bytes(), 0, "空管道应无待读数据");

            // 写入数据 → >0
            pipe.write.write_all(b"hello").unwrap();
            assert!(handle.pending_bytes() > 0, "有数据时应报告可读");

            // 读走后 → 0（Peek 同步查询，无信号 reset 竞态窗口——修复点）
            let mut buf = [0u8; 64];
            let n = pipe.read.read(&mut buf).unwrap();
            assert_eq!(n, 5);
            assert_eq!(
                handle.pending_bytes(),
                0,
                "数据读走后应立即返回 0（旧 WaitForSingleObject 存在 reset 延迟误报）"
            );

            // 再写再读（模拟启动序列后的正常续读节奏）
            pipe.write.write_all(b"more data").unwrap();
            assert!(handle.pending_bytes() > 0);
            let mut buf2 = [0u8; 64];
            let n2 = pipe.read.read(&mut buf2).unwrap();
            assert_eq!(n2, 9);
            assert_eq!(handle.pending_bytes(), 0);

            // 写端关闭：数据读完 → 0（EOF 由主循环 read Ok(0) 兜底）
            drop(pipe.write);
            let mut rest = Vec::new();
            pipe.read.read_to_end(&mut rest).unwrap();
            assert_eq!(
                handle.pending_bytes(),
                0,
                "对端关闭且数据读完 → 0（不误报有数据）"
            );
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

/// 非 Windows 平台：JobHandle 为零大小占位类型（无 Job Object 概念）
#[cfg(not(windows))]
pub struct JobHandle;

#[cfg(not(windows))]
impl JobHandle {
    pub fn new_dummy() -> Self {
        Self
    }
}

/// spawn 参数
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    /// 前端生成的 panel ID
    pub panel_id: String,
    /// 终端列数（BE-14: 反序列化后校验 ≤ i16::MAX）
    pub cols: u16,
    /// 终端行数（BE-14: 反序列化后校验 ≤ i16::MAX）
    pub rows: u16,
    /// 工作目录（可选，默认用户主目录；SEC-02: 经 validate_path_within_root 校验）
    pub cwd: Option<String>,
    /// shell 程序路径（可选，自动检测 pwsh→powershell→cmd；SEC-02: 经 validate_shell_allowlist 校验）
    pub shell: Option<String>,
}

/// spawn 请求三校验（BE-14 尺寸超限 / SEC-02 shell 白名单 / SEC-02 cwd 沙箱）
///
/// D2 可测性重构：从 pty_spawn 命令体抽取为纯函数——不依赖 AppState，
/// project_root 以引用传入，便于 L1 单测边界用例。行为与抽取前完全一致。
fn validate_spawn_request(
    request: &SpawnRequest,
    project_root: &Option<PathBuf>,
) -> Result<(), AppError> {
    // BE-14: COORD 尺寸校验——cols/rows 不能超过 i16::MAX，防止 as i16 回绕
    if request.cols > i16::MAX as u16 || request.rows > i16::MAX as u16 {
        return Err(AppError::Pty(format!(
            "终端尺寸超限: cols={}, rows={}, 最大允许值={}",
            request.cols,
            request.rows,
            i16::MAX
        )));
    }

    // SEC-02: shell 白名单校验（仅允许 pwsh/powershell/cmd）
    if let Some(ref shell) = request.shell {
        shell::validate_shell_allowlist(shell)?;
    }

    // SEC-02: cwd 路径沙箱校验
    if let Some(ref cwd) = request.cwd {
        app_state::validate_path_within_root(project_root, Path::new(cwd))?;
    }

    Ok(())
}

/// BE-01: 会话上限判定纯函数——active 达到 MAX_PTY_SESSIONS 即拒绝
///
/// 抽为纯函数便于 L1 单测边界用例（31 放行 / 32、64 拒绝）。
fn ensure_pty_capacity(active: usize) -> Result<(), AppError> {
    if active >= MAX_PTY_SESSIONS {
        return Err(AppError::Validation(format!(
            "PTY 会话数已达上限 {}，请先关闭部分终端",
            MAX_PTY_SESSIONS
        )));
    }
    Ok(())
}

/// 创建 PTY 并启动 shell，返回 session_id
///
/// 输出通过 on_output Channel 持续推送到前端。
/// BE-01: async + spawn_blocking，阻塞 I/O 不占 IPC worker。
/// BE-12: SPAWN_LOCK 仅保护 create_conpty_pair + spawn_conpty_child（锁内），
/// take_writer、CPR 注入、add_to_job_object 在锁外。
#[tauri::command]
pub async fn pty_spawn(
    state: tauri::State<'_, AppState>,
    on_output: Channel<PtyEvent>,
    request: SpawnRequest,
) -> Result<String, AppError> {
    // BE-14/SEC-02: 三校验（尺寸超限 / shell 白名单 / cwd 沙箱）委托纯函数 validate_spawn_request
    // 注意：RwLockReadGuard 非 Send——须在块内 clone 出 Option<PathBuf> 后立即释放读锁，
    // 否则 guard 跨 await 存活导致 pty_spawn future 不满足 Send
    let project_root = {
        let guard = state
            .project_root
            .read()
            .map_err(|e| AppError::Pty(format!("获取 project_root 锁失败: {}", e)))?;
        (*guard).clone()
    };
    validate_spawn_request(&request, &project_root)?;

    let session_id = Uuid::new_v4().to_string();
    let panel_id = request.panel_id.clone();

    // 解析 shell 程序（不依赖 portable-pty CommandBuilder，直接获取结构化信息）
    let shell_info = shell::resolve_shell_info(request.shell.as_deref())?;

    // 注入终端能力环境变量——Claude Code 依赖此宣告启用 True Color
    // SLTERM_PANEL_ID：子进程据此识别所属面板，供 hooks 信号文件标记事件来源
    let extra_envs: Vec<(String, String)> = vec![
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM".into(), "xterm-256color".into()),
        ("TERM_PROGRAM".into(), "slTerminal".into()),
        ("SLTERM_PANEL_ID".into(), panel_id.clone()),
    ];

    // BE-01: 会话上限检查——判定须在 SPAWN_LOCK 持锁区间内（防并发超发）。
    // spawn_blocking 闭包为 'static，无法借用 state.pty.sessions，故先取读锁快照
    // 传入闭包；判定在闭包内锁后、ConPTY 创建前执行（与 spawn 原子化）。
    // 快照至插入间的并发窗口由下方插入点 sessions 写锁内原子复查兜底。
    let active_sessions = state
        .pty
        .sessions
        .read()
        .map_err(|e| AppError::Pty(format!("获取 sessions 锁失败: {}", e)))?
        .len();

    // BE-01: clone spawn_lock Arc 移送 spawn_blocking 内获取
    let spawn_lock = state.pty.spawn_lock.clone();
    let cols = request.cols;
    let rows = request.rows;
    let cwd = request.cwd.clone();
    // 非 Windows 路径仍需 user_shell 供 resolve_shell 使用
    #[allow(unused_variables)]
    let user_shell = request.shell.clone();

    let session = tokio::task::spawn_blocking(move || -> Result<PtySession, AppError> {
        // BE-12: SPAWN_LOCK 仅保护 create_conpty_pair + spawn_conpty_child
        let _lock = spawn_lock
            .lock()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;

        // BE-01: 会话上限检查（SPAWN_LOCK 区间内，判定与 spawn 原子化）
        ensure_pty_capacity(active_sessions)?;

        // 创建 PTY 并获取 master +（Windows 独有）HPCON 用于子进程 spawn
        // Windows: 绕过 portable-pty openpty，直接调 Win32 CreatePseudoConsole 控制 flags
        #[cfg(windows)]
        let (conpty_hpc, conpty_master) = {
            let build = super::win_build::get_windows_build_number().unwrap_or_else(|e| {
                tracing::warn!("无法获取 Windows build 号: {}", e);
                0
            });
            conpty_custom::create_conpty_pair(cols, rows, build)
                .map_err(|e| AppError::Pty(e.to_string()))?
        };
        // BE-05: 克隆 reader + 微批续读检查器（Windows 专用）——必须在 conpty_master
        // 装箱前完成（其内部字段对 pty_spawn 不可见）；非 Windows 分支在下方统一位置
        #[cfg(windows)]
        let input = conpty_custom::clone_reader_with_pending_check(&conpty_master)
            .map_err(|e| AppError::Pty(e.to_string()))?;
        #[cfg(windows)]
        let master: Box<dyn portable_pty::MasterPty + Send> = Box::new(conpty_master);

        /// 非 Windows: 使用 portable-pty 原生 openpty
        #[cfg(not(windows))]
        let (master, slave) = {
            let pty_system = native_pty_system();
            let pair = pty_system
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| AppError::Pty(e.to_string()))?;
            (pair.master, pair.slave)
        };

        // spawn 子进程（仍在 SPAWN_LOCK 内）
        #[cfg(windows)]
        let mut child: Box<dyn portable_pty::Child + Send> = Box::new(
            conpty_custom::spawn_conpty_child(conpty_hpc, &shell_info, &extra_envs, cwd.as_deref())
                .map_err(|e| AppError::Pty(e.to_string()))?,
        );
        #[cfg(not(windows))]
        let child: Box<dyn portable_pty::Child + Send> = {
            let mut cmd = shell::resolve_shell(user_shell.as_deref())?;
            cmd.env("COLORTERM", "truecolor");
            cmd.env("TERM", "xterm-256color");
            cmd.env("TERM_PROGRAM", "slTerminal");
            cmd.env("SLTERM_PANEL_ID", &panel_id);
            if let Some(ref cwd) = cwd {
                cmd.cwd(cwd.replace('/', "\\"));
            }
            slave
                .spawn_command(cmd)
                .map_err(|e| AppError::Pty(e.to_string()))?
        };

        // BE-12: 释放 SPAWN_LOCK——子进程已启动，后续操作（take_writer、CPR、Job Object）无需串行化
        drop(_lock);

        // take_writer 只能调一次（0.9.0 破坏性变更），Arc<Mutex> 共享
        let raw_writer = master
            .take_writer()
            .map_err(|e| AppError::Pty(e.to_string()))?;
        let writer: Arc<Mutex<Box<dyn std::io::Write + Send>>> = Arc::new(Mutex::new(raw_writer));

        // Windows: spawn 后向 stdin 写 CPR \x1b[1;1R（锁外）
        // 补偿 ConPTY VtIo::StartIfNeeded() DSR 握手。
        #[cfg(windows)]
        {
            let mut w = writer
                .lock()
                .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
            w.write_all(b"\x1b[1;1R")?;
            w.flush()?;
        }

        // 将子进程放入 Job Object 防止孤儿进程（锁外）
        #[cfg(windows)]
        let pid = child.process_id();
        #[cfg(windows)]
        let job_handle = match pid {
            Some(pid) => match add_to_job_object(pid) {
                Ok(handle) => Some(handle),
                Err(e) => {
                    // BE-02: Job Object 创建/分配失败时显式杀子进程，不留孤儿
                    child.kill().ok();
                    return Err(e);
                }
            },
            None => None,
        };
        #[cfg(not(windows))]
        let job_handle: Option<JobHandle> = Some(JobHandle::new_dummy());

        // E1: 创建可替换 Channel 和 ring buffer
        let channel: Arc<RwLock<Option<Channel<PtyEvent>>>> =
            Arc::new(RwLock::new(Some(on_output)));
        let output_ring: Arc<Mutex<VecDeque<u8>>> = Arc::new(Mutex::new(VecDeque::new()));

        // P2-11: child 包装为 Arc<Mutex<>>，reader 线程通过 clone 获取真实退出码
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>> = Arc::new(Mutex::new(child));

        // 克隆 reader，启动 reader 线程（reader.rs 首轮读取剥离 ConPTY 启动注入序列）
        // Windows 分支的 input 已在上方 conpty_master 装箱前克隆（BE-05）
        #[cfg(not(windows))]
        let input = {
            let r = master
                .try_clone_reader()
                .map_err(|e| AppError::Pty(e.to_string()))?;
            // 非 Windows 无 ConPTY 管道非阻塞检查能力：微批退化为每轮一次 read（行为同现状）
            crate::pty::reader::PtyReaderInput::new(r, Box::new(|| false))
        };
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
                input,
                reader_channel,
                reader_ring,
                reader_child,
                reader_exit_code,
                writer_reader,
                da1_injected_reader,
            );
        });

        Ok(PtySession {
            master: Arc::new(Mutex::new(master)),
            child,
            writer,
            reader_handle: Some(reader_handle),
            channel,
            output_ring,
            exit_code: exit_code_slot,
            da1_injected,
            job_object: job_handle,
            panel_id, // SEC-08: 记录归属 panel
        })
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_spawn join error: {e}")))??;

    // 保存会话（在 async 上下文中，不在 spawn_blocking 内）
    // BE-01: sessions 写锁内原子「检查+插入」——兜底锁内快照判定后的并发窗口
    //（前序 spawn 的插入尚未完成时快照偏旧），杜绝并发超发。命中上限时显式
    // kill 已 spawn 的子进程：kill 后 ConPTY 输出端关闭 → reader 退出 →
    // PtySession drop 时 join 正常返回；Job Object KILL_ON_JOB_CLOSE 兜底。
    let mut sessions = state
        .pty
        .sessions
        .write()
        .map_err(|e| AppError::Pty(format!("获取 sessions 锁失败: {}", e)))?;
    if sessions.len() >= MAX_PTY_SESSIONS {
        if let Ok(mut child) = session.child.lock() {
            let _ = child.kill();
        }
        return Err(AppError::Validation(format!(
            "PTY 会话数已达上限 {}，请先关闭部分终端",
            MAX_PTY_SESSIONS
        )));
    }
    sessions.insert(session_id.clone(), session);

    Ok(session_id)
}

/// SEC-08: 校验 panel_id 与 session 归属一致（纯函数）
///
/// D2 可测性重构：从 pty_write/resize/kill 三命令体抽取——仅读 session.panel_id，
/// 不依赖锁与状态，便于 L1 单测归属放行/拒绝。行为与抽取前完全一致。
fn validate_session_ownership(session: &PtySession, panel_id: &str) -> Result<(), AppError> {
    if session.panel_id != panel_id {
        return Err(AppError::Pty(format!(
            "会话归属不匹配: 请求 panel_id={}, session panel_id={}",
            panel_id, session.panel_id
        )));
    }
    Ok(())
}

/// 向 PTY 写入数据（来自前端键盘输入）
///
/// BE-01: async + spawn_blocking，write_all/flush 不阻塞 IPC worker。
/// SEC-08: 校验 panel_id 与 session 归属一致。
#[tauri::command]
pub async fn pty_write(
    state: tauri::State<'_, AppState>,
    session_id: String,
    panel_id: String,
    data: Vec<u8>,
) -> Result<(), AppError> {
    let writer = {
        let sessions = state
            .pty
            .sessions
            .read()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
        // SEC-08: 校验 session 归属
        validate_session_ownership(session, &panel_id)?;
        session.writer.clone()
    };

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let mut w = writer
            .lock()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        w.write_all(&data)?;
        w.flush()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_write join error: {e}")))?
}

/// 调整 PTY 终端尺寸
///
/// BE-01: async + spawn_blocking，ResizePseudoConsole 不阻塞 IPC worker。
/// SEC-08: 校验 panel_id 与 session 归属一致。
///
/// 锁嵌套注意事项：sessions.read() → 提取 master Arc → spawn_blocking 内 master.lock()。
/// 由于 Arc clone 在 sessions 读锁释放后才进入 spawn_blocking，无死锁风险。
#[tauri::command]
pub async fn pty_resize(
    state: tauri::State<'_, AppState>,
    session_id: String,
    panel_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    let master = {
        let sessions = state
            .pty
            .sessions
            .read()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
        // SEC-08: 校验 session 归属
        validate_session_ownership(session, &panel_id)?;
        session.master.clone()
    };

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let m = master
            .lock()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::Pty(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_resize join error: {e}")))?
}

/// 销毁 PTY 会话 — 杀子进程 → 回收 reader 线程 → 从 PtyState 移除
///
/// G1b: async + spawn_blocking。先提取 session 后释放 RwLock 写锁，
/// 再在 spawn_blocking 中执行 kill+join+drop（ClosePseudoConsole 在 pre-Win11 24H2 上永久阻塞），
/// 避免持锁阻塞导致后续命令级联卡死。
/// BE-06: kill 返回值检查（失败 warn 继续——Job Object KILL_ON_JOB_CLOSE 兜底杀子进程）；
/// reader join 带 3s 超时（KILL_JOIN_TIMEOUT 轮询 is_finished，超时放弃 join 记 warn，
/// 线程随 PtySession Drop 兜底）。
/// SEC-08: 校验 panel_id 与 session 归属一致后再移除。
#[tauri::command]
pub async fn pty_kill(
    state: tauri::State<'_, AppState>,
    session_id: String,
    panel_id: String,
) -> Result<(), AppError> {
    // 提取 session 后释放写锁（锁在此 scope 结束时释放，<1ms）
    let session = {
        let mut sessions = state
            .pty
            .sessions
            .write()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        // SEC-08: 先校验归属再 remove
        let stored = sessions
            .get(&session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;
        validate_session_ownership(stored, &panel_id)?;
        sessions
            .remove(&session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?
    };

    // blocking 线程中执行 kill+join+drop，不阻塞 IPC worker
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let mut session = session;
        let mut child = session
            .child
            .lock()
            .map_err(|e| AppError::Pty(format!("锁获取失败: {}", e)))?;
        // BE-06: 检查 kill 返回值——失败仅告警并继续（Job Object
        // KILL_ON_JOB_CLOSE 兜底杀子进程；kill 失败不阻塞销毁流程）
        if let Err(e) = child.kill() {
            tracing::warn!("pty_kill: child.kill() 失败: {e}");
        }
        drop(child);
        if let Some(handle) = session.reader_handle.take() {
            // BE-06: join 带 3s 超时（轮询 is_finished）——超时放弃 join 记 warn，
            // 线程随 PtySession Drop（state.rs Drop join）兜底
            if !join_with_timeout(handle, KILL_JOIN_TIMEOUT) {
                tracing::warn!("pty_kill: reader 线程 3s 内未退出，放弃 join（随 Drop 兜底）");
            }
        }
        // session drop → master drop → ClosePseudoConsole
        Ok(())
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_kill join error: {e}")))?
}

/// 销毁全部 PTY 会话 — 关闭序列兜底（BE-08）
///
/// 前端关闭序列：先前端 TerminalRegistry 快速 kill，再调用本命令兜底——
/// 前后端 session 不一致（前端 Registry 缺失条目）时防止后端 session 泄漏。
/// 遍历 sessions 全部 kill + join（超时语义同 BE-06：KILL_JOIN_TIMEOUT 轮询
/// is_finished，超时放弃 join 记 warn，线程随 PtySession Drop 兜底），
/// 返回成功 kill 数。
#[tauri::command]
pub async fn pty_kill_all(state: tauri::State<'_, AppState>) -> Result<u32, AppError> {
    pty_kill_all_impl(&state.pty).await
}

/// pty_kill_all 命令内核（BE-08，供 L1 直接调用，无需构造 tauri::State）
///
/// 先 drain 提取全部 session（清空 sessions map）后释放写锁，再在 spawn_blocking
/// 中执行 kill+join+drop（ClosePseudoConsole 在 pre-Win11 24H2 上永久阻塞，
/// 避免持锁阻塞后续命令；同 pty_kill 的 G1b 语义）。
async fn pty_kill_all_impl(pty: &PtyState) -> Result<u32, AppError> {
    // 提取全部 session 后释放写锁（锁在此 scope 结束时释放，<1ms）
    let sessions: Vec<PtySession> = {
        let mut guard = pty
            .sessions
            .write()
            .map_err(|e| AppError::Pty(format!("获取 sessions 锁失败: {e}")))?;
        guard.drain().map(|(_, s)| s).collect()
    };

    // blocking 线程中逐个 kill+join+drop，不阻塞 IPC worker
    tokio::task::spawn_blocking(move || -> Result<u32, AppError> {
        let mut killed = 0u32;
        for mut session in sessions {
            let mut child = session
                .child
                .lock()
                .map_err(|e| AppError::Pty(format!("锁获取失败: {e}")))?;
            // BE-06 同款语义：检查 kill 返回值——失败仅告警并继续
            // （Job Object KILL_ON_JOB_CLOSE 兜底杀子进程）
            match child.kill() {
                Ok(()) => killed += 1,
                Err(e) => tracing::warn!("pty_kill_all: child.kill() 失败: {e}"),
            }
            drop(child);
            if let Some(handle) = session.reader_handle.take() {
                // BE-06 同款：join 带 3s 超时（轮询 is_finished）——超时放弃
                // join 记 warn，线程随 PtySession Drop（state.rs Drop join）兜底
                if !join_with_timeout(handle, KILL_JOIN_TIMEOUT) {
                    tracing::warn!(
                        "pty_kill_all: reader 线程 3s 内未退出，放弃 join（随 Drop 兜底）"
                    );
                }
            }
            // session drop → master drop → ClosePseudoConsole
        }
        Ok(killed)
    })
    .await
    .map_err(|e| AppError::Pty(format!("pty_kill_all join error: {e}")))?
}

/// BE-06: pty_kill 等待 reader 线程退出的超时——3s 后放弃 join，
/// 线程随 PtySession Drop / 进程退出兜底
const KILL_JOIN_TIMEOUT: Duration = Duration::from_secs(3);

/// BE-06: join 超时轮询间隔（10ms，轻量轮询，避免忙等）
const KILL_JOIN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// BE-06: 带超时的线程 join——轮询 `is_finished` 至 deadline，避免无限期阻塞
///
/// 返回 false = 超时未完成（调用方记 warn 后放弃，线程随 Drop 兜底）。
/// 轮询到 is_finished 后调用 join() 回收线程资源（立即返回）。
/// 纯逻辑 + 标准库线程，可 L1 单测（不依赖 PTY）。
fn join_with_timeout(handle: std::thread::JoinHandle<()>, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if handle.is_finished() {
            let _ = handle.join();
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(KILL_JOIN_POLL_INTERVAL);
    }
}

/// Windows Job Object — 将子进程与父进程生命周期绑定，防止孤儿进程
///
/// 构建 Job Object 名称字符串并委托给 `create_and_assign_job` 执行 Win32 调用。
/// 返回 `JobHandle` 以在整个 PTY 会话期间持有 job handle，防止 `KILL_ON_JOB_CLOSE` 过早触发。
///
/// D2 可测性重构：job_name 构造与 limit flags 计算已抽为纯函数（job_name / job_limits），
/// 由 L1 单测覆盖；Win32 调用本身（CreateJobObjectW/SetInformationJobObject/
/// AssignProcessToJobObject）由本函数内联执行。
#[cfg(windows)]
fn add_to_job_object(pid: u32) -> Result<JobHandle, AppError> {
    use std::os::windows::ffi::OsStrExt;

    let job_name = job_name(pid);
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

/// 构造 Job Object 名称（纯函数）——`slTerminal_pty_{pid}`，保证 job 名称按子进程 PID 唯一
#[cfg(windows)]
fn job_name(pid: u32) -> String {
    format!("slTerminal_pty_{pid}")
}

/// 构造带 KILL_ON_JOB_CLOSE 的扩展限制信息（纯函数，D2 抽取）
///
/// 锁死项：`LimitFlags` 必须包含 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`（父进程退出时
/// OS 自动杀所有子进程——孤儿防护核心）。测试断言具体值 0x2000 防未来误删。
#[cfg(windows)]
fn job_limits() -> windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    use windows::Win32::System::JobObjects::{
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    limits
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
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    // 创建 Job Object
    let job = CreateJobObjectW(None, PCWSTR::from_raw(job_name_wide.as_ptr()))
        .map_err(|e| AppError::Pty(format!("CreateJobObject failed: {e}")))?;

    // 设置 JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE（父进程退出时 OS 杀所有子进程）
    // limits 构造委托 job_limits 纯函数（D2 抽取，KILL_ON_JOB_CLOSE 设置由 L1 单测锁死）
    let limits = job_limits();
    SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &limits as *const _ as *const _,
        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
    )
    .map_err(|e| AppError::Pty(format!("SetInformationJobObject failed: {e}")))?;

    // 打开子进程句柄并分配到 Job Object
    let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)
        .map_err(|e| AppError::Pty(format!("OpenProcess failed: {e}")))?;

    AssignProcessToJobObject(job, process)
        .map_err(|e| AppError::Pty(format!("AssignProcessToJobObject failed: {e}")))?;

    // process 句柄仅用于 AssignProcessToJobObject，完成后立即释放
    let _ = CloseHandle(process);

    Ok(JobHandle::new(job))
}

// ─── spawn.rs 模块级测试：ring buffer 回放 + session 隔离 ───

#[cfg(test)]
mod spawn_tests {
    use super::*;
    use crate::state::PtyState;
    use portable_pty::MasterPty;
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;

    /// ring buffer drain 回放：drain 后内容正确回放，buffer 清零，后续写入正常
    #[test]
    fn ring_buffer_replay_drain_and_continue() {
        let ring = Arc::new(Mutex::new(VecDeque::new()));

        // 阶段 1: 模拟 Channel 断开时 reader 线程向 ring buffer 写数据
        ring.lock().unwrap().extend(b"LINE_A\n");
        ring.lock().unwrap().extend(b"LINE_B\n");
        ring.lock().unwrap().extend(b"LINE_C\n");
        assert_eq!(ring.lock().unwrap().len(), 21); // 3 × 7 bytes

        // 阶段 2: reattach 时 drain 回放 ring buffer 全部内容
        let replay: Vec<u8> = ring.lock().unwrap().drain(..).collect();
        let text = String::from_utf8_lossy(&replay);
        assert!(text.contains("LINE_A"), "回放数据应含 LINE_A");
        assert!(text.contains("LINE_B"), "回放数据应含 LINE_B");
        assert!(text.contains("LINE_C"), "回放数据应含 LINE_C");

        // 阶段 3: drain 后 buffer 必须为空（避免 reattach 后重复回放）
        assert!(ring.lock().unwrap().is_empty(), "drain 后 buffer 应为空");

        // 阶段 4: reattach 后新输出继续写入 ring buffer
        ring.lock().unwrap().extend(b"POST_D\n");
        assert_eq!(ring.lock().unwrap().len(), 7, "新数据应正常写入");
    }

    /// ring buffer 空 buffer drain 不会 panic（防御性验证）
    #[test]
    fn ring_buffer_empty_drain_no_panic() {
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        let drained: Vec<u8> = ring.lock().unwrap().drain(..).collect();
        assert!(drained.is_empty());
        assert!(ring.lock().unwrap().is_empty());
    }

    /// ring buffer 部分 drain 只移除指定范围
    #[test]
    fn ring_buffer_partial_drain() {
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        ring.lock().unwrap().extend(b"0123456789");
        // drain 前 5 字节
        let front: Vec<u8> = ring.lock().unwrap().drain(0..5).collect();
        assert_eq!(front, b"01234");
        assert_eq!(ring.lock().unwrap().len(), 5);
        assert_eq!(ring.lock().unwrap()[0], b'5');
    }

    /// 验证 PtyState session 移除不级联：移除一个 session 不影响其他 session
    #[cfg(windows)]
    #[test]
    fn session_removal_does_not_cascade() {
        let pty_state = PtyState::new();

        // 创建三个独立 PTY session
        let sa = make_test_session("panel-a");
        let sb = make_test_session("panel-b");
        let sc = make_test_session("panel-c");

        {
            let mut sessions = pty_state.sessions.write().unwrap();
            sessions.insert("sid-a".into(), sa);
            sessions.insert("sid-b".into(), sb);
            sessions.insert("sid-c".into(), sc);
        }
        assert_eq!(pty_state.sessions.read().unwrap().len(), 3);

        // 移除中间 session（sid-b）
        let removed = pty_state.sessions.write().unwrap().remove("sid-b");
        assert!(removed.is_some(), "sid-b 应存在且可移除");

        // 验证 sid-a 和 sid-c 仍存在
        {
            let sessions = pty_state.sessions.read().unwrap();
            assert!(
                sessions.contains_key("sid-a"),
                "移除 sid-b 后 sid-a 应仍存在——不得级联删除"
            );
            assert!(
                sessions.contains_key("sid-c"),
                "移除 sid-b 后 sid-c 应仍存在——不得级联删除"
            );
            assert!(!sessions.contains_key("sid-b"), "sid-b 应已移除");
            assert_eq!(sessions.len(), 2, "移除一个后应剩余 2 个 session");

            // panel_id 归属正确
            assert_eq!(sessions.get("sid-a").unwrap().panel_id, "panel-a");
            assert_eq!(sessions.get("sid-c").unwrap().panel_id, "panel-c");
        }

        // 清理：杀子进程防止残留
        for sid in ["sid-a", "sid-c"] {
            cleanup_session(&pty_state, sid);
        }
    }

    /// 同 panel_id 的不同 session 可独立存在与移除
    #[cfg(windows)]
    #[test]
    fn sessions_with_same_panel_id_independent() {
        let pty_state = PtyState::new();
        let s1 = make_test_session("panel-x");
        let s2 = make_test_session("panel-x"); // 同 panel_id

        pty_state
            .sessions
            .write()
            .unwrap()
            .insert("sid-1".into(), s1);
        pty_state
            .sessions
            .write()
            .unwrap()
            .insert("sid-2".into(), s2);
        assert_eq!(pty_state.sessions.read().unwrap().len(), 2);

        // 移除 sid-1，sid-2 应不受影响
        let removed = pty_state.sessions.write().unwrap().remove("sid-1");
        assert!(removed.is_some());
        // 显式 drop removed 以释放锁引用
        drop(removed);

        let sessions = pty_state.sessions.read().unwrap();
        assert!(!sessions.contains_key("sid-1"));
        assert!(
            sessions.contains_key("sid-2"),
            "同 panel_id 的另一 session 应仍存在"
        );
        assert_eq!(sessions.get("sid-2").unwrap().panel_id, "panel-x");
        drop(sessions);

        // 清理
        cleanup_session(&pty_state, "sid-2");
    }

    /// 移除不存在的 session 返回 None，不影响已有 session
    #[cfg(windows)]
    #[test]
    fn remove_nonexistent_session_no_side_effect() {
        let pty_state = PtyState::new();
        let s = make_test_session("panel-y");
        pty_state
            .sessions
            .write()
            .unwrap()
            .insert("sid-y".into(), s);
        assert_eq!(pty_state.sessions.read().unwrap().len(), 1);

        // 移除不存在的 key
        let result = pty_state
            .sessions
            .write()
            .unwrap()
            .remove("sid-nonexistent");
        assert!(result.is_none(), "移除不存在的 session 应返回 None");

        // 已有 session 不受影响
        let sessions = pty_state.sessions.read().unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions.contains_key("sid-y"));
        drop(sessions);

        // 清理
        cleanup_session(&pty_state, "sid-y");
    }

    // ─── PTY-01: Job Object 纯函数测试 ───

    /// job_name 纯函数——名称 = `slTerminal_pty_{pid}`，按子进程 PID 唯一
    #[cfg(windows)]
    #[test]
    fn job_name_format_contains_pid() {
        assert_eq!(job_name(1234), "slTerminal_pty_1234");
        assert_eq!(job_name(0), "slTerminal_pty_0");
    }

    /// job_limits 纯函数——LimitFlags 必须含 KILL_ON_JOB_CLOSE（孤儿防护核心）
    /// 锁死具体值 0x2000（Windows SDK 定义），防未来误删该标志
    #[cfg(windows)]
    #[test]
    fn job_limits_contains_kill_on_job_close() {
        use windows::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let limits = job_limits();
        assert_eq!(
            limits.BasicLimitInformation.LimitFlags, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            "LimitFlags 应包含 KILL_ON_JOB_CLOSE"
        );
        assert_eq!(
            limits.BasicLimitInformation.LimitFlags,
            JOB_OBJECT_LIMIT(0x2000),
            "KILL_ON_JOB_CLOSE 值应锁死为 0x2000（防误删孤儿防护）"
        );
    }

    /// JobHandle Drop 关闭句柄——drop 后原句柄应失效（CloseHandle 已调用）
    #[cfg(windows)]
    #[test]
    fn job_handle_drop_closes_handle() {
        use windows::Win32::Foundation::GetHandleInformation;
        use windows::Win32::System::JobObjects::CreateJobObjectW;

        // SAFETY: CreateJobObjectW 是 Win32 API；句柄由 JobHandle RAII 管理
        let job = unsafe { CreateJobObjectW(None, None) }.expect("CreateJobObjectW 应成功");
        {
            let _jh = JobHandle::new(job);
        } // 此处 drop → CloseHandle
        let mut flags: u32 = 0;
        let res = unsafe { GetHandleInformation(job, &mut flags) };
        assert!(
            res.is_err(),
            "drop 后句柄应已关闭（GetHandleInformation 应失败）"
        );
    }

    /// JobHandle 持无效句柄 drop 不 panic（CloseHandle 对无效句柄返回 FALSE，不 panic）
    #[cfg(windows)]
    #[test]
    fn job_handle_invalid_handle_drop_no_panic() {
        let jh = JobHandle::new(windows::Win32::Foundation::INVALID_HANDLE_VALUE);
        drop(jh);
    }

    // ─── PTY-02: validate_spawn_request 三校验测试 ───

    /// 构造最小 SpawnRequest（shell/cwd 可选）
    fn make_request(cols: u16, rows: u16, shell: Option<&str>, cwd: Option<&str>) -> SpawnRequest {
        SpawnRequest {
            panel_id: "p1".into(),
            cols,
            rows,
            cwd: cwd.map(String::from),
            shell: shell.map(String::from),
        }
    }

    /// 尺寸超限（cols > i16::MAX）→ 拒绝（BE-14）
    #[test]
    fn validate_spawn_request_rejects_oversize_cols() {
        let req = make_request(i16::MAX as u16 + 1, 24, None, None);
        let err = validate_spawn_request(&req, &None).unwrap_err();
        assert!(
            err.to_string().contains("终端尺寸超限"),
            "错误消息应含'终端尺寸超限'，实际: {err}"
        );
    }

    /// 尺寸超限（rows > i16::MAX）→ 拒绝（BE-14）
    #[test]
    fn validate_spawn_request_rejects_oversize_rows() {
        let req = make_request(80, i16::MAX as u16 + 1, None, None);
        assert!(validate_spawn_request(&req, &None).is_err());
    }

    /// 最大合法尺寸 → 放行
    #[test]
    fn validate_spawn_request_accepts_max_valid_size() {
        let req = make_request(i16::MAX as u16, i16::MAX as u16, None, None);
        assert!(
            validate_spawn_request(&req, &None).is_ok(),
            "i16::MAX 尺寸应放行"
        );
    }

    /// 非法 shell（白名单外）→ 拒绝（SEC-02）
    #[test]
    fn validate_spawn_request_rejects_disallowed_shell() {
        let req = make_request(80, 24, Some("definitely_not_a_shell.exe"), None);
        let err = validate_spawn_request(&req, &None).unwrap_err();
        assert!(
            err.to_string().contains("不允许的 shell"),
            "错误消息应含'不允许的 shell'，实际: {err}"
        );
    }

    /// 合法 shell（pwsh.exe 文件名）→ 放行
    #[test]
    fn validate_spawn_request_accepts_allowed_shell() {
        let req = make_request(80, 24, Some("pwsh.exe"), None);
        assert!(validate_spawn_request(&req, &None).is_ok());
    }

    /// cwd 越界（root 外）→ 拒绝（SEC-02 沙箱）
    #[test]
    fn validate_spawn_request_rejects_cwd_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("proj.txt");
        std::fs::write(&outside_file, "x").unwrap();
        let req = make_request(80, 24, None, Some(outside_file.to_str().unwrap()));
        let err = validate_spawn_request(&req, &Some(root.path().to_path_buf())).unwrap_err();
        assert!(
            err.to_string().contains("超出项目范围"),
            "错误消息应含'超出项目范围'，实际: {err}"
        );
    }

    /// cwd 在根内 → 放行
    #[test]
    fn validate_spawn_request_accepts_cwd_inside_root() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let req = make_request(80, 24, None, Some(sub.to_str().unwrap()));
        assert!(
            validate_spawn_request(&req, &Some(root.path().to_path_buf())).is_ok(),
            "根内 cwd 应放行"
        );
    }

    /// cwd 存在但 project_root 未设置 → 测试模式豁免放行（validate_path_within_root cfg!(test) 分支）
    #[test]
    fn validate_spawn_request_cwd_without_root_allowed_in_test() {
        let req = make_request(80, 24, None, Some("C:\\any\\path"));
        assert!(validate_spawn_request(&req, &None).is_ok());
    }

    // ─── PTY-03: SEC-08 归属校验测试 ───

    /// 归属校验——panel_id 匹配 → 放行
    #[cfg(windows)]
    #[test]
    fn validate_session_ownership_allows_matching_panel() {
        let session = make_test_session("panel-owner");
        assert!(
            validate_session_ownership(&session, "panel-owner").is_ok(),
            "归属匹配应放行"
        );
        // 清理：杀子进程防残留（session 未入 pty_state，需手动 kill）
        // 显式 let + drop 释放 MutexGuard，避免借用跨 session drop（E0713）
        let mut child_guard = session.child.lock().unwrap();
        let _ = child_guard.kill();
        drop(child_guard);
    }

    /// 归属校验——panel_id 不匹配 → 拒绝（含错误消息，D7 防复发）
    #[cfg(windows)]
    #[test]
    fn validate_session_ownership_rejects_mismatched_panel() {
        let session = make_test_session("panel-owner");
        let err = validate_session_ownership(&session, "intruder").unwrap_err();
        assert!(
            err.to_string().contains("会话归属不匹配"),
            "错误消息应含'会话归属不匹配'，实际: {err}"
        );
        // 清理：杀子进程防残留
        // 显式 let + drop 释放 MutexGuard，避免借用跨 session drop（E0713）
        let mut child_guard = session.child.lock().unwrap();
        let _ = child_guard.kill();
        drop(child_guard);
    }

    // ─── BE-01: PTY 会话总数上限测试（TQ-COV-03 复核：边界已全覆盖，不重复补）───
    // 三档覆盖：0/MAX-1 放行、MAX 拒绝（AppError::Validation + 消息含上限值）、
    // MAX+1/usize::MAX 拒绝（防溢出回绕）。

    /// 上限内放行（边界 31 / 空会话 0）
    #[test]
    fn pty_capacity_below_limit_passes() {
        ensure_pty_capacity(0).expect("空会话应放行");
        ensure_pty_capacity(MAX_PTY_SESSIONS - 1).expect("上限内应放行");
    }

    /// 达到上限拒绝——返回 AppError::Validation 且消息含上限值
    #[test]
    fn pty_capacity_at_limit_rejected() {
        let err = ensure_pty_capacity(MAX_PTY_SESSIONS).expect_err("达到上限应拒绝");
        assert!(matches!(err, AppError::Validation(_)));
        assert!(
            err.to_string().contains("32"),
            "错误消息应含上限值，实际: {err}"
        );
    }

    /// 超限拒绝（含 usize 极端值防溢出回绕）
    #[test]
    fn pty_capacity_above_limit_rejected() {
        assert!(ensure_pty_capacity(MAX_PTY_SESSIONS + 1).is_err());
        assert!(ensure_pty_capacity(usize::MAX).is_err());
    }

    // ─── 辅助函数 ───

    /// 测试辅助：从 PtyState 移除 session 并杀子进程（防残留）
    /// PTY-13①: 抽取自三处重复清理块
    #[cfg(windows)]
    fn cleanup_session(pty_state: &PtyState, sid: &str) {
        if let Some(s) = pty_state.sessions.write().unwrap().remove(sid) {
            if let Ok(mut c) = s.child.lock() {
                let _ = c.kill();
            }
        };
    }

    /// 创建最小 PtySession 供 session 隔离测试使用
    /// 使用 conpty_custom 创建真实 ConPTY 对 + 启动 cmd.exe 子进程
    #[cfg(windows)]
    fn make_test_session(panel_id: &str) -> PtySession {
        let shell_info = crate::pty::shell::resolve_shell_info(Some("cmd.exe"))
            .expect("resolve_shell_info 应成功");
        let (hpc, conpty_master) =
            conpty_custom::create_conpty_pair(80, 24, 26100).expect("create_conpty_pair 应成功");
        // CPR 注入（对齐生产代码 pty_spawn 行为）
        let mut w = conpty_master.take_writer().expect("take_writer 应成功");
        use std::io::Write as _;
        w.write_all(b"\x1b[1;1R").unwrap();
        w.flush().unwrap();
        let writer: Arc<Mutex<Box<dyn std::io::Write + Send>>> = Arc::new(Mutex::new(w));

        let extra_envs: Vec<(String, String)> = vec![
            ("COLORTERM".into(), "truecolor".into()),
            ("TERM".into(), "xterm-256color".into()),
            ("TERM_PROGRAM".into(), "slTerminal".into()),
        ];
        let child = conpty_custom::spawn_conpty_child(hpc, &shell_info, &extra_envs, None)
            .expect("spawn_conpty_child 应成功");

        PtySession {
            master: Arc::new(Mutex::new(Box::new(conpty_master))),
            child: Arc::new(Mutex::new(Box::new(child))),
            writer,
            reader_handle: None,
            channel: Arc::new(RwLock::new(None)),
            output_ring: Arc::new(Mutex::new(VecDeque::new())),
            exit_code: Arc::new(Mutex::new(None)),
            da1_injected: Arc::new(AtomicBool::new(false)),
            job_object: None,
            panel_id: panel_id.to_string(),
        }
    }

    // ─── BE-06: join_with_timeout 测试（TQ-COV-03 复核增强）───
    // TQ-COV-03：既有用例已覆盖 true/false 分支，按 checklist 命名对齐并增强——
    // finished 用例补「快速（<1s）」时间断言；blocked 用例改 park 线程（精确阻塞，
    // 不依赖睡眠计时），timeout=50ms 注入短超时测 false 分支。

    #[test]
    fn join_with_timeout_finished_handle_returns_true() {
        // 立即结束的线程：超时前完成 join，返回 true 且快速（<1s）
        let start = std::time::Instant::now();
        let handle = std::thread::spawn(|| {});
        assert!(join_with_timeout(handle, Duration::from_millis(200)));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "已结束线程的 join 应立即返回，实际耗时 {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn join_with_timeout_blocked_thread_returns_false() {
        // park 的线程 + 短超时（50ms）→ 返回 false（调用方记 warn，线程随 Drop 兜底）
        let handle = std::thread::spawn(|| std::thread::park());
        assert!(!join_with_timeout(handle, Duration::from_millis(50)));
    }

    #[test]
    fn join_with_timeout_abandoned_thread_finishes_later_no_panic() {
        // 超时放弃 join 后，线程自行结束不 panic（JoinHandle drop 时 detach）
        let handle = std::thread::spawn(|| std::thread::sleep(Duration::from_millis(100)));
        assert!(!join_with_timeout(handle, Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(150));
    }

    // ─── BE-08: pty_kill_all 测试 ───

    /// 空会话：返回 0，无副作用
    #[test]
    fn pty_kill_all_empty_returns_zero() {
        let pty = PtyState::new();
        let killed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(pty_kill_all_impl(&pty))
            .unwrap();
        assert_eq!(killed, 0, "空会话应返回 0");
        assert!(pty.sessions.read().unwrap().is_empty(), "sessions 保持为空");
    }

    /// 多会话：全部 kill 成功（计数 = 会话数），sessions 清空
    #[cfg(windows)]
    #[test]
    fn pty_kill_all_kills_all_sessions() {
        let pty = PtyState::new();
        pty.sessions
            .write()
            .unwrap()
            .insert("sid-a".into(), make_test_session("panel-a"));
        pty.sessions
            .write()
            .unwrap()
            .insert("sid-b".into(), make_test_session("panel-b"));
        assert_eq!(pty.sessions.read().unwrap().len(), 2);

        let killed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(pty_kill_all_impl(&pty))
            .unwrap();
        assert_eq!(killed, 2, "两个真实 session 都应 kill 成功");
        assert!(
            pty.sessions.read().unwrap().is_empty(),
            "kill_all 后 sessions 应清空（关闭序列兜底语义）"
        );
    }
}
