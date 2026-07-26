# Stage 03 Verify：前端 IPC hooks + 四态映射单点

> 断言与 Stage 03 完成后的真实中间态一致。

## 文件存在性

- [ ] `P1-FE-01` `src/ipc/hooks.ts` 存在。
- [ ] `P1-FE-03` `src/lib/claudeStatus.ts` 存在。
- [ ] `P1-FE-04` `src/__tests__/ipc-hooks-contract.test.ts` 存在。
- [ ] `P1-FE-05` `src/__tests__/claude-status.test.ts` 存在。

## IPC 封装

- [ ] `P1-FE-01` `src/ipc/hooks.ts` 导出 `inject()` / `uninstall()` / `getInjectionStatus()`，分别调用 `invoke("hooks_inject")` / `invoke("hooks_uninstall")` / `invoke("hooks_injection_status")`。
- [ ] `P1-FE-01` `src/ipc/hooks.ts` 导出 `onHookEvent(cb)`，使用 `listen<HookEventPayload>("hook-event")`。
- [ ] `P1-FE-01` `onHookEvent` 返回 unsubscribe 函数。
- [ ] `P1-FE-02` `src/ipc/index.ts` 含 `export * as hooks from "./hooks"`。

## 四态单点

- [ ] `P1-FE-03` `src/lib/claudeStatus.ts` 导出 `ClaudeStatus` 类型：`"working" | "attention" | "done" | "error" | null`。
- [ ] `P1-FE-03` `STATUS_EMOJI` 常量：`working=⚡`、`attention=🟡`、`done=✅`、`error=❌`。
- [ ] `P1-FE-03` `eventToStatus("SessionStart")` 返回 `"attention"`。
- [ ] `P1-FE-03` `eventToStatus("UserPromptSubmit")` / `"PreToolUse"` / `"PostToolUse"` 返回 `"working"`。
- [ ] `P1-FE-03` `eventToStatus("Notification", "permission_prompt")` / `"idle_prompt"` / `"agent_needs_input"` 返回 `"attention"`。
- [ ] `P1-FE-03` `eventToStatus("Notification", "auth_success")` 返回 `null`。
- [ ] `P1-FE-03` `eventToStatus("PermissionRequest")` 返回 `"attention"`。
- [ ] `P1-FE-03` `eventToStatus("Stop")` 返回 `"done"`。
- [ ] `P1-FE-03` `eventToStatus("PostToolUseFailure")` / `"StopFailure"` 返回 `"error"`。
- [ ] `P1-FE-03` `eventToStatus("SessionEnd")` 返回 `null`。

## 测试

- [ ] `P1-FE-04` `ipc-hooks-contract.test.ts` 验证三命令名与参数结构。
- [ ] `P1-FE-04` `ipc-hooks-contract.test.ts` 验证 `onHookEvent` 的 `listen` 订阅与 unsubscribe。
- [ ] `P1-FE-05` `claude-status.test.ts` 覆盖 10 事件 × notificationType 组合。
- [ ] `npm test ipc-hooks-contract claude-status` 通过。

## 静态检查

- [ ] `npx tsc --noEmit` 通过。
- [ ] `npx eslint src/ipc/hooks.ts src/ipc/index.ts src/lib/claudeStatus.ts` 通过。
