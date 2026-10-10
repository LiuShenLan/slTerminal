use log::{info, warn};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{Error, Result};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::{mem, ptr};

use windows_sys::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, FreeLibrary, HANDLE, HMODULE, S_OK,
};
use windows_sys::Win32::Globalization::{
    CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN, CompareStringOrdinal,
};
use windows_sys::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::core::{HRESULT, PWSTR};
use windows_sys::s;

use windows_sys::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    STARTUPINFOW, TerminateProcess, UpdateProcThreadAttribute,
};

use crate::event::{OnResize, WindowSize};
use crate::tty::Options;
use crate::tty::windows::blocking::{UnblockedReader, UnblockedWriter};
use crate::tty::windows::child::ChildExitWatcher;
use crate::tty::windows::spawn::{self, FLAG_WIN32_INPUT_MODE, compute_conpty_flags};
use crate::tty::windows::{Pty, cmdline, win32_string};

const PIPE_CAPACITY: usize = crate::event_loop::READ_BUFFER_SIZE;

/// Win10/Win11 分界 build：捆绑判定（`should_bundle`）与 flags 0x4 门控
/// （`compute_conpty_flags`）、DSR 毒链判定（`conhost_input_corrupts_cpr`）
/// 共用单常量——同为 ConPTY 兼容分界，Win10/Win11 分叉同源。
pub(crate) const CONPTY_WIN11_MIN_BUILD: u32 = 21376;

// NuGet `Microsoft.Windows.Console.ConPTY` 1.24.260710001 官方构建（来源与
// 哈希见 vendor/conpty/README.md）。`include_bytes!` 嵌入保持单文件 exe
// 发布形态；仅 Win10（build < 21376）且 exe 旁文件对缺失时提取到
// %LOCALAPPDATA% 加载（两阶段加载链，见 ConptyApi::new）。
const CONPTY_DLL_BYTES: &[u8] = include_bytes!("../../../vendor/conpty/conpty.dll");
const OPENCONSOLE_EXE_BYTES: &[u8] = include_bytes!("../../../vendor/conpty/OpenConsole.exe");

/// ConPTY 后端状态（回退可观测；壳 toast 降级提示数据源，消费归 09 篇）。
///
/// `fallback_reason` 与 warn 日志同一来源变量——日志与查询面零漂移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConptyStatus {
    /// 本次是否尝试了侧载加载（`Options::conpty_sideload` 置位时）。
    pub attempted: bool,
    /// 实际是否走侧载 conhost（OpenConsole）。
    pub bundled: bool,
    /// 回退原因（attempted && !bundled 时有值）。
    pub fallback_reason: Option<String>,
}

/// 状态记录槽：每次 `ConptyApi::new`（即每次 Pty 创建）覆盖记录——观测
/// 「当前生效后端」，非进程级一次性快照（同一进程内不同 Pty 可能落到
/// 不同后端，如首次提取失败后 Defender 放行再试成功）。
static STATUS: parking_lot::Mutex<ConptyStatus> = parking_lot::Mutex::new(ConptyStatus {
    attempted: false,
    bundled: false,
    fallback_reason: None,
});

fn record_status(status: ConptyStatus) {
    *STATUS.lock() = status;
}

/// 查询 ConPTY 后端状态（Win10 回退可观测）。
pub fn conpty_status() -> ConptyStatus {
    STATUS.lock().clone()
}

/// 决策纯函数：仅 Win10（build < 21376）尝试 NuGet 捆绑提取。
pub(crate) fn should_bundle(build_number: u32) -> bool {
    build_number < CONPTY_WIN11_MIN_BUILD
}

/// 决策纯函数：Win10 家族（build < 21376，含捆绑与回退路径）conhost 键事件
/// 输入模式会把 CPR 应答（CSI 1;1R）解析为 F3 键（PSReadLine CharacterSearch
/// 吞掉下一个输入字符）——该传输层上 CPR 字节写入 stdin 即是毒（其他位置
/// 形态被键事件引擎丢弃，应用永远拿不到真值）。DSR 查询在此类主机上只能
/// 剥离不答；Win11+ inbox conhost 传输正常，交 Term 以真实光标自答。
/// 阈值与 `should_bundle` 同源（Win10/Win11 分界），语义独立不复用其名。
pub(crate) fn conhost_input_corrupts_cpr(build_number: u32) -> bool {
    build_number < CONPTY_WIN11_MIN_BUILD
}

/// 提取目标目录：`%LOCALAPPDATA%\slterm\conpty`（纯路径构造，便于测试注入）。
pub(crate) fn extraction_dir_from(localappdata: &Path) -> PathBuf {
    localappdata.join("slterm").join("conpty")
}

/// 幂等提取：已存在且大小与嵌入一致 → 复用；否则覆盖重写（vendor 升级自愈）。
pub(crate) fn ensure_extracted(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_if_size_differs(&dir.join("conpty.dll"), CONPTY_DLL_BYTES)?;
    write_if_size_differs(&dir.join("OpenConsole.exe"), OPENCONSOLE_EXE_BYTES)?;
    Ok(())
}

