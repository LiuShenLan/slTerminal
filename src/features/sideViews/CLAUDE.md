# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

侧栏视图系统——活动栏（ActivityBar）+ 共享侧栏区（SideBarArea）+ 单槽位状态机。替代旧 Allotment 常驻栏（SidebarTree + ExplorerPanel 各占固定栏位），改为 VS Code 风格的活动栏+视图槽模式。

## 架构决策

### 单槽位状态机——无历史记忆，展示纯推导

侧栏区上下各一槽位（单槽位模型）。当前展示由三要素纯推导：`open`（当前打开的视图 id）、`zones`（按钮归属）、注册表（可用视图定义）。无"上次打开"历史记忆，无中间态。

**操作规则**：
- **R1（打开/替换）**：点击按钮 → 若该半区槽位为空或为其他视图 → 打开该视图（直接覆盖，隐式关闭同区旧视图）
- **R2（关闭）**：点击已打开的按钮 → 关闭该半区（槽位置 null）
- **R3-R5（布局推导）**：双空→侧栏区隐藏；仅上→single-top（下 pane visible=false）；仅下→single-bottom；双开→split
- **R6（跨区跟随+替换）**：拖拽按钮跨区 → 若该视图已打开 → 跟随到目标区并替换目标区旧视图
- **R7（跨区未打开）**：拖拽按钮跨区 → 视图未打开 → 仅归属变化，展示不变
- **R8（同区排序）**：同区内拖拽 → 仅调按钮顺序，不动打开状态
- **R9（注册表对齐）**：持久化恢复时过滤已取消注册的 id，缺失的注册 id 追加到上区末尾

全部状态函数（`toggleViewPure`/`moveButtonPure`/`deriveLayout`/`reconcileZones`/`sanitizeSideBar`）为纯函数——输入 zones + open，返回新状态，不访问 DOM/React/store。

### SideViewRegistry 扩展指南

`SideViewRegistry` 是模块级单例（同 TabTitleRegistry 模式），管理侧栏视图定义（`SideViewDef` = id + title + icon + React 组件）。ActivityBar 通过此注册表渲染按钮，SideBarArea 通过它渲染视图槽。

**新增侧栏视图只需两步**：
1. 实现 ViewComponent（接受 `SideViewComponentProps = { switchToPage, onDeletePage }`）
2. 在 `sideViewDefs.ts` 加一行 `sideViewRegistry.register({ id, title, icon, component })`

框架自动处理：活动栏按钮渲染与开关、上区/下区拖拽归属、槽位 display:none/flex 切换、持久化。

与项目现有注册表对比：同 `TabTitleRegistry`（`panels/terminal/`）的 `register/getAll/_reset` 模式；同 `ShortcutRegistry` 的模块级单例模式；同 `FileViewerRegistry` 的注册即用零额外配置模式。

### HTML5 拖拽——外层容器统一处理 + 容器中点 zone 判定

活动栏拖拽采用 HTML5 原生 DnD API，零外部依赖（不引入 react-dnd / dnd-kit）。

**架构决策**：`onDragOver`/`onDrop`/`onDragLeave` 在外层容器（`<div ref={barRef} data-e2e="activity-bar" style={containerStyle}>`）上统一处理，而非各 zone div 各自处理。**根因**：空 zone div 高度为 0，Chromium hit-test 跳过零高度元素（`codereview.chromium.org/2234173002`），导致 `dragover`/`drop` 永不触发。外层容器 `height: 100%` 全高永远可命中。

**zone 判定**：`resolveTargetZone(clientY, root)`——容器垂直中点以上 → `"top"`，以下 → `"bottom"`（`clientY >= rect.top + rect.height / 2`）。同区内精确定位仍用 `computeDropTarget` 纯函数。

- **起点**：按钮 `draggable` + `onDragStart` → `dataTransfer.setData("application/x-side-view-id", id)` + `effectAllowed = "move"` + 按钮半透明态
- **落点判定**：`computeDropTarget(clientY, buttonRects, zone)` 纯函数——clientY 在按钮上半→该按钮 index（插前方），下半→index+1（插后方），空白区→数组末尾。调用方通过 `barRef.current.querySelector` + `getBoundingClientRect()` 收集按钮矩形传入，函数本体不碰 DOM
- **指示线**：`onDragOver` 调 `computeDropTarget` → set `dropIndicator` state → 渲染 2px 指示条（颜色=FOCUS_BORDER token）
- **执行**：`onDrop` → `useSideBar.getState().moveButton(id, zone, index)`（委托 `moveButtonPure` 纯函数）
- **拖拽仅活动栏内有效**：外部不监听 drop，按钮不能拖出活动栏

### 关闭语义——display:none 保挂载 + 换区重建

