// Stage 7 — 前端中危修复（FE-03、FE-04、FE-05、FE-06、FE-07、FE-09、FE-10、FE-16、FE-18、FE-22、DOC-10）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 5 个并行 agent，文件不重叠

export const meta = {
  name: 'stage7-frontend-mid',
  description: 'Stage7: 前端生命周期与竞态修复',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7（前端 agent 不涉及，声明备查）。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。
测试约束：npm test 全绿；新增/修改测试遵循 src/__tests__/ 现有 vi.hoisted() mock 模式。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 src/workspace/Workspace.tsx 的 1 项改动：

【FE-03】render 阶段直接增删 ref（Workspace.tsx:168-182）
- 组件函数体执行期间遍历增删 pageCallbacksRef.current——StrictMode/并发下可能双注册或读到半一致状态
- 修复：回调 map 维护逻辑移入 useEffect（依赖 allPages/activePageIds），保持渲染期 ref 只读

改完跑：npx vitest run workspace 全绿。
`, { label: 'workspace-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/workspace/PageDockviewHost.tsx 的 2 项改动：

【FE-04】handleReady 监听器未显式释放（PageDockviewHost.tsx:262-304）
- onDidLayoutFromJSON/onDidLayoutChange/onDidRemovePanel 三个 disposable 收集到 ref 数组；组件卸载或 handleReady 重触发时统一 dispose

【FE-22】rebuildAndRecomputeTitles 访问非公共 API（PageDockviewHost.tsx:165）
- panel.view?.contentComponent 改从 panel.api.params 判断面板类型（注册时携带的类型信息）

改完跑：npx vitest run workspace 全绿。
`, { label: 'dockview-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/main.tsx、src/global.d.ts（新增）、src/lib/ErrorBoundary.tsx 的 2 项改动：

【FE-05】bootstrap setInterval 无上限无清理（main.tsx:8-19）
- 等 __TAURI_INTERNALS__ 的 setInterval 加最大轮次（200 次/10s），超时显示错误提示并停止；成功/失败路径均 clearInterval

【FE-18】window 扩展属性 as any（main.tsx:10/14、ErrorBoundary.tsx:42）
- 新增 src/global.d.ts 声明 interface Window { __TAURI_INTERNALS__、__sltermError、__dockviewApi、__slterm_e2e_* 等现有扩展 }，去掉 (window as any)
- global.d.ts 纳入 tsconfig 编译范围（核实 tsconfig include 已覆盖 src/）

改完跑：npx tsc --noEmit 通过。
`, { label: 'bootstrap-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/features/explorer/useFileTree.ts 与 ExplorerPanel.tsx 的 2 项改动：

【FE-06】refreshExpanded 的 gitStatus 无 generation 防护（useFileTree.ts:166-184）
- gitStatus(rp) 回调写 setGitStatusMap 前比对 genRef 快照，过期丢弃（与 loadRoot 同模式）

【FE-07】addPanel 未 try-catch（ExplorerPanel.tsx:100-110）
- dockApi.addPanel 包 try-catch；仅成功后才调 titleManager.registerEditor（保持两状态一致）

改完跑：npx vitest run explorer 全绿。
`, { label: 'explorer-agent' }),

  () => agent(`
${PREAMBLE}

你负责 4 个文件的 4 项改动：

【FE-09】TerminalPanel.tsx:72 — state.icon! 非空断言改 state.icon ?? null

【FE-10】layoutSerde.ts:97-98 — fromJSON(layout as any) 改 Parameters<DockviewApi["fromJSON"]>[0] 类型（不引入 any）

【FE-16】ipc/notify.ts:6-10 与 types/notify.ts:2-6 — FsEvent/FsEventPayload 的 detail 可选性统一为 detail?: string；消费处（ExplorerPanel 等）做空值处理

【DOC-10】layoutSerde.ts:70-74 — 删除"失败回退创建默认终端"过时注释，改述现行 Watermark 策略（对齐 workspace/CLAUDE.md）

改完跑：npx tsc --noEmit && npx eslint src/ && npx vitest run layout-serde title-manager 全绿。
`, { label: 'misc-agent' }),
])

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 7 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