/// 大小一致跳过写入；缺失或大小不一致时覆盖（嵌入内容编译期固定，大小
/// 判定足够）。
fn write_if_size_differs(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let matches = std::fs::metadata(path)
        .map(|m| m.len() == bytes.len() as u64)
        .unwrap_or(false);
    if !matches {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

/// Load the pseudoconsole API from conpty.dll if possible, otherwise use the
/// standard Windows API.
///
/// The bundled conpty.dll (provenance: THIRD-PARTY-NOTICES)
/// supports loading OpenConsole.exe, which offers many improvements and
/// bugfixes compared to the standard conpty that ships with Windows.
///
/// The conpty.dll and OpenConsole.exe files will be searched in PATH and in
/// the directory where Slterm's executable is located.
type CreatePseudoConsoleFn =
    unsafe extern "system" fn(COORD, HANDLE, HANDLE, u32, *mut HPCON) -> HRESULT;
type ResizePseudoConsoleFn = unsafe extern "system" fn(HPCON, COORD) -> HRESULT;
type ClosePseudoConsoleFn = unsafe extern "system" fn(HPCON);

struct ConptyApi {
    create: CreatePseudoConsoleFn,
    resize: ResizePseudoConsoleFn,
    close: ClosePseudoConsoleFn,
    /// Whether these entry points come from the side-loaded conpty.dll
    /// (bundled OpenConsole) rather than the in-box kernel32 ConPTY —
    /// gates the DA1 handshake priming in `new`.
    sideloaded: bool,
    // Keep the loaded code alive through ClosePseudoConsole, including when
    // several PTYs share the same DLL. Each LoadLibrary owns one reference.
    _library: Option<ConptyLibrary>,
}

struct ConptyLibrary(HMODULE);

impl Drop for ConptyLibrary {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}

impl ConptyApi {
    /// `sideload` 来自 `Options.conpty_sideload`（壳读 settings 键注入，
    /// 键域归 06 篇；core 不内嵌设置文件读取——上游读 slterm_settings.txt
    /// 的 `openconsole=off` 形态不迁）。`build_number` 由
    /// `get_windows_build_number` 注入，决定 Win10 提取臂是否启用。
    ///
    /// 两阶段加载链（Win10 兼容全链）：
    /// ① exe 旁完整文件对（`runtime/` 或 exe 目录）——认证边界：完整对 +
    ///    绝对路径 `LoadLibraryW`，PATH 同名文件不进边界；
    /// ② 文件对缺失且 Win10：`include_bytes!` NuGet 捆绑 → `%LOCALAPPDATA%`
    ///    幂等提取 → 提取目录按同一边界加载（提取失败静默回退，原因记入
    ///    `ConptyStatus::fallback_reason`）；
    /// ③ 终回退：系统 CreatePseudoConsole/Resize/Close 函数指针。
    ///
    /// Side-by-side conpty.dll + OpenConsole.exe 是默认期望路径：捆绑 host
    /// 避免了 in-box ConPTY 的 resize 视口重发怪癖与老 Win10 的鼠标转发
    /// 缺失；关闭侧载换来更快 pane spawn（未签名 exe 付 Defender 实时扫描
    /// 税），代价是只能靠自身 resize 合并保持 TUI 干净。
    fn new(sideload: bool, build_number: u32) -> Self {
        if sideload {
            match Self::load_sideloaded(build_number) {
                Ok(conpty) => {
                    info!("Using conpty.dll (OpenConsole) for pseudoconsole");
                    record_status(ConptyStatus {
                        attempted: true,
                        bundled: true,
                        fallback_reason: None,
                    });
                    return conpty;
                }
                Err(reason) => {
                    // warn 文案与 fallback_reason 同一变量——日志与状态同源零漂移。
                    warn!("conpty sideload failed, falling back to in-box ConPTY: {reason}");
                    record_status(ConptyStatus {
                        attempted: true,
                        bundled: false,
                        fallback_reason: Some(reason),
                    });
                }
            }
        } else {
            record_status(ConptyStatus {
                attempted: false,
                bundled: false,
                fallback_reason: None,
            });
        }
        info!("Using Windows API for pseudoconsole");
        Self {
            create: CreatePseudoConsole,
            resize: ResizePseudoConsole,
            close: ClosePseudoConsole,
            sideloaded: false,
            _library: None,
        }
    }

    /// 侧载加载：① exe 旁完整文件对 → ② Win10 且缺失时 NuGet 提取 →
    /// 同一认证边界（完整对 + 绝对路径）加载。Err 载荷 = 回退原因。
    fn load_sideloaded(build_number: u32) -> std::result::Result<Self, String> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
            .ok_or_else(|| "current_exe 目录解析失败".to_owned())?;
        if let Some(dir) = bundled_conpty_dir(&exe_dir) {
            return Self::load_conpty(&dir);
        }
        if should_bundle(build_number) {
            let localappdata = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .ok_or_else(|| "LOCALAPPDATA 环境变量缺失".to_owned())?;
            let dir = extraction_dir_from(&localappdata);
            ensure_extracted(&dir).map_err(|e| format!("NuGet 捆绑提取失败: {e}"))?;
            // 提取目录经同一文件对检查加载（runtime/ 子目录不存在的形态由
            // bundled_conpty_dir 第二候选覆盖）。
            if let Some(dir) = bundled_conpty_dir(&dir) {
                return Self::load_conpty(&dir);
            }
            return Err("提取后完整文件对仍缺失".to_owned());
        }
        Err("complete conpty.dll/OpenConsole.exe pair not found in runtime/ or executable directory"
            .to_owned())
    }

    /// Try loading ConptyApi from conpty.dll library.
    ///
    /// `conpty_dir` 必须是已通过 `bundled_conpty_dir` 完整文件对检查的目录
    /// ——DLL 始终按绝对路径加载，不能让 PATH 中的同名文件进入认证边界。
    fn load_conpty(conpty_dir: &Path) -> std::result::Result<Self, String> {
        type LoadedFn = unsafe extern "system" fn() -> isize;

        let dll_path = conpty_dir.join("conpty.dll");
        let dll_wide: Vec<u16> = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let hmodule = LoadLibraryW(dll_wide.as_ptr());
            if hmodule.is_null() {
                return Err(format!("LoadLibraryW 失败: {}", dll_path.display()));
            }
            let library = ConptyLibrary(hmodule);
            let create_fn = GetProcAddress(hmodule, s!("CreatePseudoConsole"));
            let resize_fn = GetProcAddress(hmodule, s!("ResizePseudoConsole"));
            let close_fn = GetProcAddress(hmodule, s!("ClosePseudoConsole"));
            let (Some(create_fn), Some(resize_fn), Some(close_fn)) =
                (create_fn, resize_fn, close_fn)
            else {
                return Err(format!(
                    "GetProcAddress 三符号解析失败: {}",
                    dll_path.display()
                ));
            };

            Ok(Self {
                create: mem::transmute::<LoadedFn, CreatePseudoConsoleFn>(create_fn),
                resize: mem::transmute::<LoadedFn, ResizePseudoConsoleFn>(resize_fn),
                close: mem::transmute::<LoadedFn, ClosePseudoConsoleFn>(close_fn),
                sideloaded: true,
                _library: Some(library),
            })
        }
    }
}

