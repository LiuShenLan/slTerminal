# Stage 07 逐项验证断言

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-19**：每条 handler 显示启停 checkbox；禁用条目在事件树中视觉区分；存在常驻提示「禁用条目由 slTerminal 托管，不出现在配置文件中」；四元组失配时显示「失效的禁用记录」。
- **P3-FE-20**：`useHooksConfig.ts` 保存前调用 `filterDisabled` 剔除禁用条目；重新启用时按四元组恢复条目。
- **P3-FE-21**：`HooksConfigPanel.tsx` 顶部工具栏存在「注入 Hooks」/「卸载 Hooks」按钮；按钮调用 `src/ipc/hooks.ts` 的 `injectHooks` / `uninstallHooks`。
- **P3-FE-22**：注入状态条显示已注入/未注入/版本过旧；调用 `getInjectionStatus` 刷新。
- **P3-TE-15**：`src/__tests__/hooks-config-disable.test.tsx` 存在；覆盖禁用→保存过滤→重载保留→重新启用恢复。
- **P3-TE-16**：`src/__tests__/hooks-config-stale-disabled.test.tsx` 存在；覆盖失效禁用记录标记与删除。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-disable hooks-config-stale-disabled`
