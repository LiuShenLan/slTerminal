# review-02 正确性

> 维度：multi-cli 重构后行为零回归的逻辑正确性。只写问题。

## 问题条目

### ZQ-1（P2）HistorySessionList 消费复合键未同步 `?? CLAUDE_CLI_ID` 回退
- 位置：src/features/agentHistory/HistorySessionList.tsx:278
- 问题：拼接方 historyModel.ts:138、AgentStatusView.tsx:133/144 均用 `${x.cliId ?? CLAUDE_CLI_ID}|${id}`，但消费方 `rowFlags` 直接写 `${session.cliId}|${session.sessionId}`。若 session.cliId 因旧数据/降级条目/类型违约为空/undefined，历史区行无法命中活跃区四态映射。
- 失败场景：扫描产出一个 cliId 为空的降级条目，或未来 provider 返回空 cliId → 历史区该行 status 始终为 undefined，运行中会话被误判为未运行。
- 修复建议：`rowFlags` 与 `findPanelForSession` 统一使用 `session.cliId ?? CLAUDE_CLI_ID`，或抽取公共 `keyOf(session)` 函数。
- 来源：独立发现

### ZQ-2（P2）MC-205 三级解析对空串 cliId 不回退
- 位置：src/panels/terminal/useXterm.ts:358-361、src/features/agentStatus/useAgentStatus.ts:134-137、src/features/notifications/useAgentNotifications.ts:65-68
- 问题：三级解析统一使用 `payload.cliId ?? ... ?? CLAUDE_CLI_ID`。`??` 只在 null/undefined 时回退，遇到空串 `""` 会将其当作有效 cliId，随后 profile 查找失败，事件被静默跳过。
- 失败场景：恶意/损坏信号文件写入 `"cliId":""` 且 `event="PermissionRequest"` → useAgentNotifications 跳过通知，useXterm/useAgentStatus 不更新 attention 状态，用户权限请求无感知。
- 修复建议：将空串与 null 同等处理，例如 `payload.cliId || ...` 或 `(payload.cliId?.trim() || ...)`。
- 来源：独立发现

### ZQ-3（P1）useAgentStatus hook 事件通道对 null-mapping 事件错误新建 attention 行
- 位置：src/features/agentStatus/useAgentStatus.ts:187-199
- 问题：新建行时写死 `status: newStatus ?? "attention"`。当事件映射结果为 null（如非 attention 的 Notification、未识别事件）且该行尚不存在时，会把本不该建行的场景强制建为 attention 行，产生幽灵活跃会话。
- 失败场景：SessionStart 信号丢失，首个到达的事件是 `Notification` 且 `notificationType` 不在 ATTENTION_NOTIFICATION_TYPES 内 → eventToStatus 返回 null，但 useAgentStatus 新建一条 attention 行，侧栏长期显示虚假的活跃会话。
- 修复建议：仅当 `newStatus` 非 null 时才允许 hook 事件通道新建行；null 状态事件只更新已有行或忽略。
- 校注（汇总核实）：问题形态属实（:194 `status: newStatus ?? "attention"`）；但修复有语义权衡——null 不建行后，SessionStart 信号丢失场景下事件到达不再能感知会话存活（行不出现直至首个可映射事件）。修复时建议：null 映射事件建行但 status 置 null（无图标），而非不建行——兼顾「感知存活」与「不误标 attention」。
- 来源：独立发现

### ZQ-4（P2）restoreSession.ts 的 panelId 基于 Date.now() 有毫秒级碰撞风险
- 位置：src/features/agentHistory/restoreSession.ts:133
- 问题：`panelId = \`terminal-${targetPageId}-${Date.now()}\``。`restoring` 仅阻塞并发，不阻塞串行调用；两次恢复落在同一毫秒会产生相同 panelId。
- 失败场景：用户连续快速恢复两个不同历史行（或自动化测试高频触发）且间隔 <1ms → 第二次 `api.addPanel` 与第一次 ID 冲突，TerminalRegistry.get(panelId) 可能取到错误/旧 entry，恢复命令被注入到错误终端。
- 修复建议：使用自增计数器或高分辨率唯一 ID，例如 `terminal-${targetPageId}-${Date.now()}-${++restoreSeq}`。
- 来源：独立发现

### ZQ-5（P2）hooks 配置读可返回 null、写拒绝 null，read-modify-write 契约不对称
- 位置：src-tauri/src/hooks/claude/config.rs:91-100、config.rs:171-181、config.rs:109-112
- 问题：`read_hooks_subtree` 对缺失文件返回 `Ok(Value::Null)`；但 `config_write_sync` 与 `write_hooks_subtree` 均要求 `hooks.is_object()`，否则返回 Validation 错误。
- 失败场景：前端首次打开某层 hooks 配置读到 null，若未做转换直接把 null 写回（或用户清空后提交 null）→ 后端返回「hooks 必须为 JSON 对象」，保存失败。
- 修复建议：写路径将 null 视作空对象 `{}` 进行 merge；或在 TS 类型层明确 `read` 返回 `Value | null` 并要求写入方保证传入对象。
- 来源：独立发现

### ZQ-6（P3）useXterm.ts 对 EXIT_EVENT 不清除页签图标
- 位置：src/panels/terminal/useXterm.ts:390-392
- 问题：`status === null` 分支仅在 `payload.event === SESSION_END_EVENT` 时调用 `onTabStateChange?.({ active: false })` 清图标；`EXIT_EVENT` 分支无处理。
- 失败场景：未来某 CLI 的 hook provider 上报 `"Exit"` 事件 → agentSession 被清空，但页签仍保留上一个 emoji 图标，状态显示不一致。
- 修复建议：将条件改为 `payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT` 时均清图标。
- 来源：独立发现

### ZQ-7（P3）复合键未防御 `|` 字符
- 位置：src/features/agentHistory/historyModel.ts:138、src/features/agentStatus/AgentStatusView.tsx:133/144、src/features/agentHistory/HistorySessionList.tsx:278
- 问题：所有复合键均通过 `${a}|${b}` 模板字符串拼接，未对 `a` 或 `b` 中含 `|` 的情况做转义或校验。
- 失败场景：新增某 CLI profile，其 sessionId 格式含 `|`（如自定义标识方案）→ deriveActiveSessionStatuses 生成键 `cli|a|b`，消费方无法匹配，活跃状态与标题覆盖全部失效。
- 修复建议：抽取 `keyOf(cliId, sessionId)` 辅助函数，对 `|` 进行转义（如 `replaceAll('|', '\\|')`）或改用结构化键（对象/数组）并在 Map 序列化时处理。
- 来源：独立发现
