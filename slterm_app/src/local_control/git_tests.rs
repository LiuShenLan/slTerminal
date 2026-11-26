//! Runtime Git 读取与破坏写：真实临时仓库上的前置重检与受限写。

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{DomainErrorCode, GitDeleteExpected, GitRollbackExpected};

use super::git::{
    GitOpenIntent, GitWriteIntent, execute_git_open, execute_git_write, git_diff_snapshot,
};
use crate::app_shell::file_operation_service::{FileOperationService, RecycleBin};
use crate::app_shell::{GitWriteTarget, RuntimeGitDiffTarget};
use crate::git::GitFacade;

/// 测试回收站：记录并移走目标文件，不触碰真实系统回收站。
struct TestRecycleBin {
    paths: Mutex<Vec<PathBuf>>,
}

impl RecycleBin for TestRecycleBin {
    fn send_to_recycle_bin(&self, path: &Path) -> io::Result<()> {
        self.paths.lock().unwrap().push(path.to_path_buf());
        std::fs::remove_file(path)
    }
}

fn write_target(root: &Path) -> (GitWriteTarget, Arc<TestRecycleBin>) {
    let recycle = Arc::new(TestRecycleBin {
        paths: Mutex::new(Vec::new()),
    });
    let target = GitWriteTarget {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        root: CanonicalPath::new(root).unwrap(),
        git: Arc::new(GitFacade::default()),
        file_operations: Arc::new(FileOperationService::with_recycle_bin_for_test(
            recycle.clone(),
        )),
    };
    (target, recycle)
}

fn init_repo(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "user.name", "test"]);
}

fn commit_file(root: &Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
    git(root, &["add", "--", name]);
    git(root, &["commit", "--quiet", "-m", "base"]);
}

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

/// 应用层纵深：即使绕过协议校验，工作区读取也不得越过仓库根。
#[test]
fn git_diff_snapshot_rejects_targets_outside_the_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "--quiet"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "test"]);
    std::fs::write(root.join("a.txt"), "one\n").unwrap();
    git(&root, &["add", "a.txt"]);
    git(&root, &["commit", "--quiet", "-m", "init"]);
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, "secret\n").unwrap();

    let target = |path: String| RuntimeGitDiffTarget {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        root: CanonicalPath::new(&root).unwrap(),
        path,
        git: Arc::new(GitFacade::default()),
    };

    // 盘符绝对路径在 Windows 上会替换 join 基路径。
    let absolute = outside.to_string_lossy().replace('\\', "/");
    assert!(
        git_diff_snapshot(&target(absolute)).is_err(),
        "仓库根外绝对路径必须拒绝"
    );

    // 仓库内目录联接指向根外另一个仓库时不得跟随读取其内容。
    let other = temp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    git(&other, &["init", "--quiet"]);
    git(&other, &["config", "user.email", "test@example.com"]);
    git(&other, &["config", "user.name", "test"]);
    std::fs::write(other.join("secret.txt"), "secret\n").unwrap();
    git(&other, &["add", "secret.txt"]);
    git(&other, &["commit", "--quiet", "-m", "other"]);
    let junction = root.join("escape");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&other)
        .status()
        .expect("必须可运行 cmd mklink");
    assert!(status.success(), "目录联接必须可创建");
    assert!(
        git_diff_snapshot(&target("escape/secret.txt".to_owned())).is_err(),
        "经目录联接逃逸仓库根的读取必须拒绝"
    );

    // 正常仓库内路径仍可读取。
    assert!(git_diff_snapshot(&target("a.txt".to_owned())).is_ok());
}

fn rollback_intent(path: &str, expected: GitRollbackExpected) -> GitWriteIntent {
    GitWriteIntent::Rollback {
        path: path.to_owned(),
        expected,
    }
}

fn delete_intent(path: &str, expected: GitDeleteExpected, permanent: bool) -> GitWriteIntent {
    GitWriteIntent::Delete {
        path: path.to_owned(),
        expected,
        permanent,
    }
}

