// =====================================================================
// Stage 09 Workflow — L4 E2E 关键路径
// =====================================================================
// 约束：本 Stage 只追加 E2E 测试，禁止修改生产代码
// =====================================================================

export const meta = {
  name: 'stage-09-e2e',
  description: 'L4 E2E：hooksConfig 面板打开与保存链路',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
本 Stage 特殊纪律：只追加 E2E 测试，禁止修改生产代码。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e-tests',
    prompt: `你负责 P3-TE-18。

【P3-TE-18】在 e2e-tests/test.e2e.ts 追加 L4 用例：
- 场景：通过 E2E helper 打开 hooksConfig 面板 → 在 JSON 模式输入合法 hooks 配置 → 点击保存 → 断言目标 settings.json 的 mtime 更新且内容正确。
- 打开面板方式：使用 window.__dockviewApi.addPanel 或新增 __slterm_e2e_openHooksConfig helper（如需要，在 e2e-tests/helpers.ts 中扩展）。
- JSON 文本输入：因 E2E 键盘输入限制（TE-17），通过 browser.execute 调用 CM6 view.dispatch 或向 textarea 输入；保存按钮使用 .click()。
- 目标文件：测试项目目录下的 .claude/settings.json（project 层）或 ~/.claude/settings.json（user 层，推荐 project 层便于断言）。
- 断言：文件存在、mtime 大于保存前、JSON 内容包含写入的事件与 handler。

约束：不改任何 src/ 或 src-tauri/ 生产代码；仅修改 e2e-tests/test.e2e.ts 与 helpers.ts。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行 E2E 全量验证：
1. npm run build:e2e
2. npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full e2e suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 09 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
