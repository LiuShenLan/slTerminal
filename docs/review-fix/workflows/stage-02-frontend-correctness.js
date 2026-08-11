// =====================================================================
// Stage 02 — 前端正确性族（ZQ-1/ZQ-2/ZQ-3/ZQ-4/ZQ-6/ZQ-7）
// =====================================================================
// 真值源: docs/review-fix/checklist.md + docs/review-fix/stages.md（Stage 02 节）
// 断言清单: docs/review-fix/workflows/verify/stage-02.md（本脚本与 fix-loop 共用同一真值源）
// 跨边界契约（stages.md 契约 3/4 原文，写死——两 agent 不各自推断）：
//   契约 3 keyOf：historyModel.ts 导出 export function keyOf(cliId: string | null | undefined, sessionId: string): string
//     ——内部 (cliId ?? CLAUDE_CLI_ID) 回退 + 两侧 replaceAll("|", "\\|") 转义 → 拼接 a竖线b
//   契约 4 resolvePayloadCliId：新建 src/panels/terminal/resolvePayloadCliId.ts
//     export function resolvePayloadCliId(payload: AgentEventPayload): string
//     ——payload.cliId?.trim() || TerminalRegistry.get(payload.panelId)?.agentSession?.cliId || CLAUDE_CLI_ID
// fix-loop args: { stage: 2, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-02.md',
//   constraints: stages.md「禁区」六条原样 }
//   ——本 Stage 无 L4 门禁，testCommands 缺省即可
// =====================================================================

