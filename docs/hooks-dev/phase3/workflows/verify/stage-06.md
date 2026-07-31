# Stage 06 逐项验证断言

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-16**：`useHooksConfig.ts` 中 JSON 合法变更后调用 `jsonToGui` 更新 GUI 模型；GUI 变更后调用 `guiToJson` 更新 JSON；JSON 非法时禁止切换到 GUI 模式并显示错误提示；两模式共享 `configJson` 与 `dirty` 状态。
- **P3-FE-17**：保存按钮流程：`JSON.parse` 语法校验 → **`json-schema-library`（`compileSchema(hooksSubSchema).validate(data)`）schema 校验**（语义式：grep 确认无 `ajv` import）；任一失败拒绝保存并弹窗；校验通过 → `filterDisabled` 剔除禁用条目 → `writeHooksConfig(layer, filtered, projectPath?)`；成功后显示「hooks 改动需重启 claude 会话生效」提示；无 `.bak` 逻辑。
- **P3-TE-13**：`src/__tests__/hooks-config-sync.test.tsx` 存在；覆盖 GUI→JSON、JSON→GUI、非法 JSON 阻止切换。
- **P3-TE-14**：同一测试文件覆盖：语法/schema 错误保存被拒、合法保存显示重启提示、**保存调用 payload 为 hooks 子树（invoke 参数键集合精确匹配 `{ layer, hooks, projectPath? }`）**。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-sync`
