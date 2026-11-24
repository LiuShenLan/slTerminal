//! Runtime Git diff 读取：真实临时仓库上的 HEAD 与工作区差异。

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use slterm_path::CanonicalPath;

use super::git::git_diff_snapshot;
use crate::app_shell::RuntimeGitDiffTarget;
use crate::git::GitFacade;

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("必须可运行 git");
    assert!(status.success(), "git {args:?} 必须成功");
}

/// 锚：HEAD 与工作区差异只含变更行，revision 随正文变化。
#[test]
fn git_diff_snapshot_reads_head_against_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "--quiet"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "test"]);
    std::fs::write(root.join("a.txt"), "one\n").unwrap();
    git(&root, &["add", "a.txt"]);
    git(&root, &["commit", "--quiet", "-m", "init"]);
    std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();

    let target = RuntimeGitDiffTarget {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        root: CanonicalPath::new(&root).unwrap(),
        path: "a.txt".to_owned(),
        git: Arc::new(GitFacade::default()),
    };
    let snapshot = git_diff_snapshot(&target).expect("diff 必须成功");
    assert!(
        snapshot.text.contains("+two"),
        "新增行必须带 + 前缀: {}",
        snapshot.text
    );
    assert!(
        !snapshot.text.contains("-one"),
        "未变化行不得输出: {}",
        snapshot.text
    );

    // 同一内容重算必须得到同一 revision，供游标续读。
    let again = git_diff_snapshot(&target).expect("diff 必须成功");
    assert_eq!(snapshot.revision, again.revision);

    // 未跟踪文件视为全文新增。
    std::fs::write(root.join("b.txt"), "fresh\n").unwrap();
    let untracked = RuntimeGitDiffTarget {
        path: "b.txt".to_owned(),
        ..target
    };
    let snapshot = git_diff_snapshot(&untracked).expect("未跟踪文件必须可比较");
    assert!(snapshot.text.contains("+fresh"), "{}", snapshot.text);
}
