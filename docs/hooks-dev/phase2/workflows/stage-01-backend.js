// =====================================================================
// Stage 1 Workflow：后端 notification 插件 + hooks_context_usage 命令
// =====================================================================
// 跨边界契约（本脚本头部写死，agent 不各自推断）：
//   命令名：hooks_context_usage
//   参数：{ transcript_path: string } / JS { transcriptPath: string }
//   返回 DTO：ContextUsage { input_tokens: u64, output_tokens: u64 }（serde camelCase）
//   JS DTO：{ inputTokens: number, outputTokens: number } | null
//   事件名：hook-event
//   通知权限：notification:default
// =====================================================================

export const meta = {
  name: 'stage1-backend',
  description: '阶段 2 Stage 1：后端 notification 插件 + hooks_context_usage 命令',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\data\learn\code\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
Stage 特殊纪律：本 Stage 只改后端代码与 Cargo 依赖；禁止改前端代码、ConPTY flags、PTY 实现；所有 Tauri 命令必须经 generate_handler! 注册并返回 Result<_, AppError>；阻塞 I/O 必须用 spawn_blocking。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-deps',
    prompt: `你负责 P2-BE-01：在 src-tauri/Cargo.toml 添加 tauri-plugin-notification 依赖。

【P2-BE-01】在 [dependencies] 追加 \`tauri-plugin-notification = "2"\`，与现有 \`tauri-plugin-dialog = "2"\` 对齐。

要求：
- 不要修改其他依赖版本。
- 完成后报告 Cargo.toml 的修改行。`
  },
  {
    label: 'backend-plugin',
    prompt: `你负责 P2-BE-02 / P2-BE-03：在 src-tauri/src/lib.rs 初始化 notification 插件并在 capabilities/default.json 追加权限。

【P2-BE-02】src-tauri/src/lib.rs：
- 在 Builder 链中加入 \`.plugin(tauri_plugin_notification::init())\`（放在 dialog/clipboard 等插件旁）。
- 在 \`generate_handler!\` 宏中追加 \`hooks_context_usage\` 命令。

【P2-BE-03】src-tauri/capabilities/default.json：
- 在 permissions 数组追加 \`"notification:default"\`，不加通配符 \`*\`。

要求：
- 不要修改其他插件初始化或命令注册。
- 完成后报告 lib.rs 与 capabilities/default.json 的修改行。`
  },
  {
    label: 'backend-usage',
    prompt: `你负责 P2-BE-04 / P2-BE-05 / P2-BE-06：在 src-tauri/src/hooks/ 模块新增 hooks_context_usage 命令与 ContextUsage DTO，并确认 signal 解析透传 transcriptPath。

前置：阶段 1 已完成 hooks 模块基础结构（mod.rs/signal.rs 等）。若文件/模块不存在，请按阶段 1 契约 C4 创建。

【P2-BE-04】新增命令 \`hooks_context_usage\`：
- 签名：\`async fn hooks_context_usage(app: AppHandle, transcript_path: String) -> Result<Option<ContextUsage>, AppError>\`（参数名以实际模块惯例为准，但概念一致）。
- 在 spawn_blocking 内执行：
  1. 打开 transcript_path 文件；
  2. 从文件尾部读取约 64KB 内容（避免加载数百 MB 全文件），按行分割；
  3. 从最后一行逆行扫描，遇到第一个 JSON 对象中 \`message.usage.input_tokens\` 与 \`output_tokens\` 均存在的行即返回 \`Some(ContextUsage { input_tokens, output_tokens })\`；
  4. 解析失败、无 usage、文件不存在等任何异常返回 \`Ok(None)\`，不 panic。

【P2-BE-05】定义 DTO：
- Rust：
  \`\`\`rust
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct ContextUsage {
      pub input_tokens: u64,
      pub output_tokens: u64,
  }
  \`\`\`
- 若阶段 1 已在 src/types/hooks.rs 或 hooks 模块内定义事件 DTO，请将 ContextUsage 放在同一位置；否则新建 src/types/hooks.rs 并在 hooks 模块引用。
- JS 侧 src/types/hooks.ts 追加 \`export interface ContextUsage { inputTokens: number; outputTokens: number }\`。

【P2-BE-06】确认 signal 解析后 HookEventPayload 含 transcript_path 字段；若缺失，补齐解析逻辑。

要求：
- 命令必须在 lib.rs generate_handler! 注册（已由 backend-plugin agent 处理，但你需确认宏参数已包含本命令名）。
- 所有 I/O 在 spawn_blocking 中完成。
- 完成后报告新增/修改的文件清单与关键函数签名。`
  }
]

const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\data\learn\code\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage1 的改动是否实际生效（项目根 D:\data\learn\code\slTerminal）。
先读 docs/hooks-dev/phase2/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
