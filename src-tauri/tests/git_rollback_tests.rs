//! git_rollback 命令域测试（GIT-12 拆分）
//!
//! 原 `git/mod.rs` 的 `#[cfg(test)] mod tests` 按命令拆分为独立集成测试文件。
//! 本文件覆盖当前实现（`std::fs::write(blob)` + `index.add_path` + `index.write`）
//! 的命令层行为：修改/删除/autocrlf/路径隔离/跨实例回滚 + 沙箱拒绝（GIT-10）
//! + 错误消息契约。
//!
//! GIT-02：`git_rollback_two_step_*` 7 条已删除——验证的是已废弃实现
//! （`reset_default` + `checkout_index` 两步法），生产已改为当前命令路径
//! （D3 测试对齐实现）。
//! GIT-01：inline 重写 git2 调用序列的测试改为命令层（`git_rollback_impl` 直调——
//! 最小 AppState + block_on await 真实命令实现）；文件底部保留两条 git2 底层原语。

mod common;

use std::path::Path;
use std::process::Command;

use common::{block_on, commit_file, init_temp_repo, make_app_state};
use slterminal_lib::git::git_rollback_impl;
use slterminal_lib::AppError;

/// 命令层辅助：构造 root=repo 的 AppState 并 await 真实命令
fn rollback(repo_path: &Path, file_path: &Path) -> Result<(), AppError> {
    let app = make_app_state(Some(repo_path.to_path_buf()));
    block_on(git_rollback_impl(
        &app,
        &repo_path.to_string_lossy(),
        &file_path.to_string_lossy(),
    ))
}

/// 回滚已修改文件：写 HEAD blob → 重建 index → status 干净
#[test]
fn git_rollback_restores_modified() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "HEAD content\n");

    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "dirty\n").unwrap();

    // 前置：修改后应在 status 中
    let repo = git2::Repository::open(&path).unwrap();
    assert!(
        repo.statuses(None)
            .unwrap()
            .iter()
            .any(|e| e.path().unwrap_or("") == "a.txt"),
        "前置：修改后应在 status 中"
    );

    rollback(&path, &file_path).unwrap();

    // 验证内容恢复 + status 干净
    let restored = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(restored, "HEAD content\n");
    assert!(
        !repo
            .statuses(None)
            .unwrap()
            .iter()
            .any(|e| e.path().unwrap_or("") == "a.txt"),
        "回滚后 git status 应干净"
    );
}

/// 回滚已删除文件：写 HEAD blob 恢复 + 重建 index
#[test]
fn git_rollback_restores_deleted() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "HEAD content\n");

    let file_path = path.join("a.txt");
    std::fs::remove_file(&file_path).unwrap();
    assert!(!file_path.exists());

    rollback(&path, &file_path).unwrap();

    assert!(file_path.exists());
    assert_eq!(
        std::fs::read_to_string(&file_path).unwrap(),
        "HEAD content\n"
    );
    let repo = git2::Repository::open(&path).unwrap();
    assert!(!repo
        .statuses(None)
        .unwrap()
        .iter()
        .any(|e| e.path().unwrap_or("") == "a.txt"));
}

/// autocrlf=true：写 LF blob → add_path(clean:LF→LF) → status 干净
/// （显式设置 core.autocrlf=true，覆盖 init_temp_repo 的仓库局部默认 false，GIT-06）
#[test]
fn git_rollback_autocrlf_clean_status() {
    let (_dir, path) = init_temp_repo();
    Command::new("git")
        .args(["config", "core.autocrlf", "true"])
        .current_dir(&path)
        .output()
        .unwrap();
    commit_file(&path, "a.txt", "line1\nline2\n");

    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "dirty\r\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    assert!(repo
        .statuses(None)
        .unwrap()
        .iter()
        .any(|e| e.path().unwrap_or("") == "a.txt"));

    rollback(&path, &file_path).unwrap();

    assert!(
        !repo
            .statuses(None)
            .unwrap()
            .iter()
            .any(|e| e.path().unwrap_or("") == "a.txt"),
        "autocrlf=true：LF 工作区 + LF index → status 应干净"
    );
}

