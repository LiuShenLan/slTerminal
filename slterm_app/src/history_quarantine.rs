//! 历史隔离批次的撤销窗口状态机。
//!
//! 完成时间提交后才启动 5 秒绝对撤销期限；批次之间期限彼此独立。
//! 悬停、聚焦、关闭视图、退出或重启都不延长也不提前永久化。
//! 撤销只有 [`HistoryQuarantine::undo_batch`] 一个执行入口：文件恢复成功后才
//! 推进为已撤销；预检阻断或文件失败进入待处理，保留材料并允许不限原窗口重试，
//! 待处理状态跨重启保留且永不自动永久化。
//!
//! 生产执行面：导航删除先经 [`quarantine_batch`] 形成不可变清单与副本，持久
//! 记录由 [`persist`] 与工作区表承载；启动维护读取清单对账并清理到期安全批次，
//! 撤销、[`export`] 与永久删除只消费同一记录与清单。

pub(crate) mod export;
pub(crate) mod maintenance;
pub(crate) mod persist;

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest as _, Sha256};

/// 绝对撤销窗口：从批次完成时间起算，不随页面活动顺延。
pub(crate) const UNDO_WINDOW: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct QuarantineBatchId(pub(crate) uuid::Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchStatus {
    /// 副本尚未完整校验并移除来源。
    Prepared,
    /// 完成时间已提交，进入撤销窗口。
    Completed,
    /// 撤销失败或预检阻断；材料保留，可不限原窗口重试。
    NeedsAttention,
    /// 已按当前批次整体撤销；材料由撤销流程保留或恢复。
    Undone,
}

/// 跨重启的批次状态事实；完成时间缺失或待处理必须显式保留。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PersistedBatch {
    pub(crate) id: QuarantineBatchId,
    pub(crate) status: BatchStatus,
    pub(crate) completed_at: Option<SystemTime>,
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

/// 隔离清单单项：原始位置、隔离副本与确认删除时的来源指纹。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineEntry {
    pub(crate) order: usize,
    pub(crate) source: PathBuf,
    pub(crate) copy: PathBuf,
    pub(crate) source_fingerprint: [u8; 32],
}

/// 一个隔离批次的完整材料；撤销与到期清理都以整批为单位。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuarantineBatchMaterial {
    pub(crate) id: QuarantineBatchId,
    pub(crate) entries: Vec<QuarantineEntry>,
}

/// 隔离批次的逐项失败阶段；失败项保留来源与副本供重试或导出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuarantineFailurePhase {
    Read,
    FingerprintMismatch,
    CopyWrite,
    CopyVerify,
    SourceRemove,
    /// 撤销预检：目标位置已有外部对象。
    RestoreTargetExists,
    /// 撤销预检：隔离副本已不存在。
    RestoreCopyMissing,
    /// 撤销预检：隔离副本指纹不一致。
    RestoreCopyCorrupted,
    /// 撤销恢复阶段 I/O 失败。
    RestoreIo,
    /// 导出读取隔离副本失败。
    ExportRead,
    /// 导出写入 ZIP 失败。
    ExportWrite,
    /// 导出路径越界或父链含符号链接。
    ExportUnsafePath,
    /// 导出超过有界上限。
    ExportTooLarge,
    /// 永久删除隔离副本失败。
    DeleteCopy,
    /// 永久删除清单失败。
    DeleteManifest,
    /// 启动对账：隔离副本缺失或损坏。
    ReconcileMissingCopy,
    /// 启动对账：批次清单缺失或损坏。
    ReconcileMissingManifest,
}

impl QuarantineFailurePhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::FingerprintMismatch => "fingerprint_mismatch",
            Self::CopyWrite => "copy_write",
            Self::CopyVerify => "copy_verify",
            Self::SourceRemove => "source_remove",
            Self::RestoreTargetExists => "restore_target_exists",
            Self::RestoreCopyMissing => "restore_copy_missing",
            Self::RestoreCopyCorrupted => "restore_copy_corrupted",
            Self::RestoreIo => "restore_io",
            Self::ExportRead => "export_read",
            Self::ExportWrite => "export_write",
            Self::ExportUnsafePath => "export_unsafe_path",
            Self::ExportTooLarge => "export_too_large",
            Self::DeleteCopy => "delete_copy",
            Self::DeleteManifest => "delete_manifest",
            Self::ReconcileMissingCopy => "reconcile_missing_copy",
            Self::ReconcileMissingManifest => "reconcile_missing_manifest",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "read" => Self::Read,
            "fingerprint_mismatch" => Self::FingerprintMismatch,
            "copy_write" => Self::CopyWrite,
            "copy_verify" => Self::CopyVerify,
            "source_remove" => Self::SourceRemove,
            "restore_target_exists" => Self::RestoreTargetExists,
            "restore_copy_missing" => Self::RestoreCopyMissing,
            "restore_copy_corrupted" => Self::RestoreCopyCorrupted,
            "restore_io" => Self::RestoreIo,
            "export_read" => Self::ExportRead,
            "export_write" => Self::ExportWrite,
            "export_unsafe_path" => Self::ExportUnsafePath,
            "export_too_large" => Self::ExportTooLarge,
            "delete_copy" => Self::DeleteCopy,
            "delete_manifest" => Self::DeleteManifest,
            "reconcile_missing_copy" => Self::ReconcileMissingCopy,
            "reconcile_missing_manifest" => Self::ReconcileMissingManifest,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QuarantineItemFailure {
    pub(crate) entry_index: usize,
    pub(crate) source: PathBuf,
    pub(crate) phase: QuarantineFailurePhase,
    pub(crate) kind: io::ErrorKind,
}

