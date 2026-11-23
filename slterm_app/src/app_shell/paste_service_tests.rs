use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use slterm_path::{AppPaths, CanonicalPath, PathGrant};
use slterm_sidebar::{
    FileClipboard, FileClipboardItem, FileClipboardMode, FileTreeEntryKind, FileTreeRoot,
    PasteTarget,
};
use slterm_workspace::ProjectId;

use super::{
    file_operation_service::{FileOperationError, FileOperationService, RecycleBin},
    paste_journal::{PasteJournal, PasteJournalError},
    paste_service::{PasteConflictDecision, PasteItemOutcome, PasteService},
};
use crate::persistence::{
    journal::{JournalRegistration, JournalStatus},
    metadata::RunMarker,
    workspace::WorkspaceDatabase,
};

struct TestRecycleBin {
    paths: Mutex<Vec<PathBuf>>,
}

impl RecycleBin for TestRecycleBin {
    fn send_to_recycle_bin(&self, path: &Path) -> io::Result<()> {
        self.paths.lock().unwrap().push(path.to_path_buf());
        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    }
}

fn canonical(path: impl Into<PathBuf>) -> CanonicalPath {
    CanonicalPath::new(path).unwrap()
}

fn root() -> FileTreeRoot {
    FileTreeRoot::Project(ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap())
}

fn clipboard(root: FileTreeRoot, directory: &Path, names: &[&str]) -> FileClipboard {
    clipboard_with_mode(FileClipboardMode::Copy, root, directory, names)
}

fn clipboard_with_mode(
    mode: FileClipboardMode,
    root: FileTreeRoot,
    directory: &Path,
    names: &[&str],
) -> FileClipboard {
    FileClipboard::new(
        mode,
        names
            .iter()
            .map(|name| FileClipboardItem {
                root: root.clone(),
                path: canonical(directory.join(name)),
                kind: FileTreeEntryKind::File,
            })
            .collect(),
    )
}

fn service() -> (PasteService, Arc<TestRecycleBin>) {
    let recycle_bin = Arc::new(TestRecycleBin {
        paths: Mutex::new(Vec::new()),
    });
    let operations = FileOperationService::with_recycle_bin_for_test(recycle_bin.clone());
    (PasteService::without_journal(operations), recycle_bin)
}

#[test]
fn explorer_paste_conflict_and_partial_success() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"new a").unwrap();
    std::fs::write(project_path.join("b.txt"), b"new b").unwrap();
    std::fs::write(output_path.join("a.txt"), b"old a").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let (service, recycle_bin) = service();
    let plan =
        clipboard(root.clone(), &project_path, &["a.txt", "b.txt"]).plan_paste(PasteTarget {
            root: root.clone(),
            directory: canonical(&output_path),
        });

    let prepared = service.prepare(&plan, &grants, 0);
    assert_eq!(prepared.conflicts().len(), 1);
    let report = service.execute(prepared, PasteConflictDecision::Cancel, &grants, 0);
    assert!(report.cancelled);
    assert_eq!(std::fs::read(output_path.join("a.txt")).unwrap(), b"old a");
    assert!(!output_path.join("b.txt").exists());
    assert!(recycle_bin.paths.lock().unwrap().is_empty());

    let prepared = service.prepare(&plan, &grants, 0);
    std::fs::remove_file(project_path.join("b.txt")).unwrap();
    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);
    assert!(!report.cancelled);
    assert!(matches!(report.items[0].outcome, PasteItemOutcome::Copied));
    assert!(matches!(
        report.items[1].outcome,
        PasteItemOutcome::Failed(_)
    ));
    assert_eq!(std::fs::read(output_path.join("a.txt")).unwrap(), b"new a");
    assert!(!output_path.join("b.txt").exists());
    assert_eq!(
        recycle_bin.paths.lock().unwrap().as_slice(),
        &[output_path.join("a.txt")]
    );
}

#[test]
fn paste_replace_does_not_recycle_target_when_source_disappears() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    let source_path = project_path.join("a.txt");
    let target_path = output_path.join("a.txt");
    std::fs::write(&source_path, b"new").unwrap();
    std::fs::write(&target_path, b"old").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let (service, recycle_bin) = service();
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    std::fs::remove_file(&source_path).unwrap();
    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);

    assert!(matches!(
        report.items[0].outcome,
        PasteItemOutcome::Failed(_)
    ));
    assert_eq!(std::fs::read(&target_path).unwrap(), b"old");
    assert!(recycle_bin.paths.lock().unwrap().is_empty());
}

#[test]
fn paste_replace_rejects_target_replaced_after_conflict_disclosure() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"new").unwrap();
    let target_path = output_path.join("a.txt");
    std::fs::write(&target_path, b"disclosed").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let (service, recycle_bin) = service();
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    std::fs::remove_file(&target_path).unwrap();
    std::fs::write(&target_path, b"concurrent").unwrap();
    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);

    assert!(matches!(
        report.items[0].outcome,
        PasteItemOutcome::Failed(_)
    ));
    assert_eq!(std::fs::read(&target_path).unwrap(), b"concurrent");
    assert!(recycle_bin.paths.lock().unwrap().is_empty());
}

