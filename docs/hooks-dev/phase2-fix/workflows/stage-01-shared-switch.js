// =====================================================================
// Stage 01：共享页面切换基础设施 + 路由修复（高风险）
//   改动项：FIX-FE-01 / FIX-FE-02 / FIX-FE-10
// =====================================================================
// 结构（偏离模板：先串行 1 agent 后并行 2 agent）：
//   Phase 1 基础设施（串行 infra）→ Phase 2 消费方（并行 notify-consumer / view-consumer）
//   → Phase 3 全量测试 → Phase 4 逐项验证
//
// 跨边界契约（写死，agent 不各自推断，原文见 stages.md C1）：
//   C1 `src/workspace/pageApis.ts`（新建）：
//     registerPageApi(pageId: string, api: DockviewApi): void
//     unregisterPageApi(pageId: string): void
//     getPageApi(pageId: string): DockviewApi | undefined
//     switchToPageShared(pageId: string): Promise<void>
//       ——activePageId 相同直接返回；查 rootPath 先 await setProjectRoot（失败 console.error 降级）；
//         setActivePage；getPageApi 命中 → window.__dockviewApi = api
//     switchToPageAndFocus(pageId: string, panelId: string): Promise<void>
//       ——await switchToPageShared → 有限轮询面板挂载（100ms×50=5s 上限）→ focus()
//   不变量：window.__dockviewApi 重指向只允许出现在三站点——
//     Workspace.switchToPage（经 switchToPageShared）、Workspace.onDeletePage、
//     Workspace.handlePageApiReady（含 pageApis.ts 内 switchToPageShared 一处）。
//
// Agent 分工（文件全集 = prompt 触碰文件，无重叠）：
//   infra           ：src/workspace/pageApis.ts（新）、src/workspace/Workspace.tsx、e2e-tests/helpers.ts
//   notify-consumer ：src/features/notifications/useClaudeNotifications.ts、src/__tests__/notifications.test.ts
//   view-consumer   ：src/features/agentStatus/AgentStatusView.tsx、src/__tests__/agent-status-view.test.tsx
//
// 本 Stage 特殊纪律（PREAMBLE_EXTRA）：
//   1. parsePageId 维持现状（本地副本）——收敛 src/lib 属 Stage 02，本 Stage 禁止新建 src/lib/panelId.ts；
//   2. helpers.ts 在根 tsconfig include 外——全量测试必须含 npx vite build 构建级兜底；
//   3. 并行 agent 不跑测试，全量测试由独立 agent 统一执行。
// fix-loop 调用本 Stage 时 args.constraints 传上述 3 条原文。
// =====================================================================

