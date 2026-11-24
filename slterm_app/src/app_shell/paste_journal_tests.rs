use std::collections::BTreeMap;

use slterm_path::{AppPaths, CanonicalPath};

use super::{
    paste_journal::PasteJournalRecord,
    paste_journal::{
        PASTE_JOURNAL_KIND, PasteJournal as _, PasteJournalRecoverer, PasteOperationKind,
        PastePath, PasteSnapshot, outcome_fingerprint,
    },
};
use crate::{
    persistence::{
        journal::{JournalRegistration, JournalStatus},
        metadata::RunMarker,
        startup::{JournalRecoverer, MaintenanceError, MaintenanceOutcome, MaintenanceRunner},
        workspace::WorkspaceDatabase,
        workspace_writer::WorkspaceWriter,
    },
    platform::document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver},
};

const EPOCH: &str = "epoch-paste-journal";

fn paths(temp: &tempfile::TempDir) -> AppPaths {
    AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
}

fn open(temp: &tempfile::TempDir) -> WorkspaceDatabase {
    let mut store = WorkspaceDatabase::open(&paths(temp)).unwrap();
    store
        .begin_run(
            EPOCH,
            RunMarker {
                run_id: uuid::Uuid::now_v7().to_string(),
                process_id: std::process::id(),
                process_created_at_ms: 1,
            },
        )
        .unwrap();
    store
}

fn snapshot(path: &std::path::Path) -> PasteSnapshot {
    let lexical_path = CanonicalPath::new(path).unwrap();
    let resolved = SystemDocumentPathResolver.resolve(&lexical_path).unwrap();
    PasteSnapshot {
        lexical_path,
        resolved,
        fingerprint: [9; 32],
        authorization_generation: 0,
    }
}

fn destination(path: &std::path::Path) -> PastePath {
    let lexical_path = CanonicalPath::new(path).unwrap();
    let canonical_path = CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap();
    PastePath {
        lexical_path,
        canonical_path,
    }
}

fn register(
    store: &mut WorkspaceDatabase,
    journal_id: &str,
    operation: PasteOperationKind,
    source: &PasteSnapshot,
    destination: &PastePath,
) {
    let registration =
        PasteJournalRecord::registration(journal_id, operation, source, destination, None, EPOCH)
            .unwrap();
    store.register_journal(registration).unwrap();
    store
        .transition_journal(journal_id, JournalStatus::Applying, None)
        .unwrap();
}

fn try_recover(store: &mut WorkspaceDatabase) -> Result<MaintenanceOutcome, MaintenanceError> {
    let recoverer = PasteJournalRecoverer;
    let recoverers = BTreeMap::from([(PASTE_JOURNAL_KIND, &recoverer as &dyn JournalRecoverer)]);
    let requests = crate::persistence::startup::MaintenanceRequestRecovererRegistry::default();
    MaintenanceRunner::new(store, EPOCH, &recoverers, &requests).run()
}

fn recover(store: &mut WorkspaceDatabase) -> MaintenanceOutcome {
    try_recover(store).unwrap()
}

#[test]
fn paste_journal_drop_reopen_reconciles_commit_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"source").unwrap();
    std::fs::write(&destination_path, b"copied").unwrap();
    let source = snapshot(&source_path);
    let destination = destination(&destination_path);

    let mut store = open(&temp);
    register(
        &mut store,
        "paste-recorded",
        PasteOperationKind::Copy,
        &source,
        &destination,
    );
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    store
        .record_step_after("paste-recorded", "paste-recorded", after)
        .unwrap();
    drop(store);

    let mut store = open(&temp);
    assert_eq!(recover(&mut store), MaintenanceOutcome::Ready);
    assert_eq!(
        store.journal("paste-recorded").unwrap().unwrap().status,
        JournalStatus::Committed
    );
    drop(store);

    let mut store = open(&temp);
    register(
        &mut store,
        "paste-unknown",
        PasteOperationKind::Copy,
        &source,
        &destination,
    );
    drop(store);

    let mut store = open(&temp);
    assert!(matches!(
        recover(&mut store),
        MaintenanceOutcome::NeedsAttention { .. }
    ));
    assert_eq!(
        store.journal("paste-unknown").unwrap().unwrap().status,
        JournalStatus::NeedsAttention
    );
    assert_eq!(std::fs::read(&destination_path).unwrap(), b"copied");
}

