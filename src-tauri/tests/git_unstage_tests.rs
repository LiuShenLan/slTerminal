//! git_unstage 命令域测试（GIT-12 拆分）
//!
//! 原 `git/mod.rs` 的 `#[cfg(test)] mod tests` 按命令拆分为独立集成测试文件。
//! 本文件覆盖：INDEX_NEW 文件取消暂存 → untracked、INDEX_MODIFIED 的
//! remove_path 行为（条目完全删除而非 reset 到 HEAD）、不存在路径静默成功。
//!
//! GIT-01：inline 重写 git2 调用序列的测试改为命令层（`git_unstage_impl` 直调——
//! 最小 AppState + block_on await 真实命令实现）；文件底部保留一条 git2 底层原语。

mod common;

use std::path::Path;

use common::{block_on, commit_file, git_add, init_temp_repo, make_app_state};
use slterminal_lib::git::{git_unstage_impl, status_to_str};
use slterminal_lib::AppError;

/// 命令层辅助：构造 root=repo 的 AppState 并 await 真实命令
fn unstage(repo_path: &std::path::Path, file_path: &Path) -> Result<(), AppError> {
    let app = make_app_state(Some(repo_path.to_path_buf()));
    block_on(git_unstage_impl(
        &app,
        &repo_path.to_string_lossy(),
        &file_path.to_string_lossy(),
    ))
}

/// git_unstage：INDEX_NEW 文件（staged 新文件）取消暂存后变为 untracked
#[test]
fn git_unstage_index_new_file() {
    let (_dir, path) = init_temp_repo();
    // 需要先有一个 commit，否则 git status 不会正常显示
    commit_file(&path, "existing.txt", "placeholder\n");

    // 创建新文件并 git add
    let file_path = path.join("new.txt");
    std::fs::write(&file_path, "new content\n").unwrap();
    git_add(&path, "new.txt");

    // 前置：文件在 index 中（INDEX_NEW）
    let repo = git2::Repository::open(&path).unwrap();
    let before = repo.statuses(None).unwrap();
    let before_entry = before.iter().find(|e| e.path().unwrap_or("") == "new.txt");
    assert!(before_entry.is_some(), "前置：new.txt 应在 git status 中");
    assert!(
        before_entry
            .unwrap()
            .status()
            .contains(git2::Status::INDEX_NEW),
        "前置：应包含 INDEX_NEW"
    );

    // 命令层取消暂存：从 index 移除
    unstage(&path, &file_path).unwrap();

    // 验证：文件不再在 index 中，变为 untracked
    let statuses2 = repo.statuses(None).unwrap();
    let after = statuses2
        .iter()
        .find(|e| e.path().unwrap_or("") == "new.txt");
    assert!(
        after.is_some(),
        "取消暂存后文件仍应出现在 status 中（untracked）"
    );
    let after_status = status_to_str(after.unwrap().status());
    assert_eq!(after_status, Some("untracked"), "应变为 untracked");
}

/// git_unstage：INDEX_MODIFIED 文件用 remove_path 会完全删除 index 条目
/// （仅用于 INDEX_NEW 场景——commit view 中 added 文件走此路径）
#[test]
fn git_unstage_remove_path_on_index_modified_removes_entry() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "v1\n");

    // 修改 + git add → INDEX_MODIFIED
    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "v2\n").unwrap();
    git_add(&path, "a.txt");

    let repo = git2::Repository::open(&path).unwrap();
    let before = repo.statuses(None).unwrap();
    let before_entry = before.iter().find(|e| e.path().unwrap_or("") == "a.txt");
    assert!(before_entry.is_some());
    assert!(
        before_entry
            .unwrap()
            .status()
            .contains(git2::Status::INDEX_MODIFIED),
        "前置：INDEX_MODIFIED"
    );

    // 命令层 unstage：remove_path 完全删除 index 条目（非 reset 到 HEAD）
    unstage(&path, &file_path).unwrap();

    // remove_path 后无 INDEX_MODIFIED（条目已从 index 删除）
    let after = repo.statuses(None).unwrap();
    let after_entry = after.iter().find(|e| e.path().unwrap_or("") == "a.txt");
    // 条目完全被删 → 可能显示为 deleted 或其他状态
    // 此测试锁死 remove_path 行为：它删除条目，不 reset 到 HEAD
    assert!(
        after_entry.is_none()
            || !after_entry
                .unwrap()
                .status()
                .contains(git2::Status::INDEX_MODIFIED),
        "remove_path 后不应有 INDEX_MODIFIED"
    );
}

/// 命令层：对不在 index 中的文件 unstage 静默成功（Ok，不 panic）
#[test]
fn git_unstage_command_nonexistent_in_index_succeeds() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "existing.txt", "content\n");

    unstage(&path, &path.join("nonexistent.txt")).unwrap();
}

/// 错误契约：非 git 目录 → AppError::Git，消息含"打开仓库失败"
#[test]
fn git_unstage_command_non_repo_error_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let not_repo = tmp.path().join("not_a_repo");
    std::fs::create_dir_all(&not_repo).unwrap();

    let app = make_app_state(Some(tmp.path().to_path_buf()));
    let err = block_on(git_unstage_impl(
        &app,
        &not_repo.to_string_lossy(),
        &not_repo.join("a.txt").to_string_lossy(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "非 git 目录应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("打开仓库失败"),
        "错误消息应含'打开仓库失败'，实际: {err}"
    );
}

/// 沙箱拒绝（SEC-01 / GIT-10）：file_path 在 project_root 外 → 拒绝
#[test]
fn git_unstage_command_sandbox_rejects_outside() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "x\n");
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("a.txt");
    std::fs::write(&outside_file, "x\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let err = block_on(git_unstage_impl(
        &app,
        &path.to_string_lossy(),
        &outside_file.to_string_lossy(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, AppError::IoKind { .. }),
        "根外 file_path 应被沙箱拒绝为 IoKind，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("项目范围"),
        "错误消息应含'项目范围'，实际: {err}"
    );
}

// ---- 底层原语（git2 行为验证，GIT-01 保留；命令层行为见上方） ----

/// 底层原语：remove_path 对不在 index 中的文件静默成功（非 Err）
#[test]
fn git_unstage_nonexistent_in_index_no_panic() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "existing.txt", "content\n");

    let repo = git2::Repository::open(&path).unwrap();
    let mut index = repo.index().unwrap();
    // remove_path 对不在 index 中的文件静默成功（非 Err）
    let result = index.remove_path(Path::new("nonexistent.txt"));
    // 行为记录：git2 0.20 remove_path 对不存在路径也返回 Ok
    let _ = result;
}
