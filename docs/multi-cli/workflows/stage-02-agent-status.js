// =====================================================================
// Stage 02 — 前端状态域
// 条目：MC-401~406、MC-107、MC-205/206、MC-410~414、MC-420~422 + D-02(claudeStatus 段)/D-04/D-06/D-12/D-14(agent.e2e 空态文案段)
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 02 分工表与实现要点）
// commit message：refactor(agent-status): 前端状态域去 claude 化——四态策略入 profile（MC-401~422）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 lib-status agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// 分工表补列（skill 纪律 5，脚本 prompt 触碰全集）：registry-xterm 补 src/types/hooks.ts
//   ——MC-205 可选字段 cliId?: string 落点（stages.md Stage 02 实现要点 1 要求，分工表漏列）；
//   同 agent 补 src/__tests__/e2e-gating-terminal.test.ts——checklist D-13 标 Stage 01/02 两段，
//   stages.md Stage 02 分工表漏列（E2E helper 门控断言随 useXterm 事件路径改造同步核对）。
// =====================================================================

export const meta = {
  name: 'stage-02-agent-status',
  description: 'Stage 02：前端状态域去 claude 化——四态策略入 profile（MC-401~422 + MC-107/205/206 + D-02/04/06/12/14）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（红线，触碰即返工）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
5. E2E 不得触碰用户真实 ~/.claude/（env 覆盖 + fixture 隔离）
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 02 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
1. 更名全表：ClaudeStatus → AgentStatus（lib/claudeStatus.ts → lib/agentStatus.ts）；ClaudeSessionInfo → AgentSessionInfo（+ 新增 cliId: string 字段）；claudeSession → agentSession；setClaudeSession → setAgentSession；useClaudeNotifications → useAgentNotifications。merge 语义 / null 清空 / undefined 不覆盖 / 缺 lastEventAt 自动填 / register 幂等保留旧值——全保留。「存在即运行中」二态模型不变。
2. MC-205 三级解析（一次写全，三消费点同一表达式形态）：payload.cliId ?? TerminalRegistry.get(panelId)?.agentSession?.cliId ?? CLAUDE_CLI_ID——本 Stage HookEventPayload 尚无 cliId 字段（Stage 03 后端加），在 src/types/hooks.ts 的 HookEventPayload 先加可选字段 cliId?: string（恒 undefined 向后兼容）；Stage 03 后端字段到达后自然生效。
3. 缺省回退常量：CLAUDE_CLI_ID 从 features/cliProfiles/profiles/claude/ import，通用层禁止写 "claude" 字符串字面量（AC-5 守卫兼容）。
4. HooksCapability 签名（profiles/claude/ 实现迁入）：eventToStatus(event, notificationType?) / classifyNotification(payload) / contextLimit = 200_000 / restartHint = 现状文案 / hasConfigEditor = true。
5. 本 Stage 订阅函数名仍为 onHookEvent、事件名仍为 "hook-event"（Stage 03 统一换 onAgentEvent / "agent-event"）——本 Stage 禁止提前更名。
【测试纪律】你不跑资源共享型测试；只做编译级检查 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 的中间态，忽略，只保证自己分工文件正确；真实执行由全量测试 agent 单点跑。除 lib-status 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（5 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'lib-status',
    prompt: `你负责 Stage 02 的 lib 迁移与 claude profile hooks 能力：MC-401、MC-422 + MC-214 前端半 + D-02（claudeStatus 段）+ test-inventory 就近登记（本 Stage 独占）。

【MC-401】src/lib/claudeStatus.ts → src/lib/agentStatus.ts：保留 AgentStatus 类型（由 ClaudeStatus 更名，四态值集不变）、STATUS_EMOJI、getStatusIcon；eventToStatus 与 ATTENTION_NOTIFICATION_TYPES 移出（迁入 profiles/claude/ 作 hooks.eventToStatus 实现）；lib 层不再含 claude 事件名字面量。
【MC-422】claude 类别判定知识迁入 src/features/cliProfiles/profiles/claude/（classifyNotification 实现）五映射：PermissionRequest → permission / Notification+permission_prompt → permission / StopFailure → error / PostToolUseFailure → error / Stop → done；行为零改动。
【MC-214 前端半】profiles/claude/ 的 capabilities.hooks 补全：eventToStatus（10 事件 + notificationType 子类型 + ATTENTION_NOTIFICATION_TYPES，原 claude-status 32 用例语义不丢）/ classifyNotification（五映射）/ contextLimit = 200_000 / restartHint = "hooks 改动需重启 claude 会话生效"（现状文案）/ hasConfigEditor = true；可拆 profiles/claude/strategies.ts。同步 src/features/cliProfiles/types.ts 的类型引用（ClaudeStatus → AgentStatus 随行）。
【D-02 claudeStatus 段】改 src/lib/index.ts：claudeStatus 导出名 → agentStatus。
【测试】删 src/__tests__/claude-status.test.ts → 新建 src/__tests__/agent-status-lib.test.ts（保留 lib 层四态/STATUS_EMOJI/getStatusIcon 用例）；扩 src/__tests__/cli-profile-claude.test.ts（eventToStatus 32 用例语义 + classifyNotification 五映射迁入，落点改此）。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：删 claude-status（32 语义迁入 cli-profile-claude + agent-status-lib）；增 agent-status-lib；改 cli-profile-claude（扩 hooks 策略用例）；改 terminal-registry / terminal-registry-subscribe / use-xterm-* / claude-history-model / claude-history-view / claude-history-hook（更名同步，用例数不变）；改 notifications（更名同步）；改 agent-status-view / agent-status-hook（更名 + 空态文案）；改 workspace-header-actions（agentSession 判定）；改 e2e agent.e2e（空态文案断言）。

文件清单（只许碰这些）：src/lib/claudeStatus.ts → src/lib/agentStatus.ts；改 src/lib/index.ts；改 src/features/cliProfiles/profiles/claude/index.ts（可拆新建 strategies.ts）、src/features/cliProfiles/types.ts；删 src/__tests__/claude-status.test.ts；新建 src/__tests__/agent-status-lib.test.ts；改 src/__tests__/cli-profile-claude.test.ts；改 .claude/test-inventory.md。`,
  },
  {
    label: 'registry-xterm',
    prompt: `你负责 Stage 02 的注册表与终端事件路径：MC-402、MC-107、MC-403、MC-205/206（三级解析落地）、D-12 + 更名连锁同步。禁止改 .claude/test-inventory.md（归 lib-status agent）。

【MC-402】改 src/panels/terminal/TerminalRegistry.ts：ClaudeSessionInfo → AgentSessionInfo——字段 sessionId/transcriptPath/matchedCommand/status/lastEventAt 保留 + 新增 cliId: string；claudeSession → agentSession、setClaudeSession → setAgentSession（merge 语义 / null 清空 / undefined 不覆盖 / 缺 lastEventAt 自动填 / register 幂等保留旧值——全保留）；「存在即运行中」二态模型不变。
【MC-107】改 src/panels/terminal/useCommandDetection.ts：OSC 133 C 命中后 setAgentSession(panelId, { cliId: profile.id, matchedCommand })——cliId 从匹配到的 profile 取。
【MC-403】改 src/panels/terminal/useXterm.ts（hook-event 消费，约 348-357 行）：按 MC-205 三级解析取 profile → profile.capabilities?.hooks?.eventToStatus(event, notificationType)；无 hooks 能力 profile → console.warn + 跳过；SessionEnd 清图标、Exit 清会话分支语义不变；setAgentSession 携 sessionId/transcriptPath/status + payload 空串归一 || undefined 防御保留。订阅函数名本 Stage 仍为 onHookEvent（Stage 03 统一更名，禁止提前）。
【MC-205 落地】src/types/hooks.ts 的 HookEventPayload 加可选字段 cliId?: string（恒 undefined 向后兼容，Stage 03 后端加字段后生效）；三级解析表达式：payload.cliId ?? TerminalRegistry.get(panelId)?.agentSession?.cliId ?? CLAUDE_CLI_ID。
【MC-206】未知 cliId（未注册）→ console.warn + 跳过（不建行/不置图标/不通知），不抛异常。
【更名连锁】src/features/claudeHistory/historyModel.ts 与 HistorySessionList.tsx 仅做 claudeSession → agentSession 字段更名同步（复合键改造属 Stage 05，本 Stage 不动）；全仓 claudeSession/setClaudeSession/ClaudeSessionInfo 引用同步（含 L3 production-osc、E2E 若引用——Stage 01 已改的 production-osc 复刻段随行）。
【D-12】改 src/__tests__/terminal-registry-subscribe.test.ts（register/remove/sessionChange 订阅测试更名同步）。
【测试同步】改 src/__tests__/{terminal-registry.test.ts, use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, claude-history-model.test.ts, claude-history-view.test.tsx, claude-history-hook.test.tsx}；核对 src/__tests__/e2e-gating-terminal.test.ts（D-13 Stage 02 段：E2E helper 门控断言随 useXterm 事件路径改造同步）；新增/调整用例：setAgentSession merge 语义 / 三级解析三分支（显式经可选字段注入 / 反查 / 缺省）/ eventToStatus 经 mock 或测试 profile 被真实调用（入参断言）/ 无 hooks 能力 warn+跳过 / 未知 cliId warn+跳过 / SessionEnd 清图标、Exit 清会话保留。

文件清单（只许碰这些）：改 src/panels/terminal/{TerminalRegistry.ts, useXterm.ts, useCommandDetection.ts}；改 src/types/hooks.ts（仅加 cliId 可选字段）；改 src/features/claudeHistory/{historyModel.ts, HistorySessionList.tsx}（仅字段更名）；改 src/__tests__/{terminal-registry.test.ts, terminal-registry-subscribe.test.ts, use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, claude-history-model.test.ts, claude-history-view.test.tsx, claude-history-hook.test.tsx}；改 test/terminal/production-osc.test.ts（若复刻段含 claudeSession 引用）。`,
  },
  {
    label: 'notifications',
    prompt: `你负责 Stage 02 的通知调度迁移：MC-420、MC-421、MC-404（通知侧）、D-04。禁止改 .claude/test-inventory.md（归 lib-status agent）。

【MC-420】src/features/notifications/useClaudeNotifications.ts → useAgentNotifications.ts：classifyEvent 纯函数两段分解——通用门控（失焦门控/去重/seenRef 截断，CLI 无关保留模块内）+ 类别判定委托 profile.capabilities?.hooks?.classifyNotification(payload)（按 MC-205 三级解析取 profile：payload.cliId ?? TerminalRegistry.get(panelId)?.agentSession?.cliId ?? CLAUDE_CLI_ID；无 hooks 能力 → 不通知；未知 cliId → console.warn + 跳过不通知）；返回类别 permission/error/done/null 语义不变。
【MC-421】toast 正文项目名反查（panelId → parseTerminalPageId → useProjects）与任务栏闪烁零改动；类别 emoji 用 CATEGORY_EMOJI。
【MC-404】CATEGORY_EMOJI（🔐❌✅ 通知类别）定义于通知模块内，与 src/lib/agentStatus.ts 的 STATUS_EMOJI（⚡🟡✅❌ 会话状态）不合并——值有重叠但语义不同，两处注释互相指引（lib-status agent 会在 agentStatus.ts 写对向注释，你写通知侧注释即可）。
【D-04】改 src/features/notifications/index.ts barrel 导出名；改 src/App.tsx 的 NotificationListener import 路径同步。
【测试】改 src/__tests__/notifications.test.ts：更名同步 + 类别判定委托 profile 的用例（mock/测试 profile classifyNotification 被真实调用、无 hooks 能力不通知）。

文件清单（只许碰这些）：src/features/notifications/useClaudeNotifications.ts → src/features/notifications/useAgentNotifications.ts；改 src/features/notifications/index.ts；改 src/App.tsx；改 src/__tests__/notifications.test.ts。`,
  },
  {
    label: 'agent-status-view',
    prompt: `你负责 Stage 02 的 AgentStatus 行与视图：MC-410、MC-411、MC-412、MC-413、MC-414 + 过渡形态清扫。禁止改 .claude/test-inventory.md（归 lib-status agent）。

【MC-410】改 src/features/agentStatus/useAgentStatus.ts：行建模双通道建行/三通道删行语义不变；hook 事件通道建行按 MC-205 三级解析（payload.cliId ?? TerminalRegistry.get(panelId)?.agentSession?.cliId ?? CLAUDE_CLI_ID）写入行 cliId；OSC 133 通道建行经 setAgentSession 的 sessionChange 自然驱动；初始扫描携 transcriptPath 拉 contextUsage 保留（ipc 调用签名本 Stage 不变，Stage 03 泛化）。
【MC-411】改 src/features/agentStatus/AgentStatusRow.tsx：cliIconRegistry 残留（Stage 01 过渡形态 get(CLAUDE_CLI_ID)）→ 按 row.cliId 查 cliProfileRegistry.get(row.cliId)?.iconSrc；图标列 40px flex 簇 /「logo 仅随 emoji」/ 空列占位布局语义不变；OSC 133-only 行同样有 cliId（logo 可展示）；未注册 cliId → 无 logo 不报错。
【MC-412】AgentStatusRow.tsx 用量计算：(inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / contextLimit——contextLimit 由 cliProfileRegistry.get(row.cliId)?.capabilities?.hooks?.contextLimit 提供，缺失 → 用量 "--"（现状语义保留）。
【MC-413】删 src/features/agentStatus/consts.ts（CLAUDE_CONTEXT_LIMIT 退役）；行组件/hook 残留 claude 命名（类型/注释/变量）全面去 claude 化，行为零改动。
【MC-414】改 src/features/agentStatus/AgentStatusView.tsx 空态文案：「无运行中的 claude 会话」→「无运行中的编码 CLI 会话」（本域唯一用户可见文案变动）；E2E 红线逐字保留：data-e2e="agent-status-view" / agent-status-row / 标题栏 "AGENT STATUS" /「选择一个项目」。
【测试】改 src/__tests__/{agent-status-view.test.tsx, agent-status-hook.test.ts}（更名 + 空态文案 + 行 cliId 用例）；改 e2e-tests/agent.e2e.ts（空态文案断言同步，其余红线断言逐字不动）。

文件清单（只许碰这些）：改 src/features/agentStatus/{useAgentStatus.ts, AgentStatusRow.tsx, AgentStatusView.tsx}；删 src/features/agentStatus/consts.ts；改 src/__tests__/{agent-status-view.test.tsx, agent-status-hook.test.ts}；改 e2e-tests/agent.e2e.ts。`,
  },
  {
    label: 'f8-rename',
    prompt: `你负责 Stage 02 的 F8 重命名禁用判定同步：MC-405、MC-406（核对）、D-06。禁止改 .claude/test-inventory.md（归 lib-status agent）。

【MC-405】改 src/workspace/PageDockviewHost.tsx（createGetContextMenu 内 F8 禁用判定）：TerminalRegistry.get(panelId)?.agentSession != null（字段更名后语义不变）——任何 CLI 活跃会话均禁用重命名；OSC 133-only 会话（matchedCommand-only）同样命中。
【MC-406 核对】PageDockviewHost.tsx 的 DefaultTab 渲染链（tabIcon emoji / tabLogo 16×16 / tabIcon && tabLogo 双条件 / inactive 双清）零改动——值来源变化仅在生产侧（OSC 133 C logo 来自 profile.iconSrc，Stage 01 已落）。仅核对，不改渲染逻辑。
【D-06】改 src/__tests__/workspace-header-actions.test.tsx：F8 禁用判定的 claudeSession 引用更名 agentSession 同步。

文件清单（只许碰这些）：改 src/workspace/PageDockviewHost.tsx；改 src/__tests__/workspace-header-actions.test.tsx。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-7 并行收集，8 最后单独串行）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。命令清单：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npm run test:l3
8. npm run e2e
执行纪律：命令 1-7 相互独立，并行启动执行，收集全部结果；待 1-7 全部结束后，再单独串行执行命令 8（npm run e2e 内部 = build:e2e + wdio 串行；它会重新构建并占用 slterminal.exe，与其他命令并行会构建失败——禁拆分、禁与其他命令并行）。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
