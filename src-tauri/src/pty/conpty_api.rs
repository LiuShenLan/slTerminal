// ConPTY API 解析层（ADR-0005）：Win10 捆绑新版 conhost 宿主
//
// 根因：老 Win10（build < 21376）in-box conhost 的 ConPTY 不转发鼠标 VT 序列
// （microsoft/terminal#376，修复 PR #4856 只在新版 conhost）——0x3/0x7 两条输入
// 路径均实测滚轮失效。修复：vendor 目录的 conpty.dll + OpenConsole.exe（官方
// NuGet Microsoft.Windows.Console.ConPTY 构建）嵌入本 dll 资源，仅 Win10 提取到
// %LOCALAPPDATA%\slterminal\conpty\ 并动态加载；失败静默回退系统 ConPTY（行为 =
// 现状）。conpty.dll 定位 OpenConsole.exe 靠同目录查找（PR #12980），故两文件
// 必须提取到同一目录。
//
// 自动化无法守卫真实鼠标转发（先例同 PASSTHROUGH_MODE/0x3）——改动必须 Win10
// 实机验证真实 claude 滚轮 + 键盘/IME/kitty。

use crate::error::AppError;
use anyhow::Error;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use windows::core::{HRESULT, PCSTR, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HMODULE};
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

/// Win10/Win11 分界 build（与前端 xterm 钳制 ADR-0004 同源）
pub const CONPTY_WIN11_MIN_BUILD: u32 = 21376;

// 嵌入 vendor 二进制（相对本文件路径；编译进 slterminal_lib.dll）
const CONPTY_DLL_BYTES: &[u8] = include_bytes!("../../vendor/conpty/conpty.dll");
const OPENCONSOLE_EXE_BYTES: &[u8] = include_bytes!("../../vendor/conpty/OpenConsole.exe");

// C ABI 函数指针签名（与 conpty.dll 导出对齐；HPCON 以 isize 表示——
// windows crate 的 HPCON 是 tuple struct，不满足 extern fn 直接传参）
type FnCreate = unsafe extern "system" fn(COORD, HANDLE, HANDLE, u32, *mut isize) -> HRESULT;
type FnClose = unsafe extern "system" fn(isize);
type FnResize = unsafe extern "system" fn(isize, COORD) -> HRESULT;

/// ConPTY 后端状态(CP-010:一次性查询,启动 toast 数据源)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConptyStatus {
    /// 是否尝试捆绑(仅 Win10 build < 21376)
    pub attempted: bool,
    /// 实际是否走捆绑 conhost
    pub bundled: bool,
    /// 回退原因(attempted && !bundled 时有值,与 warn 日志同源)
    pub fallback_reason: Option<String>,
}

/// 状态记录槽(CP-010)。生产形态 OnceLock:resolve_conpty_api 的 API OnceLock 保证
/// build_conpty_api 进程级恰好执行一次 → 此处恰好一次 set。单元测试直连
/// build_conpty_api 会跨用例重复写同一槽,OnceLock 无法重置,故测试构建改走可覆盖
/// parking_lot Mutex 槽(--test-threads=1 串行执行,无并发竞争;生产零编译)。
#[cfg(not(test))]
static STATUS: std::sync::OnceLock<ConptyStatus> = std::sync::OnceLock::new();
#[cfg(test)]
static STATUS: parking_lot::Mutex<Option<ConptyStatus>> = parking_lot::Mutex::new(None);

/// 防御兜底状态(记录槽未初始化时返回):「未尝试」。仅取 build 号失败(非 Windows)
/// 等理论场景可达——resolve 路径必先 record,此处保证查询永不 panic。
static CONPTY_STATUS_DEFAULT: ConptyStatus = ConptyStatus {
    attempted: false,
    bundled: false,
    fallback_reason: None,
};

/// 记录状态(build_conpty_api 每个形态调用一次)
fn record_status(status: ConptyStatus) {
    #[cfg(not(test))]
    {
        // 生产单次 set;重复 set(理论不可达)返回 Err 忽略,不 panic
        let _ = STATUS.set(status);
    }
    #[cfg(test)]
    {
        *STATUS.lock() = Some(status);
    }
}

/// ConPTY API 抽象：系统路径（windows crate 直接链接）或捆绑路径（动态加载）
pub struct ConptyApi {
    backend: Backend,
}

enum Backend {
    System,
    Bundled {
        create: FnCreate,
        close: FnClose,
        resize: FnResize,
        /// 保持 dll 加载态直到进程退出（函数指针生命周期）；
        /// 存 isize 而非 HMODULE——HMODULE 未实现 Send/Sync（static OnceLock 要求）
        _module: isize,
    },
}

