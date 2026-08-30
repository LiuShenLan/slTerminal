// =====================================================================
// Stage 04 L2 测试质量（TE-04 / TE-05 / TE-06）
// =====================================================================
// fix-loop 调用约定：args.constraints 传「本 Stage 只改测试，禁止改生产代码」
// 跨 agent 契约：无（三 agent 文件零重叠）

export const meta = {
  name: 'stage04-l2-test-quality',
  description: 'Stage 04 L2 测试质量：pages 注册守卫 + 短路用例 + saveLayout 断言 + 死 mock 清理',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
本 Stage 只改测试（src/__tests__/ 下文件），禁止改生产代码。
禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）；禁止前端 src/ipc/ 外出现 invoke；禁止硬编码颜色（经 theme/colors.ts token）；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）。
背景：修复要点详见 docs/settings-center-fixes/checklist.md 对应 ID 条目（先读再动手）。
测试纪律：禁止各自跑 npm test——真实执行统一由后续全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'te-04-pages-guard',
    prompt: `你负责 TE-04（照抄 docs/settings-center-fixes/checklist.md 的 TE-04 条目）：
【TE-04】新建 src/__tests__/settings-pages-registration.test.ts
- 被测：src/features/settingsCenter/pages.ts（真实注册：keybindings global:10 / planBalance global:20 / hooks project:100）
- 步骤：新文件——**不** mock pages 自身；mock 三个页面组件模块为 () => null（组件实现不属本测试面；先 Read pages.ts 确认三组件的 import 路径再写 vi.mock）；import 真实 pages.ts 触发 side-effect 注册；断言 getSettingsPageRegistry().getAll() 精确包含三条 {id, group, order}（{id:"keybindings",group:"global",order:10} / {id:"planBalance",group:"global",order:20} / {id:"hooks",group:"project",order:100}——先 Read pages.ts 核对实际注册值再写断言）；afterEach 调注册表 _reset()（硬约束 #13 注册表契约）。
- 你只新建 src/__tests__/settings-pages-registration.test.ts。`,
  },
  {
    label: 'te-05-hooks-page-tests',
    prompt: `你负责 TE-05（照抄 docs/settings-center-fixes/checklist.md 的 TE-05 条目）：
【TE-05】settings-hooks-page 短路用例 + 两文件死 mock 清理
- 位置：src/__tests__/settings-hooks-page.test.tsx（死 mock 约 :40-52、selectedCli 用例区约 :782-796）、src/__tests__/hooks-config-sync.test.tsx（死 mock 约 :56-67）——先 Read 原文确认现状（死 mock 定义与全部 reset 引用点）
- 步骤：
  ① 两文件删 mockApi/mockContainerApi 定义与全部 reset 引用（Grep 两文件全部引用点逐一清除，只删死代码，保留 mockOnPageParamsChange）
  ② settings-hooks-page.test.tsx 新增用例「点击当前已选中 CLI 不触发 onPageParamsChange」：渲染 HooksSettingsPage 时 selectedCli 传当前选中值 → 点击同一 CLI 项 → 断言 mockOnPageParamsChange 零调用（短路实现 HooksSettingsPage.tsx:132 已存在，只测不改）
- 你只改 src/__tests__/settings-hooks-page.test.tsx 与 src/__tests__/hooks-config-sync.test.tsx。`,
  },
  {
    label: 'te-06-panel-assertions',
    prompt: `你负责 TE-06（照抄 docs/settings-center-fixes/checklist.md 的 TE-06 条目）：
【TE-06】settings-panel.test.tsx 补 saveLayout 落盘断言 + 不可变用例
- 位置：src/__tests__/settings-panel.test.tsx pageParams patch 用例（约 :248-270）——先 Read 原文确认现状（renderPanel 返回 { api, containerApi } 结构）
- 步骤：
  ① pageParams patch 用例补 \`expect(containerApi.toJSON).toHaveBeenCalled()\`（saveLayout 触发源断言——先 Read SettingsPanel.tsx persistParams 确认 toJSON 确为 saveLayout 链路触发点）
  ② 新增用例「onPageParamsChange 合并既有 params 且不修改原对象」：renderPanel 携带初始 params → 触发 patch → 断言 updateParameters 合并结果含原键 + 原 params 对象引用未被改写
- 你只改 src/__tests__/settings-panel.test.tsx。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center-fixes/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
