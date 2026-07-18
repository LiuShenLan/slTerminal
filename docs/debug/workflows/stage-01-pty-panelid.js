// =====================================================================
// Debug Stage 1 — 故障B：PTY write/resize/kill 补 panelId 契约同步
// 覆盖 checklist ID：DBG-1 / DBG-2 / DBG-3 / DBG-4
// 契约写死（两 agent 不各自推断）：
//   write(sessionId, panelId, data)         → invoke("pty_write",  { sessionId, panelId, data: Array.from(data) })
//   resize(sessionId, panelId, cols, rows)  → invoke("pty_resize", { sessionId, panelId, cols, rows })
//   kill(sessionId, panelId)                → invoke("pty_kill",   { sessionId, panelId })
// 参数顺序统一 (sessionId, panelId, ...)，与后端 spawn.rs 签名阅读顺序一致。
// fix-loop 调用本 Stage 时 args.constraints 传："不改任何 Rust 代码；不改 RegisteredTerminal 结构"
// =====================================================================

export const meta = {
  name: 'debug-stage1-pty-panelid',
  description: '故障B：PTY write/resize/kill 补 panelId 契约同步 + 契约守卫测试（DBG-1/2/3/4）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：先读 docs/debug/debug-checklist.md 对应 ID 条目 + docs/debug/refactor-regressions.md §3 根因分析，再动手。
契约写死：
- write(sessionId: string, panelId: string, data: Uint8Array) → invoke("pty_write", { sessionId, panelId, data: Array.from(data) })
- resize(sessionId: string, panelId: string, cols: number, rows: number) → invoke("pty_resize", { sessionId, panelId, cols, rows })
- kill(sessionId: string, panelId: string) → invoke("pty_kill", { sessionId, panelId })
本 Stage 不改任何 Rust 代码（后端签名已正确）；不改 RegisteredTerminal 结构。`

// === Phase 1: 并行修复（两 agent 文件零重叠；不跑测试，统一由全量测试 agent 单点跑）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'pty-src-fix',
    prompt: `你负责 DBG-1 / DBG-2（生产代码修复）：

【DBG-1】src/ipc/pty.ts（27-49 行附近）：write/resize/kill 三个 wrapper 按头部契约补 panelId 参数与 invoke payload。函数 JSDoc 同步更新。

【DBG-2】8 处调用点同步（panelId 均为作用域现成值，零新增依赖）：
1. src/panels/terminal/useXterm.ts:155 附近 writeToPty → pty.write(sid, panelId, data)（panelId 已是 useCallback dep）
2. src/panels/terminal/useXterm.ts:250 附近 E2E installTerminalWriteToPty 回调内 → pty.write(sid, panelId, ...)
3. src/panels/terminal/useXterm.ts:337 附近 term.onData 回调内 → pty.write(sid, panelId, ...)
4. src/panels/terminal/useXterm.ts:357 附近 cleanup → pty.kill(entry.sessionId, panelId)
5. src/panels/terminal/useXterm.ts:405 附近字号变化 effect → pty.resize(sid, panelId, dims.cols, dims.rows)
6. src/panels/terminal/usePtyResize.ts:89 附近（仅行变化分支）→ pty.resize(sid, panelId, dims.cols, dims.rows)（panelId 是 hook 入参）
7. src/panels/terminal/usePtyResize.ts:102 附近（列变化 debounce 内）→ pty.resize(currentSid, panelId, dims.cols, dims.rows)
8. src/App.tsx:83-86 附近关窗清理：allSessions.forEach((entry, panelId) => ...) 取 Map key 作 panelId → pty.kill(entry.sessionId, panelId)

完成后自行通读改动文件确认无遗漏调用点（grep pty.write(/pty.resize(/pty.kill( 全 src/ 与 e2e-tests/，除 mock/测试文件外不得有缺 panelId 的调用）。不跑任何测试。`,
  },
  {
    label: 'pty-contract-test',
    prompt: `你负责 DBG-3 / DBG-4（仅测试文件 src/__tests__/ipc-contract.test.ts，不碰生产代码）：

【DBG-3】同步 6 条既有用例到新签名（契约见脚本头部）：
- :72-113 三条参数断言用例：write/resize/kill 调用补 panelId 实参（如 pty.write('session-1', 'panel-1', testData)），期望 payload 显式含 panelId（如 { sessionId: 'session-1', panelId: 'panel-1', data: [...] }）
- :145-169 三条异常传播用例：调用签名同步补 panelId

【DBG-4】新增 describe 块「PTY 命令 payload 契约守卫」：对 pty_write/pty_resize/pty_kill 各断言一次调用 args 的键集合精确匹配——
- pty_write → 键恰好为 sessionId/panelId/data
- pty_resize → 键恰好为 sessionId/panelId/cols/rows
- pty_kill → 键恰好为 sessionId/panelId
（未来任何单边加/减键立即红；可用 Object.keys(args).sort() 比对。）

运行 npx vitest run src/__tests__/ipc-contract.test.ts 确认该文件全绿（其余测试由全量测试 agent 跑，你不管）。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 1 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/debug/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
