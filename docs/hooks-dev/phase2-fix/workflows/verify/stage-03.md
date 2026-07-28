# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FIX-FE-08**：`src/ipc/notification.ts` 中 `sendClickableNotification` 签名为 `(title, options: { body: string }, onClick)` 且返回类型为 `Notification | null`（Read 确认）；成功路径 `return` Notification 实例（Read 确认）。
- **FIX-FE-08**：`src/ipc/notification.ts` 存在 `export { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification"`（grep 命中）。
- **FIX-FE-08**：全仓 `sendSilentNotification` 零命中（grep）。
- **FIX-FE-08**：`useClaudeNotifications.ts` 调用点为 `sendClickableNotification("slTerminal", { body: ... }, ...)`（grep `\{ body:` 命中）；不存在以字符串作为第二参数的调用（Read 确认）。
- **FIX-FE-08**：`notifications.test.ts` 全绿（mock 与断言已同步新签名）。
- **FIX-FE-09**：全仓 `getContextUsage` 零命中（grep，含 `src/__tests__/setup.ts`）。
- **FIX-FE-09**：`src/__tests__/setup.ts` 的 `../ipc/hooks` 全局 mock 含 `contextUsage` 键（grep 命中）。
- **FIX-TE-01**：`ipc-hooks-contract.test.ts` 含 `contextUsage` 用例且覆盖四维——命令名 `hooks_context_usage`、参数 `{ transcriptPath: ... }`、返回值透传、异常传播（grep 命中 + 该文件用例总数 20，npm test 绿）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
