// =====================================================================
// Stage 01 — 配色方案替换（TH-01~TH-11）
// 契约：linear 四段全值 = docs/ui-redesign-impl/checklist.md 附录 A（唯一真值源，只准照抄）
// fix-loop 调用本 Stage 时 args.constraints 传：
//   「全值只准照抄 docs/ui-redesign-impl/checklist.md 附录 A，禁止自估色值」
// 跨 agent 契约（写死，不各自推断）：
//   - 新方案 id = "linear"，label = "Linear"
//   - ui 段新增标量 3 键：accentFg / selectionHoverBg / titlebarBg
//   - editor.overrides 新增：syntax 9 键 + plainText/lineNumber/lineNumberActive
//   - facade 新增导出：ACCENT_FG / SELECTION_HOVER_BG / TITLEBAR_BG（31→34）
//   - 错误页结构：外层 color #ece9e4 + 错误消息 span color #d9706b（failsafe-main 与 test-sync 对齐）
// =====================================================================

export const meta = {
  name: 'stage01-theme-scheme',
  description: 'Stage 01: linear 配色方案替换 darcula + syntax 语法色槽位 + fail-safe 同步',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。
【本 Stage 特殊纪律】linear 全值只准照抄 docs/ui-redesign-impl/checklist.md 附录 A，禁止自估色值。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试，统一由全量测试 agent 单点跑）===
phase('并行重构')
const parallelAgents = [
  { label: "scheme-core", prompt: `你负责 TH-01/TH-02/TH-03/TH-04/TH-09：
【TH-01】src/theme/schemes/types.ts：EditorScheme overrides 新增 syntax 子组 9 键（property/string/number/keyword/function/type/operator/punctuation/comment，均 string）+ plainText/lineNumber/lineNumberActive 3 键；UiTokens 标量新增 accentFg/selectionHoverBg/titlebarBg 3 键（string）；新键带区域级消费注释（照既有 JSDoc 风格，行号不入注释）；ColorScheme.id 注释「回退 darcula」改「回退 linear」。
【TH-02】新建 src/theme/schemes/linear.ts：四段全值照抄 docs/ui-redesign-impl/checklist.md 附录 A（先 Read 该文件）；对象标注 : ColorScheme；id 为 linear、label 为 Linear；文件头 fail-safe 交叉引用注释照 darcula.ts:11-15 格式（值改 #0a0a0b/#ece9e4/#d9706b）；editor.theme 引 oneDark（import 方式同 darcula.ts:19）。
【TH-03】src/theme/schemes/index.ts 注册改 linear（import + register）；删除 src/theme/schemes/darcula.ts。
【TH-04】src/theme/schemeRegistry.ts:14 DEFAULT_SCHEME_ID 由 darcula 改 linear；文件头与 getActive/setActive/getDefaultId/_reset 注释中 darcula 字样同步改 linear。
【TH-09】src/theme/index.ts barrel schemes 导出 darcula→linear；src/theme/colors.ts facade 新增 ACCENT_FG/SELECTION_HOVER_BG/TITLEBAR_BG 3 导出（31→34，照既有导出模式取 ui 段对应键）。` },
  { label: "scheme-inject", prompt: `你负责 TH-07/TH-08：
【TH-07】src/theme/overrides.ts：新增导出 editorSyntaxHighlight(): Extension——syntaxHighlighting(HighlightStyle.define([...]))（@codemirror/language 导入），tags 映射：tags.propertyName←syntax.property、tags.string←syntax.string、tags.number←syntax.number、tags.keyword←syntax.keyword、tags.function(tags.variableName)←syntax.function、tags.typeName←syntax.type、tags.operator←syntax.operator、tags.punctuation←syntax.punctuation、tags.comment←syntax.comment（色值取 schemeRegistry.getActive().editor.overrides.syntax）；editorColorOverrides() 增规则——&.cm-editor .cm-content 的 color←plainText、&.cm-editor .cm-gutters 的 backgroundColor←background + color←lineNumber + borderRight 发丝线（1px solid ui.separatorBg 色值）、&.cm-editor .cm-lineNumbers .cm-gutterElement 的 color←lineNumber、活跃行号 gutterElement 的 color←lineNumberActive；全部规则带 &.cm-editor 前缀（ACC-05：mountStyles reverse 层叠，平级选择器恒输 oneDark——文件内既有注释有完整说明，先 Read 理解再动手）。
【TH-08】5 处消费点在扩展数组中注入 editorSyntaxHighlight() 且位置在 editorTheme 之前：src/panels/editor/useCodeMirror.ts:289、src/panels/gitshow/GitShowPanel.tsx:142、src/panels/diff/DiffPanel.tsx:520 与 :566、src/panels/hooksConfig/JsonMode.tsx:162（先 Read 各点确认现状注入形态再改；import 路径照各文件既有 theme 引用方式）。` },
  { label: "failsafe-main", prompt: `你负责 TH-05/TH-06：
【TH-05】src/main.tsx:40 附近：默认 schemeId 由 darcula 改 linear（含 36-40 行注释中 darcula 字样同步）。
【TH-06】fail-safe 三处静态色同步（UI-111）：
1. index.html:10 body background #1e1f22→#0a0a0b；
2. src-tauri/tauri.conf.json:20 backgroundColor→#0a0a0b（本 Stage 仅改此一键，decorations 不动）；
3. src/main.tsx:28 超时错误页：外层 background #1e1f22→#0a0a0b、color #e35f6c→#ece9e4，错误消息文本外加一层 span 包裹 style color #d9706b（UI-111：文字 #ece9e4、强调 #d9706b）。先 Read main.tsx:26-31 确认 inline style 拼接结构再改，保持字符串拼接形态。` },
  { label: "test-sync", prompt: `你负责 TH-10/TH-11（只改测试文件）：
先 Read docs/ui-redesign-impl/checklist.md 附录 A 全值表（断言新值的唯一依据）。
【TH-10】L2 测试 darcula 色值断言全量改附录 A 值：
- src/__tests__/colors.test.ts：gitFile 7 键（56-62 行附近）、gitGutter 3 键（80-82）、explorer 组（99-101）、sidebar 组（120-124）、标量表（145-169）、agentStatusUsage（218-221）、ROOT_CSS_VARS 断言（244-245：--sl-fg-primary 改 #b3aea6，--sl-bg-primary 同步核为 #0a0a0b）；facade 键集合断言新增 ACCENT_FG/SELECTION_HOVER_BG/TITLEBAR_BG 三键（值 #8fb4f5 / rgba(110,159,242,0.22) / #141416）；
- src/__tests__/scheme-registry.test.ts：import darcula→linear（来自 schemes/linear）、setActive 未知 id 回退断言改回退 linear、getDefaultId 断言改 linear、四段完整性断言（editor 段新增 syntax 9 键+plainText 3 键——键数断言按新结构改）、色值抽点（159-185 行附近）改附录 A 值；
- src/__tests__/overrides.test.ts：darcula 引用改 linear；新增 editorSyntaxHighlight 存在性与返回 Extension 断言；ACC-05 层叠守卫断言形态保持；
- src/__tests__/bootstrap.test.ts：darcula 字样改 linear；
- src/__tests__/main-bootstrap.test.tsx:35：#e35f6c→#d9706b（错误页强调色 span）；
- src/__tests__/explorer-git-status.test.tsx：gitFile 7 值两组（324-348 与 422-428 行附近）改附录 A；
- src/__tests__/git-gutter.test.ts:115-117：3 值改附录 A；
- src/__tests__/commit-context-menu-ui.test.tsx:246：hover 断言 #2a4371→rgba(110,159,242,0.13)（jsdom 转 rgb 形态——先 Read 周边断言确认既有写法再改）；
- src/__tests__/explorer-selection.test.tsx:12,84,168：#2a4371→rgba(110,159,242,0.13)（注释与断言同步）；
- src/__tests__/sideBarArea.test.tsx:461：PANEL_BG 注释值同步 #0a0a0b。
【TH-11】test/terminal/theme-options.test.ts:47-52：ANSI 6 色断言改附录 A terminal 段值（red #d9706b/green #93b573/yellow #d6b25e/blue #7fa8e8/magenta #b48ce0/cyan #6fbfc4）。` },
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
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run test:l3
7. npx vite build
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/ui-redesign-impl/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
