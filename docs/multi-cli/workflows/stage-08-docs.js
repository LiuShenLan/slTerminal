// =====================================================================
// Stage 08 — 文档同步（固定末位）
// 条目：MC-8、MC-109、MC-110、MC-318、MC-223（CLAUDE.md 注明）、AC-6（终验文档一致性，定义见 spec 00 §5）
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 08 分工表与实现要点）
// commit message：docs(multi-cli): 文档同步——模块索引/CLAUDE.md/test-inventory（MC-8/109/110/318）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样 + 「本 Stage 只改文档，禁改任何代码与测试」
// test-inventory：归 root-docs agent（全量对齐核对），其余 agent 禁改。
// =====================================================================

export const meta = {
  name: 'stage-08-docs',
  description: 'Stage 08：文档同步——模块索引/各模块 CLAUDE.md/test-inventory/CONTEXT.md（MC-8/109/110/318/223 + AC-6）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
本 Stage 特殊纪律：只改文档（.md），禁改任何代码与测试文件。
禁区（红线，触碰即返工）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
5. E2E 不得触碰用户真实 ~/.claude/（env 覆盖 + fixture 隔离）
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 08 实现要点，再动手。
【文档纪律（写死）】
1. 文档描述须对照当前真实代码核实（Read/Grep 验证），防文档撒谎——文件表逐行 Glob 命中磁盘实态。
2. MC-109 改写红线：机制注释中「供 claude 取消」「Ink 据此换行」等触发点描述保留（历史动机如实记录）；仅「定制/专为 claude」类归属表述改写为「终端平台能力（设计动机 Ink 系 TUI，对全部子进程生效）」。
3. MC-318 记录形态：agentHistory CLAUDE.md「已知限制」段两条（组键漂移——expandedGroups 键随组内最大 mtime 会话漂移 / 历史区相对时间无 ticker），注明「规格确认不修（决策 6）」。
4. MC-223 注明形态：panels/hooksConfig 与 features/hooksConfig 的 CLAUDE.md 注明「claude 专属编辑器」（claude hooks 协议知识不抽象、文件物理位置保留——决策 2）。
5. 术语与终态命名一致：agent-event / agent_hooks_* / agent_history_* / AgentEventPayload / AgentHistorySession / features/agentHistory / cliProfiles——禁用旧名（hook-event / hooks_* / claude_history_* / claudeHistory）。
【测试纪律】你不跑任何测试；真实执行由全量测试 agent 单点跑。除 root-docs 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（4 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'root-docs',
    prompt: `你负责 Stage 08 的根文档：MC-8（根文件段）+ test-inventory 全量对齐。

【根 CLAUDE.md】改 .claude/CLAUDE.md：
  - 模块索引表：增 src/features/cliProfiles、src/features/agentHistory、src-tauri/src/agent_history 三行（职责/入口/详情链接按表格式）；删 src/features/claudeHistory、src-tauri/src/claude_history 两行；受影响的既有行（src/ipc、src/types、src/panels、src/features/agentStatus、src/features/notifications、src-tauri/src/hooks 等职责描述含旧命名的）同步终态命名。
  - 「需求编号索引」段：补 MC 家族说明（multi-cli profile 重构——阶段项目代号先例，免逐条登记，指向 docs/multi-cli/checklist.md）。
  - 模块清单/硬性约束引用处对照终态核实。
【CONTEXT.md】改 CONTEXT.md：术语核对——「CLI profile」等词条已按规格修订，核对与终态一致（cliId / profile 注册表 / agent-event / agent_history 等词条与代码终态命名一致）。
【test-inventory 全量对齐】改 .claude/test-inventory.md：各 Stage 已就近登记，你做总数核对——L1 用例数（cargo test 输出统计行）、L2（vitest 输出）、L3、L4 spec 条目数与文档计数一致；新测试文件（cli-profile-registry / cli-profile-claude / agent-status-lib / ipc-agent-hooks-contract / ipc-agent-history-contract / agent-history-* / mock-cli-profile / no-claude-literals / mockcli.e2e）登记完整，旧名（tab-title-registry / tab-rules / cli-icons / claude-status / ipc-hooks-contract / ipc-claude-history-contract / claude-history-*）无残留。

文件清单（只许碰这些）：改 .claude/CLAUDE.md、CONTEXT.md、.claude/test-inventory.md。`,
  },
  {
    label: 'frontend-module-docs',
    prompt: `你负责 Stage 08 的前端模块 CLAUDE.md：MC-8（前端模块段）+ MC-223（features/hooksConfig 段）+ MC-318。不含 src/panels/CLAUDE.md（归 backend-module-docs）。禁止改 .claude/test-inventory.md（归 root-docs agent）。

【新建】src/features/cliProfiles/CLAUDE.md（子文件模板：职责 → 架构决策（关键约束）→ 文件表 → 测试模式——含 profile 接口契约 / CLAUDE_CLI_ID 常量约定 / AC-5 守卫指向）；src/features/agentHistory/CLAUDE.md（替代 claudeHistory/CLAUDE.md——删旧建新；含复合键 cliId|sessionId / profile 策略委托 / MC-318「已知限制」段两条，注明「规格确认不修（决策 6）」）。
【更名同步】src/features/agentStatus/CLAUDE.md、src/features/notifications/CLAUDE.md、src/lib/CLAUDE.md、src/ipc/CLAUDE.md、src/types/CLAUDE.md：文件表与职责描述对照终态（agentStatus.ts / agentHooks.ts / agent.ts / agentHistory.ts / useAgentNotifications 等）逐行 Glob 命中；旧名零残留。
【MC-223】src/features/hooksConfig/CLAUDE.md 注明「claude 专属编辑器」语义（claude hooks 协议知识不抽象、文件物理位置保留——决策 2）。
【核对段】src/{workspace,stores,theme}/CLAUDE.md、src/features/{sideViews,sidebar,explorer,commit,shortcuts}/CLAUDE.md：仅当引用到本次更名（claudeHistory / claudeStatus / cliIcons / hooks.ts 等旧名）时同步；无引用则不动（surgical）。

文件清单（只许碰这些）：新建 src/features/cliProfiles/CLAUDE.md；src/features/claudeHistory/CLAUDE.md → src/features/agentHistory/CLAUDE.md（删旧建新）；改 src/features/{agentStatus,notifications,hooksConfig}/CLAUDE.md；改 src/{lib,ipc,types}/CLAUDE.md；按需改 src/{workspace,stores,theme}/CLAUDE.md 与 src/features/{sideViews,sidebar,explorer,commit,shortcuts}/CLAUDE.md（仅旧名引用处）。`,
  },
  {
    label: 'backend-module-docs',
    prompt: `你负责 Stage 08 的后端模块 CLAUDE.md + src/panels/CLAUDE.md：MC-8（后端模块段）+ MC-109、MC-110、MC-223（panels 段）。禁止改 .claude/test-inventory.md（归 root-docs agent）。

【新建】src-tauri/src/agent_history/CLAUDE.md（替代 claude_history/CLAUDE.md——删旧建新；聚合层 + CliHistoryProvider trait + claude/ provider 结构；SEC-05 等价强制（validate_session_id 是 delete 强制前置）；env 覆盖 SLTERM_CLAUDE_PROJECTS_DIR 留 provider 内部）。
【MC-109】src-tauri/src/pty/CLAUDE.md 与 src/panels/CLAUDE.md：DA1/COLORTERM/合帧/resize/OSC 52/Kitty/Ctrl+Enter 等「claude 定制」归属表述 →「终端平台能力（设计动机 Ink 系 TUI，对全部子进程生效）」；触发点描述（「供 claude 取消」「Ink 据此换行」等）保留（红线见 PREAMBLE 文档纪律 2）。
【MC-110】src-tauri/src/pty/CLAUDE.md（或 spawn 相关段落）：SLTERM_PANEL_ID 保留为通用每终端路由键的文档记录；「无此变量 exit(0)」门控语义归各 CLI reporter 实现的说明。
【hooks 模块】src-tauri/src/hooks/CLAUDE.md：CliHooksProvider trait + claude/ 下沉结构 + 泛化命令 6 条 + agent-event 广播 + reporter 归 claude provider 资产（决策 7：显式 cliId + SCRIPT_VERSION 递增的「版本过旧」波及说明）；文件表逐行 Glob 命中（provider.rs / claude/{inject,usage,config}.rs / claude/slterm-hook-reporter.js）。
【MC-223 panels 段】src/panels/CLAUDE.md 的 hooksConfig 段注明「claude 专属编辑器」；panels/CLAUDE.md 其余引用本次更名处同步（agentStatus / agentHistory 等）。
【顶层与核对段】src-tauri/src/CLAUDE.md（lib.rs 命令注册清单终态：agent_hooks_* 6 + agent_history_* 2；旧命令名零残留）；src-tauri/src/{git,notify,fs}/CLAUDE.md 仅旧名引用处同步，无引用不动。

文件清单（只许碰这些）：src-tauri/src/claude_history/CLAUDE.md → src-tauri/src/agent_history/CLAUDE.md（删旧建新）；改 src-tauri/src/{CLAUDE.md, pty/CLAUDE.md, hooks/CLAUDE.md}；改 src/panels/CLAUDE.md；按需改 src-tauri/src/{git,notify,fs}/CLAUDE.md（仅旧名引用处）。`,
  },
  {
    label: 'e2e-l3-docs',
    prompt: `你负责 Stage 08 的测试基建文档：MC-8（测试文档段）。禁止改 .claude/test-inventory.md（归 root-docs agent）。

【e2e-tests/CLAUDE.md】：helper 更名同步（__slterm_e2e_injectHooks 泛化 cliId 参数 / __slterm_e2e_registerMockCliProfile 新增）；mock 冒烟用例（mockcli.e2e.ts 或追加位置）登记；E2E-05 备份集合 claude 硬编码 +「随第二 CLI 接入扩展」注释说明（决策 4）；agent-event / agent_hooks_* / agent_history_* 终态命名同步；E2E helper 页签/状态断言描述对照终态。
【src/__tests__/CLAUDE.md】：测试文件更名映射全表（tab-title-registry/tab-rules/cli-icons → cli-profile-registry/cli-profile-claude；claude-status → agent-status-lib + cli-profile-claude 迁入；ipc-hooks-contract → ipc-agent-hooks-contract；ipc-claude-history-contract → ipc-agent-history-contract；claude-history-* → agent-history-*）+ 新测试文件登记（mock-cli-profile.test.tsx / no-claude-literals.test.ts / helpers/mockCliProfile.ts）+ 共享测试工厂段同步；文件表逐行 Glob 命中。

文件清单（只许碰这些）：改 e2e-tests/CLAUDE.md、src/__tests__/CLAUDE.md。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-7 并行收集，8 最后单独串行——本 Stage 为终验 AC-1/AC-2）===
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-08.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 08 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照真实代码核实，防文档撒谎。
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
