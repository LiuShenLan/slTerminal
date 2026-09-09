// =====================================================================
// Stage 03 · 后端加固（BE-01/02/03/04/06）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行；测试命令逐条串行。
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 03 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-03-backend-hardening',
  description: 'S03 后端加固：join_with_timeout 上提 + fs 游标 keyset + git 注释清理 + 守卫命令改写 + 基准两轮制',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
L1 定向红线（TQ-COV-06）：cargo 定向测试一律 \`cargo test --test lib_tests <filter> -- --test-threads=1\`，禁裸 filter/--lib。
Stage 特殊纪律：parking_lot 选型不回退（CP-005）；fs_read_dir 前后端契约（cursor opaque）形态不变（keyset 只改游标编码/解码内部实现，前端零改动）。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const agents = [
  { label: 'thread-join', prompt: `你负责 BE-01（先读 checklist BE-01 六段式原文）：
新建 src-tauri/src/thread_join.rs（crate 顶层共享件，app_dir/home 同形态——硬约束 #2 模块不穿透）：JOIN_TIMEOUT（3s）+ JOIN_POLL_INTERVAL（10ms）常量 + join_with_timeout 函数原样随迁（CleanupPlan/plan_cleanup_after_join_timeout 留 pty/reader.rs 不动）。
src-tauri/src/lib.rs 加 mod thread_join;；pty/reader.rs 删原定义、消费点改新名（KILL_JOIN_TIMEOUT 全改 JOIN_TIMEOUT 不留别名）；pty/spawn.rs:11、state.rs:11 import 改；notify/mod.rs:171-173+304-306 与 hooks/watcher.rs:136-138 三处生产裸 join 换装 join_with_timeout（超时分支 tracing::warn + detach 不再阻塞）。
测试：notify/mod.rs:968、hooks/watcher.rs:448 两处测试裸 join 改 join_with_timeout assert；spawn.rs:2156-2186 与 reader.rs:882-907 两测试组合并随迁 thread_join.rs 的 mod join_tests；pty/CLAUDE.md 按 checklist 文档同步节登记。
自验：cargo test --test lib_tests join -- --test-threads=1 绿；cargo test --test lib_tests notify -- --test-threads=1 绿；cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings 绿；rg "\\.join\\(\\)" src-tauri/src 仅命中 thread_join.rs。` },
  { label: 'fs-keyset', prompt: `你负责 BE-02（先读 checklist BE-02 六段式原文）：
src-tauri/src/fs/mod.rs 游标改 keyset——sort_key(e)=(is_dir?0:1, name.to_lowercase())；encode=base64("D\\0"+lower / "F\\0"+lower)；impl 起始索引改 entries.partition_point(|e| sort_key(e) <= key)；next_cursor=末条目键编码。
新增两用例（growth_no_dup_no_hole、beyond_end_empty_page）照抄 checklist 代码块；边界（大小写变体同名理论边界）登记 src-tauri/src/fs/CLAUDE.md + src/ipc/CLAUDE.md（游标语义节，按 checklist 文档同步节口径）。
纪律：前端 opaque 契约零改动（不改 src/ipc/fs.ts 签名、不改前端任何消费点）。
自验：cargo test --test lib_tests fs_read_dir -- --test-threads=1 绿（filter 按 checklist 测试同步节实际用例名调整）；cargo fmt --check 绿。` },
  { label: 'misc-bench', prompt: `你负责 BE-03 + BE-04 + BE-06（先读 checklist 三条目六段式原文）：
【BE-03】src-tauri/src/git/mod.rs:185 注释删 ignored（死分支清理的注释残留）。
【BE-04】src-tauri/src/CLAUDE.md:64（实读行号——review 报告 :53 已漂移，以实读为准）守卫行改写，命令写死为 rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs"（逐字照抄 checklist BE-04 步骤）。
【BE-06】src-tauri/src/agent_history/claude/scan.rs:869-878 基准断言改两轮制——首轮越门槛 → sleep 2s → 重采样 20 次 → 仍越才 panic，panic 消息附两轮中位；头注 :838-842 追加加固留痕。代码照抄 checklist BE-06 步骤。
自验：cargo test --test lib_tests scan_bench -- --test-threads=1 绿（两轮制后基准抗瞬时负载；串行自验无并行抢核）；实跑 rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs" 确认零命中（守卫命令自匹配消除——守卫行在 .md 不在 src/*.rs）。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行；纯 Rust Stage 收窄门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行——CP-007 基准对 CPU 负载敏感，本 Stage 正改基准断言，并行抢核必然干扰判定）：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo test 报告最终统计行原文。
（本 Stage 零前端文件改动，收窄不跑 tsc/eslint/npm test——收窄理由已登记 stages.md。）
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
