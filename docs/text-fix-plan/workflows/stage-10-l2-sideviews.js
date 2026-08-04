// =====================================================================
// Stage 10 L2-sideviews/commit：交互精度与拆分
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-10.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试；font-size/keybindings 测试在 Stage 12 还会再碰（STS-06），本 Stage 只加 cancelPendingSave 用例，不删改既有用例
// =====================================================================

export const meta = {
  name: 'stage10-l2-sideviews',
  description: 'L2 sideviews drop index 断言 + cancelPendingSave + commit-view 拆分',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试。并行 agent 文件零重叠（sv-activity 碰 activityBar/sideBarArea/workspace-sideviews/sideBarState 测试；sv-store 碰 sideBar/font-size/keybindings 测试；sv-commit 碰 commit-view（拆三文件）/commitContextMenu/openCommitFile 测试）。font-size/keybindings 在 Stage 12 还会再碰——本 Stage 只加 cancelPendingSave 用例。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'sv-activity',
    prompt: `你负责 SVC-01、SVC-05、SVC-06、SVC-07、SVC-10、SVC-13①（sideBarState 部分），触碰文件：src/__tests__/activityBar.test.tsx、sideBarArea.test.tsx、workspace-sideviews.test.tsx、sideBarState.test.ts。逐 ID 对照 checklist 原文实施：

【SVC-01】activityBar drop 不校验 moveButton index。位置 activityBar.test.tsx:260-495。全部 drop 用例只断言 zone，computeDropTarget 落点 index 零守卫。每个 drop 用例追加 expect(moveSpy.mock.calls[0][2]).toBe(expectedIndex)；新增同按钮上/下半区插入位置差异用例。

【SVC-05】resolveTargetZone 中点边界未锁定。位置 src/features/sideViews/ActivityBar.tsx:93-99。现有用例 clientY 远离中点，阈值 >= rect.top + height/2 边界未测。补 clientY 恰好等于中点（→bottom）、中点 -1（→top）边界用例。

【SVC-06】moveButtonPure R7 目标区非空场景未测。位置 src/features/sideViews/sideBarState.ts:105-151。补"跨区拖拽未打开视图且目标区已有打开视图"用例（仅归属变化，open 不动）。

【SVC-07】SideBarArea total<=0 除零守卫未覆盖。位置 src/features/sideViews/SideBarArea.tsx:75-82。构造 total=0 场景（sizes=[0,0] 等）断言不 NaN/不崩溃、store 不被写入。

【SVC-10】workspace-sideviews props typeof 弱断言。位置 workspace-sideviews.test.tsx:284-300。typeof props.switchToPage === "function" 改引用断言（toBe 传入函数，SideBarArea 收到的与 Workspace 传入同一引用）。

【SVC-13①】sanitizeSideBar NaN/Infinity 分支未覆盖。位置 sideBarState.ts:67-70。补 width/splitRatio 为 NaN/Infinity/-Infinity 时 clamp 回退 min 用例。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'sv-store',
    prompt: `你负责 SVC-02、SVC-13②（stores/sideBar clamp 部分），触碰文件：src/__tests__/sideBar.test.ts、font-size.test.ts、keybindings.test.ts。逐 ID 对照 checklist 原文实施：

【SVC-02】sideBar.ts cancelPendingSave 零覆盖（含三 store 活跃 timer 分支）。位置 src/stores/sideBar.ts:143-149、fontSize.ts:82-85、keybindings.ts:85-89。关窗冲刷依赖 cancelPendingSave，活跃 timer 取消分支全未测（关窗竞态写盘）。触发变更产生 timer → 调 cancelPendingSave → 推进 2s → 断言 saveSettings 未再调用（三 store 各一条）。

【SVC-13②】stores/sideBar clamp NaN 分支。位置 src/stores/sideBar.ts:59。setWidth/setSplitRatio 传 NaN/Infinity 时 clamp 回退 min 用例。

注意：font-size/keybindings 测试在 Stage 12 还会再碰（STS-06 afterEach 清理）——本 Stage 只加 cancelPendingSave 用例，不删改既有用例结构。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'sv-commit',
    prompt: `你负责 SVC-03、SVC-04、SVC-08、SVC-09、SVC-11、SVC-12、SVC-14，触碰文件：src/__tests__/commit-view.test.tsx（拆三文件）、commitContextMenu 测试、openCommitFile 测试。逐 ID 对照 checklist 原文实施：

【SVC-03】useCommitStatus debounce 清理与去抖未覆盖。位置 src/features/commit/useCommitStatus.ts:88-108。连续 fs-event 仅 1 次 gitStatus（200ms 去抖）；激活 timer 后 unmount 断言 clearTimeout。

【SVC-04】openCommitFile 四条守卫路径未覆盖。位置 src/features/commit/openCommitFile.ts:47,65,112,122。补无 pageApi/未知状态（!dispatch return）/rootPath 缺失 return/addPanel 失败降级用例 + recomputeTitles 更新标题断言。

【SVC-08】CommitFileList 菜单交互 + oldPath 回退未覆盖。位置 src/features/commit/CommitFileList.tsx:130,133,253。补右键菜单打开/项点击 hover、renamed 无 oldPath 时 params.oldPath 为 undefined 用例。

【SVC-09】commitContextMenu 删除 catch 未覆盖。位置 src/features/commit/commitContextMenu.ts:76,85。补 gitUnstage/deleteEntry 失败 catch（静默/console.error）用例——菜单 action 不抛。

【SVC-11】B10 反向用例错位——改经 openCommitFile。位置 commit-view.test.tsx:488-497。B10（suffix 去重）反向用例直测 titleManager 而非 commit 分派路径。改经 openCommitFile 驱动验证"同文件不同 suffix 不误聚焦"（打开普通 editor 后再用 modified 调用，验证 addPanel 仍触发）。

【SVC-12】commit-view fake timers 混 waitFor。位置 commit-view.test.tsx:532-642。rootPath 切换用例统一计时策略（fake timers + advanceTimersByTimeAsync），不混 waitFor。

【SVC-14】commit-view.test.tsx 850+ 行拆分。拆分为状态机/分派去重/右键菜单三文件（commit-view.test.tsx 状态机列表、commit-open-file.test.ts 分派去重、commit-context-menu-ui.test.tsx 右键菜单交互）。

完成后报告：每项改动摘要 + 修改文件清单（含拆分后新文件）。`,
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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-10.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage10 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。注意 commit-view 已拆三文件——相关断言文件路径以拆分后布局为准。
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
