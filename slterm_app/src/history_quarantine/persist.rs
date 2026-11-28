//! 隔离批次的持久记录：批次状态、逐项状态与清单读取。
//!
//! 不可变清单仍是条目路径、顺序与指纹的唯一权威；本模块的记录只持久化
//! 状态机事实与逐项进展，不从来源存在性推断完成状态。

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{
    BatchStatus, QuarantineBatchId, QuarantineBatchMaterial, QuarantineBatchOutcome,
    QuarantineFailurePhase, QuarantineItemFailure, load_manifest,
};

/// 批次进度：完成提交前用于对账，完成后固定为 `Committed`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchStage {
    Prepared,
    Copied,
    SourceRemoved,
    Committed,
}

impl BatchStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Copied => "copied",
            Self::SourceRemoved => "source_removed",
            Self::Committed => "committed",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "prepared" => Self::Prepared,
            "copied" => Self::Copied,
            "source_removed" => Self::SourceRemoved,
            "committed" => Self::Committed,
            _ => return None,
        })
    }
}

impl BatchStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Completed => "completed",
            Self::NeedsAttention => "needs_attention",
            Self::Undone => "undone",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "prepared" => Self::Prepared,
            "completed" => Self::Completed,
            "needs_attention" => Self::NeedsAttention,
            "undone" => Self::Undone,
            _ => return None,
        })
    }
}

/// 逐项状态：来源是否仍隐藏、是否已恢复或已永久删除，只由这里表达。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuarantineItemState {
    /// 副本存在、来源仍可见（未完成隔离）。
    Staged,
    /// 来源已删除并继续隐藏。
    SourceRemoved,
    /// 撤销已恢复来源。
    Restored,
    /// 永久删除已移除副本。
    Deleted,
    /// 最近一次操作失败，材料保留。
    Failed,
}

impl QuarantineItemState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::SourceRemoved => "source_removed",
            Self::Restored => "restored",
            Self::Deleted => "deleted",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "staged" => Self::Staged,
            "source_removed" => Self::SourceRemoved,
            "restored" => Self::Restored,
            "deleted" => Self::Deleted,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineItemRecord {
    pub(crate) order: usize,
    pub(crate) state: QuarantineItemState,
    pub(crate) failure: Option<QuarantineFailurePhase>,
}

/// 批次状态的持久事实；`manifest_name` 只是隔离根内的清单文件名。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineBatchRecord {
    pub(crate) id: QuarantineBatchId,
    pub(crate) status: BatchStatus,
    pub(crate) stage: BatchStage,
    pub(crate) created_at: SystemTime,
    pub(crate) completed_at: Option<SystemTime>,
    pub(crate) manifest_name: String,
    pub(crate) items: Vec<QuarantineItemRecord>,
}

impl QuarantineBatchRecord {
    /// 批次文件操作结束后形成的初始记录；完成时间只由完成态携带。
    pub(crate) fn from_outcome(outcome: &QuarantineBatchOutcome, now: SystemTime) -> Self {
        let failures = outcome
            .failures
            .iter()
            .map(|failure: &QuarantineItemFailure| (failure.entry_index, failure.phase))
            .collect::<std::collections::BTreeMap<_, _>>();
        let removed = outcome
            .material
            .entries
            .iter()
            .map(|entry| entry.order)
            .collect::<BTreeSet<_>>();
        let staged = outcome
            .staged
            .iter()
            .map(|entry| entry.order)
            .collect::<BTreeSet<_>>();
        let items: Vec<QuarantineItemRecord> = outcome
            .planned
            .iter()
            .map(|entry| {
                let (state, failure) = if let Some(phase) = failures.get(&entry.order) {
                    (QuarantineItemState::Failed, Some(*phase))
                } else if removed.contains(&entry.order) {
                    (QuarantineItemState::SourceRemoved, None)
                } else if staged.contains(&entry.order) {
                    (QuarantineItemState::Staged, None)
                } else {
                    (
                        QuarantineItemState::Failed,
                        Some(QuarantineFailurePhase::Read),
                    )
                };
                QuarantineItemRecord {
                    order: entry.order,
                    state,
                    failure,
                }
            })
            .collect();
        let stage = match outcome.status {
            BatchStatus::Completed => BatchStage::Committed,
            _ if items.iter().all(|item| {
                item.state == QuarantineItemState::SourceRemoved
                    || item.failure == Some(QuarantineFailurePhase::SourceRemove)
            }) =>
            {
                BatchStage::SourceRemoved
            }
            _ if !staged.is_empty() => BatchStage::Copied,
            _ => BatchStage::Prepared,
        };
        let manifest_name = outcome
            .manifest_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("隔离清单名必须为有效文件名")
            .to_owned();
        Self {
            id: outcome.material.id,
            status: outcome.status,
            stage,
            created_at: now,
            completed_at: (outcome.status == BatchStatus::Completed).then_some(now),
            manifest_name,
            items,
        }
    }

