//! PTY spawn 规则层（旧栈 `pty/spawn.rs` 语义资产并入）。
//!
//! - ConPTY flags 能力矩阵：`compute_conpty_flags` 按 build 门控逐位取矩阵，
//!   默认矩阵与旧三态（0x7/0x7/0x3）恒等，零默认漂移。
//! - `SPAWN_LOCK` 串行化：并发 spawn 会卡死 ConPTY 输出管道——create + spawn
//!   段整段持锁（BE-12 锁界）；会话容量上限判定在锁内与占用登记原子化
//!   （BE-01，`MAX_PTY_SESSIONS`）。
//! - Job Object 孤儿防护：每个子进程放入 Job Object，
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`——父进程崩溃时 OS 自动杀整棵
//!   子进程树。

use std::io::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

use crate::event::WindowSize;
use crate::tty::Options;
use crate::tty::windows::Pty;
use crate::tty::windows::conpty;
use crate::tty::windows::conpty::CONPTY_WIN11_MIN_BUILD;

// ─── ConPTY flags 能力矩阵 ───

// CreatePseudoConsole dwFlags 位值（windows-sys 仅定义 INHERIT_CURSOR）。
use windows_sys::Win32::System::Console::PSEUDOCONSOLE_INHERIT_CURSOR as FLAG_INHERIT_CURSOR;
pub(crate) const FLAG_WIN32_INPUT_MODE: u32 = 0x4;
const FLAG_RESIZE_QUIRK: u32 = 0x2;
const FLAG_PASSTHROUGH_MODE: u32 = 0x8;

/// ConPTY 输入模式能力矩阵（设置键 DTO；段形态归 06 篇键域，core 锚定字段
/// 语义与默认三态）。壳读 settings 键经 `Options::conpty_input_modes` 注入；
/// 默认矩阵 = 现状三态，零默认漂移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConptyInputModes {
    /// 0x1 INHERIT_CURSOR：子进程继承 conhost 光标位置。
    pub inherit_cursor: bool,
    /// 0x2 RESIZE_QUIRK：启用 conhost resize 兼容行为。
    pub resize_quirk: bool,
    /// 0x4 WIN32_INPUT_MODE：键事件模式（kitty/功能键的真实传输层）。
    pub win32_input_mode: bool,
    /// 0x8 PASSTHROUGH_MODE——默认 false，启用须过人工实测门禁（见
    /// `compute_conpty_flags` 注释的实测记录）。
    pub passthrough_mode: bool,
}

impl Default for ConptyInputModes {
    fn default() -> Self {
        Self {
            inherit_cursor: true,
            resize_quirk: true,
            win32_input_mode: true,
            passthrough_mode: false,
        }
    }
}

/// 计算 ConPTY flags（默认矩阵与旧三态恒等，零漂移）：
///
/// 逐位取矩阵：0x1/0x2 直取矩阵位；0x4（WIN32_INPUT_MODE）维持门控——
/// **捆绑新 conhost**（侧载 OpenConsole）恒启用（新版完整支持 0x4，修复
/// microsoft/terminal#376 的 PR #4856 即在新版）；系统 conhost 仅
/// build >= 21376 启用；系统 conhost + 老 Win10 不置位——**回退路径**。
/// 0x3 未修复滚轮（0x3/0x7 均实测失效，根因是老 conhost 不转发鼠标 VT
/// 序列），仅因键盘/IME 已实测正常而保留，防回退场景无谓启用 0x4。
/// 0x8（PASSTHROUGH_MODE）为末行矩阵位：默认矩阵不含 0x8。
///
/// 阈值 21376 与 `CONPTY_WIN11_MIN_BUILD` 单常量共用（Win10/Win11 分界）。
///
/// **默认矩阵不含 0x8**：0x8 下 claude 等全屏 TUI（v2.1.89+ 默认
/// alt buffer + mouse tracking）的鼠标滚轮完全失效。2026-07 在 Win11
/// build 26200 真实 app 双向实测：0xF 时 SGR wheel report
/// （`\x1b[<64/65;x;yM`）完整写入 ConPTY stdin 但 claude 无反应，去掉 0x8
/// 后滚轮恢复，输出流畅度无肉眼可见退化。疑似机制：passthrough 下 conhost
/// 不解析子进程输出、不跟踪 DECSET 1000/1002/1006 mouse mode
/// （microsoft/terminal#376、PR #9970）——但**最小复现实验失败**：node
/// 直接子进程（DECSET 1002/1003/1006 + alt buffer + 60fps 负载）在 0xF 下
/// stdin 的 SGR report 仍原样透传，阻断条件仅真实 claude 场景
/// （pwsh→claude 进程树 + kitty 协议）复现。因此任何 0x8 启用/默认矩阵位
/// 翻转必须实测真实 claude 滚轮（人工实测门禁 +
/// `conpty_flags_default_matrix_matches_legacy_tristate` 守卫用例绿），
/// 勿以最小实验/单测绿为依据。
pub fn compute_conpty_flags(build_number: u32, bundled: bool, modes: &ConptyInputModes) -> u32 {
    let mut flags = if modes.inherit_cursor {
        FLAG_INHERIT_CURSOR
    } else {
        0
    };
    if modes.resize_quirk {
        flags |= FLAG_RESIZE_QUIRK;
    }
    if modes.win32_input_mode && (bundled || build_number >= CONPTY_WIN11_MIN_BUILD) {
        flags |= FLAG_WIN32_INPUT_MODE;
    }
    // 末行矩阵位：默认矩阵恒 false——启用须过人工实测门禁。
    if modes.passthrough_mode {
        flags |= FLAG_PASSTHROUGH_MODE;
    }
    flags
}

// ─── SPAWN_LOCK 串行化与会话容量上限 ───

/// BE-01：PTY 会话总数上限——防止会话无上限堆积耗尽 ConPTY/进程句柄。
pub const MAX_PTY_SESSIONS: usize = 32;

/// 并发 spawn 会卡死 ConPTY 输出管道——create + spawn 段整段持锁（BE-12
/// 锁界）。容量判定与占用登记在同一把锁内原子化，防并发超发。
static SPAWN_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// 活跃 PTY 会话计数（`spawn_locked` 成功 +1，`Pty::drop` 经
/// `session_closed` -1）。
static ACTIVE_PTY_SESSIONS: AtomicUsize = AtomicUsize::new(0);

/// BE-01：会话上限判定纯函数——active 达到 `MAX_PTY_SESSIONS` 即拒绝。
pub fn ensure_pty_capacity(active: usize) -> Result<(), Error> {
    if active >= MAX_PTY_SESSIONS {
        return Err(Error::other(format!(
            "PTY 会话数已达上限 {MAX_PTY_SESSIONS}，请先关闭部分终端"
        )));
    }
    Ok(())
}

/// `tty::windows::new` 的实际入口：`SPAWN_LOCK` 持锁 → 锁内容量判定 →
/// `conpty::new`（create + spawn 全在锁内）→ 成功后占用登记。容量命中时
/// ConPTY 尚未创建，没有要 kill 的半成品；创建失败计数不变。
pub(super) fn spawn_locked(config: &Options, window_size: WindowSize) -> Result<Pty, Error> {
    let _lock = SPAWN_LOCK.lock();
    ensure_pty_capacity(ACTIVE_PTY_SESSIONS.load(Ordering::SeqCst))?;
    let pty = conpty::new(config, window_size)?;
    ACTIVE_PTY_SESSIONS.fetch_add(1, Ordering::SeqCst);
    Ok(pty)
}

/// `Pty::drop` 的计数归还（与 `spawn_locked` 的占用登记配对）。
pub(super) fn session_closed() {
    ACTIVE_PTY_SESSIONS.fetch_sub(1, Ordering::SeqCst);
}

// ─── Job Object 孤儿防护 ───

/// Windows Job Object 句柄 RAII 包装。
///
/// 持有 `HANDLE` 以阻止 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 在 PTY 会话
/// 期间触发；Drop 时 `CloseHandle`——会话销毁路径上子进程树由 OS 兜底
/// 清空（父崩溃路径同理由 OS 回收句柄触发同一语义）。
pub struct JobHandle(HANDLE);

impl Drop for JobHandle {
    fn drop(&mut self) {
        // SAFETY: CloseHandle 可从任意线程安全调用，即使句柄无效也仅返回 FALSE。
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// SAFETY: HANDLE 在 Win32 中可跨线程传递；CloseHandle 可从任意线程安全调用。
unsafe impl Send for JobHandle {}
unsafe impl Sync for JobHandle {}

/// Windows Job Object——将子进程与父进程生命周期绑定，防止孤儿进程。
///
/// 紧跟子进程创建之后调用；返回的 `JobHandle` 须在 PTY 会话存活期间持有。
/// 失败即致命（孤儿防护不能静默丢失）——调用方负责终止已 spawn 的子进程。
pub(super) fn add_to_job_object(pid: u32) -> Result<JobHandle, Error> {
    use std::os::windows::ffi::OsStrExt;

    let job_name = job_name(pid);
    let job_name_wide: Vec<u16> = std::ffi::OsStr::new(&job_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: `pid` 来自刚创建的有效子进程；`job_name_wide` 是以 NUL 结尾的
    // 宽字符串，指针在调用期间有效；返回句柄由 RAII 负责 CloseHandle。
    unsafe { create_and_assign_job(pid, &job_name_wide) }
}

/// 构造 Job Object 名称（纯函数）——`slterm_pty_{pid}`，按子进程 PID 唯一。
fn job_name(pid: u32) -> String {
    format!("slterm_pty_{pid}")
}

/// 构造带 KILL_ON_JOB_CLOSE 的扩展限制信息（纯函数）。
///
/// 锁死项：`LimitFlags` 必须包含 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`（父
/// 进程退出时 OS 自动杀所有子进程——孤儿防护核心）。测试断言具体值
/// 0x2000 防未来误删。
fn job_limits() -> JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    // SAFETY: 全零初始化对 POD 的 JOBOBJECT_EXTENDED_LIMIT_INFORMATION 合法，
    // 等价旧栈 ::default() 形态（windows-sys 不派生 Default）。
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    limits
}