/// 批次执行结果：`material` 只含已删除来源（撤销集合），`staged` 含全部已校验副本。
#[derive(Clone, Debug)]
pub(crate) struct QuarantineBatchOutcome {
    /// 清单内的全部顺序项；撤销集合与暂存集合都只引用这里。
    pub(crate) planned: Vec<QuarantineEntry>,
    pub(crate) material: QuarantineBatchMaterial,
    pub(crate) staged: Vec<QuarantineEntry>,
    pub(crate) status: BatchStatus,
    pub(crate) manifest_path: PathBuf,
    pub(crate) failures: Vec<QuarantineItemFailure>,
}

/// 隔离清单的持久格式；清单一次写入，不做原地更新。
pub(crate) const QUARANTINE_MANIFEST_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct QuarantineManifest {
    version: u32,
    batch_id: String,
    items: Vec<QuarantineManifestItem>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct QuarantineManifestItem {
    order: usize,
    source: Vec<u16>,
    copy: Vec<u16>,
    fingerprint: Vec<u8>,
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str().encode_wide().collect()
}

fn path_from_wide(units: &[u16]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt as _;
    PathBuf::from(std::ffi::OsString::from_wide(units))
}

/// 启动对账读取的批次清单；清单只读且不推断完成状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedQuarantineManifest {
    pub(crate) batch_id: QuarantineBatchId,
    pub(crate) entries: Vec<QuarantineEntry>,
}