/// 锚：未暂存修改只从索引恢复；状态变化后拒绝写入。
#[test]
fn git_write_rollback_restores_index_and_rechecks_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    init_repo(&root);
    commit_file(&root, "a.txt", "one\n");
    std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
    let (target, _recycle) = write_target(&root);

    execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Modified),
        &target,
    )
    .expect("未暂存修改必须可回滚");
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\n"
    );

    // 工作区已与索引一致：同一前置形态不再成立，必须拒绝且不写盘。
    let error = execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Modified),
        &target,
    )
    .expect_err("状态已变化必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\n"
    );

    // 声明形态与实际不一致时同样拒绝，文件保持修改后内容。
    std::fs::write(root.join("a.txt"), "one\nthree\n").unwrap();
    let error = execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Deleted),
        &target,
    )
    .expect_err("形态不匹配必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\nthree\n"
    );

    // 未暂存删除从索引恢复。
    std::fs::remove_file(root.join("a.txt")).unwrap();
    execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Deleted),
        &target,
    )
    .expect("未暂存删除必须可回滚");
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\n"
    );
}

/// 锚：项目根外的仓库兄弟路径不得借 Runtime 获得写能力。
#[test]
fn git_write_rejects_paths_outside_the_project_root() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    init_repo(&repository);
    commit_file(&repository, "outside.txt", "one\n");
    let project = repository.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(repository.join("outside.txt"), "one\ntwo\n").unwrap();

    // 项目根是仓库子目录：仓库根兄弟位置只读。
    let (target, _recycle) = write_target(&project);
    let error = execute_git_write(
        rollback_intent("outside.txt", GitRollbackExpected::Modified),
        &target,
    )
    .expect_err("项目根外路径必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::Unauthorized);
    assert_eq!(
        std::fs::read_to_string(repository.join("outside.txt")).unwrap(),
        "one\ntwo\n"
    );
}

/// 锚：未跟踪删除默认走回收站；staged added 先取消暂存再删除。
#[test]
fn git_write_delete_covers_untracked_and_staged_added() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    init_repo(&root);
    commit_file(&root, "base.txt", "base\n");
    let (target, recycle) = write_target(&root);

    std::fs::write(root.join("notes.txt"), "draft\n").unwrap();
    execute_git_write(
        delete_intent("notes.txt", GitDeleteExpected::Untracked, false),
        &target,
    )
    .expect("未跟踪文件必须可删除");
    assert!(!root.join("notes.txt").exists());
    assert_eq!(
        recycle.paths.lock().unwrap().as_slice(),
        &[root.join("notes.txt")]
    );

    // 同一前置形态已不存在：拒绝且不写盘。
    let error = execute_git_write(
        delete_intent("notes.txt", GitDeleteExpected::Untracked, false),
        &target,
    )
    .expect_err("目标已消失必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);

    // staged added：取消暂存后永久删除，索引回到干净状态。
    std::fs::write(root.join("added.txt"), "added\n").unwrap();
    git(&root, &["add", "--", "added.txt"]);
    execute_git_write(
        delete_intent("added.txt", GitDeleteExpected::StagedAdded, true),
        &target,
    )
    .expect("暂存新增必须可删除");
    assert!(!root.join("added.txt").exists());
    let status = Command::new("git")
        .current_dir(&root)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        status.stdout.is_empty(),
        "删除后索引与工作区必须干净: {}",
        String::from_utf8_lossy(&status.stdout)
    );

    // 形态不匹配（已暂存新增却声明未跟踪）必须拒绝。
    std::fs::write(root.join("mixed.txt"), "mixed\n").unwrap();
    git(&root, &["add", "--", "mixed.txt"]);
    let error = execute_git_write(
        delete_intent("mixed.txt", GitDeleteExpected::Untracked, true),
        &target,
    )
    .expect_err("形态不匹配必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
    assert!(root.join("mixed.txt").exists());
}

/// 锚：破坏操作状态校验失败时不执行写入。
#[test]
fn runtime_destructive_precondition() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    init_repo(&root);
    commit_file(&root, "a.txt", "one\n");
    let (target, _recycle) = write_target(&root);

    // 声明形态与实际不一致：拒绝且工作区内容不变。
    std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
    let error = execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Deleted),
        &target,
    )
    .expect_err("形态不匹配必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\ntwo\n",
        "前置失败不得触碰工作区"
    );

    // 未跟踪文件声明为暂存新增：拒绝且文件保留。
    std::fs::write(root.join("notes.txt"), "draft\n").unwrap();
    let error = execute_git_write(
        delete_intent("notes.txt", GitDeleteExpected::StagedAdded, true),
        &target,
    )
    .expect_err("形态不匹配必须拒绝");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
    assert!(root.join("notes.txt").exists(), "前置失败不得删除文件");
}

