# Stage 02 逐项验证断言

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-05**：`src/ipc/hooksConfig.ts` 存在；封装 `readHooksConfig(layer, projectPath?)` 与 `writeHooksConfig(layer, content, projectPath?)`；唯一调用 `invoke` 的位置在本文件；`layer` 类型为 `"user" | "project" | "local"`。
- **P3-FE-05b**：`src/ipc/index.ts` 重新导出 `hooksConfig` 模块的函数。
- **P3-FE-06**：`src/types/hooksConfig.ts` 存在；定义 `HooksLayer`、`HooksConfigGui`、`HookEventGroup`、`HookMatcherGroup`、`HookHandlerGui`、`DisabledHookKey`。
- **P3-FE-08**：`src/panels/hooksConfig/matcherEngine.ts` 存在；导出纯函数 `matchHook`；实现精确 OR、JS 正则、全匹配、大小写敏感；`FileChanged`/`StopFailure` 窄字符集强制正则。
- **P3-FE-10**：`src/panels/hooksConfig/configModel.ts` 存在；导出 `jsonToGui` / `guiToJson` / `filterDisabled`；对非法输入降级为空模型。
- **P3-TE-05**：`src/__tests__/hooks-config-matcher.test.ts` 存在；覆盖 matcher 语义全表。
- **P3-TE-06**：`src/__tests__/hooks-config-model.test.ts` 存在；覆盖 configModel 双向转换。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-matcher hooks-config-model`
