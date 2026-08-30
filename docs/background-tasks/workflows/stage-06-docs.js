// =====================================================================
// Stage 06 文档同步（DOC-01 ~ DOC-03）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - 新模块 CLAUDE.md：src-tauri/src/background_tasks/CLAUDE.md（后端骨架：静态切片注册表 U2 形态 /
//   顺序写死「校验→落盘→内存」/ 单写通道复用 settings.rs / 锁序 / spawn-emit 包装层 L1 豁免表）；
//   src/features/backgroundTasks/CLAUDE.md（前端调度器：注册表契约 #13 / 订阅生命周期 / 失败策略 /
//   force 恒 true 理由 / 注册触发点 = useAgentHistory.ts 与 BackgroundTasksPage.tsx 顶部 import ./tasks）
// - 退役词：plan_balance_set_interval / settings-plan-balance（文档内零命中）
// - test-inventory 计数以执行后实跑统计为准（cargo test / npm test 统计行），禁照抄计划预估值
// fix-loop constraints（execution-plan.md）：本 Stage 无特殊纪律，传空串
export const meta = {
  name: 'stage06-docs',
  description: 'F12 文档收口：模块索引/CLAUDE.md 系列/CONTEXT 术语/ADR-0013 补写/test-inventory 同步',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读 docs/background-tasks/checklist.md 再动手）。
补充纪律：文档口径必须对照 Stage 01-05 完成后的真实代码核实（防文档撒谎）——先 Grep/Read 对应代码再写文档，禁止凭计划记忆抄写；用内置 Write/Edit 工具写中文 markdown（filesystem MCP 有乱码风险）；markdown 产物内反引号直接书写（JS 转义纪律不适用）；并行 agent 各持一组 md 文件，文件零重叠。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'docs-backend',
    prompt: `你负责 DOC-01 后半 / DOC-03，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目，逐条实现（先对照真实代码核实再写）：

【DOC-01 后半】后端侧文档：
- 3.1 根 .claude/CLAUDE.md 模块索引表加两行：src/features/backgroundTasks | ../src/features/backgroundTasks/CLAUDE.md 与 src-tauri/src/background_tasks | ../src-tauri/src/background_tasks/CLAUDE.md（插位照现有分组序——前端组与 src-tauri 组各自位置）。
- 3.2 新建 src-tauri/src/background_tasks/CLAUDE.md（模板照 checklist DOC-01 3.2：存在理由——为何骨架独立于 plan_balance（多任务复用/配置单写通道）→ 关键约束：静态切片注册表 U2 形态及理由 / 顺序写死「校验→落盘→内存」/ 单写通道复用 settings.rs 禁第二通道 / 锁序 CONFIG_WRITE_LOCK→SETTINGS_SAVE_LOCK / emit 事件感知通道 / 前端任务 executor=None 仅代管 → 外部坑：save_settings_blocking 是唯二消费点 / spawn-emit 包装层 L1 豁免 → 测试模式 + 既定豁免表）。
- 3.3 src-tauri/src/CLAUDE.md settings.rs 节：白名单行改写为 background_tasks::SETTINGS_KEY 口径；「planBalance 段 = F10 轮询间隔」与「plan_balance_set_interval 专用命令通道」句改写为 backgroundTasks 段 + background_tasks_set_config 通道；新增「save_settings_blocking 同步写通道（F12 抽取，供 background_tasks set_config_core 复用）」句。
- 3.4 src-tauri/src/plan_balance/CLAUDE.md：「轮询间隔：动态内存原子量」节整节改写——通用件上提 background_tasks（内存配置/首轮立即/每轮末 sleep 由骨架承载）；plan_balance_set_interval 退役，配置走 background_tasks_set_config；enabled=false 停轮询 + 快照保留 + 前端 footer 隐藏语义；默认间隔 60→10s（骨架注册表）；本模块保留执行体 poll_once_executor 与快照/merge/emit 口径不变。

【DOC-03】.claude/adr.md 补写 ADR-0013（当前 adr.md 无此条——根 CLAUDE.md F12 行已预引用，不补写即断链）：
- 按 adr.md 既有格式追加，四决策点（照 checklist DOC-03 3）：双端各自抽象（后端 poller 骨架 + 前端调度器）而非单端统一——执行体天然双栖；任务元数据单点在后端注册表（含前端任务）——配置读通道统一 background_tasks_list，前端不复制边界/默认值（DTO 无 default 字段的直接后果）；前端调度器订阅者计数生命周期（无订阅者不空转扫盘）；配置变更前端感知经 emit 事件（background-tasks-updated）而非前端总线——后端单写通道真值源。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
  {
    label: 'docs-frontend',
    prompt: `你负责 DOC-01 前半 / DOC-02，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目，逐条实现（先对照真实代码核实再写）：

【DOC-01 前半】前端侧文档：
- 3.1（与 docs-backend 分工）根 .claude/CLAUDE.md 模块索引两行由 docs-backend 处理，本 agent 不做。
- 3.2 新建 src/features/backgroundTasks/CLAUDE.md（模板照 checklist DOC-01 3.3：存在理由——双端抽象前端半；关键约束：注册表家族契约 #13 / 订阅生命周期（首个订阅者读配置+立即一轮+interval，末退订停）/ 防重入闸门 tick+manual 共用 / 失败策略 tick 静默 vs manual error / applyConfig 运行期生效与无订阅者不空转 / 扫描执行体 force 恒 true 理由（mtime 缓存不敏感）/ 注册触发点 = useAgentHistory.ts 与 BackgroundTasksPage.tsx 顶部 import ./tasks；测试模式：fake timers + hoisted mock 清单）。
- 3.3 src/features/agentHistory/CLAUDE.md「数据流与刷新时机（FE-04/FE-19/NAV-10）」节改写：sessions/state 订阅调度器快照；触发时机 = 首个订阅者立即一轮 + 配置频率定时 + triggerNow 手动；force 恒 true；scan 退役；removeLocal 经 applyLocal。
- 3.4 src/features/navTree/CLAUDE.md：「FE-19 历史扫描时机」三条改写（挂载即扫 → 订阅首轮；刷新钮 = triggerNow）；余量 footer 行追加「enabled=false 不渲染（F12）」；测试模式行追加 background-tasks 两测试文件。
- 3.5 src/ipc/CLAUDE.md：planBalance 命令段删 setPlanBalanceInterval 条目；新增「backgroundTasks 命令（F12）」段（list 读通道 / set_config 写通道+校验顺序 / onBackgroundTasksUpdated 事件）；「测试模式」节契约文件清单加 ipc-background-tasks-contract.test.ts。
- 3.6 src/types/CLAUDE.md 对照表加 backgroundTasks.ts ↔ src-tauri/src/background_tasks/mod.rs。
- 3.7 src/__tests__/CLAUDE.md：全局 mock 清单节加 ../ipc/backgroundTasks 行；F11 测试文件迁移映射表后追加 F12 行（settings-plan-balance.test.tsx 退役 → settings-background-tasks.test.tsx 新增）。
- 3.8 CONTEXT.md：「全局组」行「（快捷键、套餐余量查询频率）」改「（快捷键、后台定时任务）」；新增术语三条——后台定时任务（应用级周期性任务单元，元数据单点在后端注册表，配置统一 settings.json backgroundTasks 段）/ 扫描执行体（历史会话扫描唯一执行路径，遍历全部已注册 history provider 聚合）/ 触发来源（manual/tick，仅影响失败处理策略）。

【DOC-02】.claude/test-inventory.md 同步（计数以执行后实跑统计为准——先跑 cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 与 npm test 的统计输出，按实际数值写，禁照抄计划预估值）：
- settings.rs 行更新（删 1 改名 + 新增 save_rejects_plan_balance_key，净 +1 前后按实跑）。
- plan_balance/mod.rs 行：24 → 14（删 10 例）。
- 新增 background_tasks 模块行（mod.rs + registry.rs 用例数按实跑统计）。
- ipc-plan-balance-contract 行：16 → 12（删 setPlanBalanceInterval 四维 4 例）；新增 ipc-background-tasks-contract 行（按实跑）。
- settings-plan-balance 行删除；新增 settings-background-tasks / background-tasks-scheduler / background-tasks-session-refresh 行；agent-history-hook / nav-tree 两文件计数按实跑更新。
- E2E 区：settings.e2e.ts 11 不变；新增 background-tasks.e2e.ts（6 例：A-F）。
- 豁免登记两条：background_tasks spawn/emit 包装层（L1 不可测，兜底 L4+人工）；tick 失败静默 E2E 豁免（E2E-03 用例 G，兜底 L2+人工）。
- 变更日志加 2026-08-31 F12 行（全量计数按实跑汇总）。

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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。另：报告各测试统计行中的用例计数（cargo test 各模块与 npm test 各测试文件），供 verify 对照 test-inventory 计数断言。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断文档口径是否与真实代码一致（防文档撒谎）——字面通过但口径与代码不符判 partial 并说明理由。用例计数类断言以测试 agent 输出的统计行为准。
以下为测试 agent 的全量测试执行结果（含用例统计行），据此判定：
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
