# Stage 07 逐项验证断言

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-19**：每条 handler 显示启停 checkbox；**注入段条目（`isSltermManaged` 命中）不渲染禁用 checkbox**；禁用条目在事件树中视觉区分（置灰/删除线）；存在常驻提示「禁用条目由 slTerminal 托管，不出现在配置文件中」；四元组失配时显示「失效的禁用记录」。
- **P3-FE-20**：`useHooksConfig.ts` 保存前调用 `filterDisabled` 剔除禁用条目；重新启用时按四元组恢复条目（原位置不存在则标记失效）。
- **P3-FE-21**：`HooksConfigPanel.tsx` 顶部存在「注入 Hooks」/「卸载 Hooks」按钮；按钮调用 `src/ipc/hooks.ts` 经 `hooks` namespace 导出的 **`inject()` / `uninstall()`**（语义式：确认非 `injectHooks`/`uninstallHooks` 错误命名）；**操作完成后自动重读 user 层配置**（触发 `readHooksConfig("user", ...)`）。
- **P3-FE-22**：注入状态条显示已注入/未注入/版本过旧（injected/notInjected/outdated）；调用 `getInjectionStatus()` 刷新；操作后状态更新。
- **P3-TE-15**：`src/__tests__/hooks-config-disable.test.tsx` 存在；覆盖禁用→保存过滤（IPC hooks 不含禁用条目）→重载保留→重新启用恢复。
- **P3-TE-16**：同一测试文件覆盖：四元组失配显示失效标记、删除失效记录后消失、注入段条目无禁用 checkbox。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-disable`
