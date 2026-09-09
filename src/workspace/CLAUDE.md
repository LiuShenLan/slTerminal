# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/workspace` 管理工作区布局骨架：共享 Dockview 宿主（页组模型）、布局序列化/反序列化、页签标题集中管理。Dockview 只提供分屏/拖拽/页签容器，本层决定页面生命周期、跨页终端存活策略、布局持久化路径与全局标题契约。

## 关键约束与决策

### 共享宿主 + 页组模型（CP-004/S11 取代原「多 Dockview 实例（H6）」）

Workspace 渲染**单一** `<DockviewReact>` 共享宿主（`WorkspaceDockHost.tsx`）。每个操作页面 = 宿主内一个**顶级页组**：

- **页组协议（pageGroups.ts 纯函数族，全仓承重契约）**：页组 id = `pageGroupId(pageId)`（`page-{pageId}`）；面板 id 全量页前缀 = `panelIdInPage(pageId, localId)`（`{pageId}:localId`）；`pageOfPanelId` 解析属主页；`panelsOfPage` 页内面板查询；`panelBelongsToGroup` 越界判定。终端 localId = `terminal-N`（每页从 0 起计数，`lib/panelId.ts` 单点，标题 `terminal-N` 与 id 编号同源语义不变——编号仍 local）。
- **页面切换 = 页组容器显隐**：仅活跃页组可见（dockview 网格叶级可见性，见下「可见性机制」）。切换只更新 `useLayout.activePageId`（`switchToPageShared`）——宿主订阅刷新，终端不随切页卸载/重建，xterm 实例只 open 一次。根因：xterm.js 不支持二次 `open()`（Issue #4978）。
- **可见性机制（写死）**：dockview gridview maximize 语义——目标页组最大化占满网格，其余页组叶隐藏（DOM 保留、面板不卸载、React 不重挂载；jsdom/真实 WebView 双实证）。隐藏页组 display:none 尺寸归零 → fit/resize 不触发；切回后 dockview 重排 → ResizeObserver 自然恢复（恒挂载面板 fit/resize 仅在页组可见时执行）。`maximizePageGroup(api, pageId)` 为可见性单点（恢复/并入/切页后统一调用）。无活跃页时整宿主容器 display:none（旧「页面实例全隐藏」空白主区语义）。
- **页面总数上限消亡**：多实例每页一实例的内存/DOM 线性增长源消失——页面总数上限（原 FE-01/FE-36）随架构删除（见 `stores/CLAUDE.md`）。
- **生命周期契约**：新增面板 addPanel 显式 `position.referenceGroup = pageGroupId(pageId)`（options.group 显式指定目标页组）；**跨页组拖拽禁止**——宿主 `onDidAddPanel` 守卫（`enforcePanelGroupMembership`，越界回迁原页组 + 清走空的越界组）；页面删除先 kill 页组内终端（removeGroup → 面板卸载链 → PTY kill）再移除页组；`renderer="always"` 白名单语义不变（恒挂载面板在隐藏页组内保持挂载）。**页内分屏（单页多组）不可用**——页 = 单组多页签，拖拽拆分产物被回迁守卫清理。

### 布局恢复与变更守卫

- **fromJSON 恢复守卫**：`restoreGuardRef` 阻止 `onDidLayoutChange` 在程序化恢复布局时向 store 写回。`onDidLayoutFromJSON` 事件中置 true，`setTimeout(0)` 异步复位——Dockview 会在 JSON 恢复后立即触发 layout change。
- **不自动创建默认终端**：宿主恢复失败（空布局 `{}` 或损坏数据）时不兜底创建终端面板。空白页组由 Watermark 组件接管，用户手动创建终端。
- **布局单点（#7）**：操作页面布局只经 `layoutSerde.ts` 存取。运行期宿主 `onDidLayoutChange` → `saveLayout()`（全量 toJSON）→ `syncHostLayoutToStore`（逐页切片）→ `useProjects.updatePageLayout()`；恢复经 `composeHostLayout`（全部页切片汇编宿主全量 JSON）→ `loadLayout`/`loadPageGroup`（运行期页组并入，`reuseExistingPanels: true` 面板实例跨恢复存活）。
- **存储形态**：`OperationPage.layout` = 该页**页组子树切片**（单宿主全量 JSON 按页切分——`slicePageLayout` 单点）；旧多实例格式（每页独立全量 SerializedDockview）启动时经迁移（`normalizePageLayout`）压平归组：面板 id 重写页前缀协议、无法归组的面板丢弃 + console.error（不阻断启动，Watermark 接管空页）。

