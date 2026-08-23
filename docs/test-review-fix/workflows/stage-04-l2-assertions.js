// Stage 04：L2 断言强化 + 数据层 + 竞态（TQ-A-06, TQ-A-07, TQ-A-08, TQ-B-03, TQ-B-07, TQ-B-08, TQ-B-12, TQ-B-16, TQ-C-01, TQ-C-02, TQ-C-03, TQ-C-04）
// fix-loop 调用时 args.constraints 传：「只改测试与测试 helper；生产微改仅限加 export（usePtyOutput.ts / overrides.ts 如需）」
export const meta = {
  name: 'stage-04-l2-assertions',
  description: 'Stage 04：L2 断言强化 + 数据层 + 竞态（12 项）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。
【Stage 特殊纪律】只改测试与测试 helper；生产微改仅限「加 export」（usePtyOutput.ts 五常量 / overrides.ts 如需补导出），禁止任何逻辑改动。`

phase('并行修复')
const parallelAgents = [
  { label: 'fix-thresholds-docs', prompt: '你负责 TQ-A-06 + TQ-A-07 + TQ-C-03：usePtyOutput.ts:20-29 五常量（DIRECT_WRITE_THRESHOLD/IDLE_FLUSH_MS/MAX_FLUSH_MS/MAX_PENDING_BYTES/E2E_BUFFER_MAX_LINES）加 export（仅加 export）；use-xterm-output.test.ts 魔数改 import 生产常量；workspace-sideviews.test.tsx 的 46px 活动栏宽度等魔数断言按 checklist TQ-A-07 处理；scheme-registry.test.ts 的 linear 标量数 26 改 27（或改计数驱动断言，按 checklist）；src/theme/CLAUDE.md:77 的 26 改 27。触碰：src/panels/terminal/usePtyOutput.ts, src/__tests__/use-xterm-output.test.ts, workspace-sideviews.test.tsx, scheme-registry.test.ts, src/theme/CLAUDE.md。' },
  { label: 'fix-assertions', prompt: '你负责 TQ-A-08 + TQ-C-01：use-code-mirror.test.ts:156-172 的 toBeDefined 弱断言按 checklist 强化为行为断言；editor-confirm.test.ts:258-316 弱断言段同样强化；overrides.test.ts 的 themeRules 断言改消费生产提取结果（overrides.ts 提取方式先 Read 定，如需补导出仅加 export）。触碰：src/__tests__/use-code-mirror.test.ts, editor-confirm.test.ts, overrides.test.ts, src/theme/overrides.ts（如需补导出）。' },
  { label: 'fix-race-keybindings', prompt: '你负责 TQ-B-03 + TQ-B-16：explorer-sandbox-race.test.tsx 用既有 deferred 基建（resetDeferred/mockSetProjectRoot/resolveSetProjectRoot/sprCallOrder/rdCallOrder）构造真竞态用例——先观察实际行为再定断言，禁止放宽到时序无约束；wire-keybindings.test.ts 按 checklist TQ-B-16 补断言。只改 src/__tests__/explorer-sandbox-race.test.tsx, wire-keybindings.test.ts。' },
  { label: 'fix-drop-contract-scan', prompt: '你负责 TQ-B-07 + TQ-C-02 + TQ-C-04：activityBar.test.tsx 三槽断言补强；drop-target.test.ts（可能新建）按 checklist 补 drop 契约用例；ipc-agent-history-contract.test.ts 用 ipc-contract.ts 既有 mockThrow/expectReject 补 SEC-05 负例（payload 键集合精确匹配）；no-claude-literals.test.ts 扫描范围扩全 src——若命中既有违例须报告，禁止默默加豁免。只改 src/__tests__/activityBar.test.tsx, drop-target.test.ts, ipc-agent-history-contract.test.ts, no-claude-literals.test.ts。' },
  { label: 'fix-focus-keyboard', prompt: '你负责 TQ-B-08 + TQ-B-12：explorer-delete.test.tsx 焦点断言强化（与 TQ-B-17 的 testid 改造已在他 Stage 处理，本项只做断言强化）；新建 src/__tests__/helpers/keyboard.ts 键盘事件共享 helper，global-commands.test.ts 与 shortcuts.test.ts 的重复构造改调 helper；src/__tests__/CLAUDE.md 补 helper 登记句。触碰：src/__tests__/explorer-delete.test.tsx, global-commands.test.ts, shortcuts.test.ts, src/__tests__/helpers/keyboard.ts（新建）, src/__tests__/CLAUDE.md。' },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
「生产微改边界」断言须用 git diff 逐文件 Read 确认生产文件仅含 export。
TQ-C-04 扫描扩全 src 的命中处置：若修复 agent 报告了既有违例命中，核对报告中是否如实列出（禁止默默加豁免）。
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
