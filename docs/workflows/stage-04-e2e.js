// =====================================================================
// Stage 4 — E2E：helpers 增补 + 2 条 L4 用例（SB-25）
// =====================================================================
// 生成依据：docs/sidebar-checklist.md「Stage 4 — E2E（L4）」逐 ID 原文
//           + docs/sidebar-execution-plan.md §6（实现要点）
// 前置：Stage 1~3 已 commit 且人工验证点通过——侧栏视图在真实 app 中可用。
// 单 agent：e2e-tests/helpers.ts + e2e-tests/test.e2e.ts 同一上下文。
// 调用：Workflow({ scriptPath: 'docs/workflows/stage-04-e2e.js' })
// commit message（verify 全绿后）：
//   test: 侧栏视图 E2E——helpers 增补 + 点击开关/拖拽跨区 2 用例
//
// fix-loop 调用参数（verify 不通过时）：
//   Workflow({ scriptPath: 'docs/workflows/fix-loop.js', args: {
//     stage: 4, failedItems, fixContext,
//     verifyFile: 'docs/workflows/verify/stage-04.md',
//     constraints: '只改 e2e-tests/ 下文件，禁止改 src/ 任何文件；不改 wdio.conf.ts/run-wdio.cjs；helpers 注入点必须在 installAllE2eHelpers() 内'
//   }})
//
// 门禁说明：e2e-tests/helpers.ts 不在根 tsconfig include——tsc/eslint 覆盖不到，
//           本 Stage 门禁必须含构建级命令（npx vite build + npm run build:e2e），见全量测试清单。
//
// === 人工验证点（收尾报告中主 agent 必须提示用户实测）===
// 真实 app 拖拽手感（合成事件验证状态机 ≠ 真实鼠标拖拽）：
// 按钮拖拽视觉反馈、插入指示线位置、跨区落点正确。
// =====================================================================

export const meta = {
  name: 'stage4-e2e',
  description: 'Stage 4 E2E：helpers 增补 3 个侧栏 helper + 点击开关/拖拽跨区 2 条 L4 用例（SB-25）',
  phases: [
    { title: '重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust（src-tauri/）一律不动。
背景：修复要点详见 docs/sidebar-checklist.md 对应 ID 条目（先读再动手）；E2E 键盘/事件限制与 helper 通信方式见 e2e-tests/CLAUDE.md。
本 Stage 特殊纪律：
1. 只改 e2e-tests/helpers.ts 与 e2e-tests/test.e2e.ts 两个文件，禁止改 src/ 任何文件，禁止改 wdio.conf.ts / run-wdio.cjs。
2. helpers 注入点必须在 installAllE2eHelpers() 函数体内（E2E_ENABLED 门控链路不变），与现有 helper 同处。
3. 禁止执行任何测试/构建命令（npm test / npm run wdio / npx vite build / npx tauri build）——统一由全量测试 agent 单点跑。
4. 消费 src/stores/sideBar.ts 前先 Read 真实代码对齐签名（useSideBar.getState().toggleView / moveButton）。`

// === Phase 1: 重构（单 agent——helpers + 用例同一上下文）===
phase('重构')
const refactorResults = await agent(`${PREAMBLE}\n\n你负责 SB-25。先读 docs/sidebar-checklist.md「Stage 4 — E2E（L4）」、docs/sidebar-execution-plan.md §6「实现要点」、e2e-tests/helpers.ts（installAllE2eHelpers 结构）、e2e-tests/test.e2e.ts（现有用例风格）与 src/stores/sideBar.ts 后动手。

文件：
- e2e-tests/helpers.ts（改）
- e2e-tests/test.e2e.ts（改）

【SB-25-A】helpers.ts 新增 3 个 window 全局（installAllE2eHelpers() 内，与现有 helper 同处）：
- __slterm_e2e_getSideBarState()——返回 useSideBar.getState() 的 plain 快照（仅 zones/open/width/splitRatio/loaded 数据键，去函数）；
- __slterm_e2e_toggleSideView(id)——等价点击活动栏按钮，走 useSideBar.getState().toggleView(id)；
- __slterm_e2e_moveSideViewButton(id, zone, index)——等价拖拽落点，走 useSideBar.getState().moveButton(id, zone, index)。
【SB-25-B】test.e2e.ts 新增 describe「侧栏视图」2 用例：
- E2E-1 点击开关：__slterm_e2e_createProject 后断言默认项目列表打开且侧栏区可见（__slterm_e2e_getSideBarState().open.top === "projects"）→ browser.execute 内 document.querySelector('[data-e2e="activity-btn-projects"]').click()（真实 DOM click 走 React onClick）→ 断言 open 双空（侧栏区隐藏）→ 再 click 恢复 → click '[data-e2e="activity-btn-explorer"]' → 断言 open.top === "explorer"（R1 替换）。
- E2E-2 拖拽跨区：JS 合成 DnD——const dt = new DataTransfer(); el.dispatchEvent(new DragEvent("dragstart", { dataTransfer: dt, bubbles: true })); target.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true })); target.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true }))（React 18 合成事件经 root 监听可正常分发）→ 断言 __slterm_e2e_getSideBarState() 的 zones 变化 + open 跟随（R6/R7）。若 DataTransfer 构造器在 WebView2 不可用（老版本无），降级为 __slterm_e2e_moveSideViewButton 驱动断言状态机，并在测试文件注释注明降级原因 + 收尾报告注明拖拽手感人工验收。
断言风格与现有 12 用例一致（browser.execute + waitUntil 轮询状态）；选择器只用 data-e2e 属性。`, { label: 'e2e-cases' })

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx vite build
5. npm run build:e2e
6. npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。npm run build:e2e 与 npm run wdio 耗时长（tauri debug 构建 + 真实 WebView2 跑 14 用例），禁止中途放弃；wdio 需报告用例总数（应为 14 = 12 存量 + 2 新增）。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/workflows/verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage4 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
SHAKE-0 断言需你先执行 npx vite build（若测试 agent 已跑过且成功可复用其 dist 产物）后 grep dist/ 产物中不得含 __slterm_e2e_getSideBarState / __slterm_e2e_toggleSideView / __slterm_e2e_moveSideViewButton。
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
