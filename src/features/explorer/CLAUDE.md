# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

文件浏览器（ExplorerPanel）展示当前活跃项目的文件树，支持 CRUD、Git 状态着色与文件系统事件增量刷新。本模块同时是 `FileViewerRegistry` 的主要调用方之一，决定文件打开时使用哪种面板类型。

## 关键约束与决策

### Generation 异步取消 + rootPath 清空

切换项目页面时 `rootPath` 变化，effect **先同步清空** `rootNodes`（`[]`）和 `gitStatusMap`（空 `Map`），立即消除旧项目文件树残留；之后发起新 `loadRoot(gen)` + `gitStatus(rootPath)`。`genRef` 每次 `rootPath` 变化递增，旧请求回调检查 `gen !== genRef.current` 后丢弃结果。

- `rootPath` 为 `null` 时立即清空，不做 IPC 调用。
- 清空是同步的——`setRootNodes([])` 与后续 `setRootNodes(nodes)` 在同一渲染批次，中间不可见。
- `rootPath` 不变时不触发清空。
- `refreshExpanded`（CRUD / fs-event / file-saved 回调）不传 gen——它们操作的是当前页数据。

### `useFileTree` 自包含加载

`rootPath` 变化时 `useFileTree` 内部 effect 自动调用 `loadRoot()` + `gitStatus()`。ExplorerPanel 只负责调用 CRUD 操作后的 `refresh()`，**不在 `rootPath` 变化时重复刷新**。

### 刷新保留展开状态（`reloadPreservingExpanded`）

文件变更刷新统一走 `refreshExpanded`。旧实现直接 `loadRoot()` 整树替换 → `loadDirectory` 硬编码 `expanded: false` → 丢弃全部展开状态。现改为 `reloadPreservingExpanded` 递归重载：以旧树（`rootNodesRef` 镜像）为蓝图，每层用 `Map<path, oldNode>` 匹配，命中 `old.expanded && isDir` 则递归 `readDir` 下钻并保留 `expanded=true`。

- 已展开目录被磁盘删除 → 父层 `readDir` 不再返回该项，子树自然消失。
- 已展开子目录 `readDir` 抛错 → `loadDirectory` catch 返回 `[]`，该目录 children 为空、不冒泡。
- `reloadPreservingExpanded` **不传 gen**（操作当前页数据）。

### FE-15 已知路径子树刷新

`slterm:file-saved` 事件 300ms debounce 后，不再整树 `refreshExpanded`——在旧树中定位「变更路径的最近展开祖先」，仅重载该目录一层并原位合并（保留曾展开子目录的展开态与子树）。变更路径位于未展开目录内 → 只刷新其父层。同时补 git 着色刷新。

### `handleOpenFile` 面板分派

不再硬编码 `PANEL_EDITOR`，改为通过 `fileViewerRegistry.resolve(filePath)` 决定面板类型。命中策略（如 `.html` → `"htmlviewer"`）则用对应面板，返回 null 回退 `"editor"`。文件预览类面板通过 `isAlwaysRenderPanel()` 自动设置 `renderer: "always"` 保持 iframe browsing context 存活，避免页签切换白屏。新增文件预览类型无需修改 ExplorerPanel。

### 选中模型：单击选中 + 双击打开 + 空白取消

`selectedPath` state 由 ExplorerPanel 管理，通过 props 传入 FileTree：

- 单击文件/文件夹 → `onSelect(path)` + `container.focus()`。
- 双击文件 → `onOpenFile(path)`。
- 单击目录 → `onSelect(path)` + `onToggleExpand(path)`。
- 单击空白 → `onSelect(null)`。
- 焦点离开 → 选中态保留，但 `usePanelFocus` 的 `popContext("explorer")` 阻止快捷键在失焦时误触发。

### 焦点管理：tabIndex={-1} + usePanelFocus("explorer")

文件树容器 `<div ref={containerRef} tabIndex={-1}>` 可编程聚焦（不参与 Tab 序）。`usePanelFocus("explorer", containerRef.current, activate, deactivate)`：

- focusin → `pushContext("explorer")` + `activate()` → `setActiveExplorer(explorerActions)`。
- focusout（离开子树）→ `popContext("explorer")` + `deactivate()` → `clearActiveExplorer(explorerActions)`。
- 焦点环由全局 `:focus-visible` 接管：鼠标点击不匹配 `:focus-visible`，键盘编程聚焦时可见焦点环（FE-27/UI-808）。