impl ConptyApi {
    fn system() -> ConptyApi {
        ConptyApi {
            backend: Backend::System,
        }
    }

    /// 是否走捆绑 conhost（供 flags 决策：捆绑 → 恒 0x7）
    pub fn is_bundled(&self) -> bool {
        matches!(self.backend, Backend::Bundled { .. })
    }

    /// 创建伪控制台。
    ///
    /// # Safety
    /// 管道句柄生命周期由调用方保证（同 windows crate 约定）。
    pub unsafe fn create(
        &self,
        size: COORD,
        input: HANDLE,
        output: HANDLE,
        flags: u32,
    ) -> Result<HPCON, Error> {
        match &self.backend {
            Backend::System => {
                // SAFETY: 见方法级约定；kernel32 实现
                unsafe { CreatePseudoConsole(size, input, output, flags) }.map_err(Into::into)
            }
            Backend::Bundled { create, .. } => {
                let mut hpc: isize = -1;
                // SAFETY: 见方法级约定；符号存在性由加载时 GetProcAddress 保证
                unsafe { create(size, input, output, flags, &mut hpc) }.ok()?;
                Ok(HPCON(hpc))
            }
        }
    }

    /// 关闭伪控制台。
    ///
    /// # Safety
    /// hpc 由 create 创建且仅关闭一次（同 windows crate 约定）。
    pub unsafe fn close(&self, hpc: HPCON) {
        match &self.backend {
            Backend::System => {
                // SAFETY: 见方法级约定
                unsafe { ClosePseudoConsole(hpc) }
            }
            Backend::Bundled { close, .. } => {
                // SAFETY: 见方法级约定
                unsafe { close(hpc.0) }
            }
        }
    }

    /// 调整伪控制台尺寸。
    ///
    /// # Safety
    /// hpc 有效且未被关闭（同 windows crate 约定）。
    pub unsafe fn resize(&self, hpc: HPCON, size: COORD) -> Result<(), Error> {
        match &self.backend {
            Backend::System => {
                // SAFETY: 见方法级约定
                unsafe { ResizePseudoConsole(hpc, size) }.map_err(Into::into)
            }
            Backend::Bundled { resize, .. } => {
                // SAFETY: 见方法级约定
                unsafe { resize(hpc.0, size) }.ok().map_err(Into::into)
            }
        }
    }
}

/// 进程级单次解析：build < 21376 尝试捆绑，失败回退系统；Win11 恒系统（零变化）
pub fn resolve_conpty_api(build_number: u32) -> &'static ConptyApi {
    static API: OnceLock<ConptyApi> = OnceLock::new();
    API.get_or_init(|| build_conpty_api(build_number))
}

/// 决策纯函数：仅 Win10（build < 21376）尝试捆绑
pub fn should_bundle(build_number: u32) -> bool {
    build_number < CONPTY_WIN11_MIN_BUILD
}

/// 提取目标目录：%LOCALAPPDATA%\slterminal\conpty（纯路径构造，便于测试注入）
pub fn extraction_dir_from(localappdata: &Path) -> PathBuf {
    localappdata.join("slterminal").join("conpty")
}

/// 幂等提取：已存在且大小与嵌入一致 → 复用；否则覆盖重写（vendor 升级自愈）
pub fn ensure_extracted(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_if_size_differs(&dir.join("conpty.dll"), CONPTY_DLL_BYTES)?;
    write_if_size_differs(&dir.join("OpenConsole.exe"), OPENCONSOLE_EXE_BYTES)?;
    Ok(())
}

/// 大小一致跳过写入；缺失或大小不一致时覆盖（嵌入内容编译期固定，大小判定足够）
fn write_if_size_differs(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let matches = std::fs::metadata(path)
        .map(|m| m.len() == bytes.len() as u64)
        .unwrap_or(false);
    if !matches {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

/// 加载捆绑 conpty.dll 并解析三函数（缺失/失败由调用方回退系统）
fn load_bundled(dir: &Path) -> Result<Backend, Error> {
    let dll_path = dir.join("conpty.dll");
    let wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: wide 以 null 结尾且指向有效 UTF-16 路径
    let module = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) }?;
    let create = get_proc::<FnCreate>(module, b"ConptyCreatePseudoConsole")?;
    let close = get_proc::<FnClose>(module, b"ConptyClosePseudoConsole")?;
    let resize = get_proc::<FnResize>(module, b"ConptyResizePseudoConsole")?;
    Ok(Backend::Bundled {
        create,
        close,
        resize,
        _module: module.0 as isize,
    })
}

