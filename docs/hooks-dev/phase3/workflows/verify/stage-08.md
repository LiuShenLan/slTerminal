# Stage 08 逐项验证断言

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-23**：`src/features/shortcuts/commandCatalog.ts` 的 `COMMAND_CATALOG` 包含 `global.openHooksConfig`；元数据完整；默认键为 `Ctrl+Shift+H` 或执行期确认键；默认键不是保留键。
- **P3-FE-24**：`src/features/shortcuts/globalCommands.ts` 的 `createGlobalShortcuts` 返回 `global.openHooksConfig` 命令；handler 调用 `getDockviewApi().addPanel({ component: "hooksConfig", ... })`；无 api 时返回 `false` 透传。
- **P3-FE-25**：`src/App.tsx` 无需修改调用点；确认 `...createGlobalShortcuts(...)` 已包含新命令。
- **P3-FE-23**：`src/features/shortcuts/commandCatalog.ts` 的 `COMMAND_CATALOG` 包含 `global.openHooksConfig`；元数据完整；默认键为 `Ctrl+Shift+H` 或执行期确认键；默认键不是保留键。
- **P3-FE-24**：`src/features/shortcuts/globalCommands.ts` 的 `createGlobalShortcuts` 返回 `global.openHooksConfig` 命令；handler 调用 `getDockviewApi().addPanel({ component: "hooksConfig", ... })`；无 api 时返回 `false` 透传。
- **P3-FE-25**：`src/App.tsx` 无需修改调用点；确认 `...createGlobalShortcuts(...)` 已包含新命令。
- **P3-TE-17**：`src/__tests__/command-catalog.test.ts` 已同步：`EXPECTED_IDS` 含 `global.openHooksConfig`、长度预期为 10、补充该命令的 `commandFromMeta` 与默认键非保留键断言；`src/__tests__/hooks-config-command.test.ts`（若新建）覆盖 handler 调用 `addPanel` 与无 api 透传。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- command-catalog hooks-config-command`
