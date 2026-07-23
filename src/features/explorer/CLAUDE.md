# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件浏览器（ExplorerPanel）——左侧面板，展示当前活跃项目的文件树，支持 CRUD 操作、Git 状态着色和文件系统事件增量刷新。

## 架构决策

**Generation 异步取消 + rootPath 清空**：切换项目页面时 `rootPath` 变化，effect 首**先同步清空** `rootNodes`（设为 `[]`）和 `gitStatusMap`（设为空 `Map`），立即消除旧项目文件树残留。之后发起新 `loadRoot(gen)` + `gitStatus(rootPath)` 异步请求。`genRef` 计数器每次 `rootPath` 变化递增，旧请求的回调检查 `gen !== genRef.current` 后丢弃结果。

- rootPath 为 `null` 时也立即清空，不做 IPC 调用
- 清空是同步的——`setRootNodes([])` 与后续 `setRootNodes(nodes)` 在同一渲染批次，中间不可见
- `rootPath` 不变时不触发清空（React deps 未变化）
- 仅 `rootPath` effect 中的调用使用 generation
- `refreshExpanded`（CRUD 操作 / fs-event / file-saved 回调）不传 gen——它们操作的是当前页数据
- `loadRoot(gen?)` 的 `gen` 参数是可选的后向兼容接口

**`useFileTree` 自包含加载**：`rootPath` 变化时 `useFileTree` 内部 effect 自动调用 `loadRoot()` + `gitStatus()`。ExplorerPanel 只负责调用 CRUD 操作后的 `refresh()`，**不在 `rootPath` 变化时重复刷新**（历史重复 effect 已删除）。

**刷新保留展开状态（`reloadPreservingExpanded`）**：文件变更刷新（Ctrl+S 的 `slterm:file-saved`、fs-event、CRUD 后 `refresh()`）统一走 `refreshExpanded`。旧实现直接 `loadRoot()` 整树替换 → `loadDirectory` 硬编码 `expanded: false` → 丢弃全部展开状态（任何文件变更都折叠整棵树）。现改为 `reloadPreservingExpanded` 递归重载：以旧树（经 `rootNodesRef` 镜像读取）为蓝图，边重建边比对——每层用 `Map<path, oldNode>` 匹配，命中 `old.expanded && isDir` 则递归 `readDir` 下钻并保留 `expanded=true`，天然支持任意深度。既保留展开状态，又反映子目录内文件增删。

- 已展开目录被磁盘删除 → 父层 `readDir` 不再返回该项，节点连子树自然消失（复用 `loadDirectory` 容错，无需特判）。
- 已展开子目录 `readDir` 抛错 → `loadDirectory` catch 返回 `[]`，该目录 children 为空、不冒泡。
- `reloadPreservingExpanded` **不传 gen**（操作当前页数据），不碰 `loadRoot` 的 generation 取消机制。`loadRoot`/`fullRefresh` 保持整树替换语义不变。
- 权衡：深层多展开分支会并发多次 `readDir`（每展开层一次），fs-event 路径有 200ms 去抖限流，file-saved 路径无去抖。

**`handleOpenFile` 面板分派**：不再硬编码 `PANEL_EDITOR`，改为通过 `fileViewerRegistry.resolve(filePath)` 决定面板类型。命中策略（如 `.html` → `"htmlviewer"`）则用对应面板，返回 null 回退 `"editor"`。文件预览类面板（htmlviewer 等）通过 `isAlwaysRenderPanel()` 自动设置 `renderer: "always"` 保持 iframe browsing context 存活，避免页签切换白屏。新增文件预览类型无需修改 ExplorerPanel。

### 选中模型：单击选中 + 双击打开 + 空白取消

`selectedPath` state 由 ExplorerPanel 管理，通过 props 传入 FileTree。行为：
- **单击文件/文件夹** → `onSelect(path)` + `container.focus()`（建立焦点上下文）
- **双击文件** → `onOpenFile(path)` 打开编辑器（行为不变）
- **单击目录** → `onSelect(path)` + `onToggleExpand(path)`（选中 + 展开折叠）
- **单击空白** → `onSelect(null)` 取消选中
- **焦点离开** → 选中态保留（视觉高亮不变），但 `usePanelFocus` 的 `popContext("explorer")` 阻止快捷键在失焦时误触发

