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

/// 按 build 构建 API：Win10 提取 + 加载捆绑，任一环节失败回退系统（行为 = 现状）
fn build_conpty_api(build_number: u32) -> ConptyApi {
    if !should_bundle(build_number) {
        return ConptyApi::system();
    }
    match try_bundle() {
        Ok(backend) => ConptyApi { backend },
        Err(e) => {
            tracing::warn!("Win10 捆绑 ConPTY 加载失败，回退系统 conhost（滚轮不可用）: {e:#}");
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
}
