# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Git 版本控制模块——基于 `git2` crate，封装 `git_status`（文件状态着色）和 `git_diff`（行级差异），通过 Tauri 命令向前端暴露 CamelCase DTO。

## 测试模式

测试位于 `git/mod.rs` 的 `#[cfg(test)] mod tests`（~42 用例），是 Rust 端最大的单文件测试模块。

### 测试工厂

两个关键辅助函数，在每个测试中创建真实 git 仓库：

```rust
fn init_temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().to_path_buf();
    // git init + git config user
    Command::new("git").args(["init"]).current_dir(&repo_path).output().unwrap();
    (dir, repo_path)  // TempDir 保持存活以维持临时目录
}

fn commit_file(repo_path: &Path, filename: &str, content: &str) {
    std::fs::write(repo_path.join(filename), content).unwrap();
    Command::new("git").args(["add", filename]).current_dir(repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "test"]).current_dir(repo_path).output().unwrap();
}
```

> `commit_file` / `git_add` 调用系统 `git` CLI（非 `git2` 提交 API——`git2` 不直接支持创建 commit）。

### status_to_str 表驱动测试（12 用例）

纯函数映射 `git2::Status` → `&str`，每个状态标志一条用例：

```rust
assert_eq!(status_to_str(git2::Status::WT_NEW), "untracked");
assert_eq!(status_to_str(git2::Status::INDEX_NEW), "added");
assert_eq!(status_to_str(git2::Status::WT_MODIFIED), "modified");
assert_eq!(status_to_str(git2::Status::CURRENT), None); // 无状态
```

### git_status / git_diff 集成测试

在真实 git 仓库中操作并验证项目 API：

- **文件状态**（5 用例）：空仓库无文件、修改→`WT_MODIFIED`、新增未跟踪→`WT_NEW`、add→`INDEX_NEW`、删除→`WT_DELETED`、非仓库→Err
- **绝对路径**（4 用例）：验证 `git_status` 返回的文件路径为完整绝对路径（Windows `\\?\` 前缀经 `dunce::simplified()` 剥离）
- **diff hunk**（4 用例）：修改/新增/未跟踪/绝对路径 diff 返回 `Vec<DiffHunk>`

### diff hunk 行回调精确验证

`collect_precise_hunks()` 调用 `git2::Diff::foreach()` 的回调 API，按行分组合并连续的增删行：

- 单行修改 → 1 个 hunk（`old_line` + `new_line`）
- 连续多行增删 → 合并为 1 个 hunk
- 上下文行隔开 → 拆分为多个 hunk
- 空仓库无变更 → 空 `Vec`

### serde CamelCase 验证

```rust
let hunk = DiffHunk { ... };
let json = serde_json::to_string(&hunk).unwrap();
assert!(json.contains("oldStart"));  // camelCase，非 old_start
assert!(json.contains("newStart"));
```

### 测试隔离约束

- 每个测试创建独立 `tempdir` + `git init`，不共享仓库状态
- 系统需安装 `git` CLI（`commit_file` 依赖 `git add` + `git commit`）
- 不依赖 `[dev-dependencies]`——`tempfile`、`git2`、`serde_json` 均在 `[dependencies]` 中
