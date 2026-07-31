// =====================================================================
// Stage 04 Workflow — Schema 内嵌与 JSON 模式
// =====================================================================
// 跨边界契约：
//   - Schema 文件: src/features/hooksConfig/schema/claude-code-settings.json（自包含，本地 $ref）
//   - 使用 hooks 子 schema（properties.hooks）供编辑器与保存校验（对齐 hooks 子树编辑范围）
//   - 依赖: codemirror-json-schema + @codemirror/lint + @codemirror/autocomplete
//     禁止 ajv；保存校验用 codemirror-json-schema 底层 json-schema-library
//   - 事件导航: 30 事件按 eventsCatalog 十组（非旧九组）
// =====================================================================

export const meta = {
  name: 'stage-04-json-mode',
  description: '内嵌 SchemaStore schema（hooks 子 schema）+ CM6 JSON 模式 + 事件导航 + matcher 测试工具',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。
本 Stage 特殊纪律：禁止引入 ajv——保存前 schema 校验用 codemirror-json-schema 底层 json-schema-library（compileSchema(schema).validate(data)）；schema 文件须核实自包含性（无远程 $ref，含则预打包展开）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-json-mode',
    prompt: `你负责 P3-FE-07/11 与 P3-TE-09/10。

【P3-FE-07】内嵌 SchemaStore 官方 schema + hooks 子 schema：
- 新建目录 src/features/hooksConfig/schema/。
- 将 SchemaStore https://json.schemastore.org/claude-code-settings.json 内容复制到 src/features/hooksConfig/schema/claude-code-settings.json（离线可用、版本随 slTerminal 发布更新）。
- 执行期核实 schema 自包含性：codemirror-json-schema 仅支持本地 $ref；若含远程 $ref 需预打包展开。核实结论（有无远程 $ref）写入 schema 目录代码注释。
- 提取 properties.hooks 子 schema（导出供 JSON 模式编辑器与 Stage 06 保存校验使用——对齐 hooks 子树编辑范围）。
- 确保 Vite 可通过 JSON import 加载。
- 新增 npm 依赖：codemirror-json-schema + @codemirror/lint + @codemirror/autocomplete（peer deps，当前 package.json 缺失）+ json-schema-library（codemirror-json-schema 底层，Stage 06 保存校验直接 import——一并加入 dependencies 显式声明，不依赖 node_modules 平铺）。修改 package.json 与 package-lock.json。禁止引入 ajv。

【P3-FE-11】实现 src/panels/hooksConfig/JsonMode.tsx：
- 使用 CodeMirror 6 + @codemirror/lang-json（已有依赖）。
- 集成 codemirror-json-schema：jsonCompletion 补全 + jsonSchemaHover 悬停 + jsonSchemaLinter 波浪线，schema 使用 hooks 子 schema。
- props 接受 value、onChange、onValidationChange(isValid, diagnostics)；非法 JSON 时通过 onValidationChange 通知父组件。
- 事件导航侧栏：30 事件按 eventsCatalog（Stage 02 已建，src/panels/hooksConfig/eventsCatalog.ts）十组渲染，点击后通过简单文本搜索定位到对应事件键并 setSelection。
- 集成 MatcherTester.tsx 作为内联/浮动工具。

【MatcherTester】实现 src/panels/hooksConfig/MatcherTester.tsx：
- 输入 matcher + toolName（+event 感知窄字符集），调用 matcherEngine.matchHook（Stage 02 已建），显示命中结果与匹配模式（exact-or / regex / all）。

【收尾】在 HooksConfigPanel.tsx 中接入 JsonMode 作为 JSON 模式内容（替换 Stage 03 占位文案）。

【P3-TE-09/10】新建 src/__tests__/hooks-config-jsonmode.test.tsx（两 ID 同文件分 describe）：
- TE-09：渲染 CM6 EditorView、schema 扩展注册、非法 JSON 触发 onValidationChange(isValid=false)。
- TE-10：十大分组事件名渲染、点击后编辑器选区跳到对应事件键位置。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test -- hooks-config-jsonmode
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
