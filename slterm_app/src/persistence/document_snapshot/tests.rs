//! 真实快照文件、配额与删除边界的自动化证据。

use std::collections::BTreeSet;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Instant;

use ropey::Rope;
use slterm_document::{
    DocumentId, DocumentMetadata, DocumentRegistry, DocumentSaveReceipt, EditOrigin,
    EditTransaction, MergePolicy, PageIndex, PagedSource, TextEdit, TextStateId,
};
use slterm_path::{AppPaths, CanonicalPath, ResolvedPath};
use slterm_workspace::{
    CloseDecision, CloseIntent, CloseObject, CloseScope, CloseTarget, ProjectRegistry,
    ProjectSnapshot,
};

use super::envelope::{DocumentSnapshot, SnapshotCaptureError, SnapshotStoreError};
use super::store::DocumentSnapshotStore;
use super::writer::DocumentSnapshotWriter;
use crate::persistence::close::{CloseCommitStatus, PersistedCloseIntent};
use crate::persistence::metadata::RunMarker;
use crate::persistence::workspace::WorkspaceDatabase;

fn id(document: char) -> DocumentId {
    DocumentId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e{document}0")).unwrap()
}

fn state(document: char, revision: char) -> TextStateId {
    TextStateId::from_str(&format!(
        "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f{document}{revision}"
    ))
    .unwrap()
}

fn registry(document: char, baseline: &str) -> (DocumentRegistry, DocumentId) {
    let id = id(document);
    let mut registry = DocumentRegistry::new();
    registry
        .open(
            id,
            ResolvedPath::new(
                CanonicalPath::new(format!("D:/work/{document}.txt")).unwrap(),
                None,
            ),
            Rope::from_str(baseline),
            state(document, '0'),
            Arc::from(baseline.as_bytes()),
            DocumentMetadata::default(),
        )
        .unwrap();
    (registry, id)
}

fn edit(registry: &mut DocumentRegistry, id: DocumentId, document: char, text: &str) {
    let end = registry.session(id).unwrap().text().unwrap().len();
    let next_revision = registry.session(id).unwrap().text_revision() + 1;
    let revision = char::from_digit(next_revision as u32, 10).unwrap();
    registry
        .apply(
            EditTransaction {
                document_id: id,
                expected_text_revision: registry.session(id).unwrap().text_revision(),
                edits: vec![TextEdit::new(0..end, text)],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            },
            state(document, revision),
            Instant::now(),
        )
        .unwrap();
}

fn snapshot(document: char, text: &str) -> (DocumentRegistry, DocumentSnapshot) {
    let (mut registry, id) = registry(document, "disk");
    edit(&mut registry, id, document, text);
    let snapshot = DocumentSnapshot::capture(registry.session(id).unwrap(), "run-test").unwrap();
    (registry, snapshot)
}

fn store(temp: &tempfile::TempDir, quota: u64) -> DocumentSnapshotStore {
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store.total_quota = quota;
    store
}

fn protected(documents: &[DocumentId]) -> BTreeSet<DocumentId> {
    documents.iter().copied().collect()
}

fn close(id: DocumentId, status: CloseCommitStatus) -> PersistedCloseIntent {
    PersistedCloseIntent {
        close_id: "close-snapshot".to_owned(),
        scope: CloseScope::ApplicationExit,
        store_epoch: "epoch-test".to_owned(),
        intent: CloseIntent::new(
            vec![CloseObject {
                id: CloseTarget::Document(id),
                generation: 0,
                decision: CloseDecision::Discard,
            }],
            [7; 32],
        )
        .unwrap(),
        commit_status: status,
    }
}

#[test]
fn snapshot_envelope_round_trip_and_rejects_damage() {
    let (_registry, snapshot) = snapshot('a', "中文\ndraft\n");
    let encoded = snapshot.encode().unwrap();
    assert!(encoded.starts_with(b"SLTSNAP1"));
    assert_eq!(DocumentSnapshot::decode(&encoded).unwrap(), snapshot);

    let mut wrong_hash = encoded.clone();
    *wrong_hash.last_mut().unwrap() ^= 1;
    assert!(DocumentSnapshot::decode(&wrong_hash).is_err());
    let mut wrong_length = encoded.clone();
    wrong_length[20..28].copy_from_slice(&1_u64.to_le_bytes());
    assert!(DocumentSnapshot::decode(&wrong_length).is_err());
    assert!(DocumentSnapshot::decode(&encoded[..encoded.len() - 1]).is_err());
}

