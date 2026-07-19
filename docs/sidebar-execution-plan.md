# 侧栏改造执行计划（活动栏 + 可关闭侧栏视图）

> 2026-07-18 · 输入：`docs/sidebar-requirements.md` + `docs/sidebar-checklist.md` + `.claude/adr.md` ADR-0001
> 本计划只供执行参考——**当前未执行**。执行时逐 Stage 串行，每 Stage 一个 Workflow + 一个 commit。

## 0. 背景与决策

| 项 | 结论 |
|----|------|
| 需求 | Allotment 常驻三栏 → 活动栏 + 共享侧栏区（上下半区单槽位状态机），详见需求文档 §4/§5 |
| 布局方案 | **Allotment 全套**（非自实现 flex）：`Pane visible` 已证实 CSS 级显隐保挂载（`allotment/dist/module.js:528` visible=false→size=0+class toggle；`:1144` props.visible→setViewVisible，不动 React 树） |
| 拖拽方案 | HTML5 DnD 零依赖（项目无 react-dnd、无 DnD 先例），落点判定抽纯函数 `computeDropTarget` |
| 状态模型 | 纯函数层（`sideBarState.ts`）+ Zustand 壳（`stores/sideBar.ts`），展示 = `deriveLayout(open)` 纯推导 |
| 换区重建 | 接受（用户已确认）：视图组件跨 pane 移动即重建，ExplorerPanel 展开状态丢失——写入 explorer CLAUDE.md 已知行为 |
| 持久化 | `settings.json` 新增 `sideBar` 段，复用 settings 浅合并，**后端零改动** |
| E2E 范围 | 新增 2 用例（点击开关 + 拖拽跨区）；现有 12 用例不碰侧栏 DOM（经 `__slterm_e2e_*` helper 驱动），预期零适配 |

## 1. Stage 总览与进度跟踪

| Stage | 内容 | ID | 脚本 | 状态 |
|-------|------|-----|------|------|
| 1 | 状态层：sideBarState/dropTarget 纯函数 + SideViewRegistry + sideBar store + L2 测试 | SB-1~13 | `docs/workflows/stage-01-state-layer.js` | 未开始 |
| 2 | UI 组件：ActivityBar + SideBarArea + sideViewDefs + L2 测试 | SB-14~20 | `docs/workflows/stage-02-ui-components.js` | 未开始 |
| 3 | Workspace 集成 + App 接线 + 现有测试适配 | SB-21~24 | `docs/workflows/stage-03-workspace-integration.js` | 未开始 |
| 4 | E2E helpers + 2 条 L4 用例 | SB-25 | `docs/workflows/stage-04-e2e.js` | 未开始 |
| 5 | 文档同步（8 处） | SB-26 | `docs/workflows/stage-05-docs-sync.js` | 未开始 |

恢复规则：中断后从首个「未开始」Stage 继续，其前 commit 已落盘无需重做。

## 2. 跨 agent 契约（写死，各方不各自推断）

**`src/features/sideViews/sideBarState.ts` 导出签名**（Stage 1 agent A 实现，agent C 依赖）：

```typescript
export type Zone = "top" | "bottom";
export interface Zones { top: string[]; bottom: string[]; }
export interface OpenState { top: string | null; bottom: string | null; }
export type LayoutKind = "hidden" | "single-top" | "single-bottom" | "split";
export interface SideBarSlice { zones: Zones; open: OpenState; width: number; splitRatio: number; }

export const DEFAULT_ZONES: Zones;        // { top: ["projects", "explorer"], bottom: [] }
export const DEFAULT_OPEN: OpenState;     // { top: "projects", bottom: null }
export const ACTIVITY_BAR_SIZE = 40;
export const WIDTH_DEFAULT = 250, WIDTH_MIN = 160, WIDTH_MAX = 500;
export const SPLIT_DEFAULT = 0.5, SPLIT_MIN = 0.1, SPLIT_MAX = 0.9;

export function toggleViewPure(zones: Zones, open: OpenState, id: string): OpenState;
export function moveButtonPure(zones: Zones, open: OpenState, id: string, targetZone: Zone, index: number): { zones: Zones; open: OpenState };
export function deriveLayout(open: OpenState): LayoutKind;
export function reconcileZones(saved: Zones, open: OpenState, registeredIds: string[]): { zones: Zones; open: OpenState };
export function sanitizeSideBar(raw: unknown): SideBarSlice;  // 非法输入回退默认值 + clamp
```

