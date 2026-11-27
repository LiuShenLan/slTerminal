//! 会话结束守卫：系统关机/注销先取消本次会话结束，再进入统一关闭链。
//!
//! Windows 在 `WM_QUERYENDSESSION` 同步等待应答；同步保存会阻塞消息循环。
//! 因此守卫返回 FALSE 取消本次关机/注销，并把事件投递到壳事件泵，由
//! AppRoot 复用用户关窗的同一关闭链保存 Document、停止 PTY 与冲刷状态。

use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::WM_QUERYENDSESSION;

/// 子类身份；单窗口单实例下固定值。
const SESSION_END_SUBCLASS_ID: usize = 0x5E_5E;

static SESSION_END_CALLBACK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// 安装会话结束守卫；只允许在主窗口 HWND 上调用一次。
pub(crate) fn install_session_end_guard(
    hwnd: isize,
    on_session_end: impl Fn() + Send + Sync + 'static,
) -> Result<(), u32> {
    let _ = SESSION_END_CALLBACK.set(Box::new(on_session_end));
    // SAFETY: HWND 来自已创建并发布的 GPUI 主窗口；子类过程只在消息循环线程执行。
    let installed = unsafe {
        SetWindowSubclass(
            hwnd as HWND,
            Some(session_end_proc),
            SESSION_END_SUBCLASS_ID,
            0,
        )
    };
    if installed == 0 {
        // SAFETY: 失败后立即读取线程局部错误码；无指针参数。
        return Err(unsafe { GetLastError() });
    }
    Ok(())
}

/// 会话结束消息分类；返回 true 表示已取消本次会话结束并转交统一关闭链。
fn handle_session_message(message: u32) -> bool {
    if message != WM_QUERYENDSESSION {
        return false;
    }
    let Some(callback) = SESSION_END_CALLBACK.get() else {
        return false;
    };
    callback();
    true
}

unsafe extern "system" fn session_end_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    if handle_session_message(message) {
        // FALSE：取消本次关机/注销；统一关闭链完成并退出后由用户重新发起。
        return 0;
    }
    // SAFETY: 未处理的消息交回原窗口过程；参数来自系统消息分发。
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod session_end_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{SESSION_END_CALLBACK, WM_QUERYENDSESSION, handle_session_message};
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;

    /// 只有会话结束查询会转交统一关闭链；普通窗口消息保持原分发。
    #[test]
    fn only_query_end_session_is_deferred_to_the_close_chain() {
        let calls = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&calls);
        assert!(
            SESSION_END_CALLBACK
                .set(Box::new(move || {
                    recorded.fetch_add(1, Ordering::Relaxed);
                }))
                .is_ok()
        );
        assert!(!handle_session_message(WM_CLOSE));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(handle_session_message(WM_QUERYENDSESSION));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
