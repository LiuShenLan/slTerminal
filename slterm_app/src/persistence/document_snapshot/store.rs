//! 快照文件身份、总配额、驱逐与删除提交边界。

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use slterm_document::{DocumentId, DocumentSession};
use slterm_path::AppPaths;
use slterm_workspace::CloseTarget;

use super::envelope::{DocumentSnapshot, SNAPSHOT_BODY_LIMIT_BYTES, SnapshotStoreError};
use crate::persistence::close::{CloseCommitStatus, PersistedCloseIntent};

pub(crate) const SNAPSHOT_TOTAL_QUOTA_BYTES: u64 = 256 * 1024 * 1024;
/// 无主快照保留期；超过且不被布局引用才进入清理候选。
pub(crate) const SNAPSHOT_STALE_AGE: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 60 * 60);

struct StoredSnapshot {
    snapshot: DocumentSnapshot,
    path: PathBuf,
    modified: SystemTime,
}

pub(crate) struct SnapshotRecoveryScan {
    pub(crate) snapshots: Vec<DocumentSnapshot>,
    pub(crate) corrupt: Vec<PathBuf>,
}

/// 快照目录的唯一文件所有者；所有调用只能位于专用后台写线程。
pub(crate) struct DocumentSnapshotStore {
    root: PathBuf,
    pub(super) total_quota: u64,
}

impl DocumentSnapshotStore {
    pub(crate) fn new(paths: &AppPaths) -> Self {
        Self {
            root: paths.local_root().join("snapshots"),
            total_quota: SNAPSHOT_TOTAL_QUOTA_BYTES,
        }
    }

    fn path(&self, id: DocumentId) -> PathBuf {
        self.root.join(format!("{id}.snap"))
    }

    /// 陈旧快照候选：超过保留期且不被布局引用的普通快照文件。
    /// 文件内容不解读，损坏文件留给启动隔离。
    pub(crate) fn stale_candidates(
        &self,
        referenced: &BTreeSet<DocumentId>,
        now: SystemTime,
    ) -> Vec<(PathBuf, u64)> {
        stale_candidates_in(&self.root, referenced, now)
    }

    pub(crate) fn read(
        &self,
        id: DocumentId,
    ) -> Result<Option<DocumentSnapshot>, SnapshotStoreError> {
        let path = self.path(id);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let snapshot = DocumentSnapshot::decode(&bytes)?;
        if snapshot.document_id() != id {
            return Err(SnapshotStoreError::Corrupt);
        }
        Ok(Some(snapshot))
    }

    /// 配额只驱逐非打开脏 Document 的旧快照；不足时不改变任何既有文件。
    pub(crate) fn write(
        &mut self,
        candidate: &DocumentSnapshot,
        protected: &BTreeSet<DocumentId>,
    ) -> Result<(), SnapshotStoreError> {
        // 无关损坏文件留给启动隔离；运行期写入不因旁路文件失败。
        let (mut stored, _corrupt) = self.scan_entries(true)?;
        let body_len = candidate.body_len() as u64;
        if body_len > SNAPSHOT_BODY_LIMIT_BYTES {
            return Err(SnapshotStoreError::QuotaExceeded);
        }
        let old_candidate = stored
            .iter()
            .position(|entry| entry.snapshot.document_id() == candidate.document_id())
            .map(|index| stored.remove(index));
        let mut used = stored
            .iter()
            .try_fold(0_u64, |total, entry| {
                total.checked_add(entry.snapshot.body_len() as u64)
            })
            .ok_or(SnapshotStoreError::QuotaExceeded)?;
        let required = used
            .checked_add(body_len)
            .ok_or(SnapshotStoreError::QuotaExceeded)?;
        let mut evictions = Vec::new();
        if required > self.total_quota {
            stored.sort_by_key(|entry| (entry.modified, entry.snapshot.document_id()));
            for entry in stored {
                if protected.contains(&entry.snapshot.document_id()) {
                    continue;
                }
                used = used.saturating_sub(entry.snapshot.body_len() as u64);
                evictions.push(entry);
                if used
                    .checked_add(body_len)
                    .is_some_and(|total| total <= self.total_quota)
                {
                    break;
                }
            }
        }
        if used
            .checked_add(body_len)
            .is_none_or(|total| total > self.total_quota)
        {
            return Err(SnapshotStoreError::QuotaExceeded);
        }

        let path = self.path(candidate.document_id());
        // 头身份优先：同一 Document 的旧文件即使文件名不符也按身份覆盖，
        // 写成功后清除旧位置的残留文件。
        let stale_path = old_candidate
            .as_ref()
            .filter(|old| old.path != path)
            .map(|old| old.path.clone());
        crate::atomic_file::write_current_user(&path, &candidate.encode()?)?;
        for eviction in evictions {
            remove_file(&eviction.path)?;
        }
        if let Some(stale) = stale_path {
            remove_file(&stale)?;
        }
        Ok(())
    }

