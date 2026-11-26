//! 真实快照文件、配额与删除边界的自动化证据。

use std::collections::BTreeSet;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Instant;

use ropey::Rope;
use slterm_document::{
    DocumentId, DocumentMetadata, DocumentRegistry, DocumentSaveReceipt, EditOrigin,
    EditTransaction, MergePolicy, PageIndex, PagedLineIndex, PagedSource, TextEdit, TextStateId,
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
use crate::persistence::startup::recovery::SnapshotRecoverer;
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

/// 无关损坏文件不得阻断新快照写入；恢复扫描仍把它列为隔离候选。
#[test]
fn unrelated_corrupt_snapshot_does_not_block_writes() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp, 1024 * 1024);
    let (_registry, a) = snapshot('a', "draft");
    store.write(&a, &protected(&[])).unwrap();
    let corrupt = temp.path().join("local/snapshots/garbage.snap");
    std::fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
    std::fs::write(&corrupt, b"not a snapshot").unwrap();
    store
        .write(&a, &protected(&[]))
        .expect("损坏旁路文件不得阻断写入");
    let scan = store.scan_for_recovery().unwrap();
    assert!(scan.corrupt.contains(&corrupt));
}

/// 文件名与头身份不一致时按头身份恢复，而不是当作损坏。
#[test]
fn snapshot_header_identity_wins_over_file_name() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp, 1024 * 1024);
    let (_registry, a) = snapshot('a', "draft");
    let directory = temp.path().join("local/snapshots");
    std::fs::create_dir_all(&directory).unwrap();
    let wrong_name = directory.join(format!("{}.snap", id('b')));
    std::fs::write(&wrong_name, a.encode().unwrap()).unwrap();
    let scan = store.scan_for_recovery().unwrap();
    assert_eq!(scan.snapshots.len(), 1);
    assert_eq!(scan.snapshots[0].document_id(), a.document_id());
    assert!(scan.corrupt.is_empty());
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

fn recovered_layout(
    project_root: &std::path::Path,
    document: DocumentId,
    location: ResolvedPath,
) -> ProjectSnapshot {
    let project =
        slterm_workspace::ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a").unwrap();
    let group =
        slterm_workspace::PanelGroupId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a").unwrap();
    let mut projects = ProjectRegistry::new();
    projects
        .add_project(
            project,
            CanonicalPath::new(dunce::canonicalize(project_root).unwrap()).unwrap(),
        )
        .unwrap();
    ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project, group, document, location)
        .unwrap()
        .0
}

fn recovered_pair_layout(
    root: &std::path::Path,
    old_id: DocumentId,
    old_location: ResolvedPath,
    new_id: DocumentId,
    new_location: ResolvedPath,
) -> ProjectSnapshot {
    let layout = recovered_layout(root, old_id, old_location);
    layout
        .with_opened_document(
            slterm_workspace::ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a").unwrap(),
            slterm_workspace::PanelGroupId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a")
                .unwrap(),
            new_id,
            new_location,
        )
        .unwrap()
        .0
}

fn snapshot_at(
    document: char,
    path: &std::path::Path,
    disk: &str,
    draft: &str,
) -> DocumentSnapshot {
    snapshot_at_encoding(document, path, disk, draft, None)
}

fn snapshot_at_encoding(
    document: char,
    path: &std::path::Path,
    disk: &str,
    draft: &str,
    requested: Option<slterm_document::DocumentEncoding>,
) -> DocumentSnapshot {
    let bytes = requested.map_or_else(
        || disk.as_bytes().to_vec(),
        |encoding| {
            slterm_document::TextEncoding {
                encoding,
                bom: false,
                line_ending: slterm_document::LineEnding::Lf,
                trailing_newline: disk.ends_with('\n'),
            }
            .encode(disk)
            .unwrap()
        },
    );
    std::fs::write(path, &bytes).unwrap();
    let canonical = CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
    let identity = crate::atomic_file::file_identity(&std::fs::File::open(path).unwrap()).unwrap();
    let location = ResolvedPath::new(canonical, Some(identity));
    let total_bytes = bytes.len() as u64;
    let slterm_document::DocumentContent::Editable(decoded) =
        slterm_document::DocumentContent::decode(bytes, total_bytes, requested)
    else {
        panic!("测试基线必须可编辑")
    };
    let id = id(document);
    let mut registry = DocumentRegistry::new();
    registry
        .open(
            id,
            location,
            Rope::from_str(&decoded.text),
            state(document, '0'),
            decoded.disk_baseline,
            DocumentMetadata {
                encoding: decoded.encoding,
                read_only: false,
            },
        )
        .unwrap();
    edit(&mut registry, id, document, draft);
    DocumentSnapshot::capture(registry.session(id).unwrap(), "run-recovery").unwrap()
}

