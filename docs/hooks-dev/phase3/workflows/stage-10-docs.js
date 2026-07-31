// =====================================================================
// Stage 10 Workflow — 文档同步 + 契约回查
// =====================================================================
// 约束：本 Stage 只改文档，禁止修改生产代码
// =====================================================================

export const meta = {
  name: 'stage-10-docs',
  description: '同步 Phase 3 相关 CLAUDE.md 与 test-inventory.md + C13 契约回查',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
本 Stage 特殊纪律：只改文档（CLAUDE.md / test-inventory.md / contract.md），禁止修改生产代码。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'docs-update',
    prompt: `你负责 P3-DOC-01/02/03/04/05/06。

【P3-DOC-01】更新 src/panels/CLAUDE.md：
- 在「当前面板类型」与文件清单中加入 hooksConfig。
- 描述：双模式编辑（JSON/GUI）、hooks 子树三层配置（user/project/local）、单条启停（ADR-0002）、注入段保护（slTerminal 托管标记 + 禁删/禁禁用）、F2 并入。
- 按既有格式追加，不在根 CLAUDE.md 展开细节。

【P3-DOC-02】更新 src/ipc/CLAUDE.md：
- 模块映射表追加 src/ipc/hooksConfig.ts ↔ hooks/：hooks_config_read、hooks_config_write（hooks 子树读写 + 后端 merge 语义）。
- 说明与 src/ipc/hooks.ts 区分（后者为 C6 注入/事件/用量命令）。

【P3-DOC-03】更新 src/stores/CLAUDE.md：
- Store 清单追加 hooksConfig.ts：disabledHooks 段、loadFromDisk/saveToDisk 模式、cancelPendingSave、面板挂载时加载（不在 App init）。

【P3-DOC-04】更新 src/features/shortcuts/CLAUDE.md：
- 命令目录追加 global.openHooksConfig（含同页单例语义：hooksConfig-{pageId}）。
- 扩展指南示例保持同步。

【P3-DOC-05】更新 .claude/test-inventory.md：
- 新增 Phase 3 测试文件与用例数（按实际执行后统计填入，不得照抄计划草案数字）。
- 更新全量用例总数；计数口径与既有文件一致。

【P3-DOC-06】回查契约一致性：
- 对照 docs/hooks-dev/contract.md C13 逐项核实最终实现：命令签名（hooks_config_read/write + hooks 子树 + merge）、handler 字段矩阵（C13-3 官方版）、事件目录（30 事件 x 10 组）、面板 id 规则（hooksConfig-{pageId}）。发现偏差先记录，回议后择一修订文档或代码（本 Stage 只许改文档——代码偏差记录到回查结论中，由主 agent 决策）。
- 确认 src-tauri/src/hooks/CLAUDE.md 追加 config.rs 文件行与两条命令说明（hooks 子树读写语义）；如缺失则补上。

约束：文档描述必须与 Stage 完成后的真实代码一致（先 Read 代码核对再动笔）；不可照抄计划草案。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行文档级验证：
1. 检查所有修改的 markdown 文件是否存在语法错误（无需运行 tsc/eslint）。
2. 用 grep 确认 test-inventory.md 中 Phase 3 新增文件数与实际 src/__tests__/ 下新增测试文件数一致。
3. grep 核对文档中的 IPC 命令名（hooks_config_read / hooks_config_write）、面板类型名（hooksConfig）、store 字段名（disabledHooks）与代码一致。
逐条报告结果。
`, { label: 'doc verification' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 10 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
