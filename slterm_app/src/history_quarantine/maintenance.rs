//! 隔离批次的启动对账、撤销、导出后永久删除与到期清理。
//!
//! 状态只消费持久记录与不可变清单；文件事实用于逐项预检与对账，不用于
//! 推断批次是否已完成。永久删除必须持有显式确认值，到期清理只处理完成
//! 或已撤销且已过绝对期限的批次。

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use sha2::{Digest as _, Sha256};

use super::export::validate_quarantine_copy;
use super::persist::{
    BatchStage, QuarantineBatchRecord, QuarantineItemRecord, QuarantineItemState,
    load_batch_material,
};
use super::{
    BatchStatus, HistoryQuarantine, QuarantineBatchId, QuarantineFailurePhase, UNDO_WINDOW,
    UndoAttempt, UndoBlockReason, UndoDecision, load_manifest,
};
use crate::audit::{QuarantineAuditAction, history_quarantine as audit_quarantine};

/// 启动维护报告：只含计数，不携带路径或正文。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuarantineMaintenanceReport {
    pub(crate) reconciled: usize,
    pub(crate) orphans_recovered: usize,
    pub(crate) cleaned: usize,
    pub(crate) failed_items: usize,
}

/// 显式永久删除确认；只能由 [`confirm_permanent_delete`] 生成。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PermanentDeleteConfirmation {
    batch_id: QuarantineBatchId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PermanentDeleteRejection {
    /// 完成批次仍在撤销窗口内。
    NotExpired,
    /// 未完成批次没有可永久化的完成事实。
    NotReady,
}

/// 单项目结果；失败项保留材料并给出封闭阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuarantineItemOutcome {
    Restored,
    Deleted,
    Retained,
    Failed(QuarantineFailurePhase),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineItemResult {
    pub(crate) order: usize,
    pub(crate) outcome: QuarantineItemOutcome,
}

/// 文件动作后的批次事实；`complete` 表示数据库行可以移除。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineActionResult {
    pub(crate) record: QuarantineBatchRecord,
    pub(crate) items: Vec<QuarantineItemResult>,
    pub(crate) applied: bool,
    pub(crate) complete: bool,
}

impl QuarantineActionResult {
    fn rejected(record: &QuarantineBatchRecord, outcomes: Vec<QuarantineItemResult>) -> Self {
        Self {
            record: record.clone(),
            items: outcomes,
            applied: false,
            complete: false,
        }
    }

    pub(crate) fn complete(&self) -> bool {
        self.complete
    }
}

fn file_sha256(path: &Path) -> io::Result<[u8; 32]> {
    let bytes = fs::read(path)?;
    Ok(Sha256::digest(&bytes).into())
}

