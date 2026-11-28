//! 隔离执行面测试：真实文件、真实清单与真实 ZIP 字节。

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::*;
use crate::history_quarantine::export::{
    DEFAULT_EXPORT_LIMITS, export_batch, validate_quarantine_copy,
};
use crate::history_quarantine::maintenance::{
    PermanentDeleteRejection, QuarantineItemOutcome, confirm_permanent_delete, permanent_delete,
    reconcile_record, undo_record,
};
use crate::history_quarantine::persist::{
    BatchStage, QuarantineBatchRecord, QuarantineItemState, load_batch_material,
};
use crate::persistence::workspace::WorkspaceDatabase;

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

fn batch(seed: u8) -> QuarantineBatchId {
    let mut bytes = [0_u8; 16];
    bytes[0] = 0x02;
    bytes[1] = 0x9b;
    bytes[2] = seed;
    bytes[3] = 0xda;
    bytes[4] = 0xa8;
    bytes[5] = seed;
    bytes[6] = 0x70;
    bytes[7] = seed;
    bytes[8] = 0x80;
    QuarantineBatchId(uuid::Uuid::from_bytes(bytes))
}

fn sha(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes).into()
}

fn write_sources(directory: &Path, names: &[(&str, &str)]) -> Vec<(PathBuf, [u8; 32])> {
    names
        .iter()
        .map(|(name, content)| {
            let path = directory.join(name);
            std::fs::write(&path, content).unwrap();
            (path, sha(content.as_bytes()))
        })
        .collect()
}

fn completed_record(
    root: &Path,
    id: QuarantineBatchId,
    sources: &[(PathBuf, [u8; 32])],
    completed_at: SystemTime,
) -> QuarantineBatchRecord {
    let outcome = quarantine_batch(root, id, sources).unwrap();
    assert_eq!(outcome.status, BatchStatus::Completed);
    let mut record = QuarantineBatchRecord::from_outcome(&outcome, completed_at);
    record.completed_at = Some(completed_at);
    record
}

/// 批次清单必须同时给出完成与部分失败两类逐项事实。
#[test]
fn quarantine_record_tracks_completed_and_partial_items() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one"), ("two.jsonl", "two")]);

    let outcome = quarantine_batch(&root, batch(1), &sources).unwrap();
    let record = QuarantineBatchRecord::from_outcome(&outcome, at(100));
    assert_eq!(record.status, BatchStatus::Completed);
    assert_eq!(record.stage, BatchStage::Committed);
    assert_eq!(record.items.len(), 2);
    assert!(
        record
            .items
            .iter()
            .all(|item| item.state == QuarantineItemState::SourceRemoved)
    );
    assert!(record.manifest_name.ends_with(".manifest.json"));
    assert!(sources.iter().all(|(path, _)| !path.exists()));

    // 来源变化导致指纹复核失败：来源必须保留，失败项进入待处理。
    let pending = temp.path().join("pending.jsonl");
    std::fs::write(&pending, b"pending").unwrap();
    let failing = vec![
        (pending, sha(b"pending")),
        (temp.path().join("changed.jsonl"), sha(b"expected")),
    ];
    std::fs::write(&failing[1].0, b"changed").unwrap();
    let outcome = quarantine_batch(&root, batch(2), &failing).unwrap();
    let record = QuarantineBatchRecord::from_outcome(&outcome, at(101));
    assert_eq!(record.status, BatchStatus::NeedsAttention);
    assert_eq!(record.stage, BatchStage::Copied);
    assert!(
        record.items.iter().any(|item| {
            item.state == QuarantineItemState::Failed
                && item.failure == Some(QuarantineFailurePhase::FingerprintMismatch)
        }),
        "失败项必须保留阶段"
    );
    assert!(
        record
            .items
            .iter()
            .any(|item| item.state == QuarantineItemState::Staged),
        "已校验副本但来源未删除的项必须是暂存态"
    );
    assert!(
        failing.iter().all(|(path, _)| path.exists()),
        "任一副本失败时全部来源保留"
    );
    let material = load_batch_material(&root, &record).unwrap();
    assert_eq!(material.entries.len(), 2);
}

