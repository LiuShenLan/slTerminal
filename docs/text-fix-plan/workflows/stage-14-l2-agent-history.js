// =====================================================================
// Stage 14 L2-agent/history：跨模块同源
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-14.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更，如 classifyEvent 导出）；其余只改测试
// =====================================================================

export const meta = {
  name: 'stage14-l2-agent-history',
  description: 'L2 agent/history 四态同源回退/classifyEvent 表驱动/恢复守卫补齐',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1——本 Stage 不涉，仅作提示
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更：classifyEvent 导出为纯函数），其余一律只改测试。并行 agent 文件零重叠（ah-notify 碰 notifications/claude-history-model 测试 + useClaudeNotifications.ts（classifyEvent 导出）；ah-view 碰 agent-status-view/claude-history-{restore,hook,view,row,action-dialog} 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'ah-notify',
    prompt: `你负责 NAH-01、NAH-03、NAH-04，触碰文件：src/__tests__/notifications.test.ts、src/__tests__/claude-history-model.test.ts、src/features/notifications/useClaudeNotifications.ts（classifyEvent 导出，D2）。逐 ID 对照 checklist 原文实施：

【NAH-01】deriveActiveSessionStatuses sessionId 缺失回退未覆盖。位置 src/features/claudeHistory/historyModel.ts:131。注册表条目 claudeSession: { sessionId: null, transcriptPath: "C:/x/abc.jsonl", status: "working" }，断言 deriveActiveSessionStatuses().get("abc") === "working"（basename 去 .jsonl 回退）。

【NAH-03】useClaudeNotifications classifyEvent 表驱动缺失。位置 useClaudeNotifications.ts:76,131,139,143。导出 classifyEvent（或拆纯函数，D2 最小可测性重构），事件 × notificationType 表驱动断言返回类别 + toast 触发与否 + 标题/正文：PermissionRequest→permission、Notification 两型（permission_prompt→permission / 其他→null 或对应类别）、Stop→done、StopFailure/PostToolUseFailure→error、未识别→null。

【NAH-04】通知去重缓存 200→100 截断未覆盖。位置 useClaudeNotifications.ts:132-133。构造 250 个不同事件推进时间，断言缓存截断为 100；最旧事件重新触发应再弹 toast。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'ah-view',
    prompt: `你负责 NAH-05、NAH-06、NAH-07、NAH-08、NAH-09、NAH-10、NAH-11，触碰文件：src/__tests__/agent-status-view.test.tsx、claude-history-{restore,hook,view,row,action-dialog}.test.ts(x)。逐 ID 对照 checklist 原文实施：

【NAH-05】AgentStatusRow 行 2 未断言。位置 src/features/agentStatus/AgentStatusRow.tsx:65-66,50。渲染完整 usage 行，断言 outputTokens 文本与 formatRelativeTime 相对时间出现。

【NAH-06】AgentStatusView 标题覆盖用 mock history。位置 AgentStatusView.tsx:118。集成测试：真实 useClaudeHistory（或受控数据）含 rename 后 title，断言活跃区行标题被覆盖；无匹配 sessionId 回退原标题。

【NAH-07】restoreSession 防重入 / cwd null 守卫未进入。位置 src/features/claudeHistory/restoreSession.ts:34,36。①同步连调两次断言四步编排仅执行一次；②cwd: null 断言抛 "cwd required"。

【NAH-08】useClaudeHistory.scan generation 竞态未覆盖。位置 useClaudeHistory.ts:35,60。首次 scan 延迟 resolve + 二次立即 resolve，断言 sessions 来自第二次。

【NAH-09】HistorySessionList 默认折叠 / 右键回调未完整覆盖。①断言 expandedGroups 初始为空、点击组标题后含该组 key；②右键触发断言 onCopy/onFork/onDelete 回调参数（session 对象正确、fork 标志 true）。

【NAH-10】HistorySessionRow 图标优先级未覆盖。位置 HistorySessionRow.tsx:50。status="working" && orphan=true 断言渲染 ⚡ 而非 ✗。

【NAH-11】SessionActionDialog 空 actions 防御未覆盖。位置 SessionActionDialog.tsx:42。actions={[]} 断言不渲染弹窗。

完成后报告：每项改动摘要 + 修改文件清单。`,
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
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-14.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage14 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-14.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
