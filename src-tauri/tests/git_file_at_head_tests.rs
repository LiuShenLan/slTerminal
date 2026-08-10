//! git_file_at_head 命令域测试（GIT-12 拆分）
//!
//! 原 `git/mod.rs` 的 `#[cfg(test)] mod tests` 按命令拆分为独立集成测试文件。
//! 本文件覆盖：HEAD tree 读取、UnbornBranch 空仓库、文件不在 tree、
//! 子目录文件、已删除文件的沙箱放行 + HEAD 内容恢复。
//!
//! GIT-01：全部改为命令层测试（`git_file_at_head_impl` 直调——最小 AppState +
//! block_on await 真实命令实现），覆盖 State 注入、路径沙箱（GIT-10）、
//! spawn_blocking、错误消息契约；GIT-09：UnbornBranch 用例改调真实命令。

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{block_on, commit_file, git_add, init_temp_repo, make_app_state};
use slterminal_lib::git::git_file_at_head_impl;
use slterminal_lib::AppError;

/// 命令层辅助：构造 root=repo 的 AppState 并 await 真实命令
fn at_head(path: &Path, file_path: &Path) -> Result<String, AppError> {
    let app = make_app_state(Some(path.to_path_buf()));
    block_on(git_file_at_head_impl(
        &app,
        &path.to_string_lossy(),
        &file_path.to_string_lossy(),
    ))
}

#[test]
fn git_file_at_head_reads_content() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "readme.md", "# Hello\n\nWorld\n");

    let content = at_head(&path, &path.join("readme.md")).unwrap();
    assert_eq!(content, "# Hello\n\nWorld\n");
}

/// GIT-09 重写：UnbornBranch 错误契约——调真实命令，断言 AppError::Git
/// 消息含"HEAD 中不存在"（旧测试只验证 git2::Repository::head() 返回
/// UnbornBranch，未验证命令错误消息契约）。
#[test]
fn git_file_at_head_unborn_branch_err() {
    let (_dir, path) = init_temp_repo();
    // 空仓库无 commit → UnbornBranch
    let file_path = path.join("a.txt");
    fs::write(&file_path, "garbage").unwrap();

    let err = at_head(&path, &file_path).unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "UnbornBranch 应返回 AppError::Git，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("HEAD 中不存在"),
        "错误消息应含'HEAD 中不存在'，实际: {err}"
    );
}

/// 错误契约：文件不在 HEAD tree → AppError::Git，消息含"文件在 HEAD 中不存在"
#[test]
fn git_file_at_head_file_not_in_tree() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "existing.txt", "present");

    let err = at_head(&path, &path.join("nonexistent.txt")).unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "不存在的文件应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("HEAD 中不存在"),
        "错误消息应含'HEAD 中不存在'，实际: {err}"
    );
}

#[test]
fn git_file_at_head_subdirectory_file() {
    let (_dir, path) = init_temp_repo();
    // 在子目录中创建并 commit 文件
    let sub = path.join("src").join("lib");
    fs::create_dir_all(&sub).unwrap();
    let file_path = sub.join("mod.rs");
    fs::write(&file_path, "pub fn hello() {}").unwrap();
    git_add(&path, "src/lib/mod.rs");
    Command::new("git")
        .args(["commit", "-m", "add nested"])
        .current_dir(&path)
        .output()
        .unwrap();

    let content = at_head(&path, &file_path).unwrap();
    assert_eq!(content, "pub fn hello() {}");
}

/// 已删除文件：validate_path_within_root 放行（上溯到最近存在祖先）+ 命令读到 HEAD 内容
#[test]
fn git_file_at_head_deleted_file_roundtrip() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "will_delete.txt", "content before deletion\n");

    let file_path = path.join("will_delete.txt");
    // 物理删除文件
    std::fs::remove_file(&file_path).unwrap();
    assert!(!file_path.exists(), "前置：文件应已删除");

    // 沙箱放行 + HEAD 读取应成功返回原始内容
    let content = at_head(&path, &file_path).unwrap();
    assert_eq!(content, "content before deletion\n");
}

/// repo_path 空串：search_path 回退 file_path（get_or_open_repo 分支）仍能工作
#[test]
fn git_file_at_head_command_repo_path_empty_fallback() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "fallback content\n");

    let app = make_app_state(Some(path.clone()));
    let content = block_on(git_file_at_head_impl(
        &app,
        "",
        &path.join("a.txt").to_string_lossy(),
    ))
    .unwrap();
    assert_eq!(content, "fallback content\n");
}

/// 沙箱拒绝（SEC-01 / GIT-10）：file_path 在 project_root 外 → 拒绝
#[test]
fn git_file_at_head_command_sandbox_rejects_outside() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "x\n");
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("a.txt");
    fs::write(&outside_file, "x\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let err = block_on(git_file_at_head_impl(
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

/// 错误契约：非 git 目录 → AppError::Git，消息含"打开仓库失败"
#[test]
fn git_file_at_head_command_non_repo_error_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let not_repo = tmp.path().join("not_a_repo");
    std::fs::create_dir_all(&not_repo).unwrap();

    let app = make_app_state(Some(tmp.path().to_path_buf()));
    let err = block_on(git_file_at_head_impl(
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
