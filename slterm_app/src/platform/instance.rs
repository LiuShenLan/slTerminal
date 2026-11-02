//! OS 单实例所有权：锁独立于任何控制面服务，进程退出即释放句柄。

use std::io;
use std::path::Path;

/// 持有即拥有本数据目录的实例权；Drop 由内部锁先关句柄再清理标记。
pub(crate) struct InstanceGuard {
    _lock: crate::atomic_file::LifetimeFileLock,
}

/// 在 UI、PTY、watcher 与后台任务启动前取得实例锁。
/// 取得实例锁；只有底层锁明确返回已占用时才以 `Ok(None)` 表示冲突。
pub(crate) fn acquire() -> io::Result<Option<InstanceGuard>> {
    acquire_at(&crate::platform::dirs::local_dir().join("instance"))
}

pub(crate) fn acquire_at(marker: &Path) -> io::Result<Option<InstanceGuard>> {
    // 普通 I/O 错误原样上抛，不能用 ErrorKind 猜测锁所有权。
    Ok(crate::atomic_file::try_lifetime_lock_exact(marker)?
        .map(|lock| InstanceGuard { _lock: lock }))
}

#[cfg(test)]
mod tests {
    use super::acquire_at;

    #[test]
    fn second_acquire_reports_conflict_without_io_error() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("instance");
        let first = acquire_at(&marker)
            .expect("首实例锁 I/O 必须成功")
            .expect("首个实例必须取得 OS 锁");

        let conflict = acquire_at(&marker).expect("锁冲突不得伪装成 I/O 错误");
        assert!(conflict.is_none(), "第二次获取必须明确报告冲突");

        drop(first);
        acquire_at(&marker)
            .expect("首实例退出后的锁 I/O 必须成功")
            .expect("首实例退出后必须能重新取得锁");
    }

    #[test]
    fn instance_lock_uses_the_exact_marker_path() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("instance");
        let sidecar = directory.path().join("instance.slterm-lock");
        let guard = acquire_at(&marker)
            .expect("实例锁 I/O 必须成功")
            .expect("首个实例必须取得 OS 锁");

        assert!(marker.exists(), "持锁期间精确 instance 文件必须存在");
        assert!(!sidecar.exists(), "实例锁不得隐式追加扩展名");

        drop(guard);
        assert!(!marker.exists(), "正常释放后必须清理 instance 文件");
    }
}
