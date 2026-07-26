// =====================================================================
// Stage 05 Workflow — GUI 表单模式（Master-Detail）
// =====================================================================
// 跨边界契约：
//   - GUI 模型: HooksConfigGui / HookEventGroup / HookMatcherGroup / HookHandlerGui
//   - 5 种 handler 字段矩阵与必填项照 F6 表
//   - 事件→handler 支持矩阵约束可选类型
// =====================================================================

export const meta = {
  name: 'stage-05-gui-mode',
  description: 'GUI 模式 Master-Detail 事件树 + 5 种 handler 专用表单',
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
    label: 'frontend-gui-tree',
    prompt: `你负责 P3-FE-12/13 与 P3-TE-12。

【P3-FE-12】实现 src/panels/hooksConfig/GuiMode.tsx：
- Master-Detail 布局：左侧事件树，右侧详情区。
- props 接收 guiModel、onChange、selectedEvent/selectedMatcherIndex/selectedHandlerIndex 及 setter。
- 提供添加/删除事件、matcher 组、handler 的回调入口。

【P3-FE-13】实现 src/panels/hooksConfig/EventTree.tsx：
- 三级树：事件分组（可折叠）→ 事件名 → matcher 组 → handler 摘要。
- 显示各事件下 hook 数量。
- 选中态高亮；颜色从 theme/colors.ts 取 token（硬约束 #6）。
- props 接收 guiModel、onSelect、onAddEvent、onDeleteEvent 等。

【收尾】在 HooksConfigPanel.tsx 中接入 GuiMode 作为 GUI 模式内容。

【P3-TE-12】新建 src/__tests__/hooks-config-event-tree.test.tsx：
- 覆盖：九大分组渲染、hook 计数、选中回调、添加/删除事件。`
  },
  {
    label: 'frontend-handler-form',
    prompt: `你负责 P3-FE-14 与 P3-TE-11。

【P3-FE-14】实现 src/panels/hooksConfig/HandlerForm.tsx：
- 根据 type 渲染 5 种 handler 专用表单：
  - command: command*、args、async、asyncRewake、shell、timeout、if、allowedEnvVars
  - http: url*、method、headers、body、timeout、allowedEnvVars
  - mcp_tool: server*、tool*、args、timeout
  - prompt: prompt*、timeout
  - agent: prompt*、description、subagent_type、model（sonnet/opus/haiku/fable）、timeout
- 事件→handler 支持矩阵：
  - Notification/SessionEnd/PreCompact/PostCompact 禁用 prompt/agent
  - SessionStart/Setup 禁用 http/prompt/agent
- 切换 type 时保留公共字段（如 timeout），清除不适用的字段。
- props 接收 handler、event、onChange。

【P3-TE-11】新建 src/__tests__/hooks-config-handler-form.test.tsx：
- 覆盖：5 种 type 必填字段渲染、事件支持矩阵过滤、type 切换清理字段。`
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
3. npm test -- hooks-config-event-tree hooks-config-handler-form
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
