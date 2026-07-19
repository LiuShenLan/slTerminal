# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

工作区布局管理——多 Dockview 实例、布局序列化/反序列化、面板类型注册、**页签标题集中管理**。

## 架构决策

**多 Dockview 实例（非单实例 + tab 切换）**：每个操作页面拥有独立 `<DockviewReact>` 实例。页面切换通过 CSS `display:none/block`，终端不销毁。根因：xterm.js 不支持二次 `open()`（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。

**惰性页面初始化**：`initializedPages` Set 控制哪些页面挂载了 `PageDockview`。未初始化的页面不渲染 DOM，切换到新页面时才触发 `ensurePageInitialized`。

**fromJSON 恢复守卫**：`restoreGuardRef` 阻止 `onDidLayoutChange` 在程序化恢复布局时向 store 写回。`onDidLayoutFromJSON` 事件中置 true，`setTimeout(0)` 异步复位——因为 Dockview 会在 JSON 恢复后立即触发 layout change。

**不自动创建默认终端**：`handleReady` 在布局恢复失败（空布局 `{}` 或损坏数据）时不再兜底创建终端面板。空白页面由 Watermark 组件接管显示（"打开终端或编辑器开始工作"），用户通过 Watermark 按钮或页签 "+" 按钮手动创建终端。新建页面的空布局由 `SidebarTree.makeEmptyLayout()` 提供。已有页面布局恢复路径不变。

**DefaultTab 页签图标渲染**：`DefaultTab` 通过 `params.tabIcon` 控制图标显示。终端面板在检测到注册命令运行时通过 `api.updateParameters({ tabIcon: "..." })` 设置图标，退出时设为 `null`。`DefaultTab` 订阅 `api.onDidParametersChange` 动态切换。

> **关键坑**：Dockview `PanelApi.onDidParametersChange` 类型为 `Event<Parameters>`，回调直接接收 `Parameters` 对象（扁平 key-value），不是 `{ params: Parameters }` 包裹结构。错误写成 `event.params.tabIcon` 会导致始终读到 `undefined`。

**setProjectRoot 前置为页面切换前置条件**：`project_root` 是 `activePageId` 生效的前提，不是它的副作用。`switchToPage`（`Workspace.tsx`）改为 async——先 `await setProjectRoot(rootPath)` 再 `setActivePage(pageId)`。`App.tsx` 启动恢复 `lastPage` 同样先 `await setProjectRoot`。SEC-01 effect（`Workspace.tsx`）保留兜底（服务于 `pty_spawn` 等其它消费者 + E2E helper 直设 `activePageId` 的路径）。根因：React 同一 commit 的 passive effect 子组件先于父组件执行——旧代码中 `setProjectRoot` 在父 effect 执行时，子组件（ExplorerPanel）的 `fs_read_dir` 已因 `project_root=None` 被路径沙箱拒绝。详见 `docs/debug/refactor-regressions.md` 故障A。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：Workspace 组件、panelRegistry、PANEL_TYPES、`saveLayout`/`loadLayout`（从 `../panelRegistry` 重导出） |
| `Workspace.tsx` | 主组件：三栏布局（Allotment）容器——活动栏(40px固定)+侧栏区(visible=anyOpen)+主区 + 页面生命周期管理（`ensurePageInitialized`/`pageCallbacksRef`），单页渲染委托 PageDockviewHost；`import sideViewDefs` 触发 side-effect 注册 |
| `PageDockviewHost.tsx` | 单页面 Dockview 实例宿主组件（React.memo 包裹）：DefaultTab、Watermark、RightHeader、ContextMenu、布局恢复 |
| `layoutSerde.ts` | 布局序列化/反序列化：`saveLayout`（`api.toJSON()`）、`loadLayout`（`api.fromJSON()` + 旧格式修补 + 白名单过滤） |
| `titleManager.ts` | 页签标题集中管理：terminal-N 编号、文件标题冲突检测、handleSaveAs、onDeletePage(pageId)：清理该页面 registry 和 counters 条目 |

> **panelRegistry.ts 已提取到 `src/panelRegistry.ts`（已提取为共享配置层）**：面板注册表是全局架构组件，被 workspace、explorer、测试等多方引用，不应埋于 workspace 子路径。

## 硬约束

- **#5 面板封闭**：新增面板类型 → 在 `panels/` 下创建目录 → 在 `src/panelRegistry.ts` 注册 → `PANEL_TYPES` 数组追加类型名
- **#7 布局单点**：操作页面布局只经 `layoutSerde.ts` 存取。`PageDockview.onDidLayoutChange` → `saveLayout()` → `handlePageLayoutChange()` → `useProjects.updatePageLayout()`
- **#8 会话元数据单点**：终端 PTY 会话只在面板内管理（`panels/terminal/`），Workspace 不持有会话引用

## 数据流

