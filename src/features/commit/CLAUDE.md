# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

Commit 侧栏视图在活动栏中展示当前项目的 git 变更状态，按 Changes / Unversioned Files 两个可折叠列表呈现，双击文件按状态分派到对应面板类型（editor/gitshow/diff）。

## 关键约束与决策

### 状态机（优先级自上而下）

CommitView 四渲染态：

| 状态 | 触发条件 | UI |
|------|---------|-----|
| `no-root` | `activePageId` 无对应 project | "选择一个项目以查看变更" |
| `loading` | `gitStatus` 调用中 | "加载中…" |
| `error` | `gitStatus` 抛异常（含非 git 仓库） | "当前项目并非 git 项目" |
| `ready` | `gitStatus` 成功返回 | Changes (N) + Unversioned Files (N) |

状态推导：`activePageId` → project → `rootPath` → `gitStatus(rootPath)`。`rootPath` 变化时立即清空旧数据并重载（generation 取消模式，照 `useFileTree`）。

### 数据加载与刷新（`useCommitStatus`）

- **rootPath 推导**：同 ExplorerPanel——从 `activePageId` 反查项目 `cwd || rootPath`。
- **首次加载**：`gitStatus(rootPath)` 一次获取全量状态。
- **自动刷新**：`onFsEvent` + 200ms debounce 重新 `gitStatus`。
- **切换安全**：`genRef` 计数器 + rootPath 变化时立即设 `loading` 态，旧请求回调检查 generation 后丢弃。
- **手动刷新**：暴露 `refresh()` 强制重载。

### 分派映射表（策略模式）

双击文件列表项时，根据 git 状态决定面板类型和页签后缀：`STATUS_PANEL_MAP`（`openCommitFile.ts`）独立导出。

| git 状态 | 面板类型 | 页签后缀 |
|---------|---------|---------|
| `added` | `editor` | `(git add)` |
| `untracked` | `editor` | `(git not add)` |
| `deleted` | `gitshow` | `(git delete)` |
| `modified`/`renamed`/`conflict` | `diff` | `(git diff)` |

renamed 状态传 `oldPath` 给 diff 面板，用于 HEAD 侧查询旧路径内容。

### 去重聚焦（B10）

`openCommitFile` 打开前先 `titleManager.findExistingEditor(pageId, filePath, suffix)` 查重——传入 suffix 时仅匹配同 suffix 条目，防止已有普通编辑器被误聚焦。命中则 `panel.focus()` 不新建。未命中则 `addPanel` → `registerEditor` → `recomputeTitles`。

### 文件列表排序与着色

- 排序：按完整相对路径字母序。
- 文件名色：`GIT_FILE_COLORS[status]`（硬约束 #6，从 `theme/colors.ts` token 引用）。
- 父目录后缀：灰色 `INPUT_BORDER`，紧接文件名右侧显示相对目录路径。
- 可折叠：每列表标题栏点击折叠/展开，状态不持久化。
- 空态/状态提示文字色 = `DIM_FG`（人工验证问题 4 修订——原误用 `INPUT_BORDER`）。

### 右键菜单策略（`commitContextMenu.ts`）

```
ROLLBACK_STATES = {modified, deleted, renamed, conflict}  → "回滚"
DELETE_STATES   = {added, untracked}                       → "删除"
```

- **回滚**：`confirmDialog` 确认 → `gitRollback` → `refresh()`。
- **删除(added)**：`confirmDialog` 确认 → `gitUnstage` → `deleteEntry` → `refresh()`。
- **删除(untracked)**：`confirmDialog` 确认 → `deleteEntry` → `refresh()`。
- `CommitFileList.tsx` 不 import 任何 git/fs IPC 模块——只调用 `getContextMenuItems()` 策略函数。

### git_rollback 实现演进

四次迭代修复 Windows `core.autocrlf=true` 仓库中回滚后 `statuses()` 仍报告 dirty 的问题：

1. `std::fs::write(blob)` — 写 LF 到磁盘，index 未更新 → status 不干净。
2. `checkout_head(.path().force())` — git2 checkout API，index 持久化不可靠。
3. `checkout_head + index.write()` — 仍不可靠。
4. `reset_default(HEAD, path) + checkout_index(None, opts)` — 两步法，各自独立操作磁盘。
5. **最终方案**：`std::fs::write(blob) + index.add_path(path) + index.write()` — 写原始 HEAD blob 字节 + 重建 index 条目（同步 stat/哈希），三方完全一致。

核心教训：git2 checkout API 在 Windows autocrlf 仓库中对单个文件的 index 持久化和 smudge filter 行为不一致——直接写字节 + 重建 index 是唯一可靠路径。

## 外部坑/红线

- **右键菜单用 `confirmDialog`，不用 `dialog.ask`**：`ask` 已随 OV-02 删除，统一浮层入口在 `src/lib/ConfirmDialog`。
- **回滚的 autocrlf 陷阱**：Windows 仓库若用 git2 checkout API，回滚后 status 仍 dirty。必须写字节 + 重建 index。
- **去重必须传 suffix**：同文件不同 suffix（普通编辑器 vs git diff）是不同面板，不传 suffix 会误聚焦。
- **renamed 传 oldPath**：diff 面板 HEAD 侧需要旧路径查询 HEAD 内容。
- **空态色用 `DIM_FG`**：不要用 `INPUT_BORDER`（近乎不可见）。

## 测试模式

- **状态机**：覆盖 no-root / loading / error / ready。
- **列表**：覆盖文件名着色 token、计数、字母序排序、空态、折叠交互、fs-event 200ms debounce、rootPath 切换清空 + generation 丢弃。
- **右键菜单**：覆盖 ROLLBACK/DELETE 状态集与菜单项构造；外点关闭、菜单项点击 → confirmDialog → IPC → refresh 链路。
- **双击分派**：覆盖四种状态 addPanel 参数、去重聚焦（B10 反向用例——同文件不同 suffix 不误匹配）。
- **Mock**：`../ipc/git`（`gitStatus`）、`../ipc/notify`（`onFsEvent`）、`dockview-react`、`titleManager`。
