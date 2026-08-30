// =====================================================================
// Stage 04 设置中心页（FE-07 / FE-08）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - 设置页注册：id "backgroundTasks" / title "后台定时任务" / group "global" / order 20（替换 planBalance 页）
// - data-e2e 系列：settings-background-tasks-page / settings-background-tasks-row-{taskId} /
//   settings-background-tasks-enabled-{taskId} / settings-background-tasks-interval-{taskId} /
//   settings-background-tasks-error-{taskId}；footer 既有 plan-balance-footer / plan-balance-row 不动
// - 行内提示文案只写范围 ${intervalMin}–${intervalMax} 秒（DTO 无 default 字段，不写默认值）
// - 提交闭环：set_config 返回完整清单 → 更新行；sessionRefresh 直调 backgroundTaskScheduler.applyConfig；
//   planBalance 调 refreshPlanBalance()（反馈闭环先例）
// - 退役：settings 页 id "planBalance" / settings-plan-balance-* 选择器 / PlanBalancePage.tsx 全删（src/ 内）
// fix-loop constraints（execution-plan.md）：本 Stage 无特殊纪律，传空串
export const meta = {
  name: 'stage04-settings-page',
  description: 'F12 设置中心「后台定时任务」页：planBalance 页替换为通用任务行页 + 注册/测试适配',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读 docs/background-tasks/checklist.md 再动手）。
补充纪律：本 Stage 单 agent（页组件/注册/测试高度耦合）；配色一律经 src/theme facade token 引用（硬约束 #6，禁硬编码颜色）；删除文件用 git rm；e2e-tests/ 不在本 Stage 触碰范围（Stage 05 处理）。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'settings-page',
    prompt: `你负责 FE-07 / FE-08，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目（含可照抄代码块），逐条实现：

【FE-07】设置中心「后台定时任务」页替换：
- 3.1 src/features/settingsCenter/pages.ts：删 PlanBalancePage import 与注册段（:9, :22-28），替换为 BackgroundTasksPage import + 注册段（id "backgroundTasks" / title "后台定时任务" / group "global" / order 20，照 checklist 3.1 代码块）；头注释「Stage 02 仅注册 planBalance 一页」过时表述改写为现行三页口径。
- 3.2 新建 src/panels/settings/pages/BackgroundTasksPage.tsx（照 checklist 3.2 完整代码块）：TaskRow 通用行组件（纯渲染——listBackgroundTasks 返回清单 map 渲染，新增任务自动出现）；勾选立即提交 setBackgroundTaskConfig(taskId, { enabled })；频率失焦/回车提交，非法（非数/非整数/越界/空串）→ 行内红字 = rangeHint（${intervalMin}–${intervalMax} 秒，无默认值）不提交不 toast；后端拒绝 → toast + 保留输入；afterCommitted 公共收尾（返回清单更新行 + sessionRefresh 调 backgroundTaskScheduler.applyConfig + planBalance 调 refreshPlanBalance() 反馈闭环）；data-e2e 五系列选择器逐字照抄；配色 import 自 ../../../theme（PANEL_BG/SIDEBAR_FG/DIM_FG/INPUT_BG/INPUT_BORDER/FOCUS_BORDER/ERROR_FG）。
- 3.3 git rm src/panels/settings/pages/PlanBalancePage.tsx 与 src/__tests__/settings-plan-balance.test.tsx。
- 3.4 src/__tests__/settings-pages-registration.test.ts：mock 路径 ../panels/settings/pages/PlanBalancePage 改 ../panels/settings/pages/BackgroundTasksPage；断言数组 { id: "planBalance", group: "global", order: 20 } 改 { id: "backgroundTasks", group: "global", order: 20 }（断言计数不变 3 条）。
- 3.5 新建 src/__tests__/settings-background-tasks.test.tsx（照 settings-plan-balance.test.tsx 模式）：mock ../ipc/backgroundTasks（list/set hoisted 可控）+ ../ipc/planBalance（refreshPlanBalance）+ ../lib 仅替换 toast + mock ../features/backgroundTasks（backgroundTaskScheduler.applyConfig hoisted spy——避免真实调度器）。用例：挂载渲染两行（任务标题 + 勾选态 + 频率输入回显 + 范围提示「10–3600 秒」「2–300 秒」）；list 失败空态不崩 + console.error；勾选 planBalance → setBackgroundTaskConfig("planBalance", { enabled: false }) + 成功后行更新 + refreshPlanBalance 被调；勾选 sessionRefresh → applyConfig 被调且参数为返回清单新值；频率非法（非数/小数/越界/空串）→ 行内红字 = 范围文案 + 不提交不 toast + 输入保留；频率合法 → setBackgroundTaskConfig + 规范化回显；set reject → toast warning + 输入保留。

【FE-08】纯验证项（零代码改动）：Read src/panels/settings/SettingsPanel.tsx:181-185 确认深链失配回退逻辑仍在（saved 页 id 经 registry.get(saved) 校验，失配回退 global 组第一页）——现状已满足，不改代码；settings-panel.test.tsx 既有「selectedPage 失效 → 回退全局组第一页」用例保留即可。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
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
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