#[test]
fn paste_journal_recovers_proven_move_without_replaying_it() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"moved").unwrap();
    let source = snapshot(&source_path);
    std::fs::rename(&source_path, &destination_path).unwrap();
    let destination = destination(&destination_path);

    let mut store = open(&temp);
    register(
        &mut store,
        "paste-moved",
        PasteOperationKind::Cut,
        &source,
        &destination,
    );
    drop(store);

    let mut store = open(&temp);
    assert_eq!(recover(&mut store), MaintenanceOutcome::Ready);
    assert_eq!(
        store.journal("paste-moved").unwrap().unwrap().status,
        JournalStatus::Committed
    );
    assert!(!source_path.exists());
    assert_eq!(std::fs::read(&destination_path).unwrap(), b"moved");
}

#[test]
fn paste_journal_prepared_without_side_effects_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"source").unwrap();
    let source = snapshot(&source_path);
    let destination = PastePath {
        lexical_path: CanonicalPath::new(&destination_path).unwrap(),
        canonical_path: CanonicalPath::new(&destination_path).unwrap(),
    };
    let mut store = open(&temp);
    let registration: JournalRegistration = PasteJournalRecord::registration(
        "paste-prepared",
        PasteOperationKind::Copy,
        &source,
        &destination,
        None,
        EPOCH,
    )
    .unwrap();
    store.register_journal(registration).unwrap();
    drop(store);

    let mut store = open(&temp);
    assert_eq!(recover(&mut store), MaintenanceOutcome::Ready);
    assert_eq!(
        store.journal("paste-prepared").unwrap().unwrap().status,
        JournalStatus::Failed
    );
    assert!(!destination_path.exists());
}

#[test]
fn paste_journal_recorded_outcome_must_match_current_disk() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"source").unwrap();
    std::fs::write(&destination_path, b"before").unwrap();
    let source = snapshot(&source_path);
    let destination = destination(&destination_path);
    let mut store = open(&temp);
    register(
        &mut store,
        "paste-changed",
        PasteOperationKind::Copy,
        &source,
        &destination,
    );
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    store
        .record_step_after("paste-changed", "paste-changed", after)
        .unwrap();
    std::fs::write(&destination_path, b"external").unwrap();
    drop(store);

    let mut store = open(&temp);
    assert!(matches!(
        recover(&mut store),
        MaintenanceOutcome::NeedsAttention { .. }
    ));
    assert_eq!(
        store.journal("paste-changed").unwrap().unwrap().status,
        JournalStatus::NeedsAttention
    );
    assert_eq!(std::fs::read(&destination_path).unwrap(), b"external");
}

#[test]
fn paste_journal_port_uses_the_single_workspace_writer() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"source").unwrap();
    std::fs::write(&destination_path, b"copied").unwrap();
    let source = snapshot(&source_path);
    let destination = destination(&destination_path);
    drop(open(&temp));
    let writer = WorkspaceWriter::start(paths(&temp)).unwrap();
    let client = writer.client();
    let registration = PasteJournalRecord::registration(
        "paste-writer",
        PasteOperationKind::Copy,
        &source,
        &destination,
        None,
        EPOCH,
    )
    .unwrap();

    client.begin(registration).unwrap();
    client.mark_applying("paste-writer").unwrap();
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    client
        .record_after("paste-writer", "paste-writer", after)
        .unwrap();
    client.commit("paste-writer", after).unwrap();
    writer.shutdown().unwrap();

    let store = open(&temp);
    assert_eq!(
        store.journal("paste-writer").unwrap().unwrap().status,
        JournalStatus::Committed
    );
}

#[test]
fn paste_journal_recorded_move_still_requires_source_absence() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"moved").unwrap();
    let source = snapshot(&source_path);
    std::fs::rename(&source_path, &destination_path).unwrap();
    let destination = destination(&destination_path);
    let mut store = open(&temp);
    register(
        &mut store,
        "paste-move-recreated",
        PasteOperationKind::Cut,
        &source,
        &destination,
    );
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    store
        .record_step_after("paste-move-recreated", "paste-move-recreated", after)
        .unwrap();
    std::fs::write(&source_path, b"recreated").unwrap();
    drop(store);

    let mut store = open(&temp);
    assert!(matches!(
        recover(&mut store),
        MaintenanceOutcome::NeedsAttention { .. }
    ));
    assert_eq!(std::fs::read(&destination_path).unwrap(), b"moved");
    assert_eq!(std::fs::read(&source_path).unwrap(), b"recreated");
}

