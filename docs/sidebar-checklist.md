# 侧栏改造清单（依据 sidebar-requirements.md）

> 2026-07-18 · 输入：`docs/sidebar-requirements.md`（需求 v1.0 已定稿）+ `.claude/adr.md` ADR-0001
> 已敲定决策：换区重建接受（ExplorerPanel 展开状态丢失，文档注明）；L4 新增 2 条用例；Allotment `Pane visible` 已证实为 CSS 级显隐保挂载（`node_modules/allotment/dist/module.js:528,1144`），布局用 Allotment 全套

## 新文件总览

| 文件 | 职责 |
|------|------|
| `src/features/sideViews/sideBarState.ts` | 状态机纯函数：`toggleViewPure`/`moveButtonPure`/`deriveLayout`/`reconcileZones`/`sanitizeSideBar` + 默认值常量 |
| `src/features/sideViews/dropTarget.ts` | 拖拽落点判定纯函数 `computeDropTarget` |
| `src/features/sideViews/sideViewRegistry.ts` | `SideViewRegistry` 模块级单例（register/getAll/get/`_reset`）+ `SideViewDef` 类型 |
| `src/features/sideViews/sideViewDefs.ts` | side-effect 注册文件：注册 `projects`（📋）/`explorer`（📁）两视图（类比 `tabRules.ts`） |
| `src/features/sideViews/ActivityBar.tsx` | 活动栏组件：两组按钮 + 点击开关 + HTML5 拖拽 |
| `src/features/sideViews/SideBarArea.tsx` | 侧栏区组件：`<Allotment vertical>` 两 pane + 视图槽 display:none |
| `src/features/sideViews/index.ts` | barrel export |
| `src/features/sideViews/CLAUDE.md` | 模块文档 |
| `src/stores/sideBar.ts` | Zustand store：zones/open/width/splitRatio + 持久化 |

## 改动文件总览

| 文件 | 改动 |
|------|------|
| `src/workspace/Workspace.tsx` | Allotment 三栏 → 活动栏(40px 固定) + 侧栏区(visible=anyOpen) + 主区；尺寸常量替换；`import sideViewDefs` |
| `src/App.tsx` | init() 增加 `useSideBar.loadFromDisk()`（keybindings 之后）；关窗冲刷增加 `cancelSideBarPendingSave()` |
| `src/stores/index.ts` | re-export sideBar store |
| `e2e-tests/helpers.ts` | 新增 3 个侧栏 helper |
| `e2e-tests/test.e2e.ts` | 新增 describe「侧栏视图」2 用例 |
| 8 个 CLAUDE.md / inventory | 见 SB-26 |

**不改**：`SidebarTree` / `ExplorerPanel` 组件本体、后端全部（settings.rs 浅合并现成）、Dockview 主区、`CONTEXT.md`（已更新）、`.claude/adr.md`（已写）。

---

## Stage 1 — 状态层（纯函数 + 注册表 + store）

### 纯函数（agent A）

| ID | 位置 | 要点 |
|----|------|------|
| SB-1 | `src/features/sideViews/sideBarState.ts`（新建） | 类型 `Zone="top"\|"bottom"`、`Zones={top:string[];bottom:string[]}`、`OpenState={top:string\|null;bottom:string\|null}`、`LayoutKind="hidden"\|"single-top"\|"single-bottom"\|"split"`；常量 `DEFAULT_ZONES={top:["projects","explorer"],bottom:[]}`、`DEFAULT_OPEN={top:"projects",bottom:null}`、`WIDTH_DEFAULT=250/WIDTH_MIN=160/WIDTH_MAX=500`、`SPLIT_DEFAULT=0.5/SPLIT_MIN=0.1/SPLIT_MAX=0.9`、`ACTIVITY_BAR_SIZE=40` |
| SB-2 | 同上 | `toggleViewPure(zones, open, id): OpenState`——查 id 归属 zone；`open[zone]===id` → 置 null（R2）；否则 → `open[zone]=id`（R1，直接覆盖即隐式关闭同区旧视图）。id 未在 zones 中 → 原样返回（防御） |
| SB-3 | 同上 | `moveButtonPure(zones, open, id, targetZone, index): {zones;open}`——源区移除 id、按 clamp 后 index 插入目标区（R8）；若 `open[源区]===id` → `open[源区]=null` 且 `open[目标区]=id`（R6 跟随+替换目标区）；未打开则 open 不变（R7）。同区移动只调顺序不动 open |
| SB-4 | 同上 | `deriveLayout(open): LayoutKind`——双空→hidden；仅上→single-top；仅下→single-bottom；双开→split（R3/R4/R5）。`reconcileZones(saved, registeredIds): {zones;open}`——过滤未注册 id、保留 saved 顺序、缺失的注册 id 追加上区末尾（R9）、open 指向未知 id→null。`sanitizeSideBar(raw): SideBarSlice`——非对象→默认；zones/open 结构校验；width/splitRatio clamp |
| SB-5 | `src/features/sideViews/dropTarget.ts`（新建） | `computeDropTarget(clientY, buttonRects: {id;top;height}[], zone): {zone;index}`——clientY 在按钮上半 → 该按钮 index，下半 → index+1；空白区 → 数组末尾。纯函数不碰 DOM |

