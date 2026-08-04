//! git 测试共享工厂（GIT-12 拆分产物）
//!
//! `init_temp_repo` / `commit_file` / `git_add` 原内嵌于 `git/mod.rs` 的
//! `#[cfg(test)] mod tests`，GIT-12 拆分后提取到本模块供
//! `tests/git_status_tests.rs` / `git_diff_tests.rs` / `git_file_at_head_tests.rs` /
//! `git_rollback_tests.rs` / `git_unstage_tests.rs` 复用。
//!
//! 注意：本目录（tests/common/）不会被 Cargo 自动编译为独立测试目标，
//! 各测试文件通过 `mod common;` 引入；每个消费 crate 只引用自己用到的
//! helper，未使用的 helper 在该 crate 视角为死代码——模块级 allow 是
//! 共享测试工具的标准做法（CI rustflags 默认 -D warnings）。

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// 在临时目录中 init 一个 git 仓库，返回 tempdir（自动清理）和路径
///
/// GIT-06：仓库局部设置 `core.autocrlf=false` / `core.safecrlf=false` /
/// `init.defaultBranch=main`——隔离 runner 全局 git 配置（换环境结果不漂移）。
pub fn init_temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    // CI runner 的 %TEMP% 含 8.3 短名（如 RUNNER~1），而 git2 workdir 返回长名，
    // 两者 strip_prefix/路径断言会不匹配（dunce::simplified 只剥 verbatim 前缀、
    // 不解析短名→长名）。canonicalize 统一为长名，从源头消除短/长名差异。
    let path = dunce::canonicalize(dir.path()).unwrap();

    // -c 传递 init.defaultBranch=main：固定初始分支名，隔离 runner 全局 defaultBranch 漂移
    Command::new("git")
        .args(["-c", "init.defaultBranch=main", "init"])
        .current_dir(&path)
        .output()
        .unwrap();

    // 仓库局部配置（非全局）：隔离 autocrlf/safecrlf/user 漂移（GIT-06）
    for (key, value) in [
        ("core.autocrlf", "false"),
        ("core.safecrlf", "false"),
        ("user.email", "test@test.com"),
        ("user.name", "Test"),
    ] {
        Command::new("git")
            .args(["config", key, value])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    (dir, path)
}

/// Helper: 在 git 仓库中 commit 一个文件
pub fn commit_file(repo_path: &Path, filename: &str, content: &str) {
    let file_path = repo_path.join(filename);
    std::fs::write(&file_path, content).unwrap();
    Command::new("git")
        .args(["add", filename])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", &format!("commit {filename}")])
        .current_dir(repo_path)
        .output()
        .unwrap();
}

/// Helper: git add 一个文件
pub fn git_add(repo_path: &Path, filename: &str) {
    Command::new("git")
        .args(["add", filename])
        .current_dir(repo_path)
        .output()
        .unwrap();
}

// ---- GIT-01 命令层测试共享工厂 ----
//
// 命令层测试构造最小 AppState（project_root 显式设置）后直接 await
// `git_*_impl` 真实命令实现（GIT-01：源码最小抽函数，命令壳零行为变更）。
// 注意：集成测试视角下 lib 不编译 cfg(test)——validate_path_within_root
// 无"未设置 project_root 豁免"，故命令层测试必须设置 project_root = Some(根)。

/// 构造带指定 project_root 的最小 AppState（命令层测试用）
pub fn make_app_state(root: Option<std::path::PathBuf>) -> slterminal_lib::AppState {
    let app = slterminal_lib::AppState::new();
    *app.project_root.write().unwrap() = root;
    app
}

/// 在 current_thread runtime 上 await async 命令实现
/// （tokio 依赖仅开 "rt" feature，无 rt-multi-thread，故不用 Runtime::new）
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(f)
}
