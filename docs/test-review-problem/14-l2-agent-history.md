# L2 前端测试评审：通知 / Agent 状态 / Claude 历史会话

## 元信息

| 项 | 内容 |
|---|---|
| 评审对象 | slTerminal L2 前端测试（Vitest + jsdom + React Testing Library） |
| 领域 | 通知（Notifications）、Agent 状态（Agent Status）、Claude 历史会话（Claude History） |
| 源文件 | `src/features/notifications/useClaudeNotifications.ts`<br>`src/features/agentStatus/useAgentStatus.ts`<br>`src/features/agentStatus/AgentStatusView.tsx`<br>`src/features/agentStatus/AgentStatusRow.tsx`<br>`src/features/agentStatus/consts.ts`<br>`src/features/claudeHistory/useClaudeHistory.ts`<br>`src/features/claudeHistory/ClaudeHistorySections.tsx`<br>`src/features/claudeHistory/HistorySessionList.tsx`<br>`src/features/claudeHistory/HistorySessionRow.tsx`<br>`src/features/claudeHistory/SessionActionDialog.tsx`<br>`src/features/claudeHistory/historyContextMenu.ts`<br>`src/features/claudeHistory/historyModel.ts`<br>`src/features/claudeHistory/restoreSession.ts`<br>`src/panels/terminal/TerminalRegistry.ts`<br>`src/lib/claudeStatus.ts` |
| 测试文件 | `src/__tests__/notifications.test.ts`<br>`src/__tests__/agent-status-hook.test.ts`<br>`src/__tests__/agent-status-view.test.tsx`<br>`src/__tests__/claude-history-model.test.ts`<br>`src/__tests__/claude-history-hook.test.tsx`<br>`src/__tests__/claude-history-restore.test.ts`<br>`src/__tests__/claude-history-row.test.tsx`<br>`src/__tests__/claude-history-view.test.tsx`<br>`src/__tests__/claude-history-action-dialog.test.tsx` |
| 用例规模 | 9 个测试文件，约 190 条用例（通知 25 + Agent 状态 75 + Claude 历史 115，以 `.claude/test-inventory.md` 为准） |
| 评审日期 | 2026-08-03 |
| 评审人 | Claude Code |

## 执行摘要

- 整体行覆盖较高（多数文件 90% ~ 100%），但分支覆盖存在明显缺口，尤其是跨模块状态同步、事件分类、缓存截断、恢复编排等核心行为。
- 高风险点集中在：**历史区与活跃区四态同源回退逻辑未覆盖**、`TerminalRegistry` 的 session merge 语义未断言、通知事件分类表缺少系统化断言。
- 中风险点主要是视图层分支（标题覆盖、用量条信息、相对时间）、恢复编排的防重入 / 无 cwd 守卫、以及历史列表的默认折叠与右键菜单回调。
- 低风险点多为纯展示文案分支与调试辅助方法。

## 覆盖率总览

