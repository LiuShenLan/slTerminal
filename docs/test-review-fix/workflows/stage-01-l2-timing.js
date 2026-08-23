// Stage 01：L2 实证 flaky + 异步等待（TQ-A-01, TQ-B-04, TQ-B-06, TQ-B-09, TQ-B-18, TQ-B-19）
// fix-loop 调用时 args.constraints 传：「本 Stage 只改测试文件，禁止改生产代码」
export const meta = {
  name: 'stage-01-l2-timing',
  description: 'Stage 01：L2 实证 flaky + 异步等待修复（6 项）',
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
本 Stage 只改测试文件（src/__tests__/ 下），禁止改生产代码。`

phase('并行修复')
const parallelAgents = [
  { label: 'fix-diff-panel', prompt: '你负责 TQ-A-01：diff-panel.test.tsx 脏态弹窗用例在 dispatch 前未等 CM6 初始化（实证 1/8 偶发失败）。按 checklist TQ-A-01 步骤修：getDiffView 非空 waitFor 前置；同文件其他 getDiffView(...)! 后紧跟 dispatch 的用例同样处理。只改 src/__tests__/diff-panel.test.tsx。' },
  { label: 'fix-explorer-timing', prompt: '你负责 TQ-B-04 + TQ-B-18：explorer-race-cleanup.test.tsx G3 的 advanceTimersByTimeAsync(0) 时机假设改为 vi.waitFor 等初始加载完成；explorer-refresh-preserve.test.tsx R17 gitStatus 改可控 resolved + 终态断言改 waitFor。注意 fake timers 下用 vi.waitFor。只改这两个测试文件。' },
  { label: 'fix-commit-status', prompt: '你负责 TQ-B-06：commit-view-status.test.ts 旧请求丢弃用例（:233-294 区域）resolveOld 后改 vi.waitFor 轮询确认渲染稳定，再断言旧数据不落地。只改 src/__tests__/commit-view-status.test.ts。' },
  { label: 'fix-navtree-waitfor', prompt: '你负责 TQ-B-09：nav-tree.test.tsx 三段（:410-452/:655-714/:876-933 附近）与 nav-tree-history.test.tsx 对应段（:195-250 附近）中，fireEvent 后紧跟的同步 style/textContent 断言全部包入 waitFor。只改这两个测试文件。' },
  { label: 'fix-keyboard-mock', prompt: '你负责 TQ-B-19：keyboard.test.ts 的 readTextMock/writeTextMock 由 mockClear 改 mockReset 并补默认 mockResolvedValue。只改 src/__tests__/keyboard.test.ts。' },
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
另补跑一条（TQ-A-01 实证 flaky 回归标尺，串行执行）：按 docs/test-review/02-l2-workspace-panels.md 复跑段的 17 文件清单 npx vitest run <17 文件>，连续 3 轮，逐轮报告通过/失败。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
