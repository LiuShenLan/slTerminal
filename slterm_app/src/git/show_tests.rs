use std::{path::Path, str::FromStr as _};

use slterm_document::{PAGE_SOURCE_BYTES, RepoPath};
use slterm_path::{CanonicalPath, PathGrant};

use super::{GitError, GitFacade, GitShowContent, GitShowSnapshot};

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
    git(&repository, &["config", "core.autocrlf", "false"]);
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

fn show_committed(file: &[u8]) -> GitShowSnapshot {
    let (_temp, repository) = committed_repository(file);
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();
    let binding = facade
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    facade
        .show(
            &binding,
            &RepoPath::from_str("tracked.txt").unwrap(),
            &grants,
            0,
            1,
        )
        .unwrap()
}

#[test]
fn gitshow_large_and_binary_degradation() {
    let regular = show_committed(&vec![b'x'; 1_000_000]);
    assert!(matches!(regular.content(), GitShowContent::Text { .. }));
    assert!(!regular.requires_size_notice());

    let noticed = show_committed(&vec![b'x'; 1_000_001]);
    assert!(matches!(noticed.content(), GitShowContent::Text { .. }));
    assert!(noticed.requires_size_notice());

    let largest_inline = show_committed(&vec![b'x'; 10_000_000]);
    assert!(matches!(
        largest_inline.content(),
        GitShowContent::Text { .. }
    ));
    assert!(largest_inline.requires_size_notice());

    let paged = show_committed(&vec![b'x'; 10_000_001]);
    let source = match paged.content() {
        GitShowContent::Paged(source) => source,
        _ => panic!("大文本必须分页"),
    };
    assert!(source.index().len() > 1);
    let restored = (0..source.index().len())
        .map(|page| paged.page_text(page).unwrap())
        .collect::<String>();
    assert_eq!(restored.len(), 10_000_001);
    assert!(restored.bytes().all(|byte| byte == b'x'));

    let (_temp, repository) = committed_repository(b"before\0after");
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let facade = GitFacade::default();
    let binding = facade
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    let binary = facade
        .show(
            &binding,
            &RepoPath::from_str("tracked.txt").unwrap(),
            &grants,
            0,
            1,
        )
        .unwrap();
    assert!(matches!(binary.content(), GitShowContent::Binary));
    let transfer_root = tempfile::tempdir().unwrap();
    let opened = super::system_open::open_snapshot_at(&binary, transfer_root.path(), |path| {
        assert_eq!(std::fs::read(path).unwrap(), b"before\0after");
        Ok(())
    })
    .unwrap();
    let transfer_path = opened.path().to_path_buf();
    assert!(!transfer_path.starts_with(&repository));
    assert_eq!(transfer_path.extension().unwrap(), "txt");
    assert!(transfer_path.metadata().unwrap().permissions().readonly());
    drop(opened);
    assert!(!transfer_path.exists());
    assert!(
        git(&repository, &["status", "--porcelain"])
            .stdout
            .is_empty()
    );
    let failed = super::system_open::open_snapshot_at(&binary, transfer_root.path(), |_| {
        Err(std::io::Error::other("injected system opener failure"))
    });
    assert!(failed.is_err());
    assert_eq!(std::fs::read_dir(transfer_root.path()).unwrap().count(), 0);

    let invalid = show_committed(b"\xff");
    assert!(matches!(invalid.content(), GitShowContent::InvalidUtf8));
}

#[test]
fn gitshow_paging_preserves_unicode_and_crlf() {
    let mut text = format!(
        "{}😀\r\nnext\r\n",
        "a".repeat(PAGE_SOURCE_BYTES as usize - 2)
    );
    text.push_str(&"x".repeat(10_000_001 - text.len()));

    let snapshot = show_committed(text.as_bytes());
    let source = match snapshot.content() {
        GitShowContent::Paged(source) => source,
        _ => panic!("大文本必须分页"),
    };
    let restored = (0..source.index().len())
        .map(|page| snapshot.page_text(page).unwrap())
        .collect::<String>();

    assert_eq!(restored, text.replace("\r\n", "\n"));
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
