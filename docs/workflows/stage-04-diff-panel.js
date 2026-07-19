// =====================================================================
// Stage 04: diff 双栏面板（CV-FE-06~10）
// =====================================================================
// 跨 agent 契约（写死，两边 agent 不各自推断）：
//   - gutter-head 导出签名: buildHeadRangeSet(state: EditorState, hunks: DiffHunk[]): RangeSet<GutterMarker>
//     （src/panels/editor/gitGutter.ts 导出，diff-panel 按此签名 import）
//   - 面板类型 "diff"；params { panelId, filePath, oldPath?, repoPath }
//   - alignment 导出: computeAlignment(hunks: DiffHunk[]): { left: Map<number, number>; right: Map<number, number> }
//     （key = 在该行之后插入占位，value = 占位行数）
//   - data-e2e: "diff-panel" / "diff-left" / "diff-right"
// fix-loop 调用时 args.constraints 传 ""
// =====================================================================

export const meta = {
  name: 'stage04-diff-panel',
  description: 'Stage 04: diff 双栏面板（占位对齐 + 双侧 gutter + 滚动同步）（CV-FE-06~10）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
并行纪律：禁止跑 npm test 全量——真实测试由全量测试 agent 单点跑；你只写代码与测试文件，不执行。
假设来源注释：占位 widget 行高与滚动同步的真实渲染行为无法被 jsdom/E2E 自动验证——在这两处代码注释记录假设来源（IDEA 交互参考 + jsdom 验证边界），由收尾人工实测兜底（stages.md Stage 04 人工验证点）。

跨 agent 契约（写死，不得偏离）：
- buildHeadRangeSet(state: EditorState, hunks: DiffHunk[]): RangeSet<GutterMarker>（gitGutter.ts 导出）
- computeAlignment(hunks: DiffHunk[]): { left: Map<number, number>; right: Map<number, number> }（diff/alignment.ts 导出）
- 面板 params { panelId, filePath, oldPath?, repoPath }；data-e2e: diff-panel / diff-left / diff-right`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "diff-panel",
    prompt: `你负责 CV-FE-07 / CV-FE-08 / CV-FE-09 / CV-FE-10（alignment + 面板测试部分）：

【CV-FE-08】占位对齐纯函数（先做，面板依赖它）
- 新建 src/panels/diff/alignment.ts
- export function computeAlignment(hunks: DiffHunk[]): { left: Map<number, number>; right: Map<number, number> }
- 规则（key = 在该行之后插入占位，value = 占位行数）：
  纯新增（oldLines=0）→ left 在对应位置插 newLines 个
  纯删除（newLines=0）→ right 在对应位置插 oldLines 个
  modified 行数不等 → 少的一侧插差值个（oldLines>newLines → right 插 old-new；newLines>oldLines → left 插 new-old）
- 纯函数零 DOM，DiffHunk 类型从 src/types/git import

【CV-FE-07】DiffPanel 实现
- 新建 src/panels/diff/（index.ts、DiffPanel.tsx，可按需拆分 hooks）
- params { panelId, filePath, oldPath?, repoPath }
- 布局：纵向均分两栏 flex 50/50，容器 data-e2e="diff-panel"，左右 data-e2e="diff-left" / "diff-right"
- 左栏：gitFileAtHead(repoPath, oldPath ?? filePath) → 只读 CM（EditorState.readOnly + EditorView.editable(false)）+ 语言扩展 + HEAD 侧 gutter（buildHeadRangeSet，契约签名 import 自 ../editor/gitGutter）+ 占位 widget
- 右栏：fs.readFile(filePath) → 可编辑 CM（basicSetup + oneDark + 高度 theme + 字体 + 语言 + diffGutter()）+ workdir gutter（gitDiff(repoPath, filePath) → updateDiffGutter）+ 占位 widget
- 占位对齐：computeAlignment(gitDiff 结果) → Decoration.widget block 级空白行（高度=行高、不可选中、无文本）
- 滚动同步：两 .cm-scroller 各挂 scroll 监听，一侧滚动同步另一侧 scrollTop；syncingRef 防循环；水平滚动各自独立
- 右侧 Ctrl+S：usePanelFocus("editor", container, activate, deactivate) + setActiveEditor({ save })；save = fs.writeFile 写回 → 重新 gitDiff → 刷新 gutter + 重算 alignment dispatch（照 useCodeMirror handleSave 的写盘部分语义）
- 左栏刷新：onFsEvent 检测路径含 .git → 重取 HEAD 内容
- 右栏外部修改：净自动重载 / 脏 window.confirm 选择（照 useCodeMirror 语义）
- 大文件：阈值常量从 useCodeMirror 导出复用（MAX_FILE_SIZE_BYTES / LARGE_FILE_WARN_BYTES）

【CV-FE-09】注册 diff 面板类型
- src/panels/index.ts 导出 DiffPanel
- src/panelRegistry.ts：PANEL_DIFF = "diff"；panelRegistry.diff；PANEL_TYPES 追加
- 更新 src/__tests__/panel-registry.test.ts（五面板断言）

【CV-FE-10】diff 面板 L2 测试（你负责 alignment + 面板部分；HEAD gutter 映射测试归另一 agent）
- src/__tests__/diff-alignment.test.ts：computeAlignment 全分支（纯增/纯删/等行修改/old>new/new>old/多 hunk/空 hunks）
- src/__tests__/diff-panel.test.tsx：滚动同步（模拟 scroll 事件断言对侧 scrollTop + 防循环）；保存后重新调 gitDiff + gutter 刷新（mock ../ipc/git）`,
  },
  {
    label: "gutter-head",
    prompt: `你负责 CV-FE-06：HEAD 侧（左侧）gutter 映射（只改这两个文件）
文件：src/panels/editor/gitGutter.ts、src/__tests__/gitGutter.test.ts

要点：
1. gitGutter.ts 新增导出：export function buildHeadRangeSet(state: EditorState, hunks: DiffHunk[]): RangeSet<GutterMarker>
2. 映射规则（old 行号，1-based）：
   - modified hunk（oldLines>0 且 newLines>0）→ oldStart..oldStart+min(oldLines,newLines)-1 标 ModifiedMarker
   - 纯删除（newLines=0）→ oldStart..oldStart+oldLines-1 标 DeletedMarker
   - oldLines>newLines 的剩余 old 行（oldStart+shared 起）→ DeletedMarker
   - 纯新增（oldLines=0）→ 无标记
3. 复用现有 ModifiedMarker / DeletedMarker 类与 GIT_GUTTER_COLORS token，不新造颜色
4. 现有 buildRangeSet（workdir 侧）逻辑与现有测试零改动
5. L2 测试（gitGutter.test.ts 追加 describe）：单行修改 old 行映射 / 连续删除 old 行映射 / old>new 混合 hunk / 纯新增无标记`,
  },
];
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
