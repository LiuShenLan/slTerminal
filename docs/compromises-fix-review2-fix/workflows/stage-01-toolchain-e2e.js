// =====================================================================
// Stage 01 工具链与 e2e 设施（TE-01/02/03/04、FE-06）
// 串行形态：用户并发限制约束——agents for 循环顺序 await，不用 parallel()；
// 全量测试命令逐条串行（CP-007 基准负载敏感）。
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-01-toolchain-e2e',
  description: 'Stage 01 工具链与 e2e 设施：ts7 日志标签/vitest exclude/WARN 跨 chunk/fallback 退出码/knip 收窄',
  phases: [
    { title: '串行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点唯一真值源 = docs/compromises-fix-review2-fix/checklist.md 对应 ID 条目（先读再动手）——条目为六段式（位置/现状/修复步骤含可照抄代码块/测试同步/文档同步/验证），修复步骤段的代码块照抄适配，不自行设计；每项完成后跑条目「验证」段断言自查。`

// === Phase 1: 串行修复（用户并发限制——顺序 await，禁 parallel）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'toolchain-scripts',
    prompt: `你负责 TE-01、TE-02（先读 checklist 对应条目再动手）：
【TE-01】scripts/check-ts7-trigger.mjs:57 日志标签改「网络/HTTP/解析」（照 checklist 修复步骤）。
【TE-02】vitest.config.ts 删除 exclude 行（:7）——include 已自足，恢复 vitest 默认 exclude 防护。
触碰文件仅限：scripts/check-ts7-trigger.mjs、vitest.config.ts。
自查：node --check scripts/check-ts7-trigger.mjs exit 0。`,
  },
  {
    label: 'wdio-knip',
    prompt: `你负责 TE-03、TE-04、FE-06（先读 checklist 对应条目再动手）：
【TE-03】e2e-tests/run-wdio.cjs:36-46 整段替换为 checklist 内代码块（StringDecoder + tail 残串闭包）——残串必须在 wireWarnCounting 函数闭包内（该函数被 stdout/stderr 调两次，模块级共享会串流）。
【TE-04】同文件 :355 process.exit(code) 改 process.exit(code ?? 1)（对齐 :318 runWdio 通道）。
【FE-06】knip.json:15 收窄为 docs/compromises-fix-review-fix/workflows/*.js，并追加 docs/compromises-fix-review2-fix/workflows/*.js（本任务产物前瞻登记）；:14 前轮条目不动。
触碰文件仅限：e2e-tests/run-wdio.cjs、knip.json。
自查：node --check e2e-tests/run-wdio.cjs exit 0；npx knip --production exit 0（knip 可跑——本项即改它）。`,
  },
]
const refactorResults = []
for (const a of agentsSeq) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break // 前序失败短路，不跑下游
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行执行，禁并行——CP-007 基准负载敏感）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行，禁止并行**（CP-007 基准负载敏感，并行抢核曾致误红）：
1. npx tsc --noEmit
2. npx eslint src/
3. node --check scripts/check-ts7-trigger.mjs
4. node --check e2e-tests/run-wdio.cjs
5. npm test（报告用例总数——基线 3261，不得缩水）
6. npx knip --production
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
