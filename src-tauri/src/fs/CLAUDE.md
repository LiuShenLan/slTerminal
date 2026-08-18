# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统操作模块——封装 `fs_read_dir`、`fs_read_file`、`fs_write_file`、`fs_create_dir`、`fs_delete`、`fs_rename` Tauri 命令，适配 Windows 行尾和路径沙箱。路径沙箱核心函数 `validate_path_within_root` 已迁移至 `state.rs`，本模块通过 `use crate::state::validate_path_within_root` 导入。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | 6 条 Tauri 命令（`fs_read_dir`/`fs_read_file`/`fs_write_file`/`fs_create_dir`/`fs_delete`/`fs_rename`）+ 命令内核 `fs_*_impl` + CRLF 行尾适配（读保留原格式、新文件平台默认行尾） |

## 决策记录

### BE-21 豁免：`fs_read_dir` 不分页（登记豁免）

`fs_read_dir` 返回整个目录列表，无分页。**登记豁免**（08 P2-3）：懒加载按目录分层 + FileTree 虚拟化（FE-30）覆盖渲染侧（万级节点滚动/展开窗口化），单层万级文件罕见；改分页 = IPC 契约破坏性变更，收益不抵成本。**改此决策需先评估 FE-30 失效场景**。

### BE-03：`fs_read_file` Channel 分块推送

`fs_read_file(path, onChunk: Channel<FsReadChunk>) -> Result<(), AppError>`（S07）——先 metadata 校验大小 ≤10MB（超限 Err，行为同现状），再按 **256KB 分块**（`READ_CHUNK_BYTES`）读取推送。`FsReadChunk { data: String, done: bool }`——发送序列 = 若干 `{data, done:false}` + 终态 `{data:"", done:true}`。**UTF-8 边界**：按字节读块后回退到 char boundary 再转 String（完整多字节字符不跨块）。前端 `readFile(path): Promise<string>` 签名不变（`src/ipc/fs.ts` 内部 Channel 监听拼接），消费方（DiffPanel/HtmlPanel/useCodeMirror）零适配。L1 用 send 回调注入收集（`tauri::ipc::Channel` 无法在 L1 构造）。

## 路径沙箱对前端加载时序的要求

`validate_path_within_root`（`state.rs`）对 `project_root=None` 一律拒绝访问（非 `cfg!(test)` 路径）。覆盖全部 10 个命令：`fs_read_file`/`fs_write`/`fs_read_dir`/`fs_create_dir`/`fs_delete`/`fs_rename`（本模块）、`notify_watch`（`notify/mod.rs`）、`git_status`/`git_diff`（`git/mod.rs`）、`pty_spawn` 的 cwd（`pty/spawn.rs`）。

**前端消费方必须保证 `project_root` 已在后端设置**，方可调用上述命令。当前保障路径：

| 触发路径 | 保障方式 |
|----------|---------|
| 用户点击侧栏页面 | `Workspace.switchToPage`（async）先 `await setProjectRoot(rootPath)` 再 `setActivePage`（DBG-5） |
| 应用启动恢复 lastPage | `App.tsx` 先 `await setProjectRoot` 再 `setActivePage`（DBG-6） |
| E2E helper 创建项目/切换页面 | `__slterm_e2e_createProject`/`__slterm_e2e_switchToPage` 内部先 `await setProjectRoot`（DBG-8） |
| SEC-01 effect 兜底 | `Workspace.tsx` effect 中 fire-and-forget `setProjectRoot`（保留服务 `pty_spawn` 等非 Explorer 链路） |

> **React effect 时序坑**：同一 commit 的 passive effect 子组件先于父组件执行。若 `setProjectRoot` 仅在父 effect 中 fire-and-forget，子组件（如 `ExplorerPanel` → `useFileTree` → `readDir`）必在 `set_project_root` 到达后端前被 sandbox 拒绝——这不是概率竞态，是确定性失败（`loadDirectory` catch 静默吞错 → 文件树恒"空目录"）。

## 测试模式

测试位于 `fs/mod.rs` 的三个 `#[cfg(test)]` 模块：`read_dir_tests` + `write_file_tests` + `command_wrapper_tests`（共 43 用例）。sandbox 测试位于 `state.rs`（42 条 `#[test]`，含 symlink 豁免测试）。

### 分块读取测试（BE-03，S07）

`read_file_chunked` 为同步核心，L1 直接调（send 回调注入收集，无需构造 `tauri::ipc::Channel`）：

- `test_read_file_chunked_multi_chunk_joins_correctly` — 600KB 内容跨 3 块，拼接与原文一致
- `test_read_file_chunked_utf8_boundary_not_split` / `_4byte_boundary_not_split` — 多字节字符（3/4 字节 UTF-8）不跨块拆分
- `test_read_file_chunked_over_limit_rejected` / `_at_limit_allowed` — 10MB 上限边界
- `test_read_file_chunked_empty_file_terminal_only` — 空文件仅终态块
- `test_read_file_chunked_invalid_utf8_rejected` / `_incomplete_tail_rejected` — 非法 UTF-8 拒绝

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
- `command_wrapper_tests`（TE-14/HFN-04/HFN-08）：**命令内核层**——直接调 `fs_*_impl`（root 传 `Option<PathBuf>`），`run()` 用 `tokio Runtime::block_on` await，无需构造 `tauri::State`（State 仅为命令包装层提取 root 的通道）。覆盖参数透传、错误映射（`spawn_blocking` panic → `AppError`）、sandbox 校验分支（root 外拒绝、HFN-04 删除不存在路径）、SEC-04 `fs_rename` 覆盖已有文件/拒绝已有目录、**BE-13 路径上下文注入（`test_fs_read_file/read_dir/write_file/rename_error_message_contains_path`——错误消息含路径）**

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
