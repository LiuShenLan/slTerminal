// =====================================================================
// Stage 02 前端基建（FE-01 ~ FE-03）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - IPC 命令：background_tasks_list() 无参；background_tasks_set_config(taskId, enabled?, intervalSec?)
//   参数 JS 侧 camelCase：{ taskId, enabled?, intervalSec? }——undefined 键不发送（缺省键不入 payload）
// - 事件：background-tasks-updated，payload = BackgroundTaskInfo[]
// - DTO 六键：taskId/title/enabled/intervalSec/intervalMin/intervalMax，无 default 字段
// - taskId 值集 = ["planBalance", "sessionRefresh"]（前端 BACKGROUND_TASK_IDS 常量，与后端 TASKS 键集双边锁死）
// - 任务元数据（后端注册表单点，前端不复制）：planBalance title=套餐余量查询 默认10s 区间10-3600；
//   sessionRefresh title=会话历史刷新 默认3s 区间2-300
// - 调度器快照形状：TaskSnapshot<T> = { state: "idle"|"loading"|"ready"|"error", data: T | undefined }
// - 任务定义：{ id, run(source: TriggerSource, prev: T | undefined): Promise<T> }，TriggerSource = "manual" | "tick"
// fix-loop constraints（execution-plan.md）：本 Stage 无特殊纪律，传空串
export const meta = {
  name: 'stage02-frontend-infra',
  description: 'F12 前端基建：types DTO + IPC wrapper/契约测试 + 全局 mock + 调度器注册表 + sessionRefresh 扫描执行体',
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
补充纪律：本 Stage 只做前端源码 + L2 测试，不跑资源共享型测试；invoke 只允许出现在 src/ipc/（硬约束 #1）；DTO 双边对应（硬约束 #4）；并行 agent 文件零重叠，各改各的文件；测试文件照现有模式（ipc-plan-balance-contract.test.ts / cliProfileRegistry 测试先例）。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-ipc',
    prompt: `你负责 FE-01 / FE-02，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目（含可照抄代码块），逐条实现：

【FE-01】新建 src/types/backgroundTasks.ts：照 checklist FE-01 3.1 完整代码块原样写入——BackgroundTaskInfo 六键接口、BACKGROUND_TASK_IDS = ["planBalance", "sessionRefresh"] as const、BackgroundTaskId 类型、PLAN_BALANCE_TASK_ID / SESSION_REFRESH_TASK_ID 常量。

【FE-02】IPC 层 + 契约测试 + 全局 mock：
- 3.1 新建 src/ipc/backgroundTasks.ts（照 checklist 3.1 代码块）：listBackgroundTasks（invoke "background_tasks_list" 无参）、setBackgroundTaskConfig（invoke "background_tasks_set_config"——args 只含提供的键：enabled !== undefined 才加，intervalSec !== undefined 才加；必须含 taskId）、onBackgroundTasksUpdated（listen "background-tasks-updated" 解包 payload，返回 unsubscribe）。这是两个命令的唯一 invoke 位置。
- 3.2 src/ipc/index.ts:17 后插 export * as backgroundTasks from "./backgroundTasks";（planBalance 行保留）。
- 3.3 src/ipc/planBalance.ts：删 setPlanBalanceInterval（:31-34）及其上方注释行；getPlanBalance / refreshPlanBalance / onPlanBalanceUpdated 保留。
- 3.4 新建 src/__tests__/ipc-background-tasks-contract.test.ts（照 ipc-plan-balance-contract.test.ts 模式；setup.ts 无 ../ipc/backgroundTasks 全局 mock，直接真实导入；mock @tauri-apps/api/event 的 listen）：
  - listBackgroundTasks 四维：命令名 / 无参 payload {} / 透传两任务清单 / 异常传播
  - setBackgroundTaskConfig 四维：命令名 / payload 键集合精确（{taskId, intervalSec: 120} 与 {taskId, enabled: false} 两形态各一例 expectExactKeys） / 返回清单透传 / 异常传播
  - onBackgroundTasksUpdated 手写模拟驱动两例（解包 payload + unsubscribe 调 listen 清理函数）
  - BackgroundTaskInfo 键集合六键精确匹配（["taskId","title","enabled","intervalSec","intervalMin","intervalMax"]）
  - BACKGROUND_TASK_IDS 值集精确 == ["planBalance","sessionRefresh"]
- 3.5 src/__tests__/ipc-plan-balance-contract.test.ts：删 setPlanBalanceInterval 合约 describe 段（:144-177），文件头注释「三命令」改「两命令」。
- 3.6 src/__tests__/setup.ts 新增全局 mock（照 planBalance 先例，代码照 checklist 3.6 代码块）：vi.mock("../ipc/backgroundTasks", ...)——listBackgroundTasks resolve 两任务默认清单（planBalance 10s / sessionRefresh 3s），setBackgroundTaskConfig resolve []，onBackgroundTasksUpdated 返回 no-op 清理函数。此 mock 为下游 Stage 03 nav-tree 测试（真实 useAgentHistory → 调度器 activate → listBackgroundTasks）前置依赖，必须本 Stage 到位。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
  {
    label: 'frontend-scheduler',
    prompt: `你负责 FE-03，先读 docs/background-tasks/checklist.md 中 FE-03 完整条目（含可照抄代码块），逐条实现：

新建 src/features/backgroundTasks/ 模块（注册表家族契约 #13 模块级单例）：
- 3.1 types.ts（照 checklist 3.1 代码块）：TriggerSource / TaskRunState / TaskSnapshot<T> / BackgroundTaskDef<T>。
- 3.2 scheduler.ts（照 checklist 3.2 完整代码块）：BackgroundTaskScheduler 类——register（同 id 覆盖）/ getAll（注册序）/ _reset（停全部 timer + 清空）/ subscribe（立即回调当前快照；首个订阅者 activate；退订返回函数，最后订阅者 stopTimer）/ triggerNow（manual，与 tick 共用 runOnce 闸门）/ applyConfig（启停与改频率立即生效；无订阅者不空转；禁用→启用立即一轮 + restartTimer）/ applyLocal（updater 变换 data + 广播）/ activate（配置未读先 listBackgroundTasks，失败 console.error 且仍执行首轮但不启动 interval）/ startIfEnabled（disabled 不执行不启动）/ startTimer/stopTimer/restartTimer / runOnce（防重入 running 闸门；loading → ready / tick 失败静默快照不变 / manual 失败置 error 保留 data）/ broadcast；模块级单例导出 backgroundTaskScheduler。
- 3.3 sessionRefreshTask.ts（照 checklist 3.3 代码块）：runSessionRefresh——遍历 cliProfileRegistry.getAll() 按 p.capabilities.history !== undefined 过滤，Promise.allSettled 逐 profile scanAgentHistory(p.id, true)（恒 force=true），单 provider 失败按 cliId 过滤保留 prev 旧数据，全部失败 throw；backgroundTaskScheduler.register({ id: SESSION_REFRESH_TASK_ID, run })。SESSION_REFRESH_TASK_ID 从 ../../types/backgroundTasks 导入，禁写字面量。
- 3.4 tasks.ts（注册触发点）：内容 = import "./sessionRefreshTask";（side-effect import，硬约束 #13，禁止隐式初始化）。
- 3.5 index.ts（barrel，不触发注册）：export backgroundTaskScheduler + 类型。
- 3.6 新建 src/__tests__/background-tasks-scheduler.test.ts（mock ../ipc/backgroundTasks hoisted 可控；每用例 beforeEach backgroundTaskScheduler._reset()）：
  - 注册表契约：register/getAll 注册序 / 同 id 覆盖 / _reset 清空
  - subscribe：立即回调当前快照（idle）；首个订阅者 → listBackgroundTasks 被调 + enabled=true 立即执行一轮；enabled=false 配置不执行不启动定时；list 失败仍执行首轮 + console.error + 不启动 interval
  - 订阅者计数启停：fake timers 断言 interval 按 intervalSec 触发；全部退订 advanceTimers 不再触发；重订阅立即一轮且不重复读配置（listBackgroundTasks 仍 1 次）
  - tick 防重入：run 挂起时 tick/manual 均被闸门跳过（run 调用次数不增）
  - 失败处理：tick 失败快照不变（state 保持 ready data 保留）+ console.error；manual 失败 state=error + data 保留
  - applyConfig：运行期改频率 timer 重启；禁用停 timer；启用立即一轮 + 启动 timer；无订阅者不启动
  - applyLocal：updater 变换 data + 广播（state 不变）
- 3.7 新建 src/__tests__/background-tasks-session-refresh.test.ts（mock ../ipc/agentHistory scanAgentHistory hoisted 可控 + mock ../ipc/backgroundTasks listBackgroundTasks 恒返回 sessionRefresh 配置防 activate 真实 invoke；beforeEach backgroundTaskScheduler._reset() + cliProfileRegistry._reset() + 文件顶部 import "../features/backgroundTasks/tasks" + 注册桩 profile 含 history 能力有无两种）：
  - 遍历聚合：两 history profile → scanAgentHistory 各调一次（force=true）→ 聚合扁平列表
  - 无 history 能力 profile 被跳过（不调 scanAgentHistory）
  - 部分失败隔离：A resolve 新值 B reject → 结果 = A 新值 + B 旧值（先建 prev：首轮全成功 → 第二轮 B 失败）
  - 全部失败：tick → 快照不变；manual → state=error
  - force 恒 true：断言每次调用第二参 true

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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
