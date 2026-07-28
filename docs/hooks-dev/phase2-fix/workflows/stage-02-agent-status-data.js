// =====================================================================
// Stage 02：Agent Status 数据层修复
//   改动项：FIX-FE-03 / FIX-FE-04 / FIX-FE-05 / FIX-FE-06 / FIX-FE-07
// =====================================================================
// 结构（pipeline：A 先行，B 依赖 A 的 subscribe + panelId）：
//   Phase 1 库改造（串行 registry-lib）→ Phase 2 hook 消费方（串行 hook-consumer）
//   → Phase 3 全量测试 → Phase 4 逐项验证
//
// 跨边界契约（写死，agent 不各自推断，原文见 stages.md C2/C4）：
//   C2 TerminalRegistry.subscribe：
//     type RegistryEvent = { type: "register" | "remove"; panelId: string };
//     subscribe(listener: (e: RegistryEvent) => void): () => void  // 返回退订函数
//     ——register/remove 内同步通知全部 listener；通知在 Map 变更之后。
//   C4 src/lib/panelId.ts（新建）：
//     parseTerminalPageId(panelId: string): string | null
//     ——格式 terminal-{pageId}-{seq}，≥3 段 + 首段 terminal + 末段全数字 → 中间段 join；否则 null。
//     "terminal-page1-0" → "page1"；"terminal-my-page-2" → "my-page"；
//     "terminal-abc"（两段）/ "terminal-foo-bar"（尾段非数字）/ "editor-x-1"（非 terminal 前缀）→ null
//
// Agent 分工（文件全集 = prompt 触碰文件，无重叠）：
//   registry-lib  ：src/panels/terminal/TerminalRegistry.ts、src/lib/panelId.ts（新）、
//                   src/__tests__/terminal-registry-subscribe.test.ts（新）、src/__tests__/panelId.test.ts（新）
//   hook-consumer ：src/features/agentStatus/useAgentStatus.ts、src/__tests__/agent-status-hook.test.ts、
//                   src/features/notifications/useClaudeNotifications.ts、src/features/agentStatus/AgentStatusView.tsx
//
// 本 Stage 无特殊纪律（PREAMBLE_EXTRA 为空）——fix-loop 调用本 Stage 时
// args.constraints 无需传值（留空）。
// =====================================================================