/// 隔离：仅回滚指定文件，不影响其他文件
#[test]
fn git_rollback_paths_isolation() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "A\n");
    commit_file(&path, "b.txt", "B\n");

    std::fs::write(path.join("a.txt"), "A-modified\n").unwrap();
    std::fs::write(path.join("b.txt"), "B-modified\n").unwrap();

    rollback(&path, &path.join("a.txt")).unwrap();

    assert_eq!(std::fs::read_to_string(path.join("a.txt")).unwrap(), "A\n");
    assert_eq!(
        std::fs::read_to_string(path.join("b.txt")).unwrap(),
        "B-modified\n"
    );
}

/// 跨实例回归：命令回滚后，实例 B（全新 open）status 干净
#[test]
fn git_rollback_cross_instance_clean() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "line1\nline2\n");

    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "dirty\n").unwrap();

    // 实例 A：命令回滚
    let repo_a = git2::Repository::open(&path).unwrap();
    assert!(repo_a
        .statuses(None)
        .unwrap()
        .iter()
        .any(|e| e.path().unwrap_or("") == "a.txt"));

    rollback(&path, &file_path).unwrap();

    // 实例 B：全新 open（模拟 get_or_open_repo 新实例）
    let repo_b = git2::Repository::open(&path).unwrap();
    assert!(
        !repo_b
            .statuses(None)
            .unwrap()
            .iter()
            .any(|e| e.path().unwrap_or("") == "a.txt"),
        "跨实例回归：实例 B status 应干净"
    );
}

/// 错误契约：UnbornBranch（无 HEAD）→ AppError::Git，消息含"HEAD 中不存在"
#[test]
fn git_rollback_command_unborn_branch_err() {
    let (_dir, path) = init_temp_repo();
    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "garbage").unwrap();

    let err = rollback(&path, &file_path).unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "UnbornBranch 应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("HEAD 中不存在"),
        "错误消息应含'HEAD 中不存在'，实际: {err}"
    );
}

/// 错误契约：文件不在 HEAD tree → AppError::Git，消息含"文件在 HEAD 中不存在"
#[test]
fn git_rollback_command_file_not_in_tree_err() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "A\n");

    let err = rollback(&path, &path.join("nonexistent.txt")).unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "不在 tree 中的文件应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("HEAD 中不存在"),
        "错误消息应含'HEAD 中不存在'，实际: {err}"
    );
}

/// 沙箱拒绝（SEC-01 / GIT-10）：file_path 在 project_root 外 → 拒绝，不写盘
#[test]
fn git_rollback_command_sandbox_rejects_outside() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "x\n");
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("a.txt");
    std::fs::write(&outside_file, "x\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let err = block_on(git_rollback_impl(
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
    // 根外文件未被改动
    assert_eq!(std::fs::read_to_string(&outside_file).unwrap(), "x\n");
}

// ---- 底层原语（git2 行为验证，GIT-01 保留；命令层错误契约见上方） ----

/// 底层原语：tree.get_path 对不存在路径返回 NotFound
#[test]
fn git_rollback_nonexistent_in_tree_errors() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "A\n");

    let repo = git2::Repository::open(&path).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let result = tree.get_path(Path::new("nonexistent.txt"));
    assert!(result.is_err(), "不在 tree 中的路径应返回 Err");
}

/// 底层原语：UnbornBranch → repo.head() 返回 Err
#[test]
fn git_rollback_unborn_branch_errors() {
    let (_dir, path) = init_temp_repo();
    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "garbage").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    assert!(repo.head().is_err(), "UnbornBranch：HEAD 应不存在");
}

/// TQ-COV-06：broken HEAD（HEAD 指向 blob）→ peel_to_tree 失败 → 命令层
/// 错误契约"获取 HEAD tree 失败"（行 543-549 map_err 闭包；UnbornBranch 的
/// head() 直错分支已有用例，此处补 Ok(head) 但 peel_to_tree 失败分支）
#[test]
fn git_rollback_broken_head_peel_tree_err() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "x\n");
    let blob = Command::new("git")
        .args(["hash-object", "-w", "f.txt"])
        .current_dir(&path)
        .output()
        .unwrap();
    let blob_sha = String::from_utf8(blob.stdout).unwrap().trim().to_string();
    Command::new("git")
        .args(["tag", "blobtag", &blob_sha])
        .current_dir(&path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/tags/blobtag"])
        .current_dir(&path)
        .output()
        .unwrap();

    let err = rollback(&path, &path.join("f.txt")).unwrap_err();
    assert!(
        err.to_string().contains("获取 HEAD tree 失败"),
        "broken HEAD 应报'获取 HEAD tree 失败'，实际: {err}"
    );
}