#[test]
fn paste_replace_does_not_cover_a_new_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"source").unwrap();
    let target_path = output_path.join("a.txt");
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let (service, recycle_bin) = service();
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);
    assert!(prepared.conflicts().is_empty());

    std::fs::write(&target_path, b"new conflict").unwrap();
    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);

    assert!(matches!(
        report.items[0].outcome,
        PasteItemOutcome::Failed(_)
    ));
    assert_eq!(std::fs::read(&target_path).unwrap(), b"new conflict");
    assert!(recycle_bin.paths.lock().unwrap().is_empty());
}

#[test]
fn paste_skip_preserves_conflicts_and_copies_remaining_items() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"new a").unwrap();
    std::fs::write(project_path.join("b.txt"), b"new b").unwrap();
    std::fs::write(output_path.join("a.txt"), b"old a").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let (service, recycle_bin) = service();
    let plan =
        clipboard(root.clone(), &project_path, &["a.txt", "b.txt"]).plan_paste(PasteTarget {
            root,
            directory: canonical(&output_path),
        });

    let prepared = service.prepare(&plan, &grants, 0);
    let report = service.execute(prepared, PasteConflictDecision::Skip, &grants, 0);

    assert!(matches!(report.items[0].outcome, PasteItemOutcome::Skipped));
    assert!(matches!(report.items[1].outcome, PasteItemOutcome::Copied));
    assert_eq!(std::fs::read(output_path.join("a.txt")).unwrap(), b"old a");
    assert_eq!(std::fs::read(output_path.join("b.txt")).unwrap(), b"new b");
    assert!(recycle_bin.paths.lock().unwrap().is_empty());
}

struct DirectJournal {
    store: Mutex<WorkspaceDatabase>,
    registered: Mutex<Vec<String>>,
    fail_begin: bool,
    fail_commit: bool,
}

impl PasteJournal for DirectJournal {
    fn begin(&self, registration: JournalRegistration) -> Result<(), PasteJournalError> {
        if self.fail_begin {
            return Err(PasteJournalError::Unavailable);
        }
        self.registered
            .lock()
            .unwrap()
            .push(registration.journal_id.clone());
        self.store
            .lock()
            .unwrap()
            .register_journal(registration)
            .map(|_| ())
            .map_err(PasteJournalError::Journal)
    }

    fn mark_applying(&self, journal_id: &str) -> Result<(), PasteJournalError> {
        self.store
            .lock()
            .unwrap()
            .transition_journal(journal_id, JournalStatus::Applying, None)
            .map(|_| ())
            .map_err(PasteJournalError::Journal)
    }

    fn record_after(
        &self,
        journal_id: &str,
        step_key: &str,
        after: [u8; 32],
    ) -> Result<(), PasteJournalError> {
        self.store
            .lock()
            .unwrap()
            .record_step_after(journal_id, step_key, after)
            .map(|_| ())
            .map_err(PasteJournalError::Journal)
    }

    fn commit(&self, journal_id: &str, after: [u8; 32]) -> Result<(), PasteJournalError> {
        if self.fail_commit {
            return Err(PasteJournalError::Unavailable);
        }
        let mut store = self.store.lock().unwrap();
        store
            .transition_journal(journal_id, JournalStatus::Committing, None)
            .map_err(PasteJournalError::Journal)?;
        store
            .transition_journal(journal_id, JournalStatus::Committed, Some(after))
            .map(|_| ())
            .map_err(PasteJournalError::Journal)
    }
}

fn direct_journal(
    temp: &tempfile::TempDir,
    fail_begin: bool,
    fail_commit: bool,
) -> Arc<DirectJournal> {
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .begin_run(
            "epoch-paste-service",
            RunMarker {
                run_id: "run-paste-service".into(),
                process_id: 1,
                process_created_at_ms: 1,
            },
        )
        .unwrap();
    Arc::new(DirectJournal {
        store: Mutex::new(store),
        registered: Mutex::new(Vec::new()),
        fail_begin,
        fail_commit,
    })
}

#[test]
fn paste_service_commits_real_journal_after_copy() {
    let temp = tempfile::tempdir().unwrap();
    let journal = direct_journal(&temp, false, false);

    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"content").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let service = PasteService::new(
        FileOperationService::default(),
        journal.clone(),
        "epoch-paste-service".to_owned(),
    );
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 7);

    let report = service.execute(prepared, PasteConflictDecision::Skip, &grants, 7);

    assert!(matches!(report.items[0].outcome, PasteItemOutcome::Copied));
    assert_eq!(
        std::fs::read(output_path.join("a.txt")).unwrap(),
        b"content"
    );
    let journal_id = journal.registered.lock().unwrap()[0].clone();
    let stored = journal
        .store
        .lock()
        .unwrap()
        .journal(&journal_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, JournalStatus::Committed);
    assert_eq!(stored.steps[0].authorization_generation, 7);
}

