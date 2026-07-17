// Stage 3 — spawn.rs 低危打磨 + reader.rs（BE-07、BE-08、BE-09、BE-10、BE-16、BE-13）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 2 个并行 agent，文件不重叠（spawn.rs / reader.rs）

export const meta = {
  name: 'stage3-spawn-hardening',
  description: 'Stage3: ConPTY 错误处理加固 + 启动剥离跨边界修复',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何情况下不得修改 ConPTY flags。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。
测试约束：cargo test 必须 --test-threads=1。
并行测试纪律：本 Stage 两个 agent 并行——禁止在本阶段执行会 spawn PTY 的测试（pty_integration_tests、conpty_custom），跨进程并发 spawn ConPTY 会死锁；这类测试留全量测试阶段统一执行。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 src-tauri/src/pty/spawn.rs 的 5 项改动：

【BE-07】RawChild::try_wait/wait 未查 WaitForSingleObject 错误值（spawn.rs:278-304）
- 仅 WAIT_OBJECT_0 视为已退出；WAIT_TIMEOUT 视为未退出；其余值（含 WAIT_FAILED）返回 std::io::Error

【BE-08】clone_killer 失败 panic（spawn.rs:259-269）
- try_clone 失败不再 .expect panic；按 portable_pty::ChildKiller trait 签名允许范围内改为返回错误或用 Killer 内部记录错误延后上报（若 trait 签名不允许返回 Result，则在注释说明并降级为 log::error + 返回无效 killer，不得 panic）

【BE-09】锁中毒 expect panic（spawn.rs:199-312 多处）
- ConPtyInner/RawChild 中 Mutex 锁的 .expect("... poisoned") 改为错误返回（io::Error::other）

【BE-10】build_env_block HashMap 顺序不定（spawn.rs:101-120）
- 改 Vec<(String, String)> 保持 std::env::vars() 原始顺序，仅对 extra_envs 做覆盖

【BE-16】unsafe 块补 SAFETY 注释（spawn.rs:130-433）
- CreatePseudoConsole / CreateProcessW / ClosePseudoConsole / WaitForSingleObject / InitializeProcThreadAttributeList / UpdateProcThreadAttribute / TerminateProcess 等每个 unsafe 块补 // SAFETY: 注释（一句说明前置条件为何成立）

改完跑：cargo test --manifest-path src-tauri/Cargo.toml --no-run（仅编译验证，不执行——conpty_custom 测试会 spawn PTY，与 reader-agent 并发有死锁风险，留全量阶段统一跑）+ cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings。
`, { label: 'spawn-hardening-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src-tauri/src/pty/reader.rs 的 1 项改动：

【BE-13】启动序列剥离跨 16KB 边界失效（reader.rs:85-94）
- apply_startup_strip 只要处理过一次数据就置 startup_drained=true，导致跨缓冲区边界的启动序列（如本轮末尾部分 OSC 标题 + 下轮清屏/归位）不被剥离
- 修复：本轮数据全部被识别为启动序列（返回 None）时不置 drained，继续保持剥离状态；仅当本轮出现真实非启动输出后才置 drained
- 同步更新 apply_startup_strip 的纯函数签名（如需要）与全部相关测试

修改后必须跑 reader 全部 strip 相关测试（pty/CLAUDE.md 修改注意事项 #5）——reader 测试为纯函数测试、不 spawn PTY，可安全执行：
cargo test --manifest-path src-tauri/Cargo.toml reader -- --test-threads=1（28 条全绿，含 strip_startup_with_16k_boundary / strip_startup_with_large_payload / reader_buf_size_is_16k）
`, { label: 'reader-agent' }),
])

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 3 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
