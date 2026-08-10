// =====================================================================
// Stage 04 — 后端历史泛化 + 前端 ipc/types
// 条目：MC-301/302/303/304/305/306 + D-03(types barrel claudeHistory 段)/D-14(history.e2e 命令名段)
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 04 分工表与实现要点）
// commit message：refactor(agent-history): 后端历史会话泛化 + CliHistoryProvider 下沉 claude（MC-301~306）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 frontend-ipc-history agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// =====================================================================

export const meta = {
  name: 'stage-04-history-backend',
  description: 'Stage 04：后端历史会话泛化 + CliHistoryProvider 下沉 claude + 前端 ipc/types 同步（MC-301~306 + D-03/14）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（红线，触碰即返工）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
5. E2E 不得触碰用户真实 ~/.claude/（env 覆盖 + fixture 隔离）
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 04 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
1. 泛化命令（历史域 2 条）：agent_history_scan()（无参聚合——遍历全部已注册 provider 串行聚合，单 provider 失败不阻塞其他，全部空 → 空数组）；agent_history_delete(cliId, sessionId)（未知 cliId → AppError::Validation；delete 前经该 provider validate_session_id 前置）。旧命令名（claude_history_*）不保留兼容。
2. DTO：AgentHistorySession = sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists + cliId（provider 打标，serde camelCase 八键）；titleSource 五变体枚举 → 开放字符串（claude 值集 customTitle/aiTitle/summary/firstPrompt/none 不变；UI 不消费具体值）。
3. CliHistoryProvider trait 签名（写死）：scan() -> Vec<AgentHistorySession> / delete(session_id) -> Result<()> / validate_session_id(id) -> Result<()>；契约注释写明「validate_session_id 是 delete 的强制前置」（SEC-05 等价强制）。
4. 前端中间态：features/claudeHistory/ 目录更名与复合键改造属 Stage 05——本 Stage 前端仅做 ipc/types 更名 + 调用点签名同步（删除链传 session.cliId）。
【测试纪律】你不跑资源共享型测试（PTY/端口/全局锁/cargo test 执行）；后端只做编译级检查 cargo check --manifest-path src-tauri/Cargo.toml（或 cargo test --no-run），前端 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 中间态，忽略；真实执行由全量测试 agent 单点跑。cargo 系命令共享 target 目录锁，并行排队属正常勿中止。除 frontend-ipc-history 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-history',
    prompt: `你负责 Stage 04 的后端历史泛化：MC-301、MC-302、MC-303、MC-304、MC-305。禁止改 .claude/test-inventory.md（归 frontend-ipc-history agent）。

【MC-301】src-tauri/src/claude_history/（mod.rs/scan.rs/jsonl.rs/ops.rs 4 文件）→ src-tauri/src/agent_history/：聚合层（provider 注册表 + 命令）+ 新建 provider.rs（CliHistoryProvider trait，签名见 PREAMBLE 契约段 3）+ claude/ provider（scan.rs/jsonl.rs/ops.rs 整体下沉，行为零改动；is_uuid_filename 作为可复用工具保留）。
【MC-302】agent_history/mod.rs DTO：AgentHistorySession 七字段 + 新增 cli_id（provider 打标，serde camelCase 八键）；titleSource 开放字符串（claude provider 产出 cli_id: "claude"——provider 内部写字面量合法，MC-213 同理）。
【MC-303】命令层 + src-tauri/src/lib.rs 注册：agent_history_scan() 无参聚合（遍历全部已注册 provider 串行聚合；单 provider 失败不阻塞其他——照单文件降级条目契约的语义层级提升；全部空 → 空数组）；agent_history_delete(cliId, sessionId)：未知 cliId → Validation；delete 前经该 provider validate_session_id 前置。
【MC-304】SEC-05 保留（agent_history/claude/ ops.rs 下沉后）：UUID 形态校验 + locate_session_jsonl 遍历定位（前端不传任何路径）；trait 契约注释写明「validate_session_id 是 delete 的强制前置」——未来 provider 等价校验强制。
【MC-305】env 覆盖 SLTERM_CLAUDE_PROJECTS_DIR 留 claude provider 内部（resolve_projects_root 每次调用读 env 不缓存；ScanRootGuard 模式保留）；聚合层不假设 env 命名——未来 SLTERM_<CLI>_PROJECTS_DIR 同款模式自管。
【L1 测试】63 条用例（jsonl 28/scan 19/ops 9/mod 7）全部保留迁移；新增：聚合 scan 遍历（多 provider 桩）/ 单 provider 失败不阻塞 / delete 未知 cliId Validation / validate_session_id 前置。SEC-05（UUID 校验 + 定位不信托前端）用例保留；env 覆盖用例保留；tempdir 8.3 短名坑（dunce::canonicalize）纪律不变。你只做编译级检查（cargo check / cargo test --no-run），不执行 cargo test。

文件清单（只许碰这些）：src-tauri/src/claude_history/{mod.rs, scan.rs, jsonl.rs, ops.rs} → src-tauri/src/agent_history/（mod.rs 聚合层 + 新建 provider.rs + claude/{scan.rs, jsonl.rs, ops.rs}，含 claude/mod.rs 如需要）；改 src-tauri/src/lib.rs。`,
  },
  {
    label: 'frontend-ipc-history',
    prompt: `你负责 Stage 04 的前端 ipc/types 泛化：MC-306、D-03（claudeHistory 段）+ 调用点签名同步 + test-inventory 就近登记（本 Stage 独占）。

【MC-306】src/ipc/claudeHistory.ts → src/ipc/agentHistory.ts：scan  wrapper 无参 / delete wrapper 参数 { cliId, sessionId }（camelCase）；src/types/claudeHistory.ts → src/types/agentHistory.ts：AgentHistorySession（HistorySession 更名）八键含 cliId、titleSource 开放字符串（TitleSource 枚举类型同步放开为 string）；src/__tests__/ipc-claude-history-contract.test.ts → ipc-agent-history-contract.test.ts（8 用例：scan 无参 / delete 双参 camelCase 四维同步）。
【D-03 claudeHistory 段】改 src/types/index.ts barrel：claudeHistory → agentHistory；改 src/ipc/index.ts 同步。
【调用点签名同步】src/features/claudeHistory/useClaudeHistory.ts（scan 调用更名）、src/features/claudeHistory/HistorySessionList.tsx 与 historyContextMenu.ts（删除链改传 session.cliId + session.sessionId）——目录更名与复合键改造属 Stage 05，本 Stage 仅签名同步；类型引用 HistorySession → AgentHistorySession 等更名同步。
【测试同步】改 src/__tests__/claude-history-{model.test.ts, hook.test.tsx, view.test.tsx, restore.test.ts, row.test.tsx, action-dialog.test.tsx}（import/类型更名 + 删除链参数断言同步；文件名更名属 Stage 05）。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：L1 claude_history 63 用例迁移 agent_history（jsonl 28/scan 19/ops 9/mod 7，用例数不变）+ L1 新增聚合/前置校验用例；ipc-claude-history-contract → ipc-agent-history-contract（8 用例四维同步）；claude-history-* 测试断言同步（用例数不变）；E2E history.e2e 命令名断言同步（用例数不变）。

文件清单（只许碰这些）：src/ipc/claudeHistory.ts → src/ipc/agentHistory.ts；src/types/claudeHistory.ts → src/types/agentHistory.ts；改 src/ipc/index.ts、src/types/index.ts；src/__tests__/ipc-claude-history-contract.test.ts → src/__tests__/ipc-agent-history-contract.test.ts；改 src/features/claudeHistory/{useClaudeHistory.ts, HistorySessionList.tsx, historyContextMenu.ts}（仅签名同步）；改 src/__tests__/claude-history-{model.test.ts, hook.test.tsx, view.test.tsx, restore.test.ts, row.test.tsx, action-dialog.test.tsx}；改 .claude/test-inventory.md。`,
  },
  {
    label: 'e2e-history',
    prompt: `你负责 Stage 04 的 E2E 历史断言同步：D-14（本 Stage 段）。禁止改 .claude/test-inventory.md（归 frontend-ipc-history agent）。

【D-14 本 Stage 段】改 e2e-tests/history.e2e.ts：命令名断言 claude_history_scan / claude_history_delete → agent_history_scan / agent_history_delete 同步泛化；fixture 形态不变；恢复注入断言（claude --resume 逐字内容）本 Stage 不动（Stage 05 核对）。其余断言逐字不动。
【门禁说明】e2e-tests/ 不在根 tsconfig include——你的改动正确性由全量测试 agent 的 npm run e2e（L4）运行时兜底。

文件清单（只许碰这些）：改 e2e-tests/history.e2e.ts。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-7 并行收集，8 最后单独串行）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。命令清单：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npm run test:l3
8. npm run e2e
执行纪律：命令 1-7 相互独立，并行启动执行，收集全部结果；待 1-7 全部结束后，再单独串行执行命令 8（npm run e2e 内部 = build:e2e + wdio 串行；它会重新构建并占用 slterminal.exe，与其他命令并行会构建失败——禁拆分、禁与其他命令并行）。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