/// 启动对账只按清单与真实文件事实修正逐项状态，不把可疑批次伪装完成。
#[test]
fn quarantine_reconcile_marks_missing_copy_and_reappeared_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one")]);
    let record = completed_record(&root, batch(3), &sources, at(100));
    let material = load_batch_material(&root, &record).unwrap();
    std::fs::remove_file(&material.entries[0].copy).unwrap();

    let reconciled = reconcile_record(&root, &record, at(101));
    assert_eq!(reconciled.status, BatchStatus::NeedsAttention);
    assert_eq!(
        reconciled.items[0].failure,
        Some(QuarantineFailurePhase::ReconcileMissingCopy)
    );

    // 来源重新出现：不得继续隐藏，也不得推断为已完整隔离。
    let sources = write_sources(temp.path(), &[("two.jsonl", "two")]);
    let record = completed_record(&root, batch(4), &sources, at(100));
    std::fs::write(&sources[0].0, b"external").unwrap();
    let reconciled = reconcile_record(&root, &record, at(101));
    assert_eq!(reconciled.status, BatchStatus::NeedsAttention);
    assert_eq!(reconciled.items[0].state, QuarantineItemState::Staged);
}

/// ZIP 导出保留原始 UTF-16 路径清单，且有明确字节上限。
#[test]
fn quarantine_export_zip_preserves_manifest_and_enforces_limit() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("历史 会话.jsonl", "payload")]);
    let record = completed_record(&root, batch(5), &sources, at(100));
    let destination = temp.path().join("export.zip");

    export_batch(&root, &record, &destination, DEFAULT_EXPORT_LIMITS).unwrap();

    let bytes = std::fs::read(&destination).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    let names = zip_local_names(&bytes);
    assert!(names.contains(&"manifest.json".to_owned()), "{names:?}");
    assert!(names.contains(&"items/0000.jsonl".to_owned()), "{names:?}");
    let manifest_bytes = zip_local_entry(&bytes, "manifest.json").unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let expected = path_to_wide(&sources[0].0);
    assert_eq!(
        manifest["items"][0]["source"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u16)
            .collect::<Vec<_>>(),
        expected,
        "清单必须按 UTF-16 无损保存来源路径"
    );

    let limited = temp.path().join("limited.zip");
    let result = export_batch(
        &root,
        &record,
        &limited,
        super::export::ExportLimits {
            max_bytes: 4,
            max_entries: DEFAULT_EXPORT_LIMITS.max_entries,
        },
    );
    assert!(result.is_err(), "超过上限必须拒绝导出");
    assert!(!limited.exists(), "失败不得留下半写导出件");
}

/// 导出拒绝清单中的父链越界与符号链接父链。
#[test]
fn quarantine_export_rejects_traversal_and_symlink_parents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one")]);
    let record = completed_record(&root, batch(6), &sources, at(100));
    let manifest_path = root.join(&record.manifest_name);
    let original = std::fs::read_to_string(&manifest_path).unwrap();

    // 把副本路径改到隔离根之外：父链校验必须拒绝。
    let outside = temp.path().join("outside.jsonl");
    std::fs::write(&outside, b"outside").unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(&original).unwrap();
    manifest["items"][0]["copy"] = serde_json::json!(path_to_wide(&outside));
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let destination = temp.path().join("traversal.zip");
    assert!(
        export_batch(&root, &record, &destination, DEFAULT_EXPORT_LIMITS).is_err(),
        "越界副本路径必须拒绝导出"
    );
    assert!(!destination.exists());
    assert!(validate_quarantine_copy(&root, &outside).is_err());

    // 通过 junction 指向隔离根内部：父链含 reparse point，同样拒绝。
    let junction = temp.path().join("junction-store");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&root)
        .status()
        .unwrap();
    if status.success() {
        let linked_copy = junction.join(
            sources[0]
                .0
                .file_name()
                .map(|name| name.to_os_string())
                .unwrap(),
        );
        // 恢复真实清单，再把副本路径改为经 junction 的等价路径。
        std::fs::write(&manifest_path, &original).unwrap();
        let real_copy = load_batch_material(&root, &record).unwrap().entries[0]
            .copy
            .clone();
        std::fs::write(&linked_copy, std::fs::read(&real_copy).unwrap()).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(&original).unwrap();
        manifest["items"][0]["copy"] = serde_json::json!(path_to_wide(&linked_copy));
        std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let result = export_batch(&root, &record, &destination, DEFAULT_EXPORT_LIMITS);
        assert!(result.is_err(), "junction 父链必须拒绝导出");
    }
}

