// =====================================================================
// Stage 05 — mockcli 验收强化（KZ-7/CS-3）
// =====================================================================
// 真值源: docs/review-fix/checklist.md + docs/review-fix/stages.md（Stage 05 节）
// 断言清单: docs/review-fix/workflows/verify/stage-05.md（本脚本与 fix-loop 共用同一真值源）
// 前置依赖: Stage 04 产物（HooksCapability.configEditor/configLayers 已入 profile 类型）——
//   本 Stage 为 mockcli 夹具补齐该两字段并补双向分派断言
// fix-loop args: { stage: 5, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-05.md',
//   constraints: stages.md「禁区」六条原样,
//   testCommands: 本脚本 TEST_COMMANDS 数组原样（失败项涉 CS-3 的 L4 断言时必传） }
// =====================================================================

export const meta = {
  name: 'stage05-mockcli-acceptance',
  description: 'Stage 05: mockcli 编辑器分派双向断言 + L4 关键路径补全（KZ-7/CS-3）',
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
背景：先读 docs/review-fix/checklist.md 中你负责 ID 的条目原文 + docs/review-fix/stages.md Stage 05 节实现要点，再动手；Stage 04 产物（HooksCapability.configEditor/configLayers 签名）见 stages.md 契约 2。
本 Stage 纪律：
- 并行期间禁止跑资源共享型测试（npm run e2e 由专门 agent 统一跑）——编译级检查 npx tsc --noEmit；l2-mock-editor 允许跑自己改动的单文件 vitest
- .claude/test-inventory.md 归 l4-mockcli 单点负责——l2-mock-editor 禁止触碰（你的用例变化由它按 prompt 写明的代登记项同步）`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'l2-mock-editor',
    prompt: `你负责 KZ-7（mock 编辑器桩 + AC-4④ 双向分派断言）：

【桩组件】src/__tests__/helpers/mockCliProfile.ts（:25-57 区域）：capabilities.hooks 补两字段——
- configEditor = 桩组件：渲染可识别标记 data-e2e="mockcli-config-editor"；props 签名 = HooksConfigEditorProps（Stage 04 契约 2：{ profile; onDirtyChange?; askGuardRef? }）
- configLayers = 桩声明（如单层 [{ id: "user", label: "User", hint: "mock" }]——与 claude 三层区分开，证明数据源来自 profile）

【AC-4④ 重写】src/__tests__/mock-cli-profile.test.tsx:605-720 段——改为双向分派断言：
- 选中 mockcli → 渲染 mock 编辑器桩（data-e2e="mockcli-config-editor" 标记存在）+ 未渲染 ClaudeHooksConfigEditor（:244-245 的 mockJsonMode 零调用）
- 选中 claude → 仍渲染 claude 编辑器（mockJsonMode 被调用）+ mock 桩标记不存在

就近同步：src/__tests__/CLAUDE.md（mockCliProfile 桩能力描述 + AC-4④ 双向分派断言口径）。
禁止触碰 .claude/test-inventory.md——归 l4-mockcli 单点负责（你的 AC-4④ 重写用例数变化由它代登记）。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/mock-cli-profile.test.tsx 通过。`,
  },
  {
    label: 'l4-mockcli',
    prompt: `你负责 CS-3（L4 两条新用例 + E2E 夹具补桩）：

【夹具补桩】e2e-tests/helpers.ts:345-375（installMockCliProfile）：mockcli 定义补 configEditor + configLayers——
- configEditor = React.createElement 桩（helpers.ts 为 .ts 无 JSX），渲染 data-e2e="mockcli-config-editor" 标记（与 L2 桩同标记口径）；桩内提供保存动作入口（按钮调用 writeHooksConfig("mockcli", ...)——用于用例 ② 的 cliId 透传断言）
- configLayers = 桩声明（单层即可）
- 禁区 6：E2E_ENABLED 内联 import.meta.env 字面量形态不动（rolldown DCE 红线）

【用例 ① agent-event 注入】e2e-tests/mockcli.e2e.ts 新增：E2E helper 注册 mockcli → 打开终端面板 → 原子写信号文件（cliId="mockcli"、事件经桩策略映射 working）→ 断言页签 ⚡ emoji + 活跃区建行（真实 watcher → agent-event → 三级解析 → 桩策略全链路真实）。

【用例 ② hub 分派 + 保存透传】打开 hooksConfig 面板 → 选择行渲染 mockcli 按钮（hasConfigEditor=true 过滤命中）→ 点击 → 断言 mock 编辑器桩渲染（data-e2e="mockcli-config-editor"）→ 桩内保存动作触发 writeHooksConfig("mockcli", ...) → 断言后端「未知 cliId: mockcli」错误透传展示（mockcli 无后端 provider，错误即 cliId 全链携带的证据）。

【L4 豁免登记】review CS-3 建议的「历史条目展示」「双击恢复注入」两条 L4 不可行：历史条目由后端 provider 打标产出，生产二进制仅 claude provider（cliId 恒 "claude"），无 mockcli 后端 provider 则造不出 mockcli 历史行；为测试在生产留后门代价过大——豁免理由记入 test-inventory 豁免清单，注明兜底层级 = L2 AC-4③/⑤（mock-cli-profile.test.tsx）。

就近同步：
1. e2e-tests/CLAUDE.md（mockcli 用例清单 + 桩能力描述）
2. .claude/test-inventory.md（由你单点负责）：两条新 L4 用例登记 + L4 豁免两条（历史条目/恢复注入，含理由与兜底层级）+ 代 l2-mock-editor 登记 AC-4④ 重写后 L2 用例数变化
自查：npx tsc --noEmit 通过（helpers.ts 被 main.tsx 动态 import 在 vite 打包图内，语法级由 Stage 全量 L4 build:e2e 兜底——本阶段不做 e2e 实跑）。`,
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
