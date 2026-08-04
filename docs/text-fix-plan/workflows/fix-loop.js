// =====================================================================
// Fix Workflow — 阶段验证不通过时的修复循环
// =====================================================================
// 由主 agent 调用：Workflow({ scriptPath, args: { stage, failedItems, fixContext, verifyFile, constraints } })
// args 说明:
//   stage: number      — 所属 Stage 编号（1-17，决定门禁测试命令集合，照 execution-plan Stage 表）
//   failedItems: string[] — 未通过项 ID 列表（必填，非空；来自 Stage verify）
//   fixContext: string — verify agent 给出的 details 证据原文（失败原因线索）
//   verifyFile: string — 断言清单文件路径（必填，如 docs/text-fix-plan/workflows/verify/stage-01.md）
//                        ——与 Stage 脚本同一真值源，保证修复循环与 Stage 验证同一标尺
//   constraints: string — Stage 特殊纪律（可选；值见对应 Stage 脚本头注释 "fix-loop constraints:" 行）
// 调用上限 3 轮；3 轮未全绿 → 上报用户人工介入，不继续空转。
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
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/text-fix-plan/workflows/verify/stage-01.md）')
}

const STAGE = args.stage ?? 0
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''

// 门禁测试命令按 Stage 选择（照 execution-plan Stage 表，勿改写命令值——config.json commands 单点）
const L1_CMDS = `1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
const L2_CMDS = `1. npx tsc --noEmit
2. npx eslint src/
3. npm test`
const L3_CMDS = `1. npx tsc --noEmit
2. npm run test:l3`
const L4_CMDS = `1. npx tauri build --debug --no-bundle
2. npm run wdio`
const DOC_CMDS = `1. npm test
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

let testCmds
if (STAGE >= 1 && STAGE <= 5) testCmds = L1_CMDS
else if (STAGE >= 6 && STAGE <= 14) testCmds = L2_CMDS
else if (STAGE === 15) testCmds = L3_CMDS
else if (STAGE === 16) testCmds = L4_CMDS
else if (STAGE === 17) testCmds = DOC_CMDS
else testCmds = L2_CMDS // 未知 stage 默认 L2 门禁

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:\\data\\learn\\code\\slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/text-fix-plan/checklist.md 中对应 ID 的修复要点 + docs/text-fix-plan/stages.md 中 Stage ${STAGE} 的实现要点，再结合失败证据定位
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文
- 禁区：
  1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮
  2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）
  3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)
  4. cargo test 恒 --test-threads=1（ConPTY 并发 spawn 死锁）
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-stage${STAGE}` })

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
${testCmds}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止${STAGE === 16 ? '；E2E 全量较慢（单实例串行），耐心等待勿中止' : ''}。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（与 Stage 同一断言清单文件）===
phase('逐项验证')
const rawVerify = await agent(`
逐项复查 Stage ${STAGE} 的以下项是否已修复（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 ${VERIFY_FILE} 获取断言清单（与 Stage 验证同一标尺），用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
待复查项 ID：${FAILED.join('、')}
返回 JSON：{ "allFixed": true/false, "failedItems": ["仍未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
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
