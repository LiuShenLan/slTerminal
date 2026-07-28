// =====================================================================
// Stage 05 — 文档同步
// =====================================================================
// 编排：单 agent 文档对账 → 全量回归 → 逐项验证
//
// 纪律（写死）: 文档必须反映 Stage 01-04 完成后的最终真实代码形态——先 Read
//   代码核实再写文档，禁凭计划文档想象（文档不撒谎）。
//
// fix-loop args.constraints 应传值（单一出处，勿手写第三份）:
//   本 Stage 特殊纪律：只改文档文件（6 个），不改任何代码/测试文件；
//   发现文档与代码不一致且代码有疑，报告不改代码。
// =====================================================================

export const meta = {
  name: 'stage-05-docs',
  description: 'Stage 05 文档同步——CLAUDE.md/test-inventory 对账最终代码',
  phases: [
    { title: '文档对账' },
    { title: '全量回归' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点先读 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目 + 「跨边界契约」段（契约取值唯一真值源，禁止各自推断）。
本 Stage 特殊纪律：只改文档文件（6 个），不改任何代码/测试文件；发现文档与代码不一致且代码有疑，报告不改代码。
文档纪律：先 Read Stage 01-04 完成后的最终代码核实再写，禁凭计划文档想象（文档不撒谎）。`

// === Phase 1: 文档对账（A：6 个文档文件）===
phase('文档对账')
const fixResult = await agent(`${PREAMBLE}

你负责 PF2-DOC-02、PF2-DOC-03、PF2-DOC-04（+ PF2-DOC-01 核对——已由 Stage 03 完成，仅核对 drift）：

【PF2-DOC-02】四 CLAUDE.md 同步（先 Read 最终代码核实）
1. src/features/sideViews/CLAUDE.md：useAgentStatus 行语义改写——行 = 运行中 claude 会话（claudeSession 双通道建行/三通道删行/初始扫描只建活会话+携 transcriptPath 主动拉 usage/reconcile 对账）；AgentStatusRow 用量口径四字段（(input + cacheRead + cacheCreation) / 200_000，output 不计占用保留为信息字段）
2. src/panels/CLAUDE.md：TerminalRegistry 增 claudeSession/setClaudeSession/sessionChange 描述；TabTitleRegistry match 首 token 语义；顺带修正测试模式表格 TabTitleRegistry.test.ts 驼峰误写 → kebab-case tab-title-registry.test.ts（Glob 实证文件名）
3. src/ipc/CLAUDE.md：notification 行改写——sendClickableNotification → sendToastNotification(title, {body})（Tauri 原生通道，无 onClick；未打包 Win32 WebView2 无 AUMID 平台限制结论一并记录；banner 人工实测结果若已知一并写入——查会话上下文/git log，未知则不写实测结论、不编造）
4. e2e-tests/CLAUDE.md：L4 用例表 agent-status 段改写（静态行反转：纯 shell 无行；动态四态流程：首个信号即建行；R2/R3/R4 变体 3 条新增）

【PF2-DOC-03】.claude/test-inventory.md 对账
- 以 npm test 实际输出对账重写文件的用例数：agent-status-hook / agent-status-view / notifications / tab-rules / tab-title-registry / terminal-registry / terminal-registry-subscribe / use-xterm-lifecycle / ipc-hooks-contract（:183-189 / :45 / :57-64 段）
- L4 段（:257-259）agent-status 用例描述语义（静态行反转 + 新增 3 条防复发）；L4 用例数取数口径：静态数 test.e2e.ts 的 it 块（wdio 实际输出以 Stage 04 实跑为准）
- 你可跑 npm test 取数（单 agent 串行无冲突）

【PF2-DOC-04】src-tauri/src/hooks/CLAUDE.md 记录问题 5 结论
- hook 脚本 36-44ms/次（5 次测量 44/37/36/37/36ms，裸 node 基线 35ms）；启动路径仅 SessionStart 一个 hook 触发 → hooks 总贡献 ~0.1s 量级，非 claude 启动慢 1-3s 主因（主因 = claude 自身 Windows node 模块加载 + Ink 初始化）；接受现状，不做 per-event node spawn 优化

【PF2-DOC-01 核对】docs/hooks-dev/contract.md C12 含四字段定义与口径说明（Stage 03 已回填）——仅核对，drift 则对齐

完成判据：6 文件全部对齐最终代码（逐段 Read 代码核实）。
`, { label: 'A:docs-sync' })

// === Phase 2: 全量回归（命令相互独立，并行启动执行，收集全部结果）===
phase('全量回归')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量回归。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
5. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full regression' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量回归执行结果，测试类断言据此判定（无需重跑）：
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