/// 按名解析导出符号（name 不含结尾 null，内部补齐）
fn get_proc<T>(module: HMODULE, name: &[u8]) -> Result<T, Error> {
    let mut cname = name.to_vec();
    cname.push(0);
    // SAFETY: cname 为 null 结尾 ASCII；module 有效
    let addr = unsafe { GetProcAddress(module, PCSTR(cname.as_ptr())) }
        .ok_or_else(|| anyhow::anyhow!("GetProcAddress {} 失败", String::from_utf8_lossy(name)))?;
    // SAFETY: FARPROC 为可空函数指针（windows crate 以 Option<fn> 表示，此处已解包），
    // 与 T（函数指针）在 64 位平台同为 8 字节；符号存在性由 GetProcAddress 保证
    Ok(unsafe { std::mem::transmute_copy::<unsafe extern "system" fn() -> isize, T>(&addr) })
}

/// 按 build 构建 API：Win10 提取 + 加载捆绑，任一环节失败回退系统（行为 = 现状）；
/// 每形态同步记录 ConptyStatus（CP-010：状态可观测，warn 与 fallback_reason 同源）
fn build_conpty_api(build_number: u32) -> ConptyApi {
    if !should_bundle(build_number) {
        // Win11/未尝试：attempted=false（不弹 toast 的形态）
        record_status(ConptyStatus {
            attempted: false,
            bundled: false,
            fallback_reason: None,
        });
        return ConptyApi::system();
    }
    match try_bundle() {
        Ok(backend) => {
            record_status(ConptyStatus {
                attempted: true,
                bundled: true,
                fallback_reason: None,
            });
            ConptyApi { backend }
        }
        Err(e) => {
            // CP-010：warn 文案与 fallback_reason 同一 format!("{e:#}") 变量——
            // 日志与命令暴露同源零漂移
            let reason = format!("{e:#}");
            tracing::warn!("Win10 捆绑 ConPTY 加载失败，回退系统 conhost（滚轮不可用）: {reason}");
            record_status(ConptyStatus {
                attempted: true,
                bundled: false,
                fallback_reason: Some(reason),
            });
            ConptyApi::system()
        }
    }
}

fn try_bundle() -> Result<Backend, Error> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA 环境变量缺失"))?;
    let dir = extraction_dir_from(&base);
    ensure_extracted(&dir)?;
    load_bundled(&dir)
}

/// 查询 ConPTY 后端状态(CP-010:一次性查询,启动 toast 数据源)
///
/// conpty 解析(提取 + LoadLibraryW)在首次 spawn 懒触发;启动序列的查询早于首次
/// spawn → 防御分支按真实 build 主动触发一次 resolve——API OnceLock 单次语义,与
/// 首 spawn 同值,Win10 回退自启动即可观测(而非等第一个终端)。
#[cfg(not(test))]
pub fn conpty_status() -> &'static ConptyStatus {
    if STATUS.get().is_none() {
        if let Ok(build) = crate::pty::win_build::get_windows_build_number() {
            resolve_conpty_api(build);
        }
    }
    // 兜底:仅取 build 号失败(非 Windows 等)可达,回退「未尝试」默认,不 panic
    STATUS.get().unwrap_or(&CONPTY_STATUS_DEFAULT)
}

#[cfg(test)]
pub fn conpty_status() -> &'static ConptyStatus {
    // 测试构建:读可覆盖槽(OnceLock 无法跨用例重置)。Box::leak 换 'static——
    // 单测进程内量级可忽略的泄漏,仅测试构建存在
    let snapshot = STATUS
        .lock()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| CONPTY_STATUS_DEFAULT.clone());
    Box::leak(Box::new(snapshot))
}

/// 查询 ConPTY 后端状态(CP-010:Win10 回退可观测)
///
/// 命令注册(lib.rs generate_handler! / build.rs AppManifest / capabilities
/// 三处, SEC-07)由收口阶段统一办理,本文件仅承载命令本体。
#[tauri::command]
pub async fn pty_conpty_status() -> Result<ConptyStatus, AppError> {
    Ok(conpty_status().clone())
}

#[cfg(test)]
mod conpty_api_tests {
    use super::*;

    // T1: 决策分叉——仅 Win10 尝试捆绑
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