#[test]
fn snapshot_quota_does_not_evict_dirty() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp, 12);
    let (_registry_a, a) = snapshot('a', "aaaaaa");
    let (_registry_b, b) = snapshot('b', "bbbbbb");
    let (_registry_c, c) = snapshot('c', "cccccc");
    store.write(&a, &protected(&[a.document_id()])).unwrap();
    store
        .write(&b, &protected(&[a.document_id(), b.document_id()]))
        .unwrap();

    assert_eq!(
        store.write(
            &c,
            &protected(&[a.document_id(), b.document_id(), c.document_id()])
        ),
        Err(SnapshotStoreError::QuotaExceeded)
    );
    assert!(store.read(a.document_id()).unwrap().is_some());
    assert!(store.read(b.document_id()).unwrap().is_some());
    assert!(store.read(c.document_id()).unwrap().is_none());

    store
        .write(&c, &protected(&[a.document_id(), c.document_id()]))
        .unwrap();
    assert!(store.read(a.document_id()).unwrap().is_some());
    assert!(store.read(b.document_id()).unwrap().is_none());
    assert!(store.read(c.document_id()).unwrap().is_some());
}

#[test]
fn integration_snapshot_quota() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp, 12);
    let (_registry_a, a) = snapshot('a', "12345678");
    let (_registry_b, b) = snapshot('b', "abcd");
    store.write(&a, &protected(&[a.document_id()])).unwrap();

    // 总配额恰好容纳两个打开脏 Document；超额候选不得替换任一现场。
    store
        .write(&b, &protected(&[a.document_id(), b.document_id()]))
        .unwrap();
    assert_eq!(
        store.read(a.document_id()).unwrap().unwrap().text(),
        "12345678"
    );
    assert_eq!(store.read(b.document_id()).unwrap().unwrap().text(), "abcd");

    let (_registry_c, oversized) = snapshot('c', &"x".repeat(13));
    assert_eq!(
        store.write(
            &oversized,
            &protected(&[a.document_id(), b.document_id(), oversized.document_id()])
        ),
        Err(SnapshotStoreError::QuotaExceeded)
    );
    assert_eq!(
        store.read(a.document_id()).unwrap().unwrap().text(),
        "12345678"
    );
    assert_eq!(store.read(b.document_id()).unwrap().unwrap().text(), "abcd");
}

#[test]
fn snapshot_is_deleted_only_after_clean_or_committed_close() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp, 1024);
    let (mut registry, snapshot) = snapshot('a', "draft");
    let id = snapshot.document_id();
    store.write(&snapshot, &protected(&[id])).unwrap();
    store.reconcile(registry.session(id).unwrap()).unwrap();
    assert!(store.read(id).unwrap().is_some());

    let request = registry.begin_save(id, 0).unwrap();
    registry
        .complete_save(DocumentSaveReceipt::new(
            request,
            Arc::from(b"draft".as_slice()),
            None,
        ))
        .unwrap();
    store.reconcile(registry.session(id).unwrap()).unwrap();
    assert!(store.read(id).unwrap().is_none());

    edit(&mut registry, id, 'a', "second");
    let snapshot = DocumentSnapshot::capture(registry.session(id).unwrap(), "run-test").unwrap();
    store.write(&snapshot, &protected(&[id])).unwrap();
    assert_eq!(
        registry.undo(id),
        Ok(slterm_document::EditApplyOutcome::Applied)
    );
    store.reconcile(registry.session(id).unwrap()).unwrap();
    assert!(store.read(id).unwrap().is_none());

    edit(&mut registry, id, 'a', "third");
    let snapshot = DocumentSnapshot::capture(registry.session(id).unwrap(), "run-test").unwrap();
    store.write(&snapshot, &protected(&[id])).unwrap();
    assert_eq!(
        store.delete_closed(id, &close(id, CloseCommitStatus::Collected)),
        Err(SnapshotStoreError::CloseNotCommitted)
    );
    assert!(store.read(id).unwrap().is_some());
    store
        .delete_closed(id, &close(id, CloseCommitStatus::StateCommitted))
        .unwrap();
    assert!(store.read(id).unwrap().is_none());
}

