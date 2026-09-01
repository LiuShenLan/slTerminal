# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/git` 基于 `git2` 封装状态、diff、HEAD 读取、回滚与取消暂存命令。本模块的决策（LRU 仓库缓存、不用 checkout API、忽略文件排除策略、路径短名处理、HEAD 错误契约）受 libgit2/Windows/CI 环境限制，无法从代码本身读出，需要文档化。

## 关键约束与决策

### 仓库缓存 `get_or_open_repo`（BE-09）

`AppState.git_repo_cache` 是 workdir → `Repository` 的容量 8 LRU：
- 命中时 touch MRU；超容量淘汰 LRU；
- 命中条件为「搜索路径落在缓存 workdir 子树内」的前缀匹配，支持跨项目切换后子树命中；
- 命中后仍从磁盘 `Repository::open` 返回独立实例，缓存只保留可复用句柄；
- `discover` 到的 workdir 须经 `validate_path_within_root` 校验，防止上溯到 project_root 外的父仓库。

### `git_status` 不再扫描被忽略文件

`StatusOptions` 已移除 `.include_ignored(true)`，仅返回 tracked + untracked。原因：大项目中 50K+ 被忽略文件会导致数秒 I/O 阻塞和数 MB JSON 主线程卡顿。`status_to_str` 的 `is_ignored()` 分支保留为无害死代码。

### rename 检测必须开启

`StatusOptions` 开启 `.renames_head_to_index(true)` 与 `.renames_index_to_workdir(true)`，否则 `INDEX_RENAMED`/`WT_RENAMED` 不置位、`oldPath` 恒 null。

`compute_diff_hunks`（git_diff）侧：`DiffOptions` **不用 pathspec**——libgit2 在生成期即按 pathspec 过滤 delta，DELETED(旧路径) 在 `find_similar` 前被剔除，工作区 rename 无法合并。改为生成后命令层按 `delta.new_file().path()` 过滤（renamed 的 new 侧 = 工作区新路径）。过滤前必须先 `diff.find_similar(renames(true) + for_untracked(true))`：两者缺一不可——`renames(true)` 置位后 libgit2 跳过 repo config `diff.renames` 读取（行为确定；**勿调 `by_config()`**，其清 flags 恢复 config 读取）；`for_untracked(true)` 允许 untracked delta 作 rename 目标（工作区 rename 的新文件在 git 视角为 untracked，默认不参与合并，探针实证）。纯 rename（内容未变）合并后 0 hunk——对齐 git diff 语义。低相似度（<50%）不合并，仍走 untracked 全量新增，与 git status 一致。

### `GitStatusEntry.path` 语义 = 当前工作区路径（对齐 git status）

git2-rs 的 `StatusEntry::path_bytes()` 两个分支均返回 `delta.old_file.path`——对 renamed 条目即旧路径。命令层必须对 renamed 改取 `delta.new_file().path()`（INDEX_RENAMED → head_to_index，WT_RENAMED → index_to_workdir，双标志 INDEX 优先），非 renamed 保持 `entry.path()`。`old_path` 恒取 `old_file().path()`。前端 gitStatusMap / commit 列表 / DiffPanel（`oldPath ?? filePath` 查 HEAD 侧）均按此契约设计。

### `git_rollback` 不用 checkout API

`checkout_head`/`checkout_index`/`reset_default` 在 Windows `core.autocrlf=true` 仓库中对单文件的 index 持久化行为不一致，实测回滚后 `statuses()` 仍报告 dirty。当前实现：
1. `std::fs::write` 把 HEAD blob 原始字节写回工作区；
2. `index.add_path(rel)` 重建 index 条目，同步 stat 与哈希。

这保证 HEAD、工作区、index 三方字节一致，`statuses()` stat 快速路径必定命中。

### `git_unstage` = `git reset HEAD -- <file>`

从 index 移除条目（`index.remove_path`），不依赖 HEAD 中存在该文件。用于 commit view 中 added 文件的删除场景。

### HEAD 不存在错误契约

`git_file_at_head` 与 `git_rollback` 对以下情况返回 `AppError::Git`，消息含 `"HEAD 中不存在"`：
- `UnbornBranch`（仓库尚无提交）；
- 文件不在 HEAD tree 中。

前端统一显示占位文案，不解析具体错误内容。

### 路径沙箱与已删除文件

`validate_path_within_root` 使用 `canonicalize_or_ancestor`：目标不存在时上溯到最近存在的祖先目录，canonicalize 后再拼接剩余部分校验。已删除文件仍可被 diff/HEAD/rollback 请求校验通过。

### `dunce::simplified` 处理 workdir

`repo.workdir()` 可能带 `\\?\` 前缀。`get_or_open_repo` 返回前用 `dunce::simplified` 剥离，确保与 fs 模块返回的绝对路径格式一致。

## 外部坑/红线

- **8.3 短名坑（CI 必踩）**：GitHub runner 的 `%TEMP%` 是短名 `RUNNER~1`，git2 workdir 返回长名，直接 `strip_prefix` 会失败。`tests/common/mod.rs` 的 `init_temp_repo` 用 `dunce::canonicalize` 把 tempdir 转长名；所有 strip_prefix 站点消费 `dunce::simplified` 后的路径。
- **git CLI 最低 2.28**：`init_temp_repo` 用 `git -c init.defaultBranch=main init`，该配置 2.28 引入；早期版本会静默忽略并默认 `master`。
- **不要恢复 `include_ignored(true)`**：会重新引入大项目扫描阻塞。
- **不要改用 checkout API 做 rollback**：Windows autocrlf 场景会导致 index 不一致。
- **测试二进制 comctl32 v6 激活**：链接 tauri 的测试目标需要 SxS v6 manifest，否则启动即 `0xc0000139`（`STATUS_ENTRYPOINT_NOT_FOUND`）。`build.rs` 对测试目标注入 `/MANIFEST:EMBED` + `/MANIFESTINPUT:tests-comctl6.manifest`。

## 测试模式

- **集成测试位于 `src-tauri/tests/`**：`git_status_tests.rs`、`git_diff_tests.rs`、`git_file_at_head_tests.rs`、`git_rollback_tests.rs`、`git_unstage_tests.rs`、`git_command_shell_tests.rs`、`ci_config_tests.rs`。
- **共享工厂 `tests/common/mod.rs`**：`init_temp_repo`、`commit_file`、`git_add`、`make_app_state`、`block_on`。
- **每个测试独立 `tempdir` + `git init`**，不共享仓库。
- **命令层测试**：直接 await `git_*_impl(&app, ...)`，用 `make_app_state` 注入最小 `AppState`。
- **命令壳测试（TQ-COV-06）**：用 `tauri::test::mock_builder` 构造 mock App + `app.state::<AppState>()`，验证 `#[tauri::command]` 壳转发契约。
- **L1 必须 `--test-threads=1`**：`ci_config_tests.rs` 的 `ci_l1_uses_single_test_thread` 锁死此配置。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| git2 API 失败 map_err 分支 | 依赖 libgit2 内部 API 失败，L1 无法注入 | 核心路径已由真实仓库操作覆盖 |
| 仓库缓存 Mutex 中毒分支 | 锁内无 panic 路径 | 未来引入锁内 panic 时须换原语或补测试 |
| `get_or_open_repo` discover 成功后的 open 失败 | 竞态窗口不可注入 | 正常路径已由集成测试覆盖 |
