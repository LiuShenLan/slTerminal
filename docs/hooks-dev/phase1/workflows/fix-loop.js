// =====================================================================
// Fix Workflow: Phase 1 修复循环
// =====================================================================
// 用法: Workflow({ scriptPath: "docs/hooks-dev/phase1/workflows/fix-loop.js", args: {...} })
//
// args 强制校验：
//   stage: number
//   failedItems: string[] (非空)
//   fixContext?: string
//   verifyFile: string (必填，如 docs/hooks-dev/phase1/workflows/verify/stage-01.md)
//   constraints?: string
// =====================================================================

export const meta = {
  name: 'fix-loop',
  description: 'Phase 1 修复循环：针对 Stage 验证失败项修复并重验',
  phases: [
    { title: '修复问题' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

if (!args || !Array.isArray(args.failedItems) || args.failedItems.length === 0) {
  throw new Error('fix-loop 需要 args.failedItems（非空数组）')
}
if (typeof args.verifyFile !== 'string' || args.verifyFile.length === 0) {
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/hooks-dev/phase1/workflows/verify/stage-01.md）')
}

const STAGE = args.stage ?? 0
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:\\data\\learn\\code\\slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/hooks-dev/phase1/checklist.md 中对应 ID 的修复要点 + docs/hooks-dev/phase1/stages.md 中 Stage ${STAGE} 的实现要点，再结合失败证据定位。
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文。
- 禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-stage${STAGE}` })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。命令按 Stage 选择：
- Stage 01: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1
- Stage 02: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml pty_env_injects -- --test-threads=1 && cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1
- Stage 03: npx tsc --noEmit && npx eslint src/ipc/hooks.ts src/ipc/index.ts src/lib/claudeStatus.ts && npm test ipc-hooks-contract claude-status
- Stage 04: npx tsc --noEmit && npx eslint src/panels/terminal src/workspace/PageDockviewHost.tsx && npm test
- Stage 05: npm run build:e2e && npm run wdio
- Stage 06: npx tsc --noEmit
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: FAILED.slice(), details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 're-verify agent 未返回（被跳过或 API 错误）' } } }

return { fixResult, testResult, verifyResult }
