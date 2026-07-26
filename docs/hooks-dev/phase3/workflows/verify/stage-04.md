# Stage 04 逐项验证断言

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-07**：`src/features/hooksConfig/schema/claude-code-settings.json` 存在；文件为合法 JSON；`package.json` 新增 `codemirror-json-schema` 与 `ajv` 依赖。
- **P3-FE-11**：`src/panels/hooksConfig/JsonMode.tsx` 存在；使用 `@codemirror/lang-json` 创建 CM6 视图；注册 schema 扩展；提供 `onValidationChange` 回调；非法 JSON 时通知父组件。
- **P3-FE-11b**：事件导航侧栏渲染九大分组；点击事件名可在编辑器内定位到对应 JSON 键。
- **P3-FE-09**：`src/panels/hooksConfig/MatcherTester.tsx` 存在；调用 `matcherEngine.matchHook`；显示命中结果与匹配模式。
- **P3-TE-09**：`src/__tests__/hooks-config-json-mode.test.tsx` 存在；覆盖渲染、schema 扩展、非法 JSON 回调。
- **P3-TE-10**：`src/__tests__/hooks-config-event-nav.test.tsx` 存在；覆盖分组渲染与跳转。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-json-mode hooks-config-event-nav`
