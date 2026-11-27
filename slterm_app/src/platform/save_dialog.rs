//! 另存为目标的原生文件对话框；只在后台执行体调用。
//!
//! 文件对话框是 STA COM 模态窗口：本模块在调用线程初始化 STA，把主窗
//! HWND 作为 owner，取消不视为错误。业务侧只经 WindowsIntegration 端口
//! 消费，真实桌面交互按测试豁免人工核验。

use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileSaveDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::HSTRING;

use super::dialog_com::{ComApartment, com_error, is_cancelled};

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
            .map_err(|error| com_error(error.code(), "保存对话框"))?;
        let options = dialog
            .GetOptions()
            .map_err(|error| com_error(error.code(), "保存对话框"))?;
        dialog
            .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)
            .map_err(|error| com_error(error.code(), "保存对话框"))?;
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
                Err(com_error(error.code(), "保存对话框"))
            };
        }
        let item = dialog
            .GetResult()
            .map_err(|error| com_error(error.code(), "保存对话框"))?;
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| com_error(error.code(), "保存对话框"))?;
        let text = name
            .to_string()
            .map_err(|_| io::Error::other("保存目标路径不是有效 Unicode"))?;
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
