// =====================================================================
// stage-05-watcher.js — S05 watcher 排除与生命周期（BE-02/10/11、SEC-08）
// =====================================================================
// 跨边界契约（写死）：
//   新命令 notify_stop_watch(path: String) -> Result<(), AppError>；
//   前端 wrapper stopWatch(path: string): Promise<void>（src/ipc/notify.ts）。
//   排除目录常量 WATCH_EXCLUDE_DIRS = ["node_modules", "target", ".venv", "venv",
//   "dist", ".git", "__pycache__"]（D8 定稿，仅事件侧过滤——notify 不支持目录级
//   排除，watcher 仍注册全树）。
//   本 Stage 后命令数 33（S03 后 32 + notify_stop_watch）。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage05-watcher',
  description: 'Stage 05: watcher 事件侧排除大目录 + symlink 过滤 + notify_stop_watch 生命周期命令 + 池容量 8（BE-02/10/11、SEC-08）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S05 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：notify_stop_watch(path: String) -> Result<(), AppError>；前端 stopWatch(path: string): Promise<void>；WATCH_EXCLUDE_DIRS 七元素 = node_modules/target/.venv/venv/dist/.git/__pycache__（仅事件侧过滤）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'watcher-backend',
    prompt: `你负责 BE-02、SEC-08、BE-10、BE-11，只许改 src-tauri/src/notify/mod.rs、src-tauri/src/notify/pool.rs、src-tauri/src/lib.rs、src-tauri/capabilities/default.json：
【BE-02】notify/mod.rs:103-110 事件循环中过滤路径分量含 WATCH_EXCLUDE_DIRS（契约七元素）任一元素的事件（watcher 仍注册全树——notify 不支持目录级排除，过滤在事件侧）；need_rescan 分支不受影响。补 L1 测试（构造含 node_modules 分量路径断言被过滤）。
【SEC-08】事件循环中对事件路径做 symlink 检查（symlink_metadata），命中 symlink 的路径不 emit；need_rescan 分支不受影响（只发 watch root）。补 L1 测试（symlink 创建失败 skip 并注释注明——Windows 需管理员/developer mode）。
【BE-10】pool.rs 加 remove(path) 能力；新增命令 notify_stop_watch(path: String) -> Result<(), AppError>（pool.remove + stop）；lib.rs 的 generate_handler! 注册；capabilities/default.json 加 allow-notify_stop_watch。补 L1 测试。
【BE-11】LruWatcherPool 容量 5 → 常量 WATCHER_POOL_CAPACITY = 8 并注释理由（覆盖多项目快速切换；pause/resume 既定机制保留）。补/调 L1 测试。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'watcher-frontend',
    prompt: `你负责 BE-10（前端侧），只许改 src/ipc/notify.ts、src/features/explorer/ExplorerPanel.tsx 及对应 L2 测试文件（src/__tests__/ 下）：
【BE-10 前端】src/ipc/notify.ts 加 wrapper stopWatch(path: string): Promise<void>（invoke notify_stop_watch）；ExplorerPanel.tsx 项目移除/切换路径调用 stopWatch（当前仅 startWatch，:183-189）。补 L2 测试：mock invoke，断言项目移除/切换时 notify_stop_watch 被调用且 startWatch 既有行为不回归。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
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
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
