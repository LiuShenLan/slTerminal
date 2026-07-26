// =====================================================================
// Stage 04 Workflow: 页签四态集成
// =====================================================================
// 契约头部：
//   - useCommandDetection: OSC 133 C -> attention emoji + title; D -> clear
//   - useXterm: subscribe hook-event, filter by panelId, map to emoji
//   - TerminalPanel: handleTabStateChange only set title/icon when provided
//   - DefaultTab: render emoji string tabIcon as span, URL/path as img
//   - tabRules: remove claude icon, keep title
// =====================================================================

export const meta = {
  name: 'stage04-frontend-integration',
  description: '页签四态指示集成',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/hooks-dev/phase1/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'fe-cmd',
    prompt: `你负责 P1-F3-01：
修改 src/panels/terminal/useCommandDetection.ts：
- OSC 133 C 匹配到规则时：onTabStateChange({ active: true, title: rule.title, icon: "🟡" })
- OSC 133 D 且 isCommandRunningRef.current 为 true 时：isCommandRunningRef.current = false; onTabStateChange({ active: false })
- 其余逻辑不变。完成后跑 npx tsc --noEmit。`
  },
  {
    label: 'fe-xterm',
    prompt: `你负责 P1-F3-02：
修改 src/panels/terminal/useXterm.ts：
- import { hooks } from "../../ipc" 与 import { eventToStatus, STATUS_EMOJI } from "../../lib/claudeStatus"。
- 在主 useEffect 中（PTY spawn 相关逻辑之后）新增 onHookEvent 订阅：onHookEvent(payload => { if (payload.panelId !== panelId) return; const status = eventToStatus(payload.event, payload.notificationType); if (status === null) { if (payload.event === "SessionEnd") onTabStateChange?.({ active: false }); return; } onTabStateChange?.({ active: true, icon: STATUS_EMOJI[status] }); })
- useEffect return 中 unsubscribe（与 E2E/Terminal 清理一起）。注意避免在 hook 未就绪时调用 onTabStateChange（onTabStateChange 可能未传）。完成后跑 npx tsc --noEmit。`
  },
  {
    label: 'fe-panel',
    prompt: `你负责 P1-F3-03：
修改 src/panels/terminal/TerminalPanel.tsx：
- handleTabStateChange 改为：if (state.active) { if (state.title) api.setTitle(state.title); if (state.icon !== undefined) api.updateParameters({ ...params, tabIcon: state.icon }); } else { api.setTitle(originalTitleRef.current); api.updateParameters({ ...params, tabIcon: null }); }
- 关键：active=true 时不要覆盖 originalTitleRef；originalTitleRef 仅在组件挂载时初始化。完成后跑 npx tsc --noEmit。`
  },
  {
    label: 'fe-tab',
    prompt: `你负责 P1-F3-04、P1-F3-05：
【P1-F3-05】修改 src/panels/terminal/tabRules.ts：移除 claudeLogo import；tabTitleRegistry.register({ command: "claude", title: "claude" })（无 icon 字段）。
【P1-F3-04】修改 src/workspace/PageDockviewHost.tsx 的 DefaultTab：tabIcon 渲染分支：若 tabIcon 含 "/"、"\\"、或以 "http:" / "data:" 开头，则渲染 <img src={tabIcon} ... />；否则渲染 <span style={{ fontSize: 14, lineHeight: 1, flexShrink: 0 }}>{tabIcon}</span>。确保 img 的 alt 属性保留。
完成后跑 npx tsc --noEmit。`
  },
  {
    label: 'fe-tests',
    prompt: `你负责 P1-F3-07：
更新/新增 L2 测试：
1. src/__tests__/use-xterm-lifecycle.test.ts 或新建 src/__tests__/use-xterm-hooks.test.ts：mock onHookEvent，验证 panelId 过滤、UserPromptSubmit -> ⚡、SessionEnd -> clear、不匹配 panelId 不触发。
2. 更新 DefaultTab 相关测试（workspace-defaulttab.test.tsx）：验证 emoji tabIcon 渲染为 span，图片 tabIcon 渲染为 img。
3. 更新 tab-rules.test.ts：验证注册项无 icon 字段。
完成后跑 npm test。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/panels/terminal src/workspace/PageDockviewHost.tsx
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase1/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
