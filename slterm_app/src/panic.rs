//! 启动期错误弹面 + panic 钩(照上游 panic.rs 裁剪:Windows-only,平台臂删除)。
//!
//! 启动失败先于 logger/panic 初始化,且不得触发数据目录创建之外的副作用。

use std::fs::OpenOptions;
use std::io::Write;
use std::{io, panic, ptr, thread};

use windows_sys::Win32::System::Console::GetConsoleCP;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TASKMODAL, MessageBoxW,
};

use slterm_terminal::tty::windows::win32_string;

/// 启动期错误上弹面:logger 未起、控制台可能没有(双击启动)时的最后通知手段。
pub fn report_startup_error(error: &dyn std::fmt::Display, gui_launch: bool) {
    let has_console = unsafe { GetConsoleCP() } != 0;
    if !startup_error_needs_dialog(gui_launch, has_console) || cfg!(test) {
        return;
    }
    let message = win32_string(&error.to_string());
    let title = win32_string(crate::brand::NAME);
    // SAFETY: 启动仍是单线程,未进 GUI 事件循环。
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR | MB_OK | MB_SETFOREGROUND | MB_TASKMODAL,
        );
    }
}

/// 只给「GUI 人格启动且无控制台」的场景弹面;有控制台时 stderr 已经够用。
/// console 子系统下双击启动会自带控制台,弹面率较上游 GUI 子系统下降
/// 属已接受后果(标准流正确性优先)。
fn startup_error_needs_dialog(gui_launch: bool, has_console: bool) -> bool {
    gui_launch && !has_console
}

/// 安装 panic 钩:写 stderr + 持久日志,并弹经典 Windows 错误框。
pub fn attach_handler() {
    let panic_log = crate::platform::dirs::local_dir().join("slterm-panic.log");
    panic::set_hook(Box::new(move |panic_info| {
        let message = panic_info.to_string();
        let _ = writeln!(io::stderr(), "{message}");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&panic_log)
        {
            let _ = writeln!(file, "{message}\n");
        }

        let dialog_message = win32_string(&format!("{message}\n\nPress Ctrl-C to Copy"));
        let dialog_title = win32_string(&format!("{}: Runtime Error", crate::brand::NAME));

        // MessageBox 的模态消息泵不能跑在发生 panic 的 GPUI 线程上——会重入
        // AppCell 把原始 panic 升级成无诊断信息的 double panic。专用线程 + join
        // 保证主线程 unwind 退出前用户能看到并复制错误(因果链照上游)。
        if let Ok(dialog_thread) =
            thread::Builder::new()
                .name("panic-dialog".into())
                .spawn(move || unsafe {
                    MessageBoxW(
                        ptr::null_mut(),
                        dialog_message.as_ptr(),
                        dialog_title.as_ptr(),
                        MB_ICONERROR | MB_OK | MB_SETFOREGROUND | MB_TASKMODAL,
                    );
                })
        {
            let _ = dialog_thread.join();
        }
    }));
}

#[cfg(test)]
mod panic_tests {
    #[test]
    fn only_gui_startup_without_a_console_uses_an_error_dialog() {
        assert!(super::startup_error_needs_dialog(true, false));
        assert!(!super::startup_error_needs_dialog(true, true));
        assert!(!super::startup_error_needs_dialog(false, false));
        assert!(!super::startup_error_needs_dialog(false, true));
    }
}
