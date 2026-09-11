// =====================================================================
// Stage 04 explorer 状态机（FE-02→01→03→04，顺序写死）
// 串行形态：用户并发限制约束——单 agent 顺序执行；全量测试命令逐条串行。
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-04-explorer-state',
  description: 'Stage 04 explorer 状态机：无快照双提交收口/抑制超时兜底/恰好一次断言/注释解除点补齐',
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

// === Phase 1: 串行修复（单 agent，项间顺序写死 FE-02→01→03→04）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'file-tree-state',
    prompt: `你负责 FE-02 → FE-01 → FE-03 → FE-04（**顺序写死不可颠倒**，先读 checklist 对应条目再动手）：
【FE-02 先做】src/features/explorer/useFileTree.ts restoreExpanded 无快照分支（:466-468）删显式 commitViewState()（保留 restoringRef 复位）+ :458-460 注释改写（checklist 措辞照抄）——提交由 rootNodes 渲染落定后的 commit effect 统一承担。
【FE-01 依赖 FE-02】src/__tests__/use-file-tree.test.ts FE-01-2 用例（:1037-1040）waitFor 断言改 toHaveBeenCalledTimes(1)——FE-02 未修时必红，故顺序不可颠倒。
【FE-03】useFileTree.ts rootPath effect（:479-518）加 suppressGuard 10s 兜底定时器 + cleanup + deps 加 commitViewState（checklist 代码块照抄）；测试文件补例「FE-03: loadRoot 永不 settle → 10s 兜底解除抑制并上呼」（fake timers + 挂起桩）。
【FE-04】useFileTree.ts:496-497 注释解除点列举写全 5 点（含 loadRoot 首帧失败 catch 与 FE-03 超时兜底）。
【文档同步】src/features/explorer/CLAUDE.md「提交时机」句尾补 FE-02（无快照分支不显式提交）与 FE-03（抑制兜底 10s）两口径（checklist 文档同步段措辞照抄）。
触碰文件仅限：src/features/explorer/useFileTree.ts、src/__tests__/use-file-tree.test.ts、src/features/explorer/CLAUDE.md。
自查：npx vitest run use-file-tree 绿（含新例）。`,
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
1. npx tsc --noEmit
2. npx eslint src/
3. npx vitest run use-file-tree
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
