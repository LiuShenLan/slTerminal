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

    /// 已到期且已完成、尚未收割的批次身份；调用方负责显式清理与永久化。
    pub(crate) fn expired(&self, now: SystemTime) -> Vec<QuarantineBatchId> {
        self.batches
            .iter()
            .filter(|(id, _)| !self.undoable(**id, now))
            .map(|(id, _)| *id)
            .collect()
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
