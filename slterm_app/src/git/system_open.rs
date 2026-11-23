//! GitShow 系统打开的只读中转材料；固定 blob 不写入工作区。

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::show::GitShowSnapshot;

const TEMP_FILE_PREFIX: &str = "gitshow-";
const ORPHAN_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// 系统处理器使用期间持有中转文件；Drop 同时覆盖视图关闭与正常退出。
#[derive(Debug)]
pub(crate) struct GitShowOpenMaterial {
    path: PathBuf,
}

impl GitShowOpenMaterial {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for GitShowOpenMaterial {
    fn drop(&mut self) {
        let _ = remove_readonly_file(&self.path);
    }
}

#[allow(dead_code)] // 消费点归后续 GitShow 视图系统打开命令。
pub(crate) fn open_snapshot(snapshot: &GitShowSnapshot) -> io::Result<GitShowOpenMaterial> {
    let root = std::env::temp_dir().join("slterm").join("gitshow");
    open_snapshot_at(snapshot, &root, |path| {
        crate::platform::file_operations::try_open_in_system(path)
    })
}

pub(crate) fn open_snapshot_at(
    snapshot: &GitShowSnapshot,
    root: &Path,
    opener: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<GitShowOpenMaterial> {
    std::fs::create_dir_all(root)?;
    let extension = Path::new(snapshot.path.as_str())
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let mut file = tempfile::Builder::new()
        .prefix(TEMP_FILE_PREFIX)
        .suffix(&extension)
        .tempfile_in(root)?;
    file.write_all(&snapshot.bytes)?;
    file.flush()?;

    let temp_path = file.into_temp_path();
    let path = temp_path.to_path_buf();
    if let Err(error) = temp_path.keep() {
        let _ = remove_readonly_file(&path);
        return Err(error.error);
    }
    if let Err(error) = make_readonly(&path) {
        let _ = remove_readonly_file(&path);
        return Err(error);
    }
    if let Err(error) = opener(&path) {
        let _ = remove_readonly_file(&path);
        return Err(error);
    }
    Ok(GitShowOpenMaterial { path })
}

pub(crate) fn cleanup_expired_system_open() -> io::Result<()> {
    let root = std::env::temp_dir().join("slterm").join("gitshow");
    cleanup_expired_at(&root, SystemTime::now())
}

pub(crate) fn cleanup_expired_at(root: &Path, now: SystemTime) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(TEMP_FILE_PREFIX))
        {
            continue;
        }
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let expired = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= ORPHAN_MAX_AGE);
        if expired {
            remove_readonly_file(&path)?;
        }
    }
    Ok(())
}

fn make_readonly(path: &Path) -> io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions)
}

// Windows 的只读属性不会转移删除授权；清理自有中转文件前必须明确解除。
#[allow(clippy::permissions_set_readonly_false)]
fn remove_readonly_file(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.permissions().readonly() && metadata.file_type().is_file() {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions)?;
    }
    std::fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitshow_temp_cleanup_removes_only_expired_owned_files() {
        let root = tempfile::tempdir().unwrap();
        let expired = root.path().join("gitshow-expired.bin");
        let fresh = root.path().join("gitshow-fresh.bin");
        let unrelated = root.path().join("unrelated.bin");
        std::fs::write(&expired, b"old").unwrap();
        std::fs::write(&fresh, b"new").unwrap();
        std::fs::write(&unrelated, b"keep").unwrap();
        let now = SystemTime::now();
        std::fs::File::options()
            .write(true)
            .open(&expired)
            .unwrap()
            .set_modified(now - ORPHAN_MAX_AGE)
            .unwrap();
        let mut permissions = expired.metadata().unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&expired, permissions).unwrap();

        cleanup_expired_at(root.path(), now).unwrap();

        assert!(!expired.exists());
        assert!(fresh.exists());
        assert!(unrelated.exists());
    }
}
