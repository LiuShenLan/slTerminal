# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

工作区布局管理——多 Dockview 实例、布局序列化/反序列化、面板类型注册、**页签标题集中管理**。

## 架构决策

**多 Dockview 实例（非单实例 + tab 切换）**：每个操作页面拥有独立 `<DockviewReact>` 实例。页面切换通过 CSS `display:none/block`，终端不销毁。根因：xterm.js 不支持二次 `open()`（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。

**惰性页面初始化**：`initializedPages` Set 控制哪些页面挂载了 `PageDockview`。未初始化的页面不渲染 DOM，切换到新页面时才触发 `ensurePageInitialized`。

**fromJSON 恢复守卫**：`restoreGuardRef` 阻止 `onDidLayoutChange` 在程序化恢复布局时向 store 写回。`onDidLayoutFromJSON` 事件中置 true，`setTimeout(0)` 异步复位——因为 Dockview 会在 JSON 恢复后立即触发 layout change。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：Workspace 组件、panelRegistry、PANEL_TYPES、`saveLayout`/`loadLayout` |
| `Workspace.tsx` | 主组件：多页面管理、PageDockview 工厂、侧边栏/资源管理器三栏布局（Allotment） |
| `layoutSerde.ts` | 布局序列化/反序列化：`saveLayout`（`api.toJSON()`）、`loadLayout`（`api.fromJSON()` + 旧格式修补 + 白名单过滤） |
| `panelRegistry.ts` | 面板类型注册表：硬约束 #5 惟一定义点，白名单 `PANEL_TYPES = ["terminal", "editor", "htmlviewer"]`，`FILE_PANEL_TYPES` 文件型面板集合（参与标题计算），`isAlwaysRenderPanel()` 判断是否需要 renderer="always" |
| `titleManager.ts` | 页签标题集中管理：terminal-N 编号、文件标题冲突检测、handleSaveAs |

## 硬约束

- **#5 面板封闭**：新增面板类型 → 在 `panels/` 下创建目录 → 在 `panelRegistry.ts` 注册 → `PANEL_TYPES` 数组追加类型名
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
  → ensurePageInitialized(pageId)  // 首次切换时挂载 PageDockview
  → useLayout.setActivePage(pageId)
  → React 重渲染，目标页面 display:block，其余 display:none
  → window.__dockviewApi 更新指向活跃页面
```

## 关键集成点

- **`useLayout`** (`stores/layout.ts`)：只跟踪 `activePageId`，不持有布局数据
- **`useProjects`** (`stores/projects.ts`)：Project → OperationPage 二级模型，持有布局和 cwd
- **`panelRegistry`**：被 Workspace 传给 DockviewReact 的 `components` prop
- **SidebarTree**（侧边栏）：项目/操作页面树，位于左侧第一栏，通过 props 接收 `switchToPage` / `onDeletePage`
- **ExplorerPanel**（文件浏览器）：活跃项目的文件树，位于左侧第二栏，订阅 `onFsEvent` 做增量刷新

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

## 旧格式兼容

`layoutSerde.ts` 的 `patchLegacyLayout` 处理早期 `makeDefaultLayout` 产出的缺失字段：
- `component` → `contentComponent` 迁移
- `grid.orientation` 缺失 → 默认 `"HORIZONTAL"`
- `leaf.data.id` 缺失 → 从 `views[0]` 生成 `group-{view}`
- 顶层 `activeGroup` 缺失 → 用第一个 leaf 的 id
