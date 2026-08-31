// =====================================================================
// Stage 03 消费改造（FE-04 ~ FE-06）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - useAgentHistory 返回面：{ state, sessions, activeStatuses, rootPath, triggerNow, removeLocal }（scan 退役）
// - 调度器订阅：backgroundTaskScheduler.subscribe<AgentHistorySession[]>(SESSION_REFRESH_TASK_ID, setSnapshot)
//   首个订阅者 → 立即执行一轮（接管「挂载即扫」语义）+ 按配置频率定时；最后退订停 interval
// - 扫描执行体恒 force=true（scanAgentHistory(cliId, true) 第二参恒 true）
// - footer enabled 感知：usePlanBalance 挂载读 listBackgroundTasks + 订阅 onBackgroundTasksUpdated；
//   enabled: boolean | null（null=未加载不渲染防闪烁）；PlanBalanceFooter 守卫 enabled !== true || items.length === 0
// - taskId 常量引用：PLAN_BALANCE_TASK_ID / SESSION_REFRESH_TASK_ID 从 src/types/backgroundTasks 导入，禁写字面量
// - setup.ts 全局 mock（../ipc/backgroundTasks）已在 Stage 02 到位，本 Stage 测试直接依赖
// fix-loop constraints（execution-plan.md）：本 Stage 无特殊纪律，传空串
export const meta = {
  name: 'stage03-consumers',
  description: 'F12 消费改造：useAgentHistory 订阅调度器（scan 退役 triggerNow）+ useNavTree 适配 + footer enabled 感知',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读 docs/background-tasks/checklist.md 再动手）。
补充纪律：本 Stage 依赖 Stage 02 已就位（调度器 + ipc/backgroundTasks + setup.ts 全局 mock）；L2 测试用 vi.mock 隔离 IPC，不跑资源共享型测试；并行 agent 文件零重叠（navTree 目录下 useNavTree.ts 与 usePlanBalance.ts/PlanBalanceFooter.tsx 是不同文件）；测试文件独立。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'history-consumer',
    prompt: `你负责 FE-04 / FE-05，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目（含可照抄代码块），逐条实现：

【FE-04】重写 src/features/agentHistory/useAgentHistory.ts（照 checklist FE-04 3.1 完整代码块）：
- 返回面 { state, sessions, activeStatuses, rootPath, triggerNow, removeLocal }——scan 删除，triggerNow 新增。
- 状态真值源上移 backgroundTaskScheduler：useEffect 订阅 SESSION_REFRESH_TASK_ID（import "../backgroundTasks/tasks" 触发注册 + 从 ../backgroundTasks 导入 backgroundTaskScheduler 与 TaskSnapshot 类型 + SESSION_REFRESH_TASK_ID 从 ../../types/backgroundTasks 导入）。
- triggerNow = useCallback 调 backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID)；removeLocal 经 applyLocal（prev ?? [] 过滤 sessionId）。
- activeStatuses 保持 TerminalRegistry 订阅推导；rootPath 推导保持 hook 本地不变。
- 删除：scanAgentHistory/CLAUDE_CLI_ID import、genRef、scan、useRef import。
- 重写 src/__tests__/agent-history-hook.test.tsx（照 checklist FE-04 4 测试同步节）：mock ../ipc/agentHistory（scanAgentHistory）+ ../ipc/backgroundTasks（listBackgroundTasks 返回 sessionRefresh 单任务 enabled=true intervalSec=300 配置，set/onUpdated 防御 stub）+ TerminalRegistry（同现状）；beforeEach backgroundTaskScheduler._reset() + cliProfileRegistry._reset() + cliProfileRegistry.register(claudeProfile)（claudeProfile 自 ../features/cliProfiles/profiles/claude 导入）；文件顶部 import "../features/backgroundTasks/tasks"。用例：初始 idle / 订阅后自动执行一轮 ready+sessions（断言 scanAgentHistory("claude", true)）/ manual 失败 → error（triggerNow 后 scanAgentHistory reject）/ removeLocal 不重扫 / activeStatuses 订阅保留 / rootPath 推导保留。

【FE-05】适配 src/features/navTree/useNavTree.ts + 两测试：
- 3.1 删挂载即扫 useEffect（:198-200——订阅语义接管）；refresh 改调 history.triggerNow()（照 checklist 3.1 代码块，useCallback 依赖 [history.triggerNow]）；接口注释 :86-89 同步改写；useEffect import 若无其他消费则删。返回面字段名 refresh 不变（NavTree.tsx 零改动）。
- 3.2 src/__tests__/nav-tree.test.tsx 与 nav-tree-history.test.tsx 适配：追加 mock ../ipc/backgroundTasks（listBackgroundTasks 返回 sessionRefresh enabled=true intervalSec=300 配置——大间隔防 tick 干扰；set/onUpdated 防御 stub）；beforeEach 增加 backgroundTaskScheduler._reset() + cliProfileRegistry._reset() + cliProfileRegistry.register(claudeProfile)；文件顶部 import "../features/backgroundTasks/tasks"；「挂载即 scan」类断言语义保留（订阅首轮即调 scanAgentHistory）但断言参数由 ("claude", undefined) / ("claude") 改 ("claude", true)（force 恒 true）；nav-tree-history.test.tsx FE-19 两例保留语义：「挂载即扫一次」（订阅首轮）/「展开不重复 scan」/「刷新钮显式重扫」改断言点击刷新钮后 scanAgentHistory 第二次调用且 force=true——用例名与注释更新为 triggerNow/定时刷新语义。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
  {
    label: 'balance-footer',
    prompt: `你负责 FE-06，先读 docs/background-tasks/checklist.md 中 FE-06 完整条目（含可照抄代码块），逐条实现：

【FE-06】footer enabled 感知：
- 3.1 src/features/navTree/usePlanBalance.ts：
  - import 追加 listBackgroundTasks / onBackgroundTasksUpdated（../../ipc/backgroundTasks）与 PLAN_BALANCE_TASK_ID（../../types/backgroundTasks）。
  - 新增 state：const [enabled, setEnabled] = useState<boolean | null>(null);（null=配置未加载——footer 不渲染防闪烁）。
  - 挂载 effect 内追加（与首拉并行）：listBackgroundTasks().then(取 PLAN_BALANCE_TASK_ID 对应 enabled ?? true)（cancelled 检查）+ .catch(console.error + setEnabled(true) 回退启用) + onBackgroundTasksUpdated 订阅（同口径更新 enabled）。
  - cleanup 改 return () => { cancelled = true; unlisten(); unlistenConfig(); };
  - 返回面改 { items, refresh, enabled }；头注释更新（F12：enabled 感知通道 = list 读 + background-tasks-updated 事件订阅）。
- 3.2 src/features/navTree/PlanBalanceFooter.tsx:44-45 渲染守卫改：const { items, refresh, enabled } = usePlanBalance(); + if (enabled !== true || items.length === 0) return null;（禁用即整块不渲染，快照保留——重启用即重显最后快照；守卫必须在所有渲染之前）。
- 3.3 src/__tests__/plan-balance-footer.test.tsx：vi.mock 追加 ../ipc/backgroundTasks（hoisted：listBackgroundTasks 默认 resolve 含 planBalance enabled=true 清单，onBackgroundTasksUpdated 捕获回调供 triggerConfigUpdate 测试辅助——照 triggerUpdate 先例）；新增用例：
  - enabled=false → 整块不渲染（有快照也隐藏）——list 返回 enabled=false + getPlanBalance 返回行数据
  - 事件推送 enabled=false → 已渲染 footer 隐藏；再推 enabled=true → 重显最后快照（triggerConfigUpdate 两连发）
  - list 失败 → 按启用处理（footer 正常渲染）——reject + console.error spy

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。注：命令 5 cargo test 在本机有已知环境崩溃（0xC0000139）——测试二进制链接 tauri 栈代码后启动即崩，2026-08-31 定位并登记 test-inventory 豁免，预期 exit 127 属环境故障不算失败；其兜底验证 = cargo check --tests（编译级）+ 测试存在性 grep + clippy。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
