//! OS 单实例所有权：锁独立于任何控制面服务，进程退出即释放句柄。

use std::io;
use std::path::Path;

/// 持有即拥有本数据目录的实例权；Drop 由内部锁先关句柄再清理标记。
pub(crate) struct InstanceGuard {
    _lock: crate::atomic_file::LifetimeFileLock,
}

/// 在 UI、PTY、watcher 与后台任务启动前取得实例锁。
pub(crate) fn acquire() -> io::Result<InstanceGuard> {
    acquire_at(&crate::platform::dirs::local_dir().join("instance"))
}

fn acquire_at(marker: &Path) -> io::Result<InstanceGuard> {
    let lock = crate::atomic_file::try_lifetime_lock_exact(marker)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "another slTerminal instance owns this data directory; handover is unavailable, refusing to open a second window",
        )
    })?;
    Ok(InstanceGuard { _lock: lock })
}

#[cfg(test)]
mod tests {
    use super::acquire_at;

    #[test]
    fn second_instance_without_handover_exits_without_opening_a_window() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("instance");
        let first = acquire_at(&marker).expect("首个实例必须取得 OS 锁");

        let error = match acquire_at(&marker) {
            Err(error) => error,
            Ok(_) => panic!("无移交服务时第二实例必须失败"),
        };
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

        drop(first);
        acquire_at(&marker).expect("首实例退出后必须能重新取得锁");
    }

    #[test]
    fn instance_lock_uses_the_exact_marker_path() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("instance");
        let sidecar = directory.path().join("instance.slterm-lock");
        let guard = acquire_at(&marker).expect("首个实例必须取得 OS 锁");

        assert!(marker.exists(), "持锁期间精确 instance 文件必须存在");
        assert!(!sidecar.exists(), "实例锁不得隐式追加扩展名");

        drop(guard);
        assert!(!marker.exists(), "正常释放后必须清理 instance 文件");
    }
}
