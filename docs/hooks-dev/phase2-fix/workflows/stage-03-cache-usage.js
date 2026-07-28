// =====================================================================
// Stage 03 — ContextUsage cache tokens 契约
// =====================================================================
// 编排：并行实现 A ∥ B → 全量测试 → 逐项验证
//
// 跨边界契约（写死，agent 不各自推断；真值源 = checklist.md「跨边界契约」段）:
//   契约 3 ContextUsage 四字段: { inputTokens, outputTokens,
//     cacheReadInputTokens, cacheCreationInputTokens }。
//     用量口径 = (inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / 200_000；
//     outputTokens 不计占用，保留为信息字段。
//     transcript 缺 cache 字段默认 0（serde default，兼容旧 transcript）；
//     input_tokens 缺失仍整行 None（沿用现状）。
//     铁证：真实 transcript 尾行 input_tokens 2745 + cache_read_input_tokens 196096
//     → 旧算法 1.4% vs 实际 99.4%。
//   跨边界一致性：Rust serde 字段名（cacheReadInputTokens/cacheCreationInputTokens）
//     与 TS 字段名必须逐字符一致——两边照抄本契约，不各自推断。
//
// fix-loop args.constraints 应传值（单一出处，勿手写第三份）:
//   本 Stage 特殊纪律：A 仅碰 src-tauri/src/hooks/usage.rs 单文件（含其
//   #[cfg(test)]），可跑 cargo 系命令自验；B 仅碰前端/文档/前端测试文件，
//   可跑 npm test 自验；A/B 无共享资源（cargo target 锁与 vitest 互不干扰）。
// =====================================================================

export const meta = {
  name: 'stage-03-cache-usage',
  description: 'Stage 03 ContextUsage 增 cache tokens 四字段——用量口径对齐真实占用',
  phases: [
    { title: '并行实现' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点先读 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目 + 「跨边界契约」段（契约取值唯一真值源，禁止各自推断）。
本 Stage 特殊纪律：A 仅碰 src-tauri/src/hooks/usage.rs 单文件（含其 #[cfg(test)]），可跑 cargo 系命令自验；B 仅碰前端/文档/前端测试文件，可跑 npm test 自验；A/B 无共享资源（cargo target 锁与 vitest 互不干扰）。`

// === Phase 1: 并行实现（A ∥ B，文件零重叠）===
phase('并行实现')
const implResults = await parallel([
  () => agent(`${PREAMBLE}

你负责 PF2-BE-01、PF2-TE-06（Rust 契约层，单文件 src-tauri/src/hooks/usage.rs）：

【PF2-BE-01】usage.rs ContextUsage 增 cache 两字段
- 位置：ContextUsage {input_tokens, output_tokens} :15-20（serde camelCase）；parse_usage_line :79-88（两字段均 ? 缺失整行 None）
- 按契约 3：ContextUsage 增 cache_read_input_tokens: u64 / cache_creation_input_tokens: u64（serde camelCase + #[serde(default)] 兼容缺失）
- parse_usage_line 提取两字段（缺失 unwrap_or(0)）；input_tokens 缺失仍整行 None（现状沿用）
- serde 两测试（:255-273 context_usage_serialize_camelcase / context_usage_deserialize_camelcase）字面量同步补新字段——结构体字面量缺新字段即编译错
- parse_extra_fields_ignored（:143-151）用 cache_read/cache_write 字段名（非 cache_read_input_tokens）→ 不受影响（实证，勿动）

【PF2-TE-06】usage.rs L1 cache 分支（同文件 #[cfg(test)]）
- 新增 parse 分支用例：含 cache 两字段提取 / 缺 cache 字段默认 0（兼容旧 transcript）/ 仅 input+output 旧格式 / cache 为 0 显式值
- serde 两测试四字段 camelCase 断言（cacheReadInputTokens/cacheCreationInputTokens 断言串存在于序列化输出）
- 代码注释写明 mock 边界盲区认知：mockIPC 只守 JS 侧形状——cache 字段真实解析必须 L1 覆盖（后端真实反序列化），不得仅以 L2 mock 通过为据
- 完成判据：cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings 通过 + cargo test --manifest-path src-tauri/Cargo.toml hooks::usage -- --test-threads=1 通过（cargo target 锁与 B 的 vitest 无冲突）。`, { label: 'A:rust-usage-contract' }),

  () => agent(`${PREAMBLE}

你负责 PF2-FE-11、PF2-DOC-01、PF2-TE-07、PF2-TE-02（cache 口径部分）（前端 + 文档 + 前端测试）：

【PF2-FE-11】前端 ContextUsage 同步 + 用量口径
- src/types/hooks.ts：ContextUsage（现 2 字段）增 cacheReadInputTokens: number / cacheCreationInputTokens: number（必填）——字段名与 Rust serde 名逐字符一致（契约 3）
- src/features/agentStatus/AgentStatusRow.tsx:31-35：total 改 inputTokens + cacheReadInputTokens + cacheCreationInputTokens（outputTokens 不在总占用求和内，保留为信息字段）
- 波及确认：AgentSessionRow.usage 类型为引用 ContextUsage（Stage 01 已改），随本项自动扩为四字段

【PF2-DOC-01】docs/hooks-dev/contract.md C12 回填
- 位置：contract.md:124（C12 定义 ContextUsage { inputTokens, outputTokens }）
- 回填四字段定义 + 用量口径（(input + cacheRead + cacheCreation) / 200_000，output 不计占用保留为信息字段）+ 缺 cache 默认 0 兼容约定

【PF2-TE-07】src/__tests__/ipc-hooks-contract.test.ts 键集合守卫
- :310 mockUsage 字面量补 cache 两字段（四字段必填后旧两字面量编译错）
- 新增 ContextUsage 键集合精确匹配守卫（DBG-4 模式，照 :240-268 HookEventPayload 8 字段先例）：Object.keys(usage).sort() 精确等于四字段——存在性断言防不住字段增删漂移

【PF2-TE-02 cache 口径部分】src/__tests__/agent-status-view.test.tsx
- :266,341,351,362 makeRow usage 字面量补 cache 字段；75%/low/medium/high 四断言按新口径重算（total = inputTokens + cacheReadInputTokens + cacheCreationInputTokens）

【接力 Stage 01】src/__tests__/agent-status-hook.test.ts
- T7 usage mock 返回字面量补 cache 字段（Stage 01 产物为 2 字段——四字段必填后编译错，由你接力补齐）
- grep 兜底：inputTokens: 全仓测试文件无 2 字段字面量残留（逐一 Read 确认）
- 完成判据：npx tsc --noEmit 通过 + npm test 全绿（B 与 A 并行——cargo 与 vitest 无共享资源，你可跑 npm test）。`, { label: 'B:frontend-usage-contract' }),
])

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
3. npx tsc --noEmit
4. npx eslint src/
5. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { implResults, testResult, verifyResult }