### 注册表（agent B）

| ID | 位置 | 要点 |
|----|------|------|
| SB-6 | `src/features/sideViews/sideViewRegistry.ts`（新建） | `SideViewDef={id;title;icon;component: React.ComponentType<SideViewComponentProps>}`；`SideViewComponentProps={switchToPage:(projectId,pageId)=>void;onDeletePage:(projectId,pageId)=>void}`（与 SidebarTree props 精确匹配；ExplorerPanel 注册时箭头包装忽略 props）。模块级单例 `sideViewRegistry`（同 TabTitleRegistry 模式） |
| SB-7 | 同上 | `register(def)`（同 id 覆盖）/`getAll(): SideViewDef[]`（注册序）/`get(id)`/`_reset()`（仅测试） |

### store（agent C，依赖 A 的 SB-1~4 签名——契约写死 §执行计划）

| ID | 位置 | 要点 |
|----|------|------|
| SB-8 | `src/stores/sideBar.ts`（新建） | `SideBarState={zones;open;width;splitRatio;loaded;toggleView;moveButton;setWidth;setSplitRatio;loadFromDisk}`——toggleView/moveButton 委托 SB-2/3 纯函数；setWidth/setSplitRatio clamp；默认值为 SB-1 常量 |
| SB-9 | 同上 | 持久化照 `fontSize.ts` 模式：`loadFromDisk` 读 `loadSettings()` → `saved.sideBar` → `sanitizeSideBar` → `reconcileZones(zones, sideViewRegistry 已注册 ids)` → set + `loaded:true`（try/catch 保默认）；`subscribe` + loaded 守卫 + 2s debounce（`PERSIST_DEBOUNCE_MS` 复用 `stores/projects.ts`）→ `saveSettings({sideBar:{zones,open,width,splitRatio}})`；导出 `cancelPendingSave()` |
| SB-10 | `src/stores/index.ts` | re-export `useSideBar` + 类型 |

### Stage 1 测试（各 agent 随代码交付）

| ID | 文件 | 用例要点 |
|----|------|---------|
| SB-11 | `src/__tests__/sideBarState.test.ts`（新建，~30 用例） | toggleViewPure 4 分支；moveButtonPure 8 分支（跨区跟随替换/跟随空区/未打开仅归属/区内前移/后移/同位/index clamp）；deriveLayout 4 态；reconcileZones 5 分支；sanitizeSideBar 6 分支；**S1–S6 场景序列**（连续调用纯函数断言终态，直译需求 §5 验收用例）6 条 |
| SB-12 | `src/__tests__/sideViewRegistry.test.ts`（新建，~6 用例） | register/getAll/get、重复注册覆盖、未注册 get→undefined、`_reset` 隔离（照 TabTitleRegistry.test.ts 模式） |
| SB-13 | `src/__tests__/sideBar.test.ts`（新建，~12 用例） | 默认值；toggle/move 经 store；loadFromDisk 合法/脏数据 sanitize/缺失/异常降级；loaded 守卫防启动空写；fake timers 验证 debounce → `saveSettings({sideBar})` payload 键集合精确匹配（照 fontSize/keybindings.test.ts 模式，mock `../ipc/settings`） |

---

## Stage 2 — UI 组件（ActivityBar + SideBarArea）

### ActivityBar（agent A）

