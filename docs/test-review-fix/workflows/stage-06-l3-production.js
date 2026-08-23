// Stage 06：L3 复用生产实现（TQ-E-01, TQ-E-02）——串行 pipeline：生产抽取 → L3 改写
// fix-loop 调用时 args.constraints 传：「行为不变重构，禁止改 OSC/按键语义」
export const meta = {
  name: 'stage-06-l3-production',
  description: 'Stage 06：L3 复用生产实现——OSC/按键注册层抽纯函数（2 项，串行）',
  phases: [
    { title: '生产抽取' },
    { title: 'L3 改写' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。
【Stage 特殊纪律】抽取为行为不变重构——handler 体逐字搬移，仅依赖改参数注入；禁止顺手改 OSC/按键语义。
【跨边界契约（写死，不各自推断）】
新文件 src/panels/terminal/oscHandlers.ts 导出：MAX_OSC52_PAYLOAD / registerOsc52(term, deps): IDisposable / registerOsc133(term, deps): IDisposable / makeLinkHandler(openUrl)——签名全文以 checklist TQ-E-01 步骤 1 代码块为准。
新文件 src/panels/terminal/keyEventHandler.ts 导出：handleTerminalKeyEvent(event): boolean——以 checklist TQ-E-02 步骤 1 为准。`

phase('生产抽取')
const extractResult = await agent(`${PREAMBLE}

你负责 TQ-E-01 生产侧 + TQ-E-02 生产侧：
1. 新建 src/panels/terminal/oscHandlers.ts——从 useClipboardHandler.ts（OSC52 + 链接处理）与 useCommandDetection.ts（OSC133）抽出纯注册函数，依赖全参数注入（不 import ipc/store），导出签名严格按 checklist TQ-E-01 步骤 1 代码块。
2. useClipboardHandler.ts / useCommandDetection.ts 改为调用 oscHandlers 的注册函数（handler 体逐字搬移，行为不变）。
3. 新建 src/panels/terminal/keyEventHandler.ts——useXterm.ts:283-291 内联按键逻辑抽为 handleTerminalKeyEvent(event): boolean（checklist TQ-E-02 步骤 1），useXterm.ts 改调用。
触碰文件：src/panels/terminal/oscHandlers.ts（新建）, keyEventHandler.ts（新建）, useClipboardHandler.ts, useCommandDetection.ts, useXterm.ts。
完成后报告：各导出函数最终签名原文（供下游 L3 agent 对照）。`, { label: 'extract-osc-layer' })

phase('L3 改写')
const rewriteResult = await agent(`${PREAMBLE}

你负责 TQ-E-01 L3 侧 + TQ-E-02 L3 侧。上游生产抽取 agent 的报告如下（导出签名以其为准，不符时 Read 源码核实）：
---
${extractResult ?? '（上游 agent 未返回——直接 Read src/panels/terminal/oscHandlers.ts 与 keyEventHandler.ts 取签名）'}
---
1. test/terminal/production-osc.test.ts：删除私有复制的 OSC handler 实现，改 import 生产 oscHandlers.ts 的注册函数挂到 xterm/headless 实例（依赖 mock 注入，参照该文件既有 vi.mock 形态）；断言主体不变；头注释的「复刻生产」降级标注更新为「复用生产实现」。
2. 新建 test/terminal/shortcut-dispatch.test.ts（≥3 用例）：挂生产 keyEventHandler.ts 的 handleTerminalKeyEvent，验证 Shift+Tab / Ctrl 组合键分发行为（checklist TQ-E-02 步骤与用例名建议）。
触碰文件：test/terminal/production-osc.test.ts, test/terminal/shortcut-dispatch.test.ts（新建）。
注意：L3（vitest.l3.config.ts）无 setupFiles——import 生产模块需自行 vi.mock 其依赖。`, { label: 'rewrite-l3' })

phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
「行为不变」断言：git diff 对照 useClipboardHandler.ts / useCommandDetection.ts / useXterm.ts——handler 体应逐字搬移仅依赖注入方式变化，发现语义改动判 not_fixed。
oscHandlers.ts / keyEventHandler.ts 不得 import src/ipc 或 src/stores（grep 断言）。
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

return { extractResult, rewriteResult, testResult, verifyResult }