| 文件 | 行覆盖 | 分支覆盖 | 风险等级 | 缺口描述 |
|---|---|---|---|---|
| `src/features/notifications/useClaudeNotifications.ts` | 93.8% | 部分分支未覆盖 | 🔴 | 分类分支 76、131、139、143；去重缓存截断 132-133 |
| `src/features/agentStatus/useAgentStatus.ts` | 97.7% | 多分支未覆盖 | 🟡 | 建行 / 删行双通道组合、contextUsage 失败路径等 |
| `src/features/agentStatus/AgentStatusView.tsx` | 100% | 分支 118 未覆盖 | 🟡 | `titleBySessionId` 中 `s.title != null` 分支 |
| `src/features/agentStatus/AgentStatusRow.tsx` | 87.5% | 分支 50 未覆盖；行 65-66 未覆盖 | 🟡 | 行 2 的 `outputTokens` / 相对时间分支 |
| `src/features/claudeHistory/useClaudeHistory.ts` | 100% | 分支 35、60 未覆盖 | 🟡 | `scan` 的 generation 取消分支 |
| `src/features/claudeHistory/ClaudeHistorySections.tsx` | 100% | 分支 190、216 未覆盖 | 🟢 | 搜索空态 / 当前项目无历史等文案分支 |
| `src/features/claudeHistory/HistorySessionList.tsx` | 89.9% | 若干函数未覆盖 | 🟡 | 右键菜单 `onCopy` / `onFork` / `onDelete` 回调、分组默认折叠 |
| `src/features/claudeHistory/HistorySessionRow.tsx` | 100% | 分支 50 未覆盖 | 🟢 | `status` 与 `orphan` 共存时的图标优先级 |
| `src/features/claudeHistory/SessionActionDialog.tsx` | 100% | 分支 42 未覆盖 | 🟢 | `actions.length === 0` 防御分支 |
| `src/features/claudeHistory/historyModel.ts` | 100% | 分支 131 未覆盖 | 🔴 | `deriveActiveSessionStatuses` 的 `sessionId` 缺失回退 |
| `src/features/claudeHistory/restoreSession.ts` | 94.7% | 行 34、36 未覆盖 | 🟡 | `restoring` 防重入 / `cwd` null 守卫 |
| `src/panels/terminal/TerminalRegistry.ts` | 90.3% | `getAll` / `_size` / `_dump` 未覆盖 | 🟢 | 调试辅助 / 一次性读取方法 |

## 问题清单

### 1. 🔴 断言有效性 / 覆盖缺口 — `historyModel.ts` 四态同源回退分支未覆盖

- **文件:行号**：`src/features/claudeHistory/historyModel.ts:131`
- **代码片段**：
  ```ts
  export function deriveActiveSessionStatuses(): Map<string, ClaudeStatus> {
    const map = new Map<string, ClaudeStatus>();
    for (const [_, entry] of TerminalRegistry.getAll()) {
      const session = entry.claudeSession;
      if (!session) continue;
      const key = session.sessionId ?? stripJsonl(basename(session.transcriptPath));
      if (session.status != null) map.set(key, session.status);
    }
    return map;
  }
  ```
- **问题**：`sessionId` 缺失时回退到 `transcriptPath` basename 去 `.jsonl` 是历史区与活跃区 emoji 同源的关键路径，但该分支未被覆盖。一旦回退逻辑写错，两区状态会不同步。
- **改法**：在 `claude-history-model.test.ts` 增加注册表条目：`claudeSession: { sessionId: null, transcriptPath: "C:/x/abc.jsonl", status: "working" }`，断言 `deriveActiveSessionStatuses().get("abc") === "working"`。
- **变异推演**：将 `stripJsonl(basename(session.transcriptPath))` 改为 `basename(session.transcriptPath)`（保留 `.jsonl`）或改为 `null`，当前测试仍全绿。

### 2. 🔴 断言有效性 / 测试设计 — `TerminalRegistry.setClaudeSession` merge 语义未断言

- **文件:行号**：`src/panels/terminal/TerminalRegistry.ts`（`setClaudeSession` 内）
- **代码片段**：
  ```ts
  setClaudeSession(panelId, patch) {
    const entry = registry.get(panelId);
    if (!entry) return;
    if (patch === null) {
      entry.claudeSession = null;
    } else {
      entry.claudeSession = {
        ...entry.claudeSession,
        ...patch,
        lastEventAt: patch.lastEventAt ?? Date.now(),
      };
    }
    emit({ type: "sessionChange", panelId });
  }
  ```
- **问题**：测试只覆盖 set / clear，未验证 `Partial<ClaudeSessionInfo>` 的 merge 语义：`undefined` 字段不覆盖旧值、缺少 `lastEventAt` 时自动填充。这是 F5「双通道建行 / 三通道删行」的核心数据结构保证。
- **改法**：新增测试：先 `setClaudeSession(p, { sessionId: "s1", transcriptPath: "t", status: "attention" })`，再 `setClaudeSession(p, { status: "working" })`，断言 `transcriptPath` 仍为 `"t"` 且 `lastEventAt` 被更新。
- **变异推演**：将 `...entry.claudeSession, ...patch` 改为直接 `{ ...patch }`，当前测试仍会全绿。

