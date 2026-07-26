# Stage 03 逐项验证断言

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-01**：`src/panels/hooksConfig/` 目录存在；包含 `index.ts`、`HooksConfigPanel.tsx`、`useHooksConfig.ts` 等文件。
- **P3-FE-02**：`HooksConfigPanel.tsx` 渲染三态（loading/content/error）；顶部工具栏包含层级切换器与模式切换（GUI|JSON）按钮；使用 `theme/colors.ts` token，无硬编码色值。
- **P3-FE-03**：`src/panelRegistry.ts` 新增 `PANEL_HOOKS_CONFIG = "hooksConfig"`；`panelRegistry` 包含 `hooksConfig`；`PANEL_TYPES` 包含 `"hooksConfig"`；`FILE_PANEL_TYPES` 与 `isAlwaysRenderPanel` 不包含 hooksConfig。
- **P3-FE-04**：`src/panels/index.ts` 导出 `HooksConfigPanel`。
- **P3-FE-15**：`useHooksConfig.ts` 从 `useProjects`/`useLayout` 推导 `rootPath`；维护 `layer`、`configJson`、`guiModel`、`dirty`、`error`、`loading`；`readHooksConfig` 返回 Null 时视为 `{}`；切换 layer 前 dirty 状态触发确认。
- **P3-FE-18**：`src/stores/hooksConfig.ts` 存在；状态含 `disabledHooks` 与 `loaded`；`loadFromDisk`/`saveToDisk` 走 `src/ipc/settings`；导出 `cancelPendingSave`。
- **P3-FE-18b**：`src/App.tsx` 的 `registerCloseHandler` 保存序列中调用从 `src/stores/hooksConfig` 导入的 `cancelPendingSave`（或重命名后的别名）；与其他 store 的 `cancelPendingSave` 一并执行。
- **P3-TE-07**：`src/__tests__/hooks-config-store.test.ts` 存在；覆盖 load/sanitize/disable/enable/debounce payload `{ disabledHooks }`。
- **P3-TE-08**：`src/__tests__/hooks-config-panel.test.tsx` 存在；覆盖 PANEL_TYPES 注册、`isValidPanelType`、三态渲染、层级切换器；`src/__tests__/panel-registry.test.ts` 已同步为 6 个面板键且 `PANEL_TYPES.length === 6`。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-store hooks-config-panel panel-registry`
