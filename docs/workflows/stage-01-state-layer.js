// =====================================================================
// Stage 1 — 状态层：纯函数 + 注册表 + store（SB-1~SB-13）
// =====================================================================
// 生成依据：docs/sidebar-checklist.md「Stage 1 — 状态层」逐 ID 原文
//           + docs/sidebar-execution-plan.md §2（跨 agent 契约）/ §3（实现要点）
// 调用：Workflow({ scriptPath: 'docs/workflows/stage-01-state-layer.js' })
// commit message（verify 全绿后）：
//   feat: 侧栏视图状态层——单槽位状态机纯函数 + SideViewRegistry + sideBar store（含 L2 测试）
//
// fix-loop 调用参数（verify 不通过时）：
//   Workflow({ scriptPath: 'docs/workflows/fix-loop.js', args: {
//     stage: 1, failedItems, fixContext,
//     verifyFile: 'docs/workflows/verify/stage-01.md',
//     constraints: '本 Stage 除 src/stores/index.ts 追加 re-export 外只允许新建文件，禁止改任何其它存量代码；并行 agent 不跑测试'
//   }})
//
// === 跨 agent 契约（写死，各方不各自推断）===
// 以下签名同时嵌入 PREAMBLE 供全部 agent 消费；与 docs/sidebar-execution-plan.md §2 逐字一致。
// =====================================================================

