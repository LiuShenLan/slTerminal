# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Git 版本控制模块——基于 `git2` crate，封装 `git_status`（文件状态着色）、`git_diff`（行级差异）和 `git_file_at_head`（HEAD 文件内容），通过 Tauri 命令向前端暴露 CamelCase DTO。

## 命令

### git_status

参数：`repo_path: String`。返回 `Vec<GitStatusEntry>`。

`StatusOptions` 配置：`.recurse_untracked_dirs(true)` + `.renames_head_to_index(true).renames_index_to_workdir(true)`。未跟踪目录内文件逐个返回（非 `foo/` 单条目）。rename 检测开启后 `INDEX_RENAMED`/`WT_RENAMED` 状态位正常运行时可被置位。

### GitStatusEntry.oldPath

renamed 条目（`INDEX_RENAMED` / `WT_RENAMED`）从 delta 的 `old_file()` 取旧路径，拼接为与 `path` 同格式的绝对路径。非 renamed → `None`（序列化 `oldPath: null`，camelCase）。前提：`git_status` 的 `StatusOptions` 须开启 `.renames_head_to_index(true).renames_index_to_workdir(true)`，否则 rename 检测不工作、`oldPath` 恒 null。

### git_diff

参数：`(repo_path: String, file_path: String)`。返回 `Vec<DiffHunk>`（`oldStart/oldLines/newStart/newLines`，均为 1-based）。

### git_file_at_head（新增命令）

参数：`(repo_path: String, file_path: String)`。返回 `Result<String, AppError>`。

实现流程：① `validate_path_within_root` 校验 file_path（路径沙箱——对已删除文件同样有效：`canonicalize_or_ancestor` 在路径不存在时上溯到最近存在的祖先目录后拼接校验）；② 复用 `get_or_open_repo`（搜 file_path 所属仓库）；③ `spawn_blocking` 内：`repo.head()` → `peel_to_tree` → 按 workdir 相对路径取 blob → `String::from_utf8_lossy` 转字符串。

**HEAD 不存在错误约定**：`UnbornBranch`（仓库尚无提交）或文件不在 HEAD tree → `AppError::Git`，消息含 `"HEAD 中不存在"`。前端 catch 任意错误显示占位文案"该文件在 HEAD 中不存在"，不解析错误内容。

**workdir strip**：用 `dunce::simplified`（8.3 短名坑，见下方测试工厂注释）。

**已删除文件路径校验**：`validate_path_within_root`（`state.rs`）最初调用 `dunce::canonicalize(file_path)`，要求文件存在于磁盘上——已删除文件会被拒绝。已修改为 `canonicalize_or_ancestor`：路径不存在时上溯到最近存在的祖先目录，canonicalize 后再拼接剩余部分做校验。安全不变：祖先的 canonicalize 解析所有 symlink/`..` 穿越。

### 注册

五条命令均在 `src-tauri/src/lib.rs` 的 `generate_handler!` 注册（`git_status`、`git_diff`、`git_file_at_head`、`git_rollback`、`git_unstage`）。

### git_rollback（新增命令）

参数：`(repo_path: String, file_path: String)`。返回 `Result<(), AppError>`。

实现流程：路径沙箱校验 → `get_or_open_repo` → `spawn_blocking` 内：
1. `repo.head()?.peel_to_tree()` 获取 HEAD tree
2. `tree.get_path(rel)` → `to_object` → `peel_to_blob()` 读取 HEAD blob
3. `std::fs::write(&file_path, blob.content())` 写入原始字节到工作区
4. `repo.index()` → `index.add_path(Path::new(&rel))` → `index.write()` 重建 index 条目（同步 stat/哈希）

**为什么不用 checkout API**：`checkout_head`/`checkout_index`/`reset_default` 在 Windows `core.autocrlf=true` 仓库中对单个文件的 index 持久化和 smudge filter 行为不一致，多次实测均导致 `statuses()` 仍报告 dirty。`std::fs::write` + `index.add_path` 确保 HEAD blob、工作区、index 三方字节完全一致，`statuses()` stat 快速路径必定命中。

### git_unstage（新增命令）

参数：`(repo_path: String, file_path: String)`。返回 `Result<(), AppError>`。

实现流程：路径沙箱校验 → `get_or_open_repo` → `spawn_blocking` 内：
1. `repo.index()` 获取 index
2. `index.remove_path(Path::new(&rel))` 从 index 移除条目
3. `index.write()` 持久化

等价于 `git reset HEAD -- <file>`。对 INDEX_NEW（staged 新文件）有效——仅删除 index 条目，不依赖 HEAD 中存在该文件。仅用于 commit view 右键菜单的 "added 文件删除" 场景。

## 测试模式

GIT-12 将原 `git/mod.rs` 的 88 条 `#[cfg(test)] mod tests`（Rust 端最大的单文件测试模块）按命令拆分为独立集成测试文件（共 98 用例），共享工厂提取到 `tests/common/mod.rs`：

