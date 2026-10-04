# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

侧栏视图系统提供 VS Code 风格的活动栏 + 共享侧栏区。NAV-05 起注册视图：`nav`（导航树）、`explorer`（文件浏览器）、`commit`（Commit）；ADR-0024 追加 `agentFiles`（Agent 全局文件，实现归 `features/agentFiles/`）。原 `projects` 与 `agent-status` 视图随 NAV-06/08 退役，职责并入导航树。活动栏底部固定「配置」钮**不入注册表**。

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

1. 实现 ViewComponent（接受 `SideViewComponentProps = { switchToPage, onDeletePage, viewState?, onViewStateChange? }`——CP-016 起后两槽位为跨挂载状态受控消费接口：`viewState` 挂载时回填、`onViewStateChange` 状态变化时上呼）。
2. 在 `sideViewDefs.ts` 加一行 `sideViewRegistry.register({ id, title, icon, component })`。

框架自动处理：活动栏按钮渲染与开关、上区/下区拖拽归属、槽位 display:none/flex 切换、持久化。

默认按钮归属（`DEFAULT_ZONES`）：**top = `["nav", "explorer", "commit", "agentFiles"]`，bottom = `[]`**；`DEFAULT_OPEN.top = "nav"`（默认打开导航树）。**活动栏固定宽度 `ACTIVITY_BAR_SIZE = 46`**（NAV-05/GL-04：40 → 46，Workspace 同步引用）。

**「配置」钮（NAV-05 例外）**：id `config`、图标 IconConfig——**固定渲染于活动栏底部，不入 SideViewRegistry**（不参与拖拽/换区/持久化），点击 = `openSettings()`（设置中心唯一入口，F11，取代旧 openHooksConfigFromActivityBar）；编排细节见 `features/settingsCenter/CLAUDE.md`。

### HTML5 拖拽——外层容器统一处理 + 容器中点 zone 判定

活动栏拖拽采用 HTML5 原生 DnD API，零外部依赖。

**架构决策**：`onDragOver`/`onDrop`/`onDragLeave` 在外层容器上统一处理，而非各 zone div 各自处理。**根因**：空 zone div 高度为 0，Chromium hit-test 跳过零高度元素，导致 `dragover`/`drop` 永不触发。外层容器 `height: 100%` 全高永远可命中。

**zone 判定**：`resolveTargetZone(clientY, root)`——容器垂直中点以上 → `"top"`，以下 → `"bottom"`。同区内精确定位用 `computeDropTarget` 纯函数：clientY 在按钮上半→该按钮 index（插前方），下半→index+1（插后方），空白区→数组末尾。

- 起点：按钮 `draggable` + `onDragStart` → `dataTransfer.setData("application/x-side-view-id", id)`。
- 指示线：`onDragOver` 调 `computeDropTarget` → set `dropIndicator` state → 渲染 2px 指示条（色 `FOCUS_BORDER`）。
- 执行：`onDrop` → `useSideBar.getState().moveButton(id, zone, index)`。
- 拖拽仅活动栏内有效：外部不监听 drop，按钮不能拖出活动栏。

### 关闭语义——按需卸载（FE-21）+ 状态槽回填（CP-016）

