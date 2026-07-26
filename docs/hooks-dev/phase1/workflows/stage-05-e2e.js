// =====================================================================
// Stage 05 Workflow: L4 E2E 关键路径
// =====================================================================
// 契约头部：
//   - E2E helper：__slterm_e2e_injectHooks / __slterm_e2e_uninstallHooks / __slterm_e2e_getHookInjectionStatus
//   - L4 用例 1：注入命令状态回显
//   - L4 用例 2：Node 写信号文件 -> 页签 emoji 出现/消失
//   - 必须用 npm run build:e2e（VITE_E2E=1）构建
// =====================================================================

export const meta = {
  name: 'stage05-e2e',
  description: 'L4 E2E 页签图标 hook-event 流转关键路径',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改 e2e-tests/ 目录，不改生产代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/hooks-dev/phase1/checklist.md 对应 ID 条目（先读再动手）。本 Stage 特殊纪律：只改 e2e-tests/，禁止改生产代码。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e-helpers',
    prompt: `你负责 P1-TE-04：
修改 e2e-tests/helpers.ts 中的 installAllE2eHelpers：
- window.__slterm_e2e_injectHooks = async () => hooks.inject();
- window.__slterm_e2e_uninstallHooks = async () => hooks.uninstall();
- window.__slterm_e2e_getHookInjectionStatus = async () => hooks.getInjectionStatus();
需要在 helpers.ts 顶部 import { hooks } from "../src/ipc"（注意路径在 e2e-tests/ 下指向 ../src/ipc）。这些 helper 只在 E2E_ENABLED 时安装。完成后跑 npm run build:e2e 验证编译。`
  },
  {
    label: 'e2e-test',
    prompt: `你负责 P1-TE-03：
修改 e2e-tests/test.e2e.ts，新增 describe("hooks 状态可视化")，包含 2 条用例：
1. "注入后状态为 injected"：browser.execute(() => __slterm_e2e_injectHooks())，然后 browser.execute(() => __slterm_e2e_getHookInjectionStatus())，断言返回 status === "injected"。
2. "信号文件驱动页签图标流转"：
   - 使用 __slterm_e2e_createProject(path) 创建测试项目。
   - 使用 __dockviewApi.addPanel 创建一个 terminal 面板，记录 panelId。
   - 在 Node 端用 fs.writeFileSync 写信号文件到 require("os").homedir() + "/.slterminal/hooks-events/" + \`${panelId}-UserPromptSubmit-${Date.now()}.json\`，payload 为 { panelId, event: "UserPromptSubmit", timestamp: Date.now(), sessionId: "e2e", transcriptPath: "", cwd: path, toolName: null, notificationType: null }。
   - browser.execute 查询该面板对应 tab 的 DOM 文本包含 "⚡"。
   - Node 端再写 SessionEnd 信号文件。
   - browser.execute 查询该面板对应 tab 的 DOM 不再包含 "⚡"。
注意：panelId 格式为 terminal-{pageId}-{seq}，由 addPanel 时传入。完成后运行 npm run build:e2e && npm run wdio。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令必须顺序执行（E2E 需先构建）：
1. npm run build:e2e
2. npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase1/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
