# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-02**：`grep -c 'data-e2e="projects-load-' src/App.tsx` ≥ 3（容器 `projects-load-error` + 按钮 `projects-load-retry` + `projects-load-continue-empty`）。
- **FE-02**：init 加载失败的 catch 分支内**不**调用 `markPersistenceReady()`、**不** `setReady(true)`（Read 确认语义：失败即停，仅 setProjectsLoadError + console.error）；`markPersistenceReady()` 只出现在加载成功路径与「以空状态继续」处理器中。
- **FE-02**：「slTerminal 启动中…」文本节点仍存在于 `projectsLoadError === null`（或等价条件）分支（Read 确认，防 startup-restore 用例 4/8 回归）。
- **FE-02**：E2E 分支（`VITE_E2E === "1"`）调用 `markLoadSucceeded()`（Read 确认）。
- **FE-02**：错误页样式经 theme token（`SECONDARY_BG`/`SEPARATOR_BG`/`DIM_FG`），无硬编码颜色字面值（Read 确认）。
- **FE-03**：`grep -n "未水合" src/panels/settings/SettingsPanel.tsx` 命中；门控行语义为 `ownProjectId === null && Object.keys(projects).length === 0` 时 return，且位于 firstRun 消费之前（Read 确认顺序）。
- **TE-01**：`grep -rln "markLoadSucceeded" src/__tests__/` 命中 ≥ 6（startup-restore / startup-store-fail-warn / close-handler / error-boundary / e2e-clipboard-helper 五文件 mock + projects.test.ts 用例引用）。
- **TE-01**：`startup-restore.test.ts` 原用例 3 已语义反转（Read 确认：loadAllProjects reject 后断言不 ready、markPersistenceReady 未调、console.error 被调、错误页渲染），且存在重试成功 / 以空状态继续两新用例。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
