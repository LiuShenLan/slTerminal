//! 系统文件操作:回收站删除、系统处理器打开与文件树 CRUD。
//!
//! 删除走 `SHFileOperationW` + `FOF_ALLOWUNDO`(照抄点 25):对齐系统回收站,
//! 不提供「永久删除」旁路。打开走 `ShellExecuteW`:文件树双击未命中应用内
//! 路由时交系统默认处理器(「打开路由」末臂)。
//!
//! 新建/重命名是文件树右键菜单的唯一执行体:名字合法性、父目录存在与
//! 已存在判定都在这里统一收口,调用方只做展示层校验。
//!
//! 所有操作都必须无提示、无控制台窗口:树上一次误触不应弹出系统确认框。

use std::io;
use std::path::Path;

/// 回收站删除的系统标志:允许撤销、全静默。
fn recycle_bin_flags() -> u16 {
    use windows_sys::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    };
    (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16
}

/// 把路径送进系统回收站(失败 = 权限/占用/路径消失,返回用户可读消息)。
#[allow(dead_code)] // 消费点 = 文件树右键「删除」菜单;本模块用例经注入通道直调。
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

/// 单段名字合法性(Windows 文件/文件夹名)。
///
/// 空名、`.`/`..`、路径分隔符、控制字符、`<>:"|?*`、尾随空格/点与保留
/// 设备名一律拒绝;调用方与平台共用这一份真值,避免两处规则漂移。
pub(crate) fn validate_entry_name(name: &str) -> io::Result<()> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(' ')
        || name.ends_with('.')
        || name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        });
    if invalid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Name is not valid on Windows",
        ));
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Name is reserved on Windows",
        ));
    }
    Ok(())
}

/// 校验整条路径的最后一段名字(父目录由 [`ensure_parent_directory`] 负责)。
fn validate_entry_path(path: &Path) -> io::Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Path must end with a file or folder name",
            )
        })?;
    validate_entry_name(name)
}

/// 新建条目的父目录必须存在(不自动补建中间层)。
fn ensure_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no parent directory")
        })?;
    if parent.is_dir() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Parent directory does not exist",
        ))
    }
}

/// 新建空文件;已存在 → `AlreadyExists`,父目录缺失 → `NotFound`。
pub(crate) fn create_file(path: &Path) -> io::Result<()> {
    validate_entry_path(path)?;
    ensure_parent_directory(path)?;
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Path already exists",
        ));
    }
    std::fs::File::create_new(path).map(|_| ())
}

/// 新建单个目录(不递归);错误语义同 [`create_file`]。
pub(crate) fn create_directory(path: &Path) -> io::Result<()> {
    validate_entry_path(path)?;
    ensure_parent_directory(path)?;
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Path already exists",
        ));
    }
    std::fs::create_dir(path)
}

/// 重命名/移动条目;源缺失 → `NotFound`,目标已存在 → `AlreadyExists`。
///
/// `std::fs::rename` 在 Windows 带 `MOVEFILE_REPLACE_EXISTING`,会静默覆盖
/// 同名目标;这里必须自己挡住。唯一放行的是「同一文件换大小写」：二者
/// canonicalize 后相同,交给系统完成大小写变更。
pub(crate) fn rename_path(from: &Path, to: &Path) -> io::Result<()> {
    validate_entry_path(to)?;
    if !from.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Source path does not exist",
        ));
    }
    if from == to {
        return Ok(());
    }
    if to.try_exists()? {
        let same_entry = from
            .canonicalize()
            .ok()
            .zip(to.canonicalize().ok())
            .is_some_and(|(from, to)| from == to);
        if !same_entry {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Target path already exists",
            ));
        }
    }
    std::fs::rename(from, to)
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

    /// 新建文件/目录真实落盘;文件内容为空。
    #[test]
    fn create_file_and_directory_land_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("new.txt");
        create_file(&file).unwrap();
        assert!(file.is_file());
        assert_eq!(std::fs::read(&file).unwrap(), b"");

        let folder = directory.path().join("new_dir");
        create_directory(&folder).unwrap();
        assert!(folder.is_dir());
    }

    /// 已存在与父目录缺失分别映射 `AlreadyExists` / `NotFound`。
    #[test]
    fn create_rejects_existing_paths_and_missing_parents() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("exists.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(
            create_file(&file).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            create_directory(&file).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            create_directory(directory.path()).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists,
            "临时目录本身已存在"
        );
        assert_eq!(
            create_file(&directory.path().join("missing/child.txt"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }

    /// 重命名搬运内容;源缺失与目标已存在分别拒绝。
    #[test]
    fn rename_moves_entries_and_rejects_missing_or_existing_targets() {
        let directory = tempfile::tempdir().unwrap();
        let from = directory.path().join("old.txt");
        let to = directory.path().join("new.txt");
        std::fs::write(&from, b"content").unwrap();
        rename_path(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(std::fs::read(&to).unwrap(), b"content");

        assert_eq!(
            rename_path(&directory.path().join("gone.txt"), &to)
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        let occupied = directory.path().join("occupied.txt");
        std::fs::write(&occupied, b"y").unwrap();
        assert_eq!(
            rename_path(&to, &occupied).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        // 原名重命名是幂等 no-op,不触碰磁盘。
        rename_path(&to, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"content");
    }

    /// 非法名字在触碰磁盘前就被拒绝;合法 Unicode 名放行。
    #[test]
    fn invalid_names_are_rejected_before_touching_the_disk() {
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "bad:name",
            "bad*name",
            "bad?name",
            "CON",
            "NUL.txt",
            "trail.",
            "trail ",
            "line\nbreak",
        ] {
            assert_eq!(
                validate_entry_name(name).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{name:?} 必须拒绝"
            );
        }
        validate_entry_name("正常 文件-1.txt").unwrap();

        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            create_file(&directory.path().join("bad?name"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!directory.path().join("bad?name").exists());
    }
}
