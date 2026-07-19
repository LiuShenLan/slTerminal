// =====================================================================
// Stage 02: 前端基础设施（CV-FE-01 titleManager 后缀 / CV-FE-02 DEFAULT_ZONES）
// =====================================================================
// 跨边界契约（写死）：
//   - findExistingEditor(pageId, filePath, suffix?)：suffix 传入时仅匹配同 suffix 条目；
//     不传时仅匹配无 suffix 条目（ExplorerPanel 现有调用零改动）
//   - 后缀值: "(git add)" / "(git not add)" / "(git delete)" / "(git diff)"
//   - DEFAULT_ZONES.top = ["projects", "explorer", "commit"]
// fix-loop 调用时 args.constraints 传 ""
// =====================================================================

export const meta = {
  name: 'stage02-fe-infra',
  description: 'Stage 02: titleManager 标题后缀支持 + commit 按钮默认归属（CV-FE-01/02）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
并行纪律：禁止跑 npm test 全量（多 agent 并发 vitest 会争用资源）——真实测试由全量测试 agent 单点跑；你只写代码与测试文件，不执行。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "title-suffix",
    prompt: `你负责 CV-FE-01：titleManager 标题后缀支持
文件：src/workspace/titleManager.ts、src/__tests__/titleManager.test.ts（只允许动这两个文件）

要点：
1. EditorEntry 增加 suffix?: string（注释：git 页签后缀，如 "(git diff)"，无后缀为普通编辑器）
2. registerEditor(pageId, panelId, filePath?, suffix?) —— 追加可选第 4 参，存入 entry
3. getFileEditorTitle(pageId, rootPath, filePath, suffix?) —— 返回标题末尾拼接 suffix（无冲突 basename+suffix；冲突 relPath+suffix）
4. recomputeTitles —— 每个 entry 重算后标题末尾拼接其 entry.suffix（无 suffix 不拼）
5. findExistingEditor(pageId, filePath, suffix?) —— 契约语义：suffix 传入时仅匹配 entry.suffix === suffix 的条目；不传时仅匹配无 suffix 的条目（保证 ExplorerPanel 现有调用 findExistingEditor(pageId, filePath) 只匹配普通编辑器，与 git 页签互不误聚焦）
6. handleSaveAs 不动

L2 测试（titleManager.test.ts 追加 describe）：
- suffix 标题生成：无冲突 "a.ts(git diff)"；有冲突 "src/a.ts(git diff)"
- recomputeTitles 保留各 entry 后缀（混合：普通编辑器 + git 页签同名场景）
- findExistingEditor：带 suffix 匹配同 suffix 条目；不带 suffix 不匹配带 suffix 条目；带 suffix 不匹配无 suffix 条目
- 现有 29 用例不受影响（不传 suffix 的旧调用签名兼容）`,
  },
  {
    label: "sidebar-default",
    prompt: `你负责 CV-FE-02：commit 按钮默认归属
文件：src/features/sideViews/sideBarState.ts 的 DEFAULT_ZONES + 受影响的测试文件

要点：
1. 先 Grep "DEFAULT_ZONES" 全仓，列出全部消费方与断言位置
2. sideBarState.ts：DEFAULT_ZONES.top 改为 ["projects", "explorer", "commit"]（注释同步更新）
3. 逐个更新受影响测试断言（预计：src/__tests__/sideBarState.test.ts、src/__tests__/sideBar.test.ts、src/__tests__/workspace-sideviews.test.tsx，以实际 grep 命中为准）
4. 注意 sanitizeSideBar 非对象回退默认值的断言也要同步
5. 不动 reconcileZones 逻辑（R9 对老用户持久化数据自动追加 commit，无需改）`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
