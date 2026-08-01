// =====================================================================
// Stage 04: 历史数据层（分组/搜索/⚡派生）+ 四步恢复编排
// 覆盖项: FE-04、FE-05、FE-06
// 跨边界契约（写死，双 agent 不各自推断）:
//   useClaudeHistory() 返回形状:
//     { state, sessions, activeIds, rootPath, scan, removeLocal, updateLocalTitle }
//     state: "idle" | "loading" | "ready" | "error"
//     sessions: HistorySession[]；activeIds: Set<string>；rootPath: string | null
//     scan(): Promise<void>；removeLocal(id: string): void；updateLocalTitle(id: string, title: string): void
//   restoreHistorySession(session: HistorySession, opts?: { fork?: boolean }): Promise<void>
//   注入命令: 普通 = "claude --resume <id>\r"；fork = "claude --resume <id> --fork-session\r"
//   panelId = "terminal-{pageId}-{Date.now()}"
// 特殊纪律: 本 Stage 不建 src/features/claudeHistory/index.ts（barrel 归 Stage 05 agent B，防双 agent 冲突）
// fix-loop 调用本 Stage 时 args.constraints 传:
//   "本 Stage 不建 src/features/claudeHistory/index.ts（barrel 归 Stage 05）"
// =====================================================================

export const meta = {
  name: 'stage04-frontend-data',
  description: 'Stage 04: 历史数据层（分组/搜索/⚡派生）+ 四步恢复编排',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 FE-04/05/06 条目 + docs/claude-history-view/stages.md Stage 04 实现要点与「跨 Stage 契约」段，再动手。本 Stage 不建 src/features/claudeHistory/index.ts（barrel 归 Stage 05，防双 agent 冲突）。`

// === Phase 1: 并行重构（双 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'data-layer-agent',
    prompt: `你负责 FE-05、FE-04（数据层）：

【FE-05 src/features/claudeHistory/historyModel.ts — 纯函数模块，禁 import react/任何 hook】
- isCurrentProject(sessionCwd: string | null, rootPath: string | null): boolean — 决策 24：两侧经 normalizePath（复用 src/lib/path.ts）+ 忽略大小写后精确相等；任一侧 null/空 → false。
- groupByCwd(sessions: HistorySession[]): 分组结构 — cwd 为 null 归「(未知目录)」组（key 用 null 或专用常量，注释说明）；组内 mtimeMs 降序；组间按组内最大 mtimeMs 降序。
- matchesSearch(session: HistorySession, query: string): boolean — 标题 + firstPrompt 匹配，大小写不敏感；query 空白 → true。
- formatRelativeTime(mtimeMs: number, nowMs: number): string — 六档：<1 分钟「刚刚」；<60 分钟「N 分钟前」；<24 小时「N 小时前」；<7 天「N 天前」；同年「MM-DD」；跨年「YYYY-MM-DD」（决策 26 时间口径 = 文件 mtime）。
- deriveActiveSessionIds(): Set<string> — 从 TerminalRegistry.getAll()（src/panels/terminal/TerminalRegistry.ts，先 Read 确认 API）取各注册终端的 claudeSession?.transcriptPath，basename（去 .jsonl 后缀）→ sessionId 集合；无 transcriptPath 的条目不产出 id（无 matchedCommand 分支——matchedCommand-only 会话无法标记是已接受局限，注释说明）。

【FE-04 src/features/claudeHistory/useClaudeHistory.ts】
- 数据 hook，返回形状逐字契约：{ state, sessions, activeIds, rootPath, scan, removeLocal, updateLocalTitle }。
- 状态机 state: "idle" | "loading" | "ready" | "error"；初始 idle（未扫描）。
- rootPath 推导：activePageId → 所属 project.rootPath（照 src/features/commit/useCommitStatus.ts 先例，先 Read）。
- scan()：调 src/ipc/claudeHistory 的 scanHistory() → 成功置 ready + sessions，失败置 error（console.error 留痕，不静默吞）；供历史区首次展开与手动刷新按钮调用。
- removeLocal(id) / updateLocalTitle(id, title)：纯本地更新 sessions（删除条目 / 改 title + titleSource="customTitle"），不触发 scan、无 IPC（删除/重命名的 IPC 由调用方先做，成功后调本函数即时刷新）。
- activeIds：初值 deriveActiveSessionIds()；useEffect 内 TerminalRegistry.subscribe 监听 register/remove/sessionChange 后重算；卸载时取消订阅。
- generation 防竞：scan 进行中再触发 → 旧结果丢弃（照 useFileTree genRef 模式）。

【测试（L2，src/__tests__/）】
- claude-history-model.test.ts：纯函数全分支——isCurrentProject（等/大小写差异/反斜杠差异/null 两侧）、groupByCwd（组内排序/组间排序/无 cwd 组）、matchesSearch（标题命中/prompt 命中/大小写/空白 query/未命中）、formatRelativeTime 六档边界、deriveActiveSessionIds（有 transcriptPath/无 transcriptPath/空注册表）。
- claude-history-hook.test.tsx：renderHook 驱动——初始 idle、scan 成功 ready+sessions、scan 失败 error、removeLocal/updateLocalTitle 不触发 IPC（spy scanHistory 计数）、TerminalRegistry.subscribe 事件后 activeIds 更新、卸载取消订阅。mock ../ipc/claudeHistory 与 TerminalRegistry（vi.hoisted 模式，照既有测试先例）。`,
  },
  {
    label: 'restore-agent',
    prompt: `你负责 FE-06（四步恢复编排）：

【FE-06 src/features/claudeHistory/restoreSession.ts】
- 导出 async function restoreHistorySession(session: HistorySession, opts?: { fork?: boolean }): Promise<void>。
- 四步顺序（决策 6/25，全部复用既有原语，不改 workspace/stores 代码）：
  1. 项目入列：useProjects.getState() 查 projects 是否已有 rootPath 与 session.cwd 规范化相等的项目（复用 src/lib/path.ts normalizePath + 忽略大小写）；无则 addProject({ name: basename(cwd), rootPath: cwd, pages: [], ... })（照 SidebarTree handleAddProject 构造形状，先 Read src/stores/projects.ts 的 Project 类型与 addProject 签名）。session.cwd 为 null 时调用方已前置拦截，本函数入口仍防御性 throw。
  2. 页面保障：该项目 pages 为空则 addPage（名称「页面-N」照 SidebarTree 模式 + makeEmptyLayout() 空布局，先 Read src/features/sidebar/index.ts 的 makeEmptyLayout 签名）。
  3. 页面切换：await switchToPageShared(pages[0].pageId)（src/workspace/pageApis.ts，先 Read 签名；setProjectRoot 前置语义由其内部保证）。
  4. 终端恢复：轮询 getPageApi(pageId)（100ms × 50，照 openHooksConfigPanel 模式）→ addPanel({ id: panelId, component: "terminal", params: { panelId, cwd }, renderer: "always" })，panelId = "terminal-{pageId}-{Date.now()}" → 轮询 TerminalRegistry 出现该 panelId 注册（100ms × 50）→ pty.write（src/ipc/pty，先 Read 签名——含 panelId 参数与 Uint8Array 序列化约定）注入 "claude --resume <id>\\r"；opts.fork 时注入 "claude --resume <id> --fork-session\\r"。
- 防重入：模块级 restoring 标记，重入直接 return（注释说明并发双击场景）。
- 失败路径：任一步骤异常 → sendToastNotification（src/ipc/notification.ts，先 Read 签名）提示失败 + console.error；不静默吞错。
- 孤儿行（cwdExists=false）/无 cwd 行的禁用判定在调用方（Stage 05 菜单/双击分派），本函数不做。

【测试（L2，src/__tests__/claude-history-restore.test.ts）】
- mock useProjects / pageApis（switchToPageShared、getPageApi）/ ../ipc/pty / TerminalRegistry / ../ipc/notification（vi.hoisted 模式）。
- 断言：四步调用顺序（spy 调用序）；已有项目跳过 addProject；已有页面跳过 addPage；pty.write payload 为 "claude --resume <id>\\r"（普通）与 "...--fork-session\\r"（fork）；防重入（并发两次调用第二次无副作用）；失败 toast（addPanel throw → sendToastNotification 被调）。`,
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
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 4 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