**`src/features/sideViews/sideViewRegistry.ts` 导出签名**（Stage 1 agent B 实现，Stage 2 依赖）：

```typescript
export interface SideViewComponentProps {
  switchToPage: (projectId: string, pageId: string) => void;
  onDeletePage: (projectId: string, pageId: string) => void;
}
export interface SideViewDef {
  id: string; title: string; icon: string;
  component: React.ComponentType<SideViewComponentProps>;
}
export const sideViewRegistry: {
  register(def: SideViewDef): void;
  getAll(): SideViewDef[];
  get(id: string): SideViewDef | undefined;
  _reset(): void;  // 仅测试
};
```

**`src/stores/sideBar.ts` 导出签名**（Stage 1 agent C 实现，Stage 2/3 依赖）：

```typescript
export interface SideBarState extends SideBarSlice {
  loaded: boolean;
  toggleView(id: string): void;
  moveButton(id: string, zone: Zone, index: number): void;
  setWidth(w: number): void;
  setSplitRatio(r: number): void;
  loadFromDisk(): Promise<void>;
}
export const useSideBar: UseBoundStore<StoreApi<SideBarState>>;
export function cancelPendingSave(): void;
```

**持久化 payload 契约**：`saveSettings({ sideBar: { zones, open, width, splitRatio } })`——`save_settings` 浅合并 top-level 键，只写 `sideBar` slice，不影响 `fontSize`/`keybindings` 段。

## 3. Stage 1 — 状态层（SB-1~13）

**commit message**：`feat: 侧栏视图状态层——单槽位状态机纯函数 + SideViewRegistry + sideBar store（含 L2 测试）`

### agent 文件分工（零重叠）

| label | 负责 ID | 文件 |
|-------|---------|------|
| `state-pure` | SB-1~5, SB-11 | `src/features/sideViews/sideBarState.ts`、`src/features/sideViews/dropTarget.ts`、`src/__tests__/sideBarState.test.ts`（含 dropTarget 用例） |
| `view-registry` | SB-6, SB-7, SB-12 | `src/features/sideViews/sideViewRegistry.ts`、`src/__tests__/sideViewRegistry.test.ts` |
| `sidebar-store` | SB-8~10, SB-13 | `src/stores/sideBar.ts`、`src/stores/index.ts`、`src/__tests__/sideBar.test.ts` |

> `sidebar-store` 依赖 `state-pure` 的纯函数——按 §2 契约签名 import，各自开发期编译红可接受，全量测试 agent 统一验收。

### 实现要点

- **toggleViewPure**：`open[zone] === id` → null；否则覆盖 `open[zone] = id`（覆盖即隐式关闭，天然单槽位，无历史）；id 不在 zones 任一区 → 原样返回。
- **moveButtonPure**：源区 splice 移除 → 目标区 splice 插入（index clamp 到 [0, len]）；`open[源区]===id` → `open[源区]=null; open[目标区]=id`（同区移动跳过此步）。不校验 id 是否已注册（注册表职责分离）。
- **reconcileZones**：saved 两数组过滤 `registeredIds` → 缺失的注册 id 按 `getAll()` 顺序追加上区末尾 → open 逐项校验（值非 null 且不在 reconcile 后 zones 中 → null）。
- **sanitizeSideBar**：raw 非对象 → 全默认；`zones.top/bottom` 非 string[] → 默认；`open.top/bottom` 非 string|null → null；width/splitRatio 非 number → 默认，否则 clamp。返回完整 SideBarSlice（无 partial）。
- **store 持久化**：照 `fontSize.ts:49-87` 模式——try/catch 保默认、`loaded` 守卫、`PERSIST_DEBOUNCE_MS`（import 自 `stores/projects.ts`）debounce、`saveSettings(...).catch(()=>{})` 静默吞错（与 fontSize/keybindings 一致，侧栏状态非关键数据）。
- **loadFromDisk 顺序**：`loadSettings()` → `saved?.sideBar` → `sanitizeSideBar` → `reconcileZones(slice.zones, slice.open, sideViewRegistry.getAll().map(d=>d.id))` → set。**时序成立**：`sideViewDefs` 经静态 import 链（`main.tsx`→`App.tsx`→`Workspace.tsx`→`sideViewDefs.ts`）在入口模块求值时已完成注册，`init()` 运行时 `getAll()` 非空，直接 reconcile 无需延迟方案。**测试注意**：`sideBar.test.ts` 只 import store 不 import sideViewDefs → `getAll()` 为空会过滤全部 saved id，测试需先手动 `register` stub 视图（或 vi.mock registry）。
- 测试照既有模式：`sideBarState.test.ts` 纯函数无 mock；`sideViewRegistry.test.ts` 照 `TabTitleRegistry.test.ts`；`sideBar.test.ts` 照 `fontSize.test.ts`（mock `../ipc/settings`，fake timers，`_resetPersistence` 类辅助按 keybindings.test.ts 做法）。
- 并行 agent 不跑测试，统一由全量测试 agent 跑。

