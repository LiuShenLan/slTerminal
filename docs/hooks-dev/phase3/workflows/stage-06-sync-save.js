// =====================================================================
// Stage 06 Workflow — 双模式同步与保存安全
// =====================================================================
// 跨边界契约：
//   - 共享状态: configJson / guiModel / dirty
//   - 保存前: JSON.parse 语法校验 + ajv schema 校验
//   - 保存后提示: "hooks 改动需重启 claude 会话生效"
// =====================================================================

export const meta = {
  name: 'stage-06-sync-save',
  description: 'JSON 与 GUI 双向同步 + 保存前双重校验 + 保存成功提示',
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
    label: 'frontend-sync-save',
    prompt: `你负责 P3-FE-16/17 与 P3-TE-13/14。

【P3-FE-16】实现双模式同步：
- 在 useHooksConfig.ts 中维护 configJson 与 guiModel。
- JSON 合法变更后调用 jsonToGui 更新 guiModel。
- GUI 变更后调用 guiToJson 更新 configJson。
- 将 JsonMode 的 onChange 与 GuiMode 的 onChange 都接入到 useHooksConfig 的 setter。
- JSON 非法时（JsonMode onValidationChange 返回 false）：禁止切换到 GUI 模式，工具栏显示错误提示。
- 两模式共享 dirty 状态。

【P3-FE-17】实现保存安全：
- 保存按钮触发流程：JSON.parse 语法校验 -> ajv schema 校验 -> writeHooksConfig。
- 任一校验失败则 window.alert 提示并拒绝保存。
- 保存成功后通过状态条/Toast 显示：「hooks 改动需重启 claude 会话生效」。
- 不做 .bak（Phase 3 决策）。

【收尾】更新 HooksConfigPanel.tsx 的工具栏：模式切换按钮在 JSON 非法时禁用；保存按钮调用 useHooksConfig.save()。

【P3-TE-13/14】新建测试：
- src/__tests__/hooks-config-sync.test.tsx：GUI 新增事件 → JSON 同步；JSON 合法修改 → GUI 同步；JSON 非法 → 切 GUI 被阻止。
- src/__tests__/hooks-config-save-safety.test.tsx：语法错误保存被拒、schema 错误保存被拒、合法保存显示重启提示。`
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
3. npm test -- hooks-config-sync hooks-config-save-safety
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
