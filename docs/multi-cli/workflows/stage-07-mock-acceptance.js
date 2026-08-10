// =====================================================================
// Stage 07 — mock profile 全链路验收 + AC-5 守卫
// 条目：AC-4（全表）、AC-5、MC-4、MC-6
// 真值源：docs/multi-cli/checklist.md（§6 AC-4/AC-5 条目原文）+ docs/multi-cli/stages.md（Stage 07 分工表与实现要点）
// commit message：test(cli-profiles): mock profile 全链路验收 + AC-5 字面量守卫（AC-4/AC-5）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样 + 「本 Stage 只新增测试与 E2E helper，禁改生产逻辑
//   （唯一例外：E2E helper 页面侧挂载点追加 __slterm_e2e_registerMockCliProfile，E2E_ENABLED 门控）」
// test-inventory 独占：本 Stage 仅 mock-fixture agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// 分工表补列（skill 纪律 5）：ac5-guard 补「页面侧挂载点文件」（__slterm_e2e_registerMockCliProfile 的
//   页面侧实现在 src/ 内 E2E_ENABLED 门控段，stages.md 分工表仅列 e2e-tests/helpers.ts——grep __slterm_e2e 定位）。
// =====================================================================

export const meta = {
  name: 'stage-07-mock-acceptance',
  description: 'Stage 07：mock profile 全链路验收（AC-4 五点）+ AC-5 claude 字面量守卫',
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
本 Stage 特殊纪律：只新增测试与 E2E helper，禁改生产逻辑（唯一例外：E2E helper 页面侧挂载点追加 __slterm_e2e_registerMockCliProfile，E2E_ENABLED 门控段内）。
背景：先读 docs/multi-cli/checklist.md §6（AC-4/AC-5 条目原文）+ docs/multi-cli/stages.md 的 Stage 07 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
mock profile 约定（决策 5 + spec 06 §7）：id "mockcli"、displayName "mockcli"、commands ["mockcli"]、tabTitle "mockcli"、iconSrc "/cli-icons/mockcli.png"（Stage 01 已放资源）；hooks 全能力（eventToStatus 恒等映射 / classifyNotification 桩 / contextLimit 任意值 / restartHint 桩文案 / hasConfigEditor=true）+ history 全能力（supportsFork=true / buildResumeCommand / buildRestoreInput 桩输出带可识别前缀如 "mockcli --resume"）；仅测试环境注册（vitest 内 register + afterEach _reset 清理；L4 经 E2E helper 注册，E2E_ENABLED 门控红线不变）。
AC-5 豁免形态：通用层缺省回退经 import CLAUDE_CLI_ID（profiles/claude/ 导出常量）合法——守卫匹配规则：禁止值等于 "claude" 的字符串字面量与 claude 事件名字符串字面量；import 路径指向 features/cliProfiles/profiles/claude/ 合法。
【测试纪律】你不跑资源共享型测试；只做编译级检查 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 中间态，忽略；真实执行由全量测试 agent 单点跑。除 mock-fixture 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'mock-fixture',
    prompt: `你负责 Stage 07 的 mock 夹具与 AC-4 五点 L2 全链路用例 + test-inventory 就近登记（本 Stage 独占）。

【mock 夹具】新建 src/__tests__/helpers/mockCliProfile.ts：mockcli profile 定义（契约见 PREAMBLE）+ 注册/清理辅助（register + afterEach _reset，仅测试环境引用）。
【AC-4 五点用例】新建 src/__tests__/mock-cli-profile.test.tsx，逐点覆盖全表：
  ① OSC 133 命中：matchByCommand("mockcli --flag") 命中 → 页签标题 = "mockcli" / logo = /cli-icons/mockcli.png / setAgentSession 写入 agentSession.cliId = "mockcli"（useCommandDetection 链路 L2 断言）；
  ② eventToStatus / classifyNotification 被真实调用（spy 断言入参——经 useXterm 事件路径与通知调度路径各一次）；
  ③ 历史聚合 UI：mock 条目（AgentHistorySession cliId="mockcli" 数据）出现在历史区 + 行 logo 按 session.cliId 取 mockcli iconSrc；
  ④ hub 选择行：两枚按钮（claude + mockcli，均 hasConfigEditor=true）+ 切换渲染 mock 桩编辑器 + selectedCli 持久化恢复（updateParameters + 显式 onLayoutChange 断言）；
  ⑤ 恢复注入：pty.write 注入内容 = mock buildRestoreInput 桩输出（可识别前缀断言）。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含 ac5-guard 负责的部分）：新增 helpers/mockCliProfile.ts 夹具 + mock-cli-profile.test.tsx（AC-4 五点，用例数按实际编写登记）；新增 no-claude-literals.test.ts（AC-5 守卫）；E2E 新增 mock 冒烟用例（1-2 条）。

文件清单（只许碰这些）：新建 src/__tests__/helpers/mockCliProfile.ts、src/__tests__/mock-cli-profile.test.tsx；改 .claude/test-inventory.md。`,
  },
  {
    label: 'ac5-guard',
    prompt: `你负责 Stage 07 的 AC-5 字面量守卫 + E2E mock 冒烟。禁止改 .claude/test-inventory.md（归 mock-fixture agent）。

【AC-5 守卫】新建 src/__tests__/no-claude-literals.test.ts（L2 grep 守卫形态，照 e2e-build-config.test.ts 读文件文本做断言的先例）：测试自身用 fs 枚举扫描以下七路径的 .ts(x) 文件（新增文件自动纳入）——src/lib/、src/panels/terminal/、src/features/agentStatus/、src/features/agentHistory/、src/features/notifications/、src/ipc/、src/types/：
  - 禁止值等于 "claude" 的字符串字面量（精确匹配，不误伤 import 路径与子串）；
  - 禁止 claude 事件名字符串字面量：SessionStart / SessionEnd / UserPromptSubmit / Stop / StopFailure / PreToolUse / PostToolUse / PostToolUseFailure / Notification / PermissionRequest（作为字符串字面量出现才算；标识符、注释不计——断言写语义注释说明口径）；
  - 禁止 ~/.claude 路径字面量；
  - 豁免：import 路径指向 features/cliProfiles/profiles/claude/（CLAUDE_CLI_ID 常量引用形态）合法。
【E2E mock 冒烟】改 e2e-tests/helpers.ts 的页面侧挂载点（grep __slterm_e2e 定位 E2E_ENABLED 门控段）追加 __slterm_e2e_registerMockCliProfile（注册 mockcli profile，仅在 E2E_ENABLED 门控段内，内联 import.meta.env 字面量形态不动——禁区 6）+ helpers.ts wdio 侧封装；新增 mock 冒烟用例 1-2 条（追加进既有 e2e spec 或新建 e2e-tests/mockcli.e2e.ts）：helper 注册 mockcli → 终端注入 OSC 133 C（__e2e_writeToTerminal 或 pty.write）→ 页签标题 "mockcli" / logo 断言。
【门禁说明】e2e-tests/ 不在根 tsconfig include——你的 E2E 改动正确性由全量测试 agent 的 npm run e2e（L4）运行时兜底。

文件清单（只许碰这些）：新建 src/__tests__/no-claude-literals.test.ts；改 e2e-tests/helpers.ts + 页面侧挂载点文件（grep __slterm_e2e 定位，E2E_ENABLED 门控段内追加）；新建 e2e-tests/mockcli.e2e.ts（或追加既有 spec）。`,
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
