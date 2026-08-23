# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-A-04**：`src/workspace/PageDockviewHost.tsx` 含 `export function createWatermark`（grep 命中）；`src/__tests__/workspace-header-actions.test.tsx` import 生产 `createWatermark`（grep 命中）；文件内不存在手写 Watermark 组件（语义式：grep `const Watermark: React.FC` 应为 0，须 Read 确认 W1/W2/W3 渲染的是生产组件）。
- **TQ-A-05**：`src/__tests__/use-xterm-error-toast.test.ts`、`diff-panel.test.tsx`、`diff-panel-stale-banner.test.tsx` 的 `../lib` barrel mock 均为 `importOriginal` 形态（grep `importOriginal` 命中 3 文件）。
- **TQ-B-01**：`src/features/explorer/FileTree.tsx` 含 `data-testid="tree-node-row"`（grep 命中）；`explorer-virtualization.test.tsx` 的 `renderedRowCount` 经该 testid 计数（grep 命中）。
- **TQ-B-05**：`explorer-crud-success.test.tsx` 的 `rowBackground` 经 `tree-node-row` testid 限定（grep 命中）。
- **TQ-B-11**：`src/features/fileViewers/FileViewerRegistry.ts` 含 `export function registerDefaultViewers` 且模块级调用它完成预注册（Read 确认）；`file-viewer-registry.test.ts` import 同名函数作恢复（grep 命中，测试私有复制体已删）。
- **TQ-B-13**：`src/features/commit/CommitFileList.tsx` 含 `data-testid="commit-context-menu"`（grep 命中）；`commit-context-menu-ui.test.tsx` 菜单查询经该 testid（grep 命中）；不再含 `div[style*="position: fixed"]` 选择器（grep -c 为 0）。
- **TQ-B-17**：`src/features/explorer/FileTree.tsx` 两处内联 input 含 `data-testid="explorer-inline-input"`（grep 命中）；`explorer-delete.test.tsx`、`explorer-crud-success.test.tsx` 不再含 `inputs[inputs.length - 1]` 取尾形态（grep -c 为 0）。
- **生产微改边界（语义式）**：本 Stage 对生产文件（PageDockviewHost.tsx / FileTree.tsx / FileViewerRegistry.ts / CommitFileList.tsx）的 diff 仅含「加 export / 加 data-testid / 抽函数原样移动」，无任何逻辑语义改动（须 `git diff` 逐文件 Read 确认，防顺手改逻辑）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
