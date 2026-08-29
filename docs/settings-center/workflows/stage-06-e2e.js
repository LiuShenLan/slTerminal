// =====================================================================
// Stage 06 — E2E 完整覆盖（SC-E2E-01..02）
// =====================================================================
// 改动项: SC-E2E-01 helpers 扩展 / SC-E2E-02 settings.e2e.ts 新建 + hooks/mockcli 适配
// 分工: pipeline 两相位——phase1 agent A{helpers} → phase2 agent B{specs}
//       （B 依赖 A 的 helper 故串行，A/B 文件零重叠）
// 门禁: tsc + eslint + npm test + npx vite build + npm run e2e（全量）
//   ——helpers.ts 在根 tsconfig include 外，vite build 为构建级兜底；
//   ——npm run e2e = build:e2e（VITE_E2E=1）+ wdio 一体化，耗时长属正常勿中止
// fix-loop 调用约束: args.constraints 传
//   "E2E Stage：修复后必须重跑 npm run e2e 全量（build:e2e 串行）；禁改生产代码逻辑
//    适配测试——发现生产缺陷报告主 agent 回退对应 Stage"
// =====================================================================

export const meta = {
  name: 'stage06-e2e',
  description: 'Stage 06: 设置中心 E2E helpers + settings.e2e.ts 10 用例 + hooks/mockcli 适配（F11 L4）',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中你负责的条目全文，严格按「修复步骤」执行。
【Stage 特殊纪律】E2E helpers/spec 改动禁止反向修改生产代码适配测试——发现生产缺陷（如选择器不存在）报告主 agent，由主 agent 回退对应 Stage 修复。重构阶段不跑 npm run e2e（全量测试 agent 统一跑）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'e2e-helpers', prompt: `你负责 SC-E2E-01（照 docs/settings-center/checklist.md 条目执行）：

e2e-tests/helpers.ts（:265-283 installSettingsHelpers 旁）新增 installSettingsPanelHelpers()：
- __slterm_e2e_openSettings(): Promise<void>（调 openSettings——import 自 src/features/settingsCenter/openSettings）
- __slterm_e2e_getSettingsPanelState(): { selectedPage: string | null } | null（经 __dockviewApi 查 settings- 前缀面板读 params）
- __slterm_e2e_getSettingsPanelCount(): number（全部页面 api 计数）
- __slterm_e2e_switchSettingsPage(id: string): boolean（DOM 点击 settings-nav-\${id}）
- installAllE2eHelpers 接线

测试适配：src/__tests__/app.test.tsx helper 契约测试加新 helper 存在性断言；e2e-gating 相关测试适配（如 helper 清单计数断言存在则同步）。

自查：npx vite build 打包过（helpers.ts 在根 tsconfig include 外——vite 打包图为唯一编译级门禁，必须跑）。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（B 依赖 A 的 helper）===
phase('串行重构')
const sequentialResults = []
for (const a of [
  { label: 'e2e-specs', prompt: `你负责 SC-E2E-02（照 docs/settings-center/checklist.md 条目执行；SC-E2E-01 的 helper 已由前序 agent 落地，直接调用）：

1. 新建 e2e-tests/settings.e2e.ts（10 用例，照 checklist 逐条）：
   ① 配置钮打开（设置面板存在 + 默认全局组第一页）
   ② 再点 → 单例（count=1）
   ③ 切页 → params.selectedPage 持久化（helper 读）
   ④ 频率页 120 失焦 → 真实后端落盘（loadSettings 读段 120）+ 余量刷新
   ⑤ 频率页 5 → 行内红字 + 文件未变
   ⑥ 快捷键录制（dispatch 合成 KeyboardEvent Ctrl+Alt+KeyC）→ 生效键更新 + 2s debounce 后落盘断言
   ⑦ 切项目 → 老面板关闭（count=0）→ 新项目配置钮 → pages[0] 打开
   ⑧ 同项目切页 → 保留
   ⑨ hooks 页迁入冒烟（设置中心内 CLI 选择行渲染）
   ⑩ hooks 页 dirty → 切配置页 → confirm 弹窗 → 取消 → 未切换
   corrupted 警示条不做 L4（L2 已覆盖；无沙箱外写坏文件通道——豁免登记归 Stage 07）
2. e2e-tests/hooks.e2e.ts 适配（消费面见 checklist FB-19）：
   - 程序化 addPanel 形态改 { id: "settings-e2e-...", component: "settings", title: "设置", params: { panelId, selectedPage: "hooks" } }
   - 清理分支 p.component === "hooksConfig" 改 "settings"
   - [data-e2e="hooks-config-panel"] 选择器不变（HooksSettingsPage 根容器保留该 data-e2e）
3. e2e-tests/mockcli.e2e.ts 适配（:312-407 CS-3 用例② hub 分派）：同 hooks.e2e.ts 两处适配点
4. __slterm_e2e_setHooksConfigJson 选择器 [data-e2e="hooks-json-editor"] 不变（JsonMode 挂载点 data-e2e 随组件迁移保留）——实读确认 JsonMode 容器属性未变，变了报告主 agent（禁改生产代码）

自查：不跑 npm run e2e（全量测试 agent 统一跑）；npx tsc --noEmit + npx eslint e2e-tests/（如 eslint 覆盖）语法级自检。` },
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
4. npx vite build
5. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
注意：npm run e2e = build:e2e（VITE_E2E=1 tauri build --debug --no-bundle）+ wdio 一体化，构建+全量 spec 耗时长（十分钟级）属正常，禁止中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