### 验证项

断言真值源：`docs/workflows/verify/stage-01.md`。测试门禁：`tsc` / `eslint` / `npm test`。关键断言：

- `sideBarState.ts` 导出签名与 §2 契约逐字一致（Read 比对）
- `toggleViewPure` 无历史记忆：任何路径下返回的 OpenState 不含被替换视图（用例断言）
- `sideBar.ts` 不存在 `import { invoke }`（硬约束 #1，grep）；debounce payload 仅 `sideBar` 一键
- 三个新测试文件全绿；存量测试零回归

## 4. Stage 2 — UI 组件（SB-14~20）

**commit message**：`feat: 活动栏与侧栏区组件——ActivityBar（点击开关+HTML5 拖拽）+ SideBarArea（Allotment 双槽 display:none）`

### agent 文件分工（零重叠）

| label | 负责 ID | 文件 |
|-------|---------|------|
| `activity-bar` | SB-14, SB-15, SB-19 | `src/features/sideViews/ActivityBar.tsx`、`src/__tests__/activityBar.test.tsx` |
| `sidebar-area` | SB-16~18, SB-20 | `src/features/sideViews/SideBarArea.tsx`、`src/features/sideViews/sideViewDefs.ts`、`src/features/sideViews/index.ts`、`src/__tests__/sideBarArea.test.tsx` |

### 实现要点

- **ActivityBar active 判定**：按钮 id 归属 zone = 从 store `zones` 查；active = `open[zone] === id`（此时该视图必在展示——单槽位模型）。
- **拖拽 dataTransfer type**：`application/x-side-view-id`。`dragOver` 必须 `preventDefault` 才触发 drop。指示线：`dropIndicator` local state `{zone,index}|null`，渲染为对应位置的 2px 横线（token 色 `FOCUS_BORDER`）。
- **SideBarArea pane 结构**：`<Allotment vertical proportionalLayout={true} onChange={...}>`；pane `preferredSize` 用比例基数（`splitRatio*100` / `(1-splitRatio)*100`）；onChange 仅在两 pane 均 visible 时换算写回（单开时 sizes 另一项为 0，除零守卫）。
- **视图槽渲染**：`zones.top.map(id → registry.get(id))` 过滤 undefined（持久化 id 未注册防御）→ 每视图一槽 `display: open.top===id ? "flex" : "none"`。**换区重建为已知行为**（ADR-0001）：组件随 zones 跨 pane 移动即卸载重建。
- **sideViewDefs**：`projects` component 直接引用 `SidebarTree`（props 精确匹配 `SideViewComponentProps`）；`explorer` 用 `() => <ExplorerPanel/>` 包装。组件内不 import sideViewDefs（防循环），由 Workspace 引入。
- **配色**：仅 `theme/colors.ts` token——bg `PANEL_BG`、hover `SIDEBAR_COLORS.hover`、active `SIDEBAR_COLORS.selected`、指示条/插入线 `FOCUS_BORDER`、边框 `SIDEBAR_COLORS.border`、文字 `SIDEBAR_FG`。
- **测试 mock 要点**：activityBar 测试用真实 sideBar store（beforeEach setState 重置）+ `sideViewRegistry._reset()` 后注册 stub 视图（`() => null` 组件）；sideBarArea 测试 mock `allotment` 为「渲染 children 并记录 Pane props」的 stub（不能全 null，需断言 visible/preferredSize）；dataTransfer 手动构造 `{ setData: vi.fn(), getData: vi.fn(), effectAllowed: "", dropEffect: "" }`。

### 验证项

断言真值源：`docs/workflows/verify/stage-02.md`。测试门禁：`tsc` / `eslint` / `npm test`。关键断言：

