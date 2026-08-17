// =====================================================================
// stage-14-deadcode.js — S14 死代码清理（FE-35、BE-17/20）
// =====================================================================
// 跨边界契约：无。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；
//   args.constraints 传「每个删除点先 grep 消费方（含 src/__tests__/ 与
//   e2e-tests/）确认零消费再删」。
// =====================================================================

export const meta = {
  name: 'stage14-deadcode',
  description: 'Stage 14: 删除无消费 barrel/常量/setFocus + 移除 allow(dead_code) + 测试 cfg 改运行时分支（FE-35、BE-17/20）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S14 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 特殊纪律：每个删除点先 grep 消费方（含 src/__tests__/ 与 e2e-tests/）确认零消费再删；删除动作逐点列出 grep 证据于报告。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'deadcode-fe',
    prompt: `你负责 FE-35，只许删除/修改下列文件 + grep 发现的消费方适配：
【删 4 barrel】src/features/index.ts、src/panels/index.ts、src/features/agentHistory/index.ts、src/features/commit/index.ts——每个先 grep 消费方（含 src/__tests__/、e2e-tests/）确认零消费再删；有消费则改直接导入后删。
【未用常量】src/panelRegistry.ts:20-49 的 PANEL_GIT_SHOW/PANEL_DIFF/PANEL_HOOKS_CONFIG/terminalTabConfig——逐一 grep 后定：有消费改常量引用保留，零消费删除。
【冗余 re-export】src/workspace/index.ts、src/panels/terminal/index.ts:2 清理未消费 re-export（grep 判定）。
【ping()】src/ipc/index.ts:19 的 ping 仅测试用——保留但注释注明「测试专用」。
【setFocus()】src/ipc/window.ts:44 的 setFocus 预留无消费——grep 确认后删除。
每个删除点在报告中给出 grep 证据。只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'deadcode-be',
    prompt: `你负责 BE-20，只许改 src-tauri/src/hooks/signal.rs：
signal.rs:9 的模块级 #![allow(dead_code)] 已过时（API 已被 watcher.rs 消费）。移除该属性；cargo clippy --manifest-path src-tauri/Cargo.toml 验证零 dead_code 警告——若暴露真 dead_code（无消费函数），grep 确认后删除并在报告说明。
只做 cargo check / clippy 编译级检查，禁止 cargo test。`
  },
  {
    label: 'test-cfg',
    prompt: `你负责 BE-17，只许改 src-tauri/src/lib.rs、src-tauri/src/fs/mod.rs、src-tauri/src/settings.rs、src-tauri/src/agent_history/claude/ops.rs：
按 D5，测试代码 #[cfg(windows)] 改运行时 cfg!(windows) 分支（6 处：lib.rs:137、fs/mod.rs:586、settings.rs:438、agent_history/claude/ops.rs:429,448,477——以 grep 实际为准）。无法运行时区分且必须 Windows 特权才能建的（如 symlink 特权测试）保留 cfg 并在报告中逐处列出（豁免登记在 S19，本 Stage 只在代码注释标注豁免理由）。
只做 cargo check 编译级检查，禁止 cargo test。`
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
逐项检查 Stage 14 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-14.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