fn bundled_conpty_dir(exe_dir: &Path) -> Option<PathBuf> {
    [exe_dir.join("runtime"), exe_dir.to_path_buf()]
        .into_iter()
        .find(|dir| dir.join("conpty.dll").is_file() && dir.join("OpenConsole.exe").is_file())
}

/// RAII Pseudoconsole.
pub struct Conpty {
    pub handle: HPCON,
    api: ConptyApi,
}

/// 终端能力固定注入清单（`convert_custom_env` 末尾叠加，只补缺不覆盖）。
/// `TERM_PROGRAM=slterm` 与 `tty::setup_env` 的进程级注入同值；产品定位
/// 不做 terminfo 探测，三项定死。
const FIXED_ENV_INJECTIONS: &[(&str, &str)] = &[
    ("TERM", "xterm-256color"),
    ("COLORTERM", "truecolor"),
    ("TERM_PROGRAM", "slterm"),
];

impl Drop for Conpty {
    fn drop(&mut self) {
        // XXX: This will block until the conout pipe is drained. Will cause a deadlock if the
        // conout pipe has already been dropped by this point.
        //
        // See https://docs.microsoft.com/en-us/windows/console/closepseudoconsole.
        unsafe { (self.api.close)(self.handle) }
    }
}

// The ConPTY handle can be sent between threads.
unsafe impl Send for Conpty {}

