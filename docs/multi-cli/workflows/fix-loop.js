// =====================================================================
// Fix Workflow — multi-cli profile 重构修复循环
// =====================================================================
// 用法: 由主 agent 在 Stage verify 未通过时调用（最多 3 轮，config.json fixMaxRetries）：
//   Workflow({ scriptPath: 'docs/multi-cli/workflows/fix-loop.js',
//              args: { stage, failedItems, fixContext, verifyFile, constraints } })
//
// args 规范（execution-plan §4 原样）：
//   stage: number        — Stage 编号（1–8）
//   failedItems: string[] — verify agent 返回的 failedItems 原样透传（与 verify/stage-NN.md 同一真值源）
//   fixContext: string   — Stage 脚本头部的跨边界契约段（profile 接口/泛化命令/DTO/禁区）原样 + 本 Stage 实现要点
//   verifyFile: string   — docs/multi-cli/workflows/verify/stage-NN.md（与 Stage 脚本同一断言文件）
//   constraints: string  — stages.md「禁区」六条原样（ConPTY 0x7 / C10 / 轮询补漏 / SEC-05 / E2E 隔离 / E2E_ENABLED 内联）
//                          + Stage 特殊纪律（见对应 Stage 脚本头注释「fix-loop 调用约定」）
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
  throw new Error('fix-loop 需要 args.verifyFile（断言清单文件路径，如 docs/multi-cli/workflows/verify/stage-01.md）')
}

const STAGE = args.stage ?? 0
const FAILED = args.failedItems
const FIX_CONTEXT = args.fixContext ?? '（无附加上下文）'
const VERIFY_FILE = args.verifyFile
const CONSTRAINTS = args.constraints ?? ''

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
项目根目录 D:/data/learn/code/slTerminal。
Stage ${STAGE} 的逐项验证未通过，以下 ${FAILED.length} 项需要修复：
${FAILED.map((id, i) => `${i + 1}. ${id}`).join('\n')}

验证 agent 给出的失败证据：
${FIX_CONTEXT}

纪律：
- 先读 docs/multi-cli/checklist.md 中对应 ID 的修复要点 + docs/multi-cli/stages.md 中 Stage ${STAGE} 的实现要点，再结合失败证据定位
- 只修复上述列出的项，不顺手改无关代码；代码注释用中文
- 禁区（红线，触碰即返工）：
  1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
  2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——勿削弱
  3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
  4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
  5. E2E 不得触碰用户真实 ~/.claude/（env 覆盖 + fixture 隔离）
  6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）
${CONSTRAINTS ? `- 【Stage 特殊纪律】${CONSTRAINTS}` : ''}
- 测试纪律：你不跑资源共享型测试（PTY/端口/全局锁类）；只做编译级检查（npx tsc --noEmit / cargo check --manifest-path src-tauri/Cargo.toml）；真实执行由全量测试 agent 单点跑
- 若修复涉及用例增/删/更名，同步就近更新 .claude/test-inventory.md 对应行
完成后报告：每项的修复方式 + 修改的文件清单。
`, { label: `fix-stage${STAGE}` })

// === Phase 2: 全量测试（1-7 并行收集，8 最后单独串行）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。命令清单：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npm run test:l3
8. npm run e2e
执行纪律：命令 1-7 相互独立，并行启动执行，收集全部结果；待 1-7 全部结束后，再单独串行执行命令 8（npm run e2e 内部 = build:e2e + wdio 串行；它会重新构建并占用 slterminal.exe，与其他命令并行会构建失败——禁拆分、禁与其他命令并行）。
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
