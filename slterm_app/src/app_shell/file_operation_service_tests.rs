use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use slterm_path::{CanonicalPath, PathGrant};
use slterm_sidebar::FileTreeRoot;
use slterm_workspace::ProjectId;

use super::file_operation_service::{
    FileOperationError, FileOperationOutcome, FileOperationService, RecycleBin,
};

struct TestRecycleBin {
    paths: Mutex<Vec<PathBuf>>,
}

impl TestRecycleBin {
    fn new() -> Self {
        Self {
            paths: Mutex::new(Vec::new()),
        }
    }

    fn paths(&self) -> Vec<PathBuf> {
        self.paths.lock().unwrap().clone()
    }
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

fn service() -> (FileOperationService, Arc<TestRecycleBin>) {
    let recycle_bin = Arc::new(TestRecycleBin::new());
    (
        FileOperationService::with_recycle_bin_for_test(recycle_bin.clone()),
        recycle_bin,
    )
}

fn create_junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "Windows 必须能创建临时目录 junction");
}

#[test]
fn operation_plan_executes_single_create_rename_and_recycle_delete() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let root = root();
    let (service, recycle_bin) = service();

    let file = canonical(temp.path().join("new.txt"));
    let plan = service
        .plan_create_file(root.clone(), &file, &grants, 0)
        .unwrap();
    assert_eq!(
        service.execute(&plan, &grants, 0).unwrap(),
        FileOperationOutcome::Completed
    );
    assert_eq!(std::fs::read(file.as_path()).unwrap(), b"");

    let directory = canonical(temp.path().join("empty"));
    let plan = service
        .plan_create_directory(root.clone(), &directory, &grants, 0)
        .unwrap();
    service.execute(&plan, &grants, 0).unwrap();
    assert!(directory.as_path().is_dir());
    assert_eq!(std::fs::read_dir(directory.as_path()).unwrap().count(), 0);

    let renamed = canonical(temp.path().join("renamed.txt"));
    let plan = service
        .plan_rename(root.clone(), &file, &renamed, &grants, 0)
        .unwrap();
    service.execute(&plan, &grants, 0).unwrap();
    assert!(!file.as_path().exists());
    assert!(renamed.as_path().is_file());

    std::fs::write(renamed.as_path(), b"delete me").unwrap();
    let plan = service.plan_delete(root, &renamed, &grants, 0).unwrap();
    service.execute(&plan, &grants, 0).unwrap();
    assert!(!renamed.as_path().exists());
    assert_eq!(recycle_bin.paths(), vec![renamed.as_path().to_path_buf()]);
}

#[test]
fn operation_plan_rejects_replaced_create_parent() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let parent_path = temp.path().join("parent");
    std::fs::create_dir(&parent_path).unwrap();
    let target = canonical(parent_path.join("new.txt"));
    let (service, _) = service();
    let plan = service
        .plan_create_file(root(), &target, &grants, 0)
        .unwrap();

    std::fs::remove_dir(&parent_path).unwrap();
    std::fs::create_dir(&parent_path).unwrap();

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Changed)
    ));
    assert!(!target.as_path().exists());
}

#[test]
fn operation_plan_rejects_replaced_rename_destination_parent() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let source = canonical(temp.path().join("source.txt"));
    std::fs::write(source.as_path(), b"source").unwrap();
    let parent_path = temp.path().join("destination");
    std::fs::create_dir(&parent_path).unwrap();
    let destination = canonical(parent_path.join("renamed.txt"));
    let (service, _) = service();
    let plan = service
        .plan_rename(root(), &source, &destination, &grants, 0)
        .unwrap();

    std::fs::remove_dir(&parent_path).unwrap();
    std::fs::create_dir(&parent_path).unwrap();

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Changed)
    ));
    assert_eq!(std::fs::read(source.as_path()).unwrap(), b"source");
    assert!(!destination.as_path().exists());
}