- **槽位内切换（display:none）**：同一半区内切换视图时，旧视图 div `display:none`，新视图 div `display:flex`。两者均保持挂载——状态不丢（同 React 条件渲染 vs display 策略的经典权衡）
- **换区重建（已知行为）**：当按钮被拖拽跨区（上→下或下→上），zones 变化导致视图组件从上区 pane 移入下区 pane。React 将其视为不同父节点下的组件，触发卸载+重建——组件内部状态丢失。此行为在 ADR-0001 中已确认接受（权衡：换区为低频操作，重建成本低于跨父节点保持实例的复杂度）
- **首次双开 splitRatio 重置**：从单视图（仅一个半区有打开视图）过渡到双视图时，`SideBarArea` 通过 `useEffect` 将 `splitRatio` 重置为 0.5，确保上下各半——避免残留上次手动调节的极端值（如 0.9/0.1）导致一侧不可见

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export：组件（ActivityBar/SideBarArea）、注册表（sideViewRegistry）、类型、纯函数。不 re-export sideViewDefs（由 Workspace 显式 side-effect import，同 tabRules 模式） |
| `sideBarState.ts` | 类型定义（Zone/Zones/OpenState/LayoutKind/SideBarSlice）+ 默认值常量（DEFAULT_ZONES/DEFAULT_OPEN/ACTIVITY_BAR_SIZE/WIDTH_* /SPLIT_*）+ 纯函数（toggleViewPure/moveButtonPure/deriveLayout/reconcileZones/sanitizeSideBar） |
| `sideViewRegistry.ts` | `SideViewRegistry` 模块级单例：`register(def)`（同 id 覆盖）/`getAll()`（注册序）/`get(id)`/`_reset()`（仅测试） |
| `dropTarget.ts` | `computeDropTarget` 纯函数——零 DOM 访问，根据 clientY + 按钮矩形列表计算落点 zone + index |
| `sideViewDefs.ts` | side-effect 注册文件：注册 `projects`（📋 项目列表）+ `explorer`（📁 文件浏览器）+ `commit`（🔀 Commit）三条视图。新增视图在此追加 `sideViewRegistry.register(...)` |
| `ActivityBar.tsx` | 活动栏组件：40px 宽 flex 列（上区按钮组 + flex:1 间隔 + 下区按钮组）、点击开关（R1/R2）、HTML5 拖拽换区/排序、VS Code 风格 active 态指示条（左侧 2px FOCUS_BORDER）、`data-e2e` 选择器。配色全部 `theme/colors.ts` token（硬约束 #6） |
| `SideBarArea.tsx` | 侧栏区组件：`<Allotment vertical proportionalLayout>` 两 pane（上/下半区），每 pane `visible={!!open[zone]}`、`preferredSize` 由 splitRatio 控制；半区内按 zones 顺序渲染视图槽（`display: open[zone]===v.id ? "flex" : "none"` 保挂载）；onChange 仅双开时换算 ratio 写回 store |

## 硬约束

- **#1（前端不碰 OS）**：侧栏视图纯前端 UI 层，不涉及系统调用。store 持久化经 `src/ipc/settings`。
- **#6（配色单点）**：ActivityBar 全部颜色引用 `theme/colors.ts` token（PANEL_BG/SIDEBAR_COLORS/SIDEBAR_FG/FOCUS_BORDER），禁止硬编码色值。
- **不改组件本体**：SidebarTree 和 ExplorerPanel 组件内部逻辑零改动——仅宿主从 Allotment 常驻栏变为侧栏区视图槽。

## 关键集成点

- **`src/stores/sideBar.ts`** — Zustand store（zones/open/width/splitRatio + 持久化），组件订阅、纯函数委托
- **`src/workspace/Workspace.tsx`** — 三栏 Allotment 容器：pane1=ActivityBar（40px 固定）、pane2=SideBarArea（visible=anyOpen）、pane3=主区
- **`src/features/sidebar/SidebarTree.tsx`** — `projects` 视图组件（本体不改，仅宿主变化）
- **`src/features/explorer/ExplorerPanel.tsx`** — `explorer` 视图组件（本体不改，仅宿主变化；换区重建丢失展开状态——ADR-0001 已确认接受）
- **`src/App.tsx`** — init() 中 `useSideBar.loadFromDisk()` + 关窗冲刷 `cancelPendingSave()`

## 测试模式

测试文件位于 `src/__tests__/`：`sideBarState.test.ts`（50 用例）、`sideViewRegistry.test.ts`（7 用例）、`sideBar.test.ts`（19 用例）、`activityBar.test.tsx`（29 用例）、`sideBarArea.test.tsx`（14 用例）、`workspace-sideviews.test.tsx`（13 用例）。共 132 用例。

### activityBar 拖拽测试要点

