// =====================================================================
// Debug Stage 3 — 文档同步（DBG-11）
// 纪律：文档 agent 必须先读 git log/diff（Stage 1/2 实际改动）再动笔，禁凭记忆写文档
// fix-loop 调用本 Stage 时 args.constraints 传："只改文档，禁止改生产代码与测试代码"
// =====================================================================

export const meta = {
  name: 'debug-stage3-docs-sync',
  description: 'debug 修复文档同步：PTY panelId 契约 + sandbox 加载时序（DBG-11）',
  phases: [
    { title: '文档同步' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只改文档，禁止改生产代码与测试代码；不顺手改无关内容；中文写作；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）。
背景：先读 docs/debug/debug-checklist.md 的 DBG-11 条目 + docs/debug/refactor-regressions.md 全文，再动手。
铁律：先读 git log -3 --stat 与 Stage 1/2 的实际 diff（git show <commit>），以最终代码状态为准——禁凭记忆或凭本 preamble 摘要写文档。`

// === Phase 1: 文档同步（单 agent）===
phase('文档同步')
const refactorResults = await parallel([
  () => agent(`${PREAMBLE}

你负责 DBG-11（文档同步）。逐文件同步 Stage 1/2 的最终代码状态：

1. src/ipc/CLAUDE.md — pty.ts 封装的 write/resize/kill 签名含 panelId（模块映射表或编码约定处补一句）；「参数序列化」节补 panelId 说明
2. src/panels/CLAUDE.md — 检查 pty.write/pty.resize/pty.kill 提及处（IPC 边界节等），同步为新签名
3. src/workspace/CLAUDE.md — 「页面切换流」代码块更新：switchToPage 先 await setProjectRoot 再 setActivePage；「架构决策」节追加一条：setProjectRoot 前置为切换前置条件（SEC-01 effect 保留兜底），注明 React 子 effect 先于父 effect 的坑
4. e2e-tests/CLAUDE.md — 全局对象表更新：__slterm_e2e_createProject / __slterm_e2e_switchToPage 为 async（返回 Promise），且内部先 await setProjectRoot
5. src-tauri/src/fs/CLAUDE.md — 记录 sandbox 对前端时序的要求：fs/git/notify/pty 命令均经 validate_path_within_root，消费方须保证 project_root 已设置（前端由 switchToPage/启动恢复/E2E helper 前置 await 保证）
6. .claude/test-inventory.md — 新增用例计数：DBG-4 契约守卫（pty payload 键集合 3 条）、DBG-9 switchToPage 时序（2 条）、DBG-10 explorer-sandbox-race（1 条）——先读实际测试文件确认真实用例数再写

注意：各 CLAUDE.md 遵循渐进式披露——只同步与本次改动直接相关的段落，不重写全文。`, { label: 'docs-sync' }),
])

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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 3 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/debug/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