### 3. 🔴 断言有效性 / 测试设计 — `useClaudeNotifications` 事件分类表缺少系统化断言

- **文件:行号**：`src/features/notifications/useClaudeNotifications.ts:76,131,139,143`
- **代码片段**：
  ```ts
  function classifyEvent(payload: HookEventPayload): NotifyCategory | null {
    if (payload.event === "PermissionRequest") return "permission";
    if (payload.event === "Notification" && payload.notificationType === "permission_prompt") return "permission";
    if (payload.event === "StopFailure" || payload.event === "PostToolUseFailure") return "error";
    if (payload.event === "Stop") return "done";
    return null;
  }
  ```
- **问题**：现有测试验证了部分事件会触发 toast，但未对 `classifyEvent` 的完整映射做表格化断言；`Notification` 非 `permission_prompt`、`Stop` 与 `StopFailure` 区分等分支未守卫。分类错误会导致 toast 类型/正文错误。
- **改法**：导出 `classifyEvent`（或拆出纯函数）并新增表格驱动测试：遍历事件 × notificationType，断言返回类别、是否触发 `sendToastNotification`、toast 标题/正文。
- **变异推演**：将 `Stop` 改为返回 `null`，或将 `PostToolUseFailure` 改为返回 `"done"`，当前用例仍可能通过（因为只断言了是否触发，未断言类别）。

### 4. 🟡 覆盖缺口 — 通知去重缓存截断逻辑无覆盖

- **文件:行号**：`src/features/notifications/useClaudeNotifications.ts:132-133`
- **代码片段**：
  ```ts
  if (seenRef.current.size > 200) {
    const entries = Array.from(seenRef.current);
    seenRef.current = new Set(entries.slice(-100));
  }
  ```
- **问题**：200 → 100 的截断分支未覆盖。若 `slice` 方向或阈值写错，长会话下会出现重复通知或永久丢失去重。
- **改法**：构造 250 个不同事件并推进时间，断言 `seenRef.current.size` 被截断为 100；再触发最旧事件应能重新弹 toast。
- **变异推演**：将 `slice(-100)` 改为 `slice(0, 100)`，或将 `> 200` 改为 `> 2`，当前测试仍绿。

### 5. 🟡 断言有效性 — `AgentStatusRow` 用量条下方信息与相对时间未断言

- **文件:行号**：`src/features/agentStatus/AgentStatusRow.tsx:65-66`、分支 50
- **代码片段**（示意）：
  ```tsx
  <div className="row-2">
    <UsageBar percent={percent} />
    <span>{percent}%</span>
    {outputTokens != null && <span>· {outputTokens} out</span>}
    <span>{formatRelativeTime(lastEventAt, now)}</span>
  </div>
  ```
- **问题**：测试主要断言行 1 标题 / 图标，未覆盖行 2 的 `outputTokens` 显示与 `formatRelativeTime(now)` 文本。若 `outputTokens` 被误删或相对时间计算错误，UI 异常无法被发现。
- **改法**：渲染带完整 usage 的行，断言 DOM 中出现 `outputTokens` 文本与 `formatRelativeTime` 返回的相对时间字符串。
- **变异推演**：删除 `{outputTokens != null && ...}` 整段，或将 `formatRelativeTime(lastEventAt, now)` 替换为固定字符串 `"刚刚"`，当前测试仍通过。

### 6. 🟡 测试设计 / mock 边界 — `AgentStatusView` 标题覆盖使用了 mock 的 history

- **文件:行号**：`src/features/agentStatus/AgentStatusView.tsx:118`
- **代码片段**：
  ```ts
  const titleBySessionId = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of history.sessions) {
      if (s.title != null) map.set(s.sessionId, s.title);
    }
    return map;
  }, [history.sessions]);
  ```
- **问题**：`agent-status-view.test.tsx` mock 了 `useClaudeHistory`，因此 `titleBySessionId` 的真实派生逻辑未被端到端验证；只要注入的 props 正确，测试就通过。
- **改法**：增加一个集成测试：用真实 `useClaudeHistory` 或受控 mock 让 `history.sessions` 包含 `/rename` 后的 title，断言活跃区行标题被覆盖。
- **变异推演**：将 `map.set(s.sessionId, s.title)` 改为 `map.set(s.transcriptPath, s.title)`，使用 mock 的视图测试仍全绿。