- **槽位内切换（FE-21 按需卸载）**：同一半区内切换视图时**旧视图组件卸载**（条件渲染仅挂载当前打开视图），隐藏视图不保挂载——DOM/订阅随卸载释放。跨挂载状态经注册表状态槽存活，卸载不再等于丢状态（见下）。
- **状态槽回填（CP-016）**：视图跨挂载状态经 `sideViewRegistry` 状态槽（`getViewState`/`setViewState`，`_reset` 同清）以视图 id 为键持久——SideBarArea 向视图透传 `viewState={sideViewRegistry.getViewState(def.id)}` 与 `onViewStateChange`（上呼写回槽位，组件经 `SideViewComponentProps.viewState/onViewStateChange` 受控消费）。槽位切换/换区卸载重建后由回填恢复，不再依赖组件内部 state。
- **首次双开 splitRatio 回退（FE-19）**：从单视图过渡到双视图时，`SideBarArea` 的 `useEffect` 仅当 `splitRatio` 为默认值或越界（出 [0.1,0.9]）才回退 0.5；用户调节过的合法比例在单↔双切换中保留。**真实机理（2026-09 更正）**：视觉恢复 = 「比例像素纠偏」effect（见下）；该 effect 仅作 store 值归一化兜底，不驱动像素重排。
- **splitRatio → 像素 = ref.resize() 主动纠偏（红线：勿用 preferredSize 承载比例）**：Allotment 的 preferredSize 无法表达比例语义——number 是绝对像素；`"NN%"` 百分比字符串虽在 PaneView 层有策略分派，但**挂载路径把原始 prop 喂给 `resizeView`（库内 `Math.round("NN%")` → NaN）**，且挂载时容器尺寸未就绪（库经 ResizeObserver 异步测量）——两条路实证均失效（E2E 实测下区恒 30px）。现机制（2026-09 二轮重写）：pane 不传 preferredSize；双开（挂载即双开 = 重启恢复 / 单→双转换 = 首开或重开下区）时纠偏 effect **同步快照 splitRatio + 置纠偏窗口标记**，rAF 有界轮询（~60 帧）待「就绪探针」（首个 `sizes.length===2` 的 onChange fire = viewItems 已 populate）后按快照调 `ref.resize([h*r, h*(1-r)])`；onChange 经三道闸后才写回 store：①纠偏窗口期屏蔽 ②任一 pane 零尺寸跳过 ③`getState()` live 判定双开（禁读渲染闭包）。**三条库坑（modern.mjs v1.20.5 实证，勿再趟）**：a) viewItems 异步 populate——view 加入走「ResizeObserver 首测 → setState → update effect 才 addView」异步链，mount commit 内 viewItems 必为空，此时调 resize() 读 `undefined.minimumSize` 崩溃（Bug 1），且 `this.size=0` 时 resizeViews 的 lodash clamp 把请求钉成 `[30,30]`；b) onDidChange 回调**慢一个 commit** 换闭包——关闭下区 commit 的 layout 期 `setViewVisible(false)` 同步 fire `onChange [H,0]` 仍用上一 commit bothOpen=true 旧闭包 → ratio=1.0 → clamp 0.9 污染 store（Bug 2，故 onChange 必须 live 判定）；c) mount 链瞬时 fire `[H-30,30]`/`[H/2,H/2]` 会冲掉持久化比例（故纠偏窗口期屏蔽写回 + 快照免疫）。单开/关区不纠偏。历史事故：af342f7 时代按像素传值，致比例恢复/首开下区只剩标题高度，且 onChange 每帧把腐蚀像素比写回 clamp 0.9 落盘形成**比例棘轮**（持久化值恒收敛 0.9）；当时曾把 0.9 误诊为「残留脏数据」。配套清洗：`sanitizeSideBar` 对**原始读盘值**恰好 0.9 重置 0.5（棘轮指纹，先于 clamp 判等——clamp 后判会误伤 2.5/99 等正常越界值；已知误伤 = 手动拖到上限的合法 0.9 一次性被重置）。

## 外部坑/红线

- **空 zone div 不接收 drag 事件**：Chromium hit-test 跳过零高度元素，必须把 `onDragOver`/`onDrop` 挂在外层全高容器。
- **配置钮不入注册表**：`SideViewRegistry` 操作（拖拽/换区/持久化/注册表对齐）均不处理 `config`，ActivityBar 单独渲染。
- **换区重建（状态槽已覆盖，勿回退丢状态口径）**：跨区拖拽仍会卸载并重建视图组件（渲染形态不变），但跨挂载状态经注册表状态槽回填恢复——视图不得再以组件内部 state 承载展开集等跨挂载状态，否则重建即丢。
- **FE-21 隐藏视图卸载 + 新视图须自消费状态槽**：同一槽位切换时旧视图完全卸载，不能假设隐藏视图仍在 DOM 或保留订阅；新增侧栏视图若持有跨挂载状态（展开集等），须经 viewState/onViewStateChange 上移注册表状态槽（照 explorer/useFileTree 模式），否则切换仍丢状态（视图自身负责）。
- **配色全部走 token**：ActivityBar 全部颜色引用 `theme/colors.ts`，禁止硬编码（硬约束 #6）。

## 测试模式

- **纯函数层**：覆盖 `toggleViewPure`/`moveButtonPure`/`deriveLayout`/`reconcileZones`/`sanitizeSideBar` 全分支 + S1-S6 场景序列。
- **Store 层**：覆盖默认值、toggle/move 委托纯函数、width/splitRatio clamp、loadFromDisk sanitize、2s debounce 持久化。
- **拖拽测试**：必须 mock `getBoundingClientRect` 为按钮 + 容器提供模拟矩形；drag 事件向外层容器 `[data-e2e="activity-bar"]` 派发（非 zone div）。
- **视图槽条件渲染**：验证 FE-21 仅挂载当前打开视图，隐藏槽不渲染；换区后旧区卸载、新区挂载。
- **Workspace 集成**：验证活动栏 pane 46px 固定、侧栏区 pane visible=anyOpen、主区 minSize=200。
