use std::{path::Path, str::FromStr as _};

use slterm_document::RepoPath;
use slterm_path::{CanonicalPath, PathGrant};

use super::{GitError, GitFacade};

fn canonical(path: &Path) -> CanonicalPath {
    CanonicalPath::new(path).unwrap()
}

fn git(repository: &Path, args: &[&str]) -> std::process::Output {
    let output = super::command(repository).args(args).output().unwrap();
    assert!(output.status.success(), "测试 Git 命令必须成功");
    output
}

fn committed_repository(file: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    git(&repository, &["config", "user.name", "slTerminal Test"]);
    git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("tracked.txt"), file).unwrap();
    git(&repository, &["add", "--", "tracked.txt"]);
    git(&repository, &["commit", "--quiet", "-m", "initial"]);
    (temp, repository)
}

#[test]
fn gitshow_fixed_commit_after_head_move() {
    let (_temp, repository) = committed_repository(b"first version\n");
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();
    let binding = facade
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    let path = RepoPath::from_str("tracked.txt").unwrap();
    std::fs::write(repository.join("tracked.txt"), b"second version\n").unwrap();
    git(&repository, &["add", "--", "tracked.txt"]);
    git(&repository, &["commit", "--quiet", "-m", "second"]);

    let first = facade.show(&binding, &path, &grants, 0, 11).unwrap();
    assert_eq!(first.bytes.as_ref(), b"first version\n");
    assert_eq!(first.generation, 11);

    std::fs::write(repository.join("tracked.txt"), b"worktree only\n").unwrap();
    let refreshed = facade.show(&binding, &path, &grants, 0, 12).unwrap();
    assert_eq!(refreshed.bytes.as_ref(), b"first version\n");
    assert_eq!(refreshed.generation, 12);

    let relocated = repository.with_extension("relocated");
    std::fs::rename(&repository, &relocated).unwrap();
    assert!(matches!(
        facade.show(&binding, &path, &grants, 0, 13),
        Err(GitError::BindingUnavailable)
    ));
}

#[test]
fn gitshow_refresh_reads_same_object() {
    let (_temp, repository) = committed_repository(b"fixed\n");
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();
    let binding = facade
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    let path = RepoPath::from_str("tracked.txt").unwrap();
    let first = facade.show(&binding, &path, &grants, 0, 20).unwrap();

    std::fs::write(repository.join("tracked.txt"), b"worktree\n").unwrap();
    let refreshed = facade.show(&binding, &path, &grants, 0, 21).unwrap();

    assert_eq!(refreshed.bytes, first.bytes);
    assert_eq!(refreshed.blob, first.blob);
    assert_eq!(refreshed.commit, first.commit);
}

#[test]
fn gitshow_missing_object_preserves_error() {
    let (_temp, repository) = committed_repository(b"fixed object\n");
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();
    let binding = facade
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    let path = RepoPath::from_str("tracked.txt").unwrap();
    let output = git(&repository, &["ls-tree", "HEAD", "--", "tracked.txt"]);
    let metadata = std::str::from_utf8(&output.stdout).unwrap();
    let object = metadata.split_whitespace().nth(2).unwrap();
    let object_path = repository
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..]);
    let preserved = object_path.with_extension("preserved");
    std::fs::rename(&object_path, &preserved).unwrap();
    std::fs::write(repository.join("tracked.txt"), b"must not fallback\n").unwrap();

    assert!(matches!(
        facade.show(&binding, &path, &grants, 0, 13),
        Err(GitError::ObjectUnavailable)
    ));
    assert!(preserved.is_file());
}