| ID | 位置 | 要点 |
|----|------|------|
| SB-14 | `src/features/sideViews/ActivityBar.tsx`（新建） | 结构：40px 宽 flex 列（上区按钮组 + `flex:1` 间隔 + 下区按钮组）；按钮 40×40、emoji 居中、`title={def.title}`、`data-e2e={"activity-btn-"+id}`；active 判定 `open[zone]===id` → 高亮背景 + 左侧 2px 指示条（VS Code 风格）；点击 → `useSideBar.getState().toggleView(id)`。配色全部 `theme/colors.ts` token（bg=`PANEL_BG`、hover=`SIDEBAR_COLORS.hover`、active=`SIDEBAR_COLORS.selected`、指示条=`FOCUS_BORDER`、边框=`SIDEBAR_COLORS.border`）——硬约束 #6 |
| SB-15 | 同上 | HTML5 拖拽（零依赖，项目无先例无 react-dnd）：按钮 `draggable` + `onDragStart`（`dataTransfer.setData("application/x-side-view-id", id)`、`effectAllowed="move"`、dragging 半透明态）；按钮/半区容器 `onDragOver`（`preventDefault` + `computeDropTarget` → `dropIndicator` state {zone,index} 渲染插入指示线）；`onDrop` → `moveButton(id, zone, index)`；`onDragEnd`/`onDragLeave` 清指示线。拖拽仅活动栏内有效（外部不监听 drop） |

### SideBarArea（agent B）

| ID | 位置 | 要点 |
|----|------|------|
| SB-16 | `src/features/sideViews/SideBarArea.tsx`（新建） | props `{switchToPage;onDeletePage}`。结构：`<Allotment vertical proportionalLayout onChange={sizes→ratio}>` 两 pane：上 pane `visible={!!open.top}` `preferredSize={splitRatio*100}`，下 pane `visible={!!open.bottom}` `preferredSize={(1-splitRatio)*100}`。每 pane 内：归属该区注册视图各渲染一槽 `<div style={{display: open[zone]===v.id?"flex":"none", height:"100%"}}>`，槽内 `<v.component switchToPage onDeletePage/>`（display:none 保挂载，Q9）。onChange 换算 `ratio=sizes[0]/(sizes[0]+sizes[1])` → setSplitRatio（仅双开时） |
| SB-17 | `src/features/sideViews/sideViewDefs.ts`（新建） | side-effect 注册：`projects`→{title:"项目列表",icon:"📋",component:SidebarTree}；`explorer`→{title:"文件浏览器",icon:"📁",component:()=><ExplorerPanel/>} |
| SB-18 | `src/features/sideViews/index.ts`（新建） | barrel：ActivityBar、SideBarArea、sideViewRegistry、类型、纯函数（不 re-export sideViewDefs——由 Workspace 显式 side-effect import，同 tabRules 模式） |

### Stage 2 测试

| ID | 文件 | 用例要点 |
|----|------|---------|
| SB-19 | `src/__tests__/activityBar.test.tsx`（新建，~12 用例） | 两组按钮渲染（上/下分组+顺序）、active class+指示条、点击→toggleView（spy store）、tooltip/title、dragStart dataTransfer 内容、dragOver 指示线 state、drop→moveButton 参数（zone/index）、dragEnd 清指示、拖出活动栏无 moveButton 调用。种子 sideBar store（真实）+ 手动注册测试视图（`_reset` 后 register stub 组件）；mock dataTransfer（`{setData:vi.fn(),effectAllowed:""}`） |
| SB-20 | `src/__tests__/sideBarArea.test.tsx`（新建，~10 用例） | 单开上/下全高（visible pane 数=1）、双开两 pane visible、视图槽 display 切换、换区后视图移槽（重建可断言旧组件卸载新组件挂载）、props 透传到视图组件、onChange→setSplitRatio。mock `allotment`（捕获 Pane props，参照 workspace-e2e-ready 的 MockAllotment 模式但需渲染 children 断言可见性） |

---

## Stage 3 — Workspace 集成 + App 接线

