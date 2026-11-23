use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use slterm_path::{CanonicalPath, PathGrant};
use slterm_sidebar::{
    FileClipboard, FileClipboardItem, FileClipboardMode, FileTreeEntryKind, FileTreeRoot,
    PasteTarget,
};
use slterm_workspace::ProjectId;

use super::{
    file_operation_service::{FileOperationService, RecycleBin},
    paste_service::{PasteConflictDecision, PasteItemOutcome, PasteService},
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
    FileClipboard::new(
        FileClipboardMode::Copy,
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
    (PasteService::new(operations), recycle_bin)
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
