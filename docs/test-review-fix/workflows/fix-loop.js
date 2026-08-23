// =====================================================================
// Fix Loop（test-review-fix）— Stage 验证不通过时的修复循环
// =====================================================================
// 用法: 主 agent 调用：
//   Workflow({ scriptPath: 'docs/test-review-fix/workflows/fix-loop.js',
//              args: { stage, failedItems, fixContext, verifyFile, constraints } })
//
// args 说明:
//   stage: number      — 所属 Stage 编号（1-10，用于 label 与测试命令查表）
//   failedItems: string[] — 未通过项 ID 列表（必填，非空；来自 Stage verify）
//   fixContext: string — verify agent 给出的 details 证据原文（失败原因线索）
//   verifyFile: string — 断言清单文件路径（必填，docs/test-review-fix/workflows/verify/stage-NN.md）
//   constraints: string — Stage 特殊纪律（可选——值见对应 Stage 脚本头注释与 execution-plan.md fix-loop args 规范）
//
// 测试命令映射表与 docs/test-review-fix/execution-plan.md「Stage 表」一致（此处为脚本侧唯一定义处）。
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
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/test-review-fix/workflows/verify/stage-01.md）')
}

const STAGE = args.stage ?? 0
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''

// 各 Stage 门禁命令（与 execution-plan.md Stage 表一致；Stage 08 cargo 系串行，Stage 09 特殊）
const TEST_COMMANDS_BY_STAGE = {
  1: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test',
  2: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test',
  3: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test',
  4: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test',
  5: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test\n4. npm run test:coverage',
  6: '1. npx tsc --noEmit\n2. npx eslint src/\n3. npm test\n4. npm run test:l3',
  7: '1. npx vite build\n2. npm run e2e',
  8: 'cargo 系串行执行：\n1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings\n2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check\n3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程）',
  9: '1. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check\n2. ci.yml 语法解析校验（node + yaml/js-yaml）\n3. npm run e2e',
  10: '1. npx tsc --noEmit\n2. npx eslint src/\n3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings\n4. npm test\n5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1\n6. npm run test:l3\n7. npm run e2e',
}
const TEST_COMMANDS = TEST_COMMANDS_BY_STAGE[STAGE] ?? TEST_COMMANDS_BY_STAGE[10]

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:/data/learn/code/slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/test-review-fix/checklist.md 中对应 ID 的六段式条目 + docs/test-review-fix/stages.md 中 Stage ${STAGE} 的实现要点，再结合失败证据定位
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文
- 禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-stage${STAGE}` })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证（Stage ${STAGE} 门禁命令）：
${TEST_COMMANDS}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
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
