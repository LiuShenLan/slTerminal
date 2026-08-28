// =====================================================================
// F10 编码套餐余量展示 — Stage 02 前端展示层（PB-FE-01~06）
// =====================================================================
// 清单真值源：docs/f10-plan-balance/checklist.md（六段式 + 决策记录 D1-D16/U1-U3）
// 跨边界契约（写死，见 stages.md 头部，两 agent 不各自推断）：
//   ipc/planBalance.ts 三函数：getPlanBalance() / refreshPlanBalance()
//     → Promise<PlanBalanceInfo[]>；onPlanBalanceUpdated(cb) → unsubscribe 函数
//   事件 plan-balance-updated，payload = PlanBalanceInfo[]
//   DTO 键集合（camelCase）：PlanBalanceInfo = amount/frozen/planId/sourceId/updatedAt/windows；
//     AmountInfo = currency/value；WindowsInfo = fiveHour/sevenDay；WindowInfo = remainingPercent/resetsAt
//   logo 路径 /plan-icons/<planId>.png；data-e2e = plan-balance-footer / plan-balance-row
// Stage 特殊纪律：无（fix-loop 调用时 args.constraints 传空串）
// =====================================================================

export const meta = {
  name: 'f10-stage02-frontend',
  description: 'F10 Stage 02：前端余量 footer（DTO/ipc/纯函数/hook/组件 + L2 测试）',
  phases: [
    { title: '前端数据层' },
    { title: '前端 UI 层' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：实现依据 = docs/f10-plan-balance/checklist.md 对应 ID 条目（先读再动手）；头部「决策记录」D1-D16 与 stages.md 头部「跨边界契约」全部写死，不做设计判断。`

// === Phase 1: 前端数据层（fe-data：PB-FE-01/02/03/04；与 fe-ui 文件零重叠）===
phase('前端数据层')
const parallelAgents = [
  {
    label: 'fe-data',
    prompt: `你负责 PB-FE-01 / PB-FE-02 / PB-FE-03 / PB-FE-04（顺序执行）。先 Read docs/f10-plan-balance/checklist.md 对应四条目六段式，代码骨架**照抄适配**，不另行设计。

【PB-FE-01】新建 src/types/planBalance.ts——camelCase DTO 四接口（PlanBalanceInfo/AmountInfo/WindowsInfo/WindowInfo），键集合与 stages.md 头部跨边界契约一字不差（与后端 serde camelCase 双边对应，硬约束 #4）。
【PB-FE-02】新建 src/ipc/planBalance.ts（照 checklist 骨架：getPlanBalance / refreshPlanBalance / onPlanBalanceUpdated 返回 unsubscribe，listen 事件名 plan-balance-updated）；src/ipc/index.ts 在 notification 行后加 export * as planBalance from "./planBalance";。
【PB-FE-03】src/__tests__/setup.ts 在 agentHooks mock 块后追加 ../ipc/planBalance 全局 mock（getPlanBalance/refreshPlanBalance resolve 空数组，onPlanBalanceUpdated 返回 no-op 取消函数——照 checklist 代码块）；src/__tests__/CLAUDE.md「全局 mock 策略（setup.ts）」节清单三条后加第四条登记（照 checklist 文案）。
【PB-FE-04】新建 src/__tests__/ipc-plan-balance-contract.test.ts：
  1. 文件顶部按先例覆盖全局 mock：vi.mock("../ipc/planBalance", async (importOriginal) => importOriginal())
  2. describeIpcContract("planBalance", [...]) 两组四维（get_plan_balance / refresh_plan_balance：命令名、无参、返回透传、异常传播——工厂 helpers/ipc-contract.ts，先例 ipc-agent-history-contract.test.ts）
  3. listen 解包手写 2 例（先例 src/__tests__/ipc-agent-hooks-contract.test.ts:197-227）：捕获 listen("plan-balance-updated", handler) → 构造 { payload } → callback 收到解包数组；unsubscribe 调用链
  4. DTO 键集合断言 1 例：Object.keys(info).sort() 精确等于 ["amount","frozen","planId","sourceId","updatedAt","windows"]

【红线】invoke 只允许出现在 src/ipc/（硬约束 #1）；onPlanBalanceUpdated 必须返回 unsubscribe 函数（notify.ts onFsEvent 先例形态）。
【测试纪律】实现过程只跑 npx vitest run ipc-plan-balance 单文件验证；全量门禁由后续测试 agent 统一执行。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 前端 UI 层（fe-ui 依赖 fe-data 的 ipc 模块存在才能编译——pipeline 串行）===
phase('前端 UI 层')
const sequentialAgents = [
  {
    label: 'fe-ui',
    prompt: `你负责 PB-FE-05 / PB-FE-06（顺序执行）。先 Read docs/f10-plan-balance/checklist.md 对应两条目六段式，代码骨架**照抄适配**，不另行设计。前序 agent 已交付 src/types/planBalance.ts 与 src/ipc/planBalance.ts（三函数签名见 stages.md 头部跨边界契约，按契约 import，不各自推断）。

【PB-FE-05】新建 src/features/navTree/planBalanceModel.ts（照 checklist 骨架全文：currencySymbol / planLogoSrc / formatResetTime / formatUpdatedAt / rowText / rowTooltip——语义 D12 写死，零依赖 theme）；新建 src/__tests__/plan-balance-model.test.ts 约 20 例（照 checklist 测试同步节逐组：currencySymbol 3 / planLogoSrc 1 / formatResetTime 8 / formatUpdatedAt 2 / rowText 4 / rowTooltip 4）。
【PB-FE-06】
  1. 新建 src/features/navTree/usePlanBalance.ts（照骨架：REFRESH_THROTTLE_MS = 5000 导出、lastRefreshRef 节流、双 catch 补 console.error）
  2. 新建 src/features/navTree/PlanBalanceFooter.tsx（照骨架：items.length===0 → null 整块不渲染含发丝线；行 data-e2e="plan-balance-row"、容器 data-e2e="plan-balance-footer"；img onError 隐藏；rowBaseStyle(false, hovered, ROW_HEIGHT) + DIM_FG + fontSize 12；颜色仅 DIM_FG/SEPARATOR_BG token——硬约束 #6 无新例外，禁止任何硬编码颜色值）
  3. 改 src/features/navTree/NavTree.tsx：树区 div 之后、「添加项目」钮注释之前插入 footer 挂载（U1 位置写死），import 区加 PlanBalanceFooter
  4. 新建 src/__tests__/plan-balance-footer.test.tsx 约 10 例（照 checklist 测试同步节：文件级 vi.mock 自定义 ../../ipc/planBalance 实现接管 setup.ts 全局 mock；四场景渲染/隐藏态/初始拉取/事件订阅/点击节流 vi.spyOn(Date,"now")/logo onError 与 src/tooltip）

【红线】
- footer 必须位于树滚动区与「添加项目」钮之间（U1），不是侧栏最底部
- 全部颜色经 theme token（DIM_FG/SEPARATOR_BG），禁止硬编码 rgba/hex
- 不改动任何既有 nav-tree 测试文件（nav-tree*.test.tsx 零改动——全局 mock 已保证 footer 默认隐藏）
【测试纪律】实现过程只跑 npx vitest run plan-balance-model 与 npx vitest run plan-balance-footer 两个单文件验证；全量门禁由后续测试 agent 统一执行。`,
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
中间态说明：本 Stage 只动前端——clippy/cargo test 应零变化全绿；npm test 须含 plan-balance 三个新测试文件全部用例通过 + 既有 nav-tree 用例零回归。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/f10-plan-balance/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