#[test]
fn snapshot_recovery_runs_after_layout_reconciliation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let snapshot = snapshot_at('a', &file, "disk\n", "draft\n");
    let location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&file).unwrap()).unwrap()),
    );
    let layout = recovered_layout(&root, snapshot.document_id(), location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut database = WorkspaceDatabase::open(&paths).unwrap();
    database.save_project_snapshot(&layout).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    let recovered =
        crate::persistence::startup::StartupRecovery::recover(&mut database, Some(&mut recoverer))
            .unwrap();

    assert_eq!(recovered, layout);
    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert_eq!(session.text().unwrap().to_string(), "draft\n");
    assert_eq!(session.saved_state(), state('a', '0'));
    assert_eq!(session.current_state(), state('a', '1'));
    assert_eq!(session.disk_baseline().as_ref(), b"disk\n");
    assert!(!session.is_read_revoked());

    let persistence = crate::app_shell::app_root::AppRootPersistence::load(&paths, None);
    let restored = persistence
        .recovered_documents()
        .session(snapshot.document_id())
        .unwrap();
    assert_eq!(restored.text().unwrap().to_string(), "draft\n");
    assert!(!persistence.document_requires_authorization(snapshot.document_id()));
}

#[test]
fn unrepresentable_snapshot_draft_still_restores() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let snapshot = snapshot_at_encoding(
        'a',
        &file,
        "disk\n",
        "draft 😀\n",
        Some(slterm_document::DocumentEncoding::Gbk),
    );
    let layout = recovered_layout(&root, snapshot.document_id(), snapshot.location().clone());
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert_eq!(session.text().unwrap().to_string(), "draft 😀\n");
    assert!(session.is_dirty());
    assert!(session.external_conflict().is_none());
}

#[test]
fn binary_external_change_preserves_snapshot_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let snapshot = snapshot_at('a', &file, "disk", "draft");
    std::fs::write(&file, [0, 1, 2]).unwrap();
    let layout = recovered_layout(&root, snapshot.document_id(), snapshot.location().clone());
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert_eq!(session.text().unwrap().to_string(), "draft");
    assert_eq!(
        session.external_conflict().unwrap().kind(),
        slterm_document::ExternalConflictKind::Modified
    );
}

fn location_of(path: &std::path::Path) -> ResolvedPath {
    ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(path).unwrap()).unwrap()),
    )
}

#[test]
fn replaced_clean_document_does_not_consume_new_dirty_document() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    std::fs::write(&file, "old clean").unwrap();
    let old_location = location_of(&file);
    crate::atomic_file::write(&file, b"replacement").unwrap();
    let new = snapshot_at('b', &file, "replacement", "new draft");
    let layout = recovered_pair_layout(
        &root,
        id('a'),
        old_location,
        new.document_id(),
        new.location().clone(),
    );
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store.write(&new, &protected(&[new.document_id()])).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert!(recoverer.registry().session(id('a')).is_none());
    assert_eq!(
        recoverer
            .registry()
            .session(new.document_id())
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "new draft"
    );
}

#[test]
fn redundant_old_identity_does_not_consume_new_dirty_document() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let old = snapshot_at('a', &file, "old clean", "replacement");
    crate::atomic_file::write(&file, b"replacement").unwrap();
    let new = snapshot_at('b', &file, "replacement", "new draft");
    let layout = recovered_pair_layout(
        &root,
        old.document_id(),
        old.location().clone(),
        new.document_id(),
        new.location().clone(),
    );
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store.write(&old, &protected(&[old.document_id()])).unwrap();
    store
        .write(&new, &protected(&[old.document_id(), new.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let old_session = recoverer.registry().session(old.document_id()).unwrap();
    assert!(!old_session.is_dirty());
    assert_eq!(
        old_session.location().file_identity(),
        old.location().file_identity()
    );
    assert_eq!(
        recoverer
            .registry()
            .session(new.document_id())
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "new draft"
    );
}

#[test]
fn replaced_clean_documents_do_not_merge_or_abort_startup() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    std::fs::write(&file, "old clean").unwrap();
    let old_location = location_of(&file);
    crate::atomic_file::write(&file, b"new clean").unwrap();
    let new_location = location_of(&file);
    let layout = recovered_pair_layout(&root, id('a'), old_location, id('b'), new_location.clone());
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert!(recoverer.registry().session(id('a')).is_none());
    assert_eq!(
        recoverer
            .registry()
            .session(id('b'))
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "new clean"
    );
    assert_eq!(
        recoverer
            .registry()
            .session(id('b'))
            .unwrap()
            .location()
            .file_identity(),
        new_location.file_identity()
    );
}

#[test]
fn binary_new_source_still_marks_clean_old_identity_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    std::fs::write(&file, "old clean").unwrap();
    let old_location = location_of(&file);
    crate::atomic_file::write(&file, b"replacement").unwrap();
    let new = snapshot_at('b', &file, "replacement", "new draft");
    std::fs::write(&file, [0, 1, 2]).unwrap();
    let layout = recovered_pair_layout(
        &root,
        id('a'),
        old_location,
        new.document_id(),
        new.location().clone(),
    );
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store.write(&new, &protected(&[new.document_id()])).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert!(recoverer.identity_was_replaced(id('a')));
    assert!(recoverer.registry().session(id('a')).is_none());
    assert_eq!(
        recoverer
            .registry()
            .session(new.document_id())
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "new draft"
    );
}

