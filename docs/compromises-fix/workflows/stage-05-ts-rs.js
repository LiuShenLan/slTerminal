// =====================================================================
// Stage 05 DTO ts-rs 单源化（CP-024 单项 Stage；前置 = S02 完成）
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S05 行
//（本脚本 PREAMBLE_EXTRA 已内含「src/types/ 为生成物禁手改」纪律，与表值同源）
// =====================================================================

export const meta = {
  name: 'stage-05-ts-rs',
  description: 'S05 DTO ts-rs 单源化（CP-024）',
  phases: [
    { title: '修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
【Stage 特殊纪律】src/types/ 9 域文件为 ts-rs 生成物，禁手改——任何字段名/可选性/联合判别差异一律在 Rust 侧修（#[ts(optional)]/#[ts(rename = ...)]），不回改生成物。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md CP-024 条目（先读再动手，照抄步骤不另起方向）。`

// === Phase 1: 修复（单 agent 阶段化：依赖 → derive → 残面收口 → 契约测试同步）===
phase('修复')
const fixResult = await agent(`${PREAMBLE}

你负责 CP-024（IPC DTO ts-rs 单源化；58 文件 74 处 import 零改动是设计前提）：
照抄 checklist CP-024 阶段 0-4：
0. 确认 S02 已落地（cargo test --lib 可用）。
1. 依赖接线：Cargo.toml serde 行后加 ts-rs = "10"（含 CP-024 注释），cargo check 通过。
2. Rust derive 接线：对条目列出的 DTO 真身清单逐个加 TS derive + #[ts(export, export_to = "../src/types/<域>.ts")]（pty/fs/git/notify/agent/agentHistory/hooksConfig/backgroundTasks/planBalance 九域；同域多类型同路径合并导出）。serde-default 可选字段补 #[ts(optional)]。【先行扩 DTO 写死】hooks/claude/config.rs HookHandler 扩至 C13-3 全 16 字段矩阵（全部 Option + serde(default, skip_serializing_if)），SEC-05 校验语义不变；MatcherGroup.matcher 补 #[ts(optional)]。首次接线后跑 cargo test export_bindings -- --test-threads=1 生成 9 文件，逐文件与手写版 diff——只允许注释与等价写法差异。
3. 前端残面收口：生成物物理覆盖 9 域文件；新建 src/types/local.ts（TitleSource/BACKGROUND_TASK_IDS 常量族/ContextUsageSignal 别名）；hooksConfigGui 家族迁出 src/types/hooksConfigGui.ts；src/types/index.ts 重写（照抄 checklist 代码块）；GUI 类型原经 types/hooksConfig 导入者改指 types/hooksConfigGui（grep 逐一改）。
4. 契约测试同步：ipc-*-contract.test.ts 六件与 helpers/ipc-contract.ts 零改动；新增漂移守卫操作指令（cargo test export_bindings + git diff --exit-code -- src/types）登记进 src/types/CLAUDE.md。
测试：Rust 侧补 hook_handler_full_matrix_roundtrip（16 字段逐字段断言）；其余既有用例零改动策略（红了才查形状不等价，属缺陷须报告）。
文档：src/types/CLAUDE.md 改写单源化口径；根 CLAUDE.md 硬约束 #4 改写；src/ipc/CLAUDE.md:94-95 mockIPC 红线补句；域对照表改生成关系对照。
自查：条目「验证」段 1-8 全部满足（含 ls src/types 清单、74 处 import 计数、git status --porcelain -- src/types 为空）。
完成后报告：修改文件清单 + 生成物与手写版 diff 中发现的非等价差异（应为零，有则说明 Rust 侧修法）。`, { label: 'cp024-ts-rs' })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
2. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
3. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
4. npx tsc --noEmit
5. npx eslint src/
6. npm test
7. npm run test:l3
8. node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { fixResult, testResult, verifyResult }