/// 永久删除必须显式确认；待处理项删除失败保留可重试事实与逐项结果。
#[test]
fn quarantine_permanent_delete_requires_confirmation_and_reports_items() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one"), ("two.jsonl", "two")]);
    let record = completed_record(&root, batch(7), &sources, at(100));
    assert!(
        matches!(
            confirm_permanent_delete(&record, at(104)),
            Err(PermanentDeleteRejection::NotExpired)
        ),
        "撤销窗口内不得永久删除"
    );

    let confirmation = confirm_permanent_delete(&record, at(105)).unwrap();
    let result = permanent_delete(&root, &record, confirmation, at(105));
    assert!(result.complete(), "全量删除后记录可移除");
    assert!(
        result
            .items
            .iter()
            .all(|item| item.outcome == QuarantineItemOutcome::Deleted)
    );
    assert!(!root.join(&record.manifest_name).exists());
    assert!(
        result
            .record
            .items
            .iter()
            .all(|item| item.state == QuarantineItemState::Deleted)
    );

    // 待处理批次可显式删除；单项失败保留清单与待处理事实。
    let sources = write_sources(temp.path(), &[("three.jsonl", "three")]);
    let outcome =
        quarantine_batch(&root, batch(8), &[(sources[0].0.clone(), sha(b"wrong"))]).unwrap();
    let pending = QuarantineBatchRecord::from_outcome(&outcome, at(200));
    let material = load_batch_material(&root, &pending).unwrap();
    // 用目录占住副本路径，使删除单项失败。
    std::fs::create_dir(&material.entries[0].copy).unwrap();
    std::fs::write(material.entries[0].copy.join("child"), b"x").unwrap();
    let confirmation = confirm_permanent_delete(&pending, at(200)).unwrap();
    let result = permanent_delete(&root, &pending, confirmation, at(200));
    assert!(!result.complete(), "单项删除失败不得移除记录");
    assert_eq!(
        result.items[0].outcome,
        QuarantineItemOutcome::Failed(QuarantineFailurePhase::DeleteCopy)
    );
    assert_eq!(result.record.status, BatchStatus::NeedsAttention);
    assert!(root.join(&pending.manifest_name).exists());

    std::fs::remove_dir_all(&material.entries[0].copy).unwrap();
    let confirmation = confirm_permanent_delete(&result.record, at(201)).unwrap();
    let retried = permanent_delete(&root, &result.record, confirmation, at(201));
    assert!(retried.complete(), "删除失败必须可重试");
    assert!(!root.join(&pending.manifest_name).exists());
}

/// 撤销失败进入待处理；撤销成功后副本保留，且逐项给出恢复结果。
#[test]
fn quarantine_undo_reports_items_and_keeps_copies() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one")]);
    let mut record = completed_record(&root, batch(9), &sources, at(100));
    record.status = BatchStatus::NeedsAttention;

    let restored = undo_record(&root, &record, at(200));
    assert_eq!(restored.record.status, BatchStatus::Undone);
    assert_eq!(restored.items[0].outcome, QuarantineItemOutcome::Restored);
    assert_eq!(std::fs::read(&sources[0].0).unwrap(), b"one");
    assert!(
        load_batch_material(&root, &record)
            .unwrap()
            .entries
            .iter()
            .all(|entry| entry.copy.exists()),
        "撤销后副本保留由到期清理处理"
    );
}

/// 重启快照只隐藏逐项状态为已移除的来源；撤销成功后恢复可见。
#[test]
fn quarantine_snapshot_hides_only_source_removed_items() {
    use crate::history_quarantine::persist::QuarantineSnapshot;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let sources = write_sources(temp.path(), &[("one.jsonl", "one")]);
    let record = completed_record(&root, batch(14), &sources, at(100));

    let snapshot = QuarantineSnapshot::from_records(&root, vec![record.clone()]);
    assert!(
        snapshot.hidden_sources.contains(&sources[0].0),
        "已移除来源必须在重启后继续隐藏"
    );

    let mut undone = record;
    undone.status = BatchStatus::Undone;
    undone.items[0].state = QuarantineItemState::Restored;
    let snapshot = QuarantineSnapshot::from_records(&root, vec![undone]);
    assert!(
        snapshot.hidden_sources.is_empty(),
        "撤销成功后来源必须恢复可见"
    );
}

