# Stage 05 逐项验证断言

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-12**：`src/panels/hooksConfig/GuiMode.tsx` 存在；实现 Master-Detail 布局（左事件树右详情）；管理 `selectedEvent`/`selectedMatcherIndex`/`selectedHandlerIndex`；提供添加/删除事件、matcher、handler 回调。
- **P3-FE-13**：`src/panels/hooksConfig/EventTree.tsx` 存在；渲染三级树（分组→事件→matcher→handler）；显示 hook 计数；选中态颜色来自 `theme/colors.ts` token；**注入段条目显示「slTerminal 托管」标记且删除禁用**（语义式：确认经 `isSltermManaged` 判定）。
- **P3-FE-14**：`src/panels/hooksConfig/HandlerForm.tsx` 存在；支持 5 种 type 表单；字段矩阵为官方版（语义式逐型核对：command 含 `asyncRewake` 且 command 无 `allowedEnvVars`；http 含 `url`/`headers`/`allowedEnvVars` 且**无** `method`/`body`；mcp_tool 含 `server`/`tool`/`input` 且**无** `args`；prompt 含 `prompt`/`model`/`continueOnBlock`；agent 含 `prompt`/`model` 且**无** `description`/`subagent_type`；无 `once` 字段）；事件→handler 支持矩阵经 eventsCatalog 过滤生效（B 档无 prompt/agent；SessionStart/Setup 仅 command/mcp_tool）；不支持 matcher 的事件无 matcher 输入框；切换 type 清理不适用字段；**注入段 handler 表单只读 + 禁删 + 禁禁用**。
- **P3-TE-11**：`src/__tests__/hooks-config-handlerform.test.tsx` 存在；覆盖 5 种 type 必填字段（官方字段名断言）、支持矩阵过滤、字段清理、注入段禁改。
- **P3-TE-12**：`src/__tests__/hooks-config-gui.test.tsx` 存在；覆盖十大分组渲染、计数、选中、添加/删除事件、注入段标记与禁删、不支持 matcher 事件无 matcher 输入。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-gui hooks-config-handlerform`
