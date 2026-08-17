// =====================================================================
// stage-04-async-root.js — S04 命令异步化 + 沙箱加固（BE-04、SEC-14、FE-04）
// =====================================================================
// 跨边界契约（写死）：
//   set_project_root / notify_watch 改 async fn，前端 invoke 调用签名不变。
//   set_project_root 失败语义 = 返回 Err 且清空旧 root（防沙箱误放行旧路径）。
//   前端失败仍完成切换（D7/DBG-9 契约不动）+ toast 告警
//   「项目根路径设置失败，文件操作可能被拒绝」（toast.show("warning", ...)）。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage04-async-root',
  description: 'Stage 04: set_project_root/notify_watch 异步化 + 失败清空旧 root + 前端失败 toast 降级（BE-04、SEC-14、FE-04）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S04 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：set_project_root/notify_watch 改 async fn（前端签名不变）；set_project_root 失败 = Err + 清空旧 root；前端失败仍切换 + toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝")。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-async',
    prompt: `你负责 BE-04、SEC-14，只许改 src-tauri/src/state.rs、src-tauri/src/notify/mod.rs：
【BE-04】set_project_root（state.rs:186）与 notify_watch（notify/mod.rs:287）为同步命令、主线程阻塞 I/O。改 async fn + tokio::task::spawn_blocking 包裹阻塞段（dunce::canonicalize / FileWatcher::start）。前端 invoke 签名不变（Promise 语义已具备）。
【SEC-14】set_project_root 失败路径（canonicalize 失败/目录不可读）：返回 Err 且清空旧 root（project_root 写锁置 None，防沙箱误放行旧路径）。
补 L1 测试：失败清空旧 root（构造不存在路径调用，断言 Err 且 project_root 为 None）；异步化后成功路径行为不变。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'frontend-toast',
    prompt: `你负责 FE-04，只许改 src/App.tsx、src/stores/projects.ts、src/workspace/Workspace.tsx、src/__tests__/workspace-switch-order.test.tsx：
setProjectRoot 失败当前仅 console.error（App.tsx:84-87、projects.ts:154、Workspace.tsx:216 三处调用点）。按 D7：三处失败时 toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝")，仍完成切换（DBG-9 契约不动——禁止改为阻止切换）。
workspace-switch-order.test.tsx 14 用例补断言：mock toast，断言失败路径 toast 被调用且切换仍发生（原有顺序断言不动）。
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
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
