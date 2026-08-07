// =====================================================================
// Stage 06 文档同步：CONTEXT + ADR + 三 CLAUDE.md + color 两文档（DOC-01~06）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   文档描述的目标架构 = Stage 01~05 完成后的真实代码状态：
//   src/theme/schemes/{types.ts,darcula.ts,index.ts} + schemeRegistry.ts
//   + colors.ts facade（31 导出代理 getActive()）+ overrides.ts 四导出；
//   ColorScheme 四段 { id, label, ui, terminal, editor, libraries }；
//   main.tsx 动态 import 链保证 facade 在 setActive 后求值。
// fix-loop 调用约定：args.constraints 传『本 Stage 只改文档，禁止改代码』。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage6-docs',
  description: 'Stage 06 文档同步：CONTEXT/ADR/CLAUDE.md/color 两文档（DOC-01~06）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
本 Stage 只改文档，禁动任何代码（src/、src-tauri/、e2e-tests/、test/ 一律不碰）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-glossary',
    prompt: `你负责 DOC-01/DOC-02（CONTEXT.md + .claude/adr.md）。先 Read docs/color-plan/checklist.md 中 DOC-01/02 条目，再 Read 两个现状文件与 docs/color-scheme-refactor-spec.md §3/§8.1。

【DOC-01】CONTEXT.md 追加 4 个术语，照 spec §3 术语表：配色 token / 配色方案（ColorScheme）/ 方案注册表（SchemeRegistry）/ 启动链 fail-safe 色。术语格式与既有条目风格一致。

【DOC-02】.claude/adr.md 追加 ADR-0002，内容照 spec §8.1 全文（含 D1-D8 决策依据），格式照 ADR-0001 既有样式（编号/标题/状态/背景/决策/后果）。

完成后报告：两文件改动摘要。`,
  },
  {
    label: 'A2-claudemd',
    prompt: `你负责 DOC-03/DOC-04/DOC-05（根 .claude/CLAUDE.md + src/theme/CLAUDE.md + src/panels/CLAUDE.md）。先 Read docs/color-plan/checklist.md 中 DOC-03/04/05 条目，再 Read 三个现状文件。

【DOC-03】根 .claude/CLAUDE.md 两处：① 硬约束 #6 改写——现措辞「配色单点：所有颜色只在 theme/colors.ts 定义为 token；组件引用 token，禁止硬编码颜色（既定例外见 @../src/panels/CLAUDE.md）」改为新语义，须含两层：颜色定义于 theme/schemes/<scheme>.ts + 组件经 theme/colors.ts facade token 引用禁止硬编码（既定例外句保留）；② 模块索引 src/theme 行职责更新为「配色方案单点（schemes/ + SchemeRegistry + facade，硬约束 #6）」语义。

【DOC-04】src/theme/CLAUDE.md 重写：职责（配色方案单点）/架构决策（四件——schemes 值文件、SchemeRegistry 注册表、colors.ts facade、overrides.ts 四导出 + 启动链时序）/终端 adapter 新表述（panels/terminal/theme.ts 映射 active 方案 terminal 段）/文件表追加 schemes/ 三文件 + schemeRegistry.ts + overrides.ts/新增方案步骤/测试模式节更新（删「无独立测试文件」旧表述与「既定例外」旧表述，登记 scheme-registry.test.ts 与 overrides.test.ts）。

【DOC-05】src/panels/CLAUDE.md 硬约束 #6 例外句仅改一处（surgical）：「终端配色是历史遗留的独立主题定义」→「终端配色经 panels/terminal/theme.ts adapter 映射 active 方案 terminal 段」。其余内容不动。

完成后报告：三文件改动摘要。`,
  },
  {
    label: 'A3-colordocs',
    prompt: `你负责 DOC-06（docs/color-implementation.md + docs/color-inventory.md）。先 Read docs/color-plan/checklist.md 中 DOC-06 条目、docs/color-scheme-refactor-spec.md §9.2（两勘误），再 Read 两个现状文档与新架构代码（src/theme/schemes/、schemeRegistry.ts、colors.ts、overrides.ts）。

两文档更新为反映新架构现状：
① 删「临时摸底」类注记（文档转为长期参考）；
② 落实 spec §9.2 两勘误回写——死配置清理结果：DROPDOWN_BG / APP_BG_SECONDARY / GIT_GUTTER_COLORS.whitespaceOnly / EXPLORER_COLORS.selected / --sl-bg-secondary / --sl-fg-secondary 已删除，ON_ACCENT_FG 已新增；
③ 架构描述对齐现状：颜色定义在 schemes/ 值文件、组件经 colors.ts facade 引用、终端/编辑器/dockview/allotment 四通道映射方式、新增方案步骤入口。
只改这两份文档，不动 docs/color-plan/ 下任何计划文件（计划文件是历史记录不回改）。完成后报告：两文档改动摘要。`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（本 Stage 纯文档，无编译门禁；diff 范围核对供 verify 断言判定）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
- git diff --name-only HEAD
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
