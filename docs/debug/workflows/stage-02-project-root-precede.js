// =====================================================================
// Debug Stage 2 — 故障A：setProjectRoot 前置为页面切换前置条件（方案A1）
// 覆盖 checklist ID：DBG-5 / DBG-6 / DBG-7 / DBG-8 / DBG-9 / DBG-10
// 时序契约写死（两 agent 不各自推断）：
//   凡"使 activePageId 生效"的动作（用户切换 / 启动恢复 / E2E helper），
//   必须先 await setProjectRoot(rootPath)（失败 console.error 降级继续），再 setActivePage(...)。
//   Workspace.tsx:197-212 的 SEC-01 effect 保留不动（幂等兜底）。
// fix-loop 调用本 Stage 时 args.constraints 传："不改 Workspace.tsx:197-212 SEC-01 effect；不改 Rust 代码"
// =====================================================================

export const meta = {
  name: 'debug-stage2-project-root-precede',
  description: '故障A：setProjectRoot 前置为页面切换前置条件（方案A1）+ 可观测性与回归测试（DBG-5~10）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：先读 docs/debug/debug-checklist.md 对应 ID 条目 + docs/debug/refactor-regressions.md §2 根因分析（React 子 effect 先于父 effect 的确定性时序），再动手。
时序契约：凡"使 activePageId 生效"的动作，必须先 await setProjectRoot(rootPath)（失败 console.error 降级、仍继续切换/恢复），再 setActivePage(...)。
不动：Workspace.tsx:197-212 SEC-01 effect（幂等兜底保留）；不改任何 Rust 代码。`

// === Phase 1: 并行修复（两 agent 文件零重叠；不跑测试，统一由全量测试 agent 单点跑）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'timing-src-fix',
    prompt: `你负责 DBG-5 / DBG-6 / DBG-7 / DBG-8（生产代码 + E2E helper 修复）：

【DBG-5】src/workspace/Workspace.tsx:84-94 switchToPage 改 async：
- 从 useProjects.getState() 遍历查 pageId 所属项目的 rootPath（找不到则跳过 setProjectRoot）
- await setProjectRoot(rootPath)，失败 catch → console.error("[slTerminal] 设置项目根路径失败:", err) 后继续（降级不中止切换）
- 然后 ensurePageInitialized(pageId) + layoutStore.setActivePage(pageId)（保持原有 early-return 与 __dockviewApi 逻辑不变）
- 不动 :197-212 的 SEC-01 effect（保留兜底）

【DBG-6】src/App.tsx:60-63 启动恢复 localStorage lastPage 处：setActivePage(lastPage) 前先查 lastPage 所属项目 rootPath 并 await setProjectRoot（失败同样 console.error 降级继续）。复用与 DBG-5 相同的查找方式。

【DBG-7】src/features/explorer/useFileTree.ts 两处静默吞错加日志：
- :54-56 loadDirectory catch → catch (err) { console.error("[slTerminal] readDir 失败:", dirPath, err); return []; }
- :229-232 effect 内 gitStatus catch → 保留 gen 检查逻辑，加 console.error("[slTerminal] gitStatus 失败:", rootPath, err)

【DBG-8】e2e-tests/helpers.ts 两函数 async 化 + 时序对齐：
- __slterm_e2e_createProject（:127-158）：改为 async 函数；把现有 setProjectRoot(dirPath) 调用从 setActivePage(pageId) 之后移到之前并 await（失败 console.error 后继续）；返回 Promise<string>。Window 接口声明同步改
- __slterm_e2e_switchToPage（:187-189）：改 async；遍历 useProjects.getState() 查 pageId 所属项目 rootPath，先 await setProjectRoot（失败 console.error 降级），再 setActivePage(pageId)；Window 接口声明同步改
- 注意：该文件不在根 tsconfig include 内，无 tsc 覆盖——改完自行通读两遍确认语法
- E2E 调用侧（browser.execute 返回 promise 天然 await）无需改 e2e-tests/test.e2e.ts

完成后 grep 全 src/ 与 e2e-tests/ 确认 setActivePage 的全部生产/E2E 调用点均已前置 setProjectRoot（Workspace SEC-01 effect 与 projects store 内部的除外）。不跑任何测试。`,
  },
  {
    label: 'timing-test',
    prompt: `你负责 DBG-9 / DBG-10（仅 src/__tests__/ 下测试，不碰生产代码）：

【DBG-9】switchToPage 时序断言（新增 src/__tests__/workspace-switch-order.test.tsx，或并入最合适的现有 workspace 测试文件）：
- mock ../../src/ipc/fs（按现有测试 mock 路径惯例调整）的 setProjectRoot 为手动控制 promise（deferred）
- 种子 useProjects（一项目两页面，rootPath 已知）+ useLayout，渲染 Workspace（Dockview/Allotment 按现有 workspace 测试惯例全 mock）
- 触发 switchToPage（经 props 或直取）→ 断言：setProjectRoot promise 未 resolve 时 useLayout.getState().activePageId 未变；resolve 后 activePageId 变为目标页
- 另加一条降级用例：setProjectRoot reject 时仍完成切换（activePageId 变更）且 console.error 被调用
- 排查现有 workspace/sidebar 测试是否因 switchToPage async 化受影响（sidebar-actions.test.ts 用 mock switchToPage，不受影响；其余如受影响则适配）

【DBG-10】新增 src/__tests__/explorer-sandbox-race.test.tsx：
- mock ../ipc/fs：setProjectRoot 为手动控制 promise；readDir spy 返回 [mockEntry...]
- 种子项目/页面，渲染 ExplorerPanel（或 renderHook(useFileTree) 结合 Workspace 层——选能真实复现"切换→setProjectRoot→readDir"链路的最小方案；若单测 useFileTree 无法覆盖时序，则以 Workspace+ExplorerPanel 集成渲染为准）
- 断言：setProjectRoot promise resolve 前 readDir 调用次数为 0；resolve 后 waitFor 文件树渲染出节点
- 沿用 src/__tests__/helpers/vfs.ts 与 workspace-setup.ts 工厂（先读再写）

运行 npx vitest run workspace-switch-order explorer-sandbox-race 确认新用例全绿（其余测试由全量测试 agent 跑）。`,
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
5. npx vite build（验证 e2e-tests/helpers.ts 打包图——该文件不在 tsc include 内）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 2 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/debug/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
