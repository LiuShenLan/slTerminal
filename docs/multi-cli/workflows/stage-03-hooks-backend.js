// =====================================================================
// Stage 03 — 后端 hooks 泛化 + 前端 ipc/types
// 条目：MC-201/202/203(核对)/204(核对)/210/211/212/213/214(后端半)/215(决策 7) + 决策 3（类型更名）+ 决策 4（E2E-05 硬编码+注释）
//       + D-01/D-03(types barrel hooks 段)/D-09/D-10/D-11/D-14(hooks.e2e/agent.e2e 事件名段)
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 03 分工表与实现要点）
// commit message：refactor(hooks): 后端 hooks 信号链路泛化 + CliHooksProvider 下沉 claude（MC-201~215）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 frontend-ipc agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// =====================================================================

export const meta = {
  name: 'stage-03-hooks-backend',
  description: 'Stage 03：后端 hooks 信号链路泛化 + CliHooksProvider 下沉 claude + 前端 ipc/types 同步（MC-201~215 + D-01/03/09/10/11/14）',
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
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 03 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
1. 泛化命令（6 条 hooks 域，全表）：
   agent_hooks_inject(cliId)                       agent_hooks_uninstall(cliId)
   agent_hooks_injection_status(cliId)             agent_context_usage(cliId, transcriptPath)
   agent_hooks_config_read(cliId, layer, projectPath?)   agent_hooks_config_write(cliId, layer, hooks, projectPath?)
   未知 cliId → AppError::Validation；cliId 已注册但无 hooks 能力 → Validation（消息含「不支持 hooks 能力」语义）；旧命令名（hooks_*）不保留兼容（D10）。
2. 事件与 DTO：广播事件名 "agent-event"（旧 "hook-event" 零残留）；AgentEventPayload = panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType + 可选 cliId（serde default，缺省前端按 CLAUDE_CLI_ID 兼容）；决策 3 更名：HookEventPayload→AgentEventPayload / InjectionStatus→AgentInjectionStatus / HookInjectionStatus→AgentHookInjectionStatus / ContextUsage 保留名；DTO 双边 camelCase。
3. 前端中间态（写死）：useHooksConfig.ts / HooksConfigPanel.tsx 的泛化命令 cliId 实参暂传 CLAUDE_CLI_ID 常量（import 自 features/cliProfiles/profiles/claude/，禁字面量）——Stage 06 hub 化时改 selectedCliId 回收；useAgentStatus.ts 的 contextUsage 传行 cliId（Stage 02 已建行字段）。
4. CliHooksProvider trait 签名（写死）：inject() / uninstall() / injection_status() / context_usage(transcript_path) / config_read(layer, project_path) / config_write(layer, hooks, project_path)；注册表 = cliId 键静态映射。
【测试纪律】你不跑资源共享型测试（PTY/端口/全局锁/cargo test 执行）；后端只做编译级检查 cargo check --manifest-path src-tauri/Cargo.toml（或 cargo test --no-run），前端 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 中间态，忽略；真实执行由全量测试 agent 单点跑。cargo 系命令共享 target 目录锁，并行排队属正常勿中止。除 frontend-ipc 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-hooks',
    prompt: `你负责 Stage 03 的后端 hooks 泛化：MC-201、MC-202（后端）、MC-203（核对）、MC-204（核对）、MC-210、MC-211、MC-213、MC-214（后端半）、MC-215（决策 7）+ 决策 3 后端类型更名。禁止改 .claude/test-inventory.md（归 frontend-ipc agent）。

【MC-201】改 src-tauri/src/hooks/signal.rs：HookEventPayload → AgentEventPayload——8 字段（panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType）语义不变 + 新增可选 cli_id（serde default + camelCase cliId，缺省前端按 claude 兼容）；serde 键集合测试同步（含无 cliId 旧信号反序列化兼容用例）。
【MC-202 后端】signal.rs emit 点：广播 "hook-event" → "agent-event"。
【MC-203 核对】watcher.rs 双通道（notify 50ms debounce + 3s 轮询补漏 + 目录自动重建）零行为改动；信号目录 ~/.slterminal/hooks-events/ 单目录全 CLI 共用（路由靠 payload.panelId + cliId，不分目录）；勿削弱轮询补漏（win10 实证防线）。仅核对，不改。
【MC-204 核对】process_signal_file 读→emit→删契约不变；解析失败/emit 失败仍删文件的容错语义不变。仅核对，不改。
【MC-210】新建 src-tauri/src/hooks/provider.rs：CliHooksProvider trait 六方法（签名见 PREAMBLE 契约段 4）；静态注册表以 cliId 为键；claude 为首个实现（行为零改动）。
【MC-211】改 src-tauri/src/hooks/mod.rs 命令层 + src-tauri/src/lib.rs 注册：6 命令泛化（全表见 PREAMBLE 契约段 1）；未知 cliId → AppError::Validation("未知 cliId: ...")；无 hooks 能力 cliId → Validation（含「不支持 hooks 能力」语义——本期注册表仅 claude，走不到第二分支，但分支与测试要建好）；旧命令不保留兼容。
【MC-213】下沉 inject.rs/usage.rs/config.rs → src-tauri/src/hooks/claude/：provider 内部全部保留 claude 命名与 claude 知识（HOOK_EVENTS 10 事件、~/.claude/settings.json、matcher 结构、SCRIPT_VERSION 检测、reporter 模板、三层配置路径）——provider 内部是 claude 合法领地（D11）。
【MC-214 后端半】usage.rs 随下沉；ContextUsage DTO 四字段保留（input/output/cacheRead/cacheCreation，cache serde default 0）。
【MC-215 决策 7】移动 src-tauri/assets/slterm-hook-reporter.js → src-tauri/src/hooks/claude/slterm-hook-reporter.js（include_str! 路径同步）：payload 显式写 cliId: "claude"；SCRIPT_VERSION 递增（已注入用户变「版本过旧」需重新注入——预期波及，测试锁死此形态）；注入目标路径 ~/.slterminal/hooks/slterm-hook-reporter.js 不变（E2E 零波及）；C10 契约（任何路径 exit 0 不写 stderr）不改。
【决策 3 后端更名】InjectionStatus → AgentInjectionStatus、HookInjectionStatus → AgentHookInjectionStatus（camelCase 三态契约不变）。
【L1 测试】133 条用例（inject 34/signal 14/watcher 20/usage 26/config 27/mod 12）全部保留迁移，--test-threads=1 纪律不变；新增：注册表 get / 未知 cliId Validation / 命令 cliId 透传（block_on 直测）/ reporter 模板内嵌校验断言（显式 cliId + SCRIPT_VERSION 已递增）。你只做编译级检查（cargo check / cargo test --no-run），不执行 cargo test。

文件清单（只许碰这些）：改 src-tauri/src/hooks/{signal.rs, mod.rs}；新建 src-tauri/src/hooks/provider.rs；src-tauri/src/hooks/{inject.rs, usage.rs, config.rs} → src-tauri/src/hooks/claude/（含新建 src-tauri/src/hooks/claude/mod.rs 如需要）；移动 src-tauri/assets/slterm-hook-reporter.js → src-tauri/src/hooks/claude/slterm-hook-reporter.js；改 src-tauri/src/lib.rs；核对 src-tauri/src/hooks/watcher.rs（零改动）；迁移相关 L1 测试文件（ hooks 模块内 #[cfg(test)] 或 tests/ 现状位置随行）。`,
  },
  {
    label: 'frontend-ipc',
    prompt: `你负责 Stage 03 的前端 ipc/types 泛化与订阅更名：MC-212、MC-202（前端）、D-01、D-03（hooks 段）+ 前端调用点中间态同步 + test-inventory 就近登记（本 Stage 独占）。

【MC-212】src/ipc/hooks.ts → src/ipc/agentHooks.ts：wrapper 加 cliId 首参（6 命令全表见 PREAMBLE 契约段 1）；src/types/hooks.ts → src/types/agent.ts：决策 3 更名（AgentEventPayload / ContextUsage 保留名 / InjectionStatus→AgentInjectionStatus / HookInjectionStatus→AgentHookInjectionStatus）；src/__tests__/ipc-hooks-contract.test.ts → ipc-agent-hooks-contract.test.ts（22 用例四维同步：命令名 / 参数含 cliId camelCase / 返回 / 异常）；改 src/__tests__/ipc-hooks-config-contract.test.ts 同步。
【MC-202 前端】onHookEvent → onAgentEvent（listen<AgentEventPayload>("agent-event")，照 onFsEvent 模式）；三消费方迁移：src/panels/terminal/useXterm.ts、src/features/agentStatus/useAgentStatus.ts、src/features/notifications/useAgentNotifications.ts。
【D-01 红线】改 src/__tests__/setup.ts：全局 mock vi.mock("../ipc/hooks") → vi.mock("../ipc/agentHooks")，onHookEvent → onAgentEvent——漏改则全局 mock 失效、L2 大面积炸。
【D-03 hooks 段】改 src/types/index.ts barrel：hooks → agent；改 src/ipc/index.ts 同步。
【中间态写死】src/panels/hooksConfig/useHooksConfig.ts 与 src/panels/hooksConfig/HooksConfigPanel.tsx 的泛化命令 cliId 实参暂传 CLAUDE_CLI_ID 常量（import 自 features/cliProfiles/profiles/claude/，禁 "claude" 字面量）——Stage 06 回收；src/features/agentStatus/useAgentStatus.ts 的 contextUsage 调用传行 cliId。
【测试同步】改 src/__tests__/{use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, agent-status-view.test.tsx, agent-status-hook.test.ts, notifications.test.ts, hooks-config-panel.test.tsx, hooks-config-sync.test.tsx}（事件名/wrapper 更名 + cliId 参数断言同步）。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：L1 hooks 域 133 用例位置迁移（inject/usage/config 下沉 claude/，用例数不变）+ L1 新增注册表/命令透传/模板校验用例；ipc-hooks-contract → ipc-agent-hooks-contract（22 用例四维同步）；ipc-hooks-config-contract 同步；setup.ts 全局 mock 更名；E2E hooks.e2e/agent.e2e 事件名与命令名断言同步（用例数不变）；run-wdio.cjs 注释（无用例变动）。

文件清单（只许碰这些）：src/ipc/hooks.ts → src/ipc/agentHooks.ts；src/types/hooks.ts → src/types/agent.ts；改 src/ipc/{index.ts, hooksConfig.ts}、src/types/index.ts；改 src/__tests__/setup.ts；src/__tests__/ipc-hooks-contract.test.ts → src/__tests__/ipc-agent-hooks-contract.test.ts；改 src/__tests__/ipc-hooks-config-contract.test.ts；改 src/panels/hooksConfig/{useHooksConfig.ts, HooksConfigPanel.tsx}；改 src/features/agentStatus/useAgentStatus.ts；改 src/panels/terminal/useXterm.ts；改 src/features/notifications/useAgentNotifications.ts；改 src/__tests__/{use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, agent-status-view.test.tsx, agent-status-hook.test.ts, notifications.test.ts, hooks-config-panel.test.tsx, hooks-config-sync.test.tsx}；改 .claude/test-inventory.md。`,
  },
  {
    label: 'e2e-infra',
    prompt: `你负责 Stage 03 的 E2E 基建同步：D-09、D-10、D-11、D-14（本 Stage 段）。禁止改 .claude/test-inventory.md（归 frontend-ipc agent）。

【D-09】改 e2e-tests/helpers.ts：E2E helper __slterm_e2e_injectHooks 等调 hooks.inject() 系列 → 泛化 wrapper（cliId 参数，helper 内固定传 "claude"——E2E 辅助代码属测试基建，字面量合法）；E2E_ENABLED 内联 import.meta.env 门控红线不动（禁区 6）。
【D-10】改 e2e-tests/specUtils.ts：hooks 注入辅助（hooks_inject 等旧命令名 / wrapper 引用）同步泛化为 agent_hooks_* 六命令全表（PREAMBLE 契约段 1）。
【D-11 决策 4】改 e2e-tests/run-wdio.cjs：E2E-05 用户目录隔离备份集合（~/.claude/settings.json + ~/.slterminal/hooks/ + hooks-events/）保持 claude 硬编码 + 追加注释「随第二 CLI 接入扩展」。
【D-14 本 Stage 段】改 e2e-tests/hooks.e2e.ts：命令名断言同步泛化（agent_hooks_*）；改 e2e-tests/agent.e2e.ts：事件名断言 "hook-event" → "agent-event" 同步。其余断言逐字不动。
【门禁说明】e2e-tests/ 不在根 tsconfig include——你的改动正确性由全量测试 agent 的 npm run e2e（L4）运行时兜底；你可用 npx tsc --noEmit -p e2e-tests（若有 tsconfig）或语法级自查，不做构建级检查。

文件清单（只许碰这些）：改 e2e-tests/{helpers.ts, specUtils.ts, run-wdio.cjs, hooks.e2e.ts, agent.e2e.ts}。`,
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