export const meta = {
  name: 'stage1-state-layer',
  description: 'Stage 1 状态层：sideBarState/dropTarget 纯函数 + SideViewRegistry + sideBar store + L2 测试（SB-1~SB-13）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust（src-tauri/）一律不动。
背景：修复要点详见 docs/sidebar-checklist.md 对应 ID 条目（先读再动手）；规则编号 R1~R9 与场景 S1~S6 见 docs/sidebar-requirements.md §4/§5。
本 Stage 特殊纪律：
1. 除 src/stores/index.ts 追加 re-export 外全部为新建文件，禁止改任何其它存量代码。
2. 禁止执行测试命令（npm test / npx vitest / cargo test）——并行 agent 不跑测试，统一由全量测试 agent 单点跑；编译级自检可用 npx tsc --noEmit。
3. 跨 agent 契约签名写死如下，严格按签名 import，不各自推断；并行开发期编译红可接受（全量测试 agent 统一验收）。

跨 agent 契约（写死，与 docs/sidebar-execution-plan.md §2 逐字一致）：
[A] src/features/sideViews/sideBarState.ts 导出签名（state-pure 实现，sidebar-store 依赖）：
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
[B] src/features/sideViews/sideViewRegistry.ts 导出签名（view-registry 实现，sidebar-store 依赖）：
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
[C] 持久化 payload 契约：saveSettings({ sideBar: { zones, open, width, splitRatio } })——save_settings 浅合并 top-level 键，只写 sideBar slice，不影响 fontSize/keybindings 段。`

// === Phase 1: 并行重构（3 agent 文件零重叠；禁止跑测试，见 SKILL 5.3）===
phase('并行重构')
const parallelAgents = [
  { label: 'state-pure', prompt: `你负责 SB-1~SB-5、SB-11。先读 docs/sidebar-checklist.md「Stage 1 — 纯函数（agent A）」与 docs/sidebar-execution-plan.md §3「实现要点」再动手。

文件（全部新建）：
- src/features/sideViews/sideBarState.ts
- src/features/sideViews/dropTarget.ts
- src/__tests__/sideBarState.test.ts

【SB-1】sideBarState.ts 类型与常量：类型 Zone="top"|"bottom"、Zones={top:string[];bottom:string[]}、OpenState={top:string|null;bottom:string|null}、LayoutKind="hidden"|"single-top"|"single-bottom"|"split"、SideBarSlice={zones;open;width;splitRatio}；常量 DEFAULT_ZONES={top:["projects","explorer"],bottom:[]}、DEFAULT_OPEN={top:"projects",bottom:null}、ACTIVITY_BAR_SIZE=40、WIDTH_DEFAULT=250/WIDTH_MIN=160/WIDTH_MAX=500、SPLIT_DEFAULT=0.5/SPLIT_MIN=0.1/SPLIT_MAX=0.9。导出签名严格按 PREAMBLE 契约 [A]。
【SB-2】toggleViewPure(zones, open, id): OpenState——查 id 归属 zone；open[zone]===id → 置 null（R2）；否则 → open[zone]=id（R1，直接覆盖即隐式关闭同区旧视图）。id 未在 zones 中 → 原样返回（防御）。无任何历史记忆——返回值仅由入参推导。
【SB-3】moveButtonPure(zones, open, id, targetZone, index): {zones;open}——源区移除 id、按 clamp 后 index 插入目标区（R8，clamp 到 [0, len]）；若 open[源区]===id → open[源区]=null 且 open[目标区]=id（R6 跟随+替换目标区）；未打开则 open 不变（R7）。同区移动只调顺序不动 open。不校验 id 是否已注册（注册表职责分离）。
【SB-4】deriveLayout(open): LayoutKind——双空→hidden；仅上→single-top；仅下→single-bottom；双开→split（R3/R4/R5）。reconcileZones(saved, open, registeredIds): {zones;open}——saved 两数组过滤 registeredIds、保留 saved 顺序、缺失的注册 id 追加上区末尾（R9）、open 逐项校验（值非 null 且不在 reconcile 后 zones 中 → null）。sanitizeSideBar(raw): SideBarSlice——raw 非对象 → 全默认；zones.top/bottom 非 string[] → 默认；open.top/bottom 非 string|null → null；width/splitRatio 非 number → 默认否则 clamp；返回完整 SideBarSlice（无 partial）。
【SB-5】dropTarget.ts：computeDropTarget(clientY, buttonRects: {id;top;height}[], zone): {zone;index}——clientY 在按钮上半 → 该按钮 index，下半 → index+1；空白区 → 数组末尾。纯函数不碰 DOM（禁止 document/window/getBoundingClientRect）。
【SB-11】sideBarState.test.ts（~30 用例，纯函数无 mock）：toggleViewPure 4 分支；moveButtonPure 8 分支（跨区跟随替换/跟随空区/未打开仅归属/区内前移/后移/同位/index clamp）；deriveLayout 4 态；reconcileZones 5 分支；sanitizeSideBar 6 分支；computeDropTarget 用例（含于本文件）；S1–S6 场景序列 6 条——连续调用纯函数断言终态，直译 docs/sidebar-requirements.md §5 六个验收场景（每场景一条 it，文案含 S1…S6 标记）。` },
  { label: 'view-registry', prompt: `你负责 SB-6、SB-7、SB-12。先读 docs/sidebar-checklist.md「Stage 1 — 注册表（agent B）」再动手。

文件（全部新建）：
- src/features/sideViews/sideViewRegistry.ts
- src/__tests__/sideViewRegistry.test.ts

【SB-6】sideViewRegistry.ts：SideViewDef={id;title;icon;component: React.ComponentType<SideViewComponentProps>}；SideViewComponentProps={switchToPage:(projectId:string,pageId:string)=>void;onDeletePage:(projectId:string,pageId:string)=>void}（与 SidebarTree props 精确匹配）。模块级单例 sideViewRegistry——参照 src/panels/terminal/TabTitleRegistry.ts 同一模式（先读该文件对齐风格）。导出签名严格按 PREAMBLE 契约 [B]。
【SB-7】register(def)（同 id 覆盖）/ getAll(): SideViewDef[]（注册序）/ get(id) / _reset()（仅测试用，清空内部状态）。
【SB-12】sideViewRegistry.test.ts（~6 用例）：register/getAll/get、重复注册覆盖、未注册 get→undefined、_reset 隔离——照 src/__tests__/TabTitleRegistry.test.ts 模式（先读对齐）。` },
  { label: 'sidebar-store', prompt: `你负责 SB-8、SB-9、SB-10、SB-13。先读 docs/sidebar-checklist.md「Stage 1 — store（agent C）」、docs/sidebar-execution-plan.md §3「实现要点」、src/stores/fontSize.ts（49-87 持久化模式模板）与 src/stores/keybindings.ts 再动手。

文件：
- src/stores/sideBar.ts（新建）
- src/stores/index.ts（追加 re-export）
- src/__tests__/sideBar.test.ts（新建）

【SB-8】sideBar.ts：SideBarState extends SideBarSlice { loaded: boolean; toggleView(id); moveButton(id, zone, index); setWidth(w); setSplitRatio(r); loadFromDisk(): Promise<void> }——toggleView/moveButton 委托契约 [A] 的 toggleViewPure/moveButtonPure；setWidth/setSplitRatio 内部 clamp（WIDTH_MIN/MAX、SPLIT_MIN/MAX）；默认值取自契约 [A] 常量。导出 useSideBar（UseBoundStore<StoreApi<SideBarState>>）。
【SB-9】持久化照 fontSize.ts 模式：loadFromDisk 读 loadSettings()（import 自 ../ipc/settings）→ saved?.sideBar → sanitizeSideBar → reconcileZones(slice.zones, slice.open, sideViewRegistry.getAll().map(d=>d.id)) → set + loaded:true（try/catch 保默认）；subscribe + loaded 守卫 + 2s debounce（PERSIST_DEBOUNCE_MS import 自 ./projects）→ saveSettings({ sideBar: { zones, open, width, splitRatio } })（payload 键集合严格按契约 [C]，saveSettings(...).catch(()=>{}) 静默吞错与 fontSize/keybindings 一致）；导出 cancelPendingSave()。
【SB-10】src/stores/index.ts 追加 re-export useSideBar + 类型（SideBarState 等，对齐现有 re-export 风格）。
【SB-13】sideBar.test.ts（~12 用例）：默认值；toggle/move 经 store；loadFromDisk 合法/脏数据 sanitize/缺失/异常降级；loaded 守卫防启动空写；fake timers 验证 debounce → saveSettings({sideBar}) payload 键集合精确匹配——照 fontSize.test.ts / keybindings.test.ts 模式（mock ../ipc/settings，fake timers，_resetPersistence 类辅助按 keybindings.test.ts 做法）。
测试注意（执行计划 §3）：本测试只 import store 不 import sideViewDefs → sideViewRegistry.getAll() 为空会过滤全部 saved id，beforeEach 需先手动 sideViewRegistry.register stub 视图（id 用 "projects"/"explorer"，component 用 () => null 即可），或 vi.mock registry；每例 _reset() 隔离。
硬约束 #1：sideBar.ts 禁止 import invoke / @tauri-apps/api——只经 ../ipc/settings。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage1 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
返回 JSON：{ "allFixed": true/false, "failedItems": ["未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
