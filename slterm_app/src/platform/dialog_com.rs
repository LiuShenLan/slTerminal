//! 原生文件／目录对话框共享的 STA 与错误映射。
//!
//! 对话框是 STA COM 模态窗口：调用线程初始化 STA，退出时只释放本线程
//! 的初始化；错误只记录 HRESULT，不插值路径或用户输入。

use std::io;

use windows::Win32::Foundation::{ERROR_CANCELLED, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::core::HRESULT;

/// 后台线程 STA：对话框要求单线程单元；退出时只释放本线程的初始化。
pub(super) struct ComApartment(bool);

impl ComApartment {
    pub(super) fn initialize() -> io::Result<Self> {
        // SAFETY: CoInitializeEx 无前置条件；成对调用由 Drop 保证。
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_ok() {
            Ok(Self(true))
        } else if result == RPC_E_CHANGED_MODE {
            Err(io::Error::other("对话框需要 STA 线程"))
        } else {
            Err(com_error(result, "初始化对话框单元"))
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

pub(super) fn is_cancelled(code: HRESULT) -> bool {
    // HRESULT_FROM_WIN32(ERROR_CANCELLED) = 0x8007_04C7。
    code == HRESULT((0x8007_0000u32 | ERROR_CANCELLED.0) as i32)
}

/// 对话框错误只保留操作类别与 HRESULT，不插值路径。
pub(super) fn com_error(code: HRESULT, operation: &str) -> io::Error {
    io::Error::other(format!("{operation}失败: hresult={:#010x}", code.0))
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
    fn com_errors_do_not_echo_paths_or_user_input() {
        let error = com_error(HRESULT(0x8000_4005u32 as i32), "打开目录对话框");
        assert!(error.to_string().starts_with("打开目录对话框失败"));
        assert!(!error.to_string().contains("D:/"));
    }
}
