// =====================================================================
// Stage 07 面板状态解耦
// 并行 4 路：A1 串行链（CP-017→036→042，共享 tabClose/SettingsPanel/pageApis 族）
// ∥ CP-016 侧栏状态槽 ∥ CP-037 CM 保活 ∥ CP-019 PTY spawn ResizeObserver
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S07 行
//（本脚本 PREAMBLE_EXTRA 已内含串行链纪律，与表值同源）
// =====================================================================

export const meta = {
  name: 'stage-07-panel-state',
  description: 'S07 面板状态解耦（CP-016/017/036/042/037/019）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
【Stage 特殊纪律】CP-017→CP-036→CP-042 串行链顺序不可换（共享 tabClose.ts/SettingsPanel.tsx/pageApis.ts）。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）；大代码块按条目内「照抄 review-NN 步骤 X.Y」指针读 docs/compromises-fix/ 下对应 review 文件。
并行纪律：本阶段不跑共享资源测试，只做编译级检查（npx tsc --noEmit），真实测试由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（4 路文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'cp017-036-042-chain',
    prompt: `你负责串行链 CP-017 → CP-036 → CP-042（单 agent 顺序执行，顺序不可换）：
【CP-017】照抄 checklist 步骤 1-6：panelRegistry.ts isAlwaysRenderPanel 白名单加 settings（含 doc 注释 CP-017 口径）；SettingsPanel.tsx 删壳生命周期 effect（:331-338 整段删，替换为说明注释）；tabClose.ts closeTabGuarded 确认分支补 clearSettingsDirty（照抄代码块）；SettingsPanel SC-FE-08 确认分支同补；dirtyRegistry.ts 头注释改 CP-017 口径。pageApis.ts 的 renderer: "always" 行【不在本步改】，由 CP-042 代码块内嵌落地。
【CP-036】照抄 checklist 步骤 1-3：tabClose.ts 追加 closeTabsGuarded 批量入口（照抄代码块；import 与 CP-017 同一行）；PageDockviewHost.tsx 关闭其他/关闭全部两 action 改写走 closeTabsGuarded + 删遗留注释；收尾复核点：复核 Workspace 删页路径是否经守卫，结论（守卫或显式不守卫登记）写入报告与 workspace/CLAUDE.md。
【CP-042】照抄 checklist 步骤 1-2：pageApis.ts registerPageApi 派发 PAGE_API_READY_EVENT = "slterm:page-api-ready"；openSettingsPanel 整体重写（照抄代码块：waitPageApi 事件驱动 + 5s 超时 toast + addPanel renderer "always"）；原 doc 注释替换。
测试按三条目「测试同步」逐一点名（settings-panel-dirty/tab-close/open-settings-panel/workspace-file-panel-types/workspace-page-dockview 等）。
文档按三条目执行（workspace/CLAUDE.md、settingsCenter/CLAUDE.md、panels/CLAUDE.md）。
自查：npx tsc --noEmit 绿；条目验证段 grep 断言满足。
完成后报告：修改文件清单 + 页删除路径复核结论。`,
  },
  {
    label: 'cp016-sideview-state',
    prompt: `你负责 CP-016（侧栏视图状态上移注册表状态槽）：
照抄 checklist CP-016 步骤 1-5：sideViewRegistry.ts SideViewComponentProps 加 viewState/onViewStateChange 两可选槽位（照抄代码块）+ SideViewRegistry 增 viewStates Map 与 getViewState/setViewState（_reset 同清）；SideBarArea.tsx 两处受控消费 + 头注释换 CP-016 口径（照抄 review-04 步骤 3.3）；useFileTree.ts 展开态真值源外移（FileTreeViewState/commitViewState/hasPath/restoreExpanded——完整代码块照抄 review-04 CP-016 步骤 3.4）；ExplorerPanel.tsx 接 props 透传。
测试：sideViewRegistry.test.ts 新增状态槽组；use-file-tree.test.ts 新增 CP-016 恢复组（四场景）+ 既有 renderHook 调用点补传；新增 sidebar-area-viewstate.test.tsx。
文档：sideViews/CLAUDE.md FE-21 节改写 + 外部坑两条修订；explorer/CLAUDE.md 「已知行为：换区重建丢失展开状态」段删并换槽位契约。
自查：npx tsc --noEmit 绿；grep -n "getViewState" src/features/sideViews/SideBarArea.tsx = 2 处。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp037-cm-keepalive',
    prompt: `你负责 CP-037（markdown preview-only 改 CM display:none 保活）：
照抄 checklist CP-037 步骤 1-3：MarkdownPanel.tsx:364-368 条件渲染改恒挂载 Allotment.Pane visible={mode !== "preview"}（照抄代码块）；useCodeMirror 调用 container 去三元（恒传 cmContainerRef.current）；头注释 :13-16 改 CP-037 口径。
测试：markdown-panel.test.tsx 既有 preview 断言翻转 + 新增防复发组三例（EditorView 构造 spy 仅一次/preview 态 CM pane 在 DOM/onDocContent 链不断）。
文档：panels/CLAUDE.md docViewer 家族节「preview-only 卸载 CM」句替换；panels/markdown/CLAUDE.md 同步改写。
S10 复核注记：本形态在预览迁 webview 后须复核——在你的报告末尾显式写「S10 复核点：CP-037 保活形态待 S10-② 复核」一句即可，不做额外改动。
自查：npx tsc --noEmit 绿；条目验证段 grep 断言满足。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp019-spawn-resizeobserver',
    prompt: `你负责 CP-019（PTY spawn 改 ResizeObserver 事件驱动）：
照抄 checklist CP-019 步骤 1-4：useXterm.ts 删 MAX_FRAMES/FIT_TIMEOUT 声明与 pollFitAndSpawn 定义及启动 rAF（doSpawn 与 doSpawnRef 原样保留）；原 :384 处替换为事件驱动 spawn 块（照抄代码块：spawned 守卫 + spawnWithFit + spawnObserver + 500ms 超时防御）；清理段 rAF 取消替换为 spawnObserver.disconnect() + clearTimeout(spawnTimeoutId)；头注释同步改口径。
测试：use-xterm-lifecycle.test.ts T1-T4 改写为 ResizeObserver 驱动（补 globalThis.ResizeObserver mock——记录回调手动触发；T3/T4 合并为 500ms 超时兜底）；文件头注释同步改。
文档：panels/CLAUDE.md「PTY spawn 等待布局就绪」节整节改写。
自查：npx tsc --noEmit 绿；grep -n "requestAnimationFrame" src/panels/terminal/useXterm.ts = 0。
完成后报告修改文件清单。`,
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
4. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
页删除路径复核结论（CP-036 收尾复核点，证据取自修复报告）：
---
${refactorResults[0] ?? '（未返回）'}
---
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
