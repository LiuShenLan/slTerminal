//! 历史隔离批次的撤销窗口状态机。
//!
//! 完成时间提交后才启动 5 秒绝对撤销期限；批次之间期限彼此独立。
//! 悬停、聚焦、关闭视图、退出或重启都不延长也不提前永久化。
//! 文件复制、来源删除与启动对账由后续接线消费本状态机。

use std::time::{Duration, SystemTime};

/// 绝对撤销窗口：从批次完成时间起算，不随页面活动顺延。
pub(crate) const UNDO_WINDOW: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct QuarantineBatchId(pub(crate) uuid::Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchStatus {
    /// 副本尚未完整校验并移除来源。
    Prepared,
    /// 完成时间已提交，进入撤销窗口。
    Completed,
    /// 已按当前批次整体撤销；材料由撤销流程保留或恢复。
    Undone,
}

/// 未完成批次的持久阶段；完成提交前的异常退出从这里继续。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IncompleteProgress {
    /// 尚未写入隔离副本。
    Prepared,
    /// 副本已校验、来源尚未删除。
    Copied,
    /// 来源已删除、完成时间尚未提交。
    SourceRemoved,
}

/// 对账时读取到的文件事实。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReconcileFacts {
    pub copy_intact: bool,
    pub source_present: bool,
}

/// 启动对账结论：未完成项不得伪装完成，已隐藏项进入待处理。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReconcileDecision {
    /// 未完成：保留（或恢复）来源，不属于撤销集合。
    RetainSource,
    /// 来源已删除但缺完成时间：继续隐藏并待人工确认。
    HiddenNeedsAttention,
    /// 材料或来源状态矛盾：保留全部可用材料并待处理。
    NeedsAttention,
}

/// 对账只看真实事实；任何缺副本或矛盾状态都进入待处理而不是自动提交。
pub(crate) fn reconcile_incomplete(
    progress: IncompleteProgress,
    facts: ReconcileFacts,
) -> ReconcileDecision {
    if !facts.copy_intact {
        return ReconcileDecision::NeedsAttention;
    }
    match progress {
        IncompleteProgress::Prepared | IncompleteProgress::Copied => {
            ReconcileDecision::RetainSource
        }
        IncompleteProgress::SourceRemoved => {
            if facts.source_present {
                ReconcileDecision::NeedsAttention
            } else {
                ReconcileDecision::HiddenNeedsAttention
            }
        }
    }
}

/// 单目标撤销预检：目标已存在（外部新增或修改）时不得覆盖。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UndoTargetDecision {
    Proceed,
    /// 目标位置已有外部对象：拒绝恢复并保留双份材料。
    ExternalTarget,
}

/// 目标存在即拒绝；由撤销流程用真实读取结果填入。
pub(crate) fn precheck_undo_target(target_exists: bool) -> UndoTargetDecision {
    if target_exists {
        UndoTargetDecision::ExternalTarget
    } else {
        UndoTargetDecision::Proceed
    }
}

/// 撤销点击的重新检查结论；只有 `Allowed` 才可执行恢复动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UndoDecision {
    Allowed,
    Unknown,
    NotCompleted,
    Expired,
    AlreadyUndone,
}

#[derive(Clone, Copy, Debug)]
struct QuarantineBatch {
    status: BatchStatus,
    completed_at: Option<SystemTime>,
}

/// 全部批次的撤销窗口事实；按批次身份索引，不持有文件材料。
pub(crate) struct HistoryQuarantine {
    batches: std::collections::BTreeMap<QuarantineBatchId, QuarantineBatch>,
}

impl Default for HistoryQuarantine {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryQuarantine {
    pub(crate) fn new() -> Self {
        Self {
            batches: std::collections::BTreeMap::new(),
        }
    }

    /// 登记新批次；同一身份重复登记不改变既有窗口事实。
    pub(crate) fn start_batch(&mut self, id: QuarantineBatchId) -> bool {
        if self.batches.contains_key(&id) {
            return false;
        }
        self.batches.insert(
            id,
            QuarantineBatch {
                status: BatchStatus::Prepared,
                completed_at: None,
            },
        );
        true
    }

    /// 完成时间提交才启动期限；未完成批次不得撤销也不得到期。
    pub(crate) fn complete_batch(&mut self, id: QuarantineBatchId, now: SystemTime) -> bool {
        let Some(batch) = self.batches.get_mut(&id) else {
            return false;
        };
        if batch.status != BatchStatus::Prepared {
            return false;
        }
        batch.status = BatchStatus::Completed;
        batch.completed_at = Some(now);
        true
    }

