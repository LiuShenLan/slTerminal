# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-39**（验证项，零代码改动）：grep `src/__tests__/nav-tree-history.test.tsx` 含「最深前缀」用例；`npx vitest run nav-tree-history` 退出码 0（verify agent 实跑取数）
- **FE-40**：`src/features/explorer/FileTree.tsx` 含滚动跟随 effect（语义断言：selectedPath 变化且对应行索引在渲染窗口 [start, end] 外时写 scrollTop 使其可见——Read 确认，不限变量名/Hook 形式）
- **FE-40**：`src/__tests__/explorer-virtualization.test.tsx` 含滚动跟随用例（grep 「scrollTop」命中），且全量 L2 通过
- **FE-41**：`src/features/explorer/useFileTree.ts` 的 `refreshSubtreeAt` 含「目标已删」分支（Read 确认：readDir 失败且 targetPath 非根路径时从树中递归移除该目录行；根路径被删走原 mergeLayer 路径不动）
- **FE-41**：`src/__tests__/use-file-tree.test.ts` 含目标已删用例（grep 「删除」或「missing」命中，Read 确认断言目录行消失），且全量 L2 通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
