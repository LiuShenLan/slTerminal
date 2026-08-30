# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 只改测试——生产代码（src/ 下非 __tests__ 文件）零改动。

## 断言清单

- **TE-04**：`src/__tests__/settings-pages-registration.test.ts` 存在；文件内 `vi.mock` 调用**不**包含 `../features/settingsCenter/pages` 自身路径（Read 确认真实 pages.ts 被 import）；断言 `getSettingsPageRegistry().getAll()` 含 `{id:"keybindings",group:"global",order:10}` / `{id:"planBalance",group:"global",order:20}` / `{id:"hooks",group:"project",order:100}` 三条注册条目（Read 确认 id/group/order 精确匹配）；`afterEach` 调 `_reset()`。
- **TE-05**：`grep -n "mockContainerApi" src/__tests__/settings-hooks-page.test.tsx src/__tests__/hooks-config-sync.test.tsx` 零命中；`mockApi` 死代码定义已删（若仍有 mockApi 命中须 Read 确认确为活引用）。
- **TE-05**：`settings-hooks-page.test.tsx` 存在「点击当前已选中 CLI 不触发 onPageParamsChange」用例（语义：渲染 selectedCli 为当前选中值 → 点击同项 → 断言 mockOnPageParamsChange 零调用，Read 确认）。
- **TE-06**：`settings-panel.test.tsx` 的 pageParams patch 用例（原 :248-270 区域）含 `containerApi.toJSON` 被调用断言（Read 确认）。
- **TE-06**：存在「onPageParamsChange 合并既有 params 且不修改原对象」新用例（语义：断言合并结果含原键 + 原 params 对象未被改写，Read 确认）。
- **边界**：`git diff --name-only` 本 Stage 改动仅限 `src/__tests__/` 下文件（生产代码零改动）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
