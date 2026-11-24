use std::path::Path;

use slterm_path::{CanonicalPath, PathGrant};

use super::{GitFacade, file_status_from_snapshot, parse_status, status};

fn git(repository: &Path, args: &[&str]) {
    let output = super::command(repository).args(args).output().unwrap();
    assert!(output.status.success(), "测试 Git 命令必须成功");
}

#[test]
fn status_revision_hashes_the_exact_porcelain_bytes() {
    let base = "# branch.oid abc\0# branch.head main\0? file.txt\0";
    let with_extra_header = "# branch.oid abc\0# branch.head main\0# custom value\0? file.txt\0";

    let first = parse_status(base).unwrap();
    let repeated = parse_status(base).unwrap();
    let changed = parse_status(with_extra_header).unwrap();

    assert_eq!(first.revision, repeated.revision);
    assert_ne!(first.revision, changed.revision);
}

#[test]
fn status_parser_preserves_ignored_and_unknown_raw_facts() {
    let snapshot = parse_status(
        "# branch.oid abc\0# branch.head main\0\
         ! generated/ignored.txt\0\
         1 Z? N... 100644 100644 100644 abc def strange.txt\0",
    )
    .unwrap();

    assert!(snapshot.entries[0].ignored);
    assert_eq!(snapshot.entries[0].path, "generated/ignored.txt");
    assert_eq!(snapshot.entries[0].index, '!');
    assert_eq!(snapshot.entries[0].worktree, '!');
    assert!(!snapshot.entries[1].ignored);
    assert_eq!(snapshot.entries[1].index, 'Z');
    assert_eq!(snapshot.entries[1].worktree, '?');
    assert_eq!(
        file_status_from_snapshot(&snapshot, "generated/ignored.txt"),
        None
    );
}

fn repository_with_commit(repository: &Path) {
    std::fs::create_dir_all(repository).unwrap();
    git(repository, &["init", "--quiet"]);
    git(repository, &["config", "user.name", "slTerminal Test"]);
    git(
        repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("tracked.txt"), b"initial").unwrap();
    git(repository, &["add", "--", "tracked.txt"]);
    git(repository, &["commit", "--quiet", "-m", "initial"]);
}

#[test]
fn status_keeps_nested_repositories_and_submodules_at_the_top_level() {
    let temp = tempfile::tempdir().unwrap();
    let outer = temp.path().join("outer");
    let nested = outer.join("nested");
    let child = temp.path().join("child");
    repository_with_commit(&outer);
    repository_with_commit(&nested);
    repository_with_commit(&child);
    let child_text = child.to_string_lossy().into_owned();
    git(
        &outer,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            &child_text,
            "module",
        ],
    );
    git(&outer, &["commit", "--quiet", "-m", "submodule"]);
    std::fs::write(child.join("tracked.txt"), b"external").unwrap();
    std::fs::write(outer.join("module/tracked.txt"), b"changed").unwrap();

    let snapshot = status(&outer).unwrap();

    assert!(
        snapshot
            .entries
            .iter()
            .any(|entry| entry.path == "nested/" && entry.index == '?')
    );
    assert!(
        snapshot
            .entries
            .iter()
            .any(|entry| entry.path == "module" && entry.worktree == 'M')
    );
    assert!(
        snapshot
            .entries
            .iter()
            .all(|entry| !entry.path.starts_with("nested/") || entry.path == "nested/")
    );
    assert!(
        snapshot
            .entries
            .iter()
            .all(|entry| !entry.path.starts_with("module/"))
    );
}

#[test]
fn status_captures_ignored_entries_and_changes_status_revision() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path();
    git(repository, &["init", "--quiet"]);
    git(repository, &["config", "user.name", "slTerminal Test"]);
    git(
        repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("tracked.txt"), b"first").unwrap();
    std::fs::write(repository.join(".gitignore"), b"ignored.txt\n").unwrap();
    git(repository, &["add", "--", "tracked.txt", ".gitignore"]);
    git(repository, &["commit", "--quiet", "-m", "initial"]);
    std::fs::write(repository.join("tracked.txt"), b"second").unwrap();
    std::fs::write(repository.join("ignored.txt"), b"ignored").unwrap();

    let first = status(repository).unwrap();
    assert!(
        first
            .entries
            .iter()
            .any(|entry| entry.ignored && entry.path == "ignored.txt")
    );
    std::fs::write(repository.join("new.txt"), b"new").unwrap();
    let second = status(repository).unwrap();
    assert_ne!(first.revision, second.revision);
}

#[test]
fn git_facade_status_binds_repo_root_and_uses_invalidation_generation() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    repository_with_commit(&repository);
    let project = repository.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(repository.join("tracked.txt"), b"changed").unwrap();
    let grants = vec![PathGrant::repo_root(
        CanonicalPath::new(&repository).unwrap(),
    )];
    let facade = GitFacade::default();

    let first = facade
        .status(&CanonicalPath::new(&project).unwrap(), &grants, 0)
        .unwrap();
    assert_eq!(
        first
            .worktree_root
            .as_ref()
            .unwrap()
            .canonical_path()
            .as_path(),
        dunce::canonicalize(&repository).unwrap()
    );
    std::fs::write(repository.join("new.txt"), b"new").unwrap();
    let cached = facade
        .status(&CanonicalPath::new(&project).unwrap(), &grants, 0)
        .unwrap();
    assert_eq!(cached.revision, first.revision);

    facade.invalidate();
    let refreshed = facade
        .status(&CanonicalPath::new(&project).unwrap(), &grants, 0)
        .unwrap();
    assert_ne!(refreshed.revision, first.revision);
}

#[test]
fn git_facade_status_supports_an_unborn_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("unborn");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    std::fs::write(repository.join("new.txt"), b"new").unwrap();
    let grants = vec![PathGrant::repo_root(
        CanonicalPath::new(&repository).unwrap(),
    )];

    let snapshot = GitFacade::default()
        .status(&CanonicalPath::new(&repository).unwrap(), &grants, 0)
        .unwrap();

    assert!(snapshot.entries.iter().any(|entry| entry.path == "new.txt"));
    assert!(snapshot.worktree_root.is_some());
}

/// 回滚只从当前索引恢复工作区，不把索引重置到 HEAD。
#[test]
fn git_restore_from_index_keeps_staged_changes() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    git(&repository, &["config", "core.autocrlf", "false"]);
    git(&repository, &["config", "user.name", "slTerminal Test"]);
    git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("tracked.txt"), b"base\n").unwrap();
    git(&repository, &["add", "--", "tracked.txt"]);
    git(&repository, &["commit", "--quiet", "-m", "base"]);
    std::fs::write(repository.join("tracked.txt"), b"staged\n").unwrap();
    git(&repository, &["add", "--", "tracked.txt"]);
    std::fs::write(repository.join("tracked.txt"), b"worktree\n").unwrap();

    super::restore_from_index(&repository, &["tracked.txt"]).unwrap();

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "staged\n",
        "工作区必须匹配当前索引"
    );
    let staged = super::command(&repository)
        .args(["diff", "--cached", "--stat"])
        .output()
        .unwrap();
    assert!(!staged.stdout.is_empty(), "staged 差异必须保留");
    let unstaged = super::command(&repository)
        .args(["diff", "--stat"])
        .output()
        .unwrap();
    assert!(unstaged.stdout.is_empty(), "未暂存差异必须被放弃");
}