export const meta = {
  name: 'stage1-shared-switch',
  description: 'Stage 01：pageApis 基础设施 + toast/行点击路由复用共享 switchToPage',
  phases: [
    { title: '基础设施' },
    { title: '消费方改造' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】parsePageId 维持现状（本地副本）——收敛 src/lib 属 Stage 02，本 Stage 禁止新建 src/lib/panelId.ts；不跑测试，全量测试由独立 agent 统一执行。`

// === Phase 1: 基础设施（串行——消费方依赖 pageApis 模块）===
phase('基础设施')
const infraResult = await agent(`${PREAMBLE}

你负责 Stage 01 基础设施（FIX-FE-01 前半）：pageApis 新建 + Workspace 改造 + E2E helper 清扫。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-01 条目与 docs/hooks-dev/phase2-fix/stages.md 的 C1 契约，再动手。

契约 C1（写死，不各自推断）：
- 新建 src/workspace/pageApis.ts：模块级 Map<pageId, DockviewApi>，导出五个函数——
  registerPageApi(pageId, api) / unregisterPageApi(pageId) / getPageApi(pageId) /
  switchToPageShared(pageId): Promise<void> / switchToPageAndFocus(pageId, panelId): Promise<void>
- switchToPageShared：useLayout.getState().activePageId === pageId 时直接返回；
  经 useProjects.getState() 查 pageId 所属项目 rootPath，await setProjectRoot（失败 console.error 降级继续）；
  useLayout.getState().setActivePage(pageId)；getPageApi(pageId) 命中 → window.__dockviewApi = api
  （未初始化页面由 Workspace.handlePageApiReady 兜底重指向）。
- switchToPageAndFocus：await switchToPageShared(pageId) → 有限轮询
  getPageApi(pageId)?.getPanel(panelId)（100ms×50=5s 上限）→ 命中 focus()。
- 不变量：window.__dockviewApi 重指向只允许出现在三站点——Workspace.switchToPage（经 switchToPageShared）、
  Workspace.onDeletePage、Workspace.handlePageApiReady（含 pageApis.ts 内 switchToPageShared 一处）。

步骤：
1. src/workspace/pageApis.ts（新建）：按 C1 实现。
2. src/workspace/Workspace.tsx 三站点改造：
   - switchToPage（约 :95-120）：保留 ensurePageInitialized(pageId) 调用后委托 switchToPageShared(pageId)；
     组件内 pageApiMapRef 删除（迁入 pageApis 模块级 Map）。
   - onDeletePage（约 :123-159）：经 unregisterPageApi + getPageApi 完成注销与次页 __dockviewApi 重指向。
   - handlePageApiReady（约 :162-167）：经 registerPageApi 注册，activePageId 匹配时重指向 __dockviewApi。
3. e2e-tests/helpers.ts（:211-227）：__slterm_e2e_switchToPage 删除自行复制的 setProjectRoot→setActivePage
   时序，改委托 switchToPageShared（其余 helper 不动）。注意 helpers.ts 在根 tsconfig include 外，
   语法/导入正确性由 npx vite build 兜底（全量测试统一跑，你不用跑）。
`, { label: 'infra' })

// 前序失败短路，不跑下游
if (!infraResult) {
  return { infraResult, consumerResults: null, testResult: null, verifyResult: { allFixed: false, failedItems: ['infra-no-return'], details: { 'infra-no-return': { status: 'not_fixed', evidence: 'infra agent 未返回（被跳过或 API 错误）' } } } }
}

// === Phase 2: 消费方改造（并行——文件零重叠，均只 import pageApis 不碰其实现）===
phase('消费方改造')
const consumerAgents = [
  {
    label: 'notify-consumer',
    prompt: `你负责 FIX-FE-01 后半（routeToPanel + findPanelTitle）与 FIX-FE-10（去重注释）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-01 与 FIX-FE-10 条目，再动手。pageApis 模块已由 infra agent 建好（src/workspace/pageApis.ts，五导出），你只 import 使用，不碰其实现与 Workspace.tsx。

步骤（src/features/notifications/useClaudeNotifications.ts）：
1. routeToPanel（约 :116-143）：删除自行复制的 setProjectRoot + setActivePage + 立即 focus 逻辑，
   改为 parsePageId → await switchToPageAndFocus(pageId, panelId)。
   ——parsePageId 维持现有本地副本（Stage 02 才收敛 src/lib，本 Stage 不动）。
2. findPanelTitle（约 :101-111）：改经 getPageApi(parsePageId(panelId) ?? "")?.getPanel(panelId)?.title ?? panelId
   ——跨页面可查，不再依赖 __dockviewApi 恰好指向目标页；不得再读 window.__dockviewApi。
3. FIX-FE-10：去重注释（约 :187）「60s 内同一 session + event + timestamp 不重复通知」改为真实机制——
   同一信号文件重复投递去重（sessionId+event+timestamp 键）+ 缓存超 200 条截断保留最近 100 条；
   注释必须与 :188-196 实现一致（Read 核对后再写）。

步骤（src/__tests__/notifications.test.ts）：
4. 新增 routeToPanel 守卫用例：mock ../../workspace/pageApis，捕获 toast onClick 触发后断言
   switchToPageAndFocus 被调用（参数 panelId→正确 pageId），且 routeToPanel 不直接调用 useLayout.setActivePage。`,
  },
  {
    label: 'view-consumer',
    prompt: `你负责 FIX-FE-02（F5 行点击走共享函数）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-02 条目，再动手。pageApis 模块已由 infra agent 建好（src/workspace/pageApis.ts，五导出），你只 import 使用，不碰其实现与 Workspace.tsx。

步骤（src/features/agentStatus/AgentStatusView.tsx）：
1. handleFocus（约 :62-89）改 async：保留现有内联 pageId 解析（:65-72，Stage 02 才收敛），
   删除项目查找循环与 props.switchToPage 调用、删除"同步 switchToPage + 立即 getPanel().focus()"，
   改 await switchToPageAndFocus(pageId, panelId)。SideViewComponentProps 接口不动。

步骤（src/__tests__/agent-status-view.test.tsx）：
2. 点击用例（约 :229）改写：mock ../../workspace/pageApis，断言点击行后 switchToPageAndFocus
   被以 (pageId, panelId) 调用。`,
  },
]
const consumerResults = await parallel(
  consumerAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. npx vite build
说明：第 5 条是 e2e-tests/helpers.ts 的构建级兜底——该文件在根 tsconfig include 外，无 tsc 门禁。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { infraResult, consumerResults, testResult, verifyResult }