#[test]
fn replaced_file_and_previous_identity_restore_as_distinct_documents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let old = snapshot_at('a', &file, "disk", "old draft");
    crate::atomic_file::write(&file, b"replacement").unwrap();
    let new = snapshot_at('b', &file, "replacement", "new draft");
    assert_ne!(
        old.location().file_identity(),
        new.location().file_identity()
    );
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store.write(&old, &protected(&[old.document_id()])).unwrap();
    store
        .write(&new, &protected(&[old.document_id(), new.document_id()]))
        .unwrap();
    let mut projects = ProjectRegistry::new();
    let project =
        slterm_workspace::ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a").unwrap();
    let group =
        slterm_workspace::PanelGroupId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a").unwrap();
    projects
        .add_project(
            project,
            CanonicalPath::new(dunce::canonicalize(&root).unwrap()).unwrap(),
        )
        .unwrap();
    let (layout, _) = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project, group, old.document_id(), old.location().clone())
        .unwrap();
    let (layout, _) = layout
        .with_opened_document(project, group, new.document_id(), new.location().clone())
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert_eq!(
        recoverer
            .registry()
            .session(old.document_id())
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "old draft"
    );
    assert_eq!(
        recoverer
            .registry()
            .session(new.document_id())
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "new draft"
    );
    assert_eq!(
        recoverer
            .registry()
            .session(old.document_id())
            .unwrap()
            .location()
            .file_identity(),
        old.location().file_identity()
    );
    assert_eq!(
        recoverer
            .registry()
            .session(new.document_id())
            .unwrap()
            .location()
            .file_identity(),
        new.location().file_identity()
    );
}

#[test]
fn redundant_snapshot_reconciles_clean_without_disk_rewrite() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let snapshot = snapshot_at('a', &file, "disk\n", "draft\n");
    std::fs::write(&file, "draft\n").unwrap();
    let location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        None,
    );
    let layout = recovered_layout(&root, snapshot.document_id(), location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert!(!session.is_dirty());
    assert_eq!(session.text().unwrap().to_string(), "draft\n");
    assert!(store.read(snapshot.document_id()).unwrap().is_none());
}

#[test]
fn snapshot_disk_change_preserves_draft_and_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file.txt");
    let snapshot = snapshot_at('a', &file, "disk\n", "draft\n");
    std::fs::write(&file, "external\n").unwrap();
    let location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        None,
    );
    let layout = recovered_layout(&root, snapshot.document_id(), location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert!(session.is_dirty());
    assert_eq!(session.text().unwrap().to_string(), "draft\n");
    assert_eq!(session.disk_baseline().as_ref(), b"external\n");
    assert_eq!(
        session.external_conflict().unwrap().kind(),
        slterm_document::ExternalConflictKind::Modified
    );
    assert!(store.read(snapshot.document_id()).unwrap().is_some());
}

#[test]
fn outside_snapshot_restores_without_disk_read() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = temp.path().join("outside.txt");
    let snapshot = snapshot_at('a', &file, "disk", "draft");
    let location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        None,
    );
    let layout = recovered_layout(&root, snapshot.document_id(), location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let session = recoverer
        .registry()
        .session(snapshot.document_id())
        .unwrap();
    assert_eq!(session.text().unwrap().to_string(), "draft");
    assert!(session.is_read_revoked());
    assert!(recoverer.requires_authorization(snapshot.document_id()));
}

