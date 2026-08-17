/// ConPTY 宿主捆绑动态加载（ADR-0005）
///
/// 老版 Win10 内置 conhost 在 WIN32_INPUT_MODE (0x4) 下不转发鼠标 VT 序列——
/// claude 等全屏 TUI 滚轮失效（键盘正常；WT 对照实验证实）。exe 同目录存在捆绑的
/// conpty.dll + OpenConsole.exe（微软开源 OpenConsole，vendor/conpty/）时，经
/// LoadLibrary 动态解析 Conpty* 导出——新版宿主完整支持 Win32 input mode 鼠标转发；
/// 缺失/加载失败则静默回退系统 kernel32（Win11 行为零变化）。
///
/// 注意：自动化测试无法守卫 ConPTY 交互（先例同 PASSTHROUGH_MODE），
/// 更新 vendor 二进制后必须 Win10 实机验证 claude 滚轮 + 键盘/IME。
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use windows::core::HRESULT;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{COORD, HPCON};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

/// 进程级单次解析结果（static 永不 drop——捆绑 DLL 及其句柄保活至进程退出）
static CONPTY_API: OnceLock<ConptyApi> = OnceLock::new();

/// ConPTY PseudoConsole 三 API 的函数指针集合（捆绑 conpty.dll 或系统 kernel32）
pub struct ConptyApi {
    pub create: unsafe extern "system" fn(COORD, HANDLE, HANDLE, u32, *mut HPCON) -> HRESULT,
    pub close: unsafe extern "system" fn(HPCON),
    pub resize: unsafe extern "system" fn(HPCON, COORD) -> HRESULT,
}

// kernel32 系统回退声明（link_name 对齐 kernel32 导出名；函数项可直接 cast 为函数指针）
#[link(name = "kernel32")]
extern "system" {
    #[link_name = "CreatePseudoConsole"]
    fn create_pseudo_console_sys(
        size: COORD,
        hinput: HANDLE,
        houtput: HANDLE,
        dwflags: u32,
        phpc: *mut HPCON,
    ) -> HRESULT;
    #[link_name = "ClosePseudoConsole"]
    fn close_pseudo_console_sys(hpc: HPCON);
    #[link_name = "ResizePseudoConsole"]
    fn resize_pseudo_console_sys(hpc: HPCON, size: COORD) -> HRESULT;
}

/// exe 同目录捆绑 conpty.dll 的候选路径（存在性判断——纯函数，L1 可测）
pub fn bundled_dll_path(exe_dir: &Path) -> Option<PathBuf> {
    let p = exe_dir.join("conpty.dll");
    p.is_file().then_some(p)
}

/// 加载捆绑 conpty.dll 并解析三导出（失败返回 None → 调用方回退系统）
///
/// 导出名带 Conpty 前缀（避免与 kernel32 import 冲突，见 vendor/conpty/README.md）。
fn load_bundled(dll: &Path) -> Option<ConptyApi> {
    let wide = dll_wide(dll);
    // SAFETY: LoadLibraryW 加载绝对路径 DLL；模块句柄随 static CONPTY_API 保活至进程退出
    let module = unsafe { LoadLibraryW(windows::core::PCWSTR(wide.as_ptr())) }.ok()?;

    // SAFETY: 导出名以 null 结尾，指向只读字符串字面量
    let farproc =
        |name: &[u8]| unsafe { GetProcAddress(module, windows::core::PCSTR(name.as_ptr())) };
    let create = farproc(b"ConptyCreatePseudoConsole\0")?;
    let close = farproc(b"ConptyClosePseudoConsole\0")?;
    let resize = farproc(b"ConptyResizePseudoConsole\0")?;

    Some(ConptyApi {
        // SAFETY: GetProcAddress 解包后为裸函数指针，与目标签名同尺寸（函数指针间 transmute）；
        // 导出名与 conpty.h 声明一致
        create: unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(COORD, HANDLE, HANDLE, u32, *mut HPCON) -> HRESULT,
            >(create)
        },
        close: unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(HPCON),
            >(close)
        },
        resize: unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(HPCON, COORD) -> HRESULT,
            >(resize)
        },
    })
}

/// 解析 ConPTY API 入口（进程级单次）：优先捆绑 conpty.dll，缺失/失败回退系统 kernel32
pub fn resolve_conpty_api() -> &'static ConptyApi {
    CONPTY_API.get_or_init(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(dll) = exe_dir.as_deref().and_then(bundled_dll_path) {
            if let Some(api) = load_bundled(&dll) {
                return api;
            }
        }
        ConptyApi {
            create: create_pseudo_console_sys as _,
            close: close_pseudo_console_sys as _,
            resize: resize_pseudo_console_sys as _,
        }
    })
}

/// 路径转 null 结尾 UTF-16LE（LoadLibraryW 入参）
fn dll_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_dll_path_detects_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("conpty.dll"), b"fake").unwrap();
        assert_eq!(
            bundled_dll_path(dir.path()),
            Some(dir.path().join("conpty.dll"))
        );
    }

    #[test]
    fn bundled_dll_path_none_without_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bundled_dll_path(dir.path()), None);
    }

    #[test]
    fn resolve_is_stable_singleton() {
        // 测试环境无捆绑文件 → 回退系统 kernel32；static 单次解析（同一指针）
        let a = resolve_conpty_api();
        let b = resolve_conpty_api();
        assert!(std::ptr::eq(a, b));
    }
}
