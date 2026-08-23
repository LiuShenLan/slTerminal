// Stage 08：Rust 可测性 + L1 修复补测（TQ-COV-01, TQ-COV-03, TQ-COV-04, TQ-COV-05, TQ-COV-06, TQ-L1-01, TQ-L1-03, TQ-L1-05）
// fix-loop 调用时 args.constraints 传空
export const meta = {
  name: 'stage-08-rust',
  description: 'Stage 08：Rust 可测性抽取与 L1 覆盖补写（8 项）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。
【并行纪律】你不跑 cargo test（target 锁 + ConPTY 串行红线）——编译级检查只用 cargo test --no-run --manifest-path src-tauri/Cargo.toml；全量测试由独立测试 agent 单点跑。cargo 系命令共享 target 目录锁，排队属正常勿中止。`

phase('并行修复')
const parallelAgents = [
  { label: 'rust-panic-hook', prompt: '你负责 TQ-COV-01 + TQ-L1-01：main.rs（19 行零覆盖）panic hook 逻辑抽为 lib.rs 的 pub fn install_panic_hook() + fn write_crash_log(dir: &Path, message: &str) -> io::Result<()>（目录参数化），main.rs 改两行调用；lib.rs 测试模块补 2 用例（checklist TQ-COV-01 步骤含可照抄代码）；settings.rs 按 checklist TQ-L1-01 加测试注释（仅注释，登记 save_settings 无重试的假设）。触碰：src-tauri/src/main.rs, src-tauri/src/lib.rs, src-tauri/src/settings.rs（仅测试注释）。' },
  { label: 'rust-pty-tests', prompt: '你负责 TQ-COV-03 + TQ-L1-03 + TQ-L1-05：spawn.rs 测试模块补 ensure_pty_capacity（:1067-1075 纯函数）与 join_with_timeout（:1464-1468，注入短 timeout 测 false 分支）用例；容量超限 kill 清理（:1307-1315）I/O 不可抽——登记豁免由 Stage 10 处理；pty_integration_tests.rs 文件头加 #![cfg(windows)]（TQ-L1-05）；spawn.rs 集成测试注释说明 SPAWN_LOCK 不经 AppState 的理由（TQ-L1-03）；pty/CLAUDE.md 按 checklist 同步。触碰：src-tauri/src/pty/spawn.rs（仅测试模块+注释）, src-tauri/tests/pty_integration_tests.rs, src-tauri/src/pty/CLAUDE.md。' },
  { label: 'rust-hooks-tests', prompt: '你负责 TQ-COV-04：hooks 信号链补测——signal.rs process_signal_file_with（:79-82，注入 emit）补超限/读失败/emit 失败仍删除三分支用例；watcher.rs run_one_tick（:206-214，注入 process fn）补目录重建/停止信号用例。均只改两文件的 #[cfg(test)] 测试模块，生产代码不动。触碰：src-tauri/src/hooks/signal.rs, src-tauri/src/hooks/watcher.rs。' },
  { label: 'rust-audit-log', prompt: '你负责 TQ-COV-05：Cargo.toml 新增 [dev-dependencies] tracing-test = "0.2"（注意：现存 [dependencies] 在 :20-67，无 dev-dependencies 段，新建段）；config.rs 测试模块补 #[tracing_test::traced_test] 两例——Layer::User 写入触发 logs_contain("hooks user 层配置写入")、其他 Layer 不触发（SEC-17 审计日志在 config.rs:294-297）。写盘函数名以 config.rs 既有 config_write_sync_* 用例调用的真实函数为准。触碰：src-tauri/Cargo.toml, src-tauri/src/hooks/claude/config.rs（仅测试模块）。' },
  { label: 'rust-git-coverage', prompt: '你负责 TQ-COV-06：先跑 cargo llvm-cov --html --manifest-path src-tauri/Cargo.toml 定位 git/mod.rs 未执行函数清单（报告列出清单）；死函数删除（git/mod.rs 无内联 #[cfg(test)]，测试全在 tests/git_*_tests.rs）；活函数经 tests/git_*_tests.rs 补测，目标 git/mod.rs 函数覆盖 ≥80%。llvm-cov 不可用时降级为逐函数对照 tests/ 命中情况人工判定并报告。触碰：src-tauri/src/git/mod.rs（死函数删除时）, src-tauri/tests/git_*_tests.rs。' },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令串行执行（cargo 共享 target 锁）：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程——ConPTY 并发 spawn 死锁红线；耗时长勿中止）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
cargo test 报告总用例数与失败数。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 08 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
TQ-COV-06 须核对修复 agent 报告的未执行函数清单与处置（死删/活测）——无清单而直接补测判 partial。
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
