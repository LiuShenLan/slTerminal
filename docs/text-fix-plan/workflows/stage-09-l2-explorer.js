// =====================================================================
// Stage 09 L2-explorer/sidebar：高频路径补齐
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-09.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试；若发现生产代码缺陷（如 fullRefresh 死代码确认），按 EXP-03 处置（删除或接线）并报告
// =====================================================================

export const meta = {
  name: 'stage09-l2-explorer',
  description: 'L2 explorer OpenInTerminal/CRUD 成功路径/FileIcon 补齐',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试（EXP-03 的 fullRefresh 死代码处置除外：确认无调用方后可删除或接线，属 checklist 明确授权）。并行 agent 文件零重叠（ex-panel 碰 explorer-*.test.tsx + useFileTree/FileTree 相关；ex-side 碰 sidebar-actions/FileIcon/file-viewer-registry 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'ex-panel',
    prompt: `你负责 EXP-01、EXP-02、EXP-03、EXP-04、EXP-06、EXP-07、EXP-10、EXP-11，触碰文件：src/__tests__/explorer-*.test.tsx（含 useFileTree/FileTree 相关测试）。逐 ID 对照 checklist 原文实施：

【EXP-01】handleOpenInTerminal 零覆盖。位置 src/features/explorer/ExplorerPanel.tsx:251-262。右键"在终端中打开"触发后断言 addPanel 参数：component="terminal"、params.cwd 为目录（文件取父目录）、panelId 格式（terminal-{pageId}-{seq}）、renderer:"always"。

【EXP-02】CRUD 成功路径未断言。位置 ExplorerPanel.tsx:321,335,347,349-350。删除/重命名/新建成功后 refresh()、setRenamingPath(null)、setNewFileName(null) 未断言——静默失败不红。每个 CRUD 操作补成功路径断言（IPC 调用 + refresh 触发（readDir 二次调用）+ 状态重置）。

【EXP-03】fullRefresh 未调用 + F8 命名误导。位置 src/features/explorer/useFileTree.ts:191-206。fullRefresh 定义但无调用方无测试；F8 用例断言的是初始 mount 的 gitStatus 而非 fullRefresh 结果。确认 fullRefresh 语义（grep 调用方）：死代码则删除或接线；F8 改名或重写为真实 fullRefresh 驱动。

【EXP-04】焦点/失活/hover/错误横幅 dismiss 链路。位置 ExplorerPanel.tsx:91-157。补 focusin/focusout 上下文栈（spy setActiveExplorer/clearActiveExplorer）、hover 高亮（非选中态）、错误横幅 dismiss 按钮/3s 自动消失（fake timers + 卸载清理）用例。

【EXP-06】FileTree 输入框边界。位置 src/features/explorer/FileTree.tsx（重命名 input）。补 Escape 取消、空名、重名、失焦四边界用例（含文件夹级新建）。

【EXP-07】useFileTree 竞态清理分支。位置 useFileTree.ts:65-66,139-140,232,241。补 generation 过期丢弃（旧请求延迟 resolve → 断言丢弃）、rootPath=null 时旧回调不抛错、卸载清理、file-saved 事件缺 path 仍刷新用例。

【EXP-10】handleOpenFile 防御分支。位置 ExplorerPanel.tsx:185,187,227。补无活跃页/无 dockviewApi/重复打开去重聚焦防御用例。

【EXP-11】E6 标题矛盾 + 用例编号重复。位置 explorer-delete.test.tsx:536-547。E6 标题（handler 返回 false）与断言（deleteSelected 被调一次）对齐；全文用例编号去重统一。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'ex-side',
    prompt: `你负责 EXP-05、EXP-08、EXP-09、EXP-12，触碰文件：src/__tests__/sidebar-actions.test.ts、FileIcon 测试（file-icon.test.tsx）、src/__tests__/file-viewer-registry.test.ts。逐 ID 对照 checklist 原文实施：

【EXP-05】FileIcon 扩展名分支未覆盖。位置 src/features/explorer/FileIcon.tsx（.pyw/.markdown/.less/.scss/.gitattributes 等）。表驱动补未覆盖扩展名 → emoji 映射用例（各断言返回对应图标或文本非空）。

【EXP-08】SidebarTree 错误降级分支。位置 src/features/sidebar/SidebarTree.tsx:55-56,342,369,484。补 dialog 取消/IPC 失败降级（console.error + 状态不变）用例：dialog.open reject/返回数组/返回 null → store 不变不抛错；proj 不存在点击项目行菜单不抛错。

【EXP-09】SidebarTree hover/stopPropagation 未覆盖。位置 SidebarTree.tsx（行 hover、按钮 stopPropagation）。补 hover 样式与按钮点击不触发行选择（重命名中点击行不触发 switchToPage）用例。

【EXP-12】FileViewerRegistry 单例 side-effect 恢复。位置 file-viewer-registry.test.ts。_reset() 后模块级单例预注册（html/htm）丢失，影响后续测试。_reset() 后恢复预注册或改 per-test 新实例；并补 _reset 用例 + resolve(".gitignore")/resolve("file.") 边界。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-09.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage09 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
