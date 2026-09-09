// =====================================================================
// Stage 04 · 前端清理（FE-01/02/07/09/10/11）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行；测试命令逐条串行。
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 04 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-04-frontend-cleanup',
  description: 'S04 前端清理：fileTree 竞态 + PANEL_SETTINGS 常量 + dock 卸载清理 + PageDockviewHost 改名 + shortcuts 退役面',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
Stage 特殊纪律：PageDockviewHost 改名只走 git mv（保历史）；knip.json:211 ignore 键同步改名（漏改 knip 必红）。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const agents = [
  { label: 'file-tree', prompt: `你负责 FE-01 + FE-07（先读 checklist 两条目六段式原文——两条联动闭环，一并改 src/features/explorer/useFileTree.ts）：
【FE-01】rootPath effect（:472-474 后、loadRoot(gen) 前）加 restoringRef.current = true;；restoreExpanded（:444-455）三分支收口（无快照/域不符/空集 → restoringRef=false + commitViewState()），deps 加 commitViewState。
【FE-07】loadRoot 加 firstFrameCommitted 局部旗标（首帧 setRootNodes 后置 true）；catch 双分支——续页失败 console.error 后 return（不记 dirErrors 不清空，首帧保留；抑制经 restoreExpanded 路径解除）；首帧失败维持现状 + restoringRef.current = false（gen 匹配时，FE-01 联动）。
测试同步：src/__tests__/use-file-tree.test.ts 按 checklist 两条目测试同步节补用例；src/features/explorer/CLAUDE.md 按文档同步节登记。
自验：npx vitest run use-file-tree 绿。` },
  { label: 'panel-settings', prompt: `你负责 FE-02（先读 checklist FE-02 六段式原文）：
src/panels/panelRegistry.ts：:23 后加 export const PANEL_SETTINGS = "settings" as const;；:86/:118 两处字面量替换为 PANEL_SETTINGS。
收窄纪律（checklist 已写死）：组件映射表 :73 键与 pageApis.ts 不动（对象键字面量是一致形态）——只改上述三处，勿扩面。
自验：npx tsc --noEmit 绿 + checklist FE-02 验证节 grep 断言过。` },
  { label: 'dock-host', prompt: `你负责 FE-09 + FE-10（先读 checklist 两条目六段式原文）：
【FE-09】src/workspace/WorkspaceDockHost.tsx 加 disposablesRef + handleReady（:265 后）赋值 + 组件级卸载 useEffect 消费（for dispose + 清空）；src/__tests__/workspace-host-pages.test.tsx 新增用例（unmount 后 __dockviewApi===undefined + getPageApi null）。
【FE-10】git mv src/workspace/PageDockviewHost.tsx src/workspace/tabChrome.tsx（只走 git mv 保历史）；文件头注改写照抄 checklist；五处 import 替换（WorkspaceDockHost.tsx:37、Workspace.tsx:185、terminal-rename-apply.test.ts:9、workspace-defaulttab.test.tsx:33、workspace-header-actions.test.tsx:43）；四处注释引用替换（src/lib/panelId.ts:7——顺带修正为 pageGroups.ts、src/workspace/tabClose.ts:5、src/theme/schemes/types.ts:234、src/theme/schemes/linear.ts:193）；两处测试头注（terminal-rename-apply.test.ts:3、workspace-defaulttab.test.tsx:3）；**knip.json:211 ignore 键同步改 src/workspace/tabChrome.tsx（漏改 knip 必红）**。
自验：rg "PageDockviewHost" src/ knip.json 零命中；npx vitest run terminal-rename-apply workspace-defaulttab workspace-header-actions workspace-host-pages 绿。` },
  { label: 'shortcuts-retire', prompt: `你负责 FE-11（先读 checklist FE-11 六段式原文）：
删除退役面：src/features/shortcuts/ShortcutRegistry.ts:155-164（exportContextBindings 家族）、src/features/shortcuts/types.ts:67-71+93-94、src/features/shortcuts/index.ts:11。
src/features/shortcuts/CLAUDE.md 前向接口节删行 + 测试模式节删列举词；src/__tests__/shortcuts.test.ts:669-730 甄别（exportContextBindings 用例删、listCommands 用例保留，describe 改名）。
mock 边界盲区纪律：src/__tests__/html-panel.test.tsx:88-92 与 markdown-panel.test.tsx:102 的 mock 区——先 grep 确认生产零消费该家族，确认后整个 vi.mock 块删；有消费则保留并在报告说明。
自验：rg "exportContextBindings|ExportedBinding" src/ 零命中（CLAUDE.md 残留也清）；npx vitest run shortcuts html-panel markdown-panel 绿。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行；纯前端 Stage 收窄门禁 + knip）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行）：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx knip --production
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。npm test 报告最终统计行原文；knip 报告未用项清单原文（对照基线判断是否新增红）。
（本 Stage 零 Rust 改动，收窄不跑 clippy/fmt/L1——收窄理由已登记 stages.md。）
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
