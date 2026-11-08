//! 最终退出提交失败的原生确认；只在 GPUI 外全局关闭工作线程调用。

use std::ptr;

use slterm_terminal::tty::windows::win32_string;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDCANCEL, IDRETRY, MB_ICONWARNING, MB_RETRYCANCEL, MB_SETFOREGROUND, MB_TASKMODAL, MessageBoxW,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CloseDialogError;

/// 主窗存活时以其 HWND 为 owner；只有 GPUI 已退出的兜底路径才传 `None`。
/// 返回 true 表示重试，false 表示用户明确接受旧布局与快照可能恢复。
pub(crate) fn retry_final_commit(owner: Option<isize>) -> Result<bool, CloseDialogError> {
    let message = win32_string(
        "退出状态保存失败。\n\n选择“重试”重新提交；选择“取消”退出且不保存本次窗口状态，旧布局与可能残留的快照会在下次启动恢复。",
    );
    let title = win32_string(crate::brand::NAME);
    let owner = owner.map_or(ptr::null_mut(), |hwnd| {
        hwnd as windows_sys::Win32::Foundation::HWND
    });
    // SAFETY: owner 是仍存活的主窗或空句柄；窗口管理器负责模态禁用 owner。
    // 本调用不取得句柄所有权，文本不含路径或存储细节。
    let result = unsafe {
        MessageBoxW(
            owner,
            message.as_ptr(),
            title.as_ptr(),
            MB_RETRYCANCEL | MB_ICONWARNING | MB_SETFOREGROUND | MB_TASKMODAL,
        )
    };
    if result == 0 {
        // SAFETY: GetLastError 无前置条件，只读取当前工作线程错误码。
        let code = unsafe { GetLastError() };
        log::warn!("final commit confirmation dialog failed: os={code}");
    }
    map_message_box_result(result)
}

fn map_message_box_result(result: i32) -> Result<bool, CloseDialogError> {
    match result {
        IDRETRY => Ok(true),
        IDCANCEL => Ok(false),
        _ => Err(CloseDialogError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_box_failure_is_not_explicit_abandon() {
        assert!(map_message_box_result(0).is_err());
        assert!(matches!(map_message_box_result(IDRETRY), Ok(true)));
        assert!(matches!(
            map_message_box_result(windows_sys::Win32::UI::WindowsAndMessaging::IDCANCEL),
            Ok(false)
        ));
        assert!(map_message_box_result(-1).is_err());
    }
}
