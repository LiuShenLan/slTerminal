//! 另存为目标的原生文件对话框；只在后台执行体调用。
//!
//! 文件对话框是 STA COM 模态窗口：本模块在调用线程初始化 STA，把主窗
//! HWND 作为 owner，取消不视为错误。业务侧只经 WindowsIntegration 端口
//! 消费，真实桌面交互按测试豁免人工核验。

use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileSaveDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, HSTRING};

/// 用户选定的另存为目标；`None` = 取消。
pub(crate) fn pick_save_target(
    owner: Option<isize>,
    suggested: &Path,
) -> io::Result<Option<PathBuf>> {
    let _apartment = ComApartment::initialize()?;
    // SAFETY: 调用线程已初始化 STA；owner 是仍存活的主窗句柄或空值。
    // 对话框只在当前调用期间持有句柄，不复制或延长窗口生命周期。
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| com_error(error.code()))?;
        let options = dialog
            .GetOptions()
            .map_err(|error| com_error(error.code()))?;
        dialog
            .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)
            .map_err(|error| com_error(error.code()))?;
        if let Some(parent) = suggested.parent() {
            // 缺省目录只影响起点；失败时沿用系统上次位置。
            let folder: windows::core::Result<IShellItem> =
                SHCreateItemFromParsingName(&HSTRING::from(parent.as_os_str()), None);
            if let Ok(folder) = folder {
                let _ = dialog.SetFolder(&folder);
            }
        }
        if let Some(name) = suggested.file_name() {
            let _ = dialog.SetFileName(&HSTRING::from(name));
        }
        let hwnd = owner.map(|handle| HWND(handle as *mut _));
        if let Err(error) = dialog.Show(hwnd) {
            return if is_cancelled(error.code()) {
                Ok(None)
            } else {
                Err(com_error(error.code()))
            };
        }
        let item = dialog
            .GetResult()
            .map_err(|error| com_error(error.code()))?;
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| com_error(error.code()))?;
        let text = name
            .to_string()
            .map_err(|_| io::Error::other("保存目标路径不是有效 Unicode"))?;
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(text)))
    }
}

fn is_cancelled(code: HRESULT) -> bool {
    // HRESULT_FROM_WIN32(ERROR_CANCELLED) = 0x8007_04C7。
    code == HRESULT((0x8007_0000u32 | ERROR_CANCELLED.0) as i32)
}

fn com_error(code: HRESULT) -> io::Error {
    // 只记录 HRESULT，不插值路径或用户输入。
    io::Error::other(format!("保存对话框调用失败: hresult={:#010x}", code.0))
}

/// 后台线程 STA：对话框要求单线程单元；退出时只释放本线程的初始化。
struct ComApartment(bool);

impl ComApartment {
    fn initialize() -> io::Result<Self> {
        // SAFETY: CoInitializeEx 无前置条件；成对调用由 Drop 保证。
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_ok() {
            Ok(Self(true))
        } else if result == RPC_E_CHANGED_MODE {
            Err(io::Error::other("保存对话框需要 STA 线程"))
        } else {
            Err(com_error(result))
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: 仅释放本次 initialize 成功的单元。
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_not_an_error() {
        assert!(is_cancelled(HRESULT(0x8007_04C7u32 as i32)));
        assert!(!is_cancelled(HRESULT(0)));
        assert!(!is_cancelled(HRESULT(0x8000_4005u32 as i32)));
    }

    #[test]
    fn fake_picker_returns_configured_target_without_a_dialog() {
        use crate::platform::windows_integration::{
            FakeWindowsIntegration, WindowsIntegration as _,
        };
        let windows = FakeWindowsIntegration::default();
        windows.set_save_target(Some(PathBuf::from("D:/work/copy.txt")));
        assert_eq!(
            windows
                .pick_save_target(None, Path::new("D:/work/source.txt"))
                .unwrap(),
            Some(PathBuf::from("D:/work/copy.txt"))
        );
        windows.set_save_target(None);
        assert_eq!(
            windows
                .pick_save_target(None, Path::new("D:/work/source.txt"))
                .unwrap(),
            None
        );
    }
}
