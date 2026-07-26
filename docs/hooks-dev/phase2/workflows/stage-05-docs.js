// =====================================================================
// Stage 5 Workflow：文档同步
// =====================================================================
// 跨边界契约（本脚本头部写死，agent 不各自推断）：
//   命令名：hooks_context_usage
//   参数：{ transcriptPath: string }
//   返回 DTO：{ inputTokens: number, outputTokens: number } | null
//   侧栏视图 id：agent-status
//   通知权限：notification:default
// =====================================================================

export const meta = {
  name: 'stage5-docs',
  description: '阶段 2 Stage 5：文档同步与契约回填',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\data\learn\code\slTerminal。
纪律：只修改分配给你的文档文件，不修改任何生产/测试代码；文档描述必须与当前代码一致，禁止撒谎。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
Stage 特殊纪律：本 Stage 只改文档；改完后必须跑全量静态检查与测试，确保文档更新未遗漏代码变更导致的不一致。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'doc-ipc',
    prompt: `你负责 P2-DOC-01：更新 src/ipc/CLAUDE.md。

内容：
1. 在「模块映射」表格追加一行：\`notification.ts\` → Tauri plugin notification → 封装 \`isPermissionGranted\` / \`requestPermission\` / \`sendNotification\`。
2. 在 \`hooks.ts\` 条目追加 \`contextUsage\` 命令说明：命令名 \`hooks_context_usage\`、参数 \`{ transcriptPath: string }\`、返回 \`ContextUsage | null\`。
3. 在「thin wrapper」说明段追加 notification 先例。

要求：
- 文档中的命令名与 DTO 字段必须与代码一致。
- 完成后报告修改位置。`
  },
  {
    label: 'doc-hooks',
    prompt: `你负责 P2-DOC-02：更新 src-tauri/src/hooks/CLAUDE.md。

注意：若阶段 1 未创建该文件，则在本 Stage 新建。

内容：
1. 「命令」表格追加 \`hooks_context_usage\`：
   - 参数：\`transcript_path: String\`
   - 返回：\`Result<Option<ContextUsage>, AppError>\`
   - 用途：解析 transcript JSONL 尾部，返回最后一条 message.usage 的 token 数据。
2. 新增「ContextUsage DTO」段：字段 \`input_tokens\` / \`output_tokens\`，serde camelCase。
3. 新增「实现要点」段：尾部读取 + 逆行扫描 + 解析失败返回 None。
4. 新增「测试位置」段：指向 L1 测试文件。

要求：
- 若文件已存在，遵循其既有格式；若新建，按 slTerminal CLAUDE.md 渐进式披露原则只写本模块内容。
- 完成后报告修改/新增文件路径。`
  },
  {
    label: 'doc-sideviews',
    prompt: `你负责 P2-DOC-03：更新 src/features/sideViews/CLAUDE.md。

内容：
1. 在「SideViewRegistry 扩展指南」示例中追加 \`agent-status\` 视图：
   - id: "agent-status"
   - title: "Agent 状态"
   - icon: "🤖"
2. 更新「默认按钮归属」描述：\`DEFAULT_ZONES.top\` 含 projects / explorer / commit / agent-status。
3. 在「文件」表格追加 \`AgentStatusView\` 相关文件（若已知路径）。

要求：
- 不要修改其他视图描述。
- 完成后报告修改位置。`
  },
  {
    label: 'doc-inventory',
    prompt: `你负责 P2-DOC-04：更新 .claude/test-inventory.md。

内容：
- 追加阶段 2 新增测试文件：
  - \`src/__tests__/notifications.test.ts\`（L2，N 用例）
  - \`src/__tests__/agent-status-view.test.tsx\`（L2，N 用例）
  - \`src/__tests__/agent-status-hook.test.ts\`（L2，N 用例）
  - \`src-tauri/src/hooks/\` 内 L1 测试（N 用例）
  - \`e2e-tests/test.e2e.ts\` 追加 L4 用例（N 用例）
- 用例数在 Stage 4 完成后按实际计数回填；若当前无法确定，写 "见 Stage 4 产出" 并占位。

要求：
- 保持 test-inventory.md 既有表格格式。
- 完成后报告追加行。`
  },
  {
    label: 'doc-contract',
    prompt: `你负责 P2-DOC-05：回填 docs/hooks-dev/contract.md 的 C12 段。

内容：
在 C12「阶段 2 专有契约（F4/F5）」中，将以下占位描述替换为写死契约：
- F5 上下文用量命令名：\`hooks_context_usage\`
- 参数：\`{ transcriptPath: string }\`
- 返回 DTO：\`ContextUsage { inputTokens: number, outputTokens: number } | null\`
- 后端实现：transcript JSONL 尾部读取 + 逆行扫描最后一条 \`message.usage\` + 失败返回 null

要求：
- 回填后的 C12 与 checklist/stages/execution-plan 中的契约完全一致。
- 不要修改 C1-C11 与 C13 内容。
- 完成后报告修改行。`
  }
]

const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\data\learn\code\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage5 的改动是否实际生效（项目根 D:\data\learn\code\slTerminal）。
先读 docs/hooks-dev/phase2/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
