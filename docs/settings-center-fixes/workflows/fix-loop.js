// =====================================================================
// Fix Workflow — settings-center-fixes 阶段验证不通过时的修复循环
// =====================================================================
// 由主 agent 循环调用（最多 3 轮）：
//   Workflow({ scriptPath, args: { stage, failedItems, fixContext, verifyFile, constraints } })
//
// args 说明（取值单点定义于此，execution-plan.md 只引用不复制）：
//   stage: number      — 所属 Stage 编号（1-5）
//   failedItems: string[] — 未通过项 ID 列表（必填，非空；来自 Stage verify）
//   fixContext: string — verify agent 给出的 details 证据原文（失败原因线索）
//   verifyFile: string — 断言清单文件路径（必填，
//                        如 docs/settings-center-fixes/workflows/verify/stage-01.md）
//                        ——与 Stage 脚本同一真值源，保证修复循环与 Stage 验证同一标尺
//   constraints: string — Stage 特殊纪律（可选；Stage 04 传
//                        「本 Stage 只改测试，禁止改生产代码」，
//                        Stage 05 传「本 Stage 只改文档/注释，禁止改代码逻辑」，
//                        其余 Stage 省略）

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
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/settings-center-fixes/workflows/verify/stage-01.md）')
}

const STAGE = args.stage ?? 0
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''

// Stage 门禁命令（与对应 stage 脚本一致——03 含 e2e 构建+实跑、05 含全量四级回归）
const TEST_COMMANDS_BY_STAGE = {
  1: `1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`,
  2: `1. npx tsc --noEmit
2. npx eslint src/
3. npm test`,
  3: `组一（并行）：1. npx tsc --noEmit  2. npx eslint src/  3. npm test
组二（组一完成后串行）：4. npm run build:e2e  5. npx wdio run wdio.conf.ts --spec e2e-tests/settings.e2e.ts`,
  4: `1. npx tsc --noEmit
2. npx eslint src/
3. npm test`,
  5: `组一（并行）：1. npx tsc --noEmit  2. npx eslint src/  3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings  4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check  5. npm test  6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1  7. npm run test:l3
组二（组一完成后串行）：8. npm run e2e`,
}
const TEST_COMMANDS = TEST_COMMANDS_BY_STAGE[STAGE] ?? TEST_COMMANDS_BY_STAGE[2]

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:\\data\\learn\\code\\slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/settings-center-fixes/checklist.md 中对应 ID 的修复要点 + docs/settings-center-fixes/stages.md 中 Stage ${STAGE} 的实现要点，再结合失败证据定位
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文
- 禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）；禁止前端 src/ipc/ 外出现 invoke；禁止硬编码颜色（经 theme/colors.ts token）；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
- 你不跑全量测试（统一由后续测试 agent 单点跑）
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-stage${STAGE}` })

// === Phase 2: 全量测试（命令与对应 Stage 门禁一致）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。命令如下（分组标注时组内并行、组间串行）：
${TEST_COMMANDS}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队、wdio/e2e 耗时较长，均属正常勿中止。
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
