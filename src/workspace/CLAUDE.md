# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/workspace` 管理工作区布局骨架：多 Dockview 实例、布局序列化/反序列化、面板类型注册、页签标题集中管理。Dockview 只提供分屏/拖拽/页签容器，本层决定页面生命周期、跨页终端存活策略、布局持久化路径与全局标题契约。

## 关键约束与决策

### 多 Dockview 实例（H6）

每个操作页面拥有独立 `<DockviewReact>` 实例。页面切换通过 CSS `display:none/block`，终端不销毁。根因：xterm.js 不支持二次 `open()`（Issue #4978）。

**FE-01 豁免登记**：多实例架构不变（H6 + xterm #4978 是硬约束），仅加**页面总数上限 `MAX_PAGES = 20`**（`stores/projects.ts`，`addPage` 超限拒绝 + toast 告警）缓解无界增长。豁免登记见 ADR-0001 配套豁免表。

**惰性页面初始化**：`initializedPages` Set 控制哪些页面挂载了 `PageDockview`。未初始化的页面不渲染 DOM，切换到新页面时才触发 `ensurePageInitialized`。

### 布局恢复与变更守卫

- **fromJSON 恢复守卫**：`restoreGuardRef` 阻止 `onDidLayoutChange` 在程序化恢复布局时向 store 写回。`onDidLayoutFromJSON` 事件中置 true，`setTimeout(0)` 异步复位——Dockview 会在 JSON 恢复后立即触发 layout change。
- **不自动创建默认终端**：`handleReady` 在布局恢复失败（空布局 `{}` 或损坏数据）时不再兜底创建终端面板。空白页面由 Watermark 组件接管，用户手动创建终端。
- **布局单点（#7）**：操作页面布局只经 `layoutSerde.ts` 存取。`PageDockview.onDidLayoutChange` → `saveLayout()` → `handlePageLayoutChange()` → `useProjects.updatePageLayout()`。

### DefaultTab 页签形态（TAB-01/02/03，IC-03，F9）

`DefaultTab` 为扁平化页签（`params.tabIcon` 已退役）：

- **状态圆点**：`params.tabStatus`（`AgentStatus | null`）→ 渲染 `StatusDot` 圆点（working 绿/attention 黄/done 灰/error 红）；null 不渲染。
- **CLI 品牌 logo**：`params.tabLogo`（F9 行为修订：**跟随页签名显示**，不依赖 tabStatus）由 TerminalPanel 会话绑定写入。
- **文件型页签图标**（TAB-03）：`params.filePath` 存在（FILE_PANEL_TYPES）→ 渲染 `FileIcon` 彩色图标；与终端分支互斥。
- **激活指示条**（TAB-01）：`isActive && isGroupActive` 时渲染底部 2px 指示条（absolute 锚定 `.dv-tab` 底边，色 `FOCUS_BORDER`，`pointerEvents: none`）。
- **hover 关闭 ×**（TAB-02）：× 默认不可见（opacity 0 + pointerEvents none），hover 时显现。
- **× 关闭守卫（F11 登记，SC-FE-07）**：settings 面板且 dirty → `confirmDialog` 确认才 `api.close()`。判据 = `params.panelId` 的 `settings-` 前缀（DefaultTab 拿不到 panel——dockview 8.1.0 `IDockviewPanelProps` 无 panel 属性，`panel.view.contentComponent` 红线不适用该场景）；该前缀与 dirtyRegistry 键同源（SettingsPanel 以同一 params.panelId 注册），无漂移；非 settings 面板 / 非 dirty 直关（行为零回归）。

### Watermark 空态规范（GL-05/UI-806）

空白页面由 `createWatermark` 组件接管：**15px 线性图标 + 说明文字 13px + 可选「新建终端」次按钮**（SECONDARY_BG 底 + SEPARATOR_BG 描边，点击 addPanel 终端，`renderer: "always"`）。

### 项目切换前置：setProjectRoot 先于 activePageId（DBG-5/SEC-01）

`project_root` 是 `activePageId` 生效的前提，不是副作用。`switchToPage` 改为 async——先 `await setProjectRoot(rootPath)` 再 `setActivePage(pageId)`。`App.tsx` 启动恢复 `lastPage` 同样先 await 再切页。SEC-01 effect 保留兜底。

根因：React 同一 commit 的 passive effect 子组件先于父组件执行——旧代码中 `setProjectRoot` 在父 effect 执行时，子组件（ExplorerPanel）的 `fs_read_dir` 已因 `project_root=None` 被路径沙箱拒绝。

### 文件监听上提到项目激活层

SEC-01 effect 同时承担 `startWatch(rootPath)` / `stopWatch(prev)`——watcher 宿主从 ExplorerPanel 上提到项目激活层：编辑器外部修改 reload / commit 面板刷新等 fs-event 消费方不依赖 explorer 视图是否打开。`activePageId` 置 null（删除末页/移除活跃项目）时对 `prevRootRef.current` 调 `stopWatch` 并清 ref，防 OS 句柄残留至 LRU 淘汰。

### 面板注册表已提取到 `src/panelRegistry.ts`

`panelRegistry` / `PANEL_TYPES` / `FILE_PANEL_TYPES` / `isAlwaysRenderPanel` 是全局架构组件，被 workspace、explorer、测试等多方引用，不应埋于 workspace 子路径。新增面板类型仍按 #5 流程：创建目录 → 注册 → 追加 `PANEL_TYPES`。`isAlwaysRenderPanel` **不含 settings**（决策写死，SC-FE-06）：重建无视觉闪屏、状态在 params/store，未保存 dirty 随卸载丢失与旧 hooksConfig 行为一致继承。

### openSettingsPanel 同页单例（F11，SC-FE-02）

