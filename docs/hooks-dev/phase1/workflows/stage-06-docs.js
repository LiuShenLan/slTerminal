// =====================================================================
// Stage 06 Workflow: 文档同步
// =====================================================================
// 契约头部：
//   - 新建 src-tauri/src/hooks/CLAUDE.md
//   - 更新 src/ipc/CLAUDE.md、src/lib/CLAUDE.md、src/panels/CLAUDE.md
//   - 更新 .claude/test-inventory.md
//   - 这是最后 Stage，依赖 01-05 完成
// =====================================================================

export const meta = {
  name: 'stage06-docs',
  description: 'Phase 1 文档同步',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只改文档（CLAUDE.md、test-inventory.md），不改生产代码；中文 markdown；完成后报告修改的文件清单。
禁区：无。
背景：修复要点详见 docs/hooks-dev/phase1/checklist.md 对应 ID 条目（先读再动手）。本 Stage 特殊纪律：只改文档，禁止改生产代码。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'doc-hooks',
    prompt: `你负责 P1-DOC-01：
新建 src-tauri/src/hooks/CLAUDE.md，按项目文档规范（渐进式披露、模块级实现细节）编写：
- 模块职责：信号文件通道、hooks 注入/卸载/状态查询。
- 文件清单：mod.rs、signal.rs、watcher.rs、inject.rs、assets/slterm-hook-reporter.js。
- 关键决策：单事件单文件 + 原子 rename、SLTERM_PANEL_ID 路由、脚本任何路径 exit 0、settings.json merge/卸载规则、版本检测。
- 命令：三命令签名与用途。
- 测试模式：单元测试组织、临时目录工厂、非法 JSON 测试、注入幂等测试。
完成后用 npx markdownlint-cli 检查（如果项目未安装则至少确保无语法问题）。`
  },
  {
    label: 'doc-sync',
    prompt: `你负责 P1-DOC-02/P1-DOC-03/P1-DOC-04/P1-DOC-05：
1. 修改 src/ipc/CLAUDE.md：在「模块映射」表中追加一行 \`hooks.ts\` ↔ \`src-tauri/src/hooks/\`，命令列表追加三命令。
2. 修改 src/lib/CLAUDE.md：文件表追加 \`claudeStatus.ts\`，说明四态映射单点职责。
3. 修改 src/panels/CLAUDE.md：更新 useCommandDetection 描述（OSC 133 C→🟡 attention）、useXterm（hook-event 订阅）、DefaultTab（emoji 渲染分支）、tabRules（claude 图标移除）。
4. 修改 .claude/test-inventory.md：按实际新增/修改测试文件更新用例数。新增项参考：
   - L1：src-tauri/src/hooks/mod.rs / inject.rs（建议 20+ 用例）
   - L1：src-tauri/src/pty/spawn.rs 增加 1 条 env 注入测试
   - L2：src/__tests__/ipc-hooks-contract.test.ts（建议 6-10 用例）
   - L2：src/__tests__/claude-status.test.ts（建议 12-16 用例）
   - L2：src/__tests__/use-xterm-hooks.test.ts 或扩展既有 use-xterm-lifecycle.test.ts（建议 4-8 用例）
   - L4：e2e-tests/test.e2e.ts 新增 2 条用例
   更新总计数。
完成后确保所有 markdown 文件无语法错误。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。文档 Stage 不跑代码测试，执行：
1. npx tsc --noEmit（确保文档修改未意外引入 TypeScript 类型问题）
2. 如果可用：npx markdownlint-cli src-tauri/src/hooks/CLAUDE.md src/ipc/CLAUDE.md src/lib/CLAUDE.md src/panels/CLAUDE.md .claude/test-inventory.md
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase1/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