高亮色：`EXPLORER_SELECTION_BG`（`#094771`，VS Code list activeSelectionBackground 风格），在 `theme/colors.ts` 单点定义。
`onMouseEnter`/`onMouseLeave` 仅在非选中时动态设置背景——选中态不被 hover 覆盖。

### 焦点管理：tabIndex={-1} + usePanelFocus("explorer")

ExplorerPanel 的文件树容器 `<div ref={containerRef} tabIndex={-1}>` 可编程聚焦（不参与 Tab 序）。`usePanelFocus("explorer", containerRef.current, activate, deactivate)` 监听 focusin/focusout：

- **focusin** → `pushContext("explorer")` + `activate()` → `setActiveExplorer(explorerActions)`
- **focusout**（离开子树） → `popContext("explorer")` + `deactivate()` → `clearActiveExplorer(explorerActions)`
- **卸载** → 同上清理

### 快捷键集成：active pointer + 命令工厂

Explorer 的键盘快捷键遵循与 terminal/editor 相同的模式：

1. **`activeExplorer.ts`**（`createActivePointer<ExplorerActions>()`）：模块级指针，`setActiveExplorer`/`clearActiveExplorer`/`getActiveExplorer`
2. **`keyboard.ts`**（`createExplorerShortcuts()`）：返回 3 条 `commandFromMeta(id, handler)` 命令——`explorer.delete`（Delete）/ `explorer.open`（Enter）/ `explorer.rename`（F2）
3. 命令在 `App.tsx` 一次性注册：`registry.register([...createExplorerShortcuts()])`
4. handler 经 `getActiveExplorer()` 派发到聚焦实例

`ExplorerActions` 接口：
```ts
{ getSelectedPath(): string | null
; deleteSelected(): Promise<void>
; openSelected(): void
; renameSelected(): void
; isRenaming(): boolean
}
```

**重命名中透传**：当 `isRenaming()=true`（内联 input 活跃），Enter 和 Del 命令 handler 返回 `false` 透传，让 input 处理 Enter 确认。

### ref 模式（actions 闭包过期修复）

`explorerActions` 的 `useMemo` 使用**空依赖**（`[]`），所有数据通过 ref 间接访问——对齐 terminal/editor 的 ref 模式：

- `selectedPathRef` 每次渲染同步 `selectedPathRef.current = selectedPath`
- `getSelectedPath: () => selectedPathRef.current` 运行时读取最新值
- `deleteSelected`/`openSelected`/`renameSelected`/`isRenaming` 各自通过 `xxxRef.current` 间接调用
- `activate`/`deactivate` 闭包捕获同一个 `explorerActions` 对象（引用稳定）

**为何需要 ref 模式**：`selectedPath` 是 React state——直接闭包捕获会导致 `explorerActions` 对象内的回调持有旧值。而 `activeExplorer` 指针仅通过 DOM `focusin` 事件更新（`usePanelFocus`），容器已聚焦后再点击不触发 `focusin`，指针永远指向首次聚焦时的旧对象。ref 绕过此限制——对象永不重建，数据永远读最新值。

> 首次聚焦也存在竞态：`focus()` 同步触发 `focusin` 在 React batch commit 之前，此时 `activateRef.current` 指向旧渲染的 `activate`。ref 模式同样修复此竞态——新旧 `activate` 包裹同一 `explorerActions` 对象。

## 文件

| 文件 | 职责 |
|------|------|
| `ExplorerPanel.tsx` | React 容器组件：活跃项目推导、文件树渲染、CRUD 事件处理、`handleOpenFile` 面板分派（FileViewerRegistry）、`notify_watch` 启动、**选中模型** + **焦点管理**（tabIndex={-1} + usePanelFocus("explorer")）+ **active pointer 集成** |
| `useFileTree.ts` | 文件树数据 hook：`loadRoot` / `loadDirectory` / `toggleExpand` / `refreshExpanded`（经 `reloadPreservingExpanded` 递归重载保留展开状态）/ generation 取消 |
| `FileTree.tsx` | 递归树组件：节点渲染、git 状态着色、右键菜单、**单击选中（VS Code 风格高亮）** + **renamingPath 状态上提**（由 ExplorerPanel 管理） |
| `FileIcon.tsx` | 文件图标映射（扩展名→emoji） |
| `activeExplorer.ts` | 模块级"聚焦 explorer"指针（createActivePointer 模式，同 activeTerminal/activeEditor） |
| `keyboard.ts` | 快捷键命令工厂：`createExplorerShortcuts()` → 3 条命令（delete/open/rename），在 App.tsx 一次性注册 |