#[test]
fn paste_journal_move_rejects_a_reparse_destination() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source");
    let moved_path = temp.path().join("moved");
    let destination_path = temp.path().join("destination");
    std::fs::create_dir(&source_path).unwrap();
    std::fs::write(source_path.join("file.txt"), b"moved").unwrap();
    let source = snapshot(&source_path);
    std::fs::rename(&source_path, &moved_path).unwrap();
    let status = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&destination_path)
        .arg(&moved_path)
        .status()
        .unwrap();
    assert!(status.success());
    let destination = destination(&destination_path);
    let mut store = open(&temp);
    register(
        &mut store,
        "paste-move-link",
        PasteOperationKind::Cut,
        &source,
        &destination,
    );
    drop(store);

    let mut store = open(&temp);
    assert!(matches!(
        recover(&mut store),
        MaintenanceOutcome::NeedsAttention { .. }
    ));
    assert_eq!(
        std::fs::read(moved_path.join("file.txt")).unwrap(),
        b"moved"
    );
}

#[test]
fn paste_journal_directory_outcome_covers_nested_content() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination");
    std::fs::write(&source_path, b"source").unwrap();
    std::fs::create_dir_all(destination_path.join("nested")).unwrap();
    std::fs::write(destination_path.join("nested/file.txt"), b"before").unwrap();
    let source = snapshot(&source_path);
    let destination = destination(&destination_path);
    let root_modified = std::fs::metadata(&destination_path)
        .unwrap()
        .modified()
        .unwrap();
    let mut store = open(&temp);
    register(
        &mut store,
        "paste-directory-changed",
        PasteOperationKind::Copy,
        &source,
        &destination,
    );
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    store
        .record_step_after("paste-directory-changed", "paste-directory-changed", after)
        .unwrap();
    std::fs::write(destination_path.join("nested/file.txt"), b"external").unwrap();
    assert_eq!(
        std::fs::metadata(&destination_path)
            .unwrap()
            .modified()
            .unwrap(),
        root_modified
    );
    drop(store);

    let mut store = open(&temp);
    assert!(matches!(
        recover(&mut store),
        MaintenanceOutcome::NeedsAttention { .. }
    ));
    assert_eq!(
        std::fs::read(destination_path.join("nested/file.txt")).unwrap(),
        b"external"
    );
}

#[test]
fn paste_journal_rejects_target_changes_that_break_registration_fingerprint() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.txt");
    let destination_path = temp.path().join("destination.txt");
    std::fs::write(&source_path, b"source").unwrap();
    std::fs::write(&destination_path, b"copied").unwrap();
    let source = snapshot(&source_path);
    let destination = destination(&destination_path);
    let mut store = open(&temp);
    register(
        &mut store,
        "paste-tampered",
        PasteOperationKind::Copy,
        &source,
        &destination,
    );
    let after = outcome_fingerprint(destination.lexical_path.as_path()).unwrap();
    store
        .record_step_after("paste-tampered", "paste-tampered", after)
        .unwrap();
    drop(store);

    let connection = rusqlite::Connection::open(paths(&temp).workspace_db_path()).unwrap();
    let mut target: serde_json::Value = connection
        .query_row(
            "SELECT target_json FROM journal_steps WHERE journal_id = 'paste-tampered'",
            [],
            |row| row.get(0),
        )
        .map(|value: String| serde_json::from_str(&value).unwrap())
        .unwrap();
    target["destination"]["canonical"] = serde_json::Value::String("D:/other".into());
    connection
        .execute(
            "UPDATE journal_steps SET target_json = ?1 WHERE journal_id = 'paste-tampered'",
            [serde_json::to_string(&target).unwrap()],
        )
        .unwrap();
    drop(connection);

    let mut store = open(&temp);
    assert!(matches!(
        try_recover(&mut store),
        Err(MaintenanceError::Journal(
            crate::persistence::journal::JournalError::CorruptRecord
        ))
    ));
}
