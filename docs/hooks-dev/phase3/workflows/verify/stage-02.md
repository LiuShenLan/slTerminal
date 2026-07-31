# Stage 02 逐项验证断言

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-05**：`src/ipc/hooksConfig.ts` 存在；封装 `readHooksConfig(layer, projectPath?)` 与 `writeHooksConfig(layer, hooks, projectPath?)`；唯一调用 `invoke` 的位置在本文件；`layer` 类型为 `"user" | "project" | "local"`；**write 的 invoke payload 键名为 `hooks`（非 `content`）**（语义式：Read 代码确认 payload 字段名）。`src/ipc/index.ts` 已 re-export。
- **P3-FE-06**：`src/types/hooksConfig.ts` 存在；定义 `HooksLayer`、原始 JSON 类型（`HooksConfigJson`/`MatcherGroupJson`/`HookHandlerJson`）、GUI 模型（`HooksConfigGui`/`HookEventGroup`/`HookMatcherGroup`/`HookHandlerGui`）、`DisabledHookKey`；handler 字段矩阵照 C13-3 官方版（mcp_tool 为 `input`、http 无 method/body、agent 无 description/subagent_type）。
- **P3-FE-26**：`src/panels/hooksConfig/eventsCatalog.ts` 存在；30 事件 × 10 组完整映射（对照 `docs/hooks-dev/phase3/stages.md` 头部「事件元数据目录」全表逐行核对——语义式：事件数=30、组数=10）；每事件含 matcher 支持标记 + 匹配目标 + handler 支持档（A/B/C）；含 5 种 handler 字段矩阵常量；纯数据 + 纯函数，无 DOM/React 依赖。
- **P3-FE-08**：`src/panels/hooksConfig/matcherEngine.ts` 存在；导出纯函数 `matchHook`；实现精确 OR、JS 正则非锚定、全匹配、大小写敏感；`FileChanged`/`StopFailure` 窄字符集（仅字母/数字/`_`/`|`）强制正则；注释含版本前提（v2.1.191+/v2.1.195+）。
- **P3-FE-10**：`src/panels/hooksConfig/configModel.ts` 存在；导出 `jsonToGui` / `guiToJson` / `filterDisabled` / `isSltermManaged`；`isSltermManaged` 按 `slterm-hook-reporter` 子串判定；不支持 matcher 的事件 `guiToJson` 省略 `matcher` 键；对非法输入降级为空模型。
- **P3-TE-05**：`src/__tests__/hooks-config-matcher.test.ts` 存在；覆盖 matcher 语义全表（每分支 ≥2 用例）。
- **P3-TE-06**：`src/__tests__/hooks-config-model.test.ts` 存在；覆盖双向转换 + `isSltermManaged` + `filterDisabled` + 无 matcher 事件省略键。
- **P3-TE-19**：`src/__tests__/hooks-config-catalog.test.ts` 存在；覆盖 30 事件齐全唯一、10 分组、handler 三档断言、10 个无 matcher 事件标记、5 种 handler 字段矩阵。
- **P3-FE-05 契约测试**：`src/__tests__/ipc-hooks-config-contract.test.ts` 存在；验证两条命令名 + write payload 键集合精确为 `{ layer, hooks, projectPath? }` + 异常传播。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-matcher hooks-config-model hooks-config-catalog ipc-hooks-config-contract`
