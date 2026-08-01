// =====================================================================
// Stage 09 Workflow — L4 E2E 关键路径（走 project 层）
// =====================================================================
// 约束：本 Stage 只追加 E2E 测试，禁止修改生产代码
// 安全约束：E2E 用例禁止写真实 ~/.claude/settings.json（user 层）——
//   一律走 tempdir 项目的 project/local 层（C13-9）
// 门禁补充：若改动 e2e-tests/helpers.ts（不在根 tsconfig include），
//   追加 npx vite build 构建级验证
// =====================================================================

export const meta = {
  name: 'stage-09-e2e',
  description: 'L4 E2E：hooksConfig 面板打开与保存链路（project 层 + merge 保留断言）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
本 Stage 特殊纪律：只追加 E2E 测试，禁止修改生产代码；E2E 用例禁止写真实 ~/.claude/settings.json（走 tempdir 项目 project/local 层）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e-tests',
    prompt: `你负责 P3-TE-18。

【P3-TE-18】在 e2e-tests/test.e2e.ts 追加 L4 用例（面板打开与保存链路，走 project 层）：
- 场景：__slterm_e2e_createProject 创建 tempdir 项目 → 打开 hooksConfig 面板（经 __dockviewApi.addPanel 或合成快捷键）→ 切到 project 层 → JSON 模式写入合法 hooks 配置 → 点击保存 → 断言 <tempdir>/.claude/settings.json 真实写盘。
- 断言三件事：① 文件 mtime 更新；② hooks 内容正确（写入的事件/handler 存在）；③ merge 保留——保存前经 browser.execute + Node 侧预置其他字段（如 permissions）到该文件，保存后断言 permissions 原样保留（验证后端 read-modify-write）。
- 禁止写 user 层——不碰真实 ~/.claude/settings.json（C13-9）。
- 保存按钮使用 .click() 触发（E2E 键盘输入限制，见 e2e-tests/CLAUDE.md）；JSON 文本输入通过 browser.execute 调 CM6 view.dispatch 或新增 __slterm_e2e_* helper 注入。
- 如需 helper（打开面板 / CM6 文本注入），在 e2e-tests/helpers.ts 扩展——该文件不在根 tsconfig include，改动后本 Stage 门禁须含 npx vite build。

约束：不改任何 src/ 或 src-tauri/ 生产代码；仅修改 e2e-tests/test.e2e.ts 与（必要时）e2e-tests/helpers.ts。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行 E2E 全量验证：
1. npm run build:e2e
2. npm run wdio
3. 若本 Stage 改动了 e2e-tests/helpers.ts：追加执行 npx vite build（该文件不在根 tsconfig include，需构建级门禁兜底）；未改动则跳过并说明
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full e2e suite' })

// === Phase 3: 逐项验证 ===
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