/// 启动对账：逐项比较清单、副本与来源事实，任何矛盾都保留材料并待处理。
pub(crate) fn reconcile_record(
    root: &Path,
    record: &QuarantineBatchRecord,
    now: SystemTime,
) -> QuarantineBatchRecord {
    let _ = now;
    let mut reconciled = record.clone();
    let material = match load_batch_material(root, record) {
        Ok(material) => material,
        Err(_) => {
            reconciled.status = BatchStatus::NeedsAttention;
            reconciled.stage = BatchStage::Prepared;
            reconciled.completed_at = None;
            for item in &mut reconciled.items {
                item.state = QuarantineItemState::Failed;
                item.failure = Some(QuarantineFailurePhase::ReconcileMissingManifest);
            }
            return reconciled;
        }
    };

    let mut items = Vec::with_capacity(material.entries.len());
    let mut needs_attention = false;
    for entry in &material.entries {
        let copy_intact = validate_quarantine_copy(root, &entry.copy)
            .and_then(|()| file_sha256(&entry.copy))
            .is_ok_and(|fingerprint| fingerprint == entry.source_fingerprint);
        let source_present = fs::symlink_metadata(&entry.source).is_ok();
        let (state, failure) = match record.status {
            BatchStatus::Completed => {
                if !copy_intact {
                    needs_attention = true;
                    (
                        QuarantineItemState::Failed,
                        Some(QuarantineFailurePhase::ReconcileMissingCopy),
                    )
                } else if source_present {
                    // 来源重新出现说明该批并未完整隔离；恢复可见而不是继续隐藏。
                    needs_attention = true;
                    (QuarantineItemState::Staged, None)
                } else {
                    (QuarantineItemState::SourceRemoved, None)
                }
            }
            BatchStatus::Undone => (QuarantineItemState::Restored, None),
            BatchStatus::Prepared | BatchStatus::NeedsAttention => {
                needs_attention = true;
                if copy_intact {
                    if source_present {
                        (QuarantineItemState::Staged, None)
                    } else {
                        (QuarantineItemState::SourceRemoved, None)
                    }
                } else {
                    (
                        QuarantineItemState::Failed,
                        Some(QuarantineFailurePhase::ReconcileMissingCopy),
                    )
                }
            }
        };
        items.push(QuarantineItemRecord {
            order: entry.order,
            state,
            failure,
        });
    }
    reconciled.items = items;

    match record.status {
        BatchStatus::Completed if !needs_attention && record.completed_at.is_some() => {
            reconciled.status = BatchStatus::Completed;
            reconciled.stage = BatchStage::Committed;
        }
        BatchStatus::Undone => {
            reconciled.status = BatchStatus::Undone;
            reconciled.stage = BatchStage::Committed;
        }
        _ => {
            reconciled.status = BatchStatus::NeedsAttention;
            // 待处理批次保留已提交的完成时间，重启后仍可撤销重试；未完成
            // 批次没有完成事实，不得伪造撤销期限。
            reconciled.completed_at = match record.status {
                BatchStatus::NeedsAttention => record.completed_at,
                _ => None,
            };
            reconciled.stage = if reconciled
                .items
                .iter()
                .all(|item| item.state == QuarantineItemState::SourceRemoved)
            {
                BatchStage::SourceRemoved
            } else if reconciled
                .items
                .iter()
                .any(|item| item.state == QuarantineItemState::Staged)
            {
                BatchStage::Copied
            } else {
                BatchStage::Prepared
            };
        }
    }
    reconciled
}

/// 没有数据库行的清单：完整性未知，一律保留为待处理，绝不自动清理。
pub(crate) fn orphan_record(
    root: &Path,
    manifest_name: &str,
    now: SystemTime,
) -> QuarantineBatchRecord {
    let id = manifest_name
        .strip_suffix(".manifest.json")
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .map(QuarantineBatchId)
        .unwrap_or_else(|| QuarantineBatchId(uuid::Uuid::now_v7()));
    let mut record = QuarantineBatchRecord {
        id,
        status: BatchStatus::NeedsAttention,
        stage: BatchStage::Prepared,
        created_at: now,
        completed_at: None,
        manifest_name: manifest_name.to_owned(),
        items: Vec::new(),
    };
    let Ok(material) = load_manifest(&root.join(manifest_name)) else {
        return record;
    };
    record.items = material
        .entries
        .iter()
        .map(|entry| {
            let copy_intact = validate_quarantine_copy(root, &entry.copy)
                .and_then(|()| file_sha256(&entry.copy))
                .is_ok_and(|fingerprint| fingerprint == entry.source_fingerprint);
            let source_present = fs::symlink_metadata(&entry.source).is_ok();
            let (state, failure) = if copy_intact {
                if source_present {
                    (QuarantineItemState::Staged, None)
                } else {
                    (QuarantineItemState::SourceRemoved, None)
                }
            } else {
                (
                    QuarantineItemState::Failed,
                    Some(QuarantineFailurePhase::ReconcileMissingCopy),
                )
            };
            QuarantineItemRecord {
                order: entry.order,
                state,
                failure,
            }
        })
        .collect();
    record.stage = if record
        .items
        .iter()
        .all(|item| item.state == QuarantineItemState::SourceRemoved)
    {
        BatchStage::SourceRemoved
    } else if record
        .items
        .iter()
        .any(|item| item.state == QuarantineItemState::Staged)
    {
        BatchStage::Copied
    } else {
        BatchStage::Prepared
    };
    record
}

