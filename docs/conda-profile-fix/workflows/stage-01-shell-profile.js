// =====================================================================
// Stage 01 — 删 -NoProfile 恢复 profile 加载（B17 修复）+ 防复发测试
// =====================================================================
// 改动项（逐 ID 对照 docs/conda-profile-fix/checklist.md 原文）：
//   B17-FIX、TE-B17（同文件 src-tauri/src/pty/shell.rs，单 agent）
// 跨边界契约（写死）：本 Stage 不改任何 IPC 命令 / DTO / 前端文件；
//   e2e-tests/terminal.e2e.ts:412 的 -NoProfile 不动（一次性辅助命令，
//   非本故障链路）。
// fix-loop 调用本 Stage 时 args.constraints 传空串（无 Stage 特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage01-shell-profile',
  description: 'Stage 01: 删除 PowerShell spawn 的 -NoProfile（B17 修复）+ 防复发测试',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/conda-profile-fix/checklist.md 对应 ID 条目（先读再动手），代码块一律照抄 checklist，禁止自行设计。
契约：不改任何 IPC 命令 / DTO / 前端文件；e2e-tests/terminal.e2e.ts:412 的 -NoProfile 不动。`

// === Phase 1: 并行重构（单 agent 单文件，无共享资源冲突；只做编译级自查）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'shell-fix',
    prompt: `你负责 B17-FIX 与 TE-B17（同一文件 src-tauri/src/pty/shell.rs）：

【B17-FIX】先 Read docs/conda-profile-fix/checklist.md 的 B17-FIX 条目，按其「修复步骤」1-4 逐步执行（现状摘录在该条目中，先核对行号附近代码与摘录一致再动手，漂移以现状为准并报告）：
1. 模块头注释（L4-5 之后）追加 checklist 给出的两行禁止说明
2. resolve_shell 文档注释（L152 附近）整行替换为 checklist 给出的新行
3. build_pwsh_command（L217-229 附近）：整体替换为 checklist 给出的新版本（删 cmd.arg 的 -NoProfile 行，文档注释改写）
4. build_pwsh_info（L231-244 附近）：文档注释追加 B17 禁止说明行，args 删 -NoProfile 的 to_string 行

【TE-B17】在同一文件 #[cfg(test)] mod tests 内（PTY-06 区块之后）追加 checklist TE-B17 条目给出的测试函数 test_pwsh_args_no_noprofile_b17（照抄代码块；fake_exe / set_test_path 辅助函数已存在，直接复用）。

完成后编译级自查：cargo check --manifest-path src-tauri/Cargo.toml（不跑测试——测试由全量测试 agent 统一单点跑）。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。cargo test 报告末尾须附全部 test result 行的 passed 总数汇总（供文档 Stage 计数核对用）。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/conda-profile-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
