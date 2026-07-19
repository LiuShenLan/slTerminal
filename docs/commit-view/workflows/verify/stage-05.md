# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-FE-11**：`src/features/commit/CommitView.tsx` 存在；状态机 4 态文案精确（grep 命中）：`选择一个项目以查看变更` / `加载中…` / `当前项目并非 git 项目` / `无变更文件`；标题栏文本 `COMMIT`；两列表框标题含 `Changes (` 与 `Unversioned Files (` 计数格式；`data-e2e` 属性存在：`commit-view` / `commit-changes` / `commit-unversioned` / `commit-file-item`
- **CV-FE-11 硬约束**：`src/features/commit/` 目录内 grep `#[0-9a-fA-F]{6}` 零命中（颜色全 token）；文件名着色引用 `GIT_FILE_COLORS`（grep 命中）；grep `invoke` 零命中
- **CV-FE-12**：`src/features/commit/useCommitStatus.ts` 存在；含 `onFsEvent` 订阅与 200ms debounce（Read 确认）；rootPath 变化时旧数据清空 + 重载（Read 确认语义，不限实现——generation 或等价机制）
- **CV-FE-13**：`src/features/commit/openCommitFile.ts` 顶部导出状态→面板映射常量，四类齐全且与契约一致：`added→editor+"(git add)"`、`untracked→editor+"(git not add)"`、`deleted→gitshow+"(git delete)"`、`modified/renamed/conflict→diff+"(git diff)"`（Read 确认）；去重调用 `findExistingEditor(pageId, filePath, suffix)`（三参带 suffix）；renamed 时 params 含 `oldPath`；params 含 `repoPath`
- **CV-FE-14**：`src/features/sideViews/sideViewDefs.ts` 含 `id: "commit"` 注册（grep 命中），icon `🔀`，title `Commit`
- **CV-FE-15**：`src/__tests__/commit-view.test.tsx`（或拆分组）存在且覆盖：状态机 4 态 / 列表渲染（颜色/计数/排序/空态）/ 折叠交互 / 双击分派 4 类（component/title/params 断言）/ 去重聚焦 / B10 共存不误聚焦 / fs-event 刷新 / rootPath 切换重载（Read 确认测试语义）；全量测试通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
