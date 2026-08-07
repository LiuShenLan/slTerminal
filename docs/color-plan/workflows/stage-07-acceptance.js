// =====================================================================
// Stage 07 验收：完整性核查 + 六门禁 + 人工验证点收尾（ACC-01~06）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   本 Stage 无代码变更（验收 Stage 无 commit）；完整性核查对照
//   docs/color-plan/execution-plan.md Stage 编排总表的 commit message。
//   build:e2e 必须先于 wdio——由串行 phase 的 A1-accept 先跑 build:e2e
//   产出最新二进制，全量测试 phase 六命令并行时 wdio 复用该二进制。
// fix-loop 调用约定：本 Stage 不传 args.constraints。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage7-acceptance',
  description: 'Stage 07 验收：完整性核查 + 六门禁 + 人工验证点收尾（ACC-01~06）',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
本 Stage 不产生代码变更（验收 Stage 无 commit）；禁止改任何文件。`

// === Phase 1: 并行重构（本 Stage 无并行任务）===
phase('并行重构')
const parallelAgents = [];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构 ===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'A1-accept',
    prompt: `你负责 ACC-06 前置与完整性核查（验收准备，不改任何文件）。先 Read docs/color-plan/checklist.md 中 ACC-01~06 条目与 docs/color-plan/execution-plan.md Stage 编排总表。

【ACC-06 前置】执行 npm run build:e2e（VITE_E2E=1 门控构建）——必须成功，产出最新二进制供后续 wdio 使用；失败即报告并中止。

【完整性核查】执行 git log --oneline -8，确认 Stage 01-06 六个 commit 存在且 message 与 execution-plan.md 总表一致（refactor(theme)×4 + test(theme)×1 + docs(theme)×1）。

【ACC-01/02】由全量测试 phase 统一执行，你不重复跑。

返回文本末尾逐字附上 docs/color-plan/stages.md 中的三项人工验证点（供主 agent 收尾交付用户）：
① ACC-03 零视觉截图对比：终端/编辑器/diff/侧栏/活动栏/dockview 页签/allotment sash；
② ACC-04 降级冒烟：settings.json 写 colorScheme: "不存在" → 回退 darcula + console.warn；
③ ACC-05 五通道切换冒烟：临时注册改单色测试方案 → 指向 → 重载 → 五通道（React inline style / xterm ITheme / CM6 theme / dockview CSS 变量 / allotment CSS 变量）全生效 → 还原。`,
  },
];
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
- npx tsc --noEmit
- npx eslint src/
- npm test
- npm run test:l3
- npm run build:e2e
- npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