    /// 绝对期限判定：完成时间 + 窗口之前可撤销；时钟回拨视为仍在窗口内。
    pub(crate) fn undoable(&self, id: QuarantineBatchId, now: SystemTime) -> bool {
        let Some(batch) = self.batches.get(&id) else {
            return false;
        };
        let Some(completed_at) = batch.completed_at else {
            return false;
        };
        match now.duration_since(completed_at) {
            Ok(elapsed) => elapsed < UNDO_WINDOW,
            Err(_) => true,
        }
    }

    /// 持久事实：批次身份与已提交的完成时间；缺失完成时间的批次重启后进入待处理。
    pub(crate) fn persisted_windows(&self) -> Vec<(QuarantineBatchId, Option<SystemTime>)> {
        self.batches
            .iter()
            .map(|(id, batch)| (*id, batch.completed_at))
            .collect()
    }

    /// 重启恢复：完成时间缺失的批次进入 NeedsAttention，不伪装为可撤销或已到期。
    pub(crate) fn from_persisted(windows: &[(QuarantineBatchId, Option<SystemTime>)]) -> Self {
        let mut service = Self::new();
        for (id, completed_at) in windows {
            let status = match completed_at {
                Some(_) => BatchStatus::Completed,
                None => BatchStatus::Prepared,
            };
            service.batches.insert(
                *id,
                QuarantineBatch {
                    status,
                    completed_at: *completed_at,
                },
            );
        }
        service
    }

    /// 已到期且已完成、尚未收割的批次身份；调用方负责显式清理与永久化。
    pub(crate) fn expired(&self, now: SystemTime) -> Vec<QuarantineBatchId> {
        self.batches
            .iter()
            .filter(|(_, batch)| batch.status == BatchStatus::Completed)
            .filter(|(id, _)| !self.undoable(**id, now))
            .map(|(id, _)| *id)
            .collect()
    }

