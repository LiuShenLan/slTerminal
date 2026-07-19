// =====================================================================
// Stage 05: commit 侧栏视图（CV-FE-11~15）
// =====================================================================
// 跨边界契约（写死）：
//   - 状态→面板映射（openCommitFile.ts 顶部导出常量）：
//     added → { panelType: "editor", suffix: "(git add)" }
//     untracked → { panelType: "editor", suffix: "(git not add)" }
//     deleted → { panelType: "gitshow", suffix: "(git delete)" }
//     modified|renamed|conflict → { panelType: "diff", suffix: "(git diff)" }
//   - 视图注册: { id: "commit", title: "Commit", icon: "🔀" }
//   - 状态文案: "选择一个项目以查看变更" / "加载中…" / "当前项目并非 git 项目" / "无变更文件"
//   - data-e2e: commit-view / commit-changes / commit-unversioned / commit-file-item
// fix-loop 调用时 args.constraints 传 ""
// =====================================================================

export const meta = {
  name: 'stage05-commit-view',
  description: 'Stage 05: commit 侧栏视图（Changes/Unversioned 列表 + 双击分派）（CV-FE-11~15）',
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
本 Stage 单 agent（sideViewDefs.ts + commit 目录同一功能链）。不跑测试，由全量测试 agent 单点跑。

跨边界契约（写死，不得偏离）：
- 状态→面板映射：added→editor+"(git add)"；untracked→editor+"(git not add)"；deleted→gitshow+"(git delete)"；modified/renamed/conflict→diff+"(git diff)"
- 文案："选择一个项目以查看变更" / "加载中…" / "当前项目并非 git 项目" / "无变更文件" / 标题栏 "COMMIT"
- data-e2e：commit-view / commit-changes / commit-unversioned / commit-file-item
- 颜色一律 theme/colors.ts token（GIT_FILE_COLORS / EXPLORER_COLORS / SIDEBAR_*），禁止硬编码色值`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "commit-view",
    prompt: `你负责 CV-FE-11 / CV-FE-12 / CV-FE-13 / CV-FE-14 / CV-FE-15 全部 5 项：

【CV-FE-11】CommitView 组件
- 新建 src/features/commit/（index.ts、CommitView.tsx、CommitFileList.tsx）
- 状态机（优先级自上而下）：无 rootPath → "选择一个项目以查看变更"；loading → "加载中…"；gitStatus 任意失败 → "当前项目并非 git 项目"；正常 → 两列表
- 标题栏 "COMMIT"（28px 高、大写、letterSpacing 1、fontSize 11，样式照 ExplorerPanel 标题栏）
- 两个可折叠列表框 Changes (N) / Unversioned Files (N)：默认展开；折叠 state 仅组件内 useState 不持久化；空列表时标题仍显示（计数 0），展开内容区显示"无变更文件"
- 行渲染：文件名色 = GIT_FILE_COLORS[status] ?? EXPLORER_COLORS.fg；灰色父目录相对路径后缀（INPUT_BORDER token，relativePath/basename 用 src/lib/path）；按完整相对路径字母序排序
- Changes 内容：status ∈ added/modified/deleted/renamed/conflict；Unversioned：status === untracked
- data-e2e：容器 commit-view；两列表 commit-changes / commit-unversioned；列表项 commit-file-item

【CV-FE-12】useCommitStatus hook（src/features/commit/useCommitStatus.ts）
- rootPath 推导照 ExplorerPanel（activePageId → project.pages → cwd || rootPath）
- gitStatus(rootPath) 加载一次；onFsEvent + 200ms debounce 自动刷新
- rootPath 变化立即清空旧数据 + 重载（参考 useFileTree generation 取消模式）
- rootPath 为 null 不调 IPC

【CV-FE-13】openCommitFile（src/features/commit/openCommitFile.ts）
- 顶部导出映射常量（策略模式）：
  added → { panelType: "editor", suffix: "(git add)" }
  untracked → { panelType: "editor", suffix: "(git not add)" }
  deleted → { panelType: "gitshow", suffix: "(git delete)" }
  modified/renamed/conflict → { panelType: "diff", suffix: "(git diff)" }
- 流程照 ExplorerPanel.handleOpenFile：findExistingEditor(pageId, filePath, suffix) 命中 → getPanel + focus；未命中 → titleManager.getFileEditorTitle(pageId, root, filePath, suffix) 算标题 → dockApi.addPanel({ id: \`\${panelType}-\${Date.now()}\`, component: panelType, title, params: { panelId, filePath, repoPath, ...(renamed 时 oldPath) } }) → registerEditor(pageId, panelId, filePath, suffix) → recomputeTitles 更新冲突标题
- addPanel 抛错 → 不注册 titleManager（try-catch 照 ExplorerPanel）
- activePageId / dockApi 缺失 → 直接返回

【CV-FE-14】注册 commit 视图
- src/features/sideViews/sideViewDefs.ts 追加：sideViewRegistry.register({ id: "commit", title: "Commit", icon: "🔀", component: () => React.createElement(CommitView) })

【CV-FE-15】L2 测试（新建 src/__tests__/commit-view.test.tsx，可按需拆分）
- mock ../ipc/git（gitStatus）+ ../ipc/notify（onFsEvent）；真实 stores 种子（照 explorer 测试模式，可复用 helpers/workspace-setup.ts）
- 覆盖：状态机 4 态文案 / 列表渲染（颜色 style 断言、计数、排序、空态"无变更文件"）/ 折叠交互（点击标题折叠再展开）/ 双击分派 4 类状态（mock window.__dockviewApi 的 addPanel/getPanel，断言 component/title/params）/ 去重聚焦（findExistingEditor 命中 → focus 不 addPanel）/ 普通编辑器与 git 页签共存不误聚焦（B10）/ fs-event 200ms debounce 刷新（fake timers）/ rootPath 切换清空重载`,
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
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/commit-view/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
