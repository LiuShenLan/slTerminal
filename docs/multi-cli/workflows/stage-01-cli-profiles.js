// =====================================================================
// Stage 01 — 前端 profile 注册表 + 身份域
// 条目：MC-101/102/103/104/105/106/108 + D-02(cliIcons 段)/D-07/D-08/D-13(核对) + AC-4 资源先行(mockcli.png)
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 01 分工表与实现要点）
// commit message：refactor(cli-profiles): 前端 CliProfileRegistry + 身份域迁移（MC-101~108）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 profile-registry agent 可改 .claude/test-inventory.md
//   （execution-plan §6 就近登记 + 并行防冲突——全 Stage 用例变动系计划确定，静态清单已写进其 prompt），
//   其余 agent 禁改。
// =====================================================================

export const meta = {
  name: 'stage-01-cli-profiles',
  description: 'Stage 01：前端 CliProfileRegistry + claude 身份域迁移（MC-101~108 + D-02/07/08/13 + mockcli.png）',
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
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 01 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
profile 接口（前端，spec 00 §3.1；本 Stage 落地时 capabilities 签名引用现状类型名 ClaudeStatus / HookEventPayload / HistorySession，Stage 02/03/04 更名时随行同步）：
  interface CodingCliProfile {
    id: string;                 // cliId 公共键，如 "claude"
    displayName: string;        // 展示名，如 "claude"
    commands: string[];         // 首 token 精确匹配键集
    iconSrc: string;            // 品牌 logo 根绝对路径，如 "/cli-icons/claude.png"
    tabTitle: string;           // OSC 133 C 命中页签标题
    capabilities: { hooks?: HooksCapability; history?: HistoryCapability };
  }
缺省回退常量约定：profiles/claude/ 导出 CLAUDE_CLI_ID = "claude" 常量，通用层缺省回退一律 import 该常量，禁止写 "claude" 字符串字面量（AC-5 字面量守卫兼容）。
【测试纪律】你不跑资源共享型测试（PTY/端口/全局锁类）；只做编译级检查 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 的中间态，忽略，只保证自己分工文件正确；真实执行由全量测试 agent 单点跑。除分工指定 agent 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'profile-registry',
    prompt: `你负责 Stage 01 的 profile 注册表与 claude 身份域：MC-101、MC-102、MC-103、MC-104、MC-108 + mockcli.png 资源先行 + test-inventory 就近登记（本 Stage 独占）。

【MC-101】新建 src/features/cliProfiles/cliProfileRegistry.ts——模块级单例注册表（照 src/panels/terminal/TabTitleRegistry.ts 模式；该文件由 terminal-consumers agent 删除，你可先读参考）：register(profile)（同 id 覆盖）/ get(id) / getAll()（注册序）/ matchByCommand(commandLine) / _reset()（仅测试用）。
【MC-102】首 token 解析单点化：matchByCommand 内部实现 trim().split(/\\s+/)[0] 取首 token（现状两份拷贝在 src/lib/cliIcons.ts 与 TabTitleRegistry.ts，本 Stage 收敛为注册表内唯一实现）；对 profile.commands 逐键精确查表；空命令行/仅空白 → null；不 toLowerCase。
【MC-103】新建 src/features/cliProfiles/types.ts：CodingCliProfile / HooksCapability / HistoryCapability 按 PREAMBLE 契约段定义（HooksCapability：eventToStatus(event, notificationType?) / classifyNotification(payload) / contextLimit / restartHint / hasConfigEditor；HistoryCapability：supportsFork / buildResumeCommand(session) / buildRestoreInput(session, { fork })）；commands: string[] 支持多首 token；capabilities 签名引用现状类型名 ClaudeStatus（src/lib/claudeStatus.ts）/ HookEventPayload（src/types/hooks.ts）/ HistorySession（src/types/claudeHistory.ts）——Stage 02/03/04 更名时随行同步。
【MC-104】新建 src/features/cliProfiles/profiles/claude/index.ts 与 src/features/cliProfiles/profiles/index.ts：claude profile 身份域 id:"claude"、displayName:"claude"、commands:["claude"]、iconSrc:"/cli-icons/claude.png"、tabTitle:"claude"、capabilities 先为 {}（hooks 能力 Stage 02 迁入、history 能力 Stage 05 迁入）；profiles/claude/ 导出 CLAUDE_CLI_ID = "claude" 常量；profiles/index.ts 追加 import "./claude"（照 tabRules side-effect 先例）；public/cli-icons/claude.png 保留原位不动。新建 src/features/cliProfiles/index.ts barrel。
【MC-108】logo 资源守卫泛化：新建 src/__tests__/cli-profile-registry.test.ts 与 src/__tests__/cli-profile-claude.test.ts——把原 src/__tests__/cli-icons.test.ts 的资源守卫语义泛化为：遍历注册表全部 profile，断言 iconSrc 对应磁盘文件存在 + PNG 魔数（img 404 无报错通道，资源缺失靠此守卫）；注册表行为用例：register 同 id 覆盖 / get / getAll 注册序 / matchByCommand（多 commands、带参变体、空命令行、仅空白、未命中、不 toLowerCase）/ _reset；claude profile 身份域字段断言。原 tab-title-registry.test.ts（13 用例）/tab-rules.test.ts（6 用例）/cli-icons.test.ts（12 用例）的语义（首 token 匹配/带参变体/覆盖/单例/资源守卫）并入上述两文件。
【mockcli.png】新建 public/cli-icons/mockcli.png（1×1 透明 PNG，Stage 07 mock 夹具引用，本 Stage 仅放资源）：用 node -e 把 base64 串 iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg== 解码写入——合法 PNG 魔数，资源守卫统一零特例。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：删 tab-title-registry（13）/tab-rules（6）/cli-icons（12）；增 cli-profile-registry / cli-profile-claude（用例数按你实际编写登记）；改 use-xterm-lifecycle / use-xterm-output / use-xterm-integration / terminal / e2e-gating-terminal（用例数不变，断言改指 profile 注册表）；改 L3 production-osc（8 用例数不变，复刻段改写）。

文件清单（只许碰这些）：新建 src/features/cliProfiles/{types.ts, cliProfileRegistry.ts, index.ts}、src/features/cliProfiles/profiles/{index.ts, claude/index.ts}、src/__tests__/{cli-profile-registry.test.ts, cli-profile-claude.test.ts}、public/cli-icons/mockcli.png；改 .claude/test-inventory.md。`,
  },
  {
    label: 'terminal-consumers',
    prompt: `你负责 Stage 01 的终端域消费点迁移：MC-105、MC-106 + TabState 类型承接 + D-08、D-13（核对）。禁止改 .claude/test-inventory.md（归 profile-registry agent）。

【MC-105】改 src/panels/terminal/useCommandDetection.ts（OSC 133 C 消费点，约 53-58 行）：命令命中改经 cliProfileRegistry.matchByCommand 取 profile——title = profile.tabTitle、logo = profile.iconSrc；未命中零副作用（现状 rule == null 分支语义保留）；TabState.logo 字段保留（TerminalPanel 消费链不变，仅值来源改 profile.iconSrc）。
【MC-106】useCommandDetection.ts 中 icon: "🟡" 字面量改为 STATUS_EMOJI.attention 引用（从 src/lib/claudeStatus.ts import——claudeStatus 更名 agentStatus 属 Stage 02，本 Stage 维持现状 import 路径）。
【TabState 承接】TabTitleRegistry.ts 退役后，TabState 类型（含 logo 字段）迁入 useCommandDetection.ts 顶部导出；TerminalPanel.tsx / useXterm.ts / usePtyOutput.ts 的 import 与类型引用同步；TerminalPanel 消费链（logoRef / tabIcon && tabLogo 双条件 / inactive 双清）零行为改动。
【删除】删 src/panels/terminal/TabTitleRegistry.ts、src/panels/terminal/tabRules.ts。
【D-08】改 test/terminal/production-osc.test.ts（L3，8 用例）：OSC 133 复刻段按生产新实现复刻改写（原复刻 TabTitleRegistry/CliIconRegistry 匹配逻辑 → 复刻 matchByCommand / profile 取值），逐段来源行号注释同步。
【D-13】核对 src/__tests__/e2e-gating-terminal.test.ts：E2E helper 门控断言（useXterm/useTerminalInstance 引用点）随本次改动同步。
【测试同步】改 src/__tests__/{use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, terminal.test.tsx}——TabTitleRegistry/tabRules 相关断言改指 profile 注册表（mock 形态随之调整）。

文件清单（只许碰这些）：改 src/panels/terminal/{useCommandDetection.ts, TerminalPanel.tsx, useXterm.ts, usePtyOutput.ts}；删 src/panels/terminal/{TabTitleRegistry.ts, tabRules.ts}；改 src/__tests__/{use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, terminal.test.tsx, e2e-gating-terminal.test.ts}；改 test/terminal/production-osc.test.ts。`,
  },
  {
    label: 'peripheral-consumers',
    prompt: `你负责 Stage 01 的外围消费点迁移：cliIcons 退役 + D-02（cliIcons 段）+ D-07 + 过渡形态 + 退役测试删除。禁止改 .claude/test-inventory.md（归 profile-registry agent）。

【cliIcons 退役】删 src/lib/cliIcons.ts。
【D-02 cliIcons 段】改 src/lib/index.ts：移除 cliIcons 导出（claudeStatus 导出名本 Stage 不动，Stage 02 才更名 agentStatus）。
【D-07】改 src/workspace/Workspace.tsx：side-effect 注册触发 import（现状指 tabRules）改为 import "features/cliProfiles/profiles"（照 tabRules/schemes side-effect 先例，import 路径别名按该文件现状写法）。
【过渡形态】src/features/agentStatus/AgentStatusRow.tsx 与 src/features/claudeHistory/HistorySessionRow.tsx 中的 cliIconRegistry.getSrc("claude") 改为 cliProfileRegistry.get(CLAUDE_CLI_ID)?.iconSrc——CLAUDE_CLI_ID 从 features/cliProfiles/profiles/claude/ import，禁止写 "claude" 字符串字面量；未命中时行为与原 getSrc 未命中一致（无 logo 不报错）。这是过渡形态：行 cliId 字段 Stage 02（MC-410）/Stage 05（MC-311 数据侧）就绪后回收，verify 白名单限此两处。
【退役测试删除】删 src/__tests__/{tab-title-registry.test.ts, tab-rules.test.ts, cli-icons.test.ts}（语义已由 profile-registry agent 迁入 cli-profile-registry.test.ts / cli-profile-claude.test.ts）。

文件清单（只许碰这些）：删 src/lib/cliIcons.ts、src/__tests__/{tab-title-registry.test.ts, tab-rules.test.ts, cli-icons.test.ts}；改 src/lib/index.ts、src/workspace/Workspace.tsx、src/features/agentStatus/AgentStatusRow.tsx、src/features/claudeHistory/HistorySessionRow.tsx。`,
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
