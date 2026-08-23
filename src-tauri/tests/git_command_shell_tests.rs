//! git Tauri 命令壳测试（TQ-COV-06）
//!
//! 背景：llvm-cov 实测 git/mod.rs 函数覆盖 37.14%——未执行函数即 5 个
//! `#[tauri::command]` 命令壳（git_status/git_diff/git_file_at_head/
//! git_rollback/git_unstage）。既有命令层测试（GIT-01）直接调 `git_*_impl`
//! 内核，而命令壳需要 `tauri::State<'_, AppState>` 注入，L1 无 mock 前例
//! 时函数覆盖恒 0%。
//!
//! 本文件用 `tauri::test::mock_builder` 构造 mock App（manage 注入最小
//! AppState）→ `app.state::<AppState>()` 取 State → block_on await 命令壳，
//! 验证壳层转发与 impl 行为一致（转发契约）。
//!
//! 依赖：tauri 的 `test` feature 经 [dev-dependencies] 开启（见 Cargo.toml，
//! 仅测试编译生效，release/debug 构建零影响）。

mod common;

use std::path::Path;

use common::{block_on, commit_file, git_add, init_temp_repo, make_app_state};
use slterminal_lib::git::{git_diff, git_file_at_head, git_rollback, git_status, git_unstage};
use slterminal_lib::AppState;
use tauri::{
    test::{mock_builder, mock_context, noop_assets},
    Manager,
};

/// 构造 manage 注入 AppState 的 mock Tauri App（tauri::test，MockRuntime）
fn mock_app_with_state(root: Option<std::path::PathBuf>) -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .manage(make_app_state(root))
        .build(mock_context(noop_assets()))
        .unwrap()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

/// git_status 命令壳：转发 impl，untracked 文件以绝对路径返回
#[test]
fn git_status_command_shell_returns_entries() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "hello\n");
    std::fs::write(path.join("b.txt"), "untracked\n").unwrap();

    let app = mock_app_with_state(Some(path.clone()));
    let entries = block_on(git_status(path_str(&path), app.state::<AppState>())).unwrap();

    assert_eq!(entries.len(), 1, "应只含 untracked 的 b.txt");
    assert_eq!(entries[0].status, "untracked");
    assert_eq!(
        entries[0].path,
        path.join("b.txt").to_string_lossy().replace('\\', "/"),
        "路径应为 workdir 拼接的绝对路径（\\→/ 规范化）"
    );
}

/// git_diff 命令壳：转发 impl，单行修改 → 1 个 modified hunk
#[test]
fn git_diff_command_shell_returns_hunks() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "line1\nline2\nline3\n");
    std::fs::write(path.join("a.txt"), "line1\nchanged\nline3\n").unwrap();

    let app = mock_app_with_state(Some(path.clone()));
    let file = path.join("a.txt");
    let hunks = block_on(git_diff(
        path_str(&path),
        path_str(&file),
        app.state::<AppState>(),
    ))
    .unwrap();

    assert_eq!(hunks.len(), 1, "单行修改应合并为 1 个 hunk");
    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_lines, 1);
}

/// git_file_at_head 命令壳：转发 impl，返回 HEAD 内容
#[test]
fn git_file_at_head_command_shell_returns_content() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "readme.md", "# Hello\n");

    let app = mock_app_with_state(Some(path.clone()));
    let content = block_on(git_file_at_head(
        path_str(&path),
        path_str(&path.join("readme.md")),
        app.state::<AppState>(),
    ))
    .unwrap();

    assert_eq!(content, "# Hello\n");
}

/// git_rollback 命令壳：转发 impl，工作区修改回滚到 HEAD 内容
#[test]
fn git_rollback_command_shell_restores_file() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "original\n");
    let file = path.join("a.txt");
    std::fs::write(&file, "modified\n").unwrap();

    let app = mock_app_with_state(Some(path.clone()));
    block_on(git_rollback(
        path_str(&path),
        path_str(&file),
        app.state::<AppState>(),
    ))
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "original\n",
        "回滚后工作区应恢复 HEAD 内容"
    );
}

/// git_unstage 命令壳：转发 impl，staged 新文件移出 index 后回到 untracked
#[test]
fn git_unstage_command_shell_unstages_file() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "x\n");
    let file = path.join("b.txt");
    std::fs::write(&file, "new\n").unwrap();
    git_add(&path, "b.txt");

    let app = mock_app_with_state(Some(path.clone()));
    block_on(git_unstage(
        path_str(&path),
        path_str(&file),
        app.state::<AppState>(),
    ))
    .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let statuses = repo.statuses(None).unwrap();
    assert_eq!(statuses.len(), 1, "unstage 后只剩 b.txt 一条");
    let status = statuses.iter().next().expect("应有一条状态条目").status();
    assert!(
        status.contains(git2::Status::WT_NEW),
        "unstage 后应回到 untracked（WT_NEW），实际: {status:?}"
    );
}