pub(crate) fn load_manifest(path: &Path) -> io::Result<LoadedQuarantineManifest> {
    let bytes = std::fs::read(path)?;
    let manifest: QuarantineManifest = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if manifest.version != QUARANTINE_MANIFEST_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "隔离清单版本不受支持",
        ));
    }
    let batch_id = uuid::Uuid::parse_str(&manifest.batch_id)
        .map(QuarantineBatchId)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut items = manifest.items;
    items.sort_by_key(|item| item.order);
    let entries = items
        .into_iter()
        .map(|item| {
            let fingerprint: [u8; 32] = item
                .fingerprint
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "隔离清单指纹长度无效"))?;
            Ok(QuarantineEntry {
                order: item.order,
                source: path_from_wide(&item.source),
                copy: path_from_wide(&item.copy),
                source_fingerprint: fingerprint,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(LoadedQuarantineManifest { batch_id, entries })
}

/// 隔离批次的文件 I/O 端口；真实实现走原子写与真实删除，测试可注入失败。
pub(crate) trait QuarantineIo {
    fn ensure_store(&mut self, store: &Path) -> io::Result<()>;
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>>;
    fn write_copy(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn remove_source(&mut self, path: &Path) -> io::Result<()>;
    fn write_manifest(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()>;
}

pub(crate) struct SystemQuarantineIo;

impl QuarantineIo for SystemQuarantineIo {
    fn ensure_store(&mut self, store: &Path) -> io::Result<()> {
        std::fs::create_dir_all(store)
    }

    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write_copy(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        crate::atomic_file::write(path, bytes)
    }

    fn remove_source(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn write_manifest(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        crate::atomic_file::write(path, bytes)
    }
}

/// 单项目撤销预检失败原因；任一失败即整批零恢复。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UndoBlockReason {
    /// 目标位置已有外部对象（新增或修改）。
    TargetExists,
    /// 隔离副本已不存在。
    CopyMissing,
    /// 隔离副本指纹与确认删除时不一致。
    CopyCorrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UndoBlockage {
    pub(crate) entry: usize,
    pub(crate) reason: UndoBlockReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UndoRestoreOutcome {
    /// 全部恢复；副本材料保留，到期清理单独处理。
    Restored,
    /// 预检失败：零恢复，来源与副本原样保留供重试。
    Blocked(Vec<UndoBlockage>),
}

/// 单次撤销尝试的完整结论；状态推进与文件恢复在同一入口闭合。
#[derive(Debug)]
pub(crate) enum UndoAttempt {
    /// 未达到执行条件（未完成、已撤销、已到期或未知批次）。
    Rejected(UndoDecision),
    /// 全部恢复且状态已推进为已撤销。
    Restored,
    /// 预检阻断且状态已进入待处理；材料保留供重试。
    Blocked(Vec<UndoBlockage>),
    /// 文件副作用失败且状态已进入待处理；已创建的部分来源保留。
    Failed(io::Error),
}

impl PartialEq for UndoAttempt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Rejected(left), Self::Rejected(right)) => left == right,
            (Self::Restored, Self::Restored) => true,
            (Self::Blocked(left), Self::Blocked(right)) => left == right,
            (Self::Failed(left), Self::Failed(right)) => left.kind() == right.kind(),
            _ => false,
        }
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// 全批撤销：先逐项预检目标不存在与副本完整性，全部通过才逐项恢复。
///
/// 预检失败返回 [`UndoRestoreOutcome::Blocked`] 且零写入；恢复阶段错误
/// 按 `Err` 上抛，已创建的部分来源保留（与副本构成双份材料）供人工处理。
fn restore_quarantine_batch(material: &QuarantineBatchMaterial) -> io::Result<UndoRestoreOutcome> {
    let mut blockages = Vec::new();
    for (index, entry) in material.entries.iter().enumerate() {
        if precheck_undo_target(entry.source.exists()) == UndoTargetDecision::ExternalTarget {
            blockages.push(UndoBlockage {
                entry: index,
                reason: UndoBlockReason::TargetExists,
            });
            continue;
        }
        match std::fs::read(&entry.copy) {
            Ok(bytes) if sha256(&bytes) == entry.source_fingerprint => {}
            Ok(_) => blockages.push(UndoBlockage {
                entry: index,
                reason: UndoBlockReason::CopyCorrupted,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                blockages.push(UndoBlockage {
                    entry: index,
                    reason: UndoBlockReason::CopyMissing,
                });
            }
            Err(error) => return Err(error),
        }
    }
    if !blockages.is_empty() {
        return Ok(UndoRestoreOutcome::Blocked(blockages));
    }
    for entry in &material.entries {
        restore_entry(entry)?;
    }
    Ok(UndoRestoreOutcome::Restored)
}

/// 单项目恢复：`create_new` 保证不覆盖预检后新出现的外部对象。
fn restore_entry(entry: &QuarantineEntry) -> io::Result<()> {
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&entry.source)?;
    let mut copy = std::fs::File::open(&entry.copy)?;
    if let Err(error) = io::copy(&mut copy, &mut target) {
        drop(target);
        let _ = std::fs::remove_file(&entry.source);
        return Err(error);
    }
    target.sync_all()?;
    Ok(())
}

/// 执行删除的隔离阶段：先原子持久化不可变清单，再写副本并校验，
/// 全部副本完整后才逐项删除来源。
///
/// 任一副本失败时不删除任何来源；部分删除失败逐项报告，已删来源进入撤销集合。
pub(crate) fn quarantine_batch(
    store: &Path,
    id: QuarantineBatchId,
    items: &[(PathBuf, [u8; 32])],
) -> io::Result<QuarantineBatchOutcome> {
    quarantine_batch_with(store, id, items, &mut SystemQuarantineIo)
}

pub(crate) fn quarantine_batch_with(
    store: &Path,
    id: QuarantineBatchId,
    items: &[(PathBuf, [u8; 32])],
    io: &mut dyn QuarantineIo,
) -> io::Result<QuarantineBatchOutcome> {
    if !store.is_absolute() || items.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "隔离批次输入无效",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (source, _) in items {
        if !source.is_absolute()
            || source
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            || !seen.insert(source.clone())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "隔离来源路径无效或重复",
            ));
        }
    }
    io.ensure_store(store)?;
    // 清单在任何副作用前落盘：崩溃后可按来源/副本事实逐项对账。
    let mut planned = Vec::with_capacity(items.len());
    for (order, (source, fingerprint)) in items.iter().enumerate() {
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "来源缺少有效名称"))?;
        let copy = store.join(format!("{}-{order}-{file_name}", id.0));
        planned.push(QuarantineEntry {
            order,
            source: source.clone(),
            copy,
            source_fingerprint: *fingerprint,
        });
    }
    let manifest = QuarantineManifest {
        version: QUARANTINE_MANIFEST_VERSION,
        batch_id: id.0.to_string(),
        items: planned
            .iter()
            .enumerate()
            .map(|(order, entry)| QuarantineManifestItem {
                order,
                source: path_to_wide(&entry.source),
                copy: path_to_wide(&entry.copy),
                fingerprint: entry.source_fingerprint.to_vec(),
            })
            .collect(),
    };
    let manifest_path = store.join(format!("{}.manifest.json", id.0));
    let bytes = serde_json::to_vec(&manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    io.write_manifest(&manifest_path, &bytes)?;

    let mut staged = Vec::with_capacity(planned.len());
    let mut failures = Vec::new();
    for (index, entry) in planned.iter().enumerate() {
        match stage_copy(io, entry) {
            Ok(()) => staged.push(entry.clone()),
            Err((phase, kind)) => failures.push(QuarantineItemFailure {
                entry_index: index,
                source: entry.source.clone(),
                phase,
                kind,
            }),
        }
    }
    // 副本完整后才允许移除来源；任一副本失败时整批来源保持不变。
    if !failures.is_empty() {
        return Ok(QuarantineBatchOutcome {
            planned,
            material: QuarantineBatchMaterial {
                id,
                entries: Vec::new(),
            },
            staged,
            status: BatchStatus::NeedsAttention,
            manifest_path,
            failures,
        });
    }
    let mut entries = Vec::with_capacity(staged.len());
    for (index, entry) in staged.iter().enumerate() {
        match io.remove_source(&entry.source) {
            Ok(()) => entries.push(entry.clone()),
            Err(error) => failures.push(QuarantineItemFailure {
                entry_index: index,
                source: entry.source.clone(),
                phase: QuarantineFailurePhase::SourceRemove,
                kind: error.kind(),
            }),
        }
    }
    let status = if failures.is_empty() {
        BatchStatus::Completed
    } else {
        BatchStatus::NeedsAttention
    };
    Ok(QuarantineBatchOutcome {
        planned,
        material: QuarantineBatchMaterial { id, entries },
        staged,
        status,
        manifest_path,
        failures,
    })
}