    /// 撤销点击：重新检查状态与绝对期限，全部通过才允许恢复并标记已撤销。
    pub(crate) fn try_begin_undo(
        &mut self,
        id: QuarantineBatchId,
        now: SystemTime,
    ) -> UndoDecision {
        let Some(status) = self.batches.get(&id).map(|batch| batch.status) else {
            return UndoDecision::Unknown;
        };
        match status {
            BatchStatus::Prepared => UndoDecision::NotCompleted,
            BatchStatus::Undone => UndoDecision::AlreadyUndone,
            BatchStatus::Completed => {
                if !self.undoable(id, now) {
                    return UndoDecision::Expired;
                }
                if let Some(batch) = self.batches.get_mut(&id) {
                    batch.status = BatchStatus::Undone;
                }
                UndoDecision::Allowed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn batch(seed: u8) -> QuarantineBatchId {
        let mut bytes = [0_u8; 16];
        bytes[0] = 0x01;
        bytes[1] = 0x9b;
        bytes[2] = 0x76;
        bytes[3] = 0xda;
        bytes[4] = 0xa8;
        bytes[5] = seed;
        bytes[6] = 0x70;
        bytes[7] = seed;
        bytes[8] = 0x80;
        QuarantineBatchId(uuid::Uuid::from_bytes(bytes))
    }

    /// 连续删除的批次各自从完成时间起算 5 秒，首批到期不提前永久化第二批。
    #[test]
    fn quarantine_batches_have_independent_deadlines() {
        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(batch(1)));
        assert!(service.start_batch(batch(2)));
        assert!(service.complete_batch(batch(1), at(100)));
        assert!(service.complete_batch(batch(2), at(103)));

        assert!(service.undoable(batch(1), at(104)));
        assert!(service.undoable(batch(2), at(104)));
        assert!(
            !service.undoable(batch(1), at(105)),
            "首批完成后满 5 秒即到期"
        );
        assert!(
            service.undoable(batch(2), at(105)),
            "第二批期限不得被首批提前"
        );
        assert!(!service.undoable(batch(2), at(108)));
        assert_eq!(service.expired(at(105)), vec![batch(1)]);
        assert_eq!(service.expired(at(110)), vec![batch(1), batch(2)]);
    }

    /// 完成提交前异常退出：先对账来源与副本，未完成项保留来源，缺副本进入待处理。
    #[test]
    fn quarantine_startup_reconciles_incomplete_batch() {
        use ReconcileDecision::{HiddenNeedsAttention, NeedsAttention, RetainSource};

        assert_eq!(
            reconcile_incomplete(
                IncompleteProgress::Copied,
                ReconcileFacts {
                    copy_intact: true,
                    source_present: true,
                },
            ),
            RetainSource,
            "副本已校验但未删除来源时必须保留来源"
        );
        assert_eq!(
            reconcile_incomplete(
                IncompleteProgress::Prepared,
                ReconcileFacts {
                    copy_intact: true,
                    source_present: true,
                },
            ),
            RetainSource
        );
        assert_eq!(
            reconcile_incomplete(
                IncompleteProgress::SourceRemoved,
                ReconcileFacts {
                    copy_intact: true,
                    source_present: false,
                },
            ),
            HiddenNeedsAttention,
            "来源已删但缺完成时间必须继续隐藏并待处理"
        );
        assert_eq!(
            reconcile_incomplete(
                IncompleteProgress::SourceRemoved,
                ReconcileFacts {
                    copy_intact: false,
                    source_present: false,
                },
            ),
            NeedsAttention,
            "副本缺失不得伪装完成"
        );
        assert_eq!(
            reconcile_incomplete(
                IncompleteProgress::SourceRemoved,
                ReconcileFacts {
                    copy_intact: true,
                    source_present: true,
                },
            ),
            NeedsAttention,
            "来源重新出现属于矛盾状态"
        );
    }

    /// 删除后重启仍保留未到期窗口：完成时间随记录恢复，判定基于原绝对期限。
    #[test]
    fn quarantine_restart_preserves_undo_window() {
        let mut first = HistoryQuarantine::new();
        assert!(first.start_batch(batch(8)));
        assert!(first.start_batch(batch(9)));
        assert!(first.start_batch(batch(10)));
        assert!(first.complete_batch(batch(8), at(100)));
        assert!(first.complete_batch(batch(9), at(1_000)));
        let windows = first.persisted_windows();

        let restored = HistoryQuarantine::from_persisted(&windows);
        assert!(
            restored.undoable(batch(8), at(104)),
            "重启后未到期批次必须仍可撤销"
        );
        assert!(
            !restored.undoable(batch(8), at(105)),
            "重启不得延长绝对期限"
        );
        assert!(restored.undoable(batch(9), at(1_004)));
        assert!(
            !restored.undoable(batch(10), at(100)),
            "缺完成时间的批次不得伪装为可撤销"
        );
        assert!(
            restored.expired(at(2_000)).contains(&batch(9)),
            "到期批次按原完成时间判到期"
        );
    }

    /// 目标外部新增或修改时不覆盖：拒绝恢复、保留材料且不消耗撤销窗口。
    #[test]
    fn quarantine_undo_rejects_external_target() {
        assert_eq!(precheck_undo_target(false), UndoTargetDecision::Proceed);
        assert_eq!(
            precheck_undo_target(true),
            UndoTargetDecision::ExternalTarget,
            "目标已存在必须拒绝覆盖"
        );

        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(batch(7)));
        assert!(service.complete_batch(batch(7), at(100)));
        assert_eq!(
            precheck_undo_target(true),
            UndoTargetDecision::ExternalTarget
        );
        assert!(
            service.undoable(batch(7), at(101)),
            "拒绝预检不得消耗撤销窗口"
        );
        assert_eq!(
            service.try_begin_undo(batch(7), at(102)),
            UndoDecision::Allowed,
            "外部目标消失后仍可在窗口内撤销"
        );
    }

    /// 撤销点击重新检查期限与状态：到期、未完成、已撤销分别拒绝且不改状态。
    #[test]
    fn quarantine_undo_rechecks_deadline() {
        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(batch(4)));
        assert_eq!(
            service.try_begin_undo(batch(4), at(100)),
            UndoDecision::NotCompleted,
            "未完成批次不得撤销"
        );
        assert!(service.complete_batch(batch(4), at(100)));
        assert_eq!(
            service.try_begin_undo(batch(4), at(104)),
            UndoDecision::Allowed
        );
        assert_eq!(
            service.try_begin_undo(batch(4), at(104)),
            UndoDecision::AlreadyUndone,
            "重复点击不得再次恢复"
        );
        assert!(
            !service.expired(at(200)).contains(&batch(4)),
            "已撤销批次不进入到期清理"
        );

        assert!(service.start_batch(batch(5)));
        assert!(service.complete_batch(batch(5), at(100)));
        assert_eq!(
            service.try_begin_undo(batch(5), at(105)),
            UndoDecision::Expired,
            "点击时超过绝对期限必须拒绝"
        );
        assert!(
            !service.undoable(batch(5), at(105)),
            "到期判定与撤销检查必须一致"
        );
        assert_eq!(
            service.try_begin_undo(batch(6), at(100)),
            UndoDecision::Unknown
        );
    }

    /// 未完成批次不可撤销也不得到期；重复完成不重启窗口。
    #[test]
    fn quarantine_window_starts_only_after_completion() {
        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(batch(3)));
        assert!(!service.undoable(batch(3), at(100)));
        assert!(service.expired(at(100)).is_empty());
        assert!(service.complete_batch(batch(3), at(100)));
        assert!(!service.complete_batch(batch(3), at(200)));
        assert!(
            service.undoable(batch(3), at(104)),
            "重复完成不得延长或重置窗口"
        );
        assert!(!service.start_batch(batch(3)), "同身份不得重复登记");
    }
}
