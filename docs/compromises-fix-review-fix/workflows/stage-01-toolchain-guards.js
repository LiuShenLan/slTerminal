// =====================================================================
// Stage 01 · 工具链与门禁（TE-01/02/03/07）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：本计划全程 agent 严格串行（用户并发限制约束）——for 循环顺序
// await，不用 parallel()；测试命令逐条串行（CP-007 基准负载敏感）。
// fix-loop 调用本 Stage 时 args.constraints 传 ""（无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-01-toolchain-guards',
  description: 'S01 工具链与门禁：TS7 状态码守卫 + d.mts null 契约 + ci.yml src/types 守卫 + vitest exclude 清理',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
L1 定向红线（TQ-COV-06）：cargo 定向测试一律 \`cargo test --test lib_tests <filter> -- --test-threads=1\`，禁裸 filter/--lib。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构（全程 agent 串行——用户并发限制约束）===
phase('串行重构')
const agents = [
  { label: 'ts7-trigger', prompt: `你负责 TE-01 + TE-02（先读 checklist 两条目六段式原文）：
【TE-01】scripts/check-ts7-trigger.mjs 的 getJson 加 HTTP 状态码守卫——非 2xx 一律 reject(new Error(\`HTTP \${status}\`))（先 res.resume() 排空），并加 http/https 协议分派（url.startsWith("https:") ? https : http，import http from "node:http"，http 分支仅供测试本地桩）；getJson 改 export。check-ts7-trigger.d.mts 补 getJson 声明。脚本头注退出码 2 语义句补「（含非 2xx 响应——TE-01）」。
测试：src/__tests__/deps-ts7-trigger.test.ts 新增 describe("getJson HTTP 状态码守卫") 两用例——本地 http server 桩（listen(0, "127.0.0.1")）返 403 合法 JSON 断言 rejects.toThrow("HTTP 403")；返 200 JSON 断言正常 resolve。用例代码照抄 checklist TE-01 修复步骤 3。
【TE-02】check-ts7-trigger.d.mts 的 evaluateTrigger 两参改 string | null | undefined；deps-ts7-trigger.test.ts:35 删 \`as unknown as string\` cast。
自验：npx vitest run src/__tests__/deps-ts7-trigger.test.ts 绿（允许跑——vitest jsdom 本地桩用随机端口，无共享资源冲突）；npx tsc --noEmit 绿。` },
  { label: 'ci-guard', prompt: `你负责 TE-03（先读 checklist TE-03 六段式原文）：
.github/workflows/ci.yml 在 KaTeX 守卫 step（:59-63）之后插入 src/types 漂移守卫 step，run 块两条命令照抄 checklist（cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1 + git diff --exit-code -- src/types，仓根执行，无 working-directory）。
文档同步：src/types/CLAUDE.md:16 与 src-tauri/src/CLAUDE.md:71 的守卫操作指令句各补「（CI 门禁 step 已落地——ci.yml Guard — src/types）」。
不跑测试（CI 配置无本地自动化面）。` },
  { label: 'vitest-exclude', prompt: `你负责 TE-07（先读 checklist TE-07 六段式原文）：
vitest.config.ts:7 的 exclude 数组删除 'datalearncodeterax-ai-temp' 元素（去斜杠拼接损坏产物，从未生效），改后为 ['node_modules', '.temp', 'e2e-tests']。
不跑全量测试（全量测试 phase 统一跑）。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行——CP-007 基准负载敏感，禁并行）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行）：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1
5. git diff --exit-code -- src/types
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。npm test 报告最终「Test Files / Tests」统计行原文。
（本 Stage 无 Rust 源码改动，收窄不跑 clippy/fmt/L1 全量——收窄理由已登记 stages.md。）
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
