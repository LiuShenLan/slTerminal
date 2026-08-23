// Stage 03：L2 替身/复制脱节 + testid 微改（TQ-A-04, TQ-A-05, TQ-B-01, TQ-B-05, TQ-B-11, TQ-B-13, TQ-B-17）
// fix-loop 调用时 args.constraints 传：「生产文件微改仅限加 export / 加 data-testid / 抽函数原样移动，禁止逻辑改动」
export const meta = {
  name: 'stage-03-l2-decouple',
  description: 'Stage 03：L2 替身脱节修复 + 生产 testid/export 微改（7 项）',
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
【Stage 特殊纪律】生产文件微改仅限「加 export / 加 data-testid 属性 / 抽函数原样移动」，禁止任何逻辑改动。`

phase('并行修复')
const parallelAgents = [
  { label: 'fix-watermark', prompt: '你负责 TQ-A-04：PageDockviewHost.tsx 的 createWatermark 加 export（仅加 export）；workspace-header-actions.test.tsx 的 Watermark 回归 describe 改为渲染生产组件（手写组件删除），W1/W2/W3 断言主体不变。触碰：src/workspace/PageDockviewHost.tsx, src/__tests__/workspace-header-actions.test.tsx。' },
  { label: 'fix-barrel-mocks', prompt: '你负责 TQ-A-05：use-xterm-error-toast.test.ts / diff-panel.test.tsx / diff-panel-stale-banner.test.tsx 三文件的 ../lib barrel mock 改 importOriginal 形态（范例 editor-confirm.test.ts:107-114）；若真实成员在 jsdom 初始化抛错则按 checklist 降级方案处理并留注释。只改这 3 个测试文件。' },
  { label: 'fix-explorer-testids', prompt: '你负责 TQ-B-01 + TQ-B-05 + TQ-B-17：FileTree.tsx 加 data-testid="tree-node-row"（TreeNodeRow 根 div）与 data-testid="explorer-inline-input"（两处内联 input，仅加属性）；explorer-virtualization.test.tsx 计数改 testid 驱动（StrictMode 双渲染问题按 checklist 处理）；explorer-crud-success.test.tsx 的 rowBackground 与 input 查询改 testid；explorer-delete.test.tsx 的 input 查询改 testid。触碰：src/features/explorer/FileTree.tsx, 及上述 3 个测试文件。' },
  { label: 'fix-viewer-registry', prompt: '你负责 TQ-B-11：FileViewerRegistry.ts 抽 registerDefaultViewers 导出函数（原样移动注册语句，模块级调用保持原顺序）；file-viewer-registry.test.ts 删除私有复制体改 import 生产函数恢复。触碰：src/features/fileViewers/FileViewerRegistry.ts, src/__tests__/file-viewer-registry.test.ts。' },
  { label: 'fix-commit-menu', prompt: '你负责 TQ-B-13：CommitFileList.tsx 的 ContextMenu 根 div 加 data-testid="commit-context-menu"（仅加属性）；commit-context-menu-ui.test.tsx 的 getMenuEl 与无菜单断言改 testid 查询。触碰：src/features/commit/CommitFileList.tsx, src/__tests__/commit-context-menu-ui.test.tsx。' },
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
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
「生产微改边界」断言须用 git diff 逐文件 Read 确认生产文件仅含 export/testid/抽函数原样移动。
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
