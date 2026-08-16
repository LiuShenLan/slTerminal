// =====================================================================
// Stage 01 浮层收尾（FE-01/02/03/09/24）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// 跨 agent 契约（写死，执行 agent 不各自推断）：
//   1. confirmDialog/toast API（既有）：confirmDialog(opts): Promise<boolean>
//      （确认 true / 取消·Esc·遮罩 false）、toast.show("success"|"warning"|"error", message)；
//      危险确认传 danger: true；均 import 自 src/lib barrel
//   2. pageApis 新导出签名：findPanelForSession(cliId: string, sessionId: string): string | undefined、
//      findPageIdForPanelId(panelId: string): string | null
// =====================================================================

export const meta = {
  name: 'fix-stage01-overlay-cleanup',
  description: 'Stage 01 浮层收尾：原生 alert/confirm 清零 + 会话反查上提 pageApis',
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
并行纪律：不跑资源共享型测试（PTY/端口/全局锁类）——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  { label: "editor-overlay", prompt: `你负责 FE-01：
【FE-01】src/panels/editor/useCodeMirror.ts 原生弹窗 3 处：
- :177 保存失败 window.alert 改 toast.show("error", ...)（纯通知）
- :265 外部修改净文件重载确认 window.confirm 改 await confirmDialog({...})（确认=继续/取消=中止，语义对照现状；调用处改 async）
- :392 脏文件确认 window.confirm 同上
测试同步：src/__tests__/use-code-mirror.test.ts 与 editor-confirm.test.ts 中相关用例改 mock confirmDialog/toast（不再 mock window.alert/confirm），断言调用参数。` },
  { label: "diff-overlay", prompt: `你负责 FE-02：
【FE-02】src/panels/diff/DiffPanel.tsx:363,457 两处 window.confirm 改 await confirmDialog({...})（确认=继续/取消=中止，语义对照现状；调用处改 async）。
测试同步：src/__tests__/diff-panel.test.tsx 脏弹窗分支改 mock confirmDialog。` },
  { label: "navtree-overlay", prompt: `你负责 FE-03/09/24：
【FE-03】src/features/navTree/NavTree.tsx:565-569 项目删除 window.confirm 改 confirmDialog({ title, message, danger: true })（action 改 async）。src/__tests__/nav-tree.test.tsx 删除项目用例改 mock confirmDialog。
【FE-09】反查函数上提：NavTree.tsx:107-139 的 findPanelForSession/findPageIdForPanelId 逐字搬运至 src/workspace/pageApis.ts 并导出（签名：findPanelForSession(cliId: string, sessionId: string): string | undefined、findPageIdForPanelId(panelId: string): string | null；依赖 TerminalRegistry/useProjects/parseTerminalPageId/keyOf/basename 随函数一并 import）；NavTree 删 :56 的 TerminalRegistry import 与本地实现，调用点 :325,333,340 改调 pageApis。行为零变化（复合键 keyOf 匹配、usageSourcePath 回退、B14 前缀匹配优先 + parseTerminalPageId 兜底）。src/__tests__/pageapis.test.ts 增两函数全分支用例（复合键命中/usageSourcePath 回退/未命中；前缀匹配/parse 兜底/null）；nav-tree.test.tsx 相关 mock 同步。
【FE-24】NavTree.tsx:309 handleNewPage 删返回值（调用方 :560 不消费 pageId）。` },
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