### 共享打开链路与面板 params 持久化（docViewer 面板）

- **openFile.ts**：文件打开核心链路（守卫/去重聚焦/resolve ?? editor/renderer always/addPanel/registerEditor/recomputeTitles）由 ExplorerPanel.handleOpenFile 抽取的 workspace 单点——文件浏览器双击与预览链接点击（`openFileInActivePage` 便捷入口现取 stores + `__dockviewApi`）共用同一分发，复制即双源漂移（注册表家族契约）。ExplorerPanel 委托之（canOpenFile re-export 兼容导出面）。失败语义：守卫/异常返回 false 不弹错。面板 id 页前缀协议（localId 免撞号）+ 显式落活跃页组。
- **persistPanelParams.ts**：SettingsPanel.persistParams 先例通用化——`updateParameters(patch)` + 显式布局落盘（updateParameters 不触发 onDidLayoutChange，F8 先例）；pageId 从 useLayout.activePageId 现取（交互必在活跃页）。viewMode/splitRatio（docViewer 面板形态）持久化走此辅助。

### DefaultTab 页签形态（TAB-01/02/03，IC-03，F9）

`DefaultTab` 为扁平化页签（`params.tabIcon` 已退役）：

- **状态圆点**：`params.tabStatus`（`AgentStatus | null`）→ 渲染 `StatusDot` 圆点（working 绿/attention 黄/done 灰/error 红）；null 不渲染。
- **CLI 品牌 logo**：`params.tabLogo`（F9 行为修订：**跟随页签名显示**，不依赖 tabStatus）由 TerminalPanel 会话绑定写入。
- **文件型页签图标**（TAB-03）：`params.filePath` 存在（FILE_PANEL_TYPES）→ 渲染 `FileIcon` 彩色图标；与终端分支互斥。
- **激活指示条**（TAB-01）：`isActive && isGroupActive` 时渲染底部 2px 指示条（absolute 锚定 `.dv-tab` 底边，色 `FOCUS_BORDER`，`pointerEvents: none`）。
- **hover 关闭 ×**（TAB-02）：× 默认不可见（opacity 0 + pointerEvents none），hover 时显现。
- **共享关闭守卫（FE-49，SC-FE-07 语义统一，CP-036 批量接入）**：`tabClose.ts` 的 `closeTabGuarded(api, panelId)`（单面板）——settings 面板且 dirty → `confirmDialog` 确认才 `api.close()`，其余直关；确认丢弃即清除 dirtyRegistry 条目（CP-017 契约）。**× / Ctrl+W / 鼠标中键 / 右键菜单「关闭」四路共用同一入口**（Ctrl+W 曾直调 `api.close()` 绕过守卫——F11 登记的不对称，FE-49 修复）；「关闭其他/关闭全部」批量路径经 `closeTabsGuarded` 统一入口（CP-036）：dirty 面板列表 + 单次确认，确认后清除 dirtyRegistry 条目再全部 close；无 dirty 零交互直关（非 settings 面板行为零回归）。**页删除路径（Workspace 删页 → 页组移除）显式不守卫登记（CP-036 收尾复核结论）**：整页销毁语义——连同运行中终端 PTY 一并 kill，页面删除即用户放弃页上一切，不做面板级 dirty 确认；被删页的 dirtyRegistry 条目随之滞留（pageId 不再复用，无查询命中面，有界可接受）。判据 = panelId 形态（`isSettingsPanelId` 单点——协议 id `{pageId}:settings`；DefaultTab 拿不到 panel——dockview 8.1.0 `IDockviewPanelProps` 无 panel 属性，`panel.view.contentComponent` 红线不适用该场景）；判据与 dirtyRegistry 键同源（SettingsPanel 以同一 params.panelId 注册），无漂移。`confirmDialog` 为 `src/lib` 命令式全局契约（无 React 依赖）——shortcuts 层可安全引用本模块。
- **鼠标中键关闭页签（FE-49）**：浏览器式交互——auxclick（完整按下+弹起，按下后拖离弹起即天然取消）在 DefaultTab 内容根触发，`e.button === 1` 判中键，目标 = 本页签自身 `api`（无需聚焦/激活，对比 Ctrl+W 的 activePanel 语义）；× 上的中键经冒泡同样关闭（click 仅主键，两路径互斥）。**autoscroll 预防**：dockview `.dv-tabs-container` 为 `overflow:auto`（可横向滚动），中键按住会启动 Chromium autoscroll——宿主容器根挂 capture mousedown 单点 `preventDefault`（覆盖所有分屏组 header 含缝隙/void 空白；只消默认动作不拦传播，dockview pointerdown 对 button!==0 本就 no-op）；关闭仍走 auxclick（mousedown preventDefault 不影响其触发）。dockview 8.1.0 对中键/auxclick 零消费，无冲突。