pub fn new(config: &Options, window_size: WindowSize) -> Result<Pty> {
    // build 号获取失败回退 0 → 按老 Win10 处置（尝试捆绑、0x4 不置位、
    // DSR 剥离）——安全侧降级，与 should_bundle 同源语义。
    let build_number = super::get_windows_build_number().unwrap_or_else(|error| {
        warn!("get_windows_build_number failed ({error}); assuming pre-21376 build");
        0
    });

    let api = ConptyApi::new(config.conpty_sideload, build_number);
    crate::pty_trace(if api.sideloaded {
        "conpty api ready (sideloaded OpenConsole)"
    } else {
        "conpty api ready (in-box)"
    });
    let mut pty_handle: HPCON = 0;

    // Passing 0 as the size parameter allows the "system default" buffer
    // size to be used. There may be small performance and memory advantages
    // to be gained by tuning this in the future, but it's likely a reasonable
    // start point.
    let (conout, conout_pty_handle) = miow::pipe::anonymous(0)?;
    let (conin_pty_handle, mut conin) = miow::pipe::anonymous(0)?;

    // Prime the side-loaded OpenConsole's startup handshake: at boot it sends
    // a DA1 query and holds parts of its init until the terminal answers.
    // Pre-writing the DA1 response into the input pipe — before the
    // host is even spawned, so it's the first thing it reads — makes the
    // handshake instant instead of a per-pane wait; the same trick
    // Microsoft's own ConPTY tests use. ONLY for the bundled host: the
    // in-box ConPTY may not consume an unsolicited report and would leak
    // the response into the shell as typed input.
    //
    // 应答身份统一 `?64;22c`（D02-1 裁决：priming / Term 自答 / reader 代答
    // 三处同身份；M2.4 真机复测钉死）。
    if api.sideloaded {
        use std::io::Write;
        let _ = conin.write_all(b"\x1b[?64;22c");
        crate::pty_trace("DA1 response primed");
    }

    // flags 能力矩阵：0x1/0x2 直取矩阵位；0x4（WIN32_INPUT_MODE）按
    // `bundled || build >= 21376` 门控；0x8 默认恒关。ConPTY 把 CSI ... _
    // 记录解码为原生 INPUT_RECORD——Codex Shift+Enter 等功能键依赖 0x4，
    // 普通字符输入仍走常规 UTF-8 路径。E_INVALIDARG 重试臂保留为「矩阵判了
    // 0x4 但系统 host 拒」的兜底（preview build 场景）：无 0x4 建出的 host
    // 永不发 DECSET 9001，终端永不置 WIN32_INPUT_MODE，编码器恒走传统 VT
    // 路径——降级端到端自门控。
    let flags = compute_conpty_flags(build_number, api.sideloaded, &config.conpty_input_modes);
    let conin_handle = conin_pty_handle.as_raw_handle() as HANDLE;
    let conout_handle = conout_pty_handle.as_raw_handle() as HANDLE;
    let mut result = unsafe {
        (api.create)(
            window_size.into(),
            conin_handle,
            conout_handle,
            flags,
            &mut pty_handle as *mut _,
        )
    };
    if result != S_OK && flags & FLAG_WIN32_INPUT_MODE != 0 {
        warn!(
            "CreatePseudoConsole rejected win32 input mode (HRESULT {result:#x}); \
             retrying in legacy VT input mode"
        );
        crate::pty_trace("CreatePseudoConsole win32-input rejected; legacy retry");
        result = unsafe {
            (api.create)(
                window_size.into(),
                conin_handle,
                conout_handle,
                flags & !FLAG_WIN32_INPUT_MODE,
                &mut pty_handle as *mut _,
            )
        };
    }
    crate::pty_trace("CreatePseudoConsole done");

    // ConPTY duplicates these handles; it does not take ownership of ours.
    // Retaining our output writer prevents EOF even after the host exits,
    // stranding both the reader/drain threads and their bounded pipe buffer.
    drop(conin_pty_handle);
    drop(conout_pty_handle);

    if result != S_OK {
        return Err(Error::other(format!(
            "CreatePseudoConsole failed: HRESULT {result:#x}"
        )));
    }

    let mut conout = UnblockedReader::new(conout, PIPE_CAPACITY);
    // Declared after conout so every error closes the host while its reader
    // still exists. Spawn failures need the same drain-before-close order as Pty.
    let conpty = Conpty {
        handle: pty_handle,
        api,
    };
    let child_watcher = match spawn_shell(config, &conpty) {
        Ok(watcher) => watcher,
        Err(error) => {
            conout.drain_detached();
            return Err(error);
        }
    };
    let conin = UnblockedWriter::new(conin, PIPE_CAPACITY);

    // Job Object 指派（KILL_ON_JOB_CLOSE 孤儿防护）——紧跟 spawn 之后、任何
    // 提前返回之前。指派失败即致命：孤儿防护不能静默丢失，先杀已 spawn 的
    // 子进程再走与 spawn 失败相同的 drain-before-close 收尾。
    let job = match child_watcher.pid() {
        Some(pid) => match spawn::add_to_job_object(pid.get()) {
            Ok(job) => Some(job),
            Err(error) => {
                unsafe {
                    TerminateProcess(child_watcher.raw_handle(), 0);
                }
                conout.drain_detached();
                return Err(error);
            }
        },
        None => None,
    };

    let strip_dsr = conhost_input_corrupts_cpr(build_number);
    Ok(Pty::new(
        conpty,
        conout,
        conin,
        child_watcher,
        job,
        strip_dsr,
    ))
}

