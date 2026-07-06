# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件浏览器（ExplorerPanel）——左侧面板，展示当前活跃项目的文件树，支持 CRUD 操作、Git 状态着色和文件系统事件增量刷新。

## 架构决策

**Generation 异步取消**：切换项目页面时 `rootPath` 变化，`useFileTree` 的 effect 发起新 `loadRoot()` + `gitStatus()` 异步请求。旧 `rootPath` 的请求可能在新请求之后才返回，导致 `setRootNodes` 写入过期数据。`genRef` 计数器每次 `rootPath` 变化递增，旧请求的回调检查 `gen !== genRef.current` 后丢弃结果。

- 仅 `rootPath` effect 中的调用使用 generation
- `refreshExpanded`（CRUD 操作 / fs-event / file-saved 回调）不传 gen——它们操作的是当前页数据
- `loadRoot(gen?)` 的 `gen` 参数是可选的后向兼容接口

**`useFileTree` 自包含加载**：`rootPath` 变化时 `useFileTree` 内部 effect 自动调用 `loadRoot()` + `gitStatus()`。ExplorerPanel 只负责调用 CRUD 操作后的 `refresh()`，**不在 `rootPath` 变化时重复刷新**（历史重复 effect 已删除）。

**`handleOpenFile` 面板分派**：不再硬编码 `PANEL_EDITOR`，改为通过 `fileViewerRegistry.resolve(filePath)` 决定面板类型。命中策略（如 `.html` → `"htmlviewer"`）则用对应面板，返回 null 回退 `"editor"`。文件预览类面板（htmlviewer 等）通过 `isAlwaysRenderPanel()` 自动设置 `renderer: "always"` 保持 iframe browsing context 存活，避免页签切换白屏。新增文件预览类型无需修改 ExplorerPanel。

## 文件

| 文件 | 职责 |
|------|------|
| `ExplorerPanel.tsx` | React 容器组件：活跃项目推导、文件树渲染、CRUD 事件处理、`handleOpenFile` 面板分派（FileViewerRegistry）、`fs_watch` 启动 |
| `useFileTree.ts` | 文件树数据 hook：`loadRoot` / `loadDirectory` / `toggleExpand` / `refreshExpanded` / generation 取消 |
| `FileTree.tsx` | 递归树组件：节点渲染、git 状态着色、右键菜单 |
| `FileIcon.tsx` | 文件图标映射（扩展名→emoji） |

## 关键集成点

- **`src/ipc/fs.ts`** — `readDir` / `writeFile` / `createDir` / `deleteEntry` / `rename`
- **`src/ipc/git.ts`** — `gitStatus` 着色
- **`src/ipc/notify.ts`** — `startWatch` 启动后端监听 + `onFsEvent` 增量刷新
- **`src/stores/projects.ts` + `src/stores/layout.ts`** — 活跃项目 `rootPath` 推导
- **`src/workspace/titleManager.ts`** — 文件打开时计算编辑器页签标题 + 去重
- **`src/features/fileViewers/FileViewerRegistry.ts`** — 策略模式决定文件用哪个面板类型打开（`.html` → `"htmlviewer"`，未知 → `"editor"`）

## IPC 约束

- 所有文件操作经 `src/ipc/` 层调用，禁止组件内直接 `invoke`
- `onFsEvent` 通过 Tauri `listen()` 全局订阅，`useFileTree` 内 200ms debounce

## 测试模式

测试文件位于 `src/__tests__/`，命名规则 `explorer-*.test.tsx`。

### 技术栈

- Vitest（jsdom 环境）+ React Testing Library（`render` / `fireEvent` / `waitFor`）
- 不使用 Playwright / Cypress / 真实浏览器
- 使用 `React.createElement(Component, props)` 而非 JSX

### Mock 模式

**`vi.hoisted()` 必须**：所有 mock 状态在 `vi.hoisted()` 中创建，确保在模块级 `vi.mock()` 执行前就绪。

**三个 IPC 模块必须 mock**：
- `../ipc/fs` — `readDir`, `writeFile`, `createDir`, `deleteEntry`, `rename`
- `../ipc/git` — `gitStatus`
- `../ipc/notify` — `startWatch`, `onFsEvent`

**UI 弹窗 mock**（删除确认等）：
- `@tauri-apps/plugin-dialog` — `ask`

**Zustand stores** 使用真实实现（不 mock），通过 `.setState()` 种子数据，`beforeEach` 中重置。

### 组件渲染

**FileTree 独立渲染**：
```ts
function renderFileTree(nodes: TreeNode[], overrides = {}) {
  const props = { nodes, depth: 0, gitStatusMap: new Map(), ...overrides };
  return render(React.createElement(FileTree, props));
}
```

**ExplorerPanel 集成渲染**：需先种子 `useProjects` + `useLayout` + `window.__dockviewApi`。

### 右键菜单测试

- 触发：`fireEvent.contextMenu(element)` 找目标元素
- 菜单项定位：`getAllByText("标签名")`，注意 StrictMode 双渲染导致重复元素
- 点击菜单项：`fireEvent.click(items[0])`
- 内联输入框定位：`document.querySelectorAll('input')` 取最后一个（无 role/label）

### 异步断言

CRUD 操作涉及 IPC mock（Promise），使用 `await waitFor(() => { expect(...) })` 等待 mock 调用完成。
