# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Commit 侧栏视图——在活动栏中展示当前项目的 git 变更状态，按文件列表呈现 Changes（已跟踪变更）和 Unversioned Files（未跟踪文件），双击分派到对应面板类型。

## 架构决策

### 状态机（优先级自上而下）

CommitView 有四个渲染态，优先级从高到低：

| 状态 | 触发条件 | UI |
|------|---------|-----|
| `no-root` | `activePageId` 无对应 project | "选择一个项目以查看变更" |
| `loading` | `gitStatus` 调用中 | "加载中…" |
| `error` | `gitStatus` 抛异常（含非 git 仓库） | "当前项目并非 git 项目" |
| `ready` | `gitStatus` 成功返回 | Changes (N) + Unversioned Files (N) 两个可折叠列表 |

状态推导流程：`activePageId` → project → `rootPath` → `gitStatus(rootPath)` → 状态机分发。`rootPath` 变化时立即清空旧数据并重载（generation 取消模式，照 `useFileTree` 模式）。

### 数据加载与刷新（`useCommitStatus`）

- **rootPath 推导**：同 ExplorerPanel——从 `activePageId` 反查项目 cwd/rootPath 确认项目根目录
- **首次加载**：`gitStatus(rootPath)` 一次获取全量状态
- **自动刷新**：`onFsEvent` + 200ms debounce 自动重新 `gitStatus`
- **切换安全**：`genRef` 计数器 + rootPath 变化时立即设 `loading` 态，旧请求回调检查 generation 后丢弃

### 分派映射表（策略模式）

双击文件列表项时，根据 git 状态决定面板类型和页签后缀：

| git 状态 | 面板类型 | 页签后缀 | 说明 |
|---------|---------|---------|------|
| `added` | `editor` | `(git add)` | staged 新文件，用编辑器打开 |
| `untracked` | `editor` | `(git not add)` | 未跟踪文件，用编辑器打开 |
| `deleted` | `gitshow` | `(git delete)` | 已删除文件，用只读面板查看 HEAD 版本 |
| `modified` | `diff` | `(git diff)` | 修改文件，用双栏 diff 面板 |
| `renamed` | `diff` | `(git diff)` | 重命名文件，用双栏 diff 面板（传 oldPath） |
| `conflict` | `diff` | `(git diff)` | 冲突文件，用双栏 diff 面板 |

映射表独立导出为 `STATUS_PANEL_MAP`（`openCommitFile.ts`），策略模式，新增状态→面板映射只需追加条目。

### 去重聚焦

`openCommitFile` 打开前先 `titleManager.findExistingEditor(pageId, filePath, suffix)` 查重——传入 suffix 时仅匹配同 suffix 条目，防止已有普通编辑器被误聚焦（B10）。命中则 `panel.focus()` 不新建。未命中则 `addPanel` → `registerEditor` → `recomputeTitles`。

### 文件列表排序与着色

- 排序：按完整相对路径字母序（`relativePath` 或 `path`）
- 文件名色：`GIT_FILE_COLORS[status]`（硬约束 #6，从 `theme/colors.ts` token 引用）
- 父目录后缀：灰色 `INPUT_BORDER`，紧接文件名右侧显示相对目录路径
- 可折叠：每列表标题栏点击折叠/展开，状态不持久化
- 空态：展开时条目数为 0 显示"无变更文件"

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export：CommitView、CommitFileList、useCommitStatus、openCommitFile、STATUS_PANEL_MAP |
| `CommitView.tsx` | 主组件：标题栏 "COMMIT"（28px、大写、照 ExplorerPanel 样式）+ 状态机渲染四态 + Changes/Unversioned Files 两个 CommitFileList |
| `CommitFileList.tsx` | 可折叠文件列表：标题栏（N 计数）+ 展开/折叠 + 文件名+父目录渲染 + 双击分派 |
| `useCommitStatus.ts` | 数据加载 hook：rootPath 推导 → gitStatus + onFsEvent 200ms debounce + generation 取消 |
| `openCommitFile.ts` | 双击分派：`STATUS_PANEL_MAP` 状态→面板映射表 + `openCommitFile`（照 ExplorerPanel.handleOpenFile 流程：去重聚焦 → addPanel → registerEditor → recomputeTitles） |

## 关键集成点

- **`src/ipc/git.ts`** — `gitStatus` 全量文件状态
- **`src/ipc/notify.ts`** — `onFsEvent` 文件变更自动刷新
- **`src/stores/projects.ts` + `src/stores/layout.ts`** — 活跃项目 rootPath 推导
- **`src/workspace/titleManager.ts`** — 页签标题（含 suffix）+ 去重聚焦
- **`window.__dockviewApi`** — addPanel/getPanel/focus
- **`src/features/sideViews/sideViewDefs.ts`** — CommitView 注册为 `commit` 视图（id: "commit", icon: "🔀"）
- **`src/theme/colors.ts`** — `GIT_FILE_COLORS` 文件名着色（硬约束 #6）

## 测试模式

测试文件：`src/__tests__/commit-view.test.tsx`（28 用例）。

### 技术栈

- Vitest（jsdom 环境）+ React Testing Library（`render` / `fireEvent` / `waitFor`）
- mock `../ipc/git`（`gitStatus`）+ `../ipc/notify`（`onFsEvent`）
- mock `dockview-react` + `titleManager` stub

### 测试覆盖

- **状态机四态**：no-root / loading / error / ready 各自渲染验证
- **列表渲染**：文件名着色 token、计数显示、按相对路径字母序排序、空态"无变更文件"
- **折叠交互**：点击标题栏折叠/展开，条目数不变
- **双击分派**：四种状态（added/untracked/deleted/modified）分别验证 addPanel 调用的 component/title/params
- **去重聚焦**：已有同文件+同 suffix 面板时 focus 不新建；同文件不同 suffix 不误匹配
- **fs-event 刷新**：fake timers 验证 200ms debounce 后 gitStatus 重调
- **rootPath 切换**：切换项目清空旧数据 + generation 丢弃旧结果

### 运行

```bash
npm test                                    # L2 全量
npx vitest run commit-view                  # 仅 commit 视图测试
```
