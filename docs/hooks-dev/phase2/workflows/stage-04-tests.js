// =====================================================================
// Stage 4 Workflow：测试补全（L1/L2 + L4 骨架）
// =====================================================================
// 跨边界契约（本脚本头部写死，agent 不各自推断）：
//   命令名：hooks_context_usage
//   返回 DTO：{ inputTokens: number, outputTokens: number } | null
//   侧栏视图 id：agent-status
//   事件名：hook-event
//   四态映射：src/lib/claudeStatus.ts
// =====================================================================

export const meta = {
  name: 'stage4-tests',
  description: '阶段 2 Stage 4：L1/L2 测试补全 + L4 关键路径骨架',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\data\learn\code\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
Stage 特殊纪律：本 Stage 只新增/修改测试代码；禁止为通过测试而改生产代码逻辑；L1 测试必须用 tempfile 隔离文件系统；L2 mock 必须在 vi.hoisted() 中创建。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'test-l2-notify',
    prompt: `你负责 P2-TE-01：新建 src/__tests__/notifications.test.ts。

测试目标：F4 通知门控与事件映射。

mock 要求：
- vi.hoisted() 中创建 mockSendClickableNotification、mockRequestUserAttention、mockSetFocus、mockOnHookEventCallback。
- vi.mock("../ipc/notification", () => ({ sendClickableNotification: mockSendClickableNotification, requestPermission: vi.fn(), isPermissionGranted: vi.fn(() => Promise.resolve(true)) }))。
- vi.mock("../ipc/hooks", () => ({ onHookEvent: vi.fn((cb) => { mockOnHookEventCallback.cb = cb; return () => {}; }), contextUsage: vi.fn() }))。
- vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: vi.fn(() => ({ setFocus: mockSetFocus, requestUserAttention: mockRequestUserAttention, onFocusChanged: vi.fn(() => () => {}) })), UserAttentionType: { Critical: 1 } }))。

用例（至少）：
1. 窗口失焦 + PermissionRequest 事件 → sendClickableNotification 被调用、requestUserAttention(UserAttentionType.Critical) 被调用。
2. 窗口失焦 + Stop 事件 → sendClickableNotification 被调用（任务完成类别）、requestUserAttention 不被调用。
3. 窗口失焦 + StopFailure 事件 → sendClickableNotification 被调用（错误类别）。
4. 窗口聚焦时三类事件 → sendClickableNotification 不被调用。
5. toast onClick 路由 → setFocus、switchToPage、panel.focus 被调用。
6. panel 已关闭时 onClick 不抛异常；sendClickableNotification 的 onclick 回调通过工厂绑定，不写在 sendNotification Options 上。

注意：
- 焦点状态通过修改模块级 ref 或 window.__slterm_windowFocused 模拟。
- stores 使用真实实现 + setState 种子。
- 完成后报告用例数与覆盖项。`
  },
  {
    label: 'test-l2-view',
    prompt: `你负责 P2-TE-02 / P2-TE-04：新建 src/__tests__/agent-status-view.test.tsx。

测试目标：AgentStatusView 三态渲染、行点击、用量条降级。

mock 要求：
- vi.hoisted() 中创建 mockTerminalRegistry、mockOnHookEventCallback、mockContextUsage、mockDockviewApi。
- vi.mock("../panels/terminal/TerminalRegistry", () => mockTerminalRegistry)。
- vi.mock("../ipc/hooks", () => ({ onHookEvent: vi.fn((cb) => { mockOnHookEventCallback.cb = cb; return () => {}; }), contextUsage: mockContextUsage }))。
- vi.mock("../lib/claudeStatus", () => ({ getStatusIcon: vi.fn((s) => ({ working: "⚡", attention: "🟡", done: "✅", error: "❌" }[s] ?? "🟡")) }))。

用例（至少）：
1. 无 rootPath 时显示 no-root 占位文案。
2. 当前项目无终端时显示 empty 占位文案。
3. TerminalRegistry 含两个 panelId（分属同项目两个 page）→ 渲染两行，data-e2e="agent-status-row" 数量为 2。
4. 点击行 → switchToPage 与 dockviewApi.getPanel(...).focus 被调用。
5. contextUsage 返回正常值 → 用量条填充宽度正确（按 200000 上限计算）。
6. contextUsage 返回 null → 用量条显示不可用态（"--"）。
7. 切换 activePageId 到另一项目 → 行列表清空。

注意：
- panelId 格式使用 \`terminal-{pageId}-{seq}\`。
- 种子 useProjects / useLayout。
- 完成后报告用例数。`
  },
  {
    label: 'test-l2-hook',
    prompt: `你负责 P2-TE-03：新建 src/__tests__/agent-status-hook.test.ts。

测试目标：useAgentStatus 状态联动（事件驱动、过滤、排序、移除）。

mock 要求：
- 同 test-l2-view 的 mock 模式（TerminalRegistry、onHookEvent、contextUsage）。
- vi.mock("../lib/claudeStatus", () => ({ getStatusFromEvent: vi.fn((e) => ({ Stop: "done", StopFailure: "error", PermissionRequest: "attention" }[e] ?? "working")) }))。

用例（至少）：
1. TerminalRegistry 初始含 panelId → hook 返回对应行。
2. 新 hook-event（PermissionRequest）到达 → 对应行状态变为 attention、lastEventAt 更新。
3. Stop 事件到达 → 行状态变为 done，且仍保留在列表中。
4. SessionEnd 事件到达 → 行被移除。
5. 事件来自其他项目 pageId → 不进入当前项目 rows。
6. 多行时按 lastEventAt 倒序排列。
7. contextUsage 在含 transcriptPath 的事件到达后被调用。

注意：
- 使用 renderHook(useAgentStatus) + act 触发事件。
- 完成后报告用例数。`
  },
  {
    label: 'test-l1',
    prompt: `你负责 P2-TE-05：在 src-tauri/src/hooks/ 内新增 hooks_context_usage 的 L1 测试。

位置选择（按阶段 1 结构）：
- 若 hooks 模块为单文件 mod.rs，则在文件底部 #[cfg(test)] mod tests 内追加。
- 若已拆分，则在 usage.rs 底部或 tests/hooks_context_usage_tests.rs 新建。

测试要求：
- 使用 tempfile::NamedTempFile 创建 JSONL 文件。
- 覆盖：
  1. 正常 JSONL 含多条 message.usage，返回最后一条的 input/output_tokens。
  2. JSONL 末尾无 usage → 返回 None。
  3. 某行 JSON 损坏 → 跳过损坏行，继续逆行扫描。
  4. 空文件 → 返回 None。
  5. 大文件（>128KB）仅读尾部 → 验证未加载全文件（可通过构造 200KB 文件、最后 1KB 含 usage，断言返回正确值间接证明）。

注意：
- 不要依赖真实 transcript 文件路径。
- 测试必须在 \`--test-threads=1\` 下稳定。
- 完成后报告用例数与测试文件路径。`
  },
  {
    label: 'test-l4',
    prompt: `你负责 P2-TE-06：在 e2e-tests/test.e2e.ts 追加阶段 2 关键路径用例骨架。

目标：覆盖 Agent Status 视图行渲染与 toast 触发链路。

用例骨架（至少）：
1. Agent Status 视图存在：
   - 通过 \`__slterm_e2e_toggleSideView("agent-status")\` 打开视图。
   - 断言侧栏槽位 \`sidebar-slot-top-agent-status\` 可见。
2. 创建项目并打开 claude 终端后，Agent Status 视图出现行（依赖真实 hook 事件，若环境无 claude 可仅保留断言骨架并标注 \`@skip\`）。
3. toast 触发链路（失焦 + 权限请求/Stop/错误）：
   - 由于 E2E 键盘输入限制与系统通知中心访问不可控，本用例可写为“触发事件后验证 sendNotification 调用”或标注为“人工验证点”。
   - 推荐：写一条 it.skip('toast 触发需人工验证')，并在注释中说明真实验证步骤。

注意：
- 不要修改现有 E2E helper 签名（除非与阶段 1 冲突）。
- 仅追加 describe 块，不删改现有用例。
- 完成后报告追加的 describe/it 数量。`
  }
]

const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\data\learn\code\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage4 的改动是否实际生效（项目根 D:\data\learn\code\slTerminal）。
先读 docs/hooks-dev/phase2/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
