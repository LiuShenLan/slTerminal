// =====================================================================
// Stage 08 — 全局收敛（GL-01~GL-06）
// 划分豁免：本 Stage 仅 2 agent 且串行——全仓字号/圆角兜底扫描的文件集合事前不可枚举，
//   并行无法证明文件零重叠，接受串行时长（stages.md 已注明）
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage08-global',
  description: 'Stage 08: 滚动条/焦点环/圆角/密度/空态/字号字重全仓收敛',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 串行重构（扫描面重叠，顺序执行）===
phase('串行重构')
const sequentialAgents = [
  { label: "global-css", prompt: `你负责 GL-01/GL-02/GL-06：
【GL-01】src/App.css 新增全局滚动条规范（UI-807）：::-webkit-scrollbar 宽/高 9px、::-webkit-scrollbar-track 透明、::-webkit-scrollbar-thumb rgba(255,255,255,0.10) 圆角 5px、hover 0.20、:active 0.28、无箭头（::-webkit-scrollbar-button 隐藏）；注释注明 dockview/terminal 滚动条变量已由方案注入（本规则兜底原生滚动容器）。
【GL-02】src/App.css 新增 :focus-visible { outline: 1px solid #6e9ff2 }（值经 var() 不可行——ROOT_CSS_VARS 仅两键，此处硬编码并注释「accent 定值，与 linear.focusBorder 同步」）；审计全仓组件级 outline:"none"/outline: "none"（grep 枚举：FileTree/ExplorerPanel/HandlerForm/TerminalRenameDialog/ActivityBar 等约 10 处）——鼠标点击不显焦点环保留 outline none（配 :focus-visible 全局规则即可），键盘可达交互元素确认全局规则能生效；逐处判断写入报告。
【GL-06】FT-05/06/07 兜底：grep fontSize/font-weight/fontWeight 全仓 src/（排除测试），阶梯外字号（非 11/11.5/12/12.5/13px）与字重（非 400/500）逐处收敛（终端/编辑器内部行高计算等语义性数值除外——逐处判断写理由）；分组标题（EXPLORER/COMMIT/导航 等区块头）确认 11px+全大写+0.08em+fg-3。` },
  { label: "density-empty", prompt: `你负责 GL-03/GL-04/GL-05（前序 global-css 已完成，字号/字重不要再碰）：
【GL-03】圆角收敛（UI-306）：grep borderRadius/border-radius 全仓 src/（排除测试），阶梯外值收敛至 4/5/6/8/pill（页签类 0）——逐处按组件语义定档（小控件 4/行 5/按钮 6/输入框与浮层 8/徽标 pill）写入报告。
【GL-04】密度与间距（UI-304/305）：核对活动栏 46/页签栏 35/树行 28/会话行 30 已生效（前序 Stage 产出，只读核对）；组件间距值收敛 4/8/12/16/24（grep padding/margin 明显例外值，逐处判断——面板内容区内边距等语义值除外写理由）；dockview sash 视觉 1px 发丝线已由方案变量注入（只读核对 --dv-separator-border），拖拽热区 ≥4px 实测确认（人工验证点）。
【GL-05】空态统一（UI-806）：src/workspace/PageDockviewHost.tsx createWatermark（67-100 行附近）与 src/features/navTree/ 空态（无历史/无搜索）与 src/features/explorer/ExplorerPanel.tsx 空文件树——统一形态：居中 15px 线性图标 fg-4（icons.tsx IconEmptyBox）+ 说明文字 fg-3 + 可选次按钮（SECONDARY_BG 底 SIDEBAR_FG 字）；Watermark 既有按钮行为（addPanel）不变。
同步更新受影响测试。` },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  sequentialResults.push(r)
}

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
逐项检查 Stage 08 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { sequentialResults, testResult, verifyResult }
