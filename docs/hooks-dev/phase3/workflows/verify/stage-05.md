# Stage 05 逐项验证断言

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-12**：`src/panels/hooksConfig/GuiMode.tsx` 存在；实现 Master-Detail 布局；管理选中状态；提供添加/删除事件、matcher、handler 回调。
- **P3-FE-13**：`src/panels/hooksConfig/EventTree.tsx` 存在；渲染三级树（分组→事件→matcher→handler）；显示 hook 计数；选中态颜色来自 `theme/colors.ts` token。
- **P3-FE-14**：`src/panels/hooksConfig/HandlerForm.tsx` 存在；支持 5 种 type 表单；必填字段与 F6 表一致；事件→handler 支持矩阵过滤生效；切换 type 时清理不适用字段。
- **P3-TE-11**：`src/__tests__/hooks-config-handler-form.test.tsx` 存在；覆盖 5 种 type、支持矩阵、字段清理。
- **P3-TE-12**：`src/__tests__/hooks-config-event-tree.test.tsx` 存在；覆盖分组渲染、计数、选中、添加/删除事件。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- hooks-config-event-tree hooks-config-handler-form`