拖拽事件向外层容器 `[data-e2e="activity-bar"]` 派发（非 zone `<div>`）。关键辅助函数：
- `installRectSpy(buttonRects, bottomZoneTop)`：mock `getBoundingClientRect`，为按钮 + zone 容器 + 活动栏根容器（height=600, midpoint=300）统一提供模拟矩形
- `dispatchDragEvent(element, type, dt, clientY?)`：可选 `clientY` 供 zone 判定使用
- 29 用例覆盖：渲染结构（3）、active 态（2）、点击 toggle（2）、title/e2e（1）、dragStart（2）、dragOver/drop 同 zone（3）、dragEnd（1）、未注册防御（1）、hover（2）、跨区拖拽状态机（5）、zone 判定边界（3）、指示线/清理（4）

### 技术栈

- Vitest（jsdom 环境）+ React Testing Library（`render` / `fireEvent`）
- Zustand stores 使用真实实现 + `beforeEach` 种子
- Allotment 组件 mock（捕获 Pane props 验证 visible/preferredSize）

### 纯函数测试（sideBarState + dropTarget）

`sideBarState.test.ts`（50 用例）：
- `toggleViewPure` 4 分支（R1 打开/替换、R2 关闭、未在 zones 防御）
- `moveButtonPure` 8 分支（跨区跟随替换 R6、跟随空区、未打开仅归属 R7、区内前移/后移/同位、index clamp）
- `deriveLayout` 4 态（hidden/single-top/single-bottom/split = R3/R4/R5）
- `reconcileZones` 5 分支（过滤未注册 id、补全缺失 R9、open 指向未知 id→null、保留 saved 顺序）
- `sanitizeSideBar` 6 分支（非对象→默认、zones/open 结构校验、width/splitRatio clamp）
- **S1-S6 场景序列**（连续调用纯函数断言终态，直译需求 §5 验收用例）

`sideViewRegistry.test.ts`（7 用例）：register/getAll/get、重复注册覆盖、未注册 get→undefined、`_reset` 隔离。照 TabTitleRegistry.test.ts 模式。

### Store 测试（sideBar.test.ts）

19 用例照 fontSize/keybindings.test.ts 模式：
- 默认值、toggle/move 经 store 委托纯函数
- setWidth/setSplitRatio 内部 clamp
- loadFromDisk：合法/脏数据 sanitize/缺失/异常降级（5 分支）
- reconcileZones 对齐注册表
- loaded 守卫防启动空写
- fake timers 验证 2s debounce → `saveSettings({sideBar})` payload 键集合精确匹配
- saveSettings 失败静默吞错
- mock `../ipc/settings`

### UI 组件测试

`activityBar.test.tsx`（16 用例）：
- 两组按钮渲染（上/下分组+按 zones 顺序）、active class+指示条（VS Code 风格）
- 点击→toggleView 调用、再次点击关闭（R1/R2）
- tooltip/title + `data-e2e` 属性
- dragStart dataTransfer 内容 + 半透明态
- dragOver computeDropTarget 调用 + dropIndicator 渲染
- drop→moveButton 参数（zone/index）+ dragEnd 清拖拽状态
- zones 含未注册 id 正常渲染不抛异常
- hover 态背景色（非 active 时）vs active 态不覆盖
- 种子 sideBar store（真实）+ 手动注册测试视图（`_reset` 后 register stub 组件）；mock dataTransfer

`sideBarArea.test.tsx`（13 用例）：
- 四态渲染：单开上（下 pane hidden）、单开下（上 pane hidden）、双开（两 pane visible）、全关无用例（Workspace 层 whole sidebar hidden 不挂载）
- Allotment preferredSize = splitRatio×100 / (1-splitRatio)×100
- 视图槽 display:none/flex 切换 + display:none 保挂载
- 换区后视图移槽——旧区组件卸载、新区组件挂载（重建行为验证）
- switchToPage/onDeletePage props 透传到视图组件
- onChange 双开时换算 ratio→setSplitRatio、单开时除零守卫
- PANEL_BG token（非硬编码色值）+ 持久化中未注册 id 过滤
- mock Allotment 捕获 Pane props

`workspace-sideviews.test.tsx`（13 用例）：
- 活动栏 pane 40px 固定（preferred/min/max）
- 侧栏区 pane visible=anyOpen 四态（hidden→false、三种开→true）
- preferredSize 来自 store width、onChange→setWidth
- 主区 pane minSize=200 保持、SideBarArea props 透传
- 真实 sideBar store 种子 + mock Allotment

### 运行

```bash
npm test                                              # L2 全量（含侧栏视图）
npx vitest run sideBarState sideViewRegistry sideBar  # 仅纯函数 + store
npx vitest run activityBar sideBarArea workspace-sideviews  # 仅 UI 组件
```