### Watermark 空态规范（GL-05/UI-806）

空白页组（含空宿主）由 `createWatermark` 组件接管（dockview 对空组渲染 watermarkComponent）：**15px 线性图标 + 说明文字 13px + 可选「新建终端」次按钮**（SECONDARY_BG 底 + SEPARATOR_BG 描边，点击经 `addTerminalPanel` 落活跃页组，`renderer: "always"`）。

### 项目切换前置：setProjectRoot 先于 activePageId（DBG-5/SEC-01）

`project_root` 是 `activePageId` 生效的前提，不是副作用。`switchToPage` 改为 async——先 `await setProjectRoot(rootPath)` 再 `setActivePage(pageId)`。`App.tsx` 启动恢复 `lastPage` 同样先 await 再切页。SEC-01 effect 保留兜底。

根因：React 同一 commit 的 passive effect 子组件先于父组件执行——旧代码中 `setProjectRoot` 在父 effect 执行时，子组件（ExplorerPanel）的 `fs_read_dir` 已因 `project_root=None` 被路径沙箱拒绝。

### 文件监听上提到项目激活层

SEC-01 effect 同时承担 `startWatch(rootPath)` / `stopWatch(prev)`——watcher 宿主从 ExplorerPanel 上提到项目激活层：编辑器外部修改 reload / commit 面板刷新等 fs-event 消费方不依赖 explorer 视图是否打开。`activePageId` 置 null（删除末页/移除活跃项目）时对 `prevRootRef.current` 调 `stopWatch` 并清 ref，防 OS 句柄残留至 LRU 淘汰。

### 面板注册表已提取到 `src/panelRegistry.ts`

`panelRegistry` / `PANEL_TYPES` / `FILE_PANEL_TYPES` / `isAlwaysRenderPanel` 是全局架构组件，被 workspace、explorer、测试等多方引用，不应埋于 workspace 子路径。新增面板类型仍按 #5 流程：创建目录 → 注册 → 追加 `PANEL_TYPES`。`isAlwaysRenderPanel` **已纳入 settings**（SC-FE-06 翻案，CP-017）：dirty 真值源（dirtyRegistry）脱离壳生命周期——壳不随页签切换卸载，dirtyMap/dirtyRegistry 条目跨切签存活。

### openSettingsPanel 同页单例（F11，SC-FE-02，CP-042 事件驱动）

`openSettingsPanel(pageId, settingsPageId?)` 在 pageApis.ts——面板 id = `panelIdInPage(pageId, "settings")`（`{pageId}:settings` 页前缀协议形态）；getPanel 命中 → focus 返回 true（同页单例），未命中 → addPanel（component "settings"，**renderer "always"（CP-017）**，settingsPageId 深链注入 params.selectedPage）；页面就绪 = 页组挂载事件驱动等待 `slterm:page-api-ready`（markPageGroupMounted 派发，CP-042），5s 超时仅作防御底线——超时经 toast 可观测化后返回 false。**调用方须先切到目标页**（本函数不切页）——编排见 `features/settingsCenter/openSettings.ts`（无项目 toast 拦截在编排层，R1）。

