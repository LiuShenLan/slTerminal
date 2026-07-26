# Stage 06 逐项验证断言

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-16**：`useHooksConfig.ts` 中 JSON 合法变更后调用 `jsonToGui` 更新 GUI 模型；GUI 变更后调用 `guiToJson` 更新 JSON；JSON 非法时禁止切换到 GUI 模式并显示错误提示；两模式共享 `dirty` 状态。
- **P3-FE-17**：保存按钮流程：先 `JSON.parse` 语法校验 → `ajv` schema 校验 → 任一失败则拒绝保存并弹窗；成功后显示「hooks 改动需重启 claude 会话生效」提示；无 `.bak` 逻辑。
- **P3-TE-13**：`src/__tests__/hooks-config-sync.test.tsx` 存在；覆盖 GUI→JSON、JSON→GUI、非法 JSON 阻止切换。
- **P3-TE-14**：`src/__tests__/hooks-config-save-safety.test.tsx` 存在；覆盖语法/schema 错误保存被拒、合法保存显示提示。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-sync hooks-config-save-safety`
