// =====================================================================
// Stage 01 方案骨架：新建 schemes/ + SchemeRegistry + overrides（SCH-01~05）
// =====================================================================
// 跨边界契约（写死，并行 agent 不各自推断）：
//   C3（Stage 01→02/03）ColorScheme 四段形状：
//     { id: string, label: string, ui: UiTokens, terminal: TerminalPalette,
//       editor: EditorScheme, libraries: LibraryOverrides }
//     UiTokens = 6 组（gitFile 7 键 / gitGutter 3 键 / explorer 5 键 /
//       sidebar 8 键 / agentStatusUsage 3 键 / errorBanner 3 键）+ 23 标量
//     TerminalPalette = 25 键（兼容 xterm ITheme）
//     EditorScheme = { theme: Extension; overrides: EditorOverrides }
//       （lint 7 键 / searchMatch 4 键 / background）
//     LibraryOverrides = { dockview: Record<string,string>（20 条 CSS 变量）;
//       allotment: Record<string,string>（2 键） }
//   C2（Stage 01→03）overrides.ts 四导出签名：
//     dockviewVarStyle(): Record<string, string>——键为 CSS 变量名原样
//       （如 --dv-group-view-background-color），供 React style 内联注入
//     allotmentVarStyle(): Record<string, string>——2 键（--sash-size 不动）
//     editorTheme: Extension——= schemeRegistry.getActive().editor.theme
//     editorColorOverrides(): Extension——active 方案 editor.overrides
//       → CM6 EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background）
//   跨 agent 编译依赖：A1 的 schemes/types.ts 被 A2 import；A1 的
//   schemes/index.ts 又 import A2 的 schemeRegistry.ts——接口与签名以本注释
//   为准，tsc 门禁在合并后由全量测试 phase 统一跑。
// fix-loop 调用约定：本 Stage 不传 args.constraints。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage1-schemes-skeleton',
  description: 'Stage 01 方案骨架：新建 schemes/ + SchemeRegistry + overrides（SCH-01~05）',
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
本 Stage 只新建 5 个文件（schemes/types.ts、schemes/darcula.ts、schemes/index.ts、schemeRegistry.ts、overrides.ts），不改任何既有文件（含 colors.ts——facade 化是 Stage 02 的事）。
D1 零视觉变化：色值一律搬运现状，禁止新造。
跨边界契约以脚本头部注释 C2/C3 为准，不各自推断。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-schemes',
    prompt: `你负责 SCH-01/SCH-02/SCH-03（新建 src/theme/schemes/ 三个文件）。先 Read docs/color-plan/checklist.md 中 SCH-01~03 条目与 docs/color-scheme-refactor-spec.md §4.5/§4.8，再 Read 现状值来源文件。

【SCH-01】新建 src/theme/schemes/types.ts——ColorScheme/UiTokens/TerminalPalette/EditorScheme/LibraryOverrides 接口定义，形状以脚本头注释 C3 为准。每个槽位带 JSDoc 消费注释（说明该色在前端何处起作用，决策 D8：区域级注释于接口，不在值文件逐色重复）。

【SCH-02】新建 src/theme/schemes/darcula.ts——darcula 方案全量值，D1 零视觉变化、值一律搬运现状禁止新造：
① ui 段 6 组（gitFile 7 键 / gitGutter 3 键 / explorer 5 键 / sidebar 8 键 / agentStatusUsage 3 键 / errorBanner 3 键）+ 23 标量——逐字搬运 src/theme/colors.ts 现状值（先 Read）；
② terminal 段 25 键——逐字搬运 src/panels/terminal/theme.ts 现状 theme 段（先 Read；drawBoldTextInBrightColors/vtExtensions/scrollback 等非色选项不搬）；
③ editor 段 = oneDark 直 import 透出（import { oneDark } from @codemirror/theme-one-dark）+ overrides（lint 7 键 / searchMatch 4 键 / background——值搬运现状编辑器配色补充，先 Read spec §4.8 与 src/panels/editor/useCodeMirror.ts 定位）；
④ libraries 段 = dockview 20 条 CSS 变量 + allotment 2 键——值照 spec §4.5。
文件头注释交叉引用启动链 fail-safe 三处：index.html:10、src-tauri/tauri.conf.json:20、src/main.tsx:31 的 #1e1e1e/#1e1e2e。

【SCH-03】新建 src/theme/schemes/index.ts——side-effect 注册文件：schemeRegistry.register(darcula)，照 src/panels/terminal/tabRules.ts 与 src/features/sideViews/sideViewDefs.ts 的 side-effect import 模式（import { schemeRegistry } from ../schemeRegistry + import { darcula } from ./darcula + 注册调用）。

禁止改动任何既有文件。完成后报告：新建文件清单 + 每文件段/键数统计。`,
  },
  {
    label: 'A2-registry',
    prompt: `你负责 SCH-04/SCH-05（新建两个文件）。先 Read docs/color-plan/checklist.md 中 SCH-04/05 条目与 docs/color-scheme-refactor-spec.md §4.6/§4.8。

【SCH-04】新建 src/theme/schemeRegistry.ts——SchemeRegistry 模块级单例（项目第 6 个注册表单例，先 Read src/panels/terminal/TabTitleRegistry.ts 与 src/features/sideViews/sideViewRegistry.ts 参考模式），七方法 API：
register(scheme)（同 id 覆盖）/ get(id) / getActive() / setActive(id)（未知 id → 回退 darcula + console.warn）/ getAll() / getDefaultId() / _reset()（仅测试用，重置后 active 回默认 darcula）。
导出 schemeRegistry 单例 + SchemeRegistry 类型；import type { ColorScheme } from ./schemes/types（接口形状以脚本头注释 C3 为准）。

【SCH-05】新建 src/theme/overrides.ts——四导出（签名以脚本头注释 C2 为准，禁各自发挥）：
① dockviewVarStyle()：active 方案 libraries.dockview 20 条 CSS 变量 → Record<string, string>（键为 CSS 变量名原样，供 React style 内联注入）；
② allotmentVarStyle()：active 方案 libraries.allotment 2 键 → Record<string, string>；
③ editorTheme：= schemeRegistry.getActive().editor.theme（CM6 Extension 透出）；
④ editorColorOverrides()：active 方案 editor.overrides → CM6 EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background）。

禁止改动任何既有文件。完成后报告：新建文件清单 + 七方法/四导出签名对照表。`,
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
