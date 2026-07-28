# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FIX-FE-03**：`TerminalRegistry.ts` 导出对象含 `subscribe`（grep 命中），`register`/`remove` 内存在 listener 通知调用（须 Read 确认通知在 Map 变更之后）。
- **FIX-FE-03**：`src/__tests__/terminal-registry-subscribe.test.ts` 存在且全绿（register 通知 / remove 通知 / 退订后不再通知）。
- **FIX-FE-03**：`useAgentStatus.ts` 中存在 `TerminalRegistry.subscribe` 调用（grep 命中）；register 事件插入行、remove 事件删除行（须 Read 确认分支语义）。
- **FIX-FE-03**：`agent-status-hook.test.ts` 含 register→出现行、remove→行消失用例且全绿。
- **FIX-FE-04**（语义式）：`useAgentStatus.ts` 中标题来源为 `getPageApi(pageId)?.getPanel(panelId)?.title`，回退 `终端 ${pageId}`（须 Read 确认两处：初始扫描与事件路径）；不存在无条件 `` `终端 ${pageId}` `` 赋值（grep 枚举该模板字符串逐处确认均带 fallback 链）。
- **FIX-FE-04**：`agent-status-hook.test.ts` 含标题查找用例（mock getPageApi 返回带 title 面板）与回退用例，全绿。
- **FIX-FE-05**（语义式）：`useAgentStatus.ts` 事件处理中，`eventToStatus` 返回 null 时**不**写入行 status（保留旧值），但仍刷新 lastEventAt / 触发用量拉取（须 Read 确认分支）；`SessionEnd`/`Exit` 移除逻辑保留。
- **FIX-FE-05**：`useAgentStatus.ts` 中不存在 `payload.event === "Stop"` 特判（grep 零命中）——Stop→done 由真实 eventToStatus 映射。
- **FIX-FE-05**：`agent-status-hook.test.ts` 中不存在 claudeStatus 模块 mock（grep `vi.mock("../lib/claudeStatus"` 零命中），使用真实 `eventToStatus`；含 null 状态用例（Notification 非 attention 子类型 → 状态保持）且全绿。
- **FIX-FE-06**：`useAgentStatus.ts` 中 `projectPageIds` 由 `useMemo` 产生（grep `useMemo` 命中且 Read 确认 deps 不含每渲染新建对象）；`agent-status-hook.test.ts` 含 onHookEvent 订阅次数不随重渲染增长的用例且全绿。
- **FIX-FE-07**：`src/lib/panelId.ts` 存在且导出 `parseTerminalPageId`（grep 命中）；`src/__tests__/panelId.test.ts` 存在且全绿（正常/含连字符/非数字尾段/非 terminal 前缀/两段）。
- **FIX-FE-07**（语义式）：全仓不存在任何 `parsePageId` 本地函数定义（grep `function parsePageId` 零命中；`const parsePageId` 零命中）；`useClaudeNotifications.ts`/`useAgentStatus.ts`/`AgentStatusView.tsx` 三处均 `import { parseTerminalPageId }`（grep 逐文件命中）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
