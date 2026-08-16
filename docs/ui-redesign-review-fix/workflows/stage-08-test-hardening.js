// =====================================================================
// Stage 08 测试质量强化（TE-01~06）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传：
//   「本 Stage 只改测试与 e2e 辅助代码（src/__tests__/、e2e-tests/），禁止改 src/ 生产代码」
// 契约（写死）：waitForPanelTabIcon → waitForPanelTabStatus（签名不变：
//   (panelId, status, timeout?) —— status 为 AgentStatus | null）
// 门禁注意：e2e-tests/ 不在根 tsconfig include——本 Stage 门禁补 npm run e2e
//   （含 build:e2e，约 2-3 分钟，勿中止）
// =====================================================================

export const meta = {
  name: 'fix-stage08-test-hardening',
  description: 'Stage 08 测试质量强化：waitForPanelTabStatus 更名 + 假守卫断言强化 + 测试数据口径对齐',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/ui-redesign-review-fix/checklist.md 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】本 Stage 只改测试与 e2e 辅助代码（src/__tests__/、e2e-tests/），禁止改 src/ 生产代码。
并行纪律：不跑资源共享型测试——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  { label: "navtree-test", prompt: `你负责 TE-01：
【TE-01】src/__tests__/nav-tree.test.tsx:605-618「父节点因子」用例假守卫：查询词 "Beta" 同命中项目名与页面名——改仅命中项目名的查询（先 Read 测试种子数据确定项目名/页面名实际值，选一个仅命中项目名的子串），断言页面行因父命中而仍渲染。` },
  { label: "navhist-test", prompt: `你负责 TE-02：
【TE-02】src/__tests__/nav-tree-history.test.tsx:289-294 重扫次数 toBeGreaterThanOrEqual(2) 过宽：改精确断言——展开前记录 mockScanHistory 调用次数，展开后断言次数严格增加精确值（先 Read 现用例上下文确定真实预期次数）。` },
  { label: "e2e-rename", prompt: `你负责 TE-03：
【TE-03】e2e-tests/specUtils.ts:250 的 waitForPanelTabIcon 更名 waitForPanelTabStatus（签名不变），函数内注释术语统一为 tabStatus/StatusDot；调用点同步更名：e2e-tests/mockcli.e2e.ts:39,255、e2e-tests/hooks.e2e.ts:23,118,134,204。
注意：e2e-tests/ 不在根 tsconfig include——你只需改名与注释，正确性由本 Stage 门禁的 npm run e2e 兜底。` },
  { label: "misc-test", prompt: `你负责 TE-04/06：
【TE-04】src/__tests__/sideBarState.test.ts:24-53 测试数据中的已退役视图 id "projects" 改 "nav"（或与生产默认不冲突的测试专用 id），含相关注释口径。
【TE-06】src/__tests__/workspace-page-dockview.test.tsx:276-285 FileIcon 页签用例粒度补强：断言 svg 确为 FileIcon（特征：含类型色块 rect/path 结构或扩展名对应 fill 色——先 Read FileIcon.tsx 结构定断言锚点）；增反向用例：terminal 面板页签不渲染 FileIcon。` },
  { label: "actbar-test", prompt: `你负责 TE-05：
【TE-05】src/__tests__/activityBar.test.tsx:276-280,503-512,514-528 三处「不抛异常即通过」用例补强：补 dropIndicator DOM 状态 / 事件调用次数 / 清理后样式实断言（先 Read 现断言与 ActivityBar.tsx 实现就近补强）。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。npm run e2e 含 build:e2e + wdio 全量（约 2-3 分钟），勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-08.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 08 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
