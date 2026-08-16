// =====================================================================
// Stage 05 — 页签栏改造（TAB-01~TAB-05）
// 契约：dockview 页签底色/文字色已由 Stage 01 变量注入（激活 #0a0a0b/未激活 transparent），
//       本 Stage 只做变量覆盖不到的自绘部分（指示条/hover 关闭钮/构成/+钮）
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage05-tabs',
  description: 'Stage 05: 扁平页签 + 底部 2px 指示条 + hover 关闭钮 + 圆点logo构成',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "tab-ui", prompt: `你负责 TAB-01/TAB-02/TAB-03/TAB-04（全部在 src/workspace/PageDockviewHost.tsx，先 Read DefaultTab 与 RightHeader 现状再动手）：
【TAB-01】DefaultTab 扁平化（UI-401）：激活页签底部 2px 指示条——自绘绝对定位底条或 borderBottom，色 FOCUS_BORDER token（#6e9ff2）；未激活透明底 fg-3 文字（dockview 变量已注入，核对 DOM 实际类名与变量生效形态）；hover 仅文字变 fg-1 不变底。
【TAB-02】关闭 ×（UI-402）：默认不可见，hover 页签时出现（CSS :hover 控制 opacity/visibility 或条件渲染——执行期定，写入注释）；14px、圆角 4px、自身 hover 底 #2b2b31；激活页签同样不常驻。
【TAB-03】页签构成（UI-403）：终端/agent 页签 = StatusDot（tabStatus，Stage 03 已接入）+ CLI logo（tabLogo img 分支不动）+ 名称；文件型页签（editor/htmlviewer/gitshow/diff——panelRegistry 的 FILE_PANEL_TYPES）= FileIcon 彩色图标 + 名称（按 panel 文件路径扩展名取图标——先 Read panelRegistry 与 DefaultTab params 确认文件路径来源，gitshow/diff 页签标题含 suffix 不影响图标）；确认无 emoji 渲染分支残留。
【TAB-04】RightHeader「+」钮（UI-404）：22px、圆角 4px、fg-3（PLACEHOLDER_FG）、hover 底 #222227（ui.secondaryBg token）。` },
  { label: "tab-test", prompt: `你负责 TAB-05（只改测试）：
页签相关 L2 测试同步——src/__tests__/workspace-header-actions.test.tsx、workspace-page-dockview.test.tsx 及 grep tabIcon/tabStatus/关闭钮/默认页签 于 src/__tests__/ 命中的其它文件：断言适配新形态（tabStatus→StatusDot 存在性、底部指示条存在性、关闭钮 hover 才显、文件页签 FileIcon 存在、「+」钮尺寸规格）；先 Read tab-ui 改动后的 PageDockviewHost.tsx 再定断言形态（允许等待——若文件尚无改动，按 stages.md Stage 05 实现要点写）。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
