//! 持久状态文件的小原语:临时文件 + 原子替换,崩溃不留半写 JSON;
//! 同级锁文件防两个 slterm 实例同时改写同一配置。
//!
//! 上游对照:unix 臂(flock / rename 后 sync 父目录)随平台裁剪整支不迁;
//! Windows 形态 = `MoveFileExW`(REPLACE_EXISTING | WRITE_THROUGH)+ 句柄
//! 独占的 lifetime 锁(进程崩溃内核即释放,不存在「超时回收」窗口)。

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

/// 以临时文件 + 原子替换把 `contents` 落到 `path`,先 sync 临时文件再换名。
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("state path has no parent"))?;
    std::fs::create_dir_all(parent)?;

    let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let temporary = parent.join(format!(
        ".{file_name}.slterm-tmp-{}-{sequence}",
        std::process::id()
    ));

    let result = (|| {
        // create_new:临时名冲突( pid 复用 + 序号回绕)时报错而不是踩掉
        // 别人的在写临时文件。
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        copy_target_acl(path, &temporary);
        replace(&temporary, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// 尝试拿同级锁文件,不在 UI 线程上等待。
///
/// 短状态写锁:崩溃进程可能留下锁文件。状态写在毫秒内完成,30 秒的锁即可
/// 视为陈旧回收,不会与健康写者竞争。
pub(crate) fn try_lock(path: &Path) -> io::Result<Option<FileLock>> {
    let lock_path = lock_path(path);
    match create_lock(&lock_path) {
        Ok(lock) => Ok(Some(lock)),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let stale = std::fs::metadata(&lock_path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= Duration::from_secs(30));
            if !stale {
                return Ok(None);
            }
            let _ = std::fs::remove_file(&lock_path);
            match create_lock(&lock_path) {
                Ok(lock) => Ok(Some(lock)),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(None),
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

/// 常驻进程的全程锁:依赖 OS 句柄独占(share_mode(0)),进程崩溃内核立即
/// 释放,健康进程无论运行多久都不会被「超时回收」。锁文件可保留,所有权
/// 只看句柄；目标路径按通用 sidecar 规则派生。
pub(crate) fn try_lifetime_lock(path: &Path) -> io::Result<Option<LifetimeFileLock>> {
    try_lifetime_lock_at(lock_path(path))
}

/// 在调用方给定的精确路径上持常驻锁，不追加 sidecar 扩展名。
pub(crate) fn try_lifetime_lock_exact(path: &Path) -> io::Result<Option<LifetimeFileLock>> {
    try_lifetime_lock_at(path.to_owned())
}

fn try_lifetime_lock_at(lock_path: PathBuf) -> io::Result<Option<LifetimeFileLock>> {
    use std::os::windows::fs::OpenOptionsExt as _;

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // ERROR_SHARING_VIOLATION(32)/ERROR_LOCK_VIOLATION(33)= 已被别的实例持有。
    // truncate(false) 显式化:锁内容恒由 set_len(0) + 写 pid 重置,不靠打开截断。
    let mut file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(error) if matches!(error.raw_os_error(), Some(32) | Some(33)) => return Ok(None),
        Err(error) => return Err(error),
    };
    file.set_len(0)?;
    write!(file, "{}", std::process::id())?;
    file.sync_data()?;
    Ok(Some(LifetimeFileLock {
        path: lock_path,
        _file: Some(file),
    }))
}

fn create_lock(path: &Path) -> io::Result<FileLock> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(err) = write!(file, "{}", std::process::id()) {
        // 锁文件一旦创建成功就代表「已占用」;写 PID 失败必须立刻回收,
        // 否则一次磁盘错误会伪装成持续 30 秒的跨进程竞争。
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(err);
    }
    drop(file);
    Ok(FileLock {
        path: path.to_owned(),
    })
}

pub(crate) fn lock_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    path.with_extension(if extension.is_empty() {
        "slterm-lock".to_owned()
    } else {
        format!("{extension}.slterm-lock")
    })
}

pub(crate) struct FileLock {
    path: PathBuf,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) struct LifetimeFileLock {
    path: PathBuf,
    // Option 以便 Drop 中先关闭独占句柄、再删除锁文件(字段析构顺序无法做到)。
    // 句柄即所有权;字段从不读——持有即锁。
    _file: Option<File>,
}

impl Drop for LifetimeFileLock {
    fn drop(&mut self) {
        // 先释放独占句柄再尽力删除锁文件,避免正常退出后安装目录残留
        // 。若另一实例已经打开该文件,`share_mode(0)` 会让删除
        // 因共享冲突失败——忽略即可,锁语义仍由对方句柄持有。
        self._file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 尽力保留目标文件已有的 owner/group/DACL。
///
/// 临时文件只继承父目录的默认 ACL;若目标原本带显式 ACE,直接 rename 会让
/// 这些 ACE 丢失。复制失败只记 warn、**不阻断写**:写成功优先,ACL 保留是
/// 尽力而为。目标不存在时跳过(新文件继承父目录默认 ACL)。
fn copy_target_acl(target: &Path, temporary: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    if !target.exists() {
        return;
    }

    let wide =
        |path: &Path| -> Vec<u16> { path.as_os_str().encode_wide().chain(Some(0)).collect() };
    let source: Vec<u16> = wide(target);
    let destination: Vec<u16> = wide(temporary);
    let information =
        OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
    let mut owner = ptr::null_mut();
    let mut group = ptr::null_mut();
    let mut dacl = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // 安全描述符缓冲区由系统分配,成功后必须 LocalFree。
    let read = unsafe {
        GetNamedSecurityInfoW(
            source.as_ptr(),
            SE_FILE_OBJECT,
            information,
            &mut owner,
            &mut group,
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if read != ERROR_SUCCESS {
        log::warn!("读取目标 ACL 失败(error {read});继续写入,新文件沿用父目录默认 ACL");
        return;
    }
    let written = unsafe {
        SetNamedSecurityInfoW(
            destination.as_ptr(),
            SE_FILE_OBJECT,
            information,
            owner,
            group,
            dacl,
            ptr::null(),
        )
    };
    unsafe { LocalFree(descriptor) };
    if written != ERROR_SUCCESS {
        log::warn!("复制目标 ACL 失败(error {written});继续写入,ACL 保留降级为尽力而为");
    }
}

/// MoveFileExW + REPLACE_EXISTING:读者永远看不到撕裂文件,崩溃留下原件。
pub(crate) fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod atomic_file_tests {
    use std::path::Path;

    use super::{try_lifetime_lock, try_lock, write};

    /// 读取 DACL 的 SDDL 字符串(仅测试断言用)。
    fn dacl_sddl(path: &Path) -> Option<String> {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
        use windows_sys::Win32::Security::Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
            SDDL_REVISION_1, SE_FILE_OBJECT,
        };
        use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};

        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
        let read = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if read != ERROR_SUCCESS {
            return None;
        }
        let mut text: *mut u16 = ptr::null_mut();
        let mut length = 0u32;
        let converted = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                &mut length,
            )
        };
        unsafe { LocalFree(descriptor) };
        if converted == 0 {
            return None;
        }
        let value =
            unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(text, length as usize)) };
        unsafe { LocalFree(text as _) };
        Some(value)
    }

    #[test]
    fn atomic_write_replaces_and_locks_are_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write(&path, b"one").unwrap();
        write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");

        let first = try_lock(&path).unwrap().expect("first lock");
        assert!(try_lock(&path).unwrap().is_none());
        drop(first);
        assert!(try_lock(&path).unwrap().is_some());

        let lock_file = dir.path().join("state.json.slterm-lock");
        let owner = try_lifetime_lock(&path)
            .unwrap()
            .expect("first lifetime lock");
        assert!(lock_file.exists(), "持锁期间锁文件必须在");
        assert!(try_lifetime_lock(&path).unwrap().is_none());
        drop(owner);
        assert!(!lock_file.exists(), "正常释放后锁文件必须删除");
        assert!(try_lifetime_lock(&path).unwrap().is_some());
    }

    /// 写失败路径不得残留临时文件。
    #[test]
    fn failed_replace_removes_the_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        // 目标为目录:MoveFileExW 无法用它替换文件,写入必然失败。
        std::fs::create_dir(&path).unwrap();
        assert!(write(&path, b"content").is_err());
        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".slterm-tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "失败后残留临时文件: {leftovers:?}");
        assert!(path.is_dir(), "失败不得破坏目标");
    }

    /// 原子替换必须保留目标文件原有的显式 DACL。
    ///
    /// 无显式 ACE 时 ACL 完全由父目录继承,复制与否不可区分;先用系统自带
    /// `icacls` 加一条显式 ACE 再比对。icacls 不可用或加 ACE 失败时跳过。
    #[test]
    fn replacement_preserves_an_explicit_target_acl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write(&path, b"before").unwrap();
        let granted = std::process::Command::new("icacls")
            .arg(&path)
            .args(["/grant", "*S-1-1-0:(R)"])
            .output();
        match granted {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                eprintln!(
                    "skip: icacls 未能添加显式 ACE: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return;
            }
            Err(error) => {
                eprintln!("skip: icacls 不可用: {error}");
                return;
            }
        }
        let before = dacl_sddl(&path).expect("读取目标 DACL");
        assert!(
            before.contains("(A;"),
            "icacls 应写入显式 ACE,实际 SDDL: {before}"
        );
        write(&path, b"after").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"after");
        let after = dacl_sddl(&path).expect("读取替换后 DACL");
        assert_eq!(after, before, "原子替换必须保留目标的显式 DACL");
    }
}
