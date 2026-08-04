// =====================================================================
// Stage 08 L2-workspace：真实组件与启动顺序
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-08.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更，如 WRK-04 决策：删除/标注预留/补契约测试三选一）；其余只改测试
// =====================================================================

export const meta = {
  name: 'stage08-l2-workspace',
  description: 'L2 workspace 真实 DefaultTab + 启动顺序断言 + pageApis 覆盖',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1——本 Stage 不涉，仅作提示
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。并行 agent 文件零重叠（wk-host 碰 PageDockviewHost/DefaultTab/multi-instance/switch-order/pageApis 测试；wk-shell 碰 startup-restore/app/ipc-window/layout-serde/close-handler/main/titleManager/panel-registry 测试 + src/ipc/window.ts）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'wk-host',
    prompt: `你负责 WRK-01、WRK-02、WRK-05、WRK-06、WRK-09，触碰文件：PageDockviewHost/DefaultTab/multi-instance/switch-order/pageApis 相关测试文件（src/__tests__/workspace-defaulttab.test.tsx、workspace-multi-instance.test.tsx、workspace-switch-order.test.tsx 及 pageApis 相关测试）。逐 ID 对照 checklist 原文实施：

【WRK-01】PageDockviewHost.tsx 44.8%/12.19% 真实组件零覆盖。位置 src/workspace/PageDockviewHost.tsx（DefaultTab/Watermark/RightHeader/handleReady/onSaveAs）。补真实 DefaultTab 渲染（tabIcon emoji/img 分支——事件结构是 event.tabIcon 非 event.params.tabIcon）、Watermark 按钮 addPanel、RightHeader、handleReady 空布局不兜底创建终端、onSaveAs 重算标题用例。

【WRK-02】pageApis.ts 42.2% 页面切换核心无 L2。位置 src/workspace/pageApis.ts（switchToPageShared/switchToPageAndFocus）。setProjectRoot 先于 setActivePage 的 DBG-5/9 契约、轮询聚焦与超时降级无回归。直接调用 switchToPageShared 断言 await 顺序（spy invocationCallOrder）+ __dockviewApi 重指（D7 时序断言）；switchToPageAndFocus 补轮询命中/超时降级（100ms×50 上限）。

【WRK-05】workspace-defaulttab 手写 MockDefaultTab 漂移风险。位置 src/__tests__/workspace-defaulttab.test.tsx。测的是手写 Mock 而非生产 DefaultTab，event.params.tabIcon vs event.tabIcon 漂移无法发现。改用生产 DefaultTab 渲染断言（params 变化 → 图标切换）。

【WRK-06】workspace-switch-order 时序契约是手动模拟。位置 src/__tests__/workspace-switch-order.test.tsx。手动模拟 setProjectRoot/setActivePage 顺序而非真实驱动；另有 3000ms 超时。真实驱动 Workspace.switchToPage/switchToPageShared 断言顺序；超时收敛。

【WRK-09】workspace-multi-instance 仅 CSS display 断言 H6。位置 src/__tests__/workspace-multi-instance.test.tsx。只断言 display none/block，未验证 Dockview 实例存活（H6 核心语义）。补实例 identity 断言（同一 api 对象跨切换）+ 终端不 dispose。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'wk-shell',
    prompt: `你负责 WRK-03、WRK-04、WRK-07、WRK-08、WRK-10、WRK-11，触碰文件：startup-restore/app/ipc-window/layout-serde/close-handler/main/titleManager/panel-registry 相关测试文件 + src/ipc/window.ts（WRK-04 决策：删除或标注预留）。逐 ID 对照 checklist 原文实施：

【WRK-03】App.tsx 启动恢复顺序未断言。位置 src/App.tsx:76-84、183-186（requestUserAttention catch）。startup-restore 验证了状态流转但未锁定 setProjectRoot 先于 setActivePage（DBG-6）；通知 catch 分支未测。spy 断言两调用顺序（D7 时序断言）；补 requestUserAttention reject 静默 catch。

【WRK-04】ipc/window.ts onFocusChanged/setFocus 未覆盖。位置 src/ipc/window.ts:13-44。focus 监听与设置函数零调用零测试。先查消费方（grep onFocusChanged/setFocus 的调用点）：无消费方则删除或标注"预留"；保留则补最小契约测试（命令名/参数/异常传播四维）。处置三选一必须在本次落实，ipc/window.ts 与测试保持一致。

【WRK-07】layout-serde mock isValidPanelType 仅 3 种 vs 真实 6 种。位置 src/__tests__/layout-serde.test.ts。mock 白名单与真实 PANEL_TYPES 漂移，新面板类型过滤未验证。改用真实 PANEL_TYPES（6 种：terminal/editor/htmlviewer/gitshow/diff/hooksConfig）或断言 mock 与真实一致；gitshow/diff/hooksConfig 白名单过滤有用例。

【WRK-08】close-handler 未验证阻止默认关闭。位置 src/__tests__/（close-handler 相关）。补关窗拦截（preventDefault/二次确认）行为断言。

【WRK-10】main.tsx bootstrap catch 未覆盖。位置 src/main.tsx。补 init 失败 catch 分支断言（错误展示/不白屏）。

【WRK-11】残留行 + makeEmptyLayout 使用验证 + FILE_PANEL_TYPES 重复断言。位置 src/workspace/titleManager.ts、layoutSerde.ts、src/__tests__/panel-registry.test.ts、workspace-file-panel-types.test.ts。①titleManager/layoutSerde 覆盖残留行补测或标注；②default-layout-format 补"SidebarTree 实际使用 makeEmptyLayout"断言；③FILE_PANEL_TYPES 两处重复断言合并为单点（panel-registry 与 workspace-file-panel-types 不重复断言同一集合）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-08.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage08 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
