# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-FE-01**：`src/workspace/titleManager.ts` 的 `EditorEntry` 含 `suffix?: string`；`registerEditor`/`getFileEditorTitle` 接受可选 suffix 并拼接到标题末尾；`recomputeTitles` 输出标题保留各 entry 的 suffix（Read 确认拼接逻辑存在）；`findExistingEditor` 语义（须 Read 代码确认，不限实现）：**传入 suffix 时仅匹配同 suffix 条目；不传时仅匹配无 suffix 条目**——特别确认 ExplorerPanel 现有调用 `findExistingEditor(activePageId, filePath)`（两参）不会命中带 suffix 的 git 页签
- **CV-FE-01 测试**：`src/__tests__/titleManager.test.ts` 存在 suffix 相关 describe：suffix 标题生成（冲突/无冲突）、recomputeTitles 保留后缀、findExistingEditor 带/不带 suffix 匹配隔离（Read 确认测试存在且全量测试通过）
- **CV-FE-02**：`src/features/sideViews/sideBarState.ts` 的 `DEFAULT_ZONES` 为 `{ top: ["projects", "explorer", "commit"], bottom: [] }`（Read 确认）；grep 全仓 `["projects", "explorer"]` 旧默认值断言零残留（测试文件中的旧断言已全部更新）
- **CV-FE-02 测试**：sideBarState.test.ts / sideBar.test.ts / workspace-sideviews.test.tsx 中默认值相关断言与现值一致，全量测试通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