#[test]
fn corrupt_snapshot_is_preserved_for_quarantine() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp, 1024);
    let (_registry, snapshot) = snapshot('a', "draft");
    let id = snapshot.document_id();
    store.write(&snapshot, &protected(&[id])).unwrap();
    let path = temp
        .path()
        .join("local/snapshots")
        .join(format!("{id}.snap"));
    let mut bytes = std::fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&path, bytes).unwrap();

    assert_eq!(
        store.delete_closed(id, &close(id, CloseCommitStatus::StateCommitted)),
        Err(SnapshotStoreError::Corrupt)
    );
    assert!(path.exists());
}

#[test]
fn snapshot_writer_flushes_latest_and_retries_failed_quota() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut writer = DocumentSnapshotWriter::start_with_store(store(&temp, 12)).unwrap();
    let client = writer.client();
    let (_registry_a, a) = snapshot('a', "aaaaaa");
    let (mut registry_b, old_b) = snapshot('b', "old");
    edit(&mut registry_b, old_b.document_id(), 'b', "bbbbbb");
    let new_b =
        DocumentSnapshot::capture(registry_b.session(old_b.document_id()).unwrap(), "run-test")
            .unwrap();
    client
        .write(&old_b, &protected(&[old_b.document_id()]))
        .unwrap();
    client
        .write(&new_b, &protected(&[new_b.document_id()]))
        .unwrap();
    client
        .write(&a, &protected(&[a.document_id(), new_b.document_id()]))
        .unwrap();

    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    let store = DocumentSnapshotStore::new(&paths);
    assert_eq!(
        store.read(new_b.document_id()).unwrap().unwrap().text(),
        "bbbbbb"
    );
    assert_eq!(
        store.read(a.document_id()).unwrap().unwrap().text(),
        "aaaaaa"
    );

    let (_registry_c, c) = snapshot('c', "cccccc");
    client
        .write(
            &c,
            &protected(&[a.document_id(), new_b.document_id(), c.document_id()]),
        )
        .unwrap();
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(report.failures().len(), 1);
    assert_eq!(report.failures()[0].1, SnapshotStoreError::QuotaExceeded);
    assert!(store.read(c.document_id()).unwrap().is_none());

    // 已提交关闭取消 B 的保护并排队删除；原失败 C 可在下一 flush 重试。
    client
        .delete_closed(
            new_b.document_id(),
            &close(new_b.document_id(), CloseCommitStatus::StateCommitted),
        )
        .unwrap();
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    assert!(store.read(new_b.document_id()).unwrap().is_none());
    assert!(store.read(c.document_id()).unwrap().is_some());
    writer.shutdown().unwrap();
}

#[test]
fn snapshot_shutdown_failure_retains_pending_for_retry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut writer = DocumentSnapshotWriter::start_with_store(store(&temp, 6)).unwrap();
    let client = writer.client();
    let (_registry_a, a) = snapshot('a', "aaaaaa");
    let (_registry_b, b) = snapshot('b', "bbbbbb");
    client.write(&a, &protected(&[a.document_id()])).unwrap();
    client
        .write(&b, &protected(&[a.document_id(), b.document_id()]))
        .unwrap();

    assert!(writer.shutdown().is_err());
    client
        .delete_closed(
            a.document_id(),
            &close(a.document_id(), CloseCommitStatus::StateCommitted),
        )
        .unwrap();
    writer.shutdown().unwrap();
    let store = DocumentSnapshotStore::new(&paths);
    assert!(store.read(a.document_id()).unwrap().is_none());
    assert!(store.read(b.document_id()).unwrap().is_some());
}

#[test]
fn snapshot_debounce_failure_is_reported_without_flush() {
    let temp = tempfile::tempdir().unwrap();
    let mut writer = DocumentSnapshotWriter::start_with_store(store(&temp, 6)).unwrap();
    let failures = writer.take_failures().unwrap();
    let client = writer.client();
    let (_registry_a, a) = snapshot('a', "aaaaaa");
    let (_registry_b, b) = snapshot('b', "bbbbbb");
    client.write(&a, &protected(&[a.document_id()])).unwrap();
    client
        .write(&b, &protected(&[a.document_id(), b.document_id()]))
        .unwrap();

    let failure = failures
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        failure,
        (b.document_id(), SnapshotStoreError::QuotaExceeded)
    );
    client
        .delete_closed(
            a.document_id(),
            &close(a.document_id(), CloseCommitStatus::StateCommitted),
        )
        .unwrap();
    writer.shutdown().unwrap();
}

