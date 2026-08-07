// =====================================================================
// Stage 05 测试补全：scheme-registry/overrides 测试新增 + 四文件零改动验证 + inventory 同步（TST-02~05）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   被测对象 = Stage 01/02 产物：schemeRegistry 七方法 API（register/
//   get/getActive/setActive/getAll/getDefaultId/_reset）+ overrides.ts
//   四导出（dockviewVarStyle/allotmentVarStyle/editorTheme/
//   editorColorOverrides）+ darcula 四段形状（C3，见 Stage 01 脚本头）。
// fix-loop 调用约定：args.constraints 传『本 Stage 只改测试，禁止改生产代码』。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage5-tests',
  description: 'Stage 05 测试补全：scheme-registry/overrides 测试新增 + 四文件零改动验证 + inventory 同步（TST-02~05）',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
本 Stage 只改测试文件与 .claude/test-inventory.md，禁止改生产代码。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-registry-test',
    prompt: `你负责 TST-02（新建 src/__tests__/scheme-registry.test.ts）。先 Read docs/color-plan/checklist.md 中 TST-02 条目、src/theme/schemeRegistry.ts 与 src/theme/schemes/darcula.ts 被测实现，再 Read src/__tests__/tab-title-registry.test.ts 作为测试模式先例。

新建 src/__tests__/scheme-registry.test.ts，约 15 用例（不少于 12），覆盖：
① register / get / getAll / getDefaultId 基本行为；
② setActive 已知 id 切换生效（getActive 返回新方案）；
③ setActive 未知 id → 回退 darcula + console.warn（vi.spyOn(console, "warn") 断言）；
④ getActive 默认返回 darcula；
⑤ 重复注册同 id 覆盖；
⑥ _reset() 隔离（照 tab-title-registry.test.ts 先例，beforeEach 调 _reset）；
⑦ darcula 四段完整性：ui 6 组键数 7/3/5/8/3/3 + 23 标量、terminal 25 键、editor.theme 透出非 undefined、libraries.dockview 20 条 + libraries.allotment 2 键。
断言值与 darcula.ts 实值对齐（先 Read 再写断言，禁凭猜测）。完成后报告：用例数 + 覆盖面清单。`,
  },
  {
    label: 'A2-overrides-test',
    prompt: `你负责 TST-03（新建 src/__tests__/overrides.test.ts）。先 Read docs/color-plan/checklist.md 中 TST-03 条目、src/theme/overrides.ts 被测实现与 src/theme/schemes/darcula.ts 值来源。

新建 src/__tests__/overrides.test.ts，约 6 用例（不少于 5），覆盖：
① dockviewVarStyle() 键集合恰 20 条且值与 active 方案 libraries.dockview 一致；
② allotmentVarStyle() 恰 2 键；
③ editorTheme === active 方案 editor 段 theme（引用相等）；
④ editorColorOverrides() 返回 CM6 扩展且 lint/searchMatch/background 键生效（EditorState.create 验证，照 src/__tests__/use-code-mirror.test.ts 的扩展断言模式）；
⑤ setActive 后输出跟随切换（临时注册一个改单色的测试方案 → setActive → 四导出输出变化 → _reset 还原）。
与 TST-02 并行、两文件独立，不互相 import；测试内 _reset() 隔离照 tab-title-registry.test.ts 先例。完成后报告：用例数 + 覆盖面清单。`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构 ===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'A3-zero-change-inventory',
    prompt: `你负责 TST-04/TST-05（预期零改动验证 + inventory 同步）。先 Read docs/color-plan/checklist.md 中 TST-04/05 条目。

【TST-04】四文件预期零改动验证——theme.test.ts（13 例）/ main-bootstrap.test.tsx（1 例）/ gitshow-panel.test.tsx 与 hooks-config-jsonmode.test.tsx 的 oneDark mock / test/terminal/theme-options.test.ts（L3 5 例）：
① git diff 核对四文件是否被本重构链改动（预期零改动）；
② 逐文件跑测试确认绿：npx vitest run theme.test main-bootstrap gitshow-panel hooks-config-jsonmode 与 npm run test:l3；
③ 任一失效即停手——先查是否值漂移（值漂移属重构失败信号，报告并禁止改测试迎合）；仅当确认是合理的来源变化（如 import 路径）才可改测试，且必须在报告中附理由。

【TST-05】.claude/test-inventory.md 同步：登记 scheme-registry.test.ts 与 overrides.test.ts 两新文件及其实际用例数；colors.test.ts 用例数由 89 更新为实际值（跑 npx vitest run colors 数断言或读文件统计）。

完成后报告：四文件 git diff 结论 + 逐文件测试通过证据 + inventory 改动摘要。`,
  },
];
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
- npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