export const meta = {
  name: 'stage02-frontend-correctness',
  description: 'Stage 02: 前端正确性族——keyOf/resolvePayloadCliId 单点 + null 建行语义（ZQ-1~4/6/7）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（不可违背）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——改 payload 键时勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——改 ops.rs 时勿削弱（is_symlink 是加防御不是松校验）
5. E2E 不得触碰用户真实 ~/.claude/——AQ-4 正是强化此防线，改 run-wdio.cjs 时勿引入新降级路径
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）——改 helpers.ts 时勿动
背景：先读 docs/review-fix/checklist.md 中你负责 ID 的条目原文 + docs/review-fix/stages.md Stage 02 节的实现要点，再动手。
本 Stage 纪律：
- 并行期间禁止跑资源共享型测试（cargo test / npm run e2e 由专门 agent 统一跑）——编译级检查 npx tsc --noEmit；允许跑自己改动的单文件 vitest（npx vitest run <文件>，纯 jsdom 无共享资源）
- .claude/test-inventory.md 归 composite-key 单点负责——其余 agent 禁止触碰（你的用例变化由它按 prompt 写明的代登记项同步）`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'composite-key',
    prompt: `你负责 ZQ-1、ZQ-7（keyOf 复合键单点 + 五处统一）：

【契约 3（写死，不各自推断）】keyOf 落点与签名：
- src/features/agentHistory/historyModel.ts 导出：export function keyOf(cliId: string | null | undefined, sessionId: string): string
- 内部：cliId 为 null/undefined 时回退 CLAUDE_CLI_ID + cliId 与 sessionId 两侧分别 replaceAll 竖线为「反斜杠+竖线」两字符 → 拼接为 a竖线b 形态返回
- 消费方比较/查键一律经 keyOf——生产消费同函数即口径一致（转义对存量键零变化：现状 cliId/sessionId 均不含竖线，纯防御未来）

【ZQ-1】消费方回退缺失：HistorySessionList.tsx:278 的 rowFlags 取键、:196/:206 的 findPanelForSession 比较键统一改经 keyOf（入参 cliId 缺省回退在 keyOf 内部完成，消费方不再各自 ?? CLAUDE_CLI_ID）；findPanelForSession 入参侧（scan 数据 cliId）同样经 keyOf 归一。
【ZQ-7】三处拼接方同步改经 keyOf：historyModel.ts:138（deriveActiveSessionStatuses）、AgentStatusView.tsx:133（titleBySessionId）与 :144（displayRows）。

测试：
- src/__tests__/agent-history-model.test.ts 新增 keyOf 用例（cliId 缺省回退、cliId/sessionId 含竖线时生产消费两侧键一致）
- src/__tests__/agent-history-view.test.tsx 随行（断言经 keyOf 的键形态）
就近同步：src/features/agentHistory/CLAUDE.md（复合键段：keyOf 单点 + 回退 + 转义口径）。
.claude/test-inventory.md 由你单点负责——登记：keyOf 新用例 + 代 event-pipeline 登记（空串回退 ×3、null 建行、Exit 清图标用例）+ 代 restore-id 登记（同毫秒相异用例）。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/agent-history-model.test.ts src/__tests__/agent-history-view.test.tsx 通过。`,
  },
  {
    label: 'event-pipeline',
    prompt: `你负责 ZQ-2、ZQ-3、ZQ-6（agent-event 管道三修复）：

【契约 4（写死，不各自推断）】resolvePayloadCliId 单点：
- 新建 src/panels/terminal/resolvePayloadCliId.ts：export function resolvePayloadCliId(payload: AgentEventPayload): string
- 实现：payload.cliId?.trim() || TerminalRegistry.get(payload.panelId)?.agentSession?.cliId || CLAUDE_CLI_ID
- 空串/仅空白/null/undefined 同等回退（ZQ-2）；MC-205 三级解析语义不变

【ZQ-2】三处 ?? 链改经 helper：
- src/panels/terminal/useXterm.ts:358-361
- src/features/agentStatus/useAgentStatus.ts:134-137
- src/features/notifications/useAgentNotifications.ts:65-68（classifyEvent 纯函数导出，helper import 无循环——TerminalRegistry 不 import notifications）

【ZQ-3】null 映射事件建行但 status null（决策 2，用户拍板）：
- src/features/agentStatus/useAgentStatus.ts:194 建行 status: newStatus ?? "attention" → status: newStatus（AgentSessionRow.status 类型随行放宽含 null）
- 更新已有行 :177 的 null 不覆盖逻辑不动
- 注释注明决策：「null 映射事件建行但 status null——感知存活（SessionStart 丢失场景）且不误标 attention（ZQ-3 决策 2）」
- 核实 AgentStatusRow 渲染 status null = 无图标（现状「icon 为空仍渲染空列占位」已兼容——Read 确认，若不兼容则最小修补并说明）

【ZQ-6】useXterm.ts:391 清图标条件扩为 payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT——与 :376 删 agentSession 的双事件判定对齐。

测试（各加用例）：
- src/__tests__/agent-status-hook.test.ts：空串 cliId 回退（ZQ-2）+ null 映射事件首达建行 status=null 无图标（ZQ-3，附「SessionStart 丢失感知存活」场景注释）
- src/__tests__/use-xterm-lifecycle.test.ts：空串 cliId 回退（ZQ-2）+ Exit 事件清图标（ZQ-6）
- src/__tests__/notifications.test.ts：空串 cliId 回退（ZQ-2）
就近同步：src/features/agentStatus/CLAUDE.md、src/features/notifications/CLAUDE.md、src/panels/CLAUDE.md（useXterm 行——resolvePayloadCliId 单点 + 清图标双事件）。
禁止触碰 .claude/test-inventory.md 与 src/features/agentHistory/CLAUDE.md——归 composite-key 单点负责。
L3 复刻段核对：本改动段（agent-event 订阅、建行逻辑）不在 test/terminal/production-osc.test.ts 复刻范围（OSC 52/133/8）——确认零波及并在报告中说明。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/agent-status-hook.test.ts src/__tests__/use-xterm-lifecycle.test.ts src/__tests__/notifications.test.ts 通过。`,
  },
  {
    label: 'restore-id',
    prompt: `你负责 ZQ-4（restoreSession panelId 防毫秒碰撞）：

- 位置：src/features/agentHistory/restoreSession.ts:133。
- 问题：panelId 基于 Date.now()，restoring 仅阻塞并发不阻塞串行——两次恢复落在同一毫秒产生相同 panelId，恢复命令可能被注入错误终端。
- 修复：模块级自增计数器 let restoreSeq = 0，panelId 改为 terminal-页面id-毫秒时间戳-前置递增序号 四段拼接（template：terminal-\${targetPageId}-\${Date.now()}-\${++restoreSeq} 形态——即现状三段基础上追加第四段自增序号）。
- 测试 src/__tests__/agent-history-restore.test.ts：mock Date.now 同值，连续两次恢复断言 panelId 相异。

禁止触碰 .claude/test-inventory.md 与 src/features/agentHistory/CLAUDE.md——归 composite-key 单点负责（你的新用例由它代登记）。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/agent-history-restore.test.ts 通过。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const TEST_COMMANDS = [
  'npx tsc --noEmit',
  'npx eslint src/',
  'cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings',
  'cargo fmt --manifest-path src-tauri/Cargo.toml -- --check',
  'cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1',
  'npm test',
  'npm run test:l3',
]
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。
执行前确认：无运行中的 slterminal.exe（Windows 文件锁会致 cargo 链接 os error 5）。
以下命令相互独立，并行启动执行，收集全部结果：
${TEST_COMMANDS.map((c, i) => `${i + 1}. ${c}`).join('\n')}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