## 宿主变更

ExplorerPanel 组件本体不变。宿主从 Allotment 常驻栏（Workspace 四栏布局中独立的 `<Allotment.Pane>`）变为侧栏区视图槽——经 `src/features/sideViews/sideViewDefs.ts` 注册为 `explorer` 视图（`id: "explorer"`, `title: "文件浏览器"`, `icon: "📁"`，`component: () => React.createElement(ExplorerPanel)`），由 `SideBarArea` 经 `display:none/flex` 切换渲染。

**已知行为：换区重建丢失展开状态**（ADR-0001）。当用户从活动栏拖拽 `explorer` 按钮跨区（上→下或下→上），React 将组件从旧 pane 卸载、在新 pane 重新挂载——ExplorerPanel 内部状态（文件树展开状态、`rootNodes`）全部丢失。此行为在 ADR-0001 中已确认接受：换区为低频操作（用户通常设定一次后不改），重建成本低于跨父节点保持实例的架构复杂度。

## 关键集成点

- **`src/ipc/fs.ts`** — `readDir` / `writeFile` / `createDir` / `deleteEntry` / `rename`
- **`src/ipc/git.ts`** — `gitStatus` 着色
- **`src/ipc/notify.ts`** — `notify_watch` 启动后端监听 + `onFsEvent` 增量刷新
- **`src/stores/projects.ts` + `src/stores/layout.ts`** — 活跃项目 `rootPath` 推导
- **`src/workspace/titleManager.ts`** — 文件打开时计算编辑器页签标题 + 去重
- **`src/features/fileViewers/FileViewerRegistry.ts`** — 策略模式决定文件用哪个面板类型打开（`.html` → `"htmlviewer"`，未知 → `"editor"`）
- **`src/features/sideViews/sideViewDefs.ts`** — ExplorerPanel 注册为 `explorer` 视图（`id: "explorer"`, icon: "📁"）

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

**共享 mock 工厂**（`src/__tests__/` 下）：
- `testMocks/explorerMocks.ts` — IPC mock 接口定义，实现在 `setup.ts` 中经 `globalThis` 注入供 `vi.hoisted()` 使用
- `helpers/vfs.ts` — `makeVfs` / `findNode` / `mockEntry` 虚拟文件系统辅助
- `helpers/workspace-setup.ts` — `resetProjectStores` / `seedExplorerProject` store 种子工厂
- 已试点迁移 `explorer-rootpath-clear`、`explorer-refresh-preserve`、`explorer-notify` 三个文件

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

### 刷新保留展开状态测试（`explorer-refresh-preserve.test.tsx`，17 用例）

验证 `reloadPreservingExpanded` 递归重载保留展开状态。用 `renderHook(useFileTree)` 直接驱动，非组件渲染：

- **虚拟文件系统**：`makeVfs({ dirPath: DirEntry[] })` 构造 `Map`，`readDir` 用 `mockImplementation((dirPath) => vfs.get(dirPath))` 按路径分派；测试中动态 `vfs.set/delete` 模拟磁盘增删。
- **展开辅助**：`expand(result, path)` = `act(toggleExpand)` + `waitFor(expanded===true && loading===false)`。
- **fs-event 触发**：`onFsEvent` mock 保存回调，`triggerFsEvent()` 手动触发；配 `vi.useFakeTimers()` + `advanceTimersByTimeAsync(250)` 跨过 200ms 去抖。
- 覆盖矩阵 A–E：递归重建正确性（R1–R4/R7/R8/R10）、边界容错（R6/R9/R11/R12）、gitStatus 路径（R5/R13）、三条触发路径（R14 file-saved / R15 fs-event / R16 CRUD refresh）、竞态 last-write-wins（R17）。

### rootPath 变化清空测试（`explorer-rootpath-clear.test.tsx`，6 用例）

验证 `rootPath` 变化时 `rootNodes` 和 `gitStatusMap` 立即清空。用 `renderHook(useFileTree)` + `rerender` 驱动 `rootPath` 变化：

- **T2.1/T2.2/T2.3**：切换项目 / rootPath 为 null / 新数据正确加载 — 基本清空行为
- **T2.4**：快速连续切换 A→B→C，gen 检查丢弃 B 的结果，最终为 C 的数据
- **T2.5**：同值 rerender 不清空（React deps 未变化）
- **T2.6**：首次挂载 `rootNodes` 为 `[]`
