# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：Read `src/features/explorer/useFileTree.ts`——rootPath effect 在 loadRoot(gen) 前置 `restoringRef.current = true`；restoreExpanded 三分支（无快照/域不符/空集）均落 `restoringRef.current = false` + `commitViewState()`，deps 含 commitViewState。
- **FE-07**：Read 同文件 loadRoot——catch 双分支：续页失败（firstFrameCommitted=true）console.error 后 return（不记 dirErrors、不清空 rootNodes）；首帧失败分支含 `restoringRef.current = false`；`firstFrameCommitted` 旗标存在（rg 命中）。
- **FE-01/07（联动意图）**：不存在「续页失败清空已渲染首帧」路径（语义式——Read catch 全分支确认无 setRootNodes([]) / commitViewState 空提交）；`src/__tests__/use-file-tree.test.ts` 含 checklist 两条目测试同步节点名用例（Read 确认），npm test 绿（测试 agent 结果承载）。
- **FE-02**：`rg "PANEL_SETTINGS" src/panels/panelRegistry.ts` ≥ 3 命中（定义 + 两处引用）；Read 确认组件映射表键与 pageApis.ts 未改（收窄面零改动）。
- **FE-09**：Read `src/workspace/WorkspaceDockHost.tsx`——disposablesRef 存在、handleReady 赋值、组件级卸载 useEffect 消费（for dispose + 清空）；`src/__tests__/workspace-host-pages.test.tsx` 含 unmount 后 `__dockviewApi===undefined` + getPageApi null 断言用例（Read 确认），npm test 绿（测试 agent 结果承载）。
- **FE-10**：`rg "PageDockviewHost" src/ knip.json` 零命中（含注释与 knip ignore 键）；`src/workspace/tabChrome.tsx` 存在且头注自述 tabChrome 名随实（Read 确认）；`rg "tabChrome" knip.json` 命中（ignore 键同步）；`npx knip --production` 无新增红（测试 agent 结果承载）。
- **FE-11**：`rg "exportContextBindings|ExportedBinding" src/` 零命中（含 shortcuts/CLAUDE.md 残留——语义式：生产+测试+模块文档均无该家族存在）；Read `src/__tests__/shortcuts.test.ts` 确认 listCommands 用例保留（describe 已改名）；html-panel.test.tsx / markdown-panel.test.tsx 的对应 vi.mock 块已删（若保留须有生产消费证据——Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx knip --production`
