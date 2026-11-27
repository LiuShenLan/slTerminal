//! 目录选择的原生入口；只在后台执行体调用。
//!
//! 与另存为共用 STA 对话框基础设施；取消不视为错误。业务侧只经
//! `WindowsIntegration` 端口消费，真实桌面交互按测试豁免人工核验。

use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::HSTRING;

use super::dialog_com::{ComApartment, com_error, is_cancelled};

/// 用户选定的目录；`None` = 取消。
pub(crate) fn pick_folder(
    owner: Option<isize>,
    start: Option<&Path>,
) -> io::Result<Option<PathBuf>> {
    let _apartment = ComApartment::initialize()?;
    // SAFETY: 调用线程已初始化 STA；owner 是仍存活的主窗句柄或空值。
    // 对话框只在当前调用期间持有句柄，不复制或延长窗口生命周期。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| com_error(error.code(), "目录对话框"))?;
        let options = dialog
            .GetOptions()
            .map_err(|error| com_error(error.code(), "目录对话框"))?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
            .map_err(|error| com_error(error.code(), "目录对话框"))?;
        if let Some(start) = start {
            // 起点只影响初始位置；失败时沿用系统上次位置。
            let folder: windows::core::Result<IShellItem> =
                SHCreateItemFromParsingName(&HSTRING::from(start.as_os_str()), None);
            if let Ok(folder) = folder {
                let _ = dialog.SetFolder(&folder);
            }
        }
        let hwnd = owner.map(|handle| HWND(handle as *mut _));
        if let Err(error) = dialog.Show(hwnd) {
            return if is_cancelled(error.code()) {
                Ok(None)
            } else {
                Err(com_error(error.code(), "目录对话框"))
            };
        }
        let item = dialog
            .GetResult()
            .map_err(|error| com_error(error.code(), "目录对话框"))?;
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| com_error(error.code(), "目录对话框"))?;
        let text = name
            .to_string()
            .map_err(|_| io::Error::other("目录路径不是有效 Unicode"))?;
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(text)))
    }
}

/// 用户选定的既有文件；`None` = 取消。
pub(crate) fn pick_open_file(
    owner: Option<isize>,
    start: Option<&Path>,
) -> io::Result<Option<PathBuf>> {
    let _apartment = ComApartment::initialize()?;
    // SAFETY: 调用线程已初始化 STA；owner 是仍存活的主窗句柄或空值。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| com_error(error.code(), "打开文件对话框"))?;
        let options = dialog
            .GetOptions()
            .map_err(|error| com_error(error.code(), "打开文件对话框"))?;
        dialog
            .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)
            .map_err(|error| com_error(error.code(), "打开文件对话框"))?;
        if let Some(start) = start {
            let folder: windows::core::Result<IShellItem> =
                SHCreateItemFromParsingName(&HSTRING::from(start.as_os_str()), None);
            if let Ok(folder) = folder {
                let _ = dialog.SetFolder(&folder);
            }
        }
        let hwnd = owner.map(|handle| HWND(handle as *mut _));
        if let Err(error) = dialog.Show(hwnd) {
            return if is_cancelled(error.code()) {
                Ok(None)
            } else {
                Err(com_error(error.code(), "打开文件对话框"))
            };
        }
        let item = dialog
            .GetResult()
            .map_err(|error| com_error(error.code(), "打开文件对话框"))?;
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| com_error(error.code(), "打开文件对话框"))?;
        let text = name
            .to_string()
            .map_err(|_| io::Error::other("文件路径不是有效 Unicode"))?;
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows_integration::{FakeWindowsIntegration, WindowsIntegration as _};

    #[test]
    fn fake_picker_returns_configured_folder_without_a_dialog() {
        let windows = FakeWindowsIntegration::default();
        windows.set_folder_target(Some(PathBuf::from("D:/work/project")));
        assert_eq!(
            windows.pick_folder(None, None).unwrap(),
            Some(PathBuf::from("D:/work/project"))
        );
        windows.set_folder_target(None);
        assert_eq!(windows.pick_folder(None, None).unwrap(), None);
    }
}
