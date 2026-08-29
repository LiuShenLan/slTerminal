// =====================================================================
// Stage 02 — 设置中心框架 + 频率页（SC-FE-01..04）
// =====================================================================
// 改动项: SC-FE-01 SettingsPageRegistry+类型 / SC-FE-02 openSettings 编排+openSettingsPanel
//         / SC-FE-03 SettingsPanel 壳 / SC-FE-04 频率页+ipc wrapper
// 分工: pipeline 两相位——phase1 agent A{FE-01/02}（注册表+编排）→ phase2 agent B{FE-03/04}
//       （壳+频率页；B 依赖 A 的 SettingsPageProps 类型故串行，A/B 文件零重叠）
// 中间态: 本 Stage 不注册 panelRegistry（PANEL_TYPES 仍 6 含 hooksConfig），配置钮未切换
// 门禁: tsc + eslint + npm test
// fix-loop 调用约束: args.constraints 传
//   "中间态 Stage：panelRegistry.ts/ActivityBar.tsx 禁止触碰；pages.ts 只注册 planBalance"
// =====================================================================

export const meta = {
  name: 'stage02-settings-framework',
  description: 'Stage 02: 设置中心注册表/打开编排/壳面板/套餐余量频率页（F11 前端框架）',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中你负责的条目全文，严格按「修复步骤」执行（代码块为照抄级，禁止自行另设计）。
【Stage 特殊纪律】中间态 Stage：禁止触碰 panelRegistry.ts 与 ActivityBar.tsx（PANEL_TYPES 本 Stage 后仍含 hooksConfig）；pages.ts 只注册 planBalance 一页。重构阶段只做编译级检查（npx tsc --noEmit），npm test 由全量测试 agent 统一跑。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'fe-framework', prompt: `你负责 SC-FE-01 / SC-FE-02（逐 ID 照 docs/settings-center/checklist.md 条目执行）：

【SC-FE-01】新建 src/features/settingsCenter/ 三文件：
- types.ts：SettingsPageGroup / SettingsPageProps（onDirtyChange/pageParams/onPageParamsChange 三字段）/ SettingsPage（id/title/group/component/order?）——照 checklist 代码块逐字
- SettingsPageRegistry.ts：模块级单例 class（register 同 id 幂等覆盖 / getAll(group?) 按 order ?? 注册序 / get(id) / _reset() 仅测试）+ getSettingsPageRegistry() 惰性导出——硬约束 #13 家族契约，参照 ShortcutRegistry.ts:287-292 惰性单例先例
- index.ts barrel
- 测试：新建 src/__tests__/settings-page-registry.test.ts（注册/getAll 分组过滤/order 排序缺省注册序/重复 id 覆盖/_reset 隔离）

【SC-FE-02】打开编排：
- src/workspace/pageApis.ts :156 后新增 openSettingsPanel(pageId, settingsPageId?)——照 openHooksConfigPanel(:130-156) 模式：panelId = settings-\${pageId}；getPanel 命中 → focus?.() 返回 true；否则 addPanel({ id: panelId, component: "settings", title: "设置", params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) } })；100ms×50 轮询 + console.warn 降级
- 新建 src/features/settingsCenter/openSettings.ts：openSettings(settingsPageId?)——无项目 → toast.show("warning", "请先创建项目") + return（R1 修订，取代 openHooksConfigFromActivityBar 的静默 return）；其余编排照搬 features/hooksConfig/openHooksConfig.ts:28-58（活跃项目优先/兜底第一个/pages[0]/makeEmptyLayout 新建页/switchToPageShared → openSettingsPanel(pageId, settingsPageId)）
- 测试：新建 src/__tests__/open-settings.test.ts（无项目 toast 且不切页 / 活跃项目优先 / 兜底第一个项目 / 切页先于开面板 invocationCallOrder）+ src/__tests__/open-settings-panel.test.ts（addPanel 参数精确 {id:"settings-page-a",component:"settings",title:"设置",params:{panelId:"settings-page-a"}} / 单例 focus 不新建 / pageId 跟随 / 深链 settingsPageId 注入 selectedPage / 5s 超时降级）

自查：npx tsc --noEmit 零错。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（共享类型依赖：B 消费 A 产出的 SettingsPageProps/注册表）===
phase('串行重构')
const sequentialResults = []
for (const a of [
  { label: 'fe-shell-frequency', prompt: `你负责 SC-FE-03 / SC-FE-04（逐 ID 照 docs/settings-center/checklist.md 条目执行；SC-FE-01 的类型/注册表已由前序 agent 落地，直接 import 使用）：

【SC-FE-03】新建 src/panels/settings/{SettingsPanel.tsx, index.ts}：
- props { api: DockviewPanelApi; containerApi: DockviewApi; params?: { panelId?: string; selectedPage?: string; pageParams?: Record<string, Record<string, unknown>> } }
- 顶部 import "../../features/settingsCenter/pages"（side-effect import 注册触发点）
- selectedPage state：params.selectedPage 命中注册表 → 用之；否则全局组第一页；注册表空 → 空态「暂无配置页」
- 结构：左导航固定 180px（组序 global→project，组标题 + 页项 data-e2e=settings-nav-\${id} + dirty 圆点槽）+ 右槽位 key={selectedPage} 强制重挂载渲染 page.component，透传 { onDirtyChange, pageParams: params?.pageParams?.[selectedPage], onPageParamsChange }
- 壳是 params 持久化单点：persistParams(patch) = api.updateParameters({...params, ...patch}) + 显式 onLayoutChange(saveLayout(containerApi)) + 按 panelId settings- 前缀解析 pageId → updatePageLayout（照 HooksConfigPanel.tsx:97-106/:156-170 先例改前缀）
- onDidParametersChange 订阅外部 selectedPage 变化 → setState（扁平事件结构红线：回调直接是 Parameters）
- corrupted 警示条：挂载 loadSettings() → corrupted===true → 顶部警示条「设置文件已损坏，已从备份/默认值恢复」（× 可关，data-e2e="settings-corrupted-banner"，不阻塞）
- 配色全走 theme/colors.ts token（硬约束 #6，禁止硬编码颜色）
- 测试：新建 src/__tests__/settings-panel.test.tsx（导航组序 global 在前 / 选中渲染对应页 / 切换 persist（updateParameters+toJSON）/ params.selectedPage 失效回退全局组第一页 / corrupted 警示条渲染与关闭 / pageParams 透传与持久化 / 注册表空 → 空态）

【SC-FE-04】频率页 + ipc wrapper：
- src/ipc/planBalance.ts :30 后加 setPlanBalanceInterval(intervalSec) wrapper（invoke("plan_balance_set_interval", { intervalSec })）——照 checklist 代码块逐字
- 新建 src/features/settingsCenter/pages.ts（注册触发点）：register({ id: "planBalance", title: "套餐余量", group: "global", component: PlanBalancePage, order: 20 })——本 Stage 仅此一条注册
- 新建 src/panels/settings/pages/PlanBalancePage.tsx：挂载 loadSettings() → data?.planBalance?.intervalSec 有限数且 10-3600 → 显示，否则显示 60；失焦/Enter 提交：trim → Number 解析+整数+10-3600 → 非法行内红字「10–3600 秒，默认 60」不提交不 toast；合法 → setPlanBalanceInterval(v) → 成功后 refreshPlanBalance().catch(console.error) → Err → toast.show("warning", ...) + 保留用户输入
- 测试：新建 src/__tests__/settings-plan-balance.test.tsx（缺失/越界显示 60 / 合法提交调命令且 refresh / 非法行内红字不提交 / 命令 Err → toast+保留输入）；src/__tests__/ipc-plan-balance-contract.test.ts（已存在）加 setPlanBalanceInterval 四维契约（命令名逐字 / payload 键集合精确 {intervalSec:120} / 正常返回 / 异常传播）

自查：npx tsc --noEmit 零错。` },
]) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
中间态提醒：本 Stage 后 panelRegistry.ts 未被触碰（PANEL_TYPES 仍含 hooksConfig），pages.ts 仅注册 planBalance——按中间态判定，勿以终态误判。
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
