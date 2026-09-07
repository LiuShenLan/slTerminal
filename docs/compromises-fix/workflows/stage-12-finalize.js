// =====================================================================
// Stage 12 收尾（CP-023 + 销项总扫 + 全量回归）
// fix-loop constraints：本 Stage 无特殊纪律（省略 args.constraints）
// =====================================================================

export const meta = {
  name: 'stage-12-finalize',
  description: 'S12 收尾（CP-023 + compromises.md 销项总扫 + 全量回归）',
  phases: [
    { title: '并行收尾' },
    { title: '全量回归' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md S12 条目（先读再动手，照抄步骤不另起方向）。`

// === Phase 1: 并行收尾（A1 覆盖率口径 ∥ A2 销项总扫，文件零重叠）===
phase('并行收尾')
const parallelAgents = [
  {
    label: 'cp023-coverage',
    prompt: `你负责 CP-023（Rust 行覆盖收尾：生产代码口径重测 + pty 纯逻辑抽取收敛）：
照抄 checklist CP-023 步骤 1-5：
1. 约 40 个文件级 #[cfg(test)] mod <领域>_tests 模块逐个加 #[coverage(off)]（清单以 rg 实查为准；模块内子项不动；完成后 cfg(test) 计数与改造前相同）。
2. 基线重测：cargo llvm-cov -- --test-threads=1（全绿才计数字），记录 P_new。
3. 目标重定（写死分支）：P_new ≥ 90% → 删 test-exemptions.md:25 + 脚注区补销记；< 90% → 该行改写为生产口径 + 残余缺口逐项登记，继续步骤 4。
4. pty 缺口收敛：llvm-cov --html 逐文件扫 pty/{spawn,reader,conpty_api,shell,win_build}.rs 未覆盖区；判定/计算分支照 build_cmdline 先例抽纯函数补测；确属 Win32 组合无法纯化的逐条进 pty/CLAUDE.md 豁免表 + test-exemptions 汇总行（禁止笼统「收尾」登记）。
5. main.rs 3 行：test-exemptions.md 豁免表新增行（lib.rs run() 行之后）；不给 fn main 加 #[coverage(off)]。
测试：抽取的纯函数按「对象_行为_场景」命名补用例；抽取零行为变化（纯机械移动）。
自查：条目「验证」段全部满足（含 llvm-cov 摘要 = 登记值 ±0.1pp）。
完成后报告：修改文件清单 + P_new 实测值 + 走的分支。`,
  },
  {
    label: 's12-compromises-sweep',
    prompt: `你负责 compromises.md 销项总扫（不改代码）：
照抄 checklist「S12 销项总扫」：
1. docs/compromises.md 44 项逐条核对销项勾选 + 修复注记（格式照 CP-015 先例）；缺勾选/缺注记的补上（对照各 Stage commit 与代码现状）。
2. 转休眠/翻案项口径一致性：CP-001 机检未触发、CP-003 分支 B、CP-007 指纹分支、CP-029 环境出口、CP-031 同态维持、CP-033 分支 B2 等——按实际走的分支核对登记口径一致。
3. 文档与代码一致性抽查：抽 ≥5 个已销项的登记点原文对照代码现状（含 ADR 行、模块 CLAUDE.md 节、test-exemptions 行），失实即记录。
完成后报告：逐条销项状态表 + 抽查 5 条的证据 + 发现的失实点（若有）。`,
  },
]
const sweepResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量回归（四级测试 + 四门静态门禁 + knip + 构建）===
phase('全量回归')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量回归。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
7. npm run test:l3
8. npm run e2e
9. npx knip --production
10. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full regression' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 12 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-12.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
销项总扫报告（逐条状态表与抽查证据）：
---
${sweepResults[1] ?? '（未返回）'}
---
以下为全量回归执行结果，测试类断言据此判定（无需重跑）：
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

return { sweepResults, testResult, verifyResult }
