# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-A-06**：`src/panels/terminal/usePtyOutput.ts` 五个阈值常量均带 `export`（grep -c `export const` ≥ 5）；`use-xterm-output.test.ts` import 这些常量（grep 命中）；测试文件内不再硬编码 `65536`（grep -c 为 0）。
- **TQ-A-07**：`workspace-sideviews.test.tsx` 顶部注释含 `46px`（grep 命中）且不含 `40px`（grep -c 为 0）。
- **TQ-A-08**：`use-code-mirror.test.ts` 中 `toBeDefined()` 出现 ≤ 1 次（grep -c）；前两例各含至少一条用户可见行为断言（Read 确认）；`editor-confirm.test.ts` 重载路径含 `mockReadFile` 被调用断言（grep 命中）。
- **TQ-B-03**：`explorer-sandbox-race.test.tsx` 含新增的 DBG-10 真竞态用例（grep `真竞态` 或 `DBG-10` 命中）；断言含 `sprCallOrder` 与 `rdCallOrder` 的先后约束（grep 命中）。
- **TQ-B-07**：`activityBar.test.tsx` 的 getBoundingClientRect mock 含原实现 fallback（grep `originalGetBoundingClientRect` 命中）；computeDropTarget 纯函数测试存在（Glob `src/__tests__/drop-target.test.ts` 或既有文件内 grep `computeDropTarget` 命中用例）。
- **TQ-B-08**：`explorer-delete.test.tsx` 含经 `focusIn` 真实链路的键盘删除集成用例（grep `focusIn` 或 `focusin` 命中）。
- **TQ-B-12**：`src/__tests__/helpers/keyboard.ts` 存在且导出 `makeKeydown`/`dispatchKeydown`（Glob + Read 确认）；`global-commands.test.ts`、`shortcuts.test.ts`、`explorer-delete.test.tsx` 均 import 自该 helper（grep 命中 3 文件）。
- **TQ-B-16**：`wire-keybindings.test.ts` 含与真实 `useKeybindings` 集成的用例（grep `useKeybindings` 命中且非 fake 工厂注释）。
- **TQ-C-01**：`overrides.test.ts` 含 `syntax 9 组` 与 `ACC-05` 两个新用例（grep 命中）。
- **TQ-C-02**：`ipc-agent-history-contract.test.ts` 含 ≥ 4 处 `SEC-05` 负例用例（grep -c `SEC-05` ≥ 4）。
- **TQ-C-03**：`src/theme/CLAUDE.md` 含 `27 标量`（grep 命中）且不含 `26 标量`（grep -c 为 0）；`scheme-registry.test.ts` 注释含 4 个新增标量名（grep `accentFg` 命中）。
- **TQ-C-04**：`no-claude-literals.test.ts` 不再含 `SCAN_DIRS`（grep -c 为 0）；扫描根为全 `src` 且保留豁免目录清单（Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
