// =====================================================================
// Stage 07 L2-editor/diff：保存链与分支补齐
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-07.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试；若发现生产代码缺陷，报告主 agent 后另行处理
// =====================================================================

export const meta = {
  name: 'stage07-l2-editor',
  description: 'L2 editor/diff 保存链真实断言 + DiffPanel/useCodeMirror 分支补齐',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试。并行 agent 文件零重叠（ed-diff 碰 diff-panel/diff-alignment 测试；ed-cm 碰 useCodeMirror/gitshow/gitGutter 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'ed-diff',
    prompt: `你负责 EDF-01、EDF-02、EDF-06、EDF-07，触碰文件：src/__tests__/diff-panel.test.tsx、src/__tests__/diff-alignment.test.ts。逐 ID 对照 checklist 原文实施：

【EDF-01】diff-panel 保存链用例名实不符。位置 diff-panel.test.tsx:169-194。注释声称验证 writeFile → gitDiff → updateDiffGutter 刷新链，实际只断言 mock 函数 toBeDefined——永不可失败。真实触发保存（dispatch Ctrl+S 或调 handler，经 mocked usePanelFocus 的 activate 回调获取 editorActions 调 save()），断言 fs.writeFile → gitDiff 重调 → 双侧 gutter/占位刷新全链。

【EDF-02】DiffPanel.tsx 63.9% 关键路径大面积缺失。位置 src/panels/diff/DiffPanel.tsx:239-250（大文件）、275-302（refreshPlaceholders）、306-347（滚动重绑定）、457-468（脏文件确认）、483-501（.git 刷新）。按源码分支补 L2 用例：占位刷新同步（hunks 空/有值两路）、左侧 .git 变更重取 HEAD、外部修改净重载/脏弹窗、滚动同步重绑定（state.kind/headContent 变化后 effect 重绑）、大文件阈值。

【EDF-06】alignment key>=0 false 分支未覆盖。位置 src/panels/diff/alignment.ts:38,44。补 key<0（newStart=0）过滤分支用例。

【EDF-07】diff-panel 滚动同步固定 200ms 延时。位置 diff-panel.test.tsx:198-291。改 fake timers 或轮询断言（mock addEventListener 等待绑定），消除固定等待。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'ed-cm',
    prompt: `你负责 EDF-03、EDF-04、EDF-05、EDF-08、EDF-09，触碰文件：src/__tests__/use-code-mirror.test.ts、src/__tests__/gitshow-panel.test.tsx、src/__tests__/git-gutter.test.ts。逐 ID 对照 checklist 原文实施：

【EDF-03】useCodeMirror 大文件拒绝/警告/保存失败无直接回归。位置 src/panels/editor/useCodeMirror.ts:150-210、259-276。>10MB 拒绝、>1MB confirm 取消、fs.writeFile reject 失败 alert 均无编辑器侧直接测试。补三分支用例（mock fs + dialog.confirm）：打开 >10MB 文档被替换为拒绝文案且 filePathRef 清空；>1MB confirm 返回 false 取消；writeFile reject → alert 且不派发保存事件。

【EDF-04】gitshow 大文件警告断言薄弱 + params 切换断言无法区分。位置 gitshow-panel.test.tsx:269-279、319-387。大文件警告 header 断言薄弱；params.filePath 切换时"旧 view 销毁新 view 创建"的断言无法区分两者。警告 header 精确断言（文案出现）；切换用例断言 EditorView 实例 identity 变化（或销毁/创建计数）。

【EDF-05】gitGutter 四个 dispatch wrapper 未直接测试。位置 src/panels/editor/gitGutter.ts:261-328（updateDiffGutter/clearDiffGutter/updateHeadDiffGutter/clearHeadDiffGutter）。补四 wrapper 直接调用用例（真实 EditorView + StateField 值变化断言，dispatch 的 StateEffect 类型正确）。

【EDF-08】justSavedRef Set 多实例语义未测。位置 useCodeMirror.ts:143、365-369。模块级 Set 在多编辑器实例并存时的隔离/清理语义未测。补双实例保存-重载互不影响用例（实例 A 保存 a.ts、实例 B 收到 a.ts 的 Modify 事件仍对 B 执行自动重载）。

【EDF-09】gitshow 字号 reconfigure 未覆盖。位置 GitShowPanel.tsx:172-180。补 editorFontSize 变化 → fontCompartment.reconfigure 调用断言（mock reconfigure 或捕获 dispatch，非仅 createEditorFontExtension 被调）。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage07 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
