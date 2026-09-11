// =====================================================================
// Stage 03 后端加固（BE-02/06、BE-03、BE-05/01）
// 串行形态：用户并发限制约束——agents for 循环顺序 await，不用 parallel()；
// 全量测试命令逐条串行（CP-007 基准负载敏感）。
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-03-backend-hardening',
  description: 'Stage 03 后端加固：notify panic 回传+Drop 委托/游标解码补测/spawn 注释撞号/豁免计数',
  phases: [
    { title: '串行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点唯一真值源 = docs/compromises-fix-review2-fix/checklist.md 对应 ID 条目（先读再动手）——条目为六段式（位置/现状/修复步骤含可照抄代码块/测试同步/文档同步/验证），修复步骤段的代码块照抄适配，不自行设计；每项完成后跑条目「验证」段断言自查。
L1 定向红线：cargo test --test lib_tests <filter> -- --test-threads=1，禁 --lib/裸 filter（0xc0000139）。`

// === Phase 1: 串行修复（用户并发限制——顺序 await，禁 parallel）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'notify-harness',
    prompt: `你负责 BE-02、BE-06（同文件，先读 checklist 对应条目再动手）：
【BE-02】src-tauri/src/notify/mod.rs LoopHarness（:933-981）加 done 通道恢复 panic 检测——照 settings.rs:518-552 mpsc 回传先例（checklist 内 Rust 代码块照抄：结构加 done_rx 字段 / start() 建通道 + spawn 闭包 event_loop 返回后 send / shutdown() assert 后追加 recv().expect(...)）。
【BE-06】同文件 :300-313 FileWatcher::drop 委托 self.stop()（checklist 代码块照抄；hooks/watcher.rs:146-150 为同形态先例）。
【文档同步】src-tauri/src/notify/CLAUDE.md 测试模式节补一句 LoopHarness panic 回传机制（checklist BE-02 文档同步段口径）。
触碰文件仅限：src-tauri/src/notify/mod.rs、src-tauri/src/notify/CLAUDE.md。
自查：cargo test --test lib_tests notify -- --test-threads=1 绿；cargo fmt --check 绿。`,
  },
  {
    label: 'fs-cursor-tests',
    prompt: `你负责 BE-03（先读 checklist 对应条目再动手）：
【BE-03】src-tauri/src/fs/mod.rs 内嵌 read_dir_tests 补 3 例游标解码失败用例（非法 base64 / 非法 UTF-8 / 坏 tag+无 NUL 两形态）——直接调私有 decode_page_cursor，断言 AppError::Validation 变体匹配（不锁文案）；编码构造用 base64::engine::general_purpose::STANDARD（与 :73/:81 同引擎）。
触碰文件仅限：src-tauri/src/fs/mod.rs。
自查：cargo test --test lib_tests read_dir -- --test-threads=1 绿（含新 3 例）；cargo fmt --check 绿。`,
  },
  {
    label: 'spawn-exemptions',
    prompt: `你负责 BE-05、BE-01（先读 checklist 对应条目再动手）：
【BE-05】src-tauri/src/pty/spawn.rs:2155 注释改写（checklist 修复步骤段措辞照抄——历史轮次注记口径，消解与本计划 BE-06 编号撞车）。
【BE-01】.claude/test-exemptions.md:14（CP-011 行）「join_with_timeout 4 例」改「3 例」（与 :24 行口径对齐）；:24 行不动。
触碰文件仅限：src-tauri/src/pty/spawn.rs、.claude/test-exemptions.md。
自查：rg "BE-06" src-tauri/src/pty/spawn.rs 零命中；cargo fmt --check 绿。`,
  },
]
const refactorResults = []
for (const a of agentsSeq) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行执行，禁并行——CP-007 基准负载敏感）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行，禁止并行**（CP-007 基准负载敏感，并行抢核曾致误红）：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --test lib_tests notify -- --test-threads=1
4. cargo test --test lib_tests read_dir -- --test-threads=1
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