/// 撤销：状态推进与真实文件恢复在同一入口闭合，失败保留待处理事实。
pub(crate) fn undo_record(
    root: &Path,
    record: &QuarantineBatchRecord,
    now: SystemTime,
) -> QuarantineActionResult {
    let mut service = HistoryQuarantine::new();
    let prepared = service.start_batch(record.id);
    debug_assert!(prepared);
    let mut completed = false;
    if let Some(completed_at) = record.completed_at {
        completed = service.complete_batch(record.id, completed_at);
    }
    if record.status == BatchStatus::NeedsAttention && completed {
        service.fail_undo(record.id);
    }
    let material = match load_batch_material(root, record) {
        Ok(material) => material,
        Err(_) => {
            let mut failed = record.clone();
            failed.status = BatchStatus::NeedsAttention;
            for item in &mut failed.items {
                item.state = QuarantineItemState::Failed;
                item.failure = Some(QuarantineFailurePhase::ReconcileMissingManifest);
            }
            return QuarantineActionResult {
                items: record
                    .items
                    .iter()
                    .map(|item| QuarantineItemResult {
                        order: item.order,
                        outcome: QuarantineItemOutcome::Failed(
                            QuarantineFailurePhase::ReconcileMissingManifest,
                        ),
                    })
                    .collect(),
                record: failed,
                applied: false,
                complete: false,
            };
        }
    };
    let decision = service.check_undo(record.id, now);
    if decision != UndoDecision::Allowed {
        return QuarantineActionResult::rejected(
            record,
            record
                .items
                .iter()
                .map(|item| QuarantineItemResult {
                    order: item.order,
                    outcome: QuarantineItemOutcome::Retained,
                })
                .collect(),
        );
    }
    match service.undo_batch(&material, now) {
        UndoAttempt::Restored => {
            let mut restored = record.clone();
            restored.status = BatchStatus::Undone;
            restored.stage = BatchStage::Committed;
            for item in &mut restored.items {
                item.state = QuarantineItemState::Restored;
                item.failure = None;
            }
            let items = material
                .entries
                .iter()
                .map(|entry| {
                    audit_quarantine(QuarantineAuditAction::Undo, &entry.source, true);
                    QuarantineItemResult {
                        order: entry.order,
                        outcome: QuarantineItemOutcome::Restored,
                    }
                })
                .collect();
            QuarantineActionResult {
                record: restored,
                items,
                applied: true,
                complete: false,
            }
        }
        UndoAttempt::Blocked(blockages) => {
            let mut blocked = record.clone();
            blocked.status = BatchStatus::NeedsAttention;
            for blockage in &blockages {
                let phase = match blockage.reason {
                    UndoBlockReason::TargetExists => QuarantineFailurePhase::RestoreTargetExists,
                    UndoBlockReason::CopyMissing => QuarantineFailurePhase::RestoreCopyMissing,
                    UndoBlockReason::CopyCorrupted => QuarantineFailurePhase::RestoreCopyCorrupted,
                };
                if let Some(item) = blocked.item_mut(blockage.entry) {
                    item.state = QuarantineItemState::Failed;
                    item.failure = Some(phase);
                }
            }
            let items = material
                .entries
                .iter()
                .map(|entry| {
                    let phase = blockages
                        .iter()
                        .find(|blockage| blockage.entry == entry.order)
                        .map(|blockage| match blockage.reason {
                            UndoBlockReason::TargetExists => {
                                QuarantineFailurePhase::RestoreTargetExists
                            }
                            UndoBlockReason::CopyMissing => {
                                QuarantineFailurePhase::RestoreCopyMissing
                            }
                            UndoBlockReason::CopyCorrupted => {
                                QuarantineFailurePhase::RestoreCopyCorrupted
                            }
                        });
                    audit_quarantine(QuarantineAuditAction::Undo, &entry.source, phase.is_none());
                    QuarantineItemResult {
                        order: entry.order,
                        outcome: phase.map_or(
                            QuarantineItemOutcome::Retained,
                            QuarantineItemOutcome::Failed,
                        ),
                    }
                })
                .collect();
            QuarantineActionResult {
                record: blocked,
                items,
                applied: false,
                complete: false,
            }
        }
        UndoAttempt::Failed(_) => {
            let mut failed = record.clone();
            failed.status = BatchStatus::NeedsAttention;
            for item in &mut failed.items {
                item.state = QuarantineItemState::Failed;
                item.failure = Some(QuarantineFailurePhase::RestoreIo);
            }
            let items = material
                .entries
                .iter()
                .map(|entry| {
                    audit_quarantine(QuarantineAuditAction::Undo, &entry.source, false);
                    QuarantineItemResult {
                        order: entry.order,
                        outcome: QuarantineItemOutcome::Failed(QuarantineFailurePhase::RestoreIo),
                    }
                })
                .collect();
            QuarantineActionResult {
                record: failed,
                items,
                applied: false,
                complete: false,
            }
        }
        UndoAttempt::Rejected(_) => QuarantineActionResult::rejected(
            record,
            record
                .items
                .iter()
                .map(|item| QuarantineItemResult {
                    order: item.order,
                    outcome: QuarantineItemOutcome::Retained,
                })
                .collect(),
        ),
    }
}

