// =====================================================================
// Stage 04 Workflow — Schema 内嵌与 JSON 模式
// =====================================================================
// 跨边界契约：
//   - Schema 文件: src/features/hooksConfig/schema/claude-code-settings.json
//   - JSON 模式组件: JsonMode.tsx
//   - MatcherTester 组件: MatcherTester.tsx
// =====================================================================

export const meta = {
  name: 'stage-04-json-mode',
  description: '内嵌 SchemaStore schema + CM6 JSON 模式 + 事件导航 + matcher 测试工具',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-json-mode',
    prompt: `你负责 P3-FE-07/11 与 P3-TE-09/10。

【P3-FE-07】内嵌 SchemaStore 官方 schema：
- 新建目录 src/features/hooksConfig/schema/。
- 将 SchemaStore https://json.schemastore.org/claude-code-settings.json 内容复制到 src/features/hooksConfig/schema/claude-code-settings.json。
- 确保 Vite 可通过 import schema from ".../schema/claude-code-settings.json" 加载。
- 新增 npm 依赖：codemirror-json-schema（CM6 schema 补全/校验）、ajv（保存前 schema 校验）。修改 package.json 与 package-lock.json。

【P3-FE-11】实现 src/panels/hooksConfig/JsonMode.tsx：
- 使用 CodeMirror 6 + @codemirror/lang-json。
- 集成 codemirror-json-schema，使用内嵌 schema 提供补全、校验、悬停文档、错误波浪线。
- props 接受 value、onChange、onValidationChange(isValid, diagnostics)。
- 事件导航侧栏：30+ 事件按九大分组（会话生命周期/用户交互/工具调用/权限系统/通知/Agent 子代理/上下文压缩/环境变更/MCP 交互），点击后通过简单文本搜索定位到对应事件键并设置选区。
- 集成 MatcherTester.tsx 作为内联/浮动工具。

【P3-FE-09】实现 src/panels/hooksConfig/MatcherTester.tsx：
- 输入 matcher + toolName，调用 matcherEngine.matchHook，显示命中结果与匹配模式（exact-or / regex / all）。

【收尾】在 HooksConfigPanel.tsx 中接入 JsonMode 作为 JSON 模式内容。

【P3-TE-09/10】新建测试：
- src/__tests__/hooks-config-json-mode.test.tsx：渲染、schema 扩展注册、非法 JSON 触发 onValidationChange。
- src/__tests__/hooks-config-event-nav.test.tsx：事件分组渲染、点击跳转。`
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
3. npm test -- hooks-config-json-mode hooks-config-event-nav
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