    pub(crate) fn manifest_path(&self, root: &Path) -> PathBuf {
        root.join(&self.manifest_name)
    }

    pub(crate) fn item(&self, order: usize) -> Option<&QuarantineItemRecord> {
        self.items.iter().find(|item| item.order == order)
    }
}

/// 读取批次清单并与记录逐项对齐；清单条目是路径与顺序的唯一来源。
pub(crate) fn load_batch_material(
    root: &Path,
    record: &QuarantineBatchRecord,
) -> io::Result<QuarantineBatchMaterial> {
    validate_manifest_name(&record.manifest_name)?;
    let loaded = load_manifest(&root.join(&record.manifest_name))?;
    if loaded.batch_id != record.id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "隔离清单批次身份不一致",
        ));
    }
    if loaded.entries.len() != record.items.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "隔离清单条目数与记录不一致",
        ));
    }
    let mut recorded = record
        .items
        .iter()
        .map(|item| item.order)
        .collect::<Vec<_>>();
    recorded.sort_unstable();
    let manifest_orders = loaded
        .entries
        .iter()
        .map(|entry| entry.order)
        .collect::<Vec<_>>();
    if recorded != manifest_orders {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "隔离清单顺序与记录不一致",
        ));
    }
    Ok(QuarantineBatchMaterial {
        id: record.id,
        entries: loaded.entries,
    })
}

/// 清单文件名必须是隔离根内的单个普通文件名。
pub(crate) fn validate_manifest_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || !name.ends_with(".manifest.json")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离清单名无效",
        ));
    }
    Ok(())
}

/// 隔离根内全部清单文件名；启动对账据此发现没有数据库行的孤儿批次。
pub(crate) fn scan_manifest_names(root: &Path) -> io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let stable_id = name
            .strip_suffix(".manifest.json")
            .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok());
        if stable_id && validate_manifest_name(&name).is_ok() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// 导航与隔离区共享的恢复快照：记录是状态权威，隐藏来源是逐项推导。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuarantineSnapshot {
    pub(crate) records: Vec<QuarantineBatchRecord>,
    pub(crate) hidden_sources: BTreeSet<PathBuf>,
}

impl QuarantineSnapshot {
    pub(crate) fn from_records(root: &Path, records: Vec<QuarantineBatchRecord>) -> Self {
        let hidden_sources = hidden_sources(root, &records);
        Self {
            records,
            hidden_sources,
        }
    }
}

/// 当前隐藏来源集合：只由记录中的逐项状态决定，不扫描来源目录。
pub(crate) fn hidden_sources(root: &Path, records: &[QuarantineBatchRecord]) -> BTreeSet<PathBuf> {
    let mut hidden = BTreeSet::new();
    for record in records {
        let Ok(material) = load_batch_material(root, record) else {
            continue;
        };
        for entry in material.entries {
            if record
                .item(entry.order)
                .is_some_and(|item| item.state == QuarantineItemState::SourceRemoved)
            {
                hidden.insert(entry.source);
            }
        }
    }
    hidden
}

pub(crate) fn system_time_to_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

pub(crate) fn system_time_from_ms(milliseconds: i64) -> Option<SystemTime> {
    if milliseconds < 0 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_millis(milliseconds as u64))
}

#[cfg(test)]
mod persist_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn manifest_name_rejects_traversal_and_foreign_files() {
        for name in [
            "",
            ".",
            "..",
            "../x.manifest.json",
            "sub/x.manifest.json",
            "x.json",
        ] {
            assert!(validate_manifest_name(name).is_err(), "{name} 必须拒绝");
        }
        assert!(
            validate_manifest_name("019bfd70-80aa-7000-8000-000000000000.manifest.json").is_ok()
        );
    }

    #[test]
    fn timestamps_round_trip_without_negative_epoch() {
        let time = SystemTime::UNIX_EPOCH + Duration::from_millis(1_234);
        assert_eq!(system_time_from_ms(system_time_to_ms(time)), Some(time));
        assert_eq!(system_time_from_ms(-1), None);
    }
}
