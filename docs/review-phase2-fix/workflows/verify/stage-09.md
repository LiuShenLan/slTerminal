# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-35**：grep 全仓 `terminalTabConfig` 零命中（含 src/、e2e-tests/、knip.json——knip.json 中 S01 为其加的 ignoreExports 条目已一并删除）
- **FE-35**：`src/__tests__/panel-registry.test.ts` 无 terminalTabConfig import 与对应 describe 块（grep 零命中），`npm test` 全绿
- **FE-46**：`src/lib/ErrorBoundary.tsx` inline variant 含「重试」按钮（Read 确认：onClick 将 error state 置 null；样式经 theme token，无硬编码色值）
- **FE-46**：`src/__tests__/error-boundary.test.tsx` 含重试用例（grep 「重试」命中），`npm test` 全绿
- **FE-47**：`src/App.tsx` 的 `pty.ptyKillAll()` 包在 `Promise.race` 总超时内（Read 确认：race 含 setTimeout null 兜底分支，SHUTDOWN_TIMEOUT_MS 量级；其后 killed 判空守卫存在）
- **FE-48**：`src/workspace/pageApis.ts` 与 `src/features/agentHistory/restoreSession.ts` 两处轮询 setTimeout 均含 abort 清理（Read 确认：clearTimeout + addEventListener("abort") 各一处——语义断言，不限变量名）
- **FE-48**：`src/__tests__/pageapis.test.ts` 与 `src/__tests__/agent-history-restore.test.ts` 含 abort 立即 settle 断言（grep 「abort」命中，Read 确认），`npm test` 全绿

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx knip --production`
5. `npx tauri build --debug --no-bundle`