```
用户拖拽/分屏 → Dockview onDidLayoutChange
  → restoreGuardRef? (跳过程序化恢复)
  → saveLayout(api) → toJSON()
  → handlePageLayoutChange(pageId, layout)
  → useProjects.updatePageLayout(projId, pageId, layout)
  → Zustand subscribe → 2s debounce → saveToDisk
```

页面切换流：
```
SidebarTree.switchToPage(projectId, pageId)
  → 查 useProjects.getState() 获取 pageId 所属项目 rootPath
  → await setProjectRoot(rootPath)  // 路径沙箱前置条件（失败 console.error 降级，仍继续切换）
  → ensurePageInitialized(pageId)   // 首次切换时挂载 PageDockview
  → useLayout.setActivePage(pageId)
  → React 重渲染，目标页面 display:block，其余 display:none
  → window.__dockviewApi 更新指向活跃页面
```

> **DBG-5**: `switchToPage` 改为 async——`setProjectRoot` 必须在 `setActivePage` 之前完成，确保子组件 effect 中的 `fs_read_dir`/`git_status`/`notify_watch` 等 IPC 调用在后端 `project_root` 就绪后通过路径沙箱校验。

## 布局架构（三栏 Allotment）

Workspace 使用 Allotment 实现三栏布局（旧为常驻四栏，侧栏视图系统引入后改造）：

```
┌──────────┬──────────────────┬─────────────────────────┐
│ 活动栏    │ 侧栏区            │ 主区                     │
│ 40px 固定 │ visible=anyOpen  │ Dockview（flex:1）       │
│          │ ┌──────────────┐ │                         │
│ 📋       │ │ 上半区视图槽   │ │                         │
│ 📁       │ │ (display:none │ │  page1 display:block    │
│          │ │  保挂载)      │ │  page2 display:none     │
│          │ ├──────────────┤ │                         │
│          │ │ 下半区视图槽   │ │                         │
│          │ │ (splitRatio) │ │                         │
│          │ └──────────────┘ │                         │
└──────────┴──────────────────┴─────────────────────────┘
```

- **pane1（活动栏）**：`preferredSize=40 minSize=40 maxSize=40`，渲染 `<ActivityBar />`
- **pane2（侧栏区）**：`preferredSize=width minSize=WIDTH_MIN maxSize=WIDTH_MAX visible=anyOpen`，渲染 `<SideBarArea />`（内含垂直 Allotment 上下半区）。宽度经外层 Allotment `onChange` 同步到 `sideBar` store 持久化（`settings.json` 的 `sideBar.width` 段）
- **pane3（主区）**：`minSize=200`，多 Dockview 实例按 `activePageId` 做 CSS `display:none/block` 显隐
- **anyOpen**：由 `deriveLayout(open)` 纯推导——`hidden`→`false`，其余三态（single-top/single-bottom/split）→`true`
- **侧栏区上下分割比例**：`splitRatio` 持久化于 `settings.json` 的 `sideBar.splitRatio` 段，由 `SideBarArea` 内垂直 Allotment 的 `onChange` 同步
- 已删除旧六常量 `SIDEBAR_*`/`EXPLORER_*`，尺寸常量统一在 `src/features/sideViews/sideBarState.ts`

## 关键集成点

- **`useLayout`** (`stores/layout.ts`)：只跟踪 `activePageId`，不持有布局数据
- **`useProjects`** (`stores/projects.ts`)：Project → OperationPage 二级模型，持有布局和 cwd
- **`useSideBar`** (`stores/sideBar.ts`)：侧栏视图状态（zones/open/width/splitRatio），Workspace 订阅 `open`/`width` 控制 pane2 可见性和宽度
- **`panelRegistry`**：被 Workspace 传给 DockviewReact 的 `components` prop
- **`ActivityBar`** (`features/sideViews/ActivityBar.tsx`)：活动栏组件，渲染于 pane1，消费 `sideViewRegistry` + `useSideBar`
- **`SideBarArea`** (`features/sideViews/SideBarArea.tsx`)：侧栏区组件，渲染于 pane2，通过 `sideViewDefs` 注册的两条视图（projects/explorer）以 display:none 保挂载切换
- **SidebarTree**（侧边栏）：项目/操作页面树，宿主从 Allotment 常驻栏变为 `SideBarArea` 视图槽（经 `sideViewDefs` 注册为 `projects` 视图），组件本体不变
- **ExplorerPanel**（文件浏览器）：活跃项目的文件树，宿主同上变为视图槽（经 `sideViewDefs` 注册为 `explorer` 视图），组件本体不变。按钮跨区拖拽导致视图跨 pane 移动时组件重建（ADR-0001）

## E2E 测试支持

- `window.__slterm_e2e_workspaceReady`：Workspace 挂载时同步设置（渲染阶段，非 `useEffect`），WDIO 脚本轮询此标志等待就绪
- `window.__dockviewApi`：始终指向当前活跃页面的 DockviewApi，E2E 脚本通过它创建面板/执行操作

