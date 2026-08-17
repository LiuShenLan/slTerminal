// =====================================================================
// stage-06-reader-batch.js — S06 PTY reader 批处理（BE-05/06/12、FE-18）
// =====================================================================
// 高风险 Stage：终端核心数据路径，流畅度回归只能人工实测兜底（stages.md S06 标注）。
// 跨边界契约（写死）：
//   微批策略 = 「读到即续读」非定时器——read 成功后非阻塞 try_read 续读，
//   累积至 64KB 或无可读数据再一次 Channel::send + 一次 ring_buffer_append
//   （BE-12 随动：append 调用点仅批量一处）。
//   前端 usePtyOutput 直接写阈值 64B → 256B（2ms 空闲/16ms 强制不变）。
// fix-loop 调用约定：args.testCommands 传本脚本下方 6 条门禁；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage06-reader-batch',
  description: 'Stage 06: reader 微批处理降 IPC 频次 + kill 可靠性加固 + 前端输出 dispose 与阈值上调（BE-05/06/12、FE-18）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；WebGL 检测逻辑（failIfMajorPerformanceCaveat 相关）同样禁止触碰。
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S06 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：微批 = try_read 续读至 64KB 或无可读再 send/append（非定时器）；append 调用点仅批量一处；前端直接写阈值 64B→256B，2ms 空闲/16ms 强制不变。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'reader-backend',
    prompt: `你负责 BE-05、BE-06、BE-12，只许改 src-tauri/src/pty/reader.rs、src-tauri/src/pty/spawn.rs、src-tauri/src/state.rs：
【BE-05】reader.rs 的 reader_loop（:83-131）当前每轮 read（READER_BUF_SIZE=16384）成功即 Channel send。引入微批：read 成功后非阻塞 try_read 续读，累积至 64KB 或无可读数据再一次 Channel::send（契约：「读到即续读」，禁止引入固定延迟定时器）。reader.rs 注释更新 I/O 编排说明（DOC-01 豁免项 1 变动，豁免表同步在 S19）。
【BE-12】ring buffer 随微批改为批量 append——合并后一次 ring_buffer_append；验收 = reader.rs 中 append 调用点仅批量一处；state.rs 的 ring_buffer_append 签名如需微调允许改 state.rs，不引入无锁结构。
【BE-06】spawn.rs 的 pty_kill（:1284-1299）：当前 let _ = child.kill() 丢弃结果、handle.join() 无超时。修复：检查 kill 返回值，失败 tracing::warn! 并继续；join 改带超时的轮询 is_finished（3s 超时后放弃 join 记 warn，线程随 Drop 兜底）。可测部分抽纯函数补 L1 测试。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'output-frontend',
    prompt: `你负责 FE-18，只许改 src/panels/terminal/usePtyOutput.ts、src/panels/terminal/useXterm.ts 及对应 L2 测试（src/__tests__/ 下）、L3 测试（test/terminal/ 下，若阈值被断言）：
【FE-18】usePtyOutput 的 idleTimerRef/maxTimerRef 组件卸载未清理（:83-84）、useXterm cleanup（:485-503）未调 cancelPendingFlush。修复：usePtyOutput 暴露 dispose()（清 idle/max 双定时器 + 清 pending buffer）；useXterm 主 effect cleanup 调用 dispose()。直接写阈值 64B → 256B（契约：后端已合并小写，2ms 空闲/16ms 强制参数不变）。
补 L2 测试：卸载后定时器不再触发（fake timers 断言）；阈值边界行为（≤256B 直接写、>256B 走合并）。L3 用例若断言旧阈值同步调整。
只做 npx tsc --noEmit 编译级检查，禁止 npm test / npm run test:l3。`
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
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