fn spawn_shell(config: &Options, conpty: &Conpty) -> Result<ChildExitWatcher> {
    let mut attributes = ProcThreadAttributes::new()?;
    let mut startup_info_ex: STARTUPINFOEXW = unsafe { mem::zeroed() };
    startup_info_ex.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
    // Null standard handles and disabled inheritance keep parent handles private.
    startup_info_ex.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup_info_ex.lpAttributeList = attributes.as_mut_ptr();

    // Set thread attribute list's Pseudo Console to the specified ConPTY.
    unsafe {
        let success = UpdateProcThreadAttribute(
            startup_info_ex.lpAttributeList,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            conpty.handle as *mut std::ffi::c_void,
            mem::size_of::<HPCON>(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) > 0;

        if !success {
            return Err(Error::last_os_error());
        }
    }

    // Prepare child process creation arguments.
    let mut cmdline = win32_string(&cmdline(config));
    let cwd = config.working_directory.as_ref().map(win32_string);
    let mut creation_flags = EXTENDED_STARTUPINFO_PRESENT;
    let child_config = super::cmd_prompt::prepare(config);
    let custom_env_block = convert_custom_env(&child_config.env, child_config.env_is_complete);
    let custom_env_block_pointer = match &custom_env_block {
        Some(custom_env_block) => {
            creation_flags |= CREATE_UNICODE_ENVIRONMENT;
            custom_env_block.as_ptr() as *mut std::ffi::c_void
        }
        None => ptr::null_mut(),
    };

    let mut proc_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    crate::pty_trace("CreateProcessW (shell attach) begin");
    unsafe {
        let success = CreateProcessW(
            ptr::null(),
            cmdline.as_mut_ptr() as PWSTR,
            ptr::null_mut(),
            ptr::null_mut(),
            false as i32,
            creation_flags,
            custom_env_block_pointer,
            cwd.as_ref().map_or_else(ptr::null, |s| s.as_ptr()),
            &mut startup_info_ex.StartupInfo as *mut STARTUPINFOW,
            &mut proc_info as *mut PROCESS_INFORMATION,
        ) > 0;

        if !success {
            return Err(Error::last_os_error());
        }
    }
    crate::pty_trace("CreateProcessW (shell attach) done");

    // CreateProcess returns two caller-owned handles, even though the primary
    // thread needs no further interaction. The watcher owns the process handle.
    let process = unsafe { OwnedHandle::from_raw_handle(proc_info.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(proc_info.hThread) };
    drop(thread);
    ChildExitWatcher::new(process)
}

struct ProcThreadAttributes {
    // The opaque native list requires pointer alignment, not byte alignment.
    storage: Box<[usize]>,
}

impl ProcThreadAttributes {
    fn new() -> Result<Self> {
        let mut size = 0;
        let success =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size) };
        let error = Error::last_os_error();
        if success != 0 || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            return Err(error);
        }
        let mut storage = vec![0_usize; size.div_ceil(mem::size_of::<usize>())].into_boxed_slice();
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut size)
        } == 0
        {
            return Err(Error::last_os_error());
        }
        Ok(Self { storage })
    }

    fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}

impl Drop for ProcThreadAttributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
}

// Windows environment variables are case-insensitive, and the caller is responsible for
// deduplicating environment variables, so do that here while converting.
//
// https://learn.microsoft.com/en-us/previous-versions/troubleshoot/windows/win32/createprocess-cannot-eliminate-duplicate-variables#environment-variables
fn convert_custom_env(
    custom_env: &HashMap<String, String>,
    env_is_complete: bool,
) -> Option<Vec<u16>> {
    // Windows inherits parent's env when no `lpEnvironment` parameter is specified.
    if custom_env.is_empty() && !env_is_complete {
        return None;
    }

    let mut environment = Vec::new();
    for (custom_key, custom_value) in custom_env {
        environment.push(PendingEnvironmentVariable::new(
            OsStr::new(custom_key),
            OsStr::new(custom_value),
        ));
    }

    if !env_is_complete {
        // 终端能力固定注入（custom 之后、继承之前，只补缺不覆盖——custom
        // 同名键优先）：TERM/COLORTERM 宣告 256 色与 truecolor，TERM_PROGRAM
        // 标识终端身份；AI CLI 依赖此宣告启用全色（SLTERM_PANE_ID 等变量族
        // 由壳组装进 custom_env，归 03 篇定义，core 只负责注入时机）。
        // env_is_complete 不叠加：完整块来自会话持久化快照（快照自身已含
        // 这些键），一字不差恢复是 complete 的契约。
        for (key, value) in FIXED_ENV_INJECTIONS {
            if !custom_env.keys().any(|k| k.eq_ignore_ascii_case(key)) {
                environment.push(PendingEnvironmentVariable::new(
                    OsStr::new(key),
                    OsStr::new(value),
                ));
            }
        }

        // Pull the current process environment after, to avoid overwriting the user provided one.
        for (inherited_key, inherited_value) in std::env::vars_os() {
            environment.push(PendingEnvironmentVariable::new(
                &inherited_key,
                &inherited_value,
            ));
        }
    }

    // CreateProcess 要求按大小写不敏感的 Unicode 顺序排列。稳定排序还会让自定义
    // 项在同名继承项之前，从而保住覆盖优先级。
    environment.sort_by(|left, right| compare_environment_names(&left.key_wide, &right.key_wide));

    let mut converted_block = Vec::new();
    let mut previous_key = None::<Vec<u16>>;
    for variable in environment {
        if previous_key.as_ref().is_some_and(|previous| {
            compare_environment_names(previous, &variable.key_wide) == Ordering::Equal
        }) {
            warn!(
                "Omitting environment variable pair with duplicate key: '{}={}'",
                variable.key.to_string_lossy(),
                variable.value.to_string_lossy()
            );
            continue;
        }
        add_windows_env_key_value_to_block(&mut converted_block, &variable.key, &variable.value);
        previous_key = Some(variable.key_wide);
    }

    // 即使调用方有意传空环境，环境块也必须以双 NUL 结尾。
    if converted_block.is_empty() {
        converted_block.push(0);
    }
    converted_block.push(0);
    Some(converted_block)
}

