// =====================================================================
// stage-12-frontend-perf.js — S12 前端性能 + 页数上限
// （FE-01/15/16/17/19/20/21/29/32/33/34、BE-19）
// =====================================================================
// 跨边界契约（写死）：
//   MAX_PAGES = 20（stores/projects.ts，超限 addPage 拒绝 + toast「页面数已达上限」）。
//   agent_history_scan(cli_id: String, force: Option<bool>)（BE-19 缓存 + 显式刷新）；
//   前端 wrapper scanAgentHistory(cliId: string, force?: boolean)。
//   缓存键 = (目录 mtime, 文件数)，命中复用；force=true 绕过。
// fix-loop 调用约定：args.testCommands 传本脚本下方 6 条门禁；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage12-frontend-perf',
  description: 'Stage 12: 页数上限 20 + 树/订阅/启动加载性能优化 + 历史扫描缓存（FE-01/15/16/17/19/20/21/29/32/33/34、BE-19）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；WebGL 检测逻辑（failIfMajorPerformanceCaveat 相关）禁止触碰。
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S12 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：MAX_PAGES = 20（超限 addPage 拒绝 + toast「页面数已达上限」）；agent_history_scan(cli_id, force: Option<bool>)，前端 scanAgentHistory(cliId, force?)；缓存键 = (目录 mtime, 文件数)。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'perf-trees',
    prompt: `你负责 FE-15、FE-16、FE-19（前端侧），只许改 src/features/explorer/useFileTree.ts、src/features/navTree/useNavTree.ts、src/ipc/agentHistory.ts 及 src/__tests__/ 下对应测试：
【FE-15】file-saved 事件 300ms debounce；已知路径变更只刷新受影响子树（按变更路径定位最近展开祖先刷新，不 refreshExpanded 全量重建）。
【FE-16】useNavTree 历史归属建索引 Map<projectId, sessions> 消 O(N×M) 前缀匹配；tree 派生 useMemo 依赖精确化 + 稳定引用缓存。
【FE-19】src/ipc/agentHistory.ts wrapper 改 scanAgentHistory(cliId: string, force?: boolean)（契约）；useNavTree 挂载一次扫描，展开历史节点不重复 scan（仅显式刷新/恢复完成时 force=true）。
补 L2 测试：debounce 行为、子树刷新范围、索引命中、force 透传。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'perf-terminal',
    prompt: `你负责 FE-17、FE-29、FE-32、FE-34，只许改 src/panels/terminal/TerminalPanel.tsx、src/panels/terminal/useXterm.ts 及 src/__tests__/、test/terminal/ 下对应测试：
【FE-17】TerminalPanel（:111-129）订阅整个 TerminalRegistry 事件——订阅回调内按 e.panelId === 自身 panelId 过滤后再 setState。
【FE-29】TerminalPanel.tsx:206 加载遮罩移除 transition: opacity 0.3s（ADR-0003 无动效），显隐切换保持。
【FE-32】useLayout/useFontSize 改 selector 精确订阅（仅取所需字段）。
【FE-34】WebGL 上下文按焦点切换创建/释放（useXterm）——改为 WebGL addon 加载失败回退才重建，焦点切换不主动释放；【禁区】failIfMajorPerformanceCaveat 检测逻辑不动；若实现中发现多上下文压力证据（注释记录），恢复原释放逻辑并报告。
补/调 L2/L3 测试：订阅过滤、selector 断言、WebGL 重建路径。
只做 npx tsc --noEmit 编译级检查，禁止 npm test / npm run test:l3。`
  },
  {
    label: 'perf-workspace',
    prompt: `你负责 FE-01、FE-20、FE-21、FE-33，只许改 src/stores/projects.ts、src/workspace/Workspace.tsx、src/App.tsx、src/features/sideViews/SideBarArea.tsx 及 src/__tests__/ 下对应测试：
【FE-01】保持多 Dockview 实例（D1/H6 架构不动）；stores/projects.ts 加 MAX_PAGES = 20（契约），超限 addPage 拒绝 + toast.show("warning", "页面数已达上限")；Workspace.tsx 注释说明多实例 + 上限的豁免决策（豁免登记在 S19）。
【FE-20】App.tsx:44-69 字体/快捷键/侧栏三个 loadFromDisk 改 Promise.all 并行（各自独立 try/catch 保留）；loadAllProjects 保持在其后（markPersistenceReady 时序不动）。
【FE-21】SideBarArea 隐藏视图按需卸载（条件渲染替代 display:none 保挂载——切换时卸载旧视图组件，状态丢失语义 ADR-0001 已接受；导航树滚动位置等轻状态不保活）。
【FE-33】Workspace.tsx pageCallbacksRef effect 依赖 allPages 重建回调 map——回调按 pageId 惰性创建 + 缓存（getOrCreate 模式），effect 依赖收窄。
补 L2 测试：MAX_PAGES 拒绝+toast、三 store 并行加载、隐藏视图卸载、回调缓存。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'history-backend',
    prompt: `你负责 BE-19，只许改 src-tauri/src/agent_history/claude/scan.rs、src-tauri/src/agent_history/mod.rs：
历史扫描逐文件读取无索引/缓存。修复：扫描结果按 (目录 mtime, 文件数) 做进程内缓存（键不变命中则复用，不重复读盘）；agent_history_scan 命令加 force: Option<bool> 参数（契约：true 绕过缓存强制重扫）。
补 L1 测试：缓存命中不重复读盘（构造目录计数断言）、mtime/文件数变化失效、force=true 绕过。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 12 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-12.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
