# Stage 2 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 覆盖清单 ID：SB-14~SB-20（docs/sidebar-checklist.md「Stage 2 — UI 组件」）。

## 断言清单

- **SB-14**：`src/features/sideViews/ActivityBar.tsx` 存在，Read 确认：40px 宽 flex 列结构（上区按钮组 + `flex:1` 间隔 + 下区按钮组）；按钮 40×40、emoji 居中；每按钮带 `data-e2e={"activity-btn-" + id}`（grep `activity-btn-` 命中）与 `title={def.title}`；active 判定为 `open[zone] === id`（zone 从 store `zones` 查归属，Read 确认——**语义式**：active 不接受任何本地副本/独立 state）；active 态渲染高亮背景 + 左侧 2px 指示条；点击 → `toggleView(id)`。
- **SB-14-COLOR（硬约束 #6）**：grep `#[0-9a-fA-F]{3,6}` 在 `ActivityBar.tsx` 与 `SideBarArea.tsx` 均为零；Read 确认全部颜色经 `theme/colors.ts` token 引用（bg `PANEL_BG` 或 `SIDEBAR_COLORS.bg`、hover `SIDEBAR_COLORS.hover`、active `SIDEBAR_COLORS.selected`、指示条/插入线 `FOCUS_BORDER`、边框 `SIDEBAR_COLORS.border`、文字 `SIDEBAR_FG`/`SIDEBAR_COLORS.fg`）。
- **SB-15**：Read 确认 HTML5 拖拽：按钮 `draggable`；`onDragStart` 中 `dataTransfer.setData("application/x-side-view-id", id)`（grep `application/x-side-view-id` 命中）且 `effectAllowed = "move"`；按钮/半区容器 `onDragOver` 调 `preventDefault()` 并经 `computeDropTarget` 计算落点写入 `dropIndicator` state `{ zone, index }`，渲染插入指示线；`onDrop` → `moveButton(id, zone, index)`；`onDragEnd`/`onDragLeave` 清指示线。**语义式**：不存在任何监听活动栏之外 drop 目标的代码（拖拽仅活动栏内有效）。
- **SB-16**：`src/features/sideViews/SideBarArea.tsx` 存在，Read 确认：props `{ switchToPage; onDeletePage }`；`<Allotment vertical proportionalLayout onChange={...}>` 两 pane——上 pane `visible={!!open.top}` `preferredSize={splitRatio * 100}`，下 pane `visible={!!open.bottom}` `preferredSize={(1 - splitRatio) * 100}`；每 pane 内归属该区注册视图各渲染一槽 `<div style={{ display: open[zone] === v.id ? "flex" : "none", height: "100%" }}>`，槽内 `<v.component switchToPage={...} onDeletePage={...} />`（display:none 保挂载）；`onChange` 仅在两 pane 均 visible 时换算 `ratio = sizes[0] / (sizes[0] + sizes[1])` → `setSplitRatio`（有除零守卫）；`zones.top.map(id → registry.get(id))` 过滤 undefined（持久化 id 未注册防御）。
- **SB-17**：`src/features/sideViews/sideViewDefs.ts` 存在，Read 确认 side-effect 注册两条：`projects` → `{ title: "项目列表", icon: "📋", component: SidebarTree }`；`explorer` → `{ title: "文件浏览器", icon: "📁", component: () => <ExplorerPanel /> }`（grep `项目列表`/`文件浏览器`/`📋`/`📁` 命中）。
- **SB-18**：`src/features/sideViews/index.ts` 存在，Read 确认 barrel 导出 ActivityBar、SideBarArea、sideViewRegistry、类型、纯函数；**不 re-export sideViewDefs**（grep `sideViewDefs` 在 `index.ts` 为零）。
- **SB-19**：`src/__tests__/activityBar.test.tsx` 存在且全绿；覆盖：两组按钮渲染（上/下分组+顺序）、active class+指示条、点击→toggleView、tooltip/title、dragStart dataTransfer 内容、dragOver 指示线 state、drop→moveButton 参数（zone/index）、dragEnd 清指示、拖出活动栏无 moveButton 调用。
- **SB-20**：`src/__tests__/sideBarArea.test.tsx` 存在且全绿；覆盖：单开上/下全高（visible pane 数=1）、双开两 pane visible、视图槽 display 切换、换区后视图移槽、props 透传到视图组件、onChange→setSplitRatio。
- **DEPS-0（零新依赖）**：`git diff HEAD -- package.json package-lock.json` 输出为空；grep `react-dnd|dnd-kit` 在 `package.json` 为零。
- **BODY-0（组件本体零改动）**：`git diff HEAD -- src/features/sidebar/SidebarTree.tsx src/features/explorer/ExplorerPanel.tsx` 输出为空。
- **REG-2（存量零回归）**：`npm test` 全量绿，含全部存量套件。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