    // T2: 提取路径构造
    #[test]
    fn extraction_dir_from_appends_segments() {
        let dir = extraction_dir_from(Path::new(r"C:\Users\x\AppData\Local"));
        assert_eq!(
            dir,
            PathBuf::from(r"C:\Users\x\AppData\Local\slterminal\conpty")
        );
    }

    // T3: 提取幂等——二次调用不重写（mtime 不变）
    #[test]
    fn ensure_extracted_writes_then_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
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

    // T4: 大小不一致覆盖（vendor 升级自愈）；同大小跳过（即使内容不同）
    #[test]
    fn write_if_size_differs_overwrites_only_on_size_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("conpty.dll");
        std::fs::write(&p, b"old-vendor").unwrap();
        write_if_size_differs(&p, b"new-vendor-bytes").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new-vendor-bytes");
        // 同大小不同内容：跳过（嵌入内容编译期固定，大小判定足够）
        let p2 = tmp.path().join("OpenConsole.exe");
        std::fs::write(&p2, b"same-len-123").unwrap();
        write_if_size_differs(&p2, b"same-len-456").unwrap();
        assert_eq!(std::fs::read(&p2).unwrap(), b"same-len-123");
    }

    // CP-010:状态记录三用例。STATUS 单例跨用例污染按防御口径处理:测试构建的记录槽
    // 为可覆盖 Mutex(cfg(test) 分支,见模块顶 STATUS 注释),每用例独立场景重复
    // 记录互不污染;场景注入经 LocalAppDataGuard 临时改写 LOCALAPPDATA(串行
    // --test-threads=1 无并发),Drop 还原,不污染其他套件。

    /// 场景注入守卫:临时改写 LOCALAPPDATA 指向受控路径,drop 时还原原值
    struct LocalAppDataGuard(Option<std::ffi::OsString>);

    impl LocalAppDataGuard {
        fn set(dir: &Path) -> Self {
            let prev = std::env::var_os("LOCALAPPDATA");
            std::env::set_var("LOCALAPPDATA", dir);
            Self(prev)
        }
    }

    impl Drop for LocalAppDataGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => std::env::set_var("LOCALAPPDATA", v),
                None => std::env::remove_var("LOCALAPPDATA"),
            }
        }
    }

    // T5: Win11(≥ 21376)→ 未尝试(attempted=false,启动 toast 静默形态)
    #[test]
    fn conpty_status_win11_not_attempted() {
        build_conpty_api(CONPTY_WIN11_MIN_BUILD);
        let s = conpty_status();
        assert!(!s.attempted, "Win11 不尝试捆绑");
        assert!(!s.bundled);
        assert!(s.fallback_reason.is_none());
    }

    // T6: Win10 + 捆绑成功(tempdir 注入提取 + 真实 LoadLibraryW)→ 全量真实路径
    #[test]
    fn conpty_status_bundled_on_win10() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = LocalAppDataGuard::set(tmp.path());
        build_conpty_api(19041);
        let s = conpty_status();
        assert!(s.attempted, "Win10 应尝试捆绑");
        assert!(s.bundled, "提取 + 加载成功应走捆绑 conhost");
        assert!(s.fallback_reason.is_none());
    }

    // T7: Win10 + 加载失败注入(LOCALAPPDATA 指向已存在文件 → 提取路径不可建)
    //     → fallback_reason=Some 且与 warn 文案同源(与直跑 try_bundle 的 {:#} 派生一致)
    #[test]
    fn conpty_status_fallback_reason_matches_warn() {
        // LOCALAPPDATA 指向文件:create_dir_all(其下 slterminal/conpty)必经非目录
        // 节点 → 提取环节稳定失败注入(注入点照 ensure_extracted 幂等用例先例)
        let blocker = tempfile::tempdir().unwrap();
        let file_path = blocker.path().join("blocker-file");
        std::fs::write(&file_path, b"not-a-dir").unwrap();
        let _guard = LocalAppDataGuard::set(&file_path);

        // 同注入点直跑 try_bundle 取权威错误,{:#} 派生期望文案
        // (warn 与 fallback_reason 同用单变量的结构性证明)
        let expected = match try_bundle() {
            Ok(_) => panic!("注入点应稳定失败(文件占用路径必使提取失败)"),
            Err(e) => format!("{e:#}"),
        };

        build_conpty_api(19041);
        let s = conpty_status();
        assert!(s.attempted, "Win10 应尝试捆绑");
        assert!(!s.bundled, "提取失败应回退系统");
        let reason = s.fallback_reason.as_deref().expect("回退必有原因");
        assert_eq!(reason, expected);
    }
}