## 页签标题集中管理（`titleManager.ts`）

- 终端页签 = `terminal-N`（每页独立从 0 开始，关闭不重算）
- 编辑器页签 = 文件名；同名冲突 → 相对路径（相对 `Project.rootPath`）
- 布局持久化时忽略保存的 `title`，从 `params.filePath` 重新计算
- Save-As 通过 `slterm:file-saved-as` CustomEvent 通知 Workspace 层重算标题
- 重复文件打开：`findExistingEditor` 查重 → 聚焦已有面板（不改布局）
- `onDidRemovePanel` 监听面板关闭 → 注销 + 重算剩余面板标题
- `onDeletePage` 在页面删除时清理该页所有标题注册和计数器
- **页签图标**：`DefaultTab` 通过 `params.tabIcon`（由 `TerminalPanel` 通过 `api.updateParameters` 设置）控制终端图标渲染。非终端面板不设置此字段，无图标

## Dockview 事件结构注意事项

- `api.onDidTitleChange`：回调接收 `TitleEvent { title: string }` → `event.title`
- **`api.onDidParametersChange`**：回调直接接收 `Parameters` 对象（`Record<string, unknown>`），**不是** `{ params: Parameters }` 包裹 → `event.tabIcon`，**非** `event.params.tabIcon`

## 测试模式

测试文件位于 `src/__tests__/`：`layoutSerde.test.ts`、`default-layout-format.test.ts`、`panelRegistry.test.ts`、`titleManager.test.ts`、`workspace*.test.tsx`。

### 技术栈

- Vitest（jsdom）+ React Testing Library
- Dockview 组件全量 mock：`vi.mock("dockview-react", () => ({ DockviewReact: vi.fn(() => null) }))`
- Allotment 布局容器 mock
- Zustand stores 使用真实实现 + `beforeEach` 种子数据

### 布局序列化测试

`layoutSerde.test.ts`（16 用例）：
- **旧格式修补**：`patchLegacyLayout` 处理 `component`→`contentComponent` 迁移、缺失 `grid.orientation` 默认值、`leaf.data.id` 缺失生成、`activeGroup` 缺失填充
- **白名单过滤**：`PANEL_TYPES` 外类型的面板不被恢复（mock `isValidPanelType` 对齐真实 `PANEL_TYPES = ["terminal", "editor", "htmlviewer"]`）
- **深拷贝验证**：`toJSON` 输出不与输入共享引用
- 纯函数测试，无需 React 渲染环境

`default-layout-format.test.ts`（8 用例）：
- `makeEmptyLayout` 空布局验证：返回 `{}`、独立对象、JSON 往返一致性、纯对象检查

### 面板注册表测试

`panelRegistry.test.ts`（23 用例）：
- 三个面板注册验证（terminal/editor/htmlviewer）
- `PANEL_TYPES` 常量完整性 + `isValidPanelType` type predicate（大小写敏感）
- `FILE_PANEL_TYPES` Set 语义（编辑器 + 文件预览面板）

### 页签标题测试

`titleManager.test.ts`（29 用例）：
- `createTitleManager` 工厂，验证终端 `terminal-N` 递增、编辑器 basename/同名冲突相对路径
- `findExistingEditor` 查重、`registerAndRecompute` 重算、`handleSaveAs` 路径变更

### 多实例与 E2E 标记测试

`workspace-multi-instance.test.tsx`（4 用例）：
- 多页面切换时 Dockview 实例各自存活（不销毁重建）
- CSS 显隐控制 + `initializedPages` 惰性初始化
- 断言使用 `not.toMatch(/terminal-/)`（面板 ID 格式 `terminal-{pageId}-{seq}`，pageId 以字母开头——早期 `/terminal-\d/` 要求数字紧跟在 terminal- 后，永不匹配）
- 需种子 `useProjects`（多个页面）+ `useLayout`（活跃页面 ID）

`workspace-e2e-ready.test.tsx`（4 用例）：
- `window.__slterm_e2e_workspaceReady` 在渲染阶段（非 `useEffect`）同步设置
- 未挂载不存在、卸载保留、mock 全部子组件

`workspace-switch-order.test.tsx`（14 用例，DBG-9）：
- `switchToPage` 时序契约：`setProjectRoot` 先于 `setActivePage` 生效
- `setProjectRoot` 失败时降级（`console.error` + 仍完成切换）
- SEC-01 effect 兜底验证 + 现有测试兼容性排查

## 旧格式兼容

`layoutSerde.ts` 的 `patchLegacyLayout` 处理早期 `makeDefaultLayout` 产出的缺失字段：
- `component` → `contentComponent` 迁移
- `grid.orientation` 缺失 → 默认 `"HORIZONTAL"`
- `leaf.data.id` 缺失 → 从 `views[0]` 生成 `group-{view}`
- 顶层 `activeGroup` 缺失 → 用第一个 leaf 的 id
