// =====================================================================
// Stage 03 — transcript 概念中性化全链路（KZ-2/KZ-3）
// =====================================================================
// 真值源: docs/review-fix/checklist.md + docs/review-fix/stages.md（Stage 03 节）
// 断言清单: docs/review-fix/workflows/verify/stage-03.md（本脚本与 fix-loop 共用同一真值源）
// 跨边界契约（stages.md 契约 1 原文，写死——三 agent 不各自推断）：
//   后端 DTO（signal.rs）：pub usage_source_path: Option<String>（#[serde(default)]，rename_all camelCase → JS 键 usageSourcePath）
//     ——不加 serde alias（信号文件瞬态，旧键降级 None 仅丢该事件用量拉取；版本门控引导重新注入）
//   trait（provider.rs）：fn context_usage(&self, usage_source_path: &str) -> Result<Option<ContextUsage>, AppError>
//     ——trait 文档注明「路径语义由具体 CLI 解释（claude = transcript JSONL）」
//   命令（mod.rs）：agent_context_usage(cli_id: String, usage_source_path: String)——JS invoke 键 { cliId, usageSourcePath }
//   reporter：payload 键 transcriptPath → usageSourcePath: data.transcript_path || null；SCRIPT_VERSION 2 → 3
//     ——data.transcript_path 是 claude hook stdin 协议字段（snake_case），更名不动
//   前端 DTO（types/agent.ts）：usageSourcePath?: string | null（原 transcriptPath: string 必填 → 可选）
//   前端内部状态（TerminalRegistry AgentSessionInfo）：transcriptPath → usageSourcePath（可选语义不变）
//   wrapper（agentHooks.ts）：contextUsage(cliId, usageSourcePath)
//   更名豁免（不动）：transcript_path snake_case 仅出现于 claude hook stdin 协议模拟（hooks.e2e.ts:170）
//     与 claude provider 内部（scan_transcript_usage 函数名/JSONL 解析语义）；文档中「transcript JSONL」
//     作为 claude 概念名词可保留（CONTEXT.md 术语表除外——术语条目更名）
// fix-loop args: { stage: 3, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-03.md',
//   constraints: stages.md「禁区」六条原样,
//   testCommands: 本脚本 TEST_COMMANDS 数组原样（失败项涉 E2E 更名断言时必传） }
// =====================================================================

