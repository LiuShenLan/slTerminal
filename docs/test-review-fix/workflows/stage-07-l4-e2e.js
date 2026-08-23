// Stage 07：L4 E2E 修复（TQ-E-03, TQ-E-04, TQ-E-05, TQ-E-06, TQ-E-08）
// fix-loop 调用时 args.constraints 传空（本 Stage 无特殊纪律）
export const meta = {
  name: 'stage-07-l4-e2e',
  description: 'Stage 07：L4 E2E 修复——条件等待/吞错/粘贴断言/恢复报告/settings 隔离（5 项）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。
注意：e2e-tests/helpers.ts 不在根 tsconfig include——本 Stage 门禁以 npx vite build + npm run e2e 为准，不要跑 tsc/eslint 判定 e2e-tests 文件。`

phase('并行修复')
const parallelAgents = [
  { label: 'fix-e2e-sleeps', prompt: '你负责 TQ-E-03 + TQ-E-04：agent.e2e.ts / history.e2e.ts / mockcli.e2e.ts 三处 sleep 改 waitUntil 条件等待——先 Read 上下文定「下一步要交互的行选择器」作条件；同文件吞错的 catch 按 checklist 补可观测输出。只改 e2e-tests/agent.e2e.ts, history.e2e.ts, mockcli.e2e.ts。' },
  { label: 'fix-e2e-paste', prompt: '你负责 TQ-E-05：terminal.e2e.ts:87-131 粘贴用例——用例改名对齐实际职责（经 __e2e_writeToTerminal 注入而非真实 OS 按键），补剪贴板读回断言；豁免清单条目细化由 Stage 10 处理，本项只改测试。只改 e2e-tests/terminal.e2e.ts。' },
  { label: 'fix-launcher-restore', prompt: '你负责 TQ-E-06：run-wdio.cjs 恢复逻辑——抽 restoreAll() 返回失败清单，process.on(exit) 钩子逐条打印恢复结果，恢复失败时 process.exitCode = 1。保持 exit 钩子单一恢复点设计不变（文件头注释写明的有意设计）。只改 e2e-tests/run-wdio.cjs。' },
  { label: 'fix-e2e-reset', prompt: '你负责 TQ-E-08：helpers.ts 新增 __slterm_e2e_resetSettings helper（参照既有 __slterm_e2e_resetProjects :200-210 形态），wdio.conf.ts beforeSuite 补调用；src/global.d.ts 补新 helper 的类型声明。触碰：e2e-tests/helpers.ts, e2e-tests/wdio.conf.ts, src/global.d.ts。' },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。先跑 npx vite build（构建图兜底 helpers.ts 等 tsc include 外文件），通过后跑 npm run e2e（= build:e2e + wdio 全量，耗时长，勿中止）。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
E2E 全量须报告 9 spec 的通过/失败计数。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
TQ-E-03/E-04 的 waitUntil 条件须 Read 确认条件是「下一步交互目标出现」而非又一个固定时长的变相 sleep。
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
