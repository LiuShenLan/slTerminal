# Stage 03 逐项验证断言

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-01**：`src/panels/hooksConfig/` 目录存在；包含 `index.ts`、`HooksConfigPanel.tsx`、`useHooksConfig.ts` 等文件。
- **P3-FE-02**：`HooksConfigPanel.tsx` 渲染三态（loading/content/error）+ **配置损坏错误态**（read Err 时显示"配置文件损坏，请先修复"类文案，与无配置区分）；顶部工具栏包含层级切换器（user/project/local + 优先级标注）与模式切换（GUI|JSON）按钮、保存按钮、注入状态条占位；使用 `theme/colors.ts` token，无硬编码色值。
- **P3-FE-03**：`src/panelRegistry.ts` 新增 `PANEL_HOOKS_CONFIG = "hooksConfig"`；`panelRegistry` 包含 `hooksConfig`；`PANEL_TYPES` **末尾**包含 `"hooksConfig"`；`FILE_PANEL_TYPES` 与 `isAlwaysRenderPanel` 不包含 hooksConfig。
- **P3-FE-04**：`src/panels/index.ts` 导出 `HooksConfigPanel`。
- **P3-FE-15**：`useHooksConfig.ts` 从 `useProjects`/`useLayout` 推导 `rootPath`（rootPath 空时 project/local 层禁用）；维护 `layer`、`configJson`、`guiModel`、`dirty`、`error`、`loading`；`readHooksConfig` 返回 null 视为 `{}`；**轻量重读**：切层/面板 focusin 时重新 `readHooksConfig`，dirty 时经 `src/ipc/dialog` 的 `ask` 提示（非 `window.confirm`）；挂载时调用 `useHooksConfigStore.getState().loadFromDisk()`。
- **P3-FE-18**：`src/stores/hooksConfig.ts` 存在；状态含 `disabledHooks` 与 `loaded`；`loadFromDisk`/`saveToDisk` 走 `src/ipc/settings`；`disableHook`/`enableHook`/`isDisabled`；2s debounce + loaded 守卫；导出 `cancelPendingSave`。
- **P3-FE-18b**：`src/App.tsx` 的 `registerCloseHandler` 保存序列中调用从 `src/stores/hooksConfig` 导入的 `cancelPendingSave`（或重命名别名）；与其他 store 的 `cancelPendingSave` 一并执行。
- **P3-TE-07**：`src/__tests__/hooks-config-store.test.ts` 存在；覆盖 load/sanitize/disable/enable/debounce payload 键集合精确匹配 `{ disabledHooks }`、loaded 守卫。
- **P3-TE-08**：`src/__tests__/hooks-config-panel.test.tsx` 存在；覆盖 PANEL_TYPES 注册、`isValidPanelType`、三态 + 损坏错误态渲染、层级切换器；`src/__tests__/panel-registry.test.ts` 已同步：6 个面板键、`PANEL_TYPES.length === 6`、**`toEqual` 精确数组断言末尾含 `"hooksConfig"`**、索引断言顺延。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-store hooks-config-panel panel-registry`