/// 锚：破坏操作走审计通道，只含 operation/path/source/result。
#[test]
fn git_write_events_record_destructive_operations() {
    let captured = crate::audit::test_support::capture_events();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    init_repo(&root);
    commit_file(&root, "a.txt", "one\n");
    std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
    let (target, _recycle) = write_target(&root);

    execute_git_write(
        rollback_intent("a.txt", GitRollbackExpected::Modified),
        &target,
    )
    .expect("未暂存修改必须可回滚");
    // 前置不成立只记失败结果，不写盘。
    assert_delete_precondition_failure(&root, &target);

    let text = captured.text();
    assert!(text.contains("audit"), "事件必须落在 audit target: {text}");
    assert_audit_field(&text, "operation", "git_rollback");
    assert_audit_field(&text, "operation", "git_delete");
    assert_audit_field(&text, "source", "runtime");
    assert_audit_field(&text, "result", "ok");
    assert_audit_field(&text, "result", "error");
    assert!(text.contains("a.txt"), "事件必须含目标路径: {text}");
}

/// 字段断言容忍 tracing fmt 的引号形态（`key=value` / `key="value"`）。
fn assert_audit_field(text: &str, key: &str, value: &str) {
    let plain = format!("{key}={value}");
    let quoted = format!("{key}=\"{value}\"");
    assert!(
        text.contains(&plain) || text.contains(&quoted),
        "审计事件缺少字段 {key}={value}: {text}"
    );
}

fn assert_delete_precondition_failure(root: &Path, target: &GitWriteTarget) {
    commit_file(root, "tracked.txt", "tracked\n");
    let error = execute_git_write(
        delete_intent("tracked.txt", GitDeleteExpected::Untracked, false),
        target,
    )
    .expect_err("已跟踪文件不得按未跟踪删除");
    assert_eq!(error.data.unwrap().code, DomainErrorCode::RevisionConflict);
}

/// 锚：打开方式必须与状态映射一致；匹配时才把绑定与路径交给前台。
#[test]
fn git_open_requires_matching_status_mapping() {
    use std::sync::mpsc;
    use std::time::Duration;

    use crate::app_shell::{GitOpenTarget, RuntimeGitOpenKind, SltermShellEvent};

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    init_repo(&root);
    commit_file(&root, "modified.txt", "one\n");
    commit_file(&root, "deleted.txt", "gone\n");
    std::fs::write(root.join("modified.txt"), "one\ntwo\n").unwrap();
    std::fs::remove_file(root.join("deleted.txt")).unwrap();
    std::fs::write(root.join("untracked.txt"), "fresh\n").unwrap();

    let target = GitOpenTarget {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        root: CanonicalPath::new(&root).unwrap(),
        git: Arc::new(GitFacade::default()),
    };
    let (events, receiver) = mpsc::sync_channel(1);
    let no_event = || receiver.try_recv().is_err();

    // 删除条目不得按 Diff 打开；修改条目不得按固定版本打开。
    for intent in [
        GitOpenIntent::Diff {
            path: "deleted.txt".to_owned(),
        },
        GitOpenIntent::Deleted {
            path: "modified.txt".to_owned(),
        },
        GitOpenIntent::Diff {
            path: "untracked.txt".to_owned(),
        },
    ] {
        let error = execute_git_open(intent, &target, &events, Duration::from_millis(50))
            .expect_err("映射不一致必须拒绝");
        assert_eq!(error.data.unwrap().code, DomainErrorCode::InvalidRequest);
        assert!(no_event(), "映射不一致不得投递打开事件");
    }

    // 未暂存修改映射为 Diff：事件携带绑定与原路径，前台回执决定结果。
    let target_for_worker = GitOpenTarget {
        project_id: target.project_id,
        root: target.root.clone(),
        git: Arc::clone(&target.git),
    };
    let events_for_worker = events.clone();
    let worker = std::thread::spawn(move || {
        execute_git_open(
            GitOpenIntent::Diff {
                path: "modified.txt".to_owned(),
            },
            &target_for_worker,
            &events_for_worker,
            Duration::from_secs(5),
        )
    });
    let event = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("匹配的打开必须投递事件");
    match event {
        SltermShellEvent::RuntimeGitOpen {
            project_id,
            path,
            kind,
            completion,
            ..
        } => {
            assert_eq!(project_id, target.project_id);
            assert_eq!(kind, RuntimeGitOpenKind::Diff);
            let expected =
                CanonicalPath::new(dunce::canonicalize(root.join("modified.txt")).unwrap())
                    .unwrap();
            assert_eq!(path, expected);
            let _ = completion.try_send(Ok(()));
        }
        _ => panic!("必须是 Git 打开事件"),
    }
    worker.join().unwrap().expect("前台接受后必须成功");
}
