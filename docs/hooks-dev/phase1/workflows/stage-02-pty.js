// =====================================================================
// Stage 02 Workflow: PTY 注入 SLTERM_PANEL_ID
// =====================================================================
// 契约头部：
//   - 位置：src-tauri/src/pty/spawn.rs
//   - 变量名：SLTERM_PANEL_ID
//   - 值：request.panel_id
//   - Windows 路径：extra_envs Vec 追加
//   - 非 Windows fallback：cmd.env 追加
// =====================================================================

export const meta = {
  name: 'stage02-pty',
  description: 'PTY 注入 SLTERM_PANEL_ID 环境变量',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/hooks-dev/phase1/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'be-pty-env',
    prompt: `你负责 P1-PTY-01、P1-PTY-02：
修改 src-tauri/src/pty/spawn.rs：
1. 在 extra_envs Vec（当前 790-794 行附近）追加第 4 项 ("SLTERM_PANEL_ID", request.panel_id)，与 COLORTERM/TERM/TERM_PROGRAM 同一时机注入。
2. 在非 Windows fallback 路径（当前 854-856 行附近）的 cmd.env 链上同步追加 "SLTERM_PANEL_ID" = request.panel_id。
注意：request.panel_id 在函数内已绑定到 panel_id 变量（见 784 行附近），直接复用。不加 shell 类型判断。完成后跑 cargo check。`
  },
  {
    label: 'be-pty-test',
    prompt: `你负责 P1-PTY-03：
在 src-tauri/tests/pty_integration_tests.rs 或 src-tauri/src/pty/spawn.rs 的 #[cfg(test)] 中新增测试，验证 pty_spawn 后子进程环境变量含 SLTERM_PANEL_ID 且值等于 request.panel_id。由于 PTY 测试必须串行，使用项目已有的 SPAWN_LOCK 模式。测试名建议 pty_env_injects_slterm_panel_id。完成后单独运行 cargo test --manifest-path src-tauri/Cargo.toml pty_env_injects -- --test-threads=1 验证。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml pty_env_injects -- --test-threads=1
3. cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase1/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