| ID | 位置 | 要点 |
|----|------|------|
| SB-21 | `src/workspace/Workspace.tsx` | 三栏改造：pane1 `preferredSize=40 minSize=40 maxSize=40` → `<ActivityBar/>`；pane2 `preferredSize={width} minSize=160 maxSize=500 visible={anyOpen}` → `<SideBarArea switchToPage={switchToPage} onDeletePage={onDeletePage}/>`；pane3 主区**不变**。外层 Allotment `onChange={sizes→anyOpen&&setWidth(sizes[1])}`（anyOpen 从 `deriveLayout(open)!=="hidden"` 推导）。删除 `SIDEBAR_*`/`EXPLORER_*` 六常量，引入 `ACTIVITY_BAR_SIZE`/`WIDTH_*`（SB-1）。顶部 `import "../features/sideViews/sideViewDefs"`（side-effect 注册）。`switchToPage`/`onDeletePage`/SEC-01/回调 map 逻辑全部不动 |
| SB-22 | `src/App.tsx` | init() 在 keybindings loadFromDisk 之后加 `await useSideBar.getState().loadFromDisk()`（独立 try/catch）；关窗冲刷在 `cancelKeybindingsSave()` 后加 `cancelSideBarPendingSave()`（sideBar store 导出的 cancelPendingSave 改名引入） |
| SB-23 | 现有测试适配 | `workspace-multi-instance.test.tsx` / `workspace-switch-order.test.tsx`：真实渲染 Workspace → 种子 sideBar store 默认态（或 mock `../features/sideViews` 模块）；`workspace-e2e-ready.test.tsx` / `e2e-gating-workspace.test.tsx`：MockAllotment 结构不变（Pane=null），断言不受新 pane 影响；其余套件若因 Workspace 结构变红一并适配（全量 `npm test` 为准） |
| SB-24 | `src/__tests__/workspace-sideviews.test.tsx`（新建，~8 用例） | Workspace 集成：活动栏 pane 40 固定（preferredSize/min/max）、侧栏区 pane `visible=anyOpen` 四态（hidden→false、单开/双开→true）、onChange→setWidth、主区 pane 保持、`switchToPage` 透传 SideBarArea。mock allotment 捕获 Pane props + 真实 sideBar store 种子 |

---

## Stage 4 — E2E（L4）

| ID | 位置 | 要点 |
|----|------|------|
| SB-25 | `e2e-tests/helpers.ts` + `e2e-tests/test.e2e.ts` | helpers 新增：`__slterm_e2e_getSideBarState()`（返回 store 快照）、`__slterm_e2e_toggleSideView(id)`（等价点击，走 store.toggleView）、`__slterm_e2e_moveSideViewButton(id,zone,index)`（等价拖拽落点，走 store.moveButton）。test.e2e.ts 新增 describe「侧栏视图」2 用例：**E2E-1 点击开关**——createProject 后断言默认项目列表打开且侧栏区可见 → 真实 click `activity-btn-projects` → 侧栏区隐藏（state open 双空）→ 再 click 恢复 → click `activity-btn-explorer` → open.top==="explorer"（R1 替换）；**E2E-2 拖拽跨区**——JS 合成 DragEvent（`new DragEvent("dragstart"/"dragover"/"drop",{dataTransfer:new DataTransfer()})`，WebView2/Chromium 支持构造器）驱动按钮跨区 → 断言 state zones 变化 + open 跟随（R6/R7）；合成事件不可行则降级 helper.moveSideViewButton 驱动并在报告注明人工验收 |

---

## Stage 5 — 文档同步（最后）

| ID | 位置 | 要点 |
|----|------|------|
| SB-26 | 8 处文档 | ① 新建 `src/features/sideViews/CLAUDE.md`（职责/状态机单槽位模型/注册表扩展指南/拖拽/文件表/测试模式）；② `.claude/CLAUDE.md` 模块表加 `src/features/sideViews` 行；③ `src/workspace/CLAUDE.md` 布局段重写（活动栏+侧栏区+主区三 pane、SideBarArea 视图槽 display:none、宽度/比例持久化）；④ `src/stores/CLAUDE.md` Store 清单加 `sideBar.ts`（状态形状/settings.json `sideBar` 段/默认态）；⑤ `src/features/sidebar/CLAUDE.md` 补一句宿主变化（常驻栏→侧栏视图槽，组件本体不变）；⑥ `src/features/explorer/CLAUDE.md` 同⑤+换区重建丢展开状态的已知行为；⑦ `e2e-tests/CLAUDE.md` helpers 表加 3 个 helper + describe 清单加「侧栏视图」2 用例；⑧ `.claude/test-inventory.md` L2 新增「侧栏视图」类目（6 文件 ~78 用例）+ 总数更新 + L4 12→14 |

---

## Stage 划分映射

| Stage | 包含 ID | commit 前缀 |
|-------|---------|------------|
| Stage 1 — 状态层（纯函数+注册表+store） | SB-1~13 | `feat:` |
| Stage 2 — UI 组件（ActivityBar+SideBarArea） | SB-14~20 | `feat:` |
| Stage 3 — Workspace 集成 + App 接线 | SB-21~24 | `feat:` |
| Stage 4 — E2E 用例 | SB-25 | `test:` |
| Stage 5 — 文档同步 | SB-26 | `docs:` |
