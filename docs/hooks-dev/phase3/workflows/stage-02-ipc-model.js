// =====================================================================
// Stage 02 Workflow — IPC 封装、DTO、matcher 引擎、configModel
// =====================================================================
// 跨边界契约：
//   - 前端 wrapper: src/ipc/hooksConfig.ts
//   - 命令: hooks_config_read / hooks_config_write
//   - layer: "user" | "project" | "local"
// =====================================================================

export const meta = {
  name: 'stage-02-ipc-model',
  description: '前端 IPC 封装、DTO 类型、matcher 语义引擎、配置模型',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-ipc',
    prompt: `你负责 P3-FE-05/06。

【P3-FE-05】新建 src/ipc/hooksConfig.ts：
- 封装 readHooksConfig(layer, projectPath?) -> Promise<unknown>，调用 hooks_config_read。
- 封装 writeHooksConfig(layer, content, projectPath?) -> Promise<void>，调用 hooks_config_write。
- layer 类型为 "user" | "project" | "local"；参数使用 camelCase（Tauri 自动转 snake_case）。
- 本文件是唯一调用 invoke 的位置（硬约束 #1）。

【P3-FE-06】新建 src/types/hooksConfig.ts：
- 定义 HooksLayer = "user" | "project" | "local"。
- 定义 GUI 模型：HooksConfigGui、HookEventGroup、HookMatcherGroup、HookHandlerGui（5 种 handler 字段矩阵照 F6 表）。
- 定义 DisabledHookKey = { layer, event, matcher, command }。

【收尾】在 src/ipc/index.ts 追加 export * from "./hooksConfig";（或具名导出）。`
  },
  {
    label: 'frontend-model',
    prompt: `你负责 P3-FE-08/10 与 P3-TE-05/06。

【P3-FE-08】新建 src/panels/hooksConfig/matcherEngine.ts：
- 纯函数 matchHook(matcher, toolName, event?) -> { matched, mode }。
- 语义严格按 F6 表：
  - 仅含字母/数字/_/- /空格/\|/, -> 精确匹配 OR（大小写敏感）。
  - 含其他字符 -> JS 正则非锚定。
  - "*" / "" / 省略 -> 全匹配。
  - FileChanged / StopFailure 窄字符集仅字母/数字/_ /|，其他字符强制正则。

【P3-FE-10】新建 src/panels/hooksConfig/configModel.ts：
- 定义 HooksConfigJson 与 HooksConfigGui。
- 实现 jsonToGui(json) 与 guiToJson(gui) 双向转换。
- 实现 filterDisabled(config, disabledKeys) 剔除禁用条目。
- 对非对象/非数组输入降级为空模型，不抛错。

【P3-TE-05/06】在 src/__tests__/ 新建测试文件：
- hooks-config-matcher.test.ts：matcher 语义全表。
- hooks-config-model.test.ts：configModel 双向转换。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test -- hooks-config-matcher hooks-config-model
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
