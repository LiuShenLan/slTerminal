# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-FE-06**：`src/panels/editor/gitGutter.ts` 导出 `buildHeadRangeSet(state, hunks)`（grep 命中签名）；映射语义（须 Read 确认）：modified hunk 的 old 行标 ModifiedMarker、纯删除 old 行标 DeletedMarker、old>new 剩余 old 行标 DeletedMarker、纯新增无标记；`gitGutter.test.ts` 存在 HEAD 侧映射 describe（≥4 用例）且通过；既有 workdir 侧 `buildRangeSet` 逻辑与测试零改动（git diff 确认该函数体无变化）
- **CV-FE-07**：`src/panels/diff/DiffPanel.tsx` 存在；双栏布局含 `data-e2e="diff-left"` / `data-e2e="diff-right"`（grep 命中）；左栏只读（`EditorState.readOnly` + `EditorView.editable.of(false)`）；左栏用 `buildHeadRangeSet`、右栏用 `diffGutter`/`updateDiffGutter`（Read 确认 import 与调用）；滚动同步存在防循环标记（须 Read 确认——一侧 scroll 设置对侧 scrollTop 时不会回环触发，不限变量名如 `syncingRef`）；右侧保存动作 = 写盘 → 重新 `gitDiff` → 刷新 gutter 与对齐（Read 确认调用链）；右侧经 `usePanelFocus("editor", ...)` + `setActiveEditor` 接入 Ctrl+S（grep 命中）
- **CV-FE-07 硬约束**：diff 目录内 grep `invoke` 零命中；grep `#[0-9a-fA-F]{6}` 零命中（颜色全 token）
- **CV-FE-08**：`src/panels/diff/alignment.ts` 导出 `computeAlignment(hunks)` 返回 `{ left: Map, right: Map }`（Read 确认）；`diff-alignment.test.ts` 覆盖全分支：纯增/纯删/等行修改/old>new/new>old/多 hunk/空 hunks（Read 确认用例存在）且通过
- **CV-FE-09**：`src/panelRegistry.ts` 含 `PANEL_DIFF = "diff"`；`panelRegistry` 含 diff 键；`PANEL_TYPES` 五类型齐全（terminal/editor/htmlviewer/gitshow/diff）；`src/panels/index.ts` 导出 DiffPanel；`panel-registry.test.ts` 断言更新且通过
- **CV-FE-10**：`src/__tests__/diff-panel.test.tsx` 存在且覆盖：滚动同步（对侧 scrollTop 跟随 + 防循环）/ 保存后重新调 gitDiff 并刷新 gutter（Read 确认测试语义）；全量测试通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
