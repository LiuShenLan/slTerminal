# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

侧栏视图系统提供 VS Code 风格的活动栏 + 共享侧栏区。NAV-05 后注册三条视图：`nav`（导航树）、`explorer`（文件浏览器）、`commit`（Commit）。原 `projects` 与 `agent-status` 视图随 NAV-06/08 退役，职责并入导航树。活动栏底部固定「配置」钮**不入注册表**。

## 关键约束与决策

### 单槽位状态机——无历史记忆，展示纯推导

侧栏区上下各一槽位。当前展示由三要素纯推导：`open`（当前打开的视图 id）、`zones`（按钮归属）、注册表（可用视图定义）。无"上次打开"历史记忆，无中间态。

**操作规则**：

- **R1（打开/替换）**：点击按钮 → 若该半区槽位为空或为其他视图 → 打开该视图（直接覆盖，隐式关闭同区旧视图）。
- **R2（关闭）**：点击已打开的按钮 → 关闭该半区（槽位置 null）。
- **R3-R5（布局推导）**：双空→侧栏区隐藏；仅上→single-top（下 pane hidden）；仅下→single-bottom；双开→split。
- **R6（跨区跟随+替换）**：拖拽按钮跨区 → 若该视图已打开 → 跟随到目标区并替换目标区旧视图。
- **R7（跨区未打开）**：拖拽按钮跨区 → 视图未打开 → 仅归属变化，展示不变。
- **R8（同区排序）**：同区内拖拽 → 仅调按钮顺序，不动打开状态。
- **R9（注册表对齐）**：持久化恢复时过滤已取消注册的 id，缺失的注册 id 追加到上区末尾。

全部状态函数（`toggleViewPure` / `moveButtonPure` / `deriveLayout` / `reconcileZones` / `sanitizeSideBar`）为纯函数——输入 zones + open，返回新状态，不访问 DOM/React/store。

### SideViewRegistry 扩展指南

`SideViewRegistry` 是模块级单例，管理侧栏视图定义（`SideViewDef` = id + title + icon + React 组件）。ActivityBar 通过此注册表渲染按钮，SideBarArea 通过它渲染视图槽。

**新增侧栏视图只需两步**：

1. 实现 ViewComponent（接受 `SideViewComponentProps = { switchToPage, onDeletePage }`）。
2. 在 `sideViewDefs.ts` 加一行 `sideViewRegistry.register({ id, title, icon, component })`。

框架自动处理：活动栏按钮渲染与开关、上区/下区拖拽归属、槽位 display:none/flex 切换、持久化。

默认按钮归属（`DEFAULT_ZONES`）：**top = `["nav", "explorer", "commit"]`，bottom = `[]`**；`DEFAULT_OPEN.top = "nav"`（默认打开导航树）。**活动栏固定宽度 `ACTIVITY_BAR_SIZE = 46`**（NAV-05/GL-04：40 → 46，Workspace 同步引用）。

**「配置」钮（NAV-05 例外）**：id `config`、图标 IconConfig——**固定渲染于活动栏底部，不入 SideViewRegistry**（不参与拖拽/换区/持久化），点击 = `openHooksConfigFromActivityBar()`。决策 4 入口唯一化：目标项目 = 活跃页面所属优先兜底第一个；先 `switchToPageShared` 再 `openHooksConfigPanel`。

### HTML5 拖拽——外层容器统一处理 + 容器中点 zone 判定

活动栏拖拽采用 HTML5 原生 DnD API，零外部依赖。

**架构决策**：`onDragOver`/`onDrop`/`onDragLeave` 在外层容器上统一处理，而非各 zone div 各自处理。**根因**：空 zone div 高度为 0，Chromium hit-test 跳过零高度元素，导致 `dragover`/`drop` 永不触发。外层容器 `height: 100%` 全高永远可命中。

**zone 判定**：`resolveTargetZone(clientY, root)`——容器垂直中点以上 → `"top"`，以下 → `"bottom"`。同区内精确定位用 `computeDropTarget` 纯函数：clientY 在按钮上半→该按钮 index（插前方），下半→index+1（插后方），空白区→数组末尾。

- 起点：按钮 `draggable` + `onDragStart` → `dataTransfer.setData("application/x-side-view-id", id)`。
- 指示线：`onDragOver` 调 `computeDropTarget` → set `dropIndicator` state → 渲染 2px 指示条（色 `FOCUS_BORDER`）。
- 执行：`onDrop` → `useSideBar.getState().moveButton(id, zone, index)`。
- 拖拽仅活动栏内有效：外部不监听 drop，按钮不能拖出活动栏。

### 关闭语义——按需卸载（FE-21）+ 换区重建

- **槽位内切换（FE-21 按需卸载）**：同一半区内切换视图时**旧视图组件卸载**（条件渲染仅挂载当前打开视图），隐藏视图不保挂载——DOM/订阅随卸载释放。状态丢失语义 ADR-0001 已接受。
- **换区重建（已知行为）**：按钮被拖拽跨区时，zones 变化导致视图组件从上区 pane 移入下区 pane。React 将其视为不同父节点下的组件，触发卸载+重建——组件内部状态丢失。ADR-0001 已确认接受。
- **首次双开 splitRatio 回退（FE-19）**：从单视图过渡到双视图时，`SideBarArea` 的 `useEffect` 仅当 `splitRatio` 为默认值或越界（出 [0.1,0.9]）才回退 0.5；用户调节过的合法比例在单↔双切换中保留。

## 外部坑/红线

- **空 zone div 不接收 drag 事件**：Chromium hit-test 跳过零高度元素，必须把 `onDragOver`/`onDrop` 挂在外层全高容器。
- **配置钮不入注册表**：`SideViewRegistry` 操作（拖拽/换区/持久化/注册表对齐）均不处理 `config`，ActivityBar 单独渲染。
- **换区重建丢失状态**：跨区拖拽会卸载并重建视图组件，导航树滚动位置、文件树展开状态等都会丢失。ADR-0001 已接受。
- **FE-21 隐藏视图卸载**：同一槽位切换时旧视图完全卸载，不能假设隐藏视图仍在 DOM 或保留订阅。
- **配色全部走 token**：ActivityBar 全部颜色引用 `theme/colors.ts`，禁止硬编码（硬约束 #6）。

## 测试模式

- 测试文件位于 `src/__tests__/`：`sideBarState.test.ts`、`sideViewRegistry.test.ts`、`sideBar.test.ts`、`activityBar.test.tsx`、`sideBarArea.test.tsx`、`workspace-sideviews.test.tsx`。
- **纯函数层**：`sideBarState.test.ts` 覆盖 `toggleViewPure`/`moveButtonPure`/`deriveLayout`/`reconcileZones`/`sanitizeSideBar` 全分支 + S1-S6 场景序列。
- **Store 层**：`sideBar.test.ts` 覆盖默认值、toggle/move 委托纯函数、width/splitRatio clamp、loadFromDisk sanitize、2s debounce 持久化。
- **拖拽测试**：`activityBar.test.tsx` 必须 mock `getBoundingClientRect` 为按钮 + 容器提供模拟矩形；drag 事件向外层容器 `[data-e2e="activity-bar"]` 派发（非 zone div）。
- **视图槽条件渲染**：`sideBarArea.test.tsx` 验证 FE-21 仅挂载当前打开视图，隐藏槽不渲染；换区后旧区卸载、新区挂载。
- **Workspace 集成**：`workspace-sideviews.test.tsx` 验证活动栏 pane 46px 固定、侧栏区 pane visible=anyOpen、主区 minSize=200。
