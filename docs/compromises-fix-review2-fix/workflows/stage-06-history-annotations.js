// =====================================================================
// Stage 06 历史执行档注记（SEC-02/03、BE-04、FE-05、DOC-03/04）
// 串行形态：用户并发限制约束——单 agent 顺序执行；全量测试命令逐条串行。
// 特殊纪律（args.constraints 固定文案）：本 Stage 只追加注记，禁改任何生产逻辑/原文改写。
// =====================================================================

export const meta = {
  name: 'stage-06-history-annotations',
  description: 'Stage 06 历史执行档注记：前轮落地复核注记 6 处追加（原文保留不改写）',
  phases: [
    { title: '串行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点唯一真值源 = docs/compromises-fix-review2-fix/checklist.md 对应 ID 条目（先读再动手）——条目为六段式（位置/现状/修复步骤含可照抄代码块/测试同步/文档同步/验证），修复步骤段的代码块照抄适配，不自行设计；每项完成后跑条目「验证」段断言自查。
**本 Stage 特殊纪律（最高优先级）**：只追加「落地复核注记」段落，原文一律保留不改写、不删除、不重排（adr.md:461 先例形态）；禁止改任何生产代码/测试逻辑/脚本——本 Stage 触碰面仅历史执行档 md。`

// === Phase 1: 串行修复（单 agent，逐项顺序执行）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'history-annotations',
    prompt: `你负责 SEC-02、SEC-03、BE-04、FE-05、DOC-03、DOC-04（全部为历史执行档注记追加，先读 checklist 对应条目再动手）：
【SEC-02】docs/compromises-fix-review-fix/checklist.md:102 区追加注记（checklist 修复步骤段措辞照抄，含「落地复核注记（review2-fix SEC-02，2026-09-11）」头）。
【SEC-03】docs/compromises-fix-review-fix/checklist.md:161 区追加注记（同形态）。
【BE-04】docs/compromises-fix-review-fix/checklist.md:295 区追加注记（同形态）。
【FE-05】docs/compromises-fix-review-fix/stages.md:169 区 + docs/compromises-fix-review-fix/workflows/verify/stage-04.md:11 区各追加注记（同形态）。
【DOC-03】docs/compromises-fix-review-fix/execution-plan.md row07 下追加注记（同形态）。
【DOC-04】docs/compromises-fix-review-fix/workflows/verify/stage-07.md:9 区追加注记（同形态）。
触碰文件仅限：docs/compromises-fix-review-fix/ 下上述 5 个文件；**禁止**触碰任何 src/、src-tauri/、scripts/、e2e-tests/ 文件。
自查：git status --porcelain 甄别变更仅限预期 5 文件；每条注记含日期与条目 ID 头。`,
  },
]
const refactorResults = []
for (const a of agentsSeq) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行执行，禁并行——CP-007 基准负载敏感）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行，禁止并行**（CP-007 基准负载敏感，并行抢核曾致误红）：
1. git status --porcelain（甄别：本 Stage 变更仅预期 5 个 docs/compromises-fix-review-fix/ 下 md 文件——零生产逻辑变更确认）
2. npx tsc --noEmit
3. npx eslint src/
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 判断注记是否达成断言意图——注记须含条目 ID 与日期头、原文零改写（字面通过但意图未达判 partial 并说明理由）。
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
