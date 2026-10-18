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
/// 只看句柄。
pub(crate) fn try_lifetime_lock(path: &Path) -> io::Result<Option<LifetimeFileLock>> {
    use std::os::windows::fs::OpenOptionsExt as _;

    let lock_path = lock_path(path);
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
    Ok(Some(LifetimeFileLock { _file: file }))
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

fn lock_path(path: &Path) -> PathBuf {
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
    // 句柄即所有权;Drop 关闭句柄由 File 自身完成。字段从不读——持有即锁。
    _file: File,
}

/// MoveFileExW + REPLACE_EXISTING:读者永远看不到撕裂文件,崩溃留下原件。
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
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
    use super::{try_lifetime_lock, try_lock, write};

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

        let owner = try_lifetime_lock(&path)
            .unwrap()
            .expect("first lifetime lock");
        assert!(try_lifetime_lock(&path).unwrap().is_none());
        drop(owner);
        assert!(try_lifetime_lock(&path).unwrap().is_some());
    }
}
