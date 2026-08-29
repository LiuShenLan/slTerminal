// =====================================================================
// Stage 07 — 文档同步（SC-DOC-01..05）
// =====================================================================
// 改动项: SC-DOC-01 需求规格回写 / SC-DOC-02 CONTEXT+根 CLAUDE / SC-DOC-03 ADR-0012
//         / SC-DOC-04 模块 CLAUDE.md 群 / SC-DOC-05 test-inventory 校准
// 分工: 2 agent 并行（A=根级文档 / B=模块文档+清单），文件零重叠
// 门禁: tsCheck + eslint + clippy + rustfmt + frontendTest + rustTest + l3Test
//   ——L4 计数沿用 Stage 06 的 npm run e2e 结果（本 Stage 不动代码，收窄取数口径）
// fix-loop 调用约束: args.constraints 传
//   "本 Stage 只改文档（md），禁止改生产代码与测试代码"
// =====================================================================

export const meta = {
  name: 'stage07-docs',
  description: 'Stage 07: F11 文档同步——规格回写/CONTEXT/ADR-0012/模块 CLAUDE 群/用例清单校准',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中你负责的条目全文，严格按「修复步骤」执行。
【Stage 特殊纪律】本 Stage 只改文档（md），禁止改生产代码与测试代码。ADR-0011 代码自证原则：只记 why/红线/登记，不复述职责文件表。md 产物反引号直接书写，禁止反斜杠转义。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'docs-root', prompt: `你负责 SC-DOC-01 / SC-DOC-02 / SC-DOC-03（根级文档）：

【SC-DOC-01】docs/settings-center-requirements.md 回写三处修订（checklist 顶部 R1-R3 修订表为真值源）：
- §4.1「无项目也可打开…」改「无项目 → toast『请先创建项目』」
- §4.2 项目组禁用条目删除（R3 不可达）
- §4.7 表同步；§5.2「AppState 原子量」改「plan_balance 模块级 static 原子量（读写双方同模块，SNAPSHOT 先例，State 注入不可直调单测）」
- §7 验收 2 改 toast 分支；决策记录补 R1–R3 三行

【SC-DOC-02】CONTEXT.md + .claude/CLAUDE.md：
- CONTEXT.md 新增术语（设置中心/配置页/全局组/项目组/前端消费型/后端消费型——按规格 §2）；:26-27 面板类型列举 hooksConfig→settings；:102 活动栏配置钮描述改「设置中心唯一入口（无项目 toast）」；:227-228 双模式面板条目指向设置中心 hooks 页
- .claude/CLAUDE.md：需求索引加 F11 行；模块索引 src/features/hooksConfig 行删除、+src/features/settingsCenter 行新增

【SC-DOC-03】.claude/adr.md 末尾追加 ADR-0012「设置中心（统一配置入口 + 配置页注册表 + 后端轻量收口）」：
上下文（配置钮直达单一面板无法承载两类配置）/ 决策（Dockview 面板左导航+SettingsPageRegistry；后端三段式；写入通道二分；切项目自动关闭+全局单例；无项目 toast）/ 被否决（模态/独立窗口/侧栏视图/完整后端注册表/inventory 自注册/Ctrl+, 键盘入口）/ 后果（新增配置页=注册一条；hooksConfig 类型退役；F10 豁免口径更新）` },
  { label: 'docs-modules', prompt: `你负责 SC-DOC-04 / SC-DOC-05（模块文档 + 用例清单）：

【SC-DOC-04】模块 CLAUDE.md 群（逐文件按 checklist SC-DOC-04 位置段执行）：
- src/panels/CLAUDE.md：hooksConfig 节 → settings 节重写（壳+注册表分派+dirty 汇聚+自动关闭+isAlwaysRenderPanel 排除决策）；「添加新面板类型的步骤」第 2 步失实修正——无 src/panels/index.ts，直接 panelRegistry 注册
- 新建 src/features/settingsCenter/CLAUDE.md（存在理由/家族契约 #13/触发点 side-effect import/openSettings 编排/无项目 toast）
- 删 src/features/hooksConfig/CLAUDE.md（schema 单点 MC-223/P3-FE-07/TE-09/TE-15 并入 cliProfiles/CLAUDE.md）
- src/features/cliProfiles/CLAUDE.md :52 KZ-1 重写（编辑器归域 configEditor/，不再跨 panels 引用）
- src/features/shortcuts/CLAUDE.md（可视化 UI 落地 + setCaptureSuspended/getEffectiveKeystroke 登记）
- src/features/sideViews/CLAUDE.md（配置钮→openSettings+无项目 toast）
- src/ipc/CLAUDE.md（planBalance 四命令）
- src-tauri/src/CLAUDE.md（白名单聚合：前端消费型四键集中 + 后端消费型归域先例）
- src-tauri/src/plan_balance/CLAUDE.md（动态间隔+plan_balance_set_interval 新命令）
- src/workspace/CLAUDE.md（openSettingsPanel + × 拦截登记）
- src/__tests__/CLAUDE.md（测试文件迁移清单登记）

【SC-DOC-05】.claude/test-inventory.md 校准：
- 实跑四级测试取数（L1/L2/L3 本 Stage 门禁产出；L4 计数沿用 Stage 06 的 npm run e2e 结果——本 Stage 不动代码）校准三处计数（表头/段头/段小计一致）
- F10 豁免行口径更新（「轮询任务本体」扩注动态间隔内存读取）
- 新增豁免行「settings.json corrupted 警示条 L4」（无沙箱外写坏文件通道，L2 覆盖）
- L4 段加 settings.e2e.ts 行；L2 段：删 open-hooks-config 两行、hooks-config-panel 改 settings-hooks-page、新增文件行按实际落地登记（settings-page-registry/open-settings/open-settings-panel/settings-panel/settings-plan-balance/settings-hooks-page/settings-panel-dirty/settings-dirty-registry/settings-keybindings/settings-panel-autoclose 以磁盘实存为准）` },
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
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
7. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败（测试命令须附计数行：通过/失败/总数）；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由；文档描述须对照当前代码核实不撒谎。
以下为测试 agent 的全量测试执行结果，测试类断言与计数校准据此判定（无需重跑）：
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
