// =====================================================================
// Stage 06 — 文档同步终验（YS-1~5/WD-1~4/KZ-6 + 终态核对）
// =====================================================================
// 真值源: docs/review-fix/checklist.md + docs/review-fix/stages.md（Stage 06 节）
// 断言清单: docs/review-fix/workflows/verify/stage-06.md（本脚本与 fix-loop 共用同一真值源）
// 收尾性质: 文档必须反映 Stage 01-05 全部代码改动后的最终状态；本 Stage 全量复跑含 L4 + L1
//   ——执行前必须关闭运行中的 slterminal.exe（review-00 基线遗留：进程占用致链接器 os error 5）
// fix-loop args: { stage: 6, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-06.md',
//   constraints: stages.md「禁区」六条原样,
//   testCommands: 本脚本 TEST_COMMANDS 数组原样（失败项涉终态核对/L4 断言时必传） }
// =====================================================================

export const meta = {
  name: 'stage06-docs-final',
  description: 'Stage 06: 文档一致性修复 + 终态核对（YS-1~5/WD-1~4/KZ-6）',
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
背景：先读 docs/review-fix/checklist.md 中你负责 ID 的条目原文 + docs/review-fix/stages.md Stage 06 节实现要点，再动手。
本 Stage 纪律：
- 文档/注释类修改：每条改前 Read 对应代码确认现行实现，改后描述必须与代码一致——防文档撒谎
- 本 Stage 基本不改生产逻辑（仅注释/文档），不跑资源共享型测试——全量测试由专门 agent 统一执行
- .claude/test-inventory.md 归 root-doc 单点负责——其余 agent 禁止触碰`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'panels-doc',
    prompt: `你负责 YS-1、WD-2（文档描述与代码对齐）：

【YS-1】src/panels/CLAUDE.md:249（F3 节 useCommandDetection 描述残留 cliIconRegistry.match）：
- 改为与同文件 :275 文件表一致的新描述——改前先 Read :275 现文照齐（cliProfileRegistry.matchByCommand 命中 → logo = profile.iconSrc、icon = STATUS_EMOJI.attention）；同文件矛盾消除
- 改后对照 src/panels/terminal/useCommandDetection.ts 现行实现核实不撒谎

【WD-2】三处「补全/悬停/波浪线」→「悬停/波浪线」（JSON 模式无自动补全——2026-08-01 验收后决策删除，以 src/panels/hooksConfig/JsonMode.tsx:6-8 注释为准）：
- src/panels/CLAUDE.md:152
- src/features/hooksConfig/CLAUDE.md:21
- src/features/hooksConfig/schema/index.ts:15（代码注释）
禁止触碰 .claude/test-inventory.md——归 root-doc 单点负责。`,
  },
  {
    label: 'registry-doc',
    prompt: `你负责 YS-3、KZ-6（注册表先例引用 + 新增 CLI 步骤补全）：

【YS-3】7 处「TabTitleRegistry 模式先例」引用逐处甄别更新——改指现存注册表（CliProfileRegistry 或 ShortcutRegistry，就近取语义近者），或删先例引用只留自身机制描述：
- src/features/sideViews/CLAUDE.md:5/:28/:46/:125
- src/theme/CLAUDE.md:19
- src/theme/schemeRegistry.ts:4-5（代码注释）
- src/features/cliProfiles/CLAUDE.md:17/:29
合法迁移溯源两处保留勿误删：src/panels/CLAUDE.md:275、cliProfiles/CLAUDE.md:29 后半（「两份拷贝收敛于此」类溯源交代）——逐处 Read 甄别。

【KZ-6】src/features/cliProfiles/CLAUDE.md:88-90「新增 CLI 步骤」补四步（按 Stage 03/04 终态撰写——改前 Read 对应代码确认）：
1. 后端 hooks provider 注册（src-tauri/src/hooks/provider.rs REGISTRY）
2. 后端 history provider 注册（src-tauri/src/agent_history/provider.rs REGISTRY）
3. .claude/test-inventory.md 用例数同步
4. hasConfigEditor=true 时新增编辑器组件并挂入 profile configEditor（含 configLayers 声明）
禁止触碰 .claude/test-inventory.md 本身——归 root-doc 单点负责。`,
  },
  {
    label: 'code-comments',
    prompt: `你负责 YS-2、YS-4、YS-5、WD-4（代码注释与现行实现对齐）：

【YS-2】两处测试注释残留 CliIconRegistry.match("claude") 字样 → 改现行 API（cliProfileRegistry.matchByCommand）或删除过时注释：
- src/__tests__/terminal.test.tsx:292
- src/__tests__/use-xterm-lifecycle.test.ts:1363
【YS-4】src/theme/schemes/types.ts:46：注释 claudeHistory → agentHistory。
【YS-5】src/features/agentStatus/AgentStatusRow.tsx:55：删除「与原 cliIconRegistry.getSrc 语义一致」对照半句，或改自含描述（按行 cliId 查 profile.iconSrc，未注册不报错）。
【WD-4】src/features/agentHistory/AgentHistorySections.tsx:49：注释 Map<sessionId, status> → Map<cliId|sessionId, status>（复合键 MC-313——Stage 02 落地后键构造经 keyOf 单点，改前 Read 该文件与 historyModel.ts 确认现行口径再落笔）。
纪律：只改注释，不动任何可执行代码；每处改前 Read 上下文确认新表述与现行代码一致。
禁止触碰 .claude/test-inventory.md——归 root-doc 单点负责。`,
  },
  {
    label: 'root-doc',
    prompt: `你负责 WD-1、WD-3 + 终态核对（根文档 + 全仓兜底核对）：

【WD-1】.claude/CLAUDE.md 需求编号索引表（:178-183 区域）追加 F9 行：
| F9 | 特性 | 终端页签/侧栏 CLI 品牌 logo（按命令行首 token 匹配 profile.iconSrc） |

【WD-3】CONTEXT.md:75-76 改为：前端为统一的 CliProfileRegistry；后端按能力拆分为 hooks/history 两个 cliId 键注册表（分别见 hooks/provider.rs 与 agent_history/provider.rs）——改前 Read 两个 provider.rs 确认描述与实际一致。

【终态核对】全仓 grep 退役 API/旧命名零残留兜底，逐处甄别豁免形态：
- CliIconRegistry：仅允许迁移溯源形态（注释/文档中交代历史的表述），其余零残留
- TabTitleRegistry：仅 panels/CLAUDE.md:275 与 cliProfiles/CLAUDE.md:29 后半两处合法溯源
- claudeHistory（作为 token 名/标识符）：零残留（文档叙述中提及历史名除外）
- transcriptPath：零残留（豁免：docs/ 历史文档、迁移溯源注释；transcript_path 仅 hooks.e2e.ts:170 stdin 模拟、claude provider 内部、reporter data.transcript_path 读取）
发现非豁免残留 → 不自行改代码，记入报告交 verify 判定。
【test-inventory 终态对齐】.claude/test-inventory.md 由你单点负责：L1/L2/L3/L4 用例数与全量复跑实跑计数对齐回写（测试 agent 报告会给出各命令通过数——若报告缺计数，用静态 grep/文件核对补齐并注明口径）。`,
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
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证（收尾终验，含 L1 + L4 全量复跑）。
执行前必须确认：无运行中的 slterminal.exe（Windows 文件锁会致 cargo 链接 os error 5——review-00 基线遗留）。
以下命令 1-7 相互独立，并行启动执行，收集全部结果；第 8 条 npm run e2e（= build:e2e + wdio）与 cargo 系存在 slterminal.exe 文件占用冲突——必须等 1-7 全部完成后单独串行执行：
${TEST_COMMANDS.map((c, i) => `${i + 1}. ${c}`).join('\n')}
逐条报告：每命令一行 exit code + 通过/失败 + 通过用例计数（test-inventory 终态对齐需要）；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由；文档类断言须对照真实代码核实，防文档撒谎。
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