/// 创建 Job Object 并设置 KILL_ON_JOB_CLOSE，将子进程分配进去。
///
/// # Safety
///
/// - `pid` 必须是刚创建且仍有效的子进程 ID。
/// - `job_name_wide` 必须是以 NUL 结尾的 UTF-16 宽字符串，调用期间内存有效。
/// - 返回的 `JobHandle` 必须在 PTY 会话存活期间持有，以防
///   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 过早触发。
unsafe fn create_and_assign_job(pid: u32, job_name_wide: &[u16]) -> Result<JobHandle, Error> {
    // SAFETY: job_name_wide 由调用方保证 NUL 结尾且有效。
    let job = unsafe { CreateJobObjectW(std::ptr::null(), job_name_wide.as_ptr()) };
    if job.is_null() {
        return Err(Error::last_os_error());
    }
    // 任一后续步骤失败：job 句柄经 JobHandle RAII 释放，不泄漏。
    let job = JobHandle(job);

    let limits = job_limits();
    // SAFETY: limits 指向有效的 JOBOBJECT_EXTENDED_LIMIT_INFORMATION，尺寸匹配。
    let ok = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if ok == 0 {
        return Err(Error::last_os_error());
    }

    // SAFETY: OpenProcess 打开调用方保证有效的子进程。
    let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
    if process.is_null() {
        return Err(Error::last_os_error());
    }
    // SAFETY: job/process 均为有效句柄；process 仅用于本次指派，随后立即释放。
    let ok = unsafe { AssignProcessToJobObject(job.0, process) };
    unsafe {
        let _ = CloseHandle(process);
    }
    if ok == 0 {
        return Err(Error::last_os_error());
    }

    Ok(job)
}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    // ─── flags 能力矩阵守卫族 ───
    //
    // 默认矩阵注入下 7 条三态用例（期望输出与旧三态恒等，零默认漂移）：
    // 捆绑恒 0x7（新 conhost 完整支持 0x4）；系统按 build 分叉（老 Win10
    // 0x3 为回退路径）；默认矩阵回归守卫：默认组合不含 PASSTHROUGH_MODE
    // 0x8（passthrough 吞 terminal→child 的 SGR mouse report），0x8 仅显式
    // passthrough_mode=true 才置位（须过人工门禁）。

    #[test]
    fn flags_win10_19041_system_returns_0x3() {
        assert_eq!(
            compute_conpty_flags(19041, false, &ConptyInputModes::default()),
            0x3
        );
    }

    #[test]
    fn flags_below_threshold_21375_system_returns_0x3() {
        assert_eq!(
            compute_conpty_flags(21375, false, &ConptyInputModes::default()),
            0x3
        );
    }

    #[test]
    fn flags_threshold_21376_system_returns_0x7() {
        assert_eq!(
            compute_conpty_flags(21376, false, &ConptyInputModes::default()),
            0x7
        );
    }

    #[test]
    fn flags_win11_21h2_system_returns_0x7() {
        assert_eq!(
            compute_conpty_flags(22000, false, &ConptyInputModes::default()),
            0x7
        );
    }

    #[test]
    fn flags_win11_24h2_system_returns_0x7() {
        assert_eq!(
            compute_conpty_flags(26100, false, &ConptyInputModes::default()),
            0x7
        );
    }

    #[test]
    fn flags_win10_bundled_returns_0x7() {
        assert_eq!(
            compute_conpty_flags(19041, true, &ConptyInputModes::default()),
            0x7
        );
    }

    /// 防复发主用例：三输入 × 默认矩阵 → 与旧三态恒等（0x7/0x7/0x3）——
    /// 默认矩阵零漂移守卫；任何默认矩阵位翻转（含 0x8 默认置位）此处即红。
    #[test]
    fn conpty_flags_default_matrix_matches_legacy_tristate() {
        assert_eq!(
            compute_conpty_flags(19041, true, &ConptyInputModes::default()),
            0x7
        );
        assert_eq!(
            compute_conpty_flags(26100, false, &ConptyInputModes::default()),
            0x7
        );
        assert_eq!(
            compute_conpty_flags(19041, false, &ConptyInputModes::default()),
            0x3
        );
    }

    #[test]
    fn conpty_flags_passthrough_mode_adds_0x8() {
        let modes = ConptyInputModes {
            passthrough_mode: true,
            ..ConptyInputModes::default()
        };
        assert_eq!(compute_conpty_flags(26100, false, &modes), 0x7 | 0x8);
    }

    #[test]
    fn conpty_flags_win32_input_still_gated_by_build() {
        // win32_input_mode=true + 老 Win10 回退（系统 conhost）→ 0x4 不置位
        //（门控维持 bundled || build >= 21376，矩阵位不能绕过）。
        let modes = ConptyInputModes {
            win32_input_mode: true,
            ..ConptyInputModes::default()
        };
        assert_eq!(
            compute_conpty_flags(19041, false, &modes),
            0x3,
            "老 Win10 回退下 0x4 不得置位"
        );
    }

    #[test]
    fn flag_constants_match_win32_values() {
        assert_eq!(FLAG_INHERIT_CURSOR, 0x1);
        assert_eq!(FLAG_RESIZE_QUIRK, 0x2);
        assert_eq!(FLAG_WIN32_INPUT_MODE, 0x4);
        assert_eq!(FLAG_PASSTHROUGH_MODE, 0x8);
    }

    // ─── BE-01 会话容量上限 ───

    #[test]
    fn pty_capacity_below_limit_passes() {
        ensure_pty_capacity(0).expect("空会话应放行");
        ensure_pty_capacity(MAX_PTY_SESSIONS - 1).expect("上限内应放行");
    }

    #[test]
    fn pty_capacity_at_limit_rejected() {
        let err = ensure_pty_capacity(MAX_PTY_SESSIONS).expect_err("达到上限应拒绝");
        assert!(
            err.to_string().contains("32"),
            "错误消息应含上限值，实际: {err}"
        );
    }

    #[test]
    fn pty_capacity_above_limit_rejected() {
        assert!(ensure_pty_capacity(MAX_PTY_SESSIONS + 1).is_err());
        assert!(ensure_pty_capacity(usize::MAX).is_err());
    }

    /// SPAWN_LOCK 串行语义：复刻 `spawn_locked` 的锁内闭环（容量判定 +
    /// 占用登记），上限 -1 时两线程并发进入 → 恰一成一败不超发——串行
    /// 语义由 SPAWN_LOCK 互斥保证（进程真跑集成层归 M2.4）。
    #[test]
    fn spawn_lock_serializes_capacity_check_and_reservation() {
        use std::sync::{Arc, Barrier};

        let prev = ACTIVE_PTY_SESSIONS.swap(MAX_PTY_SESSIONS - 1, Ordering::SeqCst);
        let barrier = Arc::new(Barrier::new(2));
        let mut results = Vec::new();
        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for _ in 0..2 {
                let barrier = Arc::clone(&barrier);
                handles.push(s.spawn(move || {
                    barrier.wait();
                    let _lock = SPAWN_LOCK.lock();
                    ensure_pty_capacity(ACTIVE_PTY_SESSIONS.load(Ordering::SeqCst)).map(|()| {
                        ACTIVE_PTY_SESSIONS.fetch_add(1, Ordering::SeqCst);
                    })
                }));
            }
            for handle in handles {
                results.push(handle.join().unwrap());
            }
        });
        let succeeded = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(succeeded, 1, "上限-1 并发进入：恰一个成功");
        assert_eq!(ACTIVE_PTY_SESSIONS.load(Ordering::SeqCst), MAX_PTY_SESSIONS);
        ACTIVE_PTY_SESSIONS.store(prev, Ordering::SeqCst);
    }

    // ─── Job Object 纯函数族 ───

    #[test]
    fn job_name_format_contains_pid() {
        assert_eq!(job_name(1234), "slterm_pty_1234");
        assert_eq!(job_name(0), "slterm_pty_0");
    }

    #[test]
    fn job_limits_contains_kill_on_job_close() {
        let limits = job_limits();
        assert_eq!(
            limits.BasicLimitInformation.LimitFlags, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            "LimitFlags 应包含 KILL_ON_JOB_CLOSE"
        );
        assert_eq!(
            limits.BasicLimitInformation.LimitFlags, 0x2000,
            "KILL_ON_JOB_CLOSE 值应锁死为 0x2000（防误删孤儿防护）"
        );
    }

    #[test]
    fn job_handle_drop_closes_handle() {
        use windows_sys::Win32::Foundation::GetHandleInformation;

        // SAFETY: CreateJobObjectW 无名 Job；句柄由 JobHandle RAII 管理。
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        assert!(!job.is_null(), "CreateJobObjectW 应成功");
        {
            let _jh = JobHandle(job);
        } // 此处 drop → CloseHandle
        let mut flags: u32 = 0;
        // SAFETY: job 已关闭，GetHandleInformation 应失败（返回 0）。
        let res = unsafe { GetHandleInformation(job, &mut flags) };
        assert_eq!(res, 0, "drop 后句柄应已关闭（GetHandleInformation 应失败）");
    }

    #[test]
    fn job_handle_invalid_handle_drop_no_panic() {
        let jh = JobHandle(windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE);
        drop(jh);
    }
}
