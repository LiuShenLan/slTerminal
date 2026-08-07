// =====================================================================
// Stage 03 消费点迁移：oneDark 四处 + dockview/allotment CSS 变量 + 终端 adapter（FAC-03 + CON-01~06）
// =====================================================================
// 跨边界契约（写死，并行 agent 不各自推断）：
//   C2（Stage 01 产物，本 Stage 消费）overrides.ts 四导出签名：
//     dockviewVarStyle(): Record<string, string>——键为 CSS 变量名原样
//       （如 --dv-group-view-background-color），供 React style 内联注入
//     allotmentVarStyle(): Record<string, string>——2 键（--sash-size 不动）
//     editorTheme: Extension——= schemeRegistry.getActive().editor.theme
//     editorColorOverrides(): Extension——active 方案 editor.overrides
//       → CM6 EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background）
//   C3（adapter 形状）panels/terminal/theme.ts 的 theme 段 =
//     { ...schemeRegistry.getActive().terminal }（25 键展开，非色选项原位保留）
//   消费方式统一：token/四导出经主题 barrel（src/theme/index.ts）import，
//   import 相对层级按消费文件现状。
// fix-loop 调用约定：本 Stage 不传 args.constraints。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage3-consumers',
  description: 'Stage 03 消费点迁移：oneDark 四处/dockview/allotment/终端 adapter/JsonMode 违规收敛（FAC-03 + CON-01~06）',
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
预期零改动声明：theme.test.ts / gitshow-panel.test.tsx / hooks-config-jsonmode.test.tsx / L3 theme-options.test.ts 不应失效（值未变，仅来源换）——任一失效即停手查值漂移，禁止改这些测试来迎合。SideBarArea.tsx 不改。
跨边界契约以脚本头部注释 C2/C3 为准，不各自推断。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-cm',
    prompt: `你负责 CON-01/02/03/04（CodeMirror 系四文件）。先 Read docs/color-plan/checklist.md 中 CON-01~04 条目与 docs/color-scheme-refactor-spec.md §4.8，再 Read 四个现状文件与 src/theme/overrides.ts（四导出签名以脚本头注释 C2 为准）。

【CON-01】src/panels/editor/useCodeMirror.ts——现 :289 oneDark 引用改为经 barrel 引入 editorTheme 与 editorColorOverrides()（两个扩展按现状接入位置原样替换），删除现 :20 的 oneDark import。

【CON-02】src/panels/gitshow/GitShowPanel.tsx——现 :143 oneDark 引用同上替换，删除现 :12 的 oneDark import。

【CON-03】src/panels/diff/DiffPanel.tsx——现 :521 与 :566 两处 oneDark 引用同上替换（左右栏各自接入），删除现 :25 的 oneDark import。

【CON-04】src/panels/hooksConfig/JsonMode.tsx——现 :160 oneDark 引用同上替换，删除现 :25 的 oneDark import；另：现 :213 事件导航 hover 色 e.currentTarget.style.color = "#FFFFFF" 改为引用 ON_ACCENT_FG token（自主题 barrel import，import 语句并入该文件现有 theme import 或新增一行，按文件现状风格）。

四文件均经主题 barrel（src/theme/index.ts）import，import 相对层级按各文件现状。禁止改任何测试文件。完成后报告：四文件改动摘要 + 每文件 oneDark 残留 grep 自检（应为零）。`,
  },
  {
    label: 'A2-shell',
    prompt: `你负责 CON-05/CON-06/FAC-03（dockview/allotment 外壳两文件 + 终端 adapter）。先 Read docs/color-plan/checklist.md 中 CON-05/06 与 FAC-03 条目、docs/color-scheme-refactor-spec.md §4.5，再 Read 三个现状文件与 src/theme/overrides.ts（四导出签名以脚本头注释 C2 为准）。

【CON-05】src/workspace/PageDockviewHost.tsx——现 :369 根 div 的 style 展开 dockviewVarStyle()（与既有 style 合并），className dockview-theme-dark 保留不动。

【CON-06】src/workspace/Workspace.tsx——根容器 style 合并 allotmentVarStyle()；CSS 变量继承天然覆盖现 :224 外层与 SideBarArea.tsx:72 内层两处 Allotment——SideBarArea.tsx 不改。

【FAC-03】src/panels/terminal/theme.ts——theme 段改为 { ...schemeRegistry.getActive().terminal }（25 键自 active 方案展开，adapter 形状以脚本头注释 C3 为准）；非色选项 drawBoldTextInBrightColors / vtExtensions.kittyKeyboard / scrollback 原位保留；文件头注释更新为 adapter 语义（终端配色映射 active 方案 terminal 段）。schemeRegistry 经主题 barrel import。

禁止改任何测试文件。完成后报告：三文件改动摘要 + 每文件改动点清单。`,
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
- npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