#[test]
fn paste_journal_begin_failure_prevents_disk_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let journal = direct_journal(&temp, true, false);
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"content").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let service = PasteService::new(
        FileOperationService::default(),
        journal,
        "epoch-paste-service".to_owned(),
    );
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    let report = service.execute(prepared, PasteConflictDecision::Skip, &grants, 0);

    assert!(matches!(
        report.items[0].outcome,
        PasteItemOutcome::Failed(FileOperationError::Journal)
    ));
    assert!(!output_path.join("a.txt").exists());
}

#[test]
fn paste_journal_commit_failure_reports_needs_attention() {
    let temp = tempfile::tempdir().unwrap();
    let journal = direct_journal(&temp, false, true);
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"content").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let service = PasteService::new(
        FileOperationService::default(),
        journal.clone(),
        "epoch-paste-service".to_owned(),
    );
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    let report = service.execute(prepared, PasteConflictDecision::Skip, &grants, 0);

    assert!(matches!(
        report.items[0].outcome,
        PasteItemOutcome::NeedsAttention
    ));
    assert_eq!(
        std::fs::read(output_path.join("a.txt")).unwrap(),
        b"content"
    );
    let journal_id = journal.registered.lock().unwrap()[0].clone();
    assert_eq!(
        journal
            .store
            .lock()
            .unwrap()
            .journal(&journal_id)
            .unwrap()
            .unwrap()
            .status,
        JournalStatus::Applying
    );
}

#[test]
fn paste_service_commits_real_journal_after_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let journal = direct_journal(&temp, false, false);
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"new").unwrap();
    std::fs::write(output_path.join("a.txt"), b"old").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let recycle_bin = Arc::new(TestRecycleBin {
        paths: Mutex::new(Vec::new()),
    });
    let operations = FileOperationService::with_recycle_bin_for_test(recycle_bin.clone());
    let service = PasteService::new(
        operations,
        journal.clone(),
        "epoch-paste-service".to_owned(),
    );
    let plan = clipboard(root.clone(), &project_path, &["a.txt"]).plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);

    assert!(matches!(report.items[0].outcome, PasteItemOutcome::Copied));
    assert_eq!(std::fs::read(output_path.join("a.txt")).unwrap(), b"new");
    assert_eq!(recycle_bin.paths.lock().unwrap().len(), 1);
    let journal_id = journal.registered.lock().unwrap()[0].clone();
    let stored = journal
        .store
        .lock()
        .unwrap()
        .journal(&journal_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, JournalStatus::Committed);
    assert_eq!(stored.steps[0].kind, "replace_copy");
    assert!(stored.steps[0].target["replacement"].is_object());
}

#[test]
fn paste_service_commits_real_journal_for_cut_and_replace_cut() {
    let temp = tempfile::tempdir().unwrap();
    let journal = direct_journal(&temp, false, false);
    let project_path = temp.path().join("project");
    let output_path = project_path.join("out");
    std::fs::create_dir_all(&output_path).unwrap();
    std::fs::write(project_path.join("a.txt"), b"new a").unwrap();
    std::fs::write(project_path.join("b.txt"), b"new b").unwrap();
    std::fs::write(output_path.join("a.txt"), b"old a").unwrap();
    let root = root();
    let grants = vec![PathGrant::project_root(canonical(&project_path))];
    let recycle_bin = Arc::new(TestRecycleBin {
        paths: Mutex::new(Vec::new()),
    });
    let operations = FileOperationService::with_recycle_bin_for_test(recycle_bin);
    let service = PasteService::new(
        operations,
        journal.clone(),
        "epoch-paste-service".to_owned(),
    );
    let plan = clipboard_with_mode(
        FileClipboardMode::Cut,
        root.clone(),
        &project_path,
        &["a.txt", "b.txt"],
    )
    .plan_paste(PasteTarget {
        root,
        directory: canonical(&output_path),
    });
    let prepared = service.prepare(&plan, &grants, 0);

    let report = service.execute(prepared, PasteConflictDecision::Replace, &grants, 0);

    assert!(
        report
            .items
            .iter()
            .all(|item| matches!(item.outcome, PasteItemOutcome::Moved))
    );
    assert!(!project_path.join("a.txt").exists());
    assert!(!project_path.join("b.txt").exists());
    assert_eq!(std::fs::read(output_path.join("a.txt")).unwrap(), b"new a");
    assert_eq!(std::fs::read(output_path.join("b.txt")).unwrap(), b"new b");
    let ids = journal.registered.lock().unwrap().clone();
    assert_eq!(ids.len(), 2);
    for (id, expected_kind) in ids.iter().zip(["replace_cut", "cut"]) {
        let stored = journal.store.lock().unwrap().journal(id).unwrap().unwrap();
        assert_eq!(stored.status, JournalStatus::Committed);
        assert_eq!(stored.steps[0].kind, expected_kind);
    }
}