/// 永久删除确认：完成批次必须已过绝对撤销期限，待处理批次允许显式删除。
pub(crate) fn confirm_permanent_delete(
    record: &QuarantineBatchRecord,
    now: SystemTime,
) -> Result<PermanentDeleteConfirmation, PermanentDeleteRejection> {
    match record.status {
        BatchStatus::NeedsAttention | BatchStatus::Undone => Ok(PermanentDeleteConfirmation {
            batch_id: record.id,
        }),
        BatchStatus::Completed => {
            let Some(completed_at) = record.completed_at else {
                return Err(PermanentDeleteRejection::NotReady);
            };
            match now.duration_since(completed_at) {
                Ok(elapsed) if elapsed >= UNDO_WINDOW => Ok(PermanentDeleteConfirmation {
                    batch_id: record.id,
                }),
                _ => Err(PermanentDeleteRejection::NotExpired),
            }
        }
        BatchStatus::Prepared => Err(PermanentDeleteRejection::NotReady),
    }
}

/// 到期清理资格：只含完成/已撤销且已过期限；待处理批次永不自动删除。
pub(crate) fn cleanup_eligible(record: &QuarantineBatchRecord, now: SystemTime) -> bool {
    if !matches!(record.status, BatchStatus::Completed | BatchStatus::Undone) {
        return false;
    }
    let Some(completed_at) = record.completed_at else {
        return false;
    };
    match now.duration_since(completed_at) {
        Ok(elapsed) => elapsed >= UNDO_WINDOW,
        Err(_) => false,
    }
}

/// 维护期到期清理：执行同一永久删除，但审计操作区分为清理。
pub(crate) fn cleanup_batch(
    root: &Path,
    record: &QuarantineBatchRecord,
    confirmation: PermanentDeleteConfirmation,
    now: SystemTime,
) -> QuarantineActionResult {
    let result = permanent_delete(root, record, confirmation, now);
    audit_quarantine(
        QuarantineAuditAction::Cleanup,
        &root.join(&record.manifest_name),
        result.complete,
    );
    result
}

/// 永久删除：逐项删除副本，全部成功后才移除清单；失败保留重试事实。
pub(crate) fn permanent_delete(
    root: &Path,
    record: &QuarantineBatchRecord,
    confirmation: PermanentDeleteConfirmation,
    now: SystemTime,
) -> QuarantineActionResult {
    let _ = now;
    if confirmation.batch_id != record.id {
        return QuarantineActionResult::rejected(record, Vec::new());
    }
    let material = match load_batch_material(root, record) {
        Ok(material) => material,
        Err(_) => {
            let mut failed = record.clone();
            failed.status = BatchStatus::NeedsAttention;
            let manifest_path = root.join(&record.manifest_name);
            audit_quarantine(
                QuarantineAuditAction::PermanentDelete,
                &manifest_path,
                false,
            );
            return QuarantineActionResult {
                items: record
                    .items
                    .iter()
                    .map(|item| QuarantineItemResult {
                        order: item.order,
                        outcome: QuarantineItemOutcome::Failed(
                            QuarantineFailurePhase::ReconcileMissingManifest,
                        ),
                    })
                    .collect(),
                record: failed,
                applied: false,
                complete: false,
            };
        }
    };
    let mut updated = record.clone();
    let mut results = Vec::with_capacity(material.entries.len());
    let mut failed = false;
    for entry in &material.entries {
        if record
            .item(entry.order)
            .is_some_and(|item| item.state == QuarantineItemState::Deleted)
        {
            results.push(QuarantineItemResult {
                order: entry.order,
                outcome: QuarantineItemOutcome::Deleted,
            });
            continue;
        }
        let outcome = match validate_quarantine_copy(root, &entry.copy) {
            Ok(()) => match fs::remove_file(&entry.copy) {
                Ok(()) => QuarantineItemOutcome::Deleted,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    QuarantineItemOutcome::Deleted
                }
                Err(_) => QuarantineItemOutcome::Failed(QuarantineFailurePhase::DeleteCopy),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // 已缺失的副本按幂等删除处理；清单与记录仍由后续步骤收尾。
                QuarantineItemOutcome::Deleted
            }
            Err(_) => QuarantineItemOutcome::Failed(QuarantineFailurePhase::DeleteCopy),
        };
        if matches!(outcome, QuarantineItemOutcome::Deleted) {
            if let Some(item) = updated.item_mut(entry.order) {
                item.state = QuarantineItemState::Deleted;
                item.failure = None;
            }
        } else {
            failed = true;
            if let Some(item) = updated.item_mut(entry.order) {
                item.state = QuarantineItemState::Failed;
                item.failure = Some(QuarantineFailurePhase::DeleteCopy);
            }
        }
        audit_quarantine(
            QuarantineAuditAction::PermanentDelete,
            &entry.copy,
            matches!(outcome, QuarantineItemOutcome::Deleted),
        );
        results.push(QuarantineItemResult {
            order: entry.order,
            outcome,
        });
    }
    if failed {
        updated.status = BatchStatus::NeedsAttention;
        return QuarantineActionResult {
            record: updated,
            items: results,
            applied: false,
            complete: false,
        };
    }
    let manifest_path = root.join(&record.manifest_name);
    match fs::remove_file(&manifest_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => {
            updated.status = BatchStatus::NeedsAttention;
            audit_quarantine(
                QuarantineAuditAction::PermanentDelete,
                &manifest_path,
                false,
            );
            return QuarantineActionResult {
                record: updated,
                items: results,
                applied: false,
                complete: false,
            };
        }
    }
    audit_quarantine(QuarantineAuditAction::PermanentDelete, &manifest_path, true);
    for item in &mut updated.items {
        item.state = QuarantineItemState::Deleted;
        item.failure = None;
    }
    updated.status = BatchStatus::Undone;
    updated.stage = BatchStage::Committed;
    QuarantineActionResult {
        record: updated,
        items: results,
        applied: true,
        complete: true,
    }
}

