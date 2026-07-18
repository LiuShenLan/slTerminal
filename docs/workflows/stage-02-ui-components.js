// =====================================================================
// Stage 2 — UI 组件：ActivityBar + SideBarArea（SB-14~SB-20）
// =====================================================================
// 生成依据：docs/sidebar-checklist.md「Stage 2 — UI 组件」逐 ID 原文
//           + docs/sidebar-execution-plan.md §4（实现要点）
// 前置：Stage 1 已 commit——src/features/sideViews/sideBarState.ts、
//       sideViewRegistry.ts、src/stores/sideBar.ts 已存在，签名即下方消费面契约。
// 调用：Workflow({ scriptPath: 'docs/workflows/stage-02-ui-components.js' })
// commit message（verify 全绿后）：
//   feat: 活动栏与侧栏区组件——ActivityBar（点击开关+HTML5 拖拽）+ SideBarArea（Allotment 双槽 display:none）
//
// fix-loop 调用参数（verify 不通过时）：
//   Workflow({ scriptPath: 'docs/workflows/fix-loop.js', args: {
//     stage: 2, failedItems, fixContext,
//     verifyFile: 'docs/workflows/verify/stage-02.md',
//     constraints: '硬约束 #6 配色只允许 theme/colors.ts token；零新依赖（禁 react-dnd，不改 package.json）；SidebarTree/ExplorerPanel 组件本体不改；并行 agent 不跑测试'
//   }})
//
// === 消费面契约（Stage 1 已落盘，写死，不各自推断）===
// - useSideBar（src/stores/sideBar.ts）：state { zones, open, width, splitRatio, loaded }
//   + toggleView(id) / moveButton(id, zone, index) / setWidth(w) / setSplitRatio(r) / loadFromDisk()
// - sideViewRegistry（src/features/sideViews/sideViewRegistry.ts）：
//   register(def) / getAll(): SideViewDef[] / get(id) / _reset()；
//   SideViewDef = { id; title; icon; component: React.ComponentType<SideViewComponentProps> }；
//   SideViewComponentProps = { switchToPage(projectId, pageId); onDeletePage(projectId, pageId) }
// - computeDropTarget（src/features/sideViews/dropTarget.ts）：
//   computeDropTarget(clientY, buttonRects: {id;top;height}[], zone): { zone; index }
// - 配色 token（src/theme/colors.ts，已验证存在）：
//   PANEL_BG、SIDEBAR_FG、FOCUS_BORDER、SIDEBAR_COLORS.{bg,fg,hover,selected,border}
// =====================================================================