### 快捷键集成：active pointer + 命令工厂

与 terminal/editor 同模式：

1. `activeExplorer.ts`：`createActivePointer<ExplorerActions>()`，模块级指针。
2. `keyboard.ts`：`createExplorerShortcuts()` 返回 3 条命令——`explorer.delete` / `explorer.open` / `explorer.rename`。
3. 命令在 `App.tsx` 一次性注册。
4. handler 经 `getActiveExplorer()` 派发到聚焦实例。

**重命名中透传**：当 `isRenaming()=true`，Enter 和 Del 命令 handler 返回 `false` 透传，让 input 处理 Enter 确认。

### ref 模式（actions 闭包过期修复）

`explorerActions` 的 `useMemo` 使用**空依赖**（`[]`），所有数据通过 ref 间接访问：

- `selectedPathRef` 每次渲染同步。
- `getSelectedPath: () => selectedPathRef.current` 运行时读取最新值。
- `activate`/`deactivate` 闭包捕获同一个 `explorerActions` 对象（引用稳定）。

**为何需要 ref 模式**：`selectedPath` 是 React state，直接闭包捕获会导致 `explorerActions` 持有旧值。`activeExplorer` 指针仅通过 DOM `focusin` 事件更新，容器已聚焦后再点击不触发 `focusin`，指针永远指向首次聚焦时的旧对象。ref 绕过此限制。

### 宿主变更（ADR-0001）

ExplorerPanel 经 `src/features/sideViews/sideViewDefs.ts` 注册为 `explorer` 视图，由 `SideBarArea` 经 `display:none/flex` 切换渲染。

**已知行为：换区重建丢失展开状态**。当用户从活动栏拖拽 `explorer` 按钮跨区（上→下或下→上），React 将组件从旧 pane 卸载、在新 pane 重新挂载——ExplorerPanel 内部状态（文件树展开状态、`rootNodes`）全部丢失。此行为在 ADR-0001 中已确认接受。

## 外部坑/红线

- **watcher 成对**：`startWatch` 启动后必须在项目移除/切换时 `stopWatch`（BE-10）。本组件不再直接管理 watcher，只消费 `onFsEvent`；成对逻辑在 `Workspace` 的 SEC-01 effect 中。
- **fs-event 200ms / file-saved 300ms debounce**：刷新触发频率受此约束，测试需 fake timers。
- **rootPath 为 null 立即清空**：不发起 IPC，避免沙箱拒绝报错。
- **FileIcon 六色盘硬编码例外**：`FILE_COLORS` 常量写死于本文件，是硬约束 #6 的登记例外（IC-04）。
- **虚拟化行高 24px**：`FileTree` 手实现窗口化虚拟化，固定行高，DOM 节点数与可见行数同量级。
- **actions 空依赖 + ref 模式**：修改 ExplorerActions 相关逻辑时，禁止把 state 直接闭包进 `useMemo` 依赖，否则聚焦后点击不再更新选中路径。

## 测试模式

- 测试文件位于 `src/__tests__/`，命名规则 `explorer-*.test.ts(x)`。
- **必须 mock 三个 IPC 模块**：`../ipc/fs`、``../ipc/git``、`../ipc/notify`。
- `vi.hoisted()` 创建 mock 状态，确保模块级 `vi.mock()` 执行前就绪。
- **共享工厂**：`testMocks/explorerMocks.ts`（接口）、`helpers/vfs.ts`（虚拟文件系统）、`helpers/workspace-setup.ts`（store 种子）。
- **Zustand stores** 使用真实实现 + `.setState()` 种子数据，`beforeEach` 重置。
- **FileTree 独立渲染**：直接传 `nodes` / `gitStatusMap` props。
- **ExplorerPanel 集成渲染**：需先种子 `useProjects` + `useLayout` + `window.__dockviewApi`。
- **右键菜单**：`fireEvent.contextMenu(element)` 触发；StrictMode 双渲染会导致重复元素，取 `getAllByText` 首个。
- **刷新保留展开状态**：用 `renderHook(useFileTree)` 直接驱动，`makeVfs` 构造虚拟文件系统，`triggerFsEvent` 手动触发事件，`vi.useFakeTimers()` 跨过 debounce。
- **rootPath 变化清空**：`renderHook(useFileTree)` + `rerender` 驱动 `rootPath` 变化，验证 gen 丢弃旧结果。
