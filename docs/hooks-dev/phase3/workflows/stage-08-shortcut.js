// =====================================================================
// Stage 08 Workflow — 面板入口全局命令（同页单例）
// =====================================================================
// 跨边界契约：
//   - 命令 id: global.openHooksConfig；defaultKey: Ctrl+Shift+H（被拦截降级 Ctrl+Alt+H）
//   - 同页单例: 面板 id = hooksConfig-{pageId}；getPanel 命中 focus() 不新建
//   - 禁止引用 generatePanelId（代码库不存在该函数）；id 按单例规则拼接
//   - handler 经 createGlobalShortcuts(getDockviewApi) 现有签名追加，App.tsx 调用点不变
// =====================================================================

export const meta = {
  name: 'stage-08-shortcut',
  description: '新增 global.openHooksConfig 全局命令打开 hooksConfig 面板（同页单例）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。默认键推荐 Ctrl+Shift+H，执行期实测被 WebView2 拦截则降级 Ctrl+Alt+H。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-shortcut',
    prompt: `你负责 P3-FE-23/24/25 与 P3-TE-17。

【P3-FE-23】修改 src/features/shortcuts/commandCatalog.ts：
- 在 COMMAND_CATALOG 追加：
  - id: "global.openHooksConfig"
  - title: "打开 Hooks 配置"
  - category/context: "global"
  - defaultKey: "Ctrl+Shift+KeyH"（keystroke 字符串格式，照目录既有条目；被拦截降级 "Ctrl+Alt+KeyH"）
  - priority: 10
- 确保默认键不是保留键（reserved.ts 守卫）。

【P3-FE-24】修改 src/features/shortcuts/globalCommands.ts：
- createGlobalShortcuts(getDockviewApi) 现有签名不变，返回数组追加 commandFromMeta("global.openHooksConfig", handler)。
- handler 逻辑（同页单例）：
  1. 取 useLayout.getState().activePageId；无活跃页面返回 false 透传。
  2. 面板 id = "hooksConfig-" + activePageId（单例规则，契约 C13-7）。
  3. api = getDockviewApi()；无 api 返回 false 透传。
  4. api.getPanel(id) 命中 → panel.focus() 返回 true（不新建）。
  5. 未命中 → api.addPanel({ id, component: "hooksConfig", title: "Hooks 配置", params: { panelId: id } }) 返回 true。
- 禁止引用 generatePanelId——代码库不存在该函数；id 按上述单例规则拼接。

【P3-FE-25】确认 src/App.tsx：
- App.tsx 已 registry.register([...createGlobalShortcuts(...)])，工厂返回值追加命令后无需修改调用点——仅需确认。

【P3-TE-17】更新 src/__tests__/command-catalog.test.ts 并新建 src/__tests__/hooks-config-entry.test.ts：
- command-catalog.test.ts：EXPECTED_IDS 加入 "global.openHooksConfig"、长度预期改为 10、补充该命令元数据断言（id/title/category/context/defaultKey/priority 完整）、默认键非保留键断言。
- hooks-config-entry.test.ts：createGlobalShortcuts 返回该命令；handler 首次触发 addPanel（id 为 hooksConfig-{pageId}）；重复触发 getPanel 命中聚焦不新建；无 DockviewApi / 无活跃页面时返回 false 透传。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test -- command-catalog hooks-config-entry
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
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
