# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/fs` 是项目路径沙箱内的文件系统操作层。前端加载时序与沙箱的耦合、CRLF 行尾保持、大文件分块读取策略，无法从代码本身直接读出红线，需要文档化。

## 关键约束与决策

### `fs_read_dir` 游标分页（CP-006，2026-09；替代旧 BE-21「不分页整表返回」；游标自 BE-02 改 keyset）

`fs_read_dir(path, cursor?, limit?)` 返回 `FsReadDirPage { entries, nextCursor }`：

- `limit` 缺省 500 / 上限 1000，越界钳制 `[1, 1000]`；
- 游标 = **keyset**：上一页末条目排序键 `(isDir?0:1, 小写名)` 经 base64（`encode_page_cursor`/`decode_page_cursor`，`sort_key` 为比较基准）——**opaque**：客户端只回传不解读，解码失败返回 `Validation`；
- 续页定位 = `partition_point(sort_key <= 游标键)` 的严格大于后缀——目录在分页间变长时，排序在游标前的新增属「已翻过页」不重放、排序在游标后的新增出现在续页（无重复无遗漏，根治旧序号游标随新增平移）；
- `.git` 过滤与排序（文件夹→文件、同类型小写名称序）在整表收集**完成后**执行再切片——排序契约跨页稳定，按页序拼接即全量；
- `nextCursor: null` = 末页；游标越界（游标键大于现存全部条目）→ 空页 + null。
- **理论边界（登记）**：排序键 `(isDir, 小写名)` 冲突（同目录存在大小写变体同名条目）时，keyset 续页对同键后缀条目可能跳过/重复——Windows 默认大小写不敏感下该形态同目录不可创建，仅大小写敏感目录标志的边缘场景可触发，接受为已知边界。

增量拉取由前端续页拼接（loadRoot 首帧拉首页 + 后台续页；展开/刷新路径聚合读取），**不采用 Channel 推送**——拉取式分页已削峰，推送式增加前端状态机复杂度。FileTree 虚拟化（FE-30）渲染侧承接窗口数据，对分页契约透明。

### `fs_read_file` Channel 分块推送（BE-03）

- 先校验大小 ≤10MB，超限 `Err`；
- 按 `READ_CHUNK_BYTES = 256KB` 分块读取，每块回退到 UTF-8 char boundary 后再转 `String`；
- 发送序列 = 若干 `{data, done:false}` + 终态 `{data:"", done:true}`；
- 非法/残缺 UTF-8 → `Err`。

L1 用 send 回调注入收集，无需构造 `tauri::ipc::Channel`。

### `fs_read_resource` 二进制资源通道（ADR-0018）

docViewer 预览（md/html 本地相对图片等 data: URL 内联）按路径读任意二进制的通道：

- 沙箱与上限同 `fs_read_file`（`extract_root` + `validate_path_within_root` + 10MB）；
- 原字节按 `READ_CHUNK_BYTES` 分块 **base64 编码**推送（UTF-8 安全、二进制不受文本编码校验限制）；复用 `FsReadChunk` 载荷形态（data = base64 文本）；
- 发送序列契约同 fs_read_file；空文件直接终态；
- 不做 UTF-8 校验——与 fs_read_file 的文本语义刻意区分（`read_resource_base64_chunked` 独立核心）；
- MIME 推断在前端扩展名白名单（后端保持「读字节」单一职责）。

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

- **禁止无游标全量返回**：新增目录读取通道必须走游标分页契约（`fs_read_dir` 自 CP-006 起即游标形态）。
- **不要降低 10MB 上限或改分块大小**：前端 `readFile` Promise 拼接假设块大小与序列语义。
- **写文件必须保持原行尾**：否则每次保存都会把 CRLF 仓库刷成 LF。
- **新增文件系统命令必须走 `validate_path_within_root`**：所有命令共享路径沙箱。

## 测试模式

- **命令内核直测**：`fs_*_impl` 接收 `Option<PathBuf>` 根路径，用 `tokio Runtime::block_on` await，无需构造 `tauri::State`。
- **分块读取注入 send 回调**：L1 直接调用 `read_file_chunked(path, |chunk| { ... })` 收集块。
- **tempfile 隔离**：每个测试用 `tempfile::tempdir()`，结束时自动清理。
- **平台分支**：`new_file_defaults_to_crlf_on_windows` 用 `#[cfg(windows)]` / `#[cfg(not(windows))]` 守卫不同断言。