- ActivityBar/SideBarArea 无硬编码色值（grep `#[0-9a-fA-F]{3,6}` 两文件为零）
- ActivityBar 按钮带 `data-e2e="activity-btn-<id>"` 与 `title`
- SideBarArea 视图槽 `display` 仅 `flex`/`none` 两值；组件本体（SidebarTree/ExplorerPanel）零改动（`git diff` 两文件为空）
- 无 `react-dnd` 等新依赖（`package.json` diff 为空）

## 5. Stage 3 — Workspace 集成 + App 接线（SB-21~24）

**commit message**：`feat: Workspace 三栏改造——活动栏 + 可显隐侧栏区接入 Workspace 与 App 启动/关闭接线`

### agent 文件分工

单 agent（`workspace-integration`）：Workspace.tsx + App.tsx + 测试适配关联紧密，一个上下文完成。

| 负责 ID | 文件 |
|---------|------|
| SB-21 | `src/workspace/Workspace.tsx` |
| SB-22 | `src/App.tsx` |
| SB-23 | `src/__tests__/workspace-multi-instance.test.tsx`、`workspace-switch-order.test.tsx`、`workspace-e2e-ready.test.tsx`、`e2e-gating-workspace.test.tsx`（适配） |
| SB-24 | `src/__tests__/workspace-sideviews.test.tsx`（新建） |

### 实现要点

- **Workspace.tsx**：旧六常量（`SIDEBAR_*`/`EXPLORER_*`）删除；`anyOpen = deriveLayout(open) !== "hidden"`；pane2 `visible={anyOpen}`；外层 `onChange={(sizes) => anyOpen && setWidth(sizes[1])}`（`sizes[1]` clamp 由 store 保证）。`import "../features/sideViews/sideViewDefs"` 顶部 side-effect（静态 import 链保证 App init 的 loadFromDisk 运行时注册已完成，见 Stage 1 要点）。
- **App.tsx**：init() 在 `useKeybindings.loadFromDisk()` 之后插入 `await useSideBar.getState().loadFromDisk()` 的独立 try/catch 块；关窗冲刷在 `cancelKeybindingsSave()` 后加 `cancelSideBarSave()`（`import { cancelPendingSave as cancelSideBarSave } from "./stores/sideBar"`）。
- **适配判断**：`workspace-e2e-ready` / `e2e-gating-workspace` 的 MockAllotment（Pane=null）对 pane 数量不敏感，预期零改动；`workspace-multi-instance` / `workspace-switch-order` 真实渲染 Workspace → 需种子 `useSideBar.setState({ ...默认值, loaded: true })`（beforeEach）或 mock `../features/sideViews` 模块导出 stub 组件。**以全量 `npm test` 红绿为准逐文件适配，不预判**。
- **e2e-tests/helpers.ts 本 Stage 不动**（SB-25 在 Stage 4）。
- 人工验证点前必须 `npm run tauri dev` 实测（见下）。

### 验证项

断言真值源：`docs/workflows/verify/stage-03.md`。测试门禁：`tsc` / `eslint` / `npm test` / `npx vite build`。关键断言：

- Workspace.tsx 无 `SIDEBAR_PREFERRED_SIZE`/`EXPLORER_PREFERRED_SIZE` 等旧常量（grep 为零）；三 pane 顺序：ActivityBar → SideBarArea → 主区
- `SidebarTree`/`ExplorerPanel` 组件本体零改动（git diff 为空）
- App.tsx 启动序列：sideBar loadFromDisk 在 keybindings 之后、projects 之前（Read 确认）；关窗冲刷含 cancelSideBarSave
- 全量 L2 绿（含适配后存量套件）

### 人工验证点（commit 后、进 Stage 4 前）

`npm run tauri dev` 实测：
1. 默认项目列表打开；点击 📋 关闭 → 侧栏区消失主区占满；再点恢复且**项目树展开状态保留**（display:none 保挂载）
2. 点击 📁 → 替换为文件浏览器；拖拽 📁 到下区 → 上下分区；拖分隔条调比例
3. 拖拽 📋 到下区（两视图打开中）→ 项目列表跟随到下区，文件浏览器被关闭
4. 重启 app → 打开状态/宽度/比例/按钮归属全部恢复
5. 终端 claude 启动正常（主区零改动冒烟）

## 6. Stage 4 — E2E（SB-25）

**commit message**：`test: 侧栏视图 E2E——helpers 增补 + 点击开关/拖拽跨区 2 用例`

### agent 文件分工

单 agent（`e2e-cases`）：`e2e-tests/helpers.ts`、`e2e-tests/test.e2e.ts`。

