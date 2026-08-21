// =====================================================================
// Stage 09：稳定性与死代码（FE-35、FE-46、FE-47、FE-48）
// 编排：并行 2（文件零重叠：A=panelRegistry/ErrorBoundary/knip；B=App/pageApis/restoreSession）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-09.md
// 人工验证点：多 session 场景关窗总时长有界实测
// =====================================================================

export const meta = {
  name: 'stage09-stability',
  description: 'S09 死代码清除 + ErrorBoundary 重试 + 关窗总超时 + waitFor abort 清理（FE-35/46/47/48）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 npm test（全量测试 agent 单点跑）；编译级自查用 \`npx tsc --noEmit\`。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A-deadcode-retry',
    prompt: `你负责【FE-35】terminalTabConfig 死代码删除 +【FE-46】ErrorBoundary inline 重试按钮（D20 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 9 节 FE-35/FE-46 条目（各含可照抄代码块）。

触碰文件：\`src/panelRegistry.ts\`、\`src/__tests__/panel-registry.test.ts\`、\`knip.json\`（联动清理）、\`src/lib/ErrorBoundary.tsx\`、\`src/__tests__/error-boundary.test.tsx\`

【FE-35】步骤：
1. 删 \`src/panelRegistry.ts:72-75\` 的 \`terminalTabConfig\` 常量（含 :72 注释行）
2. \`src/__tests__/panel-registry.test.ts\`：删 :6 import 中的 \`terminalTabConfig,\`、删 :63-75 整个 describe 块（29 用例 → 26）
3. knip 联动：检查 \`knip.json\`——若 S01 为 terminalTabConfig 加了 ignoreExports 条目，一并删除
4. grep 全仓 \`terminalTabConfig\` 确认零命中

【FE-46】步骤：
5. \`src/lib/ErrorBoundary.tsx\` inline variant（约 :51-115）：「查看错误详情」\`details\`（约 :86）之前插入 checklist 条目中的重试按钮代码块（onClick 清 error state；token 复用文件已有 import——SECONDARY_BG/SEPARATOR_BG/PLACEHOLDER_FG，禁硬编码色值，约束 #6）
6. \`src/__tests__/error-boundary.test.tsx\` 增用例：抛错面板渲染占位 → 点击「重试」→ 断言子树重新渲染（用「首次抛错二次正常」桩验证恢复路径）

7. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + knip.json 联动结果 + 新增用例名。`,
  },
  {
    label: 'B-timeout-abort',
    prompt: `你负责【FE-47】关窗 ptyKillAll 包总超时 +【FE-48】waitFor 轮询 setTimeout abort 清理（2 处）。先读 \`docs/review-phase2-fix/checklist.md\` 第 9 节 FE-47/FE-48 条目（各含可照抄代码块）。

触碰文件：\`src/App.tsx\`、\`src/__tests__/close-handler.test.ts\`、\`src/workspace/pageApis.ts\`、\`src/features/agentHistory/restoreSession.ts\`、\`src/__tests__/pageapis.test.ts\`、\`src/__tests__/agent-history-restore.test.ts\`

【FE-47】步骤：
1. \`src/App.tsx:157\` \`const killed = await pty.ptyKillAll();\` 改为 checklist 条目中的 Promise.race 代码块（SHUTDOWN_TIMEOUT_MS 既有常量，:140-143 有同形 race 先例）；其后 \`if (killed > 0)\` 分支条件照条目调整（null 守卫），其余逻辑不动
2. \`src/__tests__/close-handler.test.ts\` 适配/增用例：ptyKillAll 永不 resolve 时关窗流程在超时后继续

【FE-48】步骤（两处同形）：
3. \`src/workspace/pageApis.ts:98\` 与 \`src/features/agentHistory/restoreSession.ts:48\` 的轮询 setTimeout 按 checklist 条目改为 abort 感知版（clearTimeout + addEventListener("abort", { once: true })）；restoreSession 用 POLL_INTERVAL_MS 常量
4. 循环顶部已有 signal?.aborted 检查——abort 后 resolve 落入下轮迭代顶部退出，行为正确，无需再加分支
5. \`src/__tests__/pageapis.test.ts\` / \`src/__tests__/agent-history-restore.test.ts\` 既有 abort 用例适配：增断言「abort 后轮询 Promise 立即 settle」（fake timers 下 advance 0 即完成）

6. \`npx tsc --noEmit\` 通过

完成后报告：三处改动摘要 + 测试适配摘要。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx knip --production
5. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 9 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
