// =====================================================================
// Stage 06 L2-terminal：去重 + webgl + mock 清理
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-06.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试与测试辅助（xterm-test-utils.ts），不改生产代码；若发现生产代码缺陷，报告主 agent 后另行处理
// =====================================================================

export const meta = {
  name: 'stage06-l2-terminal',
  description: 'L2 useXterm 去重 + webgl 全分支 + mock 清理',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试与测试辅助（D2 边界：测试用 mock 隔离，生产代码零改动）。并行 agent 文件零重叠（term-xterm 碰 lifecycle/output/e2e-gating 测试 + helpers；term-panel 碰 terminal/detectWebgl/useTerminalInstance/registry 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'term-xterm',
    prompt: `你负责 TRM-01、TRM-02、TRM-03、TRM-04，触碰文件：src/__tests__/use-xterm-lifecycle.test.ts、src/__tests__/use-xterm-output.test.ts、src/__tests__/e2e-gating-terminal.test.ts、src/__tests__/helpers/xterm-test-utils.ts。逐 ID 对照 checklist 原文实施：

【TRM-01】use-xterm-lifecycle 与 use-xterm-output 14 条重复用例。cancelPendingFlush/ResizeObserver 合帧等 14 条近逐字复制（CPF1/CPF2/CPF5-CPF16 + ResizeObserver 合帧）。去重归位（合帧属 output、生命周期属 lifecycle）；await Promise.resolve() 微任务时序假设脆弱——时序统一改用 fake timers 或显式 flush helper 替代裸 await Promise.resolve()。

【TRM-02】setBufferType("alternate") 虚假测试 + 死辅助删除。位置 lifecycle 测试 + helpers/xterm-test-utils.ts。源码从不读 terminal.buffer.type，测试仅给 mock 挂不会被读取的属性。删除虚假用例与 setBufferType 死辅助；交替缓冲行为改由 resize/fit 链路断言（真实读取路径）。

【TRM-03】mock 混入不属于目标模块的 hooks: 字段。位置 use-xterm-output.test.ts:136,162,175、e2e-gating-terminal.test.ts:20,113,121,134。@xterm/addon-fit/TerminalRegistry/TabTitleRegistry/e2eEnabled mock 被 copy-paste 混入 hooks: 虚假字段。删除全部 mock 中目标模块未导出的字段；确需的依赖在测试内单独 mock。

【TRM-04】usePtyOutput 64KB 淘汰 + 退出码分支 + E2E 缓冲截断。位置 src/panels/terminal/usePtyOutput.ts:191-217。pendingBufSizeRef 超 MAX_PENDING_BYTES(64KB) 丢弃最旧块、PTY 退出码传递（含 0 与非空数字）、isCommandRunningRef=false 时 E2E 缓冲行数截断均未测。补 64KB 淘汰（恰好/超过/多块）、退出码透传、缓冲截断三用例。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'term-panel',
    prompt: `你负责 TRM-05、TRM-06、TRM-07、TRM-08、NAH-02，触碰文件：src/__tests__/terminal.test.tsx、src/__tests__/detect-webgl.test.ts（及新增 webgl 测试文件）、useTerminalInstance 相关测试（src/__tests__/terminal-lifecycle.test.ts 或等价）、src/__tests__/terminal-registry.test.ts。逐 ID 对照 checklist 原文实施：

【TRM-05】TerminalPanel 分支覆盖 42.85%。位置 src/panels/terminal/TerminalPanel.tsx。1.5s 超时隐藏加载遮罩、handleTabStateChange active=false 恢复原标题、windowsPty 更新分支未测。fake timers 补超时遮罩；补 active=false 标题恢复与 windowsPty 更新断言。

【TRM-06】webgl.ts 26.4%——setupWebglWithRetry 核心路径零覆盖。位置 src/panels/terminal/webgl.ts。context loss 指数退避、重试耗尽回退 DOM、cancel() 清定时器全部未测。fake timers 补退避序列/耗尽回退/cancel 清理全分支；L4 真实 context loss 场景归 E2E-04（本 Stage 不做）。

【TRM-07】useTerminalInstance 多分支未覆盖。位置 src/panels/terminal/useTerminalInstance.ts。fonts.ready catch、fontSize undefined、prevFontSize 相同跳过、webglAddon 已存在不重复加载分支未测。四分支各补一条。

【TRM-08】TerminalRegistry getAll/_size/_dump 未覆盖。位置 src/panels/terminal/TerminalRegistry.ts。terminal-registry.test.ts 补轻量断言（getAll 只读视图、_size 计数、_dump 不抛）；或 JSDoc @internal 标注仅供测试/调试（二选一）。

【NAH-02】TerminalRegistry.setClaudeSession merge 语义未断言。位置 TerminalRegistry.ts。undefined 字段不覆盖旧值、缺 lastEventAt 自动填充、null 清空三语义未锁（F5 双通道建行/三通道删行核心保证）。先全量 set 再增量 { status: "working" }，断言 transcriptPath 保留 + lastEventAt 更新；null 清空单独断言。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
