# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统操作模块——封装 `fs_read_dir`、`fs_read_file`、`fs_write_file`、`fs_create_dir`、`fs_delete`、`fs_rename` Tauri 命令，适配 Windows 行尾和路径沙箱。路径沙箱核心函数 `validate_path_within_root` 已迁移至 `state.rs`，本模块通过 `use crate::state::validate_path_within_root` 导入。

## 路径沙箱对前端加载时序的要求

`validate_path_within_root`（`state.rs`）对 `project_root=None` 一律拒绝访问（非 `cfg!(test)` 路径）。覆盖全部 10 个命令：`fs_read_file`/`fs_write`/`fs_read_dir`/`fs_create_dir`/`fs_delete`/`fs_rename`（本模块）、`notify_watch`（`notify/mod.rs`）、`git_status`/`git_diff`（`git/mod.rs`）、`pty_spawn` 的 cwd（`pty/spawn.rs`）。

**前端消费方必须保证 `project_root` 已在后端设置**，方可调用上述命令。当前保障路径：

| 触发路径 | 保障方式 |
|----------|---------|
| 用户点击侧栏页面 | `Workspace.switchToPage`（async）先 `await setProjectRoot(rootPath)` 再 `setActivePage`（DBG-5） |
| 应用启动恢复 lastPage | `App.tsx` 先 `await setProjectRoot` 再 `setActivePage`（DBG-6） |
| E2E helper 创建项目/切换页面 | `__slterm_e2e_createProject`/`__slterm_e2e_switchToPage` 内部先 `await setProjectRoot`（DBG-8） |
| SEC-01 effect 兜底 | `Workspace.tsx` effect 中 fire-and-forget `setProjectRoot`（保留服务 `pty_spawn` 等非 Explorer 链路） |

> **React effect 时序坑**：同一 commit 的 passive effect 子组件先于父组件执行。若 `setProjectRoot` 仅在父 effect 中 fire-and-forget，子组件（如 `ExplorerPanel` → `useFileTree` → `readDir`）必在 `set_project_root` 到达后端前被 sandbox 拒绝——这不是概率竞态，是确定性失败。详见 `docs/debug/refactor-regressions.md` 故障A。

## 测试模式

测试位于 `fs/mod.rs` 的两个 `#[cfg(test)]` 模块：`read_dir_tests` + `write_file_tests`（共 16 用例）。sandbox 测试现位于 `state.rs`（15 条 `#[test]`）。

### tempfile 隔离

所有文件系统测试使用 `tempfile::tempdir()` 创建隔离的临时目录，测试结束时自动清理：

```rust
let dir = tempfile::tempdir().unwrap();
let file = dir.path().join("test.txt");
std::fs::write(&file, "hello").unwrap();
// ... test operations ...
// dir 被 drop，所有临时文件自动删除
```

### 测试组织

- `read_dir_tests`：列出子项、过滤 `.git`（仅此一个硬编码过滤，`node_modules`/`target` 等依赖懒加载控制性能）、创建目录（单层/嵌套）、删除（文件/递归目录）、重命名、空目录边界、构建产物目录可见
- `write_file_tests`：CRLF 行尾保留（CRLF→CRLF、LF→LF）、新文件平台默认行尾（Windows CRLF / Unix LF）、混合行尾归一化

### CRLF 行尾保留测试

项目约束：编辑器保存文件时必须保持原行尾格式。测试验证：

```rust
// CRLF 文件改写后仍是 CRLF
let content = b"line1\r\nline2\r\n";
std::fs::write(&path, content).unwrap();
write_file(&path, content)?; // 项目自己的 write
assert_eq!(std::fs::read(&path)?, content);

// LF 文件改写后仍是 LF
let content = b"line1\nline2\n";
// ... 同上 ...
```

### 平台分支

- `new_file_defaults_to_crlf_on_windows`：使用 `#[cfg(windows)]` / `#[cfg(not(windows))]` 守护不同断言
- 其余测试平台无关（tempfile 跨平台工作）