/// 工作区记录与清单一一对应；启动对账清理过期完成批次，保留待处理批次。
#[test]
fn quarantine_startup_reconciles_orphans_and_expires_only_safe_batches() {
    let temp = tempfile::tempdir().unwrap();
    let paths =
        slterm_path::AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let root = paths.local_root().join("history-quarantine");
    let mut store = WorkspaceDatabase::open(&paths).unwrap();

    let expired_sources = write_sources(temp.path(), &[("expired.jsonl", "expired")]);
    let expired = completed_record(&root, batch(10), &expired_sources, at(100));
    store.upsert_quarantine_batch(&expired).unwrap();

    let fresh_sources = write_sources(temp.path(), &[("fresh.jsonl", "fresh")]);
    let fresh = completed_record(&root, batch(11), &fresh_sources, at(108));
    store.upsert_quarantine_batch(&fresh).unwrap();

    let attention_sources = write_sources(temp.path(), &[("attention.jsonl", "attention")]);
    let attention = quarantine_batch(
        &root,
        batch(12),
        &[(attention_sources[0].0.clone(), sha(b"wrong"))],
    )
    .unwrap();
    let attention = QuarantineBatchRecord::from_outcome(&attention, at(100));
    store.upsert_quarantine_batch(&attention).unwrap();

    // 没有数据库行的清单：来源已删除、副本完整 → 保留为待处理并继续隐藏。
    let orphan_sources = write_sources(temp.path(), &[("orphan.jsonl", "orphan")]);
    let orphan = quarantine_batch(&root, batch(13), &orphan_sources).unwrap();
    let orphan_manifest = orphan.manifest_path.clone();
    assert!(orphan_manifest.exists());

    let report = store.reconcile_history_quarantine(&root, at(110)).unwrap();
    assert!(report.orphans_recovered >= 1);

    let records = store.quarantine_records().unwrap();
    assert!(
        records.iter().all(|record| record.id != expired.id),
        "过期完成批次必须清理"
    );
    assert!(
        !root.join(&expired.manifest_name).exists(),
        "过期清单必须删除"
    );
    let fresh_record = records.iter().find(|record| record.id == fresh.id).unwrap();
    assert_eq!(fresh_record.status, BatchStatus::Completed);
    let attention_record = records
        .iter()
        .find(|record| record.id == attention.id)
        .unwrap();
    assert_eq!(attention_record.status, BatchStatus::NeedsAttention);
    let orphan_record = records
        .iter()
        .find(|record| {
            record.manifest_name == orphan_manifest.file_name().unwrap().to_string_lossy()
        })
        .unwrap();
    assert_eq!(orphan_record.status, BatchStatus::NeedsAttention);
    assert_eq!(
        orphan_record.items[0].state,
        QuarantineItemState::SourceRemoved,
        "来源已删除的孤儿清单必须继续隐藏"
    );
}

fn zip_local_names(bytes: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut offset = 0usize;
    while offset + 30 <= bytes.len() && &bytes[offset..offset + 4] == b"PK\x03\x04" {
        let flags = u16::from_le_bytes([bytes[offset + 6], bytes[offset + 7]]);
        let name_len = u16::from_le_bytes([bytes[offset + 26], bytes[offset + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
        let name =
            String::from_utf8_lossy(&bytes[offset + 30..offset + 30 + name_len]).into_owned();
        names.push(name);
        let compressed = u32::from_le_bytes([
            bytes[offset + 18],
            bytes[offset + 19],
            bytes[offset + 20],
            bytes[offset + 21],
        ]) as usize;
        offset += 30 + name_len + extra_len + compressed;
        if flags & 0x0008 != 0 {
            // 数据描述符：跳过签名、crc 与两个 size。
            offset += 16;
        }
    }
    names
}

fn zip_local_entry(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut offset = 0usize;
    while offset + 30 <= bytes.len() && &bytes[offset..offset + 4] == b"PK\x03\x04" {
        let name_len = u16::from_le_bytes([bytes[offset + 26], bytes[offset + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
        let entry_name =
            String::from_utf8_lossy(&bytes[offset + 30..offset + 30 + name_len]).into_owned();
        let compressed = u32::from_le_bytes([
            bytes[offset + 18],
            bytes[offset + 19],
            bytes[offset + 20],
            bytes[offset + 21],
        ]) as usize;
        let data_start = offset + 30 + name_len + extra_len;
        if entry_name == name {
            return Some(bytes[data_start..data_start + compressed].to_vec());
        }
        offset = data_start + compressed + 16;
    }
    None
}

/// 供后续路径校验测试使用的真实空文件。
#[allow(dead_code)]
fn touch(path: &Path) -> io::Result<()> {
    std::fs::write(path, b"")
}