### 页签右键菜单自研（dockview 8.1 enterprise 缺位修复）

dockview 8.1.0 free core 的页签右键菜单(ContextMenu)是 **enterprise 模块**——`.dv-tab` 的 contextmenu 监听为 `contextMenuService?.show(...)`，该服务仅在商业付费包 `dockview-enterprise`（license key 激活）import 自注册时存在；free core 恒短路、事件不 preventDefault。**所有环境一致（非 jsdom 现象）**。6.6.1 → 8.1.0 升级（2026-08-18）后生产页签右键菜单从不弹出，曾以 jsdom fake contextMenuService 探针测试自证而滞留两周。

**因此页签右键菜单自研**（`createTabMenuItems` 纯函数 + `TabMenuPopup`），dockview 菜单形态代码（`getTabContextMenuItems` prop / `TabContextMenuItem` / `TAB_CONTEXT_MENU_CSS`）已删除：

- **触发**：DefaultTab 内容根 `onContextMenu`（preventDefault + stopPropagation 拦 WebView 原生菜单与库内死监听）。右键目标 = DefaultTab 渲染的 div——它是 `.dv-tab` 的**子级**，事件从 `.dv-tab` 自身派发冒泡不会经过它（测试须右键 DefaultTab 内容根，非 `.dv-tab`）。
- **上报链路**：DefaultTab 广播 `TAB_CONTEXT_MENU_EVENT`（`slterm:tab-context-menu`，window CustomEvent 协议，detail 带 panelId/x/y）——dockview-react 渲染 framework part 不经 React 子树，context 方案不可靠；宿主（唯一）监听，经 `getPanel(panelId)` 解析（panelId 页前缀全局唯一），命中即弹菜单。
- **状态单点**：菜单 state（x/y/items）在 WorkspaceDockHost（宿主单点）；`createTabMenuItems(getApi, pageId|null, onRenameRequest, projectRootPath?)` 为纯函数导出供 L2 直测（终端判据 `view.contentComponent`、claudeRunning disabled、文件型 `params.filePath` 头部「复制相对路径」、关闭族 danger、separator 令牌位置——与 dockview 6.6.1 原生菜单行为零漂移）。菜单目标页 = 面板属主页（panel.id 页前缀解析，兜底入参）；action 内面板/组引用取右键瞬间快照。
- **渲染**：`TabMenuPopup` fixed 定位（zIndex 1000），UI-802 规格内联 token（项 28px/hover SECONDARY_BG/danger ERROR_FG/disabled 0.4 + pointerEvents none），外点 mousedown / Escape / 点击项关闭；`activePageId` 变化（切页）清菜单。
- **能力边界**：DefaultTab 的 `IDockviewPanelProps` 无 panel 对象——面板经 `containerApi`/事件 detail 的 panelId 反查 `getPanel(id)`（`IDockviewPanel.group`、`group.panels` 均可达，结构兼容子集类型 `TabMenuPanel`）。

### 终端页签自定义重命名（F8）

- **入口**：页签右键菜单（见上「页签右键菜单自研」）对终端面板（判据 `panel.view.contentComponent === "terminal"`，`panel.component` 不存在）构建「重命名」项；claude 运行中（`TerminalRegistry.get(panel.id)?.agentSession != null`）→ `disabled` 置灰。
- **存储单一真值源**：`params.customTitle`（随布局 JSON 持久化）。`applyRename` 纯函数 = `updateParameters({ customTitle })` + `setTitle` + **显式保存布局**（`saveLayout(api)` 全量 → syncHostLayoutToStore 切片落盘）——`setTitle`/`updateParameters` 均不触发 `onDidLayoutChange`，须显式保存。
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

### `__dockviewApi` 宿主唯一不变量（CP-004 收敛）

`window.__dockviewApi` 恒指向共享宿主 DockviewApi（`WorkspaceDockHost` onReady 置位，宿主唯一）——不再随切页重指。其他代码点经 `getPageApi(pageId)`（宿主就绪 + 页组挂载标记后返回宿主 API）访问指定页面的语义查询。宿主卸载经 disposablesRef 消费清理（FE-09）——apiRef/`__dockviewApi`/unregisterHostApi 置空。

