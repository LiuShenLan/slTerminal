//! 系统文件操作:回收站删除与系统处理器打开(07 篇 M8.4)。
//!
//! 删除走 `SHFileOperationW` + `FOF_ALLOWUNDO`(照抄点 25):对齐系统回收站,
//! 不提供「永久删除」旁路。打开走 `ShellExecuteW`:文件树双击未命中应用内
//! 路由时交系统默认处理器(07 篇「打开路由」末臂)。
//!
//! 两者都必须无提示、无控制台窗口:树上一次误触不应弹出系统确认框。

use std::path::Path;

/// 回收站删除的系统标志:允许撤销、全静默。
fn recycle_bin_flags() -> u16 {
    use windows_sys::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    };
    (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16
}

/// 把路径送进系统回收站(失败 = 权限/占用/路径消失,返回用户可读消息)。
#[allow(dead_code)] // 消费点 = M8.5 文件树右键「删除」菜单;本步用例经注入通道直调。
pub(crate) fn send_to_recycle_bin(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;

    if !path.exists() {
        return Err("路径已不存在".to_owned());
    }
    let mut source: Vec<u16> = path.as_os_str().encode_wide().collect();
    // pFrom 必须是双 NUL 结尾的多字符串。
    source.push(0);
    source.push(0);
    run_recycle_operation(&mut source, |operation| unsafe {
        windows_sys::Win32::UI::Shell::SHFileOperationW(operation)
    })
}

/// 组装并执行一次删除;`run` 是系统调用注入点(测试断言标志位/错误码映射)。
fn run_recycle_operation(
    source: &mut [u16],
    run: impl FnOnce(&mut windows_sys::Win32::UI::Shell::SHFILEOPSTRUCTW) -> i32,
) -> Result<(), String> {
    use std::ptr;
    use windows_sys::Win32::UI::Shell::{FO_DELETE, SHFILEOPSTRUCTW};

    let mut operation = SHFILEOPSTRUCTW {
        hwnd: ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: source.as_ptr(),
        pTo: ptr::null(),
        fFlags: recycle_bin_flags(),
        fAnyOperationsAborted: 0,
        hNameMappings: ptr::null_mut(),
        lpszProgressTitle: ptr::null(),
    };
    let code = run(&mut operation);
    if code == 0 && operation.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        Err(format!("系统拒绝（代码 {code}）"))
    }
}

/// 交给系统默认处理器打开路径(文件树双击的未命中臂)。
#[cfg(not(all(test, feature = "gpui-test-support")))]
pub(crate) fn open_in_system(path: &Path) {
    use std::os::windows::ffi::OsStrExt as _;
    use std::ptr;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide = |value: &std::ffi::OsStr| -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    };
    let file = wide(path.as_os_str());
    let operation = wide(std::ffi::OsStr::new("open"));
    // 返回值 > 32 才是成功;失败静默(用户已经看到一个没反应的双击,
    // 再弹系统错误框只会更吵)。
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(test)]
mod file_operations_tests {
    use super::*;
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    };

    /// FOF_ALLOWUNDO 必须在场(删除对齐回收站),且全程静默。
    #[test]
    fn recycle_flags_allow_undo_without_prompts() {
        let flags = recycle_bin_flags();
        for (flag, name) in [
            (FOF_ALLOWUNDO, "FOF_ALLOWUNDO"),
            (FOF_NOCONFIRMATION, "FOF_NOCONFIRMATION"),
            (FOF_SILENT, "FOF_SILENT"),
            (FOF_NOERRORUI, "FOF_NOERRORUI"),
        ] {
            assert_ne!(flags & flag as u16, 0, "{name} 必须置位");
        }
    }

    /// pFrom 双 NUL、FO_DELETE、静默标志;成功码 0 映射 Ok。
    #[test]
    fn recycle_request_encodes_a_double_nul_terminated_delete() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("victim.txt");
        std::fs::write(&path, b"x").unwrap();
        let expected: Vec<u16> = {
            use std::os::windows::ffi::OsStrExt as _;
            path.as_os_str().encode_wide().collect()
        };
        let mut source = expected.clone();
        source.push(0);
        source.push(0);
        run_recycle_operation(&mut source, |operation| {
            assert_eq!(operation.wFunc, FO_DELETE);
            let observed =
                unsafe { std::slice::from_raw_parts(operation.pFrom, expected.len() + 2) };
            assert_eq!(&observed[..expected.len()], expected.as_slice());
            assert_eq!(&observed[expected.len()..], &[0, 0], "双 NUL 结尾");
            assert_ne!(
                operation.fFlags & FOF_ALLOWUNDO as u16,
                0,
                "系统调用必须带 FOF_ALLOWUNDO"
            );
            0
        })
        .expect("成功码必须映射 Ok");
    }

    /// 错误码与用户取消(aborted)分别映射为用户可读错误。
    #[test]
    fn recycle_errors_map_to_readable_messages() {
        use windows_sys::Win32::UI::Shell::SHFILEOPSTRUCTW;
        let mut source = vec![b'x' as u16, 0, 0];
        let failure = run_recycle_operation(&mut source, |_operation: &mut SHFILEOPSTRUCTW| 5);
        assert!(failure.unwrap_err().contains("代码 5"));

        let aborted = run_recycle_operation(&mut source, |operation| {
            operation.fAnyOperationsAborted = 1;
            0
        });
        assert!(aborted.is_err(), "用户取消不能报成功");
    }

    /// 消失的路径不进入系统调用。
    #[test]
    fn missing_path_is_rejected_before_the_system_call() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("gone.txt");
        let error = send_to_recycle_bin(&path).expect_err("不存在的路径必须拒绝");
        assert_eq!(error, "路径已不存在");
    }
}