| 文件 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `tests/git_status_tests.rs` | 41 | status_to_str 映射、git_status 状态行为、include_ignored 行为、绝对路径格式、递归未跟踪目录、renamed oldPath + git2 底层原语（dunce/discover/get_or_open_repo 缓存） |
| `tests/git_diff_tests.rs` | 32 | git_diff hunk 收集与 diff 行为 |
| `tests/git_file_at_head_tests.rs` | 8 | HEAD 文件内容读取 + UnbornBranch/不存在错误契约（GIT-09 直测命令） |
| `tests/git_rollback_tests.rs` | 10 | 回滚行为 + autocrlf 仓库三方一致 |
| `tests/git_unstage_tests.rs` | 6 | 取消暂存行为 |
| `tests/ci_config_tests.rs` | 1 | `ci_l1_uses_single_test_thread`（GIT-11 领域污染迁移，锁死 CI L1 的 `--test-threads=1`） |

各文件顶部 `mod common;` 引入共享工厂；`tests/common/` 不会被 Cargo 自动编译为独立测试目标，`#![allow(dead_code)]` 是共享测试工具的标准做法（CI rustflags 默认 `-D warnings`）。

### 测试工厂（`tests/common/mod.rs`，GIT-12 提取）

`init_temp_repo` / `commit_file` / `git_add` / `make_app_state` / `block_on` 五个共享辅助：

```rust
pub fn init_temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    // 必须 canonicalize：CI runner 的 %TEMP% 含 8.3 短名（RUNNER~1），而 git2 workdir
    // 返回长名，两者 strip_prefix/路径断言会不匹配（dunce::simplified 不解析短名→长名）。
    let path = dunce::canonicalize(dir.path()).unwrap();
    // GIT-06：-c 传 init.defaultBranch=main + 仓库局部 core.autocrlf=false /
    // core.safecrlf=false / user.email——隔离 runner 全局 git 配置（换环境结果不漂移）
    Command::new("git").args(["-c", "init.defaultBranch=main", "init"]).current_dir(&path).output().unwrap();
    (dir, path)  // TempDir 保持存活以维持临时目录
}

pub fn commit_file(repo_path: &Path, filename: &str, content: &str) { /* write + git add + git commit */ }
pub fn git_add(repo_path: &Path, filename: &str) { /* git add */ }
pub fn make_app_state(root: Option<PathBuf>) -> AppState { /* 命令层测试的最小 AppState */ }
pub fn block_on<F: std::future::Future>(f: F) -> F::Output { /* tokio Runtime block_on */ }
```

> `commit_file` / `git_add` 调用系统 `git` CLI（非 `git2` 提交 API——`git2` 不直接支持创建 commit）。
> **git CLI 最低版本 2.28（GIT-08③）**：`init_temp_repo` 的 `git -c init.defaultBranch=main init` 依赖 `init.defaultBranch` 配置（2.28 引入，早期版本会静默忽略该键）。

> **8.3 短名坑（CI 必踩）**：GitHub runner 用户 `runneradmin` 的 `%TEMP%` 是短名 `RUNNER~1`，git2 `repo.workdir()` 解析为长名，直接 `strip_prefix`/断言比较会全部失败（曾致 L1 22 个 git 测试红、E2E 被 skipped）。规避两手：① `init_temp_repo` 对返回路径 `dunce::canonicalize`（短名→长名）；② 裸 `repo.workdir()` 的 strip_prefix 站点统一包 `dunce::simplified(...)`（剥 verbatim）。守卫测试 `init_temp_repo_path_canonicalized_and_strips` / `get_or_open_repo_workdir_equals_canonical_path` 锁死此不变量。

### 命令层测试（GIT-01：最小 AppState + block_on await 真实命令）

五命令均抽 `git_*_impl` 命令内核（`git_status_impl`/`git_diff_impl`/`git_file_at_head_impl`/`git_rollback_impl`/`git_unstage_impl`，async 函数，root 从 `State` 提取后传入）。命令层测试用 `make_app_state` 构造最小 AppState（project_root 注入）后经 `common::block_on` await 真实命令实现，覆盖 State 注入、路径沙箱拒绝、`spawn_blocking`、错误消息契约（"HEAD 中不存在"）。其余直接调用 git2 API 的测试为**底层原语**行为验证（非命令层）。

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

- **文件状态**（9 用例）：空仓库无文件、修改→`WT_MODIFIED`、新增未跟踪→`WT_NEW`、add→`INDEX_NEW`、删除→`WT_DELETED`、非仓库→Err、排除被忽略文件（`include_ignored` 未设置）、未忽略文件仍跟踪、已跟踪后被忽略仍显示状态

> **`include_ignored` 已移除**：`git_status` 不再调用 `.include_ignored(true)`。被 `.gitignore` 忽略的文件（如 `target/`、`node_modules/`）不再被 libgit2 扫描，`git_status` 仅返回 tracked + untracked 文件的状态。此举消除了大项目中 50K+ 文件扫描导致的 5-8 秒 I/O 饱和和 ~8MB JSON 主线程阻塞。`status_to_str` 的 `is_ignored()` 分支保留（无害死代码）。

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
- 系统需安装 `git` CLI，**最低版本 2.28**（`commit_file`/`git_add` 依赖 `git add` + `git commit`；`init.defaultBranch` 配置 2.28 引入）
- 不依赖 `[dev-dependencies]`——`tempfile`、`git2`、`serde_json` 均在 `[dependencies]` 中
