//! 持久状态文件的小原语:临时文件 + 原子替换,崩溃不留半写 JSON;
//! 同级锁文件防两个 slterm 实例同时改写同一配置。
//!
//! 上游对照:unix 臂(flock / rename 后 sync 父目录)随平台裁剪整支不迁;
//! Windows 形态 = `MoveFileExW`(REPLACE_EXISTING | WRITE_THROUGH)+ 句柄
//! 独占的 lifetime 锁(进程崩溃内核即释放,不存在「超时回收」窗口)。

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

/// 原子替换前后稳定不变的文件身份，用于防止旧所有者误删替代文件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    volume_serial_number: u32,
    file_index: u64,
}

/// 以临时文件 + 原子替换把 `contents` 落到 `path`,先 sync 临时文件再换名。
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_with_acl(path, contents, copy_target_acl).map(|_| ())
}

/// 发布只许当前用户访问的文件；目标旧 ACL 不得继承到替代文件。
pub(crate) fn write_current_user(path: &Path, contents: &[u8]) -> io::Result<FileIdentity> {
    write_with_acl(path, contents, |_, temporary| {
        restrict_to_current_user(temporary)
    })
}

fn write_with_acl(
    path: &Path,
    contents: &[u8],
    copy_acl: fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<FileIdentity> {
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
        let identity = file_identity(&file)?;
        drop(file);
        copy_acl(path, &temporary)?;
        replace(&temporary, path)?;
        Ok(identity)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn file_identity(file: &File) -> io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    // SAFETY: 该 C 输出结构仅含整数，全零是位级有效初始值。
    let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: file 是有效打开句柄，输出结构在本栈帧内可写。
    let queried = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut information) };
    if queried == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileIdentity {
        volume_serial_number: information.dwVolumeSerialNumber,
        file_index: u64::from(information.nFileIndexHigh) << 32
            | u64::from(information.nFileIndexLow),
    })
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

/// 把 DACL 收敛为当前令牌用户的一条显式允许 ACE，并切断父目录继承。
fn restrict_to_current_user(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GetLastError, LocalFree,
    };
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, SetEntriesInAclW,
        SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetTokenInformation, NO_INHERITANCE,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = ptr::null_mut();
    // SAFETY: 进程伪句柄只用于本调用；成功取得的 token 在闭包结束后关闭。
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut byte_len = 0_u32;
        // SAFETY: 空缓冲用于查询所需大小；byte_len 是有效输出指针。
        let queried =
            unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut byte_len) };
        if queried != 0 {
            return Err(io::Error::other("TokenUser 空缓冲查询意外成功"));
        }
        // SAFETY: 上一 Win32 调用刚返回，立即读取本线程错误码。
        let query_error = unsafe { GetLastError() };
        if query_error != ERROR_INSUFFICIENT_BUFFER || byte_len == 0 {
            return Err(io::Error::from_raw_os_error(query_error as i32));
        }
        let mut buffer = vec![0_usize; byte_len.div_ceil(size_of::<usize>() as u32) as usize];
        // SAFETY: usize 缓冲满足 TOKEN_USER 对齐，容量不小于 Win32 返回的 byte_len。
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                byte_len,
                &mut byte_len,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: 成功的 TokenUser 查询保证缓冲以 TOKEN_USER 开头，SID 随缓冲存活。
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: FILE_ALL_ACCESS,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: ptr::null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: user.User.Sid.cast(),
            },
        };
        let mut dacl = ptr::null_mut();
        // SAFETY: access 与 SID 在调用期间有效；oldacl 为空表示不合并任何旧 ACE。
        let built = unsafe { SetEntriesInAclW(1, &raw const access, ptr::null(), &mut dacl) };
        if built != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(built as i32));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: 路径与新建 DACL 有效；PROTECTED 标志显式禁止继承 ACE。
        let written = unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl,
                ptr::null(),
            )
        };
        // SAFETY: dacl 由 SetEntriesInAclW 分配，SetNamedSecurityInfoW 后不再需要。
        unsafe { LocalFree(dacl.cast()) };
        if written != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(written as i32));
        }
        Ok(())
    })();
    // SAFETY: token 由 OpenProcessToken 成功返回，闭包已停止使用它。
    unsafe { CloseHandle(token) };
    result
}