    /// 保存完成或撤销回 saved_state 后删除冗余快照；脏正文绝不在此删除。
    pub(crate) fn reconcile(
        &mut self,
        session: &DocumentSession,
    ) -> Result<(), SnapshotStoreError> {
        if session.is_dirty() {
            return Ok(());
        }
        self.delete_validated(session.id())
    }

    /// 最后引用关闭必须携带包含该 Document 且已越过状态提交边界的真实意图。
    pub(crate) fn delete_closed(
        &mut self,
        id: DocumentId,
        close: &PersistedCloseIntent,
    ) -> Result<(), SnapshotStoreError> {
        if close.commit_status == CloseCommitStatus::Collected
            || !close
                .intent
                .objects()
                .any(|object| object.id == CloseTarget::Document(id))
        {
            return Err(SnapshotStoreError::CloseNotCommitted);
        }
        self.delete_validated(id)
    }

    /// 损坏文件留给启动隔离，任何清理路径都不得直接物理删除。
    pub(super) fn delete_validated(&mut self, id: DocumentId) -> Result<(), SnapshotStoreError> {
        if self.read(id)?.is_none() {
            return Ok(());
        }
        remove_file(&self.path(id))
    }

    pub(crate) fn scan_for_recovery(&self) -> Result<SnapshotRecoveryScan, SnapshotStoreError> {
        let (entries, corrupt) = self.scan_entries(true)?;
        Ok(SnapshotRecoveryScan {
            snapshots: entries.into_iter().map(|entry| entry.snapshot).collect(),
            corrupt,
        })
    }

    /// 只把本目录内的损坏快照改名到独立隔离目录，不读取或删除正文。
    pub(crate) fn quarantine(&self, path: &Path) -> io::Result<PathBuf> {
        if path.parent() != Some(self.root.as_path())
            || path.extension().and_then(|value| value.to_str()) != Some("snap")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "隔离来源不是本快照目录文件",
            ));
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "快照文件名无效"))?;
        let directory = self
            .root
            .parent()
            .ok_or_else(|| io::Error::other("快照目录缺少本地根"))?
            .join("quarantine/snapshots");
        std::fs::create_dir_all(&directory)?;
        let target = directory.join(format!("{name}.{}.quarantine", uuid::Uuid::now_v7()));
        std::fs::rename(path, &target)?;
        Ok(target)
    }

    fn scan_entries(
        &self,
        allow_corrupt: bool,
    ) -> Result<(Vec<StoredSnapshot>, Vec<PathBuf>), SnapshotStoreError> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((Vec::new(), Vec::new()));
            }
            Err(error) => return Err(error.into()),
        };
        let mut snapshots = Vec::new();
        let mut corrupt = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("snap") {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(SnapshotStoreError::Corrupt);
            }
            // 文件身份以信封头为准：文件名不一致不构成损坏。
            let decoded = DocumentSnapshot::decode(&std::fs::read(&path)?).ok();
            if let Some(snapshot) = decoded {
                snapshots.push(StoredSnapshot {
                    snapshot,
                    modified: entry.metadata()?.modified()?,
                    path,
                });
            } else if allow_corrupt {
                corrupt.push(path);
            } else {
                return Err(SnapshotStoreError::Corrupt);
            }
        }
        Ok((snapshots, corrupt))
    }
}

fn remove_file(path: &Path) -> Result<(), SnapshotStoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn stale_candidates_in(
    root: &Path,
    referenced: &BTreeSet<DocumentId>,
    now: SystemTime,
) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".snap") else {
            continue;
        };
        let Ok(id) = stem.parse::<DocumentId>() else {
            continue;
        };
        if referenced.contains(&id) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= SNAPSHOT_STALE_AGE);
        if !stale {
            continue;
        }
        candidates.push((path, metadata.len()));
    }
    candidates.sort();
    candidates
}