impl QuarantineBatchRecord {
    fn item_mut(&mut self, order: usize) -> Option<&mut QuarantineItemRecord> {
        self.items.iter_mut().find(|item| item.order == order)
    }
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;
    use crate::history_quarantine::maintenance::{
        PermanentDeleteRejection, confirm_permanent_delete,
    };
    use crate::history_quarantine::persist::QuarantineBatchRecord;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn record(status: BatchStatus, completed_at: Option<SystemTime>) -> QuarantineBatchRecord {
        QuarantineBatchRecord {
            id: QuarantineBatchId(Uuid::nil_probe()),
            status,
            stage: BatchStage::Committed,
            created_at: at(1),
            completed_at,
            manifest_name: "019bfd70-80aa-7000-8000-000000000000.manifest.json".into(),
            items: Vec::new(),
        }
    }

    /// 仅本地占位：UUID 工厂测试不需要真实 v7 顺序。
    struct Uuid;
    impl Uuid {
        fn nil_probe() -> uuid::Uuid {
            match uuid::Uuid::parse_str("019bfd70-80aa-7000-8000-000000000000") {
                Ok(id) => id,
                Err(_) => panic!("固定 UUID 必须可解析"),
            }
        }
    }

    #[test]
    fn permanent_delete_confirmation_distinguishes_window_and_pending() {
        let completed = record(BatchStatus::Completed, Some(at(100)));
        assert_eq!(
            confirm_permanent_delete(&completed, at(104)),
            Err(PermanentDeleteRejection::NotExpired)
        );
        assert!(confirm_permanent_delete(&completed, at(105)).is_ok());
        let pending = record(BatchStatus::NeedsAttention, None);
        assert!(confirm_permanent_delete(&pending, at(100)).is_ok());
        let prepared = record(BatchStatus::Prepared, None);
        assert_eq!(
            confirm_permanent_delete(&prepared, at(105)),
            Err(PermanentDeleteRejection::NotReady)
        );
    }

    #[test]
    fn cleanup_eligible_excludes_pending_and_unexpired() {
        assert!(!cleanup_eligible(
            &record(BatchStatus::NeedsAttention, Some(at(100))),
            at(200)
        ));
        assert!(!cleanup_eligible(
            &record(BatchStatus::Completed, Some(at(100))),
            at(104)
        ));
        assert!(cleanup_eligible(
            &record(BatchStatus::Completed, Some(at(100))),
            at(105)
        ));
        assert!(cleanup_eligible(
            &record(BatchStatus::Undone, Some(at(100))),
            at(105)
        ));
    }
}
