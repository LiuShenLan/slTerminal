// =====================================================================
// Stage 11 L2-hooks-config：竞态与校验链
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-11.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试；若发现生产代码缺陷，报告主 agent 后另行处理
// =====================================================================

export const meta = {
  name: 'stage11-l2-hooks-config',
  description: 'L2 hooks-config linter 顺序/generation 竞态/校验链补齐',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试。并行 agent 文件零重叠（hk-data 碰 hooks-config-panel/sync/schema（新建）/entry 测试；hk-ui 碰 jsonmode/gui/handlerform/eventtree 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'hk-data',
    prompt: `你负责 HKC-02、HKC-03、HKC-07、HKC-08、HKC-09，触碰文件：hooks-config-panel/sync 相关测试（src/__tests__/hooks-config-panel.test.tsx、hooks-config-sync.test.tsx）、src/__tests__/hooks-config-schema.test.ts（新建）、hooks-config-entry 测试（open-hooks-config-panel.test.ts）。逐 ID 对照 checklist 原文实施：

【HKC-02】useHooksConfig.load() generation 竞态无守卫。位置 src/panels/hooksConfig/useHooksConfig.ts:110-129。模拟旧请求延迟 resolve，断言最终 configJson 为目标层数据（过期结果被丢弃）：先挂起旧层 read → 切层 → 新层 resolve → 旧层延迟 resolve。

【HKC-03】HooksConfigPanel.handleJsonChange 非法 JSON catch 无回归。位置 HooksConfigPanel.tsx:146-155。onChange 传非法文本，断言 configJson 保持原快照、保存按钮禁用、不崩溃。

【HKC-07】handleUninstall 失败分支未覆盖。位置 HooksConfigPanel.tsx:239-252。补 uninstall reject → 错误提示（hooks-injection-error 出现"卸载失败"文案）+ 状态条不变用例。

【HKC-08】validateHooksJson 直接边界未覆盖。位置 src/features/hooksConfig/schema/index.ts:61-79。新建 hooks-config-schema.test.ts，直测 validateHooksJson（合法/缺 hooks 键/非法 matcher/未知事件告警边界：handler type 不在枚举、command handler 缺必填 command、http handler 缺必填 url、顶层数组拒绝、空对象合法通过）。

【HKC-09】open-hooks-config-panel getPanel 无 focus 降级未覆盖。位置 open-hooks-config-panel 相关测试。补 getPanel 命中但无 focus 方法时的降级路径断言（不抛错、addPanel 不再调用）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'hk-ui',
    prompt: `你负责 HKC-01、HKC-04、HKC-05、HKC-06、HKC-10，触碰文件：src/__tests__/hooks-config-jsonmode.test.tsx、hooks-config-gui.test.tsx、hooks-config-handlerform.test.tsx、hooks-config-jsonmode/eventtree 相关测试。逐 ID 对照 checklist 原文实施：

【HKC-01】JsonMode linter 包装顺序未锁定。位置 hooks-config-jsonmode.test.tsx:158-181。只断言 options，未锁定 [0]=jsonParseLinter、[1]=jsonSchemaLinter——交换后语法错误进 schema linter，误报误导。追加 linterCalls[0][0]/linterCalls[1][0] 身份断言。

【HKC-04】HandlerForm record/stringArray 清空删键未覆盖。位置 src/panels/hooksConfig/HandlerForm.tsx:337-342。补字段清空 → 对象中删键（非置空）用例（渲染 command 型 handler 填 args/headers 再清空 textarea → onChange 新对象无该键）。

【HKC-05】GuiMode 删除选中项后选中态重置未覆盖。位置 GuiMode.tsx:229-244、267-293。补删除当前选中事件/handler → 选中态回退空态用例（先选中 matcher 组/handler 再删除 → 详情区回退空态/HandlerForm 消失）。

【HKC-06】EventTree 未知事件分组未覆盖。位置 EventTree.tsx:143-154。补配置含未知事件（group 为 UnknownGroup）→ 归「未知事件」组渲染用例。

【HKC-10】展示分支杂项：补 JsonMode schema hover 触发、注入状态条初始 "--"、MatcherTester placeholder 随事件变化三处展示断言。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-11.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage11 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-11.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
