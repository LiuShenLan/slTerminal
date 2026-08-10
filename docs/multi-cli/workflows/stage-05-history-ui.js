// =====================================================================
// Stage 05 — 前端历史聚合 UI
// 条目：MC-310/311/312(核对)/313/314/315/316/317 + D-05/D-14(history.e2e 恢复注入段核对)
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 05 分工表与实现要点）
// commit message：refactor(agent-history): 前端历史聚合 UI 泛化 + 复合键（MC-310~317）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 host-and-tests agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// =====================================================================

export const meta = {
  name: 'stage-05-history-ui',
  description: 'Stage 05：前端历史聚合 UI 泛化 + 复合键 cliId|sessionId（MC-310~317 + D-05/14）',
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
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 05 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
1. 复合键格式：cliId|sessionId（竖线分隔）三处——deriveActiveSessionStatuses（historyModel.ts:123-137）、findPanelForSession（HistorySessionList.tsx:192-204）、titleBySessionId（AgentStatusView.tsx:126-141）；transcriptPath basename 回退兼容保留（旧数据无 cliId 按 CLAUDE_CLI_ID——常量引用非字面量）。
2. HistoryCapability 签名：supportsFork / buildResumeCommand(session) / buildRestoreInput(session, { fork })；claude = supportsFork true、buildRestoreInput = "claude --resume <id>" + fork 追加 " --fork-session" + \\r 结尾。
3. 更名全表：features/claudeHistory/ → features/agentHistory/；ClaudeHistorySections → AgentHistorySections；useClaudeHistory → useAgentHistory；barrel index.ts 同步；宿主 AgentStatusView.tsx import 同步；data-e2e 选择器与空态文案红线不动。
4. 跨边界依赖形态：聚合 UI 调 profile.capabilities?.history?.buildResumeCommand(...) 可选链——类型 Stage 01 已定义，claude 实现由 claude-history-cap agent 同 Stage 交付，编译互不阻塞。
5. 缺省回退常量：CLAUDE_CLI_ID 从 features/cliProfiles/profiles/claude/ import，通用层禁 "claude" 字面量（AC-5 守卫兼容）。
【测试纪律】你不跑资源共享型测试；只做编译级检查 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 中间态，忽略；真实执行由全量测试 agent 单点跑。除 host-and-tests 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'features-migrate',
    prompt: `你负责 Stage 05 的 features 目录迁移与委托改造：MC-310、MC-311、MC-313（historyModel/HistorySessionList 两处）、MC-315、MC-316、MC-317、MC-312（核对）。禁止改 .claude/test-inventory.md（归 host-and-tests agent）。

【MC-310】src/features/claudeHistory/ → src/features/agentHistory/（9 文件：index.ts、ClaudeHistorySections.tsx→AgentHistorySections.tsx、HistorySessionList.tsx、HistorySessionRow.tsx、SessionActionDialog.tsx、historyContextMenu.ts、historyModel.ts、useClaudeHistory.ts→useAgentHistory.ts、restoreSession.ts）——聚合 UI 泛化为 CLI 无关；组件/Hook/类型/注释全面去 claude 化（MC-5）；barrel index.ts 同步。
【MC-311】HistorySessionRow.tsx：Stage 01 过渡形态 cliProfileRegistry.get(CLAUDE_CLI_ID) → 按 session.cliId 查 profile.iconSrc；logo 仍「仅随 status emoji」（孤儿 ✗ 后不加图）；未注册 cliId → 无 logo 不报错。
【MC-312 核对】historyModel.ts groupByCwd 分组维度保持 cwd 不变；同目录不同 CLI 同组、行级 logo 区分；「全部项目」区聚合全部 provider 数据（scan 无参聚合直达 UI，无前端二次过滤）。仅核对，零改动。
【MC-313】historyModel.ts deriveActiveSessionStatuses + HistorySessionList.tsx findPanelForSession：复合键 cliId|sessionId（防跨 CLI sessionId 理论冲突）；transcriptPath basename 回退兼容保留（旧数据无 cliId 按 CLAUDE_CLI_ID 常量）。
【MC-315】restoreSession.ts：四步框架零改动（项目入列/页面/切换/终端注入；防重入/失败 toast 保留）；第 4 步注入内容 = profile.capabilities?.history?.buildRestoreInput(session, { fork })（claude = claude --resume <id> + fork 追加）；addPanel title: "claude" 字面量 → profile.tabTitle；文件内不得出现 "claude" 字符串字面量。
【MC-316】historyContextMenu.ts：buildResumeCommand → profile.capabilities?.history?.buildResumeCommand(session) 委托；操作矩阵禁用态（孤儿/无 cwd 禁分支恢复、运行中禁删除）通用语义保留；supportsFork=false 不展示「分支恢复」；cwd 单引号限制（PowerShell '' 转义缺失）注释留 claude profile 的 buildResumeCommand 实现内（由 claude-history-cap agent 写）。
【MC-317】HistorySessionList.tsx：删除（ask → agent_history_delete(session.cliId, session.sessionId) → removeLocal 不重扫）与双击三分派（普通→恢复 / 孤儿·无 cwd→无操作 / 运行中→SessionActionDialog）语义不变。

文件清单（只许碰这些）：src/features/claudeHistory/ 9 文件 → src/features/agentHistory/（含更名）。`,
  },
  {
    label: 'claude-history-cap',
    prompt: `你负责 Stage 05 的 claude profile history 能力：profiles/claude/ 的 capabilities.history 实现 + 用例。禁止改 .claude/test-inventory.md（归 host-and-tests agent）。

【history 能力】改 src/features/cliProfiles/profiles/claude/（index.ts 或拆 strategies.ts）：capabilities.history = { supportsFork: true, buildResumeCommand(session), buildRestoreInput(session, { fork }) }。
  - buildResumeCommand：现状 historyContextMenu.ts:51-54 的 claude 恢复命令逻辑原样迁入（含 cwd 单引号限制——PowerShell '' 转义缺失——注释留在实现内）。
  - buildRestoreInput：现状 restoreSession.ts:130 字面量逻辑原样迁入——输出必须与现状逐字一致：claude --resume <id> + fork 追加 " --fork-session" + \\r 结尾（E2E history.e2e 恢复编排用例应零改动通过；断言漂移即实现有误）。
  - 实现内写 "claude" 字面量合法（profiles/claude/ 是 claude 合法领地，D11）。
【测试】扩 src/__tests__/cli-profile-claude.test.ts：history 策略用例——buildResumeCommand 输出形态 / buildRestoreInput 的 resume 命令 / fork 追加 / \\r 结尾（断言与现状逐字一致）；supportsFork=true 断言。

文件清单（只许碰这些）：改 src/features/cliProfiles/profiles/claude/index.ts（或新建 strategies.ts）；改 src/__tests__/cli-profile-claude.test.ts。`,
  },
  {
    label: 'host-and-tests',
    prompt: `你负责 Stage 05 的宿主同步与测试迁移：MC-314、D-05 + 测试迁移 + history.e2e 核对 + test-inventory 就近登记（本 Stage 独占）。

【MC-314】改 src/features/agentStatus/AgentStatusView.tsx：titleBySessionId 键改 cliId|sessionId 复合键；claude /rename → custom-title → scan 联动语义保留（数据路径在 claude provider）；import 同步（ClaudeHistorySections → AgentHistorySections、useClaudeHistory → useAgentHistory，D-05）。
【D-05】src/features/agentStatus/AgentStatusView.tsx 的 import 更名同步（barrel 由 features-migrate agent 改，你只改 import 引用）。
【测试迁移】src/__tests__/claude-history-{model.test.ts, hook.test.tsx, view.test.tsx, restore.test.ts, row.test.tsx, action-dialog.test.tsx} → agent-history-{model, hook, view, restore, row, action-dialog}.test.ts(x)——断言同步（目录更名/复合键/委托 profile 策略）；supportsFork 菜单显隐用例（mock 或局部注册测试 profile supportsFork=false → 无「分支恢复」项——允许用例内局部注册测试 profile）；restoreSession 注入内容 L2 断言逐字一致（= claude profile 策略输出）；复合键三处用例（活跃状态 derive / findPanelForSession / 标题覆盖）；transcriptPath basename 回退用例保留。改 src/__tests__/agent-status-view.test.tsx（import 断言同步）。
【D-14 恢复注入段核对】核对 e2e-tests/history.e2e.ts：恢复注入断言与 claude profile buildRestoreInput 输出逐字一致——应零改动通过；若断言漂移即 features-migrate / claude-history-cap 实现有误，在报告中指出（本文件预期零改动，仅必要断言同步时可改）。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：claude-history-{model,hook,view,restore,row,action-dialog} → agent-history-* 更名（用例数不变 + supportsFork 显隐新增用例）；cli-profile-claude 扩 history 策略用例；agent-status-view 断言同步（用例数不变）；history.e2e 核对（预期零变动）。

文件清单（只许碰这些）：改 src/features/agentStatus/AgentStatusView.tsx；src/__tests__/claude-history-{model.test.ts, hook.test.tsx, view.test.tsx, restore.test.ts, row.test.tsx, action-dialog.test.tsx} → src/__tests__/agent-history-* 对应更名；改 src/__tests__/agent-status-view.test.tsx；核对 e2e-tests/history.e2e.ts（预期零改动）；改 .claude/test-inventory.md。`,
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
