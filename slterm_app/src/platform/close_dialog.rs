//! 最终退出提交失败的原生确认；只在 GPUI 已退出后调用。

use std::ptr;

use slterm_terminal::tty::windows::win32_string;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDRETRY, MB_ICONWARNING, MB_RETRYCANCEL, MB_SETFOREGROUND, MB_TASKMODAL, MessageBoxW,
};

/// 返回 true 表示重试；否则用户明确接受旧布局与快照可能在下次启动恢复。
pub(crate) fn retry_final_commit() -> bool {
    let message = win32_string(
        "退出状态保存失败。\n\n选择“重试”重新提交；选择“取消”退出且不保存本次窗口状态，旧布局与可能残留的快照会在下次启动恢复。",
    );
    let title = win32_string(crate::brand::NAME);
    // SAFETY: GPUI 已退出，原生模态框不会重入 AppCell；文本不含路径或存储细节。
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_RETRYCANCEL | MB_ICONWARNING | MB_SETFOREGROUND | MB_TASKMODAL,
        ) == IDRETRY
    }
}
