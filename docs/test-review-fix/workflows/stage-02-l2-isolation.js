// Stage 02：L2 隔离性/全局污染（TQ-A-02, TQ-A-03, TQ-B-02, TQ-B-10, TQ-B-14, TQ-B-15）
// fix-loop 调用时 args.constraints 传：「本 Stage 只改测试与测试 helper，禁止改生产代码」
export const meta = {
  name: 'stage-02-l2-isolation',
  description: 'Stage 02：L2 隔离性/全局污染修复（6 项）',
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
本 Stage 只改测试文件与测试 helper（src/__tests__/ 下），禁止改生产代码。`

phase('并行修复')
const parallelAgents = [
  { label: 'fix-global-stubs', prompt: '你负责 TQ-A-02：workspace.test.tsx / workspace-multi-instance.test.tsx / workspace-switch-order.test.tsx / workspace-page-dockview.test.tsx 四文件顶层 ResizeObserver stub 加 originalResizeObserver 保存 + afterAll 恢复；use-xterm-integration.test.ts 顶层 getContext spyOn 加 afterAll mockRestore。只改这 5 个测试文件。' },
  { label: 'fix-setup-geometry', prompt: '你负责 TQ-A-03：setup.ts 的 beforeAll 内补 Range.prototype.getClientRects stub（jsdom 缺失致 CM6 测量走异常回退刷 stderr）。按 checklist TQ-A-03 步骤，类型报错时用 unknown 强转形态。只改 src/__tests__/setup.ts。' },
  { label: 'fix-sideview-isolation', prompt: '你负责 TQ-B-02：sideBar.test.ts / sideBarArea.test.tsx / activityBar.test.tsx 三文件加 vi.mock("../features/sideViews/sideViewDefs", () => ({})) 阻断 side-effect 混入；beforeEach 的 _reset() 后补 getAll().length === 0 防御断言。只改这 3 个测试文件。' },
  { label: 'fix-store-reset', prompt: '你负责 TQ-B-10 + TQ-B-15：helpers/workspace-setup.ts 的 resetProjectStores 扩 useSideBar + useKeybindings 重置（import 路径先 Read 确认）；commit-view.test.tsx / nav-tree.test.tsx / explorer-crud-success.test.tsx 的 beforeEach 改为调用共享重置；commit-open-file.test.ts 与 explorer-crud-success.test.tsx 的 afterEach 补 delete __dockviewApi。只改这 5 个文件。' },
  { label: 'fix-profile-reset', prompt: '你负责 TQ-B-14：nav-history-row.test.tsx afterEach 补 CliProfileRegistry._reset()（注册表导出形态先 Read src/features/cliProfiles/index.ts 确认）。只改 src/__tests__/nav-history-row.test.tsx。' },
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
另：npm test 输出中不得再出现 getClientRects is not a function（TQ-A-03 判定依据），报告是否出现。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
