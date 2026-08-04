// =====================================================================
// Stage 12 L2-shortcuts/theme/store：断言真实化
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-12.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试；font-size/keybindings 测试的 afterEach 清理在 Stage 10 已加 cancelPendingSave 用例基础上补（不冲突）
// =====================================================================

export const meta = {
  name: 'stage12-l2-shortcuts-theme',
  description: 'L2 shortcuts colors 断言真实化 + forceContext 反向 + timer 清理',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试。并行 agent 文件零重叠（st-theme 碰 colors/theme/claude-status/global-commands/shortcuts/command-catalog/inject-script 测试；st-store 碰 projects/font-size/keybindings 测试）。font-size/keybindings 在 Stage 10 已加 cancelPendingSave 用例——本 Stage 在既有基础上改（afterEach 清理），不冲突。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'st-theme',
    prompt: `你负责 STS-01、STS-02、STS-03、STS-04、STS-05、STS-08、STS-09、STS-11①，触碰文件：src/__tests__/colors.test.ts、theme.test.ts、claude-status.test.ts、global-commands.test.ts、shortcuts.test.ts、command-catalog.test.ts、inject-script.test.ts。逐 ID 对照 checklist 原文实施：

【STS-01】colors.test.ts 循环断言——改读 colors.ts 实际值。位置 colors.test.ts:61-67,81-87,104-109,125-130,218-227。expect(expected).toMatch(HEX6_RE) 断言的是测试文件内硬编码字面量自身，token 漂移/拼错永不红。改 expect(GIT_FILE_COLORS[key]).toBe(expected) 等真实导出值比对（GIT_FILE/GIT_GUTTER/EXPLORER/SIDEBAR/AGENT_STATUS_USAGE 五组全改）。

【STS-02】global-commands 用例名与断言不符。位置 global-commands.test.ts:166-174。名"handler 不传播异常"实只 toBeDefined（handler 从未调用）。真调 handler 断言行为（getDockviewApi 抛异常时 handler 不向上抛），或改名"factory 在 getter 抛异常时仍能创建命令对象"（二选一）。

【STS-03】forceContext 平局 tie-breaker 反向分支未覆盖。位置 src/features/shortcuts/ShortcutRegistry.ts:236-242。补注册顺序 global 在前、terminal 在后 + forceContext="terminal" 用例（覆盖 aForced=0,bForced=1 方向）。

【STS-04】getStatusIcon(null) 分支未覆盖。位置 src/lib/claudeStatus.ts:23-26。补 getStatusIcon(null)==="" 与 getStatusIcon("working")==="⚡"。

【STS-05】theme.test.ts 未断言 kittyKeyboard。位置 src/panels/terminal/theme.ts:43。补 terminalOptions.vtExtensions?.kittyKeyboard === true 断言（与 E2E-02 的 L3 主题加载互补）。

【STS-08】command-catalog commandFromMeta 仅 5/9。位置 command-catalog.test.ts:76-135。改参数化遍历 EXPECTED_IDS 全 9 条，统一断言 id/context/defaultKey/handler。

【STS-09】colors 缺 EXPLORER_SELECTION_BG 等 token。位置 src/theme/index.ts:23-28。EXPLORER_SELECTION_BG/HTML_PANEL_LOADING_FG/HTML_PANEL_IFRAME_BG 加入 uiTokenCases（配合 STS-01 真实值断言）。

【STS-11①】inject-script 性能断言删除。位置 inject-script.test.ts:222-228。删 elapsed < 500ms 时间断言（保留结果断言）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'st-store',
    prompt: `你负责 STS-06、STS-07、STS-10、STS-11②，触碰文件：src/__tests__/projects.test.ts、font-size.test.ts、keybindings.test.ts。逐 ID 对照 checklist 原文实施：

【STS-06】store debounce 测试 afterEach 未清活跃 timer。位置 projects.test.ts:580-668、font-size.test.ts:33-35、keybindings.test.ts:29-31。afterEach 统一调 cancelPendingSave()（或 vi.runOnlyPendingTimers() + vi.clearAllTimers()）。本 Stage 在 Stage 10 已加的 cancelPendingSave 用例基础上补 afterEach 清理。

【STS-07】projects codify 可疑行为需注释。位置 projects.test.ts:179-189、358-371。不存在 pageId 操作仍递增 version 被锁为强契约，阻塞未来优化。按 D3 保留断言 + 注释"已知当前行为（无影响操作仍 bump version），非强契约"。

【STS-10】renamePage 不存在 projectId 守卫 + 名实不符改名。位置 src/stores/projects.ts:175-179、projects.test.ts:339-342。①补 renamePage 对不存在 projectId 状态不变（projects 不变、version 不变）用例；②"markPersistenceReady 应允许后续 save"补实际 save 断言或改名。

【STS-11②】同引用快照改深拷贝。位置 projects.test.ts:173-177,290-300。状态不变断言改 structuredClone 快照比对（当前 toEqual 同引用无法验证不可变性）。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-12.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage12 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-12.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