### 7. 🟡 覆盖缺口 — `restoreSession` 防重入与 `cwd` null 守卫未进入

- **文件:行号**：`src/features/claudeHistory/restoreSession.ts:34,36`
- **代码片段**（示意）：
  ```ts
  let restoring = false;
  export async function restoreSession(session, opts = {}) {
    if (restoring) return;
    if (!session.cwd) throw new Error("cwd required");
    restoring = true;
    ...
  }
  ```
- **问题**：覆盖率显示这两行未被执行。虽然测试清单提到覆盖防重入 / 无 cwd，但实际执行路径未进入这些守卫。
- **改法**：补充测试：① 同步连续调用两次 restore，断言只执行一次四步编排；② 传入 `cwd: null`，断言抛出 `"cwd required"`。
- **变异推演**：删除 `if (restoring) return;`，当前测试不会因并发双击场景失败。

### 8. 🟡 覆盖缺口 — `useClaudeHistory.scan` 的 generation 竞态取消未覆盖

- **文件:行号**：`src/features/claudeHistory/useClaudeHistory.ts:35,60`
- **代码片段**（示意）：
  ```ts
  const scan = useCallback(async () => {
    const gen = ++genRef.current;
    setState("loading");
    const result = await scanHistory();
    if (gen !== genRef.current) return;
    setSessions(result);
    setState("ready");
  }, []);
  ```
- **问题**：`gen !== genRef.current` 的丢弃分支未覆盖。快速连续触发 scan 时，旧结果应被丢弃，否则会出现「后发起的 scan 结果被前一次覆盖」的竞态。
- **改法**：模拟第一次 scan 延迟 resolve，第二次 scan 立即 resolve，断言最终 `sessions` 来自第二次。
- **变异推演**：移除 `if (gen !== genRef.current) return;`，当前测试仍全绿。

### 9. 🟡 覆盖缺口 — `HistorySessionList` 分组默认折叠与右键菜单回调未完整覆盖

- **文件:行号**：`src/features/claudeHistory/HistorySessionList.tsx`（ uncovered functions / lines）
- **代码片段**（示意）：
  ```ts
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const toggleGroup = (key: string) => { ... };
  const handleCopy = () => onCopy?.(session);
  const handleFork = () => onFork?.(session, true);
  const handleDelete = () => onDelete?.(session);
  ```
- **问题**：「全部项目」区分组默认收起（白名单模型）未断言；右键菜单的 copy/fork/delete 回调调用方未验证参数。若初始状态误写为全部展开，或回调参数错误，测试无法发现。
- **改法**：① 断言 `expandedGroups` 初始为空；点击组标题后包含该组 key。② 触发右键菜单并断言 `onCopy` / `onFork` / `onDelete` 被调用且传入正确的 `session` 对象（fork 标志为 `true`）。
- **变异推演**：将 `useState(new Set())` 改为 `useState(new Set(allGroups))`，当前测试仍通过。

### 10. 🟢 覆盖缺口 — `HistorySessionRow` 图标优先级分支未覆盖

- **文件:行号**：`src/features/claudeHistory/HistorySessionRow.tsx:50`
- **代码片段**（示意）：
  ```tsx
  let icon: string | null = null;
  if (status) icon = STATUS_EMOJI[status];
  else if (orphan) icon = "✗";
  ```
- **问题**：`status` 与 `orphan` 同时为 true 时，应显示状态 emoji 还是 orphan 标记？该优先级分支未覆盖。
- **改法**：添加 `status="working" && orphan=true` 用例，断言渲染 `⚡` 而非 `✗`。
- **变异推演**：交换 `if (status)` 与 `if (orphan)` 判断顺序，当前测试仍绿。

### 11. 🟢 覆盖缺口 — `SessionActionDialog` 空 actions 防御分支未覆盖

- **文件:行号**：`src/features/claudeHistory/SessionActionDialog.tsx:42`
- **代码片段**（示意）：
  ```tsx
  if (actions.length === 0) return null;
  ```