export const meta = {
  name: 'stage03-usage-source-path',
  description: 'Stage 03: transcript 概念中性化——usageSourcePath 全链路更名（KZ-2/KZ-3）',
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
背景：先读 docs/review-fix/checklist.md 中 KZ-2/KZ-3 条目原文 + docs/review-fix/stages.md 契约 1 与 Stage 03 节实现要点，再动手。
本 Stage 纪律：
- 机械更名、逐文件核对：stages.md 契约 1「消费方全量清单」表逐文件打勾，禁凭记忆
- 并行期间禁止跑资源共享型测试——后端 agent 只做 cargo check；前端 agent 允许跑自己改动的单文件 vitest；全量测试由专门 agent 统一执行
- .claude/test-inventory.md 归 frontend-consumers 单点负责——其余 agent 禁止触碰`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend',
    prompt: `你负责 KZ-3（trait/命令更名）+ KZ-2 后端部分（DTO/reporter）——契约 1 后端段（写死）：

【DTO】src-tauri/src/hooks/signal.rs:33：pub transcript_path: String → pub usage_source_path: Option<String>（加 #[serde(default)]；不加 serde alias——信号文件瞬态，旧键降级 None 仅丢该事件用量拉取）。同文件 11 处测试 JSON 键同步（transcriptPath → usageSourcePath camelCase）。
【trait】src-tauri/src/hooks/provider.rs:33：context_usage 参数 transcript_path → usage_source_path；trait 文档注明「路径语义由具体 CLI 解释（claude = transcript JSONL）」。
【命令】src-tauri/src/hooks/mod.rs：agent_context_usage 参数 transcriptPath → usageSourcePath（:162/165/225/227；Tauri camelCase 双边——JS invoke 键 { cliId, usageSourcePath }）；测试 :342/364/431/439/523 随行——serde 键集合测试（:364 区域）同步为 9 键含 usageSourcePath；「无 cliId 旧信号兼容」用例形态扩展「无 usageSourcePath 信号 → None」。
【claude provider】src-tauri/src/hooks/claude/mod.rs:128-129 随行；src-tauri/src/hooks/claude/usage.rs:344/346/362（测试）随行——:346 测试名 context_usage_passes_transcript_path 更名为 context_usage_passes_usage_source_path。
【reporter】src-tauri/src/hooks/claude/slterm-hook-reporter.js:50：payload 键 transcriptPath → usageSourcePath: data.transcript_path || null（data.transcript_path 是 claude hook stdin 协议字段，snake_case 不动）；SCRIPT_VERSION 2 → 3（决策 7 先例：已注入用户显示「版本过旧」需重新注入）——C10 契约勿削弱（任何路径 exit(0)、不写 stderr）。
【inject.rs 随行】src-tauri/src/hooks/claude/inject.rs:389 的模板版本断言 assert_eq!(v, 2, ...) 同步改为 3（include_str! 内嵌模板版本校验；:473/:497/:987/:997 的磁盘脚本测试夹具用动态值或独立数据，不受影响——Read 甄别勿误改）。

就近同步：src-tauri/src/hooks/CLAUDE.md（:36/158/160/198/243/316 更名 + serde 语义：usage_source_path 可选、旧键信号降级 None）。
禁止触碰 .claude/test-inventory.md——归 frontend-consumers 单点负责（你的 L1 测试名更名由它代登记）。
自查：cargo check --manifest-path src-tauri/Cargo.toml 通过；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check 通过；node --check src-tauri/src/hooks/claude/slterm-hook-reporter.js 通过。`,
  },
  {
    label: 'frontend-contract',
    prompt: `你负责 KZ-2 前端 DTO + KZ-3 wrapper（契约层）——契约 1 前端契约段（写死）：

【DTO】src/types/agent.ts:26：transcriptPath: string → usageSourcePath?: string | null（必填 → 可选——对应后端 Option<String>）。
【wrapper】src/ipc/agentHooks.ts:42-44：contextUsage(cliId, transcriptPath) → contextUsage(cliId, usageSourcePath)——invoke 键 { cliId, usageSourcePath }（Tauri camelCase 双边，与后端命令参数对应）。
【契约测试】src/__tests__/ipc-agent-hooks-contract.test.ts（:158-165/226/273/288）：expectExactKeys 同步 = ["cliId", "usageSourcePath"]；参数结构断言随行。

就近同步：src/ipc/CLAUDE.md:26（wrapper 签名行）、src/types/CLAUDE.md（agent.ts 行——usageSourcePath 可选语义 + 后端 Option 对应）。
禁止触碰 .claude/test-inventory.md——归 frontend-consumers 单点负责。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/ipc-agent-hooks-contract.test.ts 通过。`,
  },
  {
    label: 'frontend-consumers',
    prompt: `你负责 KZ-2 前端内部状态 + 全消费方 + L3/E2E/文档随行——契约 1 消费方段（写死）：

【内部状态】src/panels/terminal/TerminalRegistry.ts:16/95：AgentSessionInfo.transcriptPath → usageSourcePath（可选语义不变；merge 语义注释随行）。
【消费方】字段更名，逻辑语义不变（含 basename 回退逻辑不动）：
- src/panels/terminal/useXterm.ts:384
- src/features/agentStatus/useAgentStatus.ts:43/179-180/196/207-208/264/272/340/346
- src/features/agentHistory/historyModel.ts:121-135
- src/features/agentHistory/HistorySessionList.tsx:201-203
【前端测试】随行：src/__tests__/agent-status-hook.test.ts（:6/81-84/236/248/260/269/446-470/543-561/906-994 共 18 处）、terminal-registry.test.ts:106-162/305/317、mock-cli-profile.test.tsx:283/508/511、notifications.test.ts:132、cli-profile-claude.test.ts:353、agent-history-hook.test.tsx:130、agent-history-model.test.ts:251/303/319/326。
【L3】test/terminal/production-osc.test.ts:106（注释更名）。
【E2E】信号构造 camelCase 键 transcriptPath → usageSourcePath：e2e-tests/agent.e2e.ts（:235/249/263/284/304/315/349/355/385/446/461/539 共 12 处）、hooks.e2e.ts:107/126、history.e2e.ts:488/497。豁免不动：hooks.e2e.ts:170 的 transcript_path（snake_case，模拟 claude hook stdin 协议字段——claude 领地知识）。
【文档】src/features/agentStatus/CLAUDE.md:27、src/features/agentHistory/CLAUDE.md:43、src/panels/CLAUDE.md:277、CONTEXT.md:229（术语条目更名）。
【test-inventory】.claude/test-inventory.md 由你单点负责：:282 行更名 + 代 backend 登记 L1 测试名更名（context_usage_passes_transcript_path → context_usage_passes_usage_source_path）。

完成后全仓 grep 自查：transcriptPath 零残留（豁免：docs/ 历史文档、迁移溯源注释）；transcript_path 残留仅 hooks.e2e.ts:170 stdin 模拟、claude provider 内部（scan_transcript_usage 等）、reporter 的 data.transcript_path 读取。
自查：npx tsc --noEmit 通过；npx vitest run 上述你改动的测试文件逐个通过。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-7 并行；L4 单独最后串行——exe 占用冲突）===
phase('全量测试')
const TEST_COMMANDS = [
  'npx tsc --noEmit',
  'npx eslint src/',
  'cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings',
  'cargo fmt --manifest-path src-tauri/Cargo.toml -- --check',
  'cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1',
  'npm test',
  'npm run test:l3',
  'npm run e2e',
]
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。
执行前确认：无运行中的 slterminal.exe（Windows 文件锁会致 cargo 链接 os error 5）。
以下命令 1-7 相互独立，并行启动执行，收集全部结果；第 8 条 npm run e2e（= build:e2e + wdio）与 cargo 系存在 slterminal.exe 文件占用冲突——必须等 1-7 全部完成后单独串行执行：
${TEST_COMMANDS.map((c, i) => `${i + 1}. ${c}`).join('\n')}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
