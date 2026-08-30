// =====================================================================
// Stage 03 E2E 隔离 + L4 补强（TE-02 / TE-03 / FE-04）
// =====================================================================
// fix-loop 调用约定：args.constraints 无需传值（无特殊纪律）
// 跨 agent 契约（写死，三方不各自推断）：
//   - env 名 SLTERM_DATA_DIR（Stage 01 BE-01 后端已落地，本 Stage 只消费）
//   - × 按钮选择器 [data-e2e="tab-close-{panelId}"]（FE-04 提供，TE-03 用例⑪消费）
//   - helper 名 __slterm_e2e_setSettingsDirty(panelId, dirty)；
//     getSettingsPanelState 返回新增 panelId 字段（TE-03 自给）
//   - ConfirmDialog 选择器：confirm-dialog / confirm-cancel / confirm-ok

export const meta = {
  name: 'stage03-e2e-isolation',
  description: 'Stage 03 E2E 隔离：SLTERM_DATA_DIR 数据目录 + ×关闭 dirty 守卫 L4 用例 + 用例⑧强化',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）；禁止前端 src/ipc/ 外出现 invoke；禁止硬编码颜色（经 theme/colors.ts token）；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）。
背景：修复要点详见 docs/settings-center-fixes/checklist.md 对应 ID 条目（先读再动手）。
测试纪律：禁止各自跑 npm test / build:e2e / wdio——真实执行统一由后续全量测试 agent 单点跑。
跨 agent 契约：env 名 SLTERM_DATA_DIR；× 按钮选择器 [data-e2e="tab-close-{panelId}"]（FE-04 提供）；helper 名 __slterm_e2e_setSettingsDirty(panelId, dirty)；getSettingsPanelState 返回新增 panelId 字段；ConfirmDialog 选择器 confirm-dialog / confirm-cancel / confirm-ok。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'te-02-wdio-isolation',
    prompt: `你负责 TE-02（照抄 docs/settings-center-fixes/checklist.md 的 TE-02 条目）：
【TE-02】e2e-tests/run-wdio.cjs SLTERM_DATA_DIR 隔离 + 清理失实备份
- 位置：e2e-tests/run-wdio.cjs（失实备份段约 :70-71、projects 备份三件套约 :104-121、还原段约 :135-151、settings 还原段约 :152-155——先 Read 全文确认现状行号再动手）
- 步骤：
  ① 文件头常量区新增（可照抄 checklist TE-02 代码块）：e2eDataDir = path.join(os.tmpdir(), 'slterm-e2e-data')，rmSync force 清理 + mkdirSync recursive 创建 + process.env.SLTERM_DATA_DIR = e2eDataDir；位置必须在 spawn wdio（fallback()）之前（env 链式继承：run-wdio → npx wdio → tauri driver → slterminal.exe）
  ② 删除四段备份还原代码：~/.slterminal/settings.json 备份段（失实对象，BE-16 便携化后应用读写 exe 同级）+ projects.json/.bak 备份清空段 + 对应两还原段
  ③ 进程退出清理 e2eDataDir（rmSync recursive force）
  ④ 文件头注释同步新隔离机制
- 你只改 e2e-tests/run-wdio.cjs。`,
  },
  {
    label: 'te-03-e2e-cases',
    prompt: `你负责 TE-03（照抄 docs/settings-center-fixes/checklist.md 的 TE-03 条目）：
【TE-03】e2e-tests/settings.e2e.ts + e2e-tests/helpers.ts 补强
- 位置：e2e-tests/settings.e2e.ts :64（settingsJsonPath 硬编码）、:575-606（用例⑧）；e2e-tests/helpers.ts :305-344（installSettingsPanelHelpers）——先 Read 原文确认现状
- 步骤：
  ① settings.e2e.ts :64 改为 \`join(process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug"), "settings.json")\`；:213-243 suite 级快照还原段保留（直跑 wdio 无 env 时兜底，路径改 env 推导后自动跟随）
  ② helpers.ts getSettingsPanelState 返回值加 \`panelId: panel.id\`；新增后门 helper \`window.__slterm_e2e_setSettingsDirty = (panelId, dirty) => { setSettingsDirty(panelId, dirty); }\`（import setSettingsDirty from src/features/settingsCenter/dirtyRegistry——先 Read 该模块确认导出名与 helpers.ts 的 import/注入结构现状）
  ③ 用例⑧补断言：切页前后 helper 读到的 panelId 恒为 \`settings-\${pageIdA}\`
  ④ 新增用例⑪ × 关闭 dirty 守卫：createProject → openSettingsCenter → 后门 __slterm_e2e_setSettingsDirty 置 dirty → 点 [data-e2e="tab-close-settings-{pageId}"]（FE-04 提供的契约选择器）→ confirm-dialog 出现 → confirm-cancel 面板保留 → 再点 × → confirm-ok 面板关闭
- 你只改 e2e-tests/settings.e2e.ts 与 e2e-tests/helpers.ts。`,
  },
  {
    label: 'fe-04-tab-close-e2e',
    prompt: `你负责 FE-04（照抄 docs/settings-center-fixes/checklist.md 的 FE-04 条目）：
【FE-04】src/workspace/PageDockviewHost.tsx × 按钮 data-e2e + 注释修正
- 位置：src/workspace/PageDockviewHost.tsx × button（约 :459-479）、filePath 判据注释（约 :422-425）——先 Read 原文确认 tabParams 取值变量名
- 步骤：
  ① × button 加 data-e2e：panelId 存在时为 \`tab-close-\${panelId}\`，否则 "tab-close"（模板字符串）
  ② 注释「terminal/hooksConfig 恒不设置」改为「terminal/settings 恒不设置」
- 你只改 src/workspace/PageDockviewHost.tsx。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。命令分两组——组内并行、组间串行（build:e2e 必须先于 wdio 实跑）：
组一（并行启动）：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
组二（组一完成后串行）：
4. npm run build:e2e
5. npx wdio run wdio.conf.ts --spec e2e-tests/settings.e2e.ts
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。wdio 实跑可能耗时数分钟，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center-fixes/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