#[test]
fn outside_snapshot_recovery_marks_requires_authorization() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let location = ResolvedPath::new(
        CanonicalPath::new(temp.path().join("missing.txt")).unwrap(),
        None,
    );
    let document = id('a');
    let layout = recovered_layout(&root, document, location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert!(recoverer.registry().session(document).is_none());
    assert!(recoverer.requires_authorization(document));
    assert_eq!(recoverer.layout(), &layout);
}

#[test]
fn orphan_snapshot_requires_explicit_target() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (_registry, snapshot) = snapshot('a', "orphan");
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let layout = ProjectSnapshot::restore(ProjectRegistry::new(), Vec::new()).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert_eq!(recoverer.orphans(), std::slice::from_ref(&snapshot));
    assert!(
        recoverer
            .registry()
            .session(snapshot.document_id())
            .is_none()
    );
    assert!(store.read(snapshot.document_id()).unwrap().is_some());
    assert_eq!(recoverer.layout(), &layout);
}

#[test]
fn corrupt_snapshot_is_quarantined_without_blocking_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (_registry, snapshot) = snapshot('a', "damaged");
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let path = temp
        .path()
        .join("local/snapshots")
        .join(format!("{}.snap", snapshot.document_id()));
    let mut bytes = std::fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&path, bytes).unwrap();
    let layout = ProjectSnapshot::restore(ProjectRegistry::new(), Vec::new()).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    assert_eq!(recoverer.quarantined().len(), 1);
    assert!(recoverer.quarantined()[0].exists());
    assert!(recoverer.orphans().is_empty());
    assert!(store.read(snapshot.document_id()).unwrap().is_none());
}

/// 快照位置与布局位置漂移（外部改名后崩溃）不得拖垮整次恢复：正文保留并标记冲突。
#[test]
fn snapshot_location_drift_restores_draft_with_conflict_instead_of_failing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let old = temp.path().join("old.txt");
    let new = root.join("new.txt");
    std::fs::write(&new, "other\n").unwrap();
    let snapshot = snapshot_at('a', &old, "disk", "draft");
    let new_location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&new).unwrap()).unwrap(),
        Some(crate::atomic_file::file_identity(&std::fs::File::open(&new).unwrap()).unwrap()),
    );
    let layout = recovered_layout(&root, snapshot.document_id(), new_location.clone());
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);

    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();

    let registry = recoverer.take_registry();
    let session = registry.session(snapshot.document_id()).unwrap();
    assert_eq!(
        session.text().unwrap().to_string(),
        "draft",
        "漂移位置的快照正文必须保留"
    );
    assert_eq!(
        session.location().canonical_path(),
        new_location.canonical_path(),
        "正文必须重绑布局当前位置"
    );
    assert_eq!(
        session.external_conflict().map(|conflict| conflict.kind()),
        Some(slterm_document::ExternalConflictKind::Modified),
        "磁盘基线不可信时必须标记冲突而不是允许直接保存"
    );
    assert!(recoverer.orphans().is_empty());
}

#[test]
fn integration_session_grant_restore() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let file = temp.path().join("outside.txt");
    let snapshot = snapshot_at('a', &file, "disk", "draft");
    let location = ResolvedPath::new(
        CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
        None,
    );
    let layout = recovered_layout(&root, snapshot.document_id(), location);
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = DocumentSnapshotStore::new(&paths);
    store
        .write(&snapshot, &protected(&[snapshot.document_id()]))
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    let mut recoverer = super::recovery::DocumentSnapshotRecoverer::new(&paths);
    SnapshotRecoverer::recover(&mut recoverer, &layout).unwrap();
    std::fs::write(&file, "changed").unwrap();
    let canonical = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();

    let (location, grant) = recoverer
        .prepare_reauthorize(snapshot.document_id())
        .unwrap();
    assert_eq!(location.canonical_path(), &canonical);
    let prepared = super::recovery::read_reauthorized_document(&location, grant).unwrap();
    let mut registry = recoverer.take_registry();
    recoverer
        .finish_reauthorize(&mut registry, snapshot.document_id(), prepared)
        .unwrap();
    assert!(
        recoverer
            .prepare_reauthorize(snapshot.document_id())
            .is_none(),
        "成功后不得再次重授权"
    );

    let session = registry.session(snapshot.document_id()).unwrap();
    assert!(!session.is_read_revoked());
    assert_eq!(session.text().unwrap().to_string(), "draft");
    assert_eq!(session.disk_baseline().as_ref(), b"changed");
    assert_eq!(
        session.external_conflict().unwrap().kind(),
        slterm_document::ExternalConflictKind::Modified
    );
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
            PagedSource::new(
                PageIndex::new(0, Vec::new()).unwrap(),
                [0; 32],
                PagedLineIndex::from_page_stats(&[]),
            ),
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