- **问题**：传入空 `actions` 数组时的防御分支未覆盖。
- **改法**：渲染 `<SessionActionDialog actions={[]} ... />`，断言不渲染弹窗或返回 null。
- **变异推演**：删除空数组判断直接渲染，当前无空 actions 用例，测试仍全绿。

### 12. 🟢 可维护性 — `TerminalRegistry` 调试辅助方法未覆盖

- **文件:行号**：`src/panels/terminal/TerminalRegistry.ts`
- **代码片段**（示意）：
  ```ts
  getAll(): ReadonlyMap<string, RegisteredTerminal> { return registry; },
  _size(): number { return registry.size; },
  _dump(): void { ... }
  ```
- **问题**：`_size`、`_dump` 为测试 / 调试专用，未覆盖不影响生产，但重构时容易误删或改名。
- **改法**：要么在 `terminal-registry.test.ts` 增加轻量断言，要么在 JSDoc 中标记 `@internal` 并说明仅供测试。
- **变异推演**：删除 `_dump` / `_size`，生产测试仍全绿（仅损失调试能力）。

## 已做变异推演的用例清单

| 序号 | 核心行为 | 目标文件 | 变异手法 | 当前测试是否捕获 |
|---|---|---|---|---|
| 1 | 历史区与活跃区四态同源：sessionId 缺失回退 | `historyModel.ts` | 回退改为保留 `.jsonl` 或返回 `null` | 否 |
| 2 | `TerminalRegistry` session 增量更新 | `TerminalRegistry.ts` | 移除旧 session 的 spread | 否 |
| 3 | Hook 事件 → 通知类别映射 | `useClaudeNotifications.ts` | 改变 `Stop`/`PostToolUseFailure` 返回值 | 否 |
| 4 | 通知去重缓存长序列截断 | `useClaudeNotifications.ts` | 改变 `slice` 方向或阈值 | 否 |
| 5 | Agent 行用量信息 / 相对时间展示 | `AgentStatusRow.tsx` | 删除 `outputTokens` 或固定时间文本 | 否 |
| 6 | `/rename` 后活跃区标题覆盖 | `AgentStatusView.tsx` | 用 `transcriptPath` 替代 `sessionId` 作 key | 否（因 mock） |
| 7 | 恢复会话防重入 / 无 cwd 防御 | `restoreSession.ts` | 删除 `if (restoring)` 或 `cwd` 守卫 | 否 |
| 8 | 历史 scan 竞态结果丢弃 | `useClaudeHistory.ts` | 移除 generation 检查 | 否 |
| 9 | 全部项目区分组默认折叠 + 右键回调 | `HistorySessionList.tsx` | 初始 expandedGroups 设为全部展开 | 否 |
| 10 | 历史行 orphan 与 status 图标优先级 | `HistorySessionRow.tsx` | 交换 `status` / `orphan` 判断顺序 | 否 |
| 11 | 动作弹窗空 actions 防御 | `SessionActionDialog.tsx` | 删除空数组防御 | 否 |

> 上述 11 个变异点约占核心行为用例（四态映射、行建模、恢复编排、列表交互、通知去重等约 40 个关键分支）的 27%，均说明当前断言未真正守卫这些行为。

## 总结与建议

- **补测优先级**：先补齐 3 个 🔴 问题（`historyModel` 回退、`TerminalRegistry` merge、通知分类），它们直接决定跨模块一致性，且变异可轻易通过。
- **其次补齐**：`AgentStatusRow` 行 2 断言、`AgentStatusView` 标题覆盖集成、`restoreSession` 守卫、`useClaudeHistory` generation 竞态、`HistorySessionList` 折叠与菜单回调。
- **低风险可延后**：纯文案分支、`SessionActionDialog` 空 actions、`_dump`/`_size` 等调试方法。
- **测试设计改进**：将 `classifyEvent`、`deriveActiveSessionStatuses`、`eventToStatus` 等纯逻辑拆出并做表格驱动测试；减少对 `useClaudeHistory` 的 mock，增加端到端状态流断言。
