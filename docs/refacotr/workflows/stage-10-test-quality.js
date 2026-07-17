// Stage 10 — 测试质量整改（TE-04、TE-08、TE-09、TE-10、TE-11、TE-12）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 4 个并行 agent，测试文件不重叠（对齐 refactoring-stages.md Stage 10 分工表）
// 本 Stage 只改测试与测试基建——生产代码零改动
// fix-loop 调用本 Stage 时 args.constraints 传："本 Stage 只改测试，禁止改生产代码"

export const meta = {
  name: 'stage10-test-quality',
  description: 'Stage10: 测试可信度整改（去 flaky/假阳性/mock 泄漏）',
  phases: [
    { title: '并行整改' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改测试文件与测试基建（setup.ts/helpers），不改生产代码；若发现断言假绿指向真实 bug，只报告不修复。代码注释用中文；完成后报告修改的测试文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7 的 4 条守卫测试（spawn.rs conpty_custom::tests）原样保留，不得改动。
背景文档：整改要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。`

phase('并行整改')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 TE-04 + TE-08（Rust 测试）：

【TE-04】src-tauri/tests/pty_integration_tests.rs 固定 sleep flaky
- 输出等待改 marker 轮询模式：循环读输出 + 短 sleep（如 50ms）+ 上限 10s 超时 panic
- 保持 SPAWN_LOCK 串行与 --test-threads=1 约束

【TE-08】src-tauri/src/notify/pool.rs 测试断言不足（pool.rs:289-331）
- make_test_watcher 线程改为真实监听 stop_rx（recv 收到信号即退出），不再空转
- LRU 淘汰/替换用例补 is_running() === false 断言

跑：cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（pty 集成 + notify 36+ 条全绿）
`, { label: 'rust-test-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-09 + TE-11（use-xterm 部分）（src/__tests__/use-xterm-output.test.ts、use-xterm-lifecycle.test.ts）：

【TE-09】条件断言假阳性（use-xterm-output.test.ts:811-834/984-1007/1197-1228 附近）
- if (writeCalls.length > 0) 式条件断言改为：先 expect(writeCalls.length).toBeGreaterThan(0)，再断言内容
- NF3 标题已在 Stage 6 随 TE-16 修正，此处不重复

【TE-11】debounce/定时器用例换 fake timers（use-xterm 两文件）
- 真实 setTimeout 等待统一改 vi.useFakeTimers() + advanceTimersByTime 驱动；afterEach 恢复真实定时器
- 保留行为语义：idle 2ms flush、max 16ms 强制 flush、DEC 2026 包裹断言不变

跑：npx vitest run use-xterm 全绿（108+ 条）
`, { label: 'xterm-test-fix-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-10 + TE-11（DOM 部分）（src/__tests__/ 下 DOM 类测试）：

【TE-10】waitFor 未设超时
- close-handler / editor-confirm / e2e-gating / explorer-* 等测试文件中 waitFor 调用全部加显式 { timeout: 2000~5000 }（按用例性质选值）
- grep 全 src/__tests__/ 排查，不止上述文件

【TE-11】debounce 测试真实 setTimeout（DOM 部分）
- use-file-tree.test.ts、explorer-notify/html-panel.test.tsx 中的 debounce/定时器用例统一 vi.useFakeTimers() + advanceTimersByTime

跑：npm test 全绿
`, { label: 'dom-test-fix-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-12：src/__tests__/setup.ts 全局 mock 泄漏风险。

- notify 全局 mock 处（setup.ts:58/61-64 附近）补警告注释：本 mock 会遮蔽真实实现，需要真实 startWatch/onFsEvent 的测试必须在本文件内 vi.mock 显式覆盖（参考 ipc-contract.test.ts 顶部做法）
- canvas spy 用 beforeAll/afterAll 包装（安装/恢复成对），防跨文件泄漏
- 注意：setup.ts 是全局基建，改动后必须全量 npm test 验证无隐性依赖

跑：npm test 全绿
`, { label: 'setup-agent' }),
])

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
6. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-10.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 10 的整改是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read/git status 逐条核实并给出证据。
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