/// 保留目标文件已有的 owner/group/DACL。
///
/// 临时文件只继承父目录的默认 ACL;若目标原本带显式 ACE,直接 rename 会让
/// 这些 ACE 丢失。目标不存在时允许新文件继承父目录 ACL；其他读取或设置
/// 失败必须返回错误，调用方不得继续替换目标。
fn copy_target_acl(target: &Path, temporary: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, LocalFree,
    };
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

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
        if matches!(read, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Ok(());
        }
        return Err(io::Error::from_raw_os_error(read as i32));
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
        return Err(io::Error::from_raw_os_error(written as i32));
    }
    Ok(())
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

/// 仅当路径仍指向原文件且内容一致时，按已打开句柄标记删除。
/// 比对期间排除所有其他读写句柄；最终组件按 reparse 点自身打开。
/// 路径在打开后被替换时，删除只作用于旧句柄。
pub(crate) fn remove_if_matches(
    path: &Path,
    expected_identity: FileIdentity,
    expected_contents: &[u8],
) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::Foundation::GENERIC_READ;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FileDispositionInfo, SetFileInformationByHandle,
    };

    let mut file = OpenOptions::new()
        .read(true)
        .access_mode(DELETE | GENERIC_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .share_mode(0)
        .open(path)?;
    if file_identity(&file)? != expected_identity {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "文件身份已变化，拒绝由旧所有者删除",
        ));
    }
    let mut contents = Vec::new();
    Read::by_ref(&mut file)
        .take(expected_contents.len() as u64 + 1)
        .read_to_end(&mut contents)?;
    if contents != expected_contents {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "文件内容已变化，拒绝由旧所有者删除",
        ));
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: 1 };
    // SAFETY: file 具有 DELETE 权限；disposition 指针与字节长度匹配且在本调用内有效。
    let marked = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&raw const disposition).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if marked == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod atomic_file_tests {
    use std::path::Path;

    use super::{
        copy_target_acl, file_identity, remove_if_matches, try_lifetime_lock, try_lock, write,
        write_with_acl,
    };

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

    /// ACL 复制失败不得替换目标，也不得留下临时文件。
    #[test]
    fn acl_failure_aborts_replace_and_preserves_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write(&path, b"before").unwrap();

        let result = write_with_acl(&path, b"after", |_, _| {
            Err(std::io::Error::other("injected ACL failure"))
        });

        assert!(result.is_err(), "ACL 失败必须使写入失败");
        assert_eq!(std::fs::read(&path).unwrap(), b"before");
        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".slterm-tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "失败后残留临时文件: {leftovers:?}");
    }

    /// Win32 ACL 设置失败必须作为错误返回，不能降级为告警。
    #[test]
    fn acl_copy_reports_setter_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write(&path, b"before").unwrap();
        let missing_temporary = dir.path().join("missing-temporary");

        assert!(copy_target_acl(&path, &missing_temporary).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"before");
    }

    /// 最终组件是 reparse 点时，必须核对链接自身而非其目标身份。
    #[test]
    fn removal_rejects_final_reparse_point_instead_of_following_target() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};

        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE,
        };

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("endpoint-link");
        std::fs::create_dir(&target).unwrap();
        let target_contents = target.join("record.json");
        std::fs::write(&target_contents, b"keep-target").unwrap();
        let linked = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .unwrap();
        assert!(linked.status.success(), "必须真实创建 NTFS junction");
        let metadata = std::fs::symlink_metadata(&link).unwrap();
        assert_ne!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);

        let target_handle = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(&target)
            .unwrap();
        let target_identity = file_identity(&target_handle).unwrap();
        let error = remove_if_matches(&link, target_identity, b"").unwrap_err();
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::AlreadyExists,
            "必须识别 reparse 点身份变化，而不是跟随目标"
        );
        drop(target_handle);
        let metadata = std::fs::symlink_metadata(&link).unwrap();
        assert_ne!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
        assert_eq!(std::fs::read(&target_contents).unwrap(), b"keep-target");
    }

    /// 无显式 ACE 时 ACL 完全由父目录继承,复制与否不可区分;先用系统自带
    /// `icacls` 加一条显式 ACE 再比对。icacls 不可用或加 ACE 失败时跳过。
    /// 原子替换必须保留目标文件原有的显式 DACL。
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