/// 单项副本阶段：读取来源、校验指纹、原子写副本并回读校验。
fn stage_copy(
    io: &mut dyn QuarantineIo,
    entry: &QuarantineEntry,
) -> Result<(), (QuarantineFailurePhase, io::ErrorKind)> {
    let bytes = io
        .read(&entry.source)
        .map_err(|error| (QuarantineFailurePhase::Read, error.kind()))?;
    if sha256(&bytes) != entry.source_fingerprint {
        return Err((
            QuarantineFailurePhase::FingerprintMismatch,
            io::ErrorKind::InvalidData,
        ));
    }
    io.write_copy(&entry.copy, &bytes)
        .map_err(|error| (QuarantineFailurePhase::CopyWrite, error.kind()))?;
    let written = io
        .read(&entry.copy)
        .map_err(|error| (QuarantineFailurePhase::CopyVerify, error.kind()))?;
    if sha256(&written) != entry.source_fingerprint {
        return Err((
            QuarantineFailurePhase::CopyVerify,
            io::ErrorKind::InvalidData,
        ));
    }
    Ok(())
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

    /// 持久事实：批次身份、状态与已提交的完成时间；待处理状态不得丢失。
    pub(crate) fn persisted_windows(&self) -> Vec<PersistedBatch> {
        self.batches
            .iter()
            .map(|(id, batch)| PersistedBatch {
                id: *id,
                status: batch.status,
                completed_at: batch.completed_at,
            })
            .collect()
    }

    /// 重启恢复：待处理与已撤销状态原样保留；缺完成时间不得伪装为可撤销或已到期。
    pub(crate) fn from_persisted(windows: &[PersistedBatch]) -> Self {
        let mut service = Self::new();
        for window in windows {
            // 完成状态必须有完成时间；缺失时降级为未完成，不能伪装可撤销。
            let status = if window.status == BatchStatus::Completed && window.completed_at.is_none()
            {
                BatchStatus::Prepared
            } else {
                window.status
            };
            service.batches.insert(
                window.id,
                QuarantineBatch {
                    status,
                    completed_at: window.completed_at,
                },
            );
        }
        service
    }

    /// 已到期且仍可撤销的完成批次身份；待处理批次永不自动永久化。
    pub(crate) fn expired(&self, now: SystemTime) -> Vec<QuarantineBatchId> {
        self.batches
            .iter()
            .filter(|(_, batch)| batch.status == BatchStatus::Completed)
            .filter(|(id, _)| !self.undoable(**id, now))
            .map(|(id, _)| *id)
            .collect()
    }

    /// 撤销点击的只读预检：不改变状态；通过后必须走 [`Self::undo_batch`]。
    pub(crate) fn check_undo(&self, id: QuarantineBatchId, now: SystemTime) -> UndoDecision {
        let Some(status) = self.batches.get(&id).map(|batch| batch.status) else {
            return UndoDecision::Unknown;
        };
        match status {
            BatchStatus::Prepared => UndoDecision::NotCompleted,
            BatchStatus::Undone => UndoDecision::AlreadyUndone,
            // 待处理批次保留材料，可在原窗口后继续显式重试。
            BatchStatus::NeedsAttention => UndoDecision::Allowed,
            BatchStatus::Completed => {
                if !self.undoable(id, now) {
                    return UndoDecision::Expired;
                }
                UndoDecision::Allowed
            }
        }
    }

    /// 撤销的唯一执行入口：先检查状态与期限，再恢复文件，最后按真实结局推进状态。
    ///
    /// 预检或文件恢复失败进入待处理态，保留全部材料供重试；绝不在副作用前
    /// 标记已撤销。成功后才推进为 `Undone`。
    pub(crate) fn undo_batch(
        &mut self,
        material: &QuarantineBatchMaterial,
        now: SystemTime,
    ) -> UndoAttempt {
        match self.check_undo(material.id, now) {
            UndoDecision::Allowed => {}
            decision => return UndoAttempt::Rejected(decision),
        }
        match restore_quarantine_batch(material) {
            Ok(UndoRestoreOutcome::Restored) => {
                self.complete_undo(material.id);
                UndoAttempt::Restored
            }
            Ok(UndoRestoreOutcome::Blocked(blockages)) => {
                self.fail_undo(material.id);
                UndoAttempt::Blocked(blockages)
            }
            Err(error) => {
                self.fail_undo(material.id);
                UndoAttempt::Failed(error)
            }
        }
    }

    /// 恢复成功后的状态推进；只接受仍在撤销流程中的批次。
    pub(crate) fn complete_undo(&mut self, id: QuarantineBatchId) -> bool {
        let Some(batch) = self.batches.get_mut(&id) else {
            return false;
        };
        if !matches!(
            batch.status,
            BatchStatus::Completed | BatchStatus::NeedsAttention
        ) {
            return false;
        }
        batch.status = BatchStatus::Undone;
        true
    }

    /// 恢复失败后的状态推进：保留完成时间与材料，进入不限原窗口的待处理态。
    pub(crate) fn fail_undo(&mut self, id: QuarantineBatchId) -> bool {
        let Some(batch) = self.batches.get_mut(&id) else {
            return false;
        };
        if !matches!(
            batch.status,
            BatchStatus::Completed | BatchStatus::NeedsAttention
        ) {
            return false;
        }
        batch.status = BatchStatus::NeedsAttention;
        true
    }
}