`openSettingsPanel(pageId, settingsPageId?)` 在 pageApis.ts——面板 id = `settings-{pageId}`；getPanel 命中 → focus 返回 true（同页单例），未命中 → addPanel（component "settings"，settingsPageId 深链注入 params.selectedPage）；100ms×50 轮询 getPageApi 就绪，超时 console.warn 降级返回 false。**调用方须先切到目标页**（本函数不切页）——编排见 `features/settingsCenter/openSettings.ts`（无项目 toast 拦截在编排层，R1）。

### 终端页签自定义重命名（F8）

- **入口**：右键菜单对终端面板（判据 `panel.view.contentComponent === "terminal"`，`panel.component` 不存在）插入「重命名」项；claude 运行中（`TerminalRegistry.get(panel.id)?.agentSession != null`）→ `disabled` 置灰。
- **存储单一真值源**：`params.customTitle`（随布局 JSON 持久化）。`applyRename` 纯函数 = `updateParameters({ customTitle })` + `setTitle` + **显式 `onLayoutChange(saveLayout(api))`**——`setTitle`/`updateParameters` 均不触发 `onDidLayoutChange`，须显式保存。
- **恢复链路**：`rebuildAndRecomputeTitles` 重算编辑器标题 + **终端标题**——无 customTitle 的终端面板用 `titleManager.getTerminalTitle(pageId)` 重算（持久化 title 可能是瞬态值），customTitle 保留不重算。
- **约束**：`titleManager` 计数器不动（F8 不占用编号）；编辑器等非终端面板菜单无「重命名」。

### 页签标题集中管理（`titleManager.ts`）

- 终端页签 = `terminal-N`（每页独立从 0 开始，关闭不重算；恢复布局同样经 `getTerminalTitle` 消费编号）。
- 编辑器页签 = 文件名；同名冲突 → 相对路径（相对 `Project.rootPath`）。
- 布局持久化时忽略保存的 `title`，从 `params.filePath` 重新计算。
- Save-As 通过 `slterm:file-saved-as` CustomEvent 通知 Workspace 层重算标题。
- 重复文件打开：`findExistingEditor` 查重 → 聚焦已有面板。
- **suffix 字段**：`EditorEntry` 含 `suffix`（如 `(git diff)`）。`findExistingEditor(pageId, filePath, suffix?)` 按 suffix 匹配，保证普通编辑器与 git 页签互不误聚焦（B10）。

### Dockview 事件结构注意事项

- `api.onDidTitleChange`：回调接收 `TitleEvent { title: string }` → `event.title`。
- **`api.onDidParametersChange`**：回调直接接收 `Parameters` 对象（`Record<string, unknown>`），**不是** `{ params: Parameters }` 包裹 → `event.tabStatus`，**非** `event.params.tabStatus`。

### `__dockviewApi` 重指不变量

`window.__dockviewApi` 重指向只允许出现在三个位置：`switchToPageShared`（页面切换时立即重指已初始化页面）、`Workspace.handlePageApiReady`（页面首次初始化时重指）、`Workspace.onDeletePage`（删除页面后重指次页）。其他代码点通过 `getPageApi(pageId)` 访问指定页面的 API。

### E2E 测试支持

- `window.__slterm_e2e_workspaceReady`：Workspace 挂载时同步设置（渲染阶段，非 `useEffect`），WDIO 轮询等待就绪。
- `window.__dockviewApi`：始终指向当前活跃页面的 DockviewApi。

### 旧格式兼容

`layoutSerde.ts` 的 `patchLegacyLayout` 处理早期布局缺失字段：
- `component` → `contentComponent` 迁移
- `grid.orientation` 缺失 → 默认 `"HORIZONTAL"`
- `leaf.data.id` 缺失 → 从 `views[0]` 生成
- 顶层 `activeGroup` 缺失 → 用第一个 leaf 的 id

## 外部坑/红线

- **xterm.js 不可二次 `open()`**：多实例架构因此不可改为单实例 + tab 切换。
- **Dockview `onDidParametersChange` 扁平结构**：回调直接是 `Parameters`，不要写成 `event.params.xxx`。
- **`panel.component` 不存在**：判断面板类型用 `panel.view.contentComponent`。
- **新建终端编号延迟分配（FE-04）**：`nextPanelId()` 在菜单 action 执行时才调用，菜单构建期不消耗编号。
- **重命名必须显式保存布局**：`setTitle`/`updateParameters` 不触发 `onDidLayoutChange`，须手动 `onLayoutChange(saveLayout(api))`。
- **删除页面时 stopWatch**：`activePageId` 置 null 必须释放 watcher，否则 OS 句柄残留。
- **renderer="always" 白名单**：仅 `terminal` 和 `htmlviewer`。editor/gitshow/diff 故意排除——CM6 重建无视觉闪屏，且大文件编辑器若始终挂载会显著增加内存开销。

## 测试模式

- **Dockview 全量 mock**：`vi.mock("dockview-react", () => ({ DockviewReact: vi.fn(() => null) }))`。
- **Allotment 布局容器 mock**；Zustand stores 使用真实实现 + `beforeEach` 种子数据。
- **布局序列化**：测旧格式修补、白名单过滤、深拷贝。
- **页签标题**：测 terminal-N 递增、basename/冲突相对路径、suffix 匹配。
- **多实例**：测 Dockview 实例各自存活、CSS 显隐、`initializedPages` 惰性初始化。
- **DefaultTab**：渲染生产 `DefaultTab`（非 mock），重点断言 `tabStatus` 圆点、`tabLogo` 跟随页签名、`onDidParametersChange` 扁平事件结构。
- **切换时序**：测 `setProjectRoot` 先于 `setActivePage` 生效。
