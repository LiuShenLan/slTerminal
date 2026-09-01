# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/fs` 是项目路径沙箱内的文件系统操作层。前端加载时序与沙箱的耦合、CRLF 行尾保持、大文件分块读取策略，无法从代码本身直接读出红线，需要文档化。

## 关键约束与决策

### `fs_read_dir` 不分页（BE-21）

返回整个目录列表，无分页。收益：FileTree 虚拟化在渲染侧处理万级节点；改分页会破 IPC 契约。如评估 FE-30 失效场景再议。

### `fs_read_file` Channel 分块推送（BE-03）

- 先校验大小 ≤10MB，超限 `Err`；
- 按 `READ_CHUNK_BYTES = 256KB` 分块读取，每块回退到 UTF-8 char boundary 后再转 `String`；
- 发送序列 = 若干 `{data, done:false}` + 终态 `{data:"", done:true}`；
- 非法/残缺 UTF-8 → `Err`。

L1 用 send 回调注入收集，无需构造 `tauri::ipc::Channel`。

### CRLF 行尾保持

`fs_write_file` 写盘前检测原文件样本（前 `CRLF_SAMPLE_MAX_BYTES` 字节）：
- 原文件含 CRLF → 写入内容统一转 CRLF；
- 原文件 LF → 保持 LF；
- 新文件 → Windows 默认 CRLF，其他平台 LF。

### 路径沙箱校验范围

- `fs_read_file` / `fs_read_dir` / `fs_create_dir` / `fs_delete` / `fs_rename`：校验目标路径；
- `fs_write_file`：校验父目录（文件可能尚不存在）。

### 前端必须保证 `project_root` 已设置

`validate_path_within_root` 对 `project_root=None` 一律拒绝（`cfg!(test)` 豁免）。调用下列命令前前端必须先完成 `setProjectRoot`：
- 用户点击侧栏页面：`Workspace.switchToPage` 先 await `setProjectRoot` 再 `setActivePage`（DBG-5）；
- 应用启动恢复 lastPage：`App.tsx` 先 await `setProjectRoot` 再 `setActivePage`（DBG-6）；
- E2E helper：`__slterm_e2e_createProject` / `__slterm_e2e_switchToPage` 内部先 await `setProjectRoot`（DBG-8）。

> React effect 时序坑：同一 commit 的 passive effect 子组件先于父组件执行。若 `setProjectRoot` 只在父 effect fire-and-forget，子组件（如 `ExplorerPanel` → `useFileTree` → `readDir`）会在 root 到达后端前被沙箱拒绝。

## 外部坑/红线

- **禁止给 `fs_read_dir` 加分页**：除非同步改前端 FileTree 虚拟化与 IPC 契约。
- **不要降低 10MB 上限或改分块大小**：前端 `readFile` Promise 拼接假设块大小与序列语义。
- **写文件必须保持原行尾**：否则每次保存都会把 CRLF 仓库刷成 LF。
- **新增文件系统命令必须走 `validate_path_within_root`**：所有命令共享路径沙箱。

## 测试模式

- **命令内核直测**：`fs_*_impl` 接收 `Option<PathBuf>` 根路径，用 `tokio Runtime::block_on` await，无需构造 `tauri::State`。
- **分块读取注入 send 回调**：L1 直接调用 `read_file_chunked(path, |chunk| { ... })` 收集块。
- **tempfile 隔离**：每个测试用 `tempfile::tempdir()`，结束时自动清理。
- **平台分支**：`new_file_defaults_to_crlf_on_windows` 用 `#[cfg(windows)]` / `#[cfg(not(windows))]` 守卫不同断言。
