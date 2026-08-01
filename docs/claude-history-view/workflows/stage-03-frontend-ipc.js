// =====================================================================
// Stage 03: 前端 DTO + IPC 封装与契约测试
// 覆盖项: FE-01、FE-02、FE-03
// 跨边界契约（写死，agent 不各自推断）:
//   invoke 命令名: claude_history_scan / claude_history_delete / claude_history_rename
//   invoke 参数 camelCase: delete → { sessionId }；rename → { sessionId, newTitle }
//   HistorySession TS 接口七键: sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists
//   TitleSource = "customTitle"|"aiTitle"|"summary"|"firstPrompt"|"none"
// fix-loop 调用本 Stage 时 args.constraints 传空串（无特殊纪律）
// =====================================================================

export const meta = {
  name: 'stage03-frontend-ipc',
  description: 'Stage 03: 前端 DTO + IPC 封装与契约测试',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 FE-01..03 条目 + docs/claude-history-view/stages.md Stage 03 实现要点，并 Read 后端 src-tauri/src/claude_history/mod.rs 确认 DTO 序列化键名（双边对照，硬约束 #4），再动手。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-ipc-agent',
    prompt: `你负责 FE-01、FE-02、FE-03：

【FE-01 src/types/claudeHistory.ts】
- 新建 HistorySession 接口，恰七键（与后端 mod.rs serde 输出逐字一致）：
  sessionId: string
  cwd: string | null
  title: string | null
  titleSource: TitleSource
  firstPrompt: string | null
  mtimeMs: number
  cwdExists: boolean
- TitleSource 联合类型："customTitle" | "aiTitle" | "summary" | "firstPrompt" | "none"。
- src/types/index.ts 追加 export 登记（Read 现状照既有模式）。

【FE-02 src/ipc/claudeHistory.ts】
- 新建三函数封装（照 src/ipc/hooksConfig.ts 模式，invoke 单点，硬约束 #1）：
  scanHistory(): Promise<HistorySession[]> — invoke("claude_history_scan")
  deleteHistorySession(sessionId: string): Promise<void> — invoke("claude_history_delete", { sessionId })
  renameHistorySession(sessionId: string, newTitle: string): Promise<void> — invoke("claude_history_rename", { sessionId, newTitle })
- src/ipc/index.ts barrel 追加 export（Read 现状照既有模式）。

【FE-03 src/__tests__/ipc-claude-history-contract.test.ts】
- 照 src/__tests__/ipc-hooks-config-contract.test.ts 的 mockIPC 模式：三命令 × 四维（命令名/参数结构/正常返回/异常传播）= 12 条用例。
- 命令名断言 snake_case 逐字（claude_history_scan 等，非驼峰）；参数结构断言键集合精确匹配（{ sessionId } / { sessionId, newTitle }，防字段漂移）；异常传播用 rejects 断言不吞异常。
- scanHistory 返回数组透传断言（mock 返回 HistorySession[] 样例，含七字段全形态）。

约束：invoke 只允许出现在 src/ipc/claudeHistory.ts；不改后端任何文件；不建 src/features/claudeHistory/（归 Stage 04/05）。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 3 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
