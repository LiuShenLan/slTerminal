// =====================================================================
// stage-13-stability.js — S13 稳定性与生命周期（FE-22~28、BE-07/08/09）
// =====================================================================
// 跨边界契约（写死）：
//   新命令 pty_kill_all() -> Result<u32, AppError>（返回 kill 数；逐 session
//   kill+join 超时语义同 S06/BE-06）；前端 wrapper ptyKillAll(): Promise<number>。
//   本 Stage 后命令数 34（S05 后 33 + pty_kill_all）。
//   switchToPageAndFocus(pageId: string, signal?: AbortSignal)；
//   restoreSession 的 waitFor(cond, signal?: AbortSignal)——可选参数后向兼容。
//   git_repo_cache LRU 容量 = 8（零新依赖手实现）。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage13-stability',
  description: 'Stage 13: 面板级错误边界 + 生命周期守卫 + pty_kill_all 兜底 + git 缓存 LRU（FE-22~28、BE-07/08/09）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S13 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：pty_kill_all() -> Result<u32, AppError>，前端 ptyKillAll(): Promise<number>；switchToPageAndFocus(pageId, signal?: AbortSignal)；waitFor(cond, signal?: AbortSignal)；git_repo_cache LRU 容量 8（零新依赖）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'boundary',
    prompt: `你负责 FE-22、FE-28、BE-08（前端侧），只许改 src/panelRegistry.ts、src/App.tsx、src/ipc/pty.ts 及 src/__tests__/ 下对应测试：
【FE-22】单面板渲染错误扩大为整页崩溃——panelRegistry.ts 的 components 映射处统一包 inline ErrorBoundary（HOC 单点包裹，ErrorBoundary 从 src/lib 导入）。补 L2 测试：构造抛错面板验证同页其他面板存活。
【FE-28】App.tsx:237-246 的 TitleBar、Workspace 容器、NotificationListener、ConfirmDialogHost、ToastHost 分别包 inline ErrorBoundary（降级渲染占位）。
【BE-08 前端】src/ipc/pty.ts 加 wrapper ptyKillAll(): Promise<number>（契约）；App.tsx 关闭序列（:110-127）：先前端 TerminalRegistry 快速 kill（现状保留），再 ptyKillAll() 兜底（前后端不一致时后端 session 不泄漏）。
补 L2 测试：错误边界隔离、关闭序列调用 ptyKillAll。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'lifecycle',
    prompt: `你负责 FE-23、FE-24、FE-25、FE-26、FE-27，只许改 src/features/agentStatus/useAgentStatus.ts、src/panels/terminal/useXterm.ts、src/panels/hooksConfig/useHooksConfig.ts、src/workspace/pageApis.ts、src/features/agentHistory/restoreSession.ts、grep 发现的轮询调用点适配文件 及 src/__tests__/ 下对应测试：
【FE-23】useAgentStatus 初始扫描（:307-344）引入 genRef（照 useFileTree 先例），setRows 前检查 generation 是否过期。
【FE-24】useXterm 的 readHistoryTitle promise（:431-448）加 isDisposedRef 守卫，卸载后忽略过期结果。
【FE-25】useHooksConfig（:156-173）：setLayer async IIFE 加 try/catch + toast；confirmDiscard 的 setTimeout id 存 ref，effect cleanup clearTimeout。
【FE-26】pageApis.ts 的 switchToPageAndFocus（:88-95）100ms×50 轮询支持 AbortSignal（契约：可选第二参）；grep 全部调用点（toast 点击/导航树行点击等）传 Controller，卸载/再次点击时 abort。
【FE-27】restoreSession.ts 的 waitFor（:34-43）接受 AbortSignal，循环前检查 signal.aborted；恢复编排四步共享一个 Controller，新恢复发起时 abort 旧的。
补 L2 测试：generation 过期不覆盖、卸载守卫、timeout 清理、abort 后停止轮询。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'backend-stability',
    prompt: `你负责 BE-07、BE-08（后端侧）、BE-09，只许改 src-tauri/src/notify/mod.rs、src-tauri/src/pty/spawn.rs、src-tauri/src/lib.rs、src-tauri/capabilities/default.json、src-tauri/src/state.rs、src-tauri/src/git/mod.rs：
【BE-07】notify/mod.rs 的 fs-event 已有 300ms debounce——补事件合并上限：单批 paths 超阈值（常量，建议 100）时合并为 Rescan 变体下发（不再逐路径）。agent-event（hooks/signal.rs）低频不节流——不改代码，评估结论注释记录（文档登记在 S19）。补 L1 测试。
【BE-08 后端】spawn.rs 新增 pty_kill_all() -> Result<u32, AppError>（契约）：遍历 sessions 全部 kill + join（超时语义同 S06/BE-06——3s 轮询 is_finished），返回成功 kill 数；lib.rs generate_handler! 注册；capabilities/default.json 加 allow-pty_kill_all。补 L1 测试。
【BE-09】state.rs:78 git_repo_cache 无上限无淘汰——改简易 LRU（容量 8，零新依赖手实现：HashMap + 访问顺序 Vec 或 LinkedHashMap 思路）；修正「目录切换时清除」失实注释；git/mod.rs 消费点适配。补 L1 测试（容量淘汰、命中复用）。
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 13 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-13.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
