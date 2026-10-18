//! 下载元数据的持久证据(`updates/download.json`)。
//!
//! 记录只描述下载结果,**不授予安装权限**;commit.json 才是唯一安装授权。
//! 失败的记录同样落盘,供下次启动「未见过」判定;缓存文件本身在
//! [`load_at`] 中重新做完整校验,损坏即降级为失败态。

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    DownloadJob, DownloadStatus, UpdateAsset, download_paths_at, validate_asset, verify_file,
};

/// 后台写者串行:过期写者不能覆盖新代际的结果。
static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Record {
    asset: UpdateAsset,
    failure: Option<String>,
    #[serde(default)]
    installation: bool,
}

fn path(root: &Path) -> PathBuf {
    root.join("updates/download.json")
}

pub(super) fn save_at(
    root: &Path,
    asset: &UpdateAsset,
    status: &DownloadStatus,
) -> std::io::Result<()> {
    let failure = match status {
        DownloadStatus::Failed(error) | DownloadStatus::InstallFailed(error) => Some(error.clone()),
        DownloadStatus::Ready { .. } => None,
        // Absent/Downloading 不是终结证据,不落盘。
        _ => return Ok(()),
    };
    let record = Record {
        asset: asset.clone(),
        failure,
        installation: matches!(status, DownloadStatus::InstallFailed(_)),
    };
    let data = serde_json::to_vec(&record)?;
    crate::atomic_file::write(&path(root), &data)
}

/// 只序列化后台写者;已取消的完成写入不得越过重启存活。
pub(super) fn save_job(
    root: &Path,
    job: &DownloadJob,
    status: &DownloadStatus,
) -> std::io::Result<()> {
    let _writer = WRITER.lock().unwrap_or_else(|error| error.into_inner());
    if !job.is_current() {
        return Ok(());
    }
    save_at(root, &job.asset, status)?;
    if !job.is_current() {
        // WRITER 阻止更新的写者,直到过期记录被删除。
        std::fs::remove_file(path(root))?;
    }
    Ok(())
}

/// 安装结果比下载缓存新 → 失败证据优先(下载侧记录已过期)。
pub(super) fn supersedes(result: &Path) -> bool {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|file| file.modified())
            .ok()
    };
    matches!(
        (modified(result), modified(&path_for_compare(result))),
        (Some(result), Some(cache)) if cache > result
    )
}

/// `supersedes` 只需要与同目录的下载缓存比较;调用方保证 result 位于
/// `<root>/updates/handoffs/<transaction>/result.json`。
fn path_for_compare(result: &Path) -> PathBuf {
    result
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map_or_else(
            || result.to_owned(),
            |updates| updates.join("download.json"),
        )
}

/// 失败记录 mtime 晚于提示状态 mtime = 用户还没见过这次失败。
pub(super) fn failure_unseen_at(root: &Path, prompt_state: &Path) -> bool {
    let failure = std::fs::metadata(path(root))
        .and_then(|file| file.modified())
        .ok();
    let prompted = std::fs::metadata(prompt_state)
        .and_then(|file| file.modified())
        .ok();
    match (failure, prompted) {
        (Some(failure), Some(prompted)) => failure > prompted,
        (_, None) => true,
        _ => false,
    }
}

/// 启动恢复:缓存记录 + 文件重新校验双证据。
pub(super) fn load_at(root: &Path) -> Option<(UpdateAsset, DownloadStatus)> {
    let file = std::fs::File::open(path(root)).ok()?;
    if file.metadata().ok()?.len() > 64 * 1024 {
        return None;
    }
    let record: Record = serde_json::from_reader(file.take(64 * 1024)).ok()?;
    validate_asset(&record.asset).ok()?;
    let status = if let Some(error) = record.failure {
        if record.installation {
            DownloadStatus::InstallFailed(error)
        } else {
            DownloadStatus::Failed(error)
        }
    } else {
        let (_, path) = download_paths_at(root, &record.asset).ok()?;
        match verify_file(&path, &record.asset) {
            Ok(bytes) => DownloadStatus::Ready { path, bytes },
            Err(error) => DownloadStatus::Failed(error),
        }
    };
    Some((record.asset, status))
}

/// 会话内失败写回缓存(handoff 失败路径消费;安装弹窗归提示步)。
#[allow(dead_code)]
pub(super) fn record_installation_failure(
    root: &Path,
    asset: &UpdateAsset,
    error: String,
) -> std::io::Result<()> {
    save_at(root, asset, &DownloadStatus::InstallFailed(error))
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use sha2::{Digest as _, Sha256};

    fn asset(body: &[u8]) -> UpdateAsset {
        let name = "slTerminal-v1.6.0-windows-x64-setup.exe";
        UpdateAsset {
            version: "1.6.0".to_owned(),
            url: format!(
                "https://github.com/LiuShenLan/slTerminal/releases/download/v1.6.0/{name}"
            ),
            name: name.to_owned(),
            size: Some(body.len() as u64),
            digest: Some(
                Sha256::digest(body)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            ),
        }
    }

    #[test]
    fn ready_state_survives_a_restart_only_with_a_verified_file() {
        let root = tempfile::tempdir().unwrap();
        let body = b"MZcached installer";
        let asset = asset(body);
        let (_, final_path) = download_paths_at(root.path(), &asset).unwrap();
        std::fs::write(&final_path, body).unwrap();
        save_at(
            root.path(),
            &asset,
            &DownloadStatus::Ready {
                path: final_path.clone(),
                bytes: body.len() as u64,
            },
        )
        .unwrap();
        let (loaded, status) = load_at(root.path()).unwrap();
        assert_eq!(loaded, asset);
        assert!(matches!(status, DownloadStatus::Ready { .. }));

        std::fs::write(&final_path, b"MZtampered").unwrap();
        let (_, status) = load_at(root.path()).unwrap();
        assert!(matches!(status, DownloadStatus::Failed(_)));
    }

    #[test]
    fn installation_failures_persist_and_compare_against_prompt_state() {
        let root = tempfile::tempdir().unwrap();
        let asset = asset(b"MZinstaller");
        let prompt = root.path().join("update_state.json");
        std::fs::write(&prompt, b"{}").unwrap();
        record_installation_failure(root.path(), &asset, "setup failed".into()).unwrap();
        // 确保失败记录 mtime 严格晚于提示状态(文件系统时间精度兜底)。
        std::thread::sleep(std::time::Duration::from_millis(20));
        record_installation_failure(root.path(), &asset, "setup failed".into()).unwrap();
        assert!(failure_unseen_at(root.path(), &prompt));
        let (_, status) = load_at(root.path()).unwrap();
        assert!(matches!(status, DownloadStatus::InstallFailed(_)));
    }

    #[test]
    fn cancelled_writers_never_leave_a_stale_record() {
        let root = tempfile::tempdir().unwrap();
        let asset = asset(b"MZinstaller");
        let job = super::super::begin_download_session(&asset).unwrap();
        super::super::cancel(&asset);
        save_job(
            root.path(),
            &job,
            &DownloadStatus::Ready {
                path: root.path().join("x"),
                bytes: 1,
            },
        )
        .unwrap();
        assert!(!path(root.path()).exists());
    }
}
