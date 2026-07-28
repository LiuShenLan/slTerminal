// =====================================================================
// Phase 2 Fix — 修复循环 Workflow（阶段验证不通过时由主 agent 循环调用，最多 3 轮）
// =====================================================================
// 用法: Workflow({ scriptPath: 'docs/hooks-dev/phase2-fix/workflows/fix-loop.js',
//   args: { stageName, failedItems, verifyFile, testCommands, constraints, fixContext? } })
//
// args 说明（与 docs/hooks-dev/phase2-fix/execution-plan.md「fix-loop args 规范」一致）:
//   stageName: string     — 所属 Stage 名（用于 label 与报告，如 stage-01-row-model）
//   failedItems: string[] — 未通过项原文列表（必填，非空；照抄 verify agent 返回，不改写不概括）
//   verifyFile: string    — 断言清单文件路径（必填，如 docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md）
//                           ——与 Stage 脚本同一真值源，保证修复循环与 Stage 验证同一标尺
//   testCommands: string[]— 该 Stage 门禁命令（必填；照 execution-plan.md「各 Stage 门禁命令」表逐条）
//   constraints: string   — Stage 特殊纪律（可选；从对应 Stage 脚本 PREAMBLE 复制——单一出处，禁手写第三份）
//   fixContext: string    — verify agent details 证据原文（可选，失败原因线索）
// =====================================================================

export const meta = {
  name: 'fix-loop',
  description: '修复循环: 针对 Stage 验证失败项修复并重验（由主 agent 循环调用，最多 3 轮）',
  phases: [
    { title: '修复问题' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

// args 强制校验——缺失即抛错，不静默降级（静默降级会让修复循环空跑）
if (!args || !Array.isArray(args.failedItems) || args.failedItems.length === 0) {
  throw new Error('fix-loop 需要 args.failedItems（非空数组）')
}
if (typeof args.verifyFile !== 'string' || args.verifyFile.length === 0) {
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md）')
}
if (!Array.isArray(args.testCommands) || args.testCommands.length === 0) {
  throw new Error('fix-loop 需要 args.testCommands（非空数组，照 execution-plan.md 门禁命令表逐条）')
}

const STAGE = args.stageName ?? 'unknown-stage'
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''
const TEST_COMMANDS = args.testCommands.map((c, i) => `${i + 1}. ${c}`).join('\n')

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:/data/learn/code/slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/hooks-dev/phase2-fix/checklist.md 中对应 ID 的修复要点 + docs/hooks-dev/phase2-fix/stages.md 中对应 Stage 的实现要点，再结合失败证据定位
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文
- 禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-${STAGE}` })

// === Phase 2: 全量测试（命令来自 args.testCommands——该 Stage 门禁命令表）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
${TEST_COMMANDS}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止；wdio 类长耗时命令（分钟级）勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（与 Stage 同一断言清单文件）===
phase('逐项验证')
const rawVerify = await agent(`
逐项复查 Stage ${STAGE} 的以下项是否已修复（项目根 D:/data/learn/code/slTerminal）。
先读 ${VERIFY_FILE} 获取断言清单（与 Stage 验证同一标尺），用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
待复查项：${FAILED.join('、')}
返回 JSON：{ "allFixed": true/false, "failedItems": ["仍未通过项原文"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 're-verify', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

// agent() 未返回时返回 null——兜底保留原 failedItems（不丢修复现场）
const verifyResult = rawVerify ?? { allFixed: false, failedItems: FAILED.slice(), details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 're-verify agent 未返回（被跳过或 API 错误）' } } }

return { fixResult, testResult, verifyResult }
