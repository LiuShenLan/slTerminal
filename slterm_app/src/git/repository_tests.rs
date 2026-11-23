use std::{path::Path, str::FromStr};

use slterm_document::CommitId;
use slterm_path::{CanonicalPath, PathGrant};

use super::{GitError, GitFacade};

fn canonical(path: impl AsRef<Path>) -> CanonicalPath {
    CanonicalPath::new(path.as_ref()).unwrap()
}

fn git(repository: &Path, args: &[&str]) -> std::process::Output {
    super::command(repository).args(args).output().unwrap()
}

fn init_repository(repository: &Path) {
    std::fs::create_dir_all(repository).unwrap();
    assert!(git(repository, &["init", "--quiet"]).status.success());
    assert!(
        git(repository, &["config", "user.name", "slTerminal Test"])
            .status
            .success()
    );
    assert!(
        git(
            repository,
            &["config", "user.email", "slterm-test@example.invalid"]
        )
        .status
        .success()
    );
}

fn commit_file(repository: &Path, contents: &[u8]) {
    std::fs::write(repository.join("tracked.txt"), contents).unwrap();
    assert!(
        git(repository, &["add", "--", "tracked.txt"])
            .status
            .success()
    );
    assert!(
        git(repository, &["commit", "--quiet", "-m", "test"])
            .status
            .success()
    );
}

fn head(repository: &Path) -> CommitId {
    let output = git(repository, &["rev-parse", "HEAD"]);
    assert!(output.status.success());
    CommitId::from_str(std::str::from_utf8(&output.stdout).unwrap().trim()).unwrap()
}

#[test]
fn git_facade_captures_and_revalidates_real_repository_binding() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    init_repository(&repository);
    commit_file(&repository, b"first");
    let project = repository.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();

    let original = facade
        .repository_binding(&canonical(&project), &grants, 0)
        .unwrap();
    assert_eq!(original.head(), &head(&repository));
    assert_eq!(
        original.worktree_root().canonical_path().as_path(),
        dunce::canonicalize(&repository).unwrap()
    );
    assert!(original.git_dir().file_identity().is_some());
    assert!(original.common_dir().file_identity().is_some());

    let original_head = original.head().clone();
    commit_file(&repository, b"second");
    let current = facade
        .repository_binding(&canonical(&project), &grants, 0)
        .unwrap();
    assert_ne!(current.head(), original.head());
    facade.revalidate_binding(&original, &grants, 0).unwrap();

    let relocated = temp.path().join("relocated");
    std::fs::rename(&repository, &relocated).unwrap();
    assert!(matches!(
        facade.revalidate_binding(&original, &grants, 0),
        Err(GitError::BindingUnavailable)
    ));
    assert_eq!(original.head(), &original_head);
}

#[test]
fn git_facade_repository_capture_never_initializes_a_non_repository() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("plain");
    std::fs::create_dir(&directory).unwrap();
    let grants = vec![PathGrant::repo_root(canonical(&directory))];
    let facade = GitFacade::default();

    assert!(matches!(
        facade.repository_binding(&canonical(&directory), &grants, 0),
        Err(GitError::NotRepository(_))
    ));
    assert!(!directory.join(".git").exists());
}