export const meta = {
  name: 'stage2-agent-status-data',
  description: 'Stage 02：F5 数据层——行生命周期订阅/标题查找/null 语义/重订阅修复/parsePageId 收敛',
  phases: [
    { title: '库改造' },
    { title: 'hook 消费方' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 库改造（串行——hook-consumer 依赖 subscribe + panelId）===
phase('库改造')
const libResult = await agent(`${PREAMBLE}

你负责 FIX-FE-03 前半（TerminalRegistry.subscribe）与 FIX-FE-07 前半（panelId.ts）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-03 / FIX-FE-07 条目与 docs/hooks-dev/phase2-fix/stages.md 的 C2/C4 契约，再动手。

契约 C2（写死）：
- type RegistryEvent = { type: "register" | "remove"; panelId: string };
- subscribe(listener: (e: RegistryEvent) => void): () => void——返回退订函数；
- register/remove 内同步通知全部 listener；通知在 Map 变更之后。

契约 C4（写死）：
- 新建 src/lib/panelId.ts，导出 parseTerminalPageId(panelId: string): string | null；
- 格式 terminal-{pageId}-{seq}：≥3 段 + 首段 "terminal" + 末段全数字 → 中间段 join 为 pageId；否则 null。
- "terminal-page1-0" → "page1"；"terminal-my-page-2" → "my-page"；
  "terminal-abc"（两段）/ "terminal-foo-bar"（尾段非数字）/ "editor-x-1"（非 terminal 前缀）→ null。

步骤：
1. src/panels/terminal/TerminalRegistry.ts：按 C2 新增 subscribe（listeners Set + register/remove 内通知）。
2. src/lib/panelId.ts（新建）：按 C4 实现。
3. src/__tests__/terminal-registry-subscribe.test.ts（新建）：register 通知 / remove 通知 / 退订后不再通知。
4. src/__tests__/panelId.test.ts（新建）：正常 / 含连字符 pageId / 非数字尾段 / 非 terminal 前缀 / 两段。
不跑测试——全量测试由独立 agent 统一执行。
`, { label: 'registry-lib' })

// 前序失败短路，不跑下游
if (!libResult) {
  return { libResult, hookResult: null, testResult: null, verifyResult: { allFixed: false, failedItems: ['registry-lib-no-return'], details: { 'registry-lib-no-return': { status: 'not_fixed', evidence: 'registry-lib agent 未返回（被跳过或 API 错误）' } } } }
}

// === Phase 2: hook 消费方（串行——单 agent，文件含 Stage 01 已改文件，Stage 串行无冲突）===
phase('hook 消费方')
const hookResult = await agent(`${PREAMBLE}

你负责 FIX-FE-03/04/05/06（useAgentStatus 数据层）与 FIX-FE-07 引用替换。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-03/04/05/06/07 条目，再动手。registry-lib 已建好 TerminalRegistry.subscribe 与 src/lib/panelId.ts，你只 import 使用。

步骤（src/features/agentStatus/useAgentStatus.ts）：
1. FE-03 订阅增删：useEffect 内 TerminalRegistry.subscribe——register 且 panelId 属当前项目 →
   插入 🟡 行（同初始扫描语义）；remove → 移除对应行。
2. FE-04 标题：初始扫描与事件路径两处 \`终端 \${pageId}\` 改
   getPageApi(pageId)?.getPanel(panelId)?.title ?? \`终端 \${pageId}\`（getPageApi 来自 ../workspace/pageApis 或
   ../../workspace/pageApis，按所在路径调整）；事件到达时顺带刷新已有行标题。
3. FE-05 null 跳过：eventToStatus 返回 null → 不写入行 status（保留旧值），仍刷新 lastEventAt /
   transcriptPath / 触发用量拉取；SessionEnd/Exit 移除逻辑不变；删除 payload.event === "Stop" ? "done"
   特判（Stop → done 由真实 eventToStatus 映射）。
4. FE-06 useMemo 稳定：projectPageIds = useMemo(() => new Set(...), [activeProject])，
   handleHookEvent 的 useCallback deps 随之稳定，useEffect([handleHookEvent]) 不再每渲染重订阅。
5. FE-07 引用替换：删除本地 parsePageId（约 :48-62），改 import { parseTerminalPageId }。
   注意返回 string | null——原本地实现不返回 null 的路径按类型收窄处理。

步骤（其余两文件，仅 FE-07 引用替换）：
6. src/features/notifications/useClaudeNotifications.ts：删本地 parsePageId（约 :77-81），统一 import parseTerminalPageId。
7. src/features/agentStatus/AgentStatusView.tsx：删内联解析（约 :65-72），统一 import parseTerminalPageId；
   null 时 handleFocus 直接返回（不发起切换）。

步骤（src/__tests__/agent-status-hook.test.ts）：
8. 删除 claudeStatus 模块 mock（:46-67），改用真实 eventToStatus / getStatusIcon；审计现有 21 用例
   对真实映射（SessionStart→attention 等）的断言并修正。
9. mock 补 TerminalRegistry.subscribe 与 ../workspace/pageApis（getPageApi）。
10. 新增用例：null 状态跳过（⚡ 行收 Notification(auth_success) 仍为 ⚡；未知事件同理）/
    register 出现行 / remove 行消失 / 标题查找（mock getPageApi 返回带 title 面板）与回退 /
    行更新触发重渲染后 onHookEvent 调用次数不增（spy 计数）。
不跑测试——全量测试由独立 agent 统一执行。
`, { label: 'hook-consumer' })

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/hooks-dev/phase2-fix/workflows/verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { libResult, hookResult, testResult, verifyResult }