#[test]
fn snapshot_writer_drop_flushes_pending() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = DocumentSnapshotWriter::start(&paths).unwrap();
    let (_registry, snapshot) = snapshot('a', "disconnect");
    writer
        .client()
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    drop(writer);

    let store = DocumentSnapshotStore::new(&paths);
    assert_eq!(
        store.read(snapshot.document_id()).unwrap().unwrap().text(),
        "disconnect"
    );
}

#[test]
fn snapshot_delete_commit_boundary_crash() {
    const TEST: &str =
        "persistence::document_snapshot::tests::snapshot_delete_commit_boundary_crash";
    const ROOT: &str = "SLTERM_SNAPSHOT_CRASH_ROOT";
    const PHASE: &str = "SLTERM_SNAPSHOT_CRASH_PHASE";
    if let Ok(root) = std::env::var(ROOT) {
        let root = std::path::PathBuf::from(root);
        let paths = AppPaths::new(root.join("config"), root.join("local")).unwrap();
        let phase = std::env::var(PHASE).unwrap();
        let (_registry, snapshot) = snapshot('a', "draft");
        let id = snapshot.document_id();
        let mut snapshots = DocumentSnapshotStore::new(&paths);
        snapshots.write(&snapshot, &protected(&[id])).unwrap();
        let mut database = WorkspaceDatabase::open(&paths).unwrap();
        let run = database
            .begin_run(
                "epoch-test",
                RunMarker {
                    run_id: "run-snapshot-crash".to_owned(),
                    process_id: std::process::id(),
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
        let mut close = close(id, CloseCommitStatus::Collected);
        close.store_epoch = run.current.store_epoch;
        let close = database
            .commit_close_intent(
                &ProjectSnapshot::restore(ProjectRegistry::new(), Vec::new()).unwrap(),
                &close,
            )
            .unwrap();
        if phase == "after" {
            snapshots.delete_closed(id, &close).unwrap();
        }
        // 已收到真实 SQLite 提交回执；跳过全部析构与正常退出链。
        std::process::exit(0);
    }

    for phase in ["before", "after"] {
        let temp = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--test-threads=1"])
            .env(ROOT, temp.path())
            .env(PHASE, phase)
            .status()
            .unwrap();
        assert!(status.success());
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let snapshots = DocumentSnapshotStore::new(&paths);
        let id = id('a');
        assert_eq!(snapshots.read(id).unwrap().is_some(), phase == "before");
        let mut database = WorkspaceDatabase::open(&paths).unwrap();
        let close = database
            .load_close_intent("close-snapshot")
            .unwrap()
            .unwrap();
        assert_eq!(close.commit_status, CloseCommitStatus::StateCommitted);
    }
}

#[test]
fn clean_paged_and_oversized_documents_have_no_snapshot() {
    let (clean_registry, id) = registry('a', "disk");
    assert_eq!(
        DocumentSnapshot::capture(clean_registry.session(id).unwrap(), "run-test"),
        Err(SnapshotCaptureError::NotDirty)
    );

    let mut paged = DocumentRegistry::new();
    paged
        .open(
            id,
            ResolvedPath::new(CanonicalPath::new("D:/work/paged.txt").unwrap(), None),
            PagedSource::new(PageIndex::new(0, Vec::new()).unwrap(), [0; 32]),
            state('a', '0'),
            Arc::default(),
            DocumentMetadata {
                read_only: true,
                ..DocumentMetadata::default()
            },
        )
        .unwrap();
    assert_eq!(
        DocumentSnapshot::capture(paged.session(id).unwrap(), "run-test"),
        Err(SnapshotCaptureError::ReadOnly)
    );

    let (mut registry, id) = registry('a', "disk");
    edit(&mut registry, id, 'a', &"x".repeat((64 * 1024 * 1024) + 1));
    assert_eq!(
        DocumentSnapshot::capture(registry.session(id).unwrap(), "run-test"),
        Err(SnapshotCaptureError::BodyTooLarge)
    );
}