struct PendingEnvironmentVariable {
    key: OsString,
    value: OsString,
    key_wide: Vec<u16>,
}

impl PendingEnvironmentVariable {
    fn new(key: &OsStr, value: &OsStr) -> Self {
        Self {
            key: key.to_os_string(),
            value: value.to_os_string(),
            key_wide: key.encode_wide().collect(),
        }
    }
}

fn compare_environment_names(left: &[u16], right: &[u16]) -> Ordering {
    let result = unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            1,
        )
    };
    match result {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        CSTR_GREATER_THAN => Ordering::Greater,
        // 环境变量名受 CreateProcess 环境块上限约束，正常不会失败；异常时仍给出
        // 全序，不能让排序过程 panic。
        _ => left.cmp(right),
    }
}

// According to the `lpEnvironment` parameter description:
// https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessa#parameters
//
// > An environment block consists of a null-terminated block of null-terminated strings. Each
// string is in the following form:
// >
// > name=value\0
fn add_windows_env_key_value_to_block(block: &mut Vec<u16>, key: &OsStr, value: &OsStr) {
    block.extend(key.encode_wide());
    block.push('=' as u16);
    block.extend(value.encode_wide());
    block.push(0);
}

impl OnResize for Conpty {
    fn on_resize(&mut self, window_size: WindowSize) {
        // A failed resize (e.g. racing a pane teardown, or the host died) must
        // not take the whole process down — log and let the exit path handle it.
        let result = unsafe { (self.api.resize)(self.handle, window_size.into()) };
        if result != S_OK {
            // stderr 兜底：GPUI 主窗形态没有装 logger，失败不能无声。
            eprintln!("[slterm:conpty] ResizePseudoConsole failed: HRESULT {result:#x}");
            warn!("ResizePseudoConsole failed: HRESULT {result:#x}");
        }
    }
}

impl From<WindowSize> for COORD {
    fn from(window_size: WindowSize) -> Self {
        let lines = window_size.num_lines;
        let columns = window_size.num_cols;
        COORD {
            X: columns as i16,
            Y: lines as i16,
        }
    }
}
#[cfg(test)]
mod runtime_asset_tests {
    use super::{
        CONPTY_DLL_BYTES, CONPTY_WIN11_MIN_BUILD, ConptyApi, OPENCONSOLE_EXE_BYTES,
        bundled_conpty_dir, conhost_input_corrupts_cpr, conpty_status, convert_custom_env,
        ensure_extracted, extraction_dir_from, should_bundle, write_if_size_differs,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "slterm-conpty-{name}-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_pair(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("conpty.dll"), b"dll").unwrap();
        std::fs::write(dir.join("OpenConsole.exe"), b"host").unwrap();
    }

    #[test]
    fn conpty_prefers_complete_runtime_pair() {
        let dir = TestDir::new("runtime");
        write_pair(dir.path());
        write_pair(&dir.path().join("runtime"));

        assert_eq!(
            bundled_conpty_dir(dir.path()),
            Some(dir.path().join("runtime"))
        );
    }

    #[test]
    fn conpty_does_not_mix_partial_runtime_with_legacy_pair() {
        let dir = TestDir::new("partial");
        write_pair(dir.path());
        let runtime = dir.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        std::fs::write(runtime.join("conpty.dll"), b"dll-only").unwrap();

        assert_eq!(
            bundled_conpty_dir(dir.path()),
            Some(dir.path().to_path_buf())
        );
    }

    #[test]
    fn conpty_returns_none_without_complete_pair() {
        let dir = TestDir::new("missing");
        let runtime = dir.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        std::fs::write(runtime.join("OpenConsole.exe"), b"host-only").unwrap();

        assert_eq!(bundled_conpty_dir(dir.path()), None);
    }

