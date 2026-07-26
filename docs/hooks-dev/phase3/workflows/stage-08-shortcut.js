// =====================================================================
// Stage 08 Workflow — 面板入口全局命令
// =====================================================================
// 跨边界契约：
//   - 命令 id: global.openHooksConfig
//   - 默认键: Ctrl+Shift+H（待执行期确认）
//   - handler 通过 window.__dockviewApi.addPanel 打开 hooksConfig 面板
// =====================================================================

export const meta = {
  name: 'stage-08-shortcut',
  description: '新增 global.openHooksConfig 全局命令打开 hooksConfig 面板',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。默认键待执行期确认，推荐 Ctrl+Shift+H。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-shortcut',
    prompt: `你负责 P3-FE-23/24/25 与 P3-TE-17。

【P3-FE-23】修改 src/features/shortcuts/commandCatalog.ts：
- 在 COMMAND_CATALOG 追加 global.openHooksConfig：
  - id: "global.openHooksConfig"
  - title: "打开 Hooks 配置"
  - category: "global"
  - context: "global"
  - defaultKey: key("KeyH", { ctrl: true, shift: true })  // 待执行期确认
  - priority: 10
- 确保默认键不是保留键（reserved.test.ts 守卫）。

【P3-FE-24】修改 src/features/shortcuts/globalCommands.ts：
- 在 createGlobalShortcuts 返回数组中追加 commandFromMeta("global.openHooksConfig", handler)。
- handler: 获取 api = getDockviewApi()；若 api 存在则 api.addPanel({ id: 生成唯一 panelId, component: "hooksConfig", params: {} }) 并返回 true；否则返回 false 透传。
- panelId 建议使用 hooksConfig-${Date.now()} 或递增计数器，避免重复。

【P3-FE-25】确认 src/App.tsx：
- App.tsx 已使用 ...createGlobalShortcuts(() => window.__dockviewApi)，无需修改调用点。只需确认无需改动。

【P3-TE-17】更新 src/__tests__/command-catalog.test.ts：
- EXPECTED_IDS 加入 "global.openHooksConfig"。
- 长度预期改为 10。
- 补充该命令的 commandFromMeta 断言（id/title/category/context/defaultKey/priority 完整）。
- 验证 defaultKey 不是保留键。`
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
2. npx eslint src/
3. npm test -- command-catalog hooks-config-command
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 08 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
