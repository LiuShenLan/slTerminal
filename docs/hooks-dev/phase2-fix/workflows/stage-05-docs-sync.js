// =====================================================================
// Stage 05：文档同步（FIX-DOC-01 / FIX-DOC-02）
// =====================================================================
// 结构：单 docs agent → 全量测试 → 逐项验证
// （单 agent 理由：多文档间口径需一致，拆分收益低）
//
// Agent 分工（文件全集）：
//   docs（FIX-DOC-01/02）：.claude/test-inventory.md、src/ipc/CLAUDE.md、
//     src/lib/CLAUDE.md、src/workspace/CLAUDE.md、src/panels/CLAUDE.md、
//     src/features/sideViews/CLAUDE.md、e2e-tests/CLAUDE.md、.claude/CLAUDE.md
//
// 本 Stage 特殊纪律（PREAMBLE_EXTRA）：只改文档，禁止改任何代码/测试文件。
// fix-loop 调用本 Stage 时 args.constraints 传：
//   "本 Stage 只改文档，禁止改任何代码/测试文件"
// =====================================================================

export const meta = {
  name: 'stage5-docs-sync',
  description: 'Stage 05：test-inventory 失实重写 + CLAUDE.md 系列对齐最终代码',
  phases: [
    { title: '文档同步' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】本 Stage 只改文档，禁止改任何代码/测试文件。`

// === Phase 1: 文档同步（单 agent）===
phase('文档同步')
const docsResult = await agent(`${PREAMBLE}

你负责 FIX-DOC-01（test-inventory.md 失实重写 + 计数更新）+ FIX-DOC-02（CLAUDE.md 系列同步）。先读 docs/hooks-dev/phase2-fix/checklist.md 的两个条目（含各行号与逐条要点），再动手。

【FIX-DOC-01】.claude/test-inventory.md（失实行号见 checklist :117 枚举）：
1. 按各文件**实际内容**重写覆盖描述：mod.rs 行剔除 hooks_context_usage；usage.rs 行改正函数名（parse_usage_line / scan_transcript_usage）与覆盖项；ipc-contract / ipc-hooks-contract 行按 FIX-TE-01 落地后实况写；notifications 行剔 4 项不存在用例；agent-status-hook 行剔"轮询"改事件驱动；agent-status-view 行剔 tooltip/加载态/错误态；L4 行按 FIX-TE-03 落地后实况写；changelog 括注改真实增量构成。
2. 用例数更新：以 Stage 1-4 完成后的**实跑统计**为准——先跑 npm test 与 cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 取尾行统计数（新增 panelId.test.ts、terminal-registry-subscribe.test.ts；ipc-hooks-contract 16→20；L4 18 active→20 active + 1 skip），禁凭估计填写。
3. 自查：grep 失实关键词 parse_context_usage / read_context_usage_file / total_tokens / exit_code / 轮询 / tooltip 在该文件中零命中。

【FIX-DOC-02】逐文件对照真实代码同步（文档不撒谎，每条先 Read 代码再写）：
- src/ipc/CLAUDE.md：notification.ts 条目写 sendClickableNotification 契约（签名/返回值/re-export 三函数/ensureNotificationPermission）；修正"thin wrapper 直接 re-export 不添加额外逻辑"的失实归类（notification 含 onclick 工厂逻辑）。
- src/lib/CLAUDE.md：文件表追加 panelId.ts（parseTerminalPageId）；测试模式段追加 panelId.test.ts。
- src/workspace/CLAUDE.md：文件表追加 pageApis.ts；「页面切换流」补 switchToPageShared / switchToPageAndFocus 与 __dockviewApi 三站点重指向不变量。
- src/panels/CLAUDE.md：TerminalRegistry 条目补 subscribe API。
- src/features/sideViews/CLAUDE.md：useAgentStatus 条目补订阅增删/null 跳过/标题查找语义。
- e2e-tests/CLAUDE.md：补 settings.json 备份/还原机制 + Agent Status 静态/动态用例描述。
- .claude/CLAUDE.md：workspace 模块行如需补 pageApis 则补（对照模块索引表风格，保持渐进式披露，不展开细节）。
`, { label: 'docs' })

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/hooks-dev/phase2-fix/workflows/verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { docsResult, testResult, verifyResult }
