# Stage 04 逐项验证断言

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-07**：`src/features/hooksConfig/schema/claude-code-settings.json` 存在且为合法 JSON；可被 Vite JSON import 加载；**自包含性核实结论以注释形式落盘**（有无远程 `$ref`；含则已预打包展开）；**`properties.hooks` 子 schema 已导出**供编辑器与保存校验使用；`package.json` 新增依赖为 `codemirror-json-schema` + `@codemirror/lint` + `@codemirror/autocomplete`；**无 ajv**（语义式：grep `"ajv"` 在 package.json 不出现）。
- **P3-FE-11**：`src/panels/hooksConfig/JsonMode.tsx` 存在；使用 `@codemirror/lang-json` 创建 CM6 视图；注册 `jsonCompletion`/`jsonSchemaHover`/`jsonSchemaLinter`（schema 为 hooks 子 schema）；提供 `onValidationChange` 回调；非法 JSON 时通知父组件。
- **P3-FE-11 导航**：事件导航侧栏渲染 **eventsCatalog 十组**（非旧九组——语义式：对照 eventsCatalog 分组常量）；点击事件名可在编辑器内定位到对应 JSON 键（setSelection）。
- **P3-FE-11 MatcherTester**：`src/panels/hooksConfig/MatcherTester.tsx` 存在；调用 `matcherEngine.matchHook`；显示命中结果与匹配模式（exact-or/regex/all）；event 感知窄字符集。
- **P3-TE-09**：`src/__tests__/hooks-config-jsonmode.test.tsx` 存在；覆盖渲染、schema 扩展注册、非法 JSON 触发 `onValidationChange(false)`。
- **P3-TE-10**：同一测试文件覆盖十大分组渲染与点击跳转选区。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-jsonmode`