#[cfg(test)]
mod action_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn sha(bytes: &[u8]) -> [u8; 32] {
        use sha2::{Digest as _, Sha256};
        Sha256::digest(bytes).into()
    }

    /// 内存隔离端口：记录调用顺序并按索引注入复制／删除失败。
    #[derive(Default)]
    struct FakeQuarantineIo {
        files: std::sync::Mutex<std::collections::BTreeMap<PathBuf, Vec<u8>>>,
        calls: Vec<String>,
        fail_manifest: bool,
        fail_copy_index: Option<usize>,
        fail_remove_index: Option<usize>,
        copy_count: usize,
        remove_count: usize,
    }

    impl FakeQuarantineIo {
        fn insert(&self, path: PathBuf, bytes: Vec<u8>) {
            self.files.lock().unwrap().insert(path, bytes);
        }

        fn contains(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }

        fn read_raw(&self, path: &Path) -> Option<Vec<u8>> {
            self.files.lock().unwrap().get(path).cloned()
        }

        fn fail_copy_index(&mut self, index: usize) {
            self.fail_copy_index = Some(index);
        }

        fn fail_remove_index(&mut self, index: usize) {
            self.fail_remove_index = Some(index);
        }
    }

    impl QuarantineIo for FakeQuarantineIo {
        fn ensure_store(&mut self, _store: &Path) -> io::Result<()> {
            Ok(())
        }

        fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing"))
        }

        fn write_copy(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            let index = self.copy_count;
            self.copy_count += 1;
            self.calls.push(format!("copy:{}", path.display()));
            if self.fail_copy_index == Some(index) {
                return Err(io::Error::other("copy failed"));
            }
            self.insert(path.to_path_buf(), bytes.to_vec());
            Ok(())
        }

        fn remove_source(&mut self, path: &Path) -> io::Result<()> {
            let index = self.remove_count;
            self.remove_count += 1;
            self.calls.push(format!("remove:{}", path.display()));
            if self.fail_remove_index == Some(index) {
                return Err(io::Error::other("remove failed"));
            }
            self.files.lock().unwrap().remove(path);
            Ok(())
        }

        fn write_manifest(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            self.calls.push("manifest".to_owned());
            if self.fail_manifest {
                return Err(io::Error::other("manifest failed"));
            }
            self.insert(path.to_path_buf(), bytes.to_vec());
            Ok(())
        }
    }

    fn stage_material(
        directory: &std::path::Path,
        items: &[(&str, &str)],
    ) -> QuarantineBatchMaterial {
        let store = directory.join("quarantine");
        std::fs::create_dir_all(&store).unwrap();
        let entries = items
            .iter()
            .enumerate()
            .map(|(order, (name, content))| {
                let copy = store.join(name);
                std::fs::write(&copy, content).unwrap();
                QuarantineEntry {
                    order,
                    source: directory.join(name),
                    copy,
                    source_fingerprint: sha(content.as_bytes()),
                }
            })
            .collect();
        QuarantineBatchMaterial {
            id: batch(11),
            entries,
        }
    }

    /// 构造已完成且处于撤销窗口内的批次服务。
    fn undo_ready(material: &QuarantineBatchMaterial) -> HistoryQuarantine {
        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(material.id));
        assert!(service.complete_batch(material.id, at(100)));
        service
    }

    /// 目标外部新增、副本缺失或指纹不符时零恢复并保留全部材料。
    #[test]
    fn quarantine_undo_rejects_external_target_at_file_level() {
        let directory = tempfile::tempdir().unwrap();
        let material = stage_material(directory.path(), &[("a.txt", "old-a"), ("b.txt", "old-b")]);

        let mut service = undo_ready(&material);
        std::fs::write(directory.path().join("a.txt"), "external").unwrap();
        let outcome = service.undo_batch(&material, at(104));
        assert_eq!(
            outcome,
            UndoAttempt::Blocked(vec![UndoBlockage {
                entry: 0,
                reason: UndoBlockReason::TargetExists,
            }]),
            "外部目标必须拒绝恢复"
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("a.txt")).unwrap(),
            "external",
            "外部内容不得被覆盖"
        );
        assert!(
            !directory.path().join("b.txt").exists(),
            "任一预检失败必须零恢复"
        );
        assert!(material.entries.iter().all(|entry| entry.copy.exists()));

        std::fs::remove_file(directory.path().join("a.txt")).unwrap();
        std::fs::write(&material.entries[1].copy, "tampered").unwrap();
        let outcome = service.undo_batch(&material, at(104));
        assert_eq!(
            outcome,
            UndoAttempt::Blocked(vec![UndoBlockage {
                entry: 1,
                reason: UndoBlockReason::CopyCorrupted,
            }]),
            "副本指纹不符必须拒绝恢复"
        );
        assert!(!directory.path().join("a.txt").exists(), "拒绝时零恢复");

        std::fs::remove_file(&material.entries[1].copy).unwrap();
        let outcome = service.undo_batch(&material, at(104));
        assert_eq!(
            outcome,
            UndoAttempt::Blocked(vec![UndoBlockage {
                entry: 1,
                reason: UndoBlockReason::CopyMissing,
            }]),
            "副本缺失必须拒绝恢复"
        );

        std::fs::write(&material.entries[1].copy, "old-b").unwrap();
        assert_eq!(
            service.undo_batch(&material, at(104)),
            UndoAttempt::Restored
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("a.txt")).unwrap(),
            "old-a"
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("b.txt")).unwrap(),
            "old-b"
        );
        assert!(
            material.entries.iter().all(|entry| entry.copy.exists()),
            "撤销后副本材料保留，到期清理单独处理"
        );
    }

    /// 清单必须先落盘，之后才允许复制副本或删除来源。
    #[test]
    fn quarantine_manifest_is_persisted_before_touching_sources() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [
            (directory.path().join("one.jsonl"), b"one".to_vec()),
            (directory.path().join("two.jsonl"), b"two".to_vec()),
        ];
        let mut io = FakeQuarantineIo::default();
        for (path, bytes) in &sources {
            io.insert(path.clone(), bytes.clone());
        }
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();

        let outcome = quarantine_batch_with(&store, batch(30), &items, &mut io).unwrap();

        assert_eq!(outcome.status, BatchStatus::Completed);
        assert_eq!(
            io.calls.first().map(String::as_str),
            Some("manifest"),
            "批次清单必须先于任何副本写入或来源删除"
        );
        let manifest = io
            .read_raw(&outcome.manifest_path)
            .expect("清单必须真实存在");
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(manifest["version"], 1);
        assert_eq!(manifest["batch_id"], batch(30).0.to_string());
        assert_eq!(manifest["items"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["items"][0]["order"], 0);
        assert_eq!(
            manifest["items"][0]["fingerprint"]
                .as_array()
                .unwrap()
                .len(),
            32
        );
        assert_eq!(outcome.material.entries.len(), 2);
        assert!(outcome.failures.is_empty());
    }

    /// 任一副本失败时不得删除任何来源，并返回逐项失败与已产生材料。
    #[test]
    fn quarantine_copy_failure_keeps_all_sources_and_reports_items() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [
            (directory.path().join("one.jsonl"), b"one".to_vec()),
            (directory.path().join("two.jsonl"), b"two".to_vec()),
        ];
        let mut io = FakeQuarantineIo::default();
        for (path, bytes) in &sources {
            io.insert(path.clone(), bytes.clone());
        }
        io.fail_copy_index(1);
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();

        let outcome = quarantine_batch_with(&store, batch(31), &items, &mut io).unwrap();

        assert_eq!(outcome.status, BatchStatus::NeedsAttention);
        assert!(
            outcome.material.entries.is_empty(),
            "失败批次不属于撤销集合"
        );
        assert_eq!(
            outcome.staged.len(),
            1,
            "已校验副本属于已产生材料，必须返回"
        );
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].phase, QuarantineFailurePhase::CopyWrite);
        assert!(
            !io.calls.iter().any(|call| call.starts_with("remove:")),
            "任一副本失败时不得删除来源"
        );
        for (path, _) in &sources {
            assert!(io.contains(path), "来源必须保留: {}", path.display());
        }
        assert!(io.read_raw(&outcome.manifest_path).is_some());
    }

    /// 清单必须可被启动对账原样读取：批次、顺序与来源/副本指纹完整。
    #[test]
    fn quarantine_manifest_round_trips_for_startup_reconciliation() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [
            (directory.path().join("one.jsonl"), b"one".to_vec()),
            (directory.path().join("two.jsonl"), b"two".to_vec()),
        ];
        for (path, bytes) in &sources {
            std::fs::write(path, bytes).unwrap();
        }
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();
        let outcome = quarantine_batch(&store, batch(34), &items).unwrap();

        let loaded = load_manifest(&outcome.manifest_path).unwrap();

        assert_eq!(loaded.batch_id, batch(34));
        assert_eq!(loaded.entries, outcome.material.entries);
    }

    /// 空批次、相对路径与重复来源必须在任何副作用前拒绝。
    #[test]
    fn quarantine_batch_rejects_invalid_sources_before_side_effects() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("one.jsonl");
        let mut io = FakeQuarantineIo::default();
        io.insert(source.clone(), b"one".to_vec());

        assert!(
            quarantine_batch_with(&directory.path().join("store"), batch(40), &[], &mut io)
                .is_err()
        );
        assert!(
            quarantine_batch_with(
                Path::new("relative-store"),
                batch(41),
                &[(source.clone(), sha(b"one"))],
                &mut io
            )
            .is_err()
        );
        assert!(
            quarantine_batch_with(
                &directory.path().join("store"),
                batch(42),
                &[(source.clone(), sha(b"one")), (source, sha(b"one"))],
                &mut io,
            )
            .is_err()
        );
        assert!(
            io.calls.is_empty(),
            "非法输入不得写清单、副本或来源: {:?}",
            io.calls
        );
    }

    /// 清单落盘失败必须零副作用：不复制、不删除任何来源。
    #[test]
    fn quarantine_manifest_failure_touches_no_source() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [(directory.path().join("one.jsonl"), b"one".to_vec())];
        let mut io = FakeQuarantineIo::default();
        for (path, bytes) in &sources {
            io.insert(path.clone(), bytes.clone());
        }
        io.fail_manifest = true;
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();

        let result = quarantine_batch_with(&store, batch(33), &items, &mut io);

        assert!(result.is_err());
        assert!(
            io.calls.iter().all(|call| call == "manifest"),
            "清单失败后不得写副本或删除来源: {:?}",
            io.calls
        );
        assert!(io.contains(&sources[0].0));
    }

    /// 副本全部校验后删除来源中途失败：已删项属于撤销集合，失败项保留来源与待处理态。
    #[test]
    fn quarantine_remove_failure_reports_completed_and_failed_items() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [
            (directory.path().join("one.jsonl"), b"one".to_vec()),
            (directory.path().join("two.jsonl"), b"two".to_vec()),
        ];
        let mut io = FakeQuarantineIo::default();
        for (path, bytes) in &sources {
            io.insert(path.clone(), bytes.clone());
        }
        io.fail_remove_index(1);
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();

        let outcome = quarantine_batch_with(&store, batch(32), &items, &mut io).unwrap();

        assert_eq!(outcome.status, BatchStatus::NeedsAttention);
        assert_eq!(
            outcome.material.entries.len(),
            1,
            "只有已删来源进入撤销集合"
        );
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].phase,
            QuarantineFailurePhase::SourceRemove
        );
        assert!(!io.contains(&sources[0].0), "成功项来源已删除");
        assert!(io.contains(&sources[1].0), "失败项来源必须保留");
        assert!(
            outcome
                .material
                .entries
                .iter()
                .all(|entry| io.contains(&entry.copy)),
            "已产生副本必须保留"
        );
    }

    /// 端到端：删除形成批次 → 完成窗口内撤销恢复；窗口内外部重建时整批拒绝。
    #[test]
    fn integration_history_delete_undo() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("quarantine");
        let sources = [
            (directory.path().join("one.jsonl"), b"one".to_vec()),
            (directory.path().join("two.jsonl"), b"two".to_vec()),
        ];
        for (path, bytes) in &sources {
            std::fs::write(path, bytes).unwrap();
        }
        let items = sources
            .iter()
            .map(|(path, bytes)| (path.clone(), sha(bytes)))
            .collect::<Vec<_>>();

        let outcome = quarantine_batch(&store, batch(12), &items).unwrap();
        assert_eq!(outcome.status, BatchStatus::Completed);
        assert!(outcome.failures.is_empty());
        let material = outcome.material;
        for (path, _) in &sources {
            assert!(!path.exists(), "成功隔离后来源必须删除");
        }
        assert_eq!(material.entries.len(), 2);
        assert!(material.entries.iter().all(|entry| entry.copy.exists()));

        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(material.id));
        assert!(service.complete_batch(material.id, at(100)));
        assert_eq!(
            service.check_undo(material.id, at(104)),
            UndoDecision::Allowed
        );
        assert_eq!(
            service.undo_batch(&material, at(104)),
            UndoAttempt::Restored,
            "窗口内撤销必须恢复全部来源"
        );
        assert_eq!(
            std::fs::read(&sources[0].0).unwrap(),
            b"one",
            "恢复内容必须来自隔离副本"
        );
        assert_eq!(
            service.undo_batch(&material, at(104)),
            UndoAttempt::Rejected(UndoDecision::AlreadyUndone),
            "成功撤销后不得再次恢复"
        );

        let material = quarantine_batch(&store, batch(13), &items)
            .unwrap()
            .material;
        assert!(service.start_batch(material.id));
        assert!(service.complete_batch(material.id, at(100)));
        std::fs::write(&sources[0].0, b"external").unwrap();
        let outcome = service.undo_batch(&material, at(104));
        assert_eq!(
            outcome,
            UndoAttempt::Blocked(vec![UndoBlockage {
                entry: 0,
                reason: UndoBlockReason::TargetExists,
            }])
        );
        assert_eq!(std::fs::read(&sources[0].0).unwrap(), b"external");
        assert!(!sources[1].0.exists(), "整批拒绝时不得部分恢复");
        assert!(material.entries.iter().all(|entry| entry.copy.exists()));
        // 失败批次进入待处理，窗口过后仍可重试，不自动永久化。
        assert_eq!(
            service.check_undo(material.id, at(10_000)),
            UndoDecision::Allowed
        );
        assert!(!service.expired(at(10_000)).contains(&material.id));
    }

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
            service.check_undo(batch(7), at(102)),
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
            service.check_undo(batch(4), at(100)),
            UndoDecision::NotCompleted,
            "未完成批次不得撤销"
        );
        assert!(service.complete_batch(batch(4), at(100)));
        assert_eq!(service.check_undo(batch(4), at(104)), UndoDecision::Allowed);
        assert!(
            service.complete_undo(batch(4)),
            "只有恢复成功后才推进为已撤销"
        );
        assert_eq!(
            service.check_undo(batch(4), at(104)),
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
            service.check_undo(batch(5), at(105)),
            UndoDecision::Expired,
            "点击时超过绝对期限必须拒绝"
        );
        assert!(
            !service.undoable(batch(5), at(105)),
            "到期判定与撤销检查必须一致"
        );
        assert_eq!(service.check_undo(batch(6), at(100)), UndoDecision::Unknown);
    }

    /// 恢复失败进入待处理并可跨重启重试；撤销状态绝不在副作用前推进。
    #[test]
    fn quarantine_undo_failure_state_survives_restart_and_allows_retry() {
        let mut service = HistoryQuarantine::new();
        assert!(service.start_batch(batch(14)));
        assert!(service.complete_batch(batch(14), at(100)));
        assert_eq!(
            service.check_undo(batch(14), at(104)),
            UndoDecision::Allowed
        );
        // 预检或文件恢复失败：不得提前标记已撤销。
        assert!(service.fail_undo(batch(14)));
        assert_eq!(
            service.check_undo(batch(14), at(104)),
            UndoDecision::Allowed,
            "失败批次必须保留重试能力"
        );
        assert!(
            !service.expired(at(10_000)).contains(&batch(14)),
            "待处理批次不得因期限到达被永久化"
        );
        assert!(
            service.check_undo(batch(14), at(10_000)) == UndoDecision::Allowed,
            "待处理批次在窗口后仍可重试"
        );

        let windows = service.persisted_windows();
        let mut restored = HistoryQuarantine::from_persisted(&windows);
        assert_eq!(
            restored.check_undo(batch(14), at(10_000)),
            UndoDecision::Allowed,
            "待处理状态必须跨重启保留"
        );
        assert!(restored.complete_undo(batch(14)));
        assert_eq!(
            restored.check_undo(batch(14), at(10_000)),
            UndoDecision::AlreadyUndone
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
