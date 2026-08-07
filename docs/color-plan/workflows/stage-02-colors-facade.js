// =====================================================================
// Stage 02 facade 切换：colors.ts facade 化 + theme/index.ts barrel + colors.test.ts 同步（FAC-01/02 + TST-01）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   C1（Stage 02→03/04/05）facade 31 导出名精确清单：
//     组 5 个：GIT_FILE_COLORS（7 键）/ GIT_GUTTER_COLORS（3 键）/
//       EXPLORER_COLORS（5 键）/ SIDEBAR_COLORS（8 键）/
//       AGENT_STATUS_USAGE_COLORS（3 键）
//     ERROR_BANNER 标量 3 个（名称沿用 src/theme/colors.ts 现状）
//     其他标量 22 个：PANEL_BG / SIDEBAR_BG / SECONDARY_BG / APP_BG /
//       APP_BG_PRIMARY / EDITOR_BG / SIDEBAR_FG / ERROR_FG / PLACEHOLDER_FG /
//       BUTTON_FG / DIM_FG / INPUT_BG / INPUT_BORDER / FOCUS_BORDER /
//       ACTIVE_SELECTION_BG / EXPLORER_SELECTION_BG / SEPARATOR_BG /
//       CONTEXT_MENU_BORDER / SHADOW_MENU / HTML_PANEL_LOADING_FG /
//       HTML_PANEL_IFRAME_BG / ON_ACCENT_FG（值 "#FFFFFF"）
//     ROOT_CSS_VARS：键集合恰 { "--sl-bg-primary", "--sl-fg-primary" }
//   删除后不得存在：DROPDOWN_BG / APP_BG_SECONDARY /
//     GIT_GUTTER_COLORS.whitespaceOnly / EXPLORER_COLORS.selected /
//     --sl-bg-secondary
//   保留不动：SIDEBAR_COLORS.selected（src/theme/colors.ts:81）
// fix-loop 调用约定：本 Stage 不传 args.constraints。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage2-colors-facade',
  description: 'Stage 02 facade 切换：colors.ts facade 化 + barrel 更新 + colors.test.ts 同步（FAC-01/02 + TST-01）',
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
本 Stage 单 agent：三文件强耦合（colors.ts 导出清单与 colors.test.ts 断言必须同视野）。
本 Stage 不动 main.tsx——facade 在 import 时按默认 darcula 求值，与现状视觉一致（D1）；ROOT_CSS_VARS 注入时机风险由 Stage 04 解决。
跨边界契约以脚本头部注释 C1 为准，不各自推断。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-facade',
    prompt: `你负责 FAC-01/FAC-02/TST-01（src/theme/colors.ts 全文重写为 facade + src/theme/index.ts barrel 更新 + src/__tests__/colors.test.ts 同步）。先 Read docs/color-plan/checklist.md 中 FAC-01/02 与 TST-01 条目、docs/color-scheme-refactor-spec.md §4.7，再 Read 三个现状文件与 Stage 01 产物（src/theme/schemes/types.ts、src/theme/schemes/darcula.ts、src/theme/schemeRegistry.ts、src/theme/overrides.ts）。

【FAC-01】src/theme/colors.ts 全文重写为 facade——导出清单以脚本头注释 C1 为准（恰 31 个导出）：
① 5 组 + 3 个 ERROR_BANNER 标量 + 22 个其他标量 + ROOT_CSS_VARS，每个导出值代理 schemeRegistry.getActive() 对应槽位；
② 删除项（现值标注供定位，删除后全仓不得再出现）：DROPDOWN_BG（现 colors.ts:48 "#2A2D2E"）、APP_BG_SECONDARY（现 :51 "#2b2b3c"）、GIT_GUTTER_COLORS.whitespaceOnly（现 :26 "#4C4638"）、EXPLORER_COLORS.selected（现 :35 "#37373D"）；SIDEBAR_COLORS.selected（现 :81）保留不动；
③ 新增 ON_ACCENT_FG，值 "#FFFFFF"；
④ ROOT_CSS_VARS 键集合改为恰 { "--sl-bg-primary", "--sl-fg-primary" }——删 --sl-bg-secondary，增 --sl-fg-primary（值 "#cdd6f4"）；
⑤ 文件头注释更新为 facade 语义（颜色定义在 schemes/，本文件代理 active 方案）。

【FAC-02】src/theme/index.ts barrel 更新：现状 32 个 re-export → 31 个（随 colors.ts 删 2 增 1），并追加 re-export：./schemeRegistry（schemeRegistry 单例 + SchemeRegistry 类型）、./schemes（ColorScheme 等类型 + darcula）、./overrides 四导出（dockviewVarStyle/allotmentVarStyle/editorTheme/editorColorOverrides）。

【TST-01】src/__tests__/colors.test.ts 六处同步（与 FAC-01 同视野逐条对齐）：
① import 块删 DROPDOWN_BG/APP_BG_SECONDARY、增 ON_ACCENT_FG；
② GIT_GUTTER_COLORS 键数断言 4 → 3；
③ EXPLORER_COLORS 键数断言 6 → 5；
④ 标量计数断言 25 → 24；
⑤ ROOT_CSS_VARS 断言：删 --sl-bg-secondary 两条断言、增 --sl-fg-primary 存在性与值 "#cdd6f4" 断言（键数 len 2 不变）；
⑥ 文件头注释（现 :7）更新为 facade 语义。
本 Stage 不动其他测试文件——theme.test.ts 等不应失效（值未变，仅来源换）。

完成后报告：三文件改动摘要 + facade 导出计数自检（须恰 31）。`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
- npx tsc --noEmit
- npx eslint src/
- npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