export const meta = {
  name: 'stage2-ui-components',
  description: 'Stage 2 UI 组件：ActivityBar（点击开关+HTML5 拖拽）+ SideBarArea（Allotment 双槽 display:none）+ sideViewDefs + L2 测试（SB-14~SB-20）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust（src-tauri/）一律不动。
背景：修复要点详见 docs/sidebar-checklist.md 对应 ID 条目（先读再动手）；规则编号 R1~R9 见 docs/sidebar-requirements.md §4；外观规范见 §6；换区重建为已确认行为（ADR-0001）。
本 Stage 特殊纪律：
1. 硬约束 #6 配色单点：全部颜色只允许引用 src/theme/colors.ts token（PANEL_BG、SIDEBAR_FG、FOCUS_BORDER、SIDEBAR_COLORS.{bg,fg,hover,selected,border}），禁止任何硬编码色值。
2. 零新依赖：HTML5 DnD 原生实现，禁止引入 react-dnd / dnd-kit 等任何新包，禁止改 package.json / package-lock.json。
3. 禁止改 SidebarTree / ExplorerPanel 组件本体（仅 sideViewDefs.ts 中引用）。
4. 禁止执行测试命令（npm test / npx vitest）——并行 agent 不跑测试，统一由全量测试 agent 单点跑；编译级自检可用 npx tsc --noEmit。
5. Stage 1 消费面契约见脚本头部注释（useSideBar / sideViewRegistry / computeDropTarget 签名），严格按签名消费，不各自推断。`

// === Phase 1: 并行重构（2 agent 文件零重叠；禁止跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'activity-bar', prompt: `你负责 SB-14、SB-15、SB-19。先读 docs/sidebar-checklist.md「Stage 2 — ActivityBar（agent A）」与 docs/sidebar-execution-plan.md §4「实现要点」再动手。

文件（全部新建）：
- src/features/sideViews/ActivityBar.tsx
- src/__tests__/activityBar.test.tsx

【SB-14】ActivityBar.tsx 结构：40px 宽 flex 列（上区按钮组 + flex:1 间隔 + 下区按钮组）；按钮 40×40、emoji 居中、title={def.title}、data-e2e={"activity-btn-"+id}；active 判定——按钮 id 归属 zone 从 useSideBar 的 zones 查，active = open[zone]===id（此时该视图必在展示，单槽位模型）；active 态高亮背景 + 左侧 2px 指示条（VS Code 风格）；hover 反馈；点击 → useSideBar.getState().toggleView(id)。按钮渲染数据：zones.top/bottom 顺序 map id → sideViewRegistry.get(id)，过滤 undefined（持久化 id 未注册防御）。
【SB-15】HTML5 拖拽（零依赖）：按钮 draggable + onDragStart（dataTransfer.setData("application/x-side-view-id", id)、effectAllowed="move"、dragging 半透明态）；按钮/半区容器 onDragOver（preventDefault——必须，否则不触发 drop；收集本 zone 各按钮 rect 后调 computeDropTarget(clientY, rects, zone) → dropIndicator local state {zone,index}，渲染对应位置 2px 插入指示线，token 色 FOCUS_BORDER）；onDrop → useSideBar.getState().moveButton(id, zone, index)；onDragEnd/onDragLeave 清指示线。拖拽仅活动栏内有效（外部不监听 drop）。
【SB-19】activityBar.test.tsx（~12 用例）：两组按钮渲染（上/下分组+顺序）、active class+指示条、点击→toggleView（spy store）、tooltip/title、dragStart dataTransfer 内容、dragOver 指示线 state、drop→moveButton 参数（zone/index）、dragEnd 清指示、拖出活动栏无 moveButton 调用。种子 sideBar store（真实 store，beforeEach setState 重置默认）+ sideViewRegistry._reset() 后 register stub 视图（() => null 组件）；mock dataTransfer（{ setData: vi.fn(), getData: vi.fn(), effectAllowed: "", dropEffect: "" }）。` },
  { label: 'sidebar-area', prompt: `你负责 SB-16、SB-17、SB-18、SB-20。先读 docs/sidebar-checklist.md「Stage 2 — SideBarArea（agent B）」与 docs/sidebar-execution-plan.md §4「实现要点」再动手。

文件（全部新建）：
- src/features/sideViews/SideBarArea.tsx
- src/features/sideViews/sideViewDefs.ts
- src/features/sideViews/index.ts
- src/__tests__/sideBarArea.test.tsx

【SB-16】SideBarArea.tsx：props { switchToPage; onDeletePage }。结构：<Allotment vertical proportionalLayout={true} onChange={...}> 两 pane——上 pane visible={!!open.top} preferredSize={splitRatio*100}，下 pane visible={!!open.bottom} preferredSize={(1-splitRatio)*100}（preferredSize 用比例基数）。每 pane 内：zones[zone].map(id → sideViewRegistry.get(id)) 过滤 undefined → 每视图一槽 <div style={{ display: open[zone]===v.id ? "flex" : "none", height: "100%" }}>，槽内 <v.component switchToPage={switchToPage} onDeletePage={onDeletePage} />（display:none 保挂载，隐藏不卸载）。onChange 换算 ratio = sizes[0]/(sizes[0]+sizes[1]) → setSplitRatio——仅在两 pane 均 visible 时写回（单开时另一项为 0，除零守卫）。换区重建为已知行为（ADR-0001）：组件随 zones 跨 pane 移动即卸载重建。
【SB-17】sideViewDefs.ts：side-effect 注册两条（类比 src/panels/terminal/tabRules.ts 模式）——sideViewRegistry.register({ id: "projects", title: "项目列表", icon: "📋", component: SidebarTree })（SidebarTree 从 ../../features/sidebar 导入，props 精确匹配 SideViewComponentProps）；sideViewRegistry.register({ id: "explorer", title: "文件浏览器", icon: "📁", component: () => <ExplorerPanel /> })（ExplorerPanel 从 ../../features/explorer 导入，箭头包装忽略 props）。
【SB-18】index.ts barrel：导出 ActivityBar、SideBarArea、sideViewRegistry、类型（SideViewDef/SideViewComponentProps 等）、纯函数（sideBarState.ts 与 dropTarget.ts 的导出）——不 re-export sideViewDefs（由 Workspace 显式 side-effect import，同 tabRules 模式；组件内不 import sideViewDefs 防循环）。
【SB-20】sideBarArea.test.tsx（~10 用例）：单开上/下全高（visible pane 数=1）、双开两 pane visible、视图槽 display 切换、换区后视图移槽（重建可断言旧组件卸载新组件挂载）、props 透传到视图组件、onChange→setSplitRatio。mock allotment 为「渲染 children 并记录 Pane props」的 stub（不能全 null——需断言 visible/preferredSize；参照 src/__tests__/workspace-e2e-ready.test.tsx 的 MockAllotment 模式但保留 children 渲染）；种子真实 sideBar store + sideViewRegistry._reset() 后 register stub 视图。` },
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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/workflows/verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage2 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
