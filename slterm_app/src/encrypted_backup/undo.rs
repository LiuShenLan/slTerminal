//! 备份撤回：按恢复后指纹逐项回到恢复前状态，冲突项保留当前内容。
//!
//! 撤回只经维护请求在启动维护期执行；恢复点材料不重输口令。
//! 冲突或失败项保留恢复点，供再次撤回或诊断；完整成功才清理材料。

use std::fs;
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};
use slterm_path::AppPaths;
use slterm_workspace::ViewStateRegistry;

use super::maintenance::capture_workspace_snapshot;
use super::recovery::{OriginalFile, decode_point, write_originals};
use super::workspace::{decode_workspace, restore_workspace};
use super::{BackupCategory, category_root, restore_path};
use crate::persistence::maintenance_request::{
    MaintenanceRequest, MaintenanceRequestStatus, RecoveryPointStatus,
};
use crate::persistence::startup::{MaintenanceRequestError, MaintenanceRequestRecoverer};
use crate::persistence::workspace::WorkspaceDatabase;

/// 备份撤回维护请求的 kind；只有本模块恢复器解释恢复点材料。
pub(crate) const BACKUP_UNDO_KIND: &str = "backup.undo";

/// 撤回恢复器：处理运行期登记的 `backup.undo` 请求。
pub(crate) struct BackupUndoRecoverer {
    paths: AppPaths,
    registry: Arc<ViewStateRegistry>,
}

impl BackupUndoRecoverer {
    pub(crate) fn new(paths: AppPaths, registry: Arc<ViewStateRegistry>) -> Self {
        Self { paths, registry }
    }

    fn point_id(request: &MaintenanceRequest) -> Option<String> {
        let name = Path::new(&request.staged_path).file_name()?.to_str()?;
        name.strip_suffix(".slterm-recovery").map(str::to_owned)
    }

    fn current_workspace_fingerprint(&self, store: &mut WorkspaceDatabase) -> Option<[u8; 32]> {
        capture_workspace_snapshot(store, &crate::platform::machine::machine_key())
            .ok()
            .and_then(|payload| {
                serde_json::to_vec(&payload)
                    .ok()
                    .map(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)))
            })
    }

    fn file_matches(original: &OriginalFile, paths: &AppPaths, after: [u8; 32]) -> bool {
        let expected = after;
        let Ok(path) = restore_path(
            category_root(paths, original.target.category),
            &original.target.name,
        ) else {
            return false;
        };
        match fs::read(path) {
            Ok(bytes) => <[u8; 32]>::from(Sha256::digest(&bytes)) == expected,
            Err(_) => false,
        }
    }

    fn workspace_matches(&self, store: &mut WorkspaceDatabase, after: [u8; 32]) -> bool {
        self.current_workspace_fingerprint(store) == Some(after)
    }

    fn restore_one(
        &self,
        store: &mut WorkspaceDatabase,
        original: &OriginalFile,
    ) -> Result<(), String> {
        if original.target.category == BackupCategory::Workspace {
            let bytes = original
                .bytes
                .as_deref()
                .ok_or_else(|| "工作区恢复点缺少原值".to_owned())?;
            let payload = decode_workspace(bytes)?;
            return restore_workspace(
                store,
                &payload,
                &crate::platform::machine::machine_key(),
                &self.registry,
            );
        }
        write_originals(&self.paths, std::slice::from_ref(original))
    }

    fn undo(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), String> {
        if matches!(
            request.status,
            MaintenanceRequestStatus::Applied | MaintenanceRequestStatus::RolledBack
        ) {
            return Ok(());
        }
        let Some(point_id) = Self::point_id(request) else {
            return self.block(store, request, "撤回材料路径无效");
        };
        let rows = store
            .recovery_points(&point_id)
            .map_err(|error| format!("读取恢复点登记失败: {error}"))?;
        if rows.is_empty() {
            return self.block(store, request, "撤回目标恢复点不存在");
        }
        if rows
            .iter()
            .any(|row| row.status != RecoveryPointStatus::Applied)
        {
            return self.block(store, request, "恢复点尚未完成应用");
        }
        let point_path = restore_path(self.paths.local_root(), &request.staged_path)?;
        let packet = match fs::read(&point_path) {
            Ok(packet) => packet,
            Err(_) => return self.block(store, request, "恢复点材料缺失或不可读"),
        };
        let originals = match decode_point(&packet) {
            Ok(originals) => originals,
            Err(_) => return self.block(store, request, "恢复点材料无法解密"),
        };
        let expected: std::collections::BTreeSet<String> =
            rows.iter().map(|row| row.object_key.clone()).collect();
        let actual: std::collections::BTreeSet<String> = originals
            .iter()
            .map(|original| super::maintenance::object_key(&original.target))
            .collect();
        if expected != actual {
            return self.block(store, request, "恢复点集合与登记事实不一致");
        }
        if request.status == MaintenanceRequestStatus::Prepared {
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Prepared,
                    MaintenanceRequestStatus::Applying,
                )
                .map_err(|error| format!("推进撤回请求失败: {error}"))?;
        }
        let after_by_key: std::collections::BTreeMap<String, Option<[u8; 32]>> = rows
            .iter()
            .map(|row| (row.object_key.clone(), row.after_fingerprint))
            .collect();
        let mut conflicts = 0usize;
        let mut failures = 0usize;
        for original in &originals {
            let after = after_by_key
                .get(&super::maintenance::object_key(&original.target))
                .copied()
                .flatten();
            let matched = match after {
                Some(after) if original.target.category == BackupCategory::Workspace => {
                    self.workspace_matches(store, after)
                }
                Some(after) => Self::file_matches(original, &self.paths, after),
                None => false,
            };
            if !matched {
                conflicts += 1;
                audit_undo(crate::audit::AuditResult::Suspended);
                continue;
            }
            match self.restore_one(store, original) {
                Ok(()) => audit_undo(crate::audit::AuditResult::Ok),
                Err(error) => {
                    log::warn!("撤回目标失败: {error}");
                    failures += 1;
                    audit_undo(crate::audit::AuditResult::Error);
                }
            }
        }
        store
            .transition_maintenance_request(
                &request.request_id,
                MaintenanceRequestStatus::Applying,
                MaintenanceRequestStatus::RolledBack,
            )
            .map_err(|error| format!("标记撤回终态失败: {error}"))?;
        if conflicts == 0 && failures == 0 {
            if let Err(error) = store.clear_recovery_points(&point_id) {
                return self.block(store, request, &format!("清理恢复点登记失败: {error}"));
            }
            match fs::remove_file(&point_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    log::warn!("恢复点密文删除失败: {error}");
                }
            }
        } else {
            log::info!("撤回保留材料: conflicts={conflicts} failures={failures} point={point_id}");
        }
        Ok(())
    }

    fn block(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
        detail: &str,
    ) -> Result<(), String> {
        let _ = store.transition_maintenance_request(
            &request.request_id,
            request.status,
            MaintenanceRequestStatus::NeedsAttention,
        );
        Err(detail.to_owned())
    }
}

fn audit_undo(result: crate::audit::AuditResult) {
    crate::audit::record(crate::audit::AuditEvent::new(
        "backup_restore_undo",
        "encrypted_backup::undo",
        result,
    ));
}

impl MaintenanceRequestRecoverer for BackupUndoRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), MaintenanceRequestError> {
        self.undo(store, request)
            .map_err(MaintenanceRequestError::new)
    }
}
