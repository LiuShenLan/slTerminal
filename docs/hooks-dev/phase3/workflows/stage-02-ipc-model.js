// =====================================================================
// Stage 02 Workflow — IPC 封装、DTO、eventsCatalog、matcher 引擎、configModel
// =====================================================================
// 跨边界契约：
//   - 前端 wrapper: src/ipc/hooksConfig.ts
//     readHooksConfig(layer, projectPath?) -> 该层 hooks 子树（或 null）
//     writeHooksConfig(layer, hooks, projectPath?) -> invoke payload 键名为 hooks（非 content）
//   - 命令: hooks_config_read / hooks_config_write；layer: "user" | "project" | "local"
//   - eventsCatalog: 30 事件 x 10 组 x matcher 支持 x 匹配目标 x handler 三档（真值表在
//     docs/hooks-dev/phase3/stages.md 头部「事件元数据目录」，逐行实现，禁凭记忆）
// =====================================================================

export const meta = {
  name: 'stage-02-ipc-model',
  description: '前端 IPC 封装、DTO 类型、eventsCatalog、matcher 语义引擎、配置模型',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-ipc',
    prompt: `你负责 P3-FE-05/06（含 IPC 契约测试）。

【P3-FE-05】新建 src/ipc/hooksConfig.ts：
- 封装 readHooksConfig(layer, projectPath?) -> Promise<unknown>，调用 hooks_config_read（返回该层 hooks 子树或 null）。
- 封装 writeHooksConfig(layer, hooks, projectPath?) -> Promise<void>，调用 hooks_config_write，invoke payload 字段名为 hooks（非 content）。
- layer 类型为 "user" | "project" | "local"；参数使用 camelCase（Tauri 自动转 snake_case）。
- 本文件是唯一调用 invoke 的位置（硬约束 #1）。

【P3-FE-06】新建 src/types/hooksConfig.ts：
- 定义 HooksLayer = "user" | "project" | "local"。
- 定义原始 JSON 类型：HooksConfigJson（settings.json 的 hooks 子树：Record<事件名, MatcherGroupJson[]>）、MatcherGroupJson、HookHandlerJson。
- 定义 GUI 模型：HooksConfigGui、HookEventGroup、HookMatcherGroup、HookHandlerGui（5 种 handler 字段矩阵照 contract.md C13-3 官方版：command=command*/args/async/asyncRewake/shell、http=url*/headers/allowedEnvVars、mcp_tool=server*/tool*/input、prompt=prompt*/model/continueOnBlock、agent=prompt*/model；通用 if/timeout/statusMessage；once 不展示）。
- 定义 DisabledHookKey = { layer, event, matcher, command }。

【IPC 契约测试】新建 src/__tests__/ipc-hooks-config-contract.test.ts（照既有 src/__tests__/ipc-hooks-contract.test.ts 的 mockIPC 模式）：
- 验证 readHooksConfig 命令名 hooks_config_read + 参数结构 + 返回透传 + 异常传播。
- 验证 writeHooksConfig 命令名 hooks_config_write + payload 键集合精确为 { layer, hooks, projectPath? }。

【收尾】在 src/ipc/index.ts 追加 re-export。`
  },
  {
    label: 'frontend-model',
    prompt: `你负责 P3-FE-26/08/10 与 P3-TE-05/06/19。

【P3-FE-26】新建 src/panels/hooksConfig/eventsCatalog.ts（事件元数据单点）：
- 先读 docs/hooks-dev/phase3/stages.md 头部「事件元数据目录」全表（30 事件 x 10 组 x matcher 支持 x 匹配目标 x handler 支持档），逐行实现为常量数据，禁凭记忆/禁省略事件。
- 每事件元数据：所属分组、是否支持 matcher、matcher 匹配目标、支持的 handler 类型（A=全 5 种 / B=command+http+mcp_tool / C=command+mcp_tool；MessageDisplay 保守按 B 档并注释推断依据）。
- 5 种 handler 字段矩阵常量（照 contract.md C13-3：各类型字段清单 + 必填项 + 通用字段 if/timeout/statusMessage；once 不展示）。
- 纯数据 + 纯查询函数，零 DOM/React，供 EventTree/HandlerForm/JsonMode 导航/MatcherTester 共用。

【P3-FE-08】新建 src/panels/hooksConfig/matcherEngine.ts：
- 纯函数 matchHook(matcher, toolName, event?) -> { matched: boolean, mode: "exact-or" | "regex" | "all" }。
- 语义严格按 contract.md C13-5：
  - 窄字符集（字母/数字/_/-/空格/|/,）→ 精确匹配 OR（大小写敏感）。
  - 含其他字符 → JS 正则非锚定。
  - "*" / "" / 省略 → 全匹配。
  - FileChanged / StopFailure 窄字符集仅字母/数字/_/|（连字符/空格/逗号强制走正则）。
- 注释写明版本前提：逗号/空格分隔需 claude v2.1.191+、连字符需 v2.1.195+。

【P3-FE-10】新建 src/panels/hooksConfig/configModel.ts：
- 实现 jsonToGui(json) 与 guiToJson(gui) 双向转换（输入为 hooks 子树）。
- 实现 filterDisabled(config, disabledKeys) 剔除禁用条目。
- 实现 isSltermManaged(handler)：command 含 slterm-hook-reporter 子串判定（照 C9 识别规则）。
- 不支持 matcher 的事件（eventsCatalog 标记）：guiToJson 省略 matcher 键但保留数组包裹。
- jsonToGui 对非对象/非数组输入降级为空模型，不抛错。

【P3-TE-05/06/19】在 src/__tests__/ 新建测试文件：
- hooks-config-matcher.test.ts：matcher 语义全表（每分支至少 2 用例）。
- hooks-config-model.test.ts：configModel 双向转换（空配置/多事件多 handler/字段缺失容错/无 matcher 事件省略键/isSltermManaged/filterDisabled）。
- hooks-config-catalog.test.ts：eventsCatalog 常量守卫（30 事件齐全唯一、10 分组、handler 三档抽查+全量断言、10 个无 matcher 事件标记、5 种 handler 字段矩阵与 C13-3 一致）。`
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
3. npm test -- hooks-config-matcher hooks-config-model hooks-config-catalog ipc-hooks-config-contract
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
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
