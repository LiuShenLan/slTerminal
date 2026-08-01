// =====================================================================
// Stage 04 — L4 防复发用例
// =====================================================================
// 编排：单 agent 用例改写 → build:e2e + wdio 实跑 → 逐项验证
//
// 行建模新语义（Stage 01/03 已落地的最终形态，本 Stage 据此写用例，不各自推断）:
//   行 = 运行中的 claude 会话（纯 shell 终端无行）；建行 = sessionChange ∨
//   hook 事件（首个信号即建行）；删行 = sessionChange(null) ∨ SessionEnd/Exit ∨
//   remove；初始扫描只建活会话行且携 transcriptPath 主动拉 usage（切项目
//   用量保持）；remove 事件经 deps [] 稳定订阅不丢失（R4 根治）。
//
// 门禁特殊性（写死）: e2e-tests/test.e2e.ts 不在根 tsconfig include 内
//   （tsc/eslint 不覆盖）——本 Stage 门禁 = npm run build:e2e + npm run wdio
//   实跑（构建级 + 行为级双覆盖）。必须用 npm run build:e2e（= cross-env
//   VITE_E2E=1 tauri build --debug --no-bundle），不用裸 tauri build——E2E
//   helper 由 E2E_ENABLED 门控，tauri build 前端恒 production（DEV=false），
//   不设 VITE_E2E=1 则 helper 被 tree-shake、wdio 全卡 Workspace 未就绪。
//
// fix-loop args.constraints 应传值（单一出处，勿手写第三份）:
//   本 Stage 特殊纪律：只改 e2e-tests/test.e2e.ts 单文件；不改生产代码、
//   不改 e2e-tests/helpers.ts（自查无行建模逻辑复制即可，有问题报告不修改）。
// =====================================================================

export const meta = {
  name: 'stage-04-l4-e2e',
  description: 'Stage 04 L4 防复发——R2/R3/R4 变体常驻用例 + 静态行语义反转',
  phases: [
    { title: '用例改写' },
    { title: '构建与实跑' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点先读 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目 + 「跨边界契约」段（契约取值唯一真值源，禁止各自推断）。
本 Stage 特殊纪律：只改 e2e-tests/test.e2e.ts 单文件；不改生产代码、不改 e2e-tests/helpers.ts（自查无行建模逻辑复制即可，有问题报告不修改）。`

// === Phase 1: 用例改写（A：e2e-tests/test.e2e.ts 单文件）===
phase('用例改写')
const fixResult = await agent(`${PREAMBLE}

你负责 PF2-TE-09（L4 防复发用例，R2/R3/R4 变体）。前置：Stage 01/03 已落地行建模新语义与 cache 四字段——先 Read 以下最终形态再写用例（禁凭计划想象）：src/features/agentStatus/useAgentStatus.ts（建行/删行通道）、e2e-tests/test.e2e.ts Agent Status describe（:1532 起，用例 2a :1615-1696、用例 2b :1713-1874）、e2e-tests/CLAUDE.md 测试与应用通信方式段。

【改写 1】用例 2a 语义反转（:1615-1696 静态行渲染）
- 行建模改后纯 shell 终端无行——「创建终端 → 初始扫描生成 🟡 行」反转为「创建终端 → agent-status-row 不出现（纯 shell 无行）」
- 用例 1（:1603 已断言「无运行中的 claude 会话」空态）保留作回归

【改写 2】用例 2b 流程适配（:1713-1874 动态四态）
- 删第 4 步「等待静态行出现」（:1777-1782）——首个 PreToolUse 信号文件到达即 hook 事件建行（断言 agent-status-row 出现且含 ⚡）
- Stop→✅、SessionEnd→行消失断言保留

【新增常驻 3 条（R2/R3/R4 变体防复发）】
1. R2 变体（切项目用量保持）：Node 端写假 transcript JSONL（合法 JSONL 且含 message.usage 四字段行——input_tokens/output_tokens/cache_read_input_tokens/cache_creation_input_tokens）→ 信号文件携真实 transcriptPath 建行 + usage 拉取（行含量化百分比）→ 切项目往返 → 用量数值保持（初始扫描携 transcriptPath 主动拉取）。后端 hooks_context_usage 真实解析（非 mock）——同时 L4 级覆盖 cache 口径全链路
2. R3 变体（SessionEnd 删行 + 切项目不复活）：hook 事件建行 → SessionEnd 信号 → 行消失 → 切项目往返 → 行仍不存在（claudeSession 已 null，初始扫描不建行）
3. R4 变体（会话终端关页签删行）：hook 事件建行 → __dockviewApi.removePanel(panel) → 行消失（remove 事件 + ref 稳定订阅——R4 原始竞态不重现）。注意：用 __dockviewApi.removePanel，不用 panel.close()（R4 原始探针教训：panel?.close is not a function）

【既有先例沿用】
- 信号文件照现有先例：Node 端原子写（.tmp → rename .json）到 ~/.slterminal/hooks-events/
- 切项目往返照用例 2b 既有模式（__slterm_e2e_addPage / __slterm_e2e_switchToPage）

【自查】e2e-tests/helpers.ts 无行建模逻辑复制（只装钩子）——无 DBG-8 类同型 bug；有问题报告，不修改

完成判据：本阶段不跑构建——build:e2e + wdio 由下一阶段全量测试 agent 执行。
`, { label: 'A:l4-e2e-cases' })

// === Phase 2: 构建与实跑（串行依赖：wdio 依赖 build:e2e 产出的二进制）===
phase('构建与实跑')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行 L4 门禁。两条命令串行依赖，按序执行（wdio 依赖 build:e2e 产出的二进制）：
1. npm run build:e2e
2. npm run wdio
构建与 wdio 实跑耗时长（分钟级），勿中止。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要（wdio 失败附失败用例名与错误信息摘要），勿贴完整输出。
`, { label: 'build:e2e + wdio' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的 build:e2e + wdio 执行结果，测试类断言据此判定（无需重跑）：
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

return { fixResult, testResult, verifyResult }