### 实现要点

- helpers 三函数注入点在 `installAllE2eHelpers()`（`E2E_ENABLED` 门控内，与现有 helper 同处）；`__slterm_e2e_getSideBarState` 返回 `useSideBar.getState()` 的 plain 快照（去函数）。
- E2E-1 点击用 `browser.execute` 内 `document.querySelector('[data-e2e="activity-btn-projects"]').click()`（真实 DOM click 走 React onClick）。
- E2E-2 合成 DnD：`const dt = new DataTransfer(); el.dispatchEvent(new DragEvent("dragstart",{dataTransfer:dt,bubbles:true})); target.dispatchEvent(new DragEvent("dragover",{dataTransfer:dt,bubbles:true})); target.dispatchEvent(new DragEvent("drop",{dataTransfer:dt,bubbles:true}));`——React 18 合成事件经 root 监听可正常分发；**若 DataTransfer 构造器在 WebView2 不可用**（老版本无），降级 `__slterm_e2e_moveSideViewButton` 驱动断言状态机，并在收尾报告注明拖拽手感人工验收。
- 门禁：`npm run build:e2e` + `npm run wdio`（L4 全量，含现有 12 用例回归）。注意 helpers.ts 不在根 tsconfig include——本 Stage 改 helpers.ts 必须 `npx vite build` + `build:e2e` 双构建验证。

### 验证项

断言真值源：`docs/workflows/verify/stage-04.md`。关键断言：

- `npm run e2e` 全绿（14 用例：12 存量 + 2 新增）
- helpers 注入在 `E2E_ENABLED` 门控内（Read 确认）；生产构建 grep 无 `__slterm_e2e_getSideBarState`

### 人工验证点

真实 app 拖拽手感（合成事件验证状态机 ≠ 真实鼠标拖拽）：按钮拖拽视觉反馈、插入指示线位置、跨区落点正确。

## 7. Stage 5 — 文档同步（SB-26）

**commit message**：`docs: 侧栏改造文档同步——sideViews 模块文档 + 各 CLAUDE.md + test-inventory`

单 agent（`docs-sync`）：先读 `git log -4 --stat` 与四个 Stage 实际 diff 再动笔，禁凭记忆写文档。八处文件按 checklist SB-26 表逐项同步；`test-inventory.md` 用例数以 `npm test` 实际输出为准（L2 新增类目「侧栏视图」）+ `npm run wdio` 结果（L4 12→14）。门禁：`tsc` / `eslint` / `npm test`。

## 8. 执行规则（SKILL Step 5 要点）

1. **脚本调用**：`Workflow({ scriptPath: 'docs/workflows/stage-NN-*.js' })`；首次调用前语法预检（`new Function` 仅编译，SKILL 5.1 命令）。
2. **verify 失败** → `fix-loop.js`（args: stage/failedItems/fixContext/verifyFile/constraints），最多 3 轮，超过报告用户。
3. **no-return 分流**：verify 阶段 → 主 agent inline spawn 单 verify agent；重构/测试阶段 → `TaskStop` + `resumeFromRunId`。
4. **git add 路径限定**：`git add src/ e2e-tests/ .claude/CLAUDE.md .claude/test-inventory.md docs/`，commit 前 `git status` 甄别预期外改动；**禁止 `git add -A`**。
5. **时间盒**：后台运行每 15-20 分钟查 /workflows；单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户。
6. **并行纪律**：同 Stage 两 agent 文件零重叠（分工表为证）；并行 agent 不跑测试，统一由全量测试 agent 单点跑。
7. **禁区**：`compute_conpty_flags` 固定 0x7，任何 agent 不得修改 ConPTY flags（含 4 条守卫测试）；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust 一律不动。
8. **逐 ID 对照**：写脚本时逐 ID 对照 `sidebar-checklist.md` 原文，禁止凭记忆。

## 9. 执行前准备

- [x] 需求定稿（`docs/sidebar-requirements.md` v1.0，12 轮 grilling）
- [x] 术语落盘（`CONTEXT.md`「侧栏」节）+ ADR-0001（`.claude/adr.md`）
- [x] checklist 与执行计划落盘（`docs/sidebar-checklist.md` + 本文件）
- [x] 5 个 workflow 脚本 + 5 个 verify 断言文件落盘（`docs/workflows/`）
- [x] 5 个 JS 脚本语法预检通过（2026-07-19）
- [x] 用户确认启动 Stage 1（2026-07-19）
