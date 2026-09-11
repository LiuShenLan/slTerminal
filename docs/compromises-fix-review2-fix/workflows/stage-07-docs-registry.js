// =====================================================================
// Stage 07 文档与登记收口（DOC-01/02/05、TE-05，末位 Stage）
// 串行形态：用户并发限制约束——单 agent 顺序执行；全量测试命令逐条串行。
// 特殊纪律（args.constraints 固定文案）：本 Stage 只改文档/注释/测试文案，禁改生产逻辑。
// =====================================================================

export const meta = {
  name: 'stage-07-docs-registry',
  description: 'Stage 07 文档与登记收口：失实 CSP 表述修正/ADR 行内注记/skill 约定/workspace 签名口径',
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
**本 Stage 特殊纪律（最高优先级）**：只改文档/注释/测试断言文案，禁止改任何生产逻辑——DOC-01 的五处替换文本与 checklist「修复步骤」段逐字一致；DOC-02 为行内注记追加（不打断 ADR 段落结构）。`

// === Phase 1: 串行修复（单 agent，逐项顺序执行）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'docs-registry',
    prompt: `你负责 DOC-01、DOC-02、DOC-05、TE-05（先读 checklist 对应条目再动手）：
【DOC-01】失实 CSP 表述五处替换（checklist「修复步骤」段替换文本逐字照抄）：src/__tests__/markdown-assets.test.ts:19、e2e-tests/html.e2e.ts:163 与 :293、e2e-tests/markdown.e2e.ts:14 与 :234。
【DOC-02】四处行内注记追加：.claude/adr.md:446/:459/:505 与 .claude/test-exemptions.md:39（checklist 注记措辞照抄，行内追加、不打断段落结构）。
【DOC-05】.claude/skills/systematic-changes-execute/SKILL.md 5.6 节追加「提交即登记本 Stage 行」条目（checklist 措辞照抄，含「禁止跨 Stage 捎带」与实证出处）。
【TE-05】src/workspace/CLAUDE.md:75 签名口径改回调形态 getProjectRootPath?: (pageId: string | null) => string | undefined + 回调缘由半句（S11 回归修复）——动手前先 Read src/workspace/tabChrome.tsx 第 4 参真实签名核对，逐字一致防文档撒谎。
触碰文件仅限：上述 7 个文件；**禁止**触碰任何生产逻辑文件。
自查：rg 旧失实表述零命中（见 checklist 各项「验证」段）；git status --porcelain 甄别变更仅限预期 7 文件。`,
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
1. git status --porcelain（甄别：本 Stage 变更仅预期 7 文件——markdown-assets.test.ts / html.e2e.ts / markdown.e2e.ts / adr.md / test-exemptions.md / SKILL.md / workspace/CLAUDE.md，零生产逻辑变更确认）
2. npx tsc --noEmit
3. npx eslint src/
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 判断实现是否达成断言意图——DOC-01 替换文本逐字一致、DOC-02 行内注记不打断段落、TE-05 签名与真实代码逐字一致（字面通过但意图未达判 partial 并说明理由）。
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
