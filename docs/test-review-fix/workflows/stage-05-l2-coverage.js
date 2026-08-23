// Stage 05：L2 前端覆盖缺口补写（TQ-COV-07, TQ-COV-08, TQ-COV-09, TQ-COV-10）
// fix-loop 调用时 args.constraints 传：「只改测试与测试 helper」
export const meta = {
  name: 'stage-05-l2-coverage',
  description: 'Stage 05：L2 前端覆盖缺口补写（4 大项）',
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
本 Stage 只改测试文件（src/__tests__/ 下），禁止改生产代码。
补测前先跑 npm run test:coverage 取各文件未覆盖分支清单再补测；每例断言用户可见行为（toast/console/DOM），不只「不 throw」。`

phase('并行修复')
const parallelAgents = [
  { label: 'cov-terminal-hooks', prompt: '你负责 TQ-COV-07：终端 hooks 未覆盖分支补测——useTerminalInstance 错误/清理分支、useXterm 系错误分支。按归属追加到既有测试文件（src/__tests__/terminal-instance.test.ts, use-xterm-*.test.ts），不新建文件。分支清单以 npm run test:coverage 报告为准。' },
  { label: 'cov-dockview-host', prompt: '你负责 TQ-COV-08：PageDockviewHost.tsx（07 报告误写为 DockviewHost.tsx，已翻案留痕）真实覆盖补测——以 v8 报告该文件未覆盖行为准，追加到 src/__tests__/workspace-page-dockview.test.tsx。' },
  { label: 'cov-nav-explorer', prompt: '你负责 TQ-COV-09：NavPageRow 与 ExplorerPanel 未覆盖分支补测——nav-page-row.test.tsx（新建或并入 nav-tree.test.tsx，按 checklist 决策）与 explorer 相关测试文件。分支清单以 coverage 报告为准。' },
  { label: 'cov-lowrisk-pack', prompt: '你负责 TQ-COV-10：低危打包项补测——ipc-window.test.ts（新建）、sideViewDefs（新建或并入 sideBar 系）、nav-history-row.test.tsx 追加、gitshow-panel.test.tsx 追加等，逐项以 checklist TQ-COV-10 清单为准。' },
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
4. npm run test:coverage（复测对照——报告前端行覆盖率总数值，及 TQ-COV-07~10 目标文件的行/分支覆盖率变化）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由；补测用例若只有「不 throw」式断言而无用户可见行为断言，判 partial。
以下为测试 agent 的全量测试执行结果（含 coverage 复测数值），测试类断言据此判定（无需重跑）：
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