    #[test]
    fn complete_environment_does_not_restore_parent_variables() {
        let custom = HashMap::from([("SLTERM_REFRESH_TEST".to_owned(), "fresh".to_owned())]);
        let block = convert_custom_env(&custom, true).expect("environment block");
        let entries: Vec<String> = block
            .split(|character| *character == 0)
            .filter(|entry| !entry.is_empty())
            .map(String::from_utf16_lossy)
            .collect();

        assert_eq!(entries, vec!["SLTERM_REFRESH_TEST=fresh"]);
    }

    #[test]
    fn complete_environment_block_is_sorted_case_insensitively() {
        let custom = HashMap::from([
            ("z-last".to_owned(), "3".to_owned()),
            ("Middle".to_owned(), "2".to_owned()),
            ("a-first".to_owned(), "1".to_owned()),
        ]);
        let block = convert_custom_env(&custom, true).expect("environment block");
        let entries: Vec<String> = block
            .split(|character| *character == 0)
            .filter(|entry| !entry.is_empty())
            .map(String::from_utf16_lossy)
            .collect();

        assert_eq!(entries, vec!["a-first=1", "Middle=2", "z-last=3"]);
    }

    // ─── 固定注入清单（convert_custom_env 末尾叠加）───

    fn block_entries(block: &[u16]) -> Vec<String> {
        block
            .split(|character| *character == 0)
            .filter(|entry| !entry.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    #[test]
    fn fixed_env_injections_filled_when_absent() {
        // 增量合并形态：custom 缺能力键 → 固定清单补缺。
        let custom = HashMap::from([("SLTERM_REFRESH_TEST".to_owned(), "fresh".to_owned())]);
        let block = convert_custom_env(&custom, false).expect("environment block");
        let entries = block_entries(&block);
        assert!(entries.contains(&"TERM=xterm-256color".to_owned()));
        assert!(entries.contains(&"COLORTERM=truecolor".to_owned()));
        assert!(entries.contains(&"TERM_PROGRAM=slterm".to_owned()));
    }

    #[test]
    fn fixed_env_injections_never_override_custom() {
        // custom 同名键（大小写变体也算）优先——固定注入只补缺。
        let custom = HashMap::from([("term".to_owned(), "custom-term".to_owned())]);
        let block = convert_custom_env(&custom, false).expect("environment block");
        let entries = block_entries(&block);
        assert!(entries.contains(&"term=custom-term".to_owned()));
        assert!(
            !entries.contains(&"TERM=xterm-256color".to_owned()),
            "固定注入不得覆盖 custom 同名键"
        );
    }

    #[test]
    fn fixed_env_injections_apply_to_inherited_merge() {
        // 非 complete 形态（与父进程环境合并）同样叠加固定清单。
        let custom = HashMap::new();
        let block = convert_custom_env(&custom, false);
        // custom 为空且非 complete → None（走父进程继承,无环境块）——本形态
        // 不叠加;叠加语义只在「有块」时生效。
        assert!(block.is_none());
        let custom = HashMap::from([("A".to_owned(), "1".to_owned())]);
        let entries = block_entries(&convert_custom_env(&custom, false).expect("block"));
        assert!(entries.contains(&"TERM=xterm-256color".to_owned()));
    }

    // ─── ConPTY 加载决策流（两阶段合并）───

    #[test]
    fn should_bundle_below_threshold() {
        assert!(should_bundle(19041));
        assert!(should_bundle(21375));
    }

    #[test]
    fn should_not_bundle_at_or_above_threshold() {
        assert!(!should_bundle(21376));
        assert!(!should_bundle(26100));
    }

    #[test]
    fn cpr_corrupted_below_threshold() {
        assert!(conhost_input_corrupts_cpr(19041));
        assert!(conhost_input_corrupts_cpr(21375));
        // build 获取失败回退 0 → 按 Win10 处置（剥离，安全侧）。
        assert!(conhost_input_corrupts_cpr(0));
    }

    #[test]
    fn cpr_not_corrupted_at_or_above_threshold() {
        assert!(!conhost_input_corrupts_cpr(21376));
        assert!(!conhost_input_corrupts_cpr(26100));
    }

    #[test]
    fn extraction_dir_from_appends_segments() {
        let dir = extraction_dir_from(Path::new(r"C:\Users\x\AppData\Local"));
        assert_eq!(
            dir,
            PathBuf::from(r"C:\Users\x\AppData\Local\slterm\conpty")
        );
    }

    #[test]
    fn ensure_extracted_writes_then_idempotent() {
        let tmp = TestDir::new("extract");
        let target = tmp.path().join("conpty");
        ensure_extracted(&target).unwrap();
        let dll = target.join("conpty.dll");
        let exe = target.join("OpenConsole.exe");
        assert!(dll.is_file());
        assert!(exe.is_file());
        assert_eq!(std::fs::read(&dll).unwrap(), CONPTY_DLL_BYTES);
        assert_eq!(std::fs::read(&exe).unwrap(), OPENCONSOLE_EXE_BYTES);
        let m1 = std::fs::metadata(&dll).unwrap().modified().unwrap();
        ensure_extracted(&target).unwrap();
        let m2 = std::fs::metadata(&dll).unwrap().modified().unwrap();
        assert_eq!(m1, m2, "同大小应跳过写入，mtime 不变");
    }

    #[test]
    fn write_if_size_differs_overwrites_only_on_size_mismatch() {
        let tmp = TestDir::new("size-check");
        let p = tmp.path().join("conpty.dll");
        std::fs::write(&p, b"old-vendor").unwrap();
        write_if_size_differs(&p, b"new-vendor-bytes").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new-vendor-bytes");
        // 同大小不同内容：跳过（嵌入内容编译期固定，大小判定足够）。
        let p2 = tmp.path().join("OpenConsole.exe");
        std::fs::write(&p2, b"same-len-123").unwrap();
        write_if_size_differs(&p2, b"same-len-456").unwrap();
        assert_eq!(std::fs::read(&p2).unwrap(), b"same-len-123");
    }

    /// 场景注入守卫：临时改写 LOCALAPPDATA 指向受控路径，drop 时还原。
    struct LocalAppDataGuard(Option<std::ffi::OsString>);

    impl LocalAppDataGuard {
        fn set(dir: &Path) -> Self {
            let prev = std::env::var_os("LOCALAPPDATA");
            unsafe { std::env::set_var("LOCALAPPDATA", dir) };
            Self(prev)
        }
    }

    impl Drop for LocalAppDataGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => unsafe { std::env::set_var("LOCALAPPDATA", v) },
                None => unsafe { std::env::remove_var("LOCALAPPDATA") },
            }
        }
    }

    #[test]
    fn conpty_status_not_attempted_when_sideload_off() {
        // sideload 关闭 → 未尝试（toast 静默形态），后端系统 API。
        let api = ConptyApi::new(false, 19041);
        assert!(!api.sideloaded);
        let s = conpty_status();
        assert!(!s.attempted, "sideload 关闭不尝试");
        assert!(!s.bundled);
        assert!(s.fallback_reason.is_none());
    }

    #[test]
    fn conpty_status_win11_missing_pair_falls_back() {
        // Win11 + exe 旁无文件对 → 不触发提取臂，直接回退（机器无关：
        // 测试 exe 目录恒无完整文件对；LOCALAPPDATA 注入 tempdir 防真提取）。
        let tmp = TestDir::new("win11-no-pair");
        let _guard = LocalAppDataGuard::set(tmp.path());
        let api = ConptyApi::new(true, CONPTY_WIN11_MIN_BUILD);
        assert!(!api.sideloaded, "Win11 不触发提取，文件对缺失回退系统");
        let s = conpty_status();
        assert!(s.attempted);
        assert!(!s.bundled);
        assert!(s.fallback_reason.is_some(), "回退必有原因");
        assert!(!tmp.path().join("slterm").exists(), "Win11 不得执行提取");
    }

    #[test]
    fn conpty_status_bundled_on_win10_extraction() {
        // Win10 + 文件对缺失 → NuGet 嵌入字节提取到注入目录 + 真实
        // LoadLibraryW 加载 → 全量真实路径（机器无关：字节编译期嵌入）。
        let tmp = TestDir::new("win10-extract");
        let _guard = LocalAppDataGuard::set(tmp.path());
        let api = ConptyApi::new(true, 19041);
        assert!(api.sideloaded, "提取 + 加载成功应走侧载 conhost");
        let s = conpty_status();
        assert!(s.attempted);
        assert!(s.bundled);
        assert!(s.fallback_reason.is_none());
        assert!(
            tmp.path()
                .join("slterm")
                .join("conpty")
                .join("conpty.dll")
                .is_file(),
            "提取目录应含 conpty.dll"
        );
    }

    #[test]
    fn conpty_status_fallback_reason_matches_load_error() {
        // LOCALAPPDATA 指向文件 → 提取路径不可建 → 稳定失败注入；
        // fallback_reason 与直跑 load_sideloaded 的 Err 同源（同一变量）。
        let blocker = TestDir::new("win10-blocked");
        let file_path = blocker.path().join("blocker-file");
        std::fs::write(&file_path, b"not-a-dir").unwrap();
        let _guard = LocalAppDataGuard::set(&file_path);

        let expected = match ConptyApi::load_sideloaded(19041) {
            Ok(_) => panic!("注入点应稳定失败（文件占用路径必使提取失败）"),
            Err(reason) => reason,
        };

        let api = ConptyApi::new(true, 19041);
        assert!(!api.sideloaded, "提取失败应回退系统");
        let s = conpty_status();
        assert!(s.attempted);
        assert!(!s.bundled);
        let reason = s.fallback_reason.as_deref().expect("回退必有原因");
        assert_eq!(reason, expected, "状态原因与加载错误同源零漂移");
    }
}