### E2E 测试支持

- `window.__slterm_e2e_workspaceReady`：Workspace 挂载时同步设置（渲染阶段，非 `useEffect`），WDIO 轮询等待就绪。
- `window.__dockviewApi`：恒为共享宿主 DockviewApi（宿主唯一）。
- 宿主容器锚点 class `slterm-dock-host`。

### 旧格式兼容（布局迁移）

`layoutSerde.ts` 的迁移/修补处理早期布局缺失字段与旧多实例格式：
- `component` → `contentComponent` 迁移；`grid.orientation` 缺失 → `"HORIZONTAL"`；`leaf.data.id` 缺失 → 从 `views[0]` 生成；顶层 `activeGroup` 缺失 → 用第一个 leaf 的 id。
- **旧多实例格式 → 页组切片迁移（CP-004）**：每页独立全量布局（组 id 无 `page-` 前缀）压平为该页单一页组——面板 id 重写页前缀协议（`{pageId}:{oldId}`，params.panelId 同步）；幽灵引用/白名单外类型丢弃 + console.error（不阻断启动）。

## 外部坑/红线

- **xterm.js 不可二次 `open()`**：页组可见性切换（隐藏/重现）必须保证面板不卸载重建——dockview 组隐藏保留 DOM 已实证，勿改用组移除/重建方案。
- **Dockview `onDidParametersChange` 扁平结构**：回调直接是 `Parameters`，不要写成 `event.params.xxx`。
- **`panel.component` 不存在**：判断面板类型用 `panel.view.contentComponent`。
- **新建终端编号延迟分配（FE-04）**：localId 编号在菜单 action 执行时才分配（`lib/panelId` 模块级计数），菜单构建期不消耗编号。
- **重命名必须显式保存布局**：`setTitle`/`updateParameters` 不触发 `onDidLayoutChange`，须显式保存。
- **删除页面时 stopWatch**：`activePageId` 置 null 必须释放 watcher，否则 OS 句柄残留。
- **renderer="always" 白名单**：`terminal`、`htmlviewer`、`markdownviewer` 与 `settings`（panelRegistry.ts 单点）。editor/gitshow/diff 故意排除——CM6 重建无视觉闪屏，且大文件编辑器若始终挂载会显著增加内存开销；markdownviewer 纳入因 iframe browsing context 与 CM 编辑实例切走切回不重建（草稿/缩放保活，决策 #17）；settings 纳入因 dirty 真值源脱离壳生命周期（CP-017）。隐藏页组内恒挂载面板保持挂载（页组级显隐不改变白名单语义）。
- **页组可见性 = dockview gridview maximize**：勿用 CSS display 直接操作 dockview 内部叶元素（布局管理器不知情会错位）；页切换/恢复后经 `maximizePageGroup` 单点刷新。
- **whole-grid fromJSON（页并入）会重建组对象**：面板实例经 reuseExistingPanels 存活（内容不重挂载），但组对象引用会变——勿缓存 group 对象跨并入操作断言 identity。

## 测试模式

- **页组协议纯函数**：page-groups.test.ts（id 往返/越界判定/页过滤）。
- **真实 Dockview 集成（非 mock）**：workspace-page-dockview.test.tsx 渲染真实宿主（种子 projects store 驱动恢复/水印/右键菜单/onSaveAs——jsdom 可跑真实 dockview）；workspace-host-pages.test.tsx（多页组共存/切页不卸载/H6）；workspace-callback-cache.test.tsx（页面目录变更不扰动既有页组）。
- **宿主 API 单例重置**：beforeEach `unregisterHostApi()` + stores 重置（宿主级模块态隔离）。
- **回迁守卫**：workspace-cross-page-guard.test.ts（enforcePanelGroupMembership 直测——越界回迁/空壳清理/失败降级）。
- **layoutSerde**：旧格式修补 + 白名单过滤 + 深拷贝 + 页切片/汇编/迁移两分支。
- **切换时序**：测 `setProjectRoot` 先于 `setActivePage` 生效（workspace-page-apis.test.ts）。
