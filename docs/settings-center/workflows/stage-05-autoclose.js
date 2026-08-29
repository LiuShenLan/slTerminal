// =====================================================================
// Stage 05 — 切项目自动关闭（SC-FE-08）
// =====================================================================
// 改动项: SC-FE-08 SettingsPanel 壳 effect 切项目自动关闭
// 分工: 单 agent（SettingsPanel.tsx + 1 新测试）
// 门禁: tsc + eslint + npm test
// fix-loop 调用约束: args.constraints 传
//   "只动 SettingsPanel.tsx 壳 effect 与其新测试文件，不顺手改其他面板逻辑"
// =====================================================================

export const meta = {
  name: 'stage05-autoclose',
  description: 'Stage 05: 切项目自动关闭设置中心面板（F11）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中 SC-FE-08 条目全文，严格按「修复步骤」执行。
【Stage 特殊纪律】只动 SettingsPanel.tsx 壳 effect 与其新测试文件，不顺手改其他面板逻辑。`

// === Phase 1: 并行重构（单 agent Stage；重构 agent 不跑全量测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'fe-autoclose', prompt: `你负责 SC-FE-08（照 docs/settings-center/checklist.md 条目执行）：

src/panels/settings/SettingsPanel.tsx 加壳 effect（订阅 useLayout(s=>s.activePageId) + useProjects）：
1. ownPageId = params.panelId 去 settings- 前缀；ownProjectId = projects 反查
2. activeProjectId = activePageId 所属项目；activePageId === null → 不动（删除末页/启动瞬态，防连锁误关）
3. activeProjectId 与 ownProjectId 均非空且不同 → dirty 守卫（isSettingsDirty → confirmDialog，取消则不关——面板暂留非活跃项目，尊重用户选择，代码注释注明此决策）→ api.close()
4. 初始评估：挂载时 activeProjectId 已定且不一致（布局恢复场景）→ 直接 api.close() 静默（新挂载不可能 dirty）

测试：新建 src/__tests__/settings-panel-autoclose.test.tsx（切项目 → api.close 调用 / 同项目切页 → 不关 / 初始不一致 → 挂载即静默关 / activePageId null → 不关 / dirty confirm 取消 → 不关）

自查：npx tsc --noEmit 零错 + npx eslint src/panels/settings/ 零警告。` },
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

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