#[test]
fn operation_plan_rejects_replaced_file_identity_without_mutating_it() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let file = canonical(temp.path().join("victim.txt"));
    std::fs::write(file.as_path(), b"original").unwrap();
    let modified = std::fs::metadata(file.as_path())
        .unwrap()
        .modified()
        .unwrap();
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &file, &grants, 0).unwrap();

    std::fs::remove_file(file.as_path()).unwrap();
    std::fs::write(file.as_path(), b"original").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(file.as_path())
        .unwrap()
        .set_modified(modified)
        .unwrap();

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Changed)
    ));
    assert_eq!(std::fs::read(file.as_path()).unwrap(), b"original");
    assert!(recycle_bin.paths().is_empty());
}

#[test]
fn operation_plan_rejects_content_change_without_identity_change() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let file = canonical(temp.path().join("victim.txt"));
    std::fs::write(file.as_path(), b"original").unwrap();
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &file, &grants, 0).unwrap();

    std::fs::write(file.as_path(), b"changed").unwrap();

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Changed)
    ));
    assert_eq!(std::fs::read(file.as_path()).unwrap(), b"changed");
    assert!(recycle_bin.paths().is_empty());
}

#[test]
fn operation_plan_treats_externally_deleted_target_as_completed() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let file = canonical(temp.path().join("victim.txt"));
    std::fs::write(file.as_path(), b"original").unwrap();
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &file, &grants, 0).unwrap();

    std::fs::remove_file(file.as_path()).unwrap();

    assert_eq!(
        service.execute(&plan, &grants, 0).unwrap(),
        FileOperationOutcome::AlreadyAbsent
    );
    assert!(recycle_bin.paths().is_empty());
}

#[test]
fn operation_plan_rejects_dangling_reparse_point_as_still_present() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let original_path = project_path.join("original");
    std::fs::create_dir(&project_path).unwrap();
    std::fs::create_dir(&original_path).unwrap();
    let link_path = project_path.join("linked");
    create_junction(&link_path, &original_path);

    let project = canonical(&project_path);
    let grants = vec![PathGrant::project_root(project)];
    let link = canonical(&link_path);
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &link, &grants, 0).unwrap();

    std::fs::remove_dir(&link_path).unwrap();
    create_junction(&link_path, &temp.path().join("missing"));

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Changed)
    ));
    assert!(std::fs::symlink_metadata(&link_path).is_ok());
    assert!(recycle_bin.paths().is_empty());
}

#[test]
fn operation_plan_rejects_stale_authorization_generation() {
    let temp = tempfile::tempdir().unwrap();
    let project = canonical(temp.path());
    let grants = vec![PathGrant::project_root(project)];
    let file = canonical(temp.path().join("victim.txt"));
    std::fs::write(file.as_path(), b"keep").unwrap();
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &file, &grants, 0).unwrap();

    assert!(matches!(
        service.execute(&plan, &grants, 1),
        Err(FileOperationError::Unauthorized)
    ));
    assert_eq!(std::fs::read(file.as_path()).unwrap(), b"keep");
    assert!(recycle_bin.paths().is_empty());
}

#[test]
fn explorer_rechecks_links_before_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    let original_path = project_path.join("original");
    let outside_path = temp.path().join("outside");
    for directory in [&project_path, &original_path, &outside_path] {
        std::fs::create_dir(directory).unwrap();
    }
    std::fs::write(original_path.join("original.txt"), b"original").unwrap();
    std::fs::write(outside_path.join("outside.txt"), b"outside").unwrap();
    let link_path = project_path.join("linked");
    create_junction(&link_path, &original_path);

    let project = canonical(&project_path);
    let grants = vec![PathGrant::project_root(project)];
    let link = canonical(&link_path);
    let (service, recycle_bin) = service();
    let plan = service.plan_delete(root(), &link, &grants, 0).unwrap();

    std::fs::remove_dir(&link_path).unwrap();
    create_junction(&link_path, &outside_path);

    assert!(matches!(
        service.execute(&plan, &grants, 0),
        Err(FileOperationError::Unauthorized)
    ));
    assert!(outside_path.join("outside.txt").is_file());
    assert!(original_path.join("original.txt").is_file());
    assert!(recycle_bin.paths().is_empty());
}
