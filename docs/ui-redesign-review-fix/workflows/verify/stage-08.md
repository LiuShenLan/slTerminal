# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律：本 Stage 只改测试与 e2e 辅助代码——`git diff` 确认 src/ 生产代码零变更（src/__tests__/ 除外）。

## 断言清单

- **TE-01**：`nav-tree.test.tsx` 父节点因子用例的查询词仅命中项目名（须 Read 种子数据确认查询词不再同时命中页面名），断言页面行因父命中而渲染；用例通过
- **TE-02**：`nav-tree-history.test.tsx` 重扫次数断言不再使用 `toBeGreaterThanOrEqual`（grep 零命中该用例处），改精确次数/差值断言（须 Read 确认）；用例通过
- **TE-03**：`grep -r "waitForPanelTabIcon" e2e-tests/` 零命中；`waitForPanelTabStatus` 在 specUtils.ts 定义且 mockcli.e2e.ts/hooks.e2e.ts 调用点全部更名（grep 命中 6 处调用 + 1 处定义）；注释术语为 tabStatus/StatusDot
- **TE-04**：`sideBarState.test.ts` 测试数据无 `"projects"` 视图 id（grep 零命中——注意区分注释中的历史叙述）；用例通过
- **TE-05**：`activityBar.test.tsx` :276-280,503-512,514-528 三处不再以「不抛异常」收尾（须 Read 确认每处有 dropIndicator/事件次数/样式实断言）；用例通过
- **TE-06**：`workspace-page-dockview.test.tsx` FileIcon 用例含 FileIcon 特征断言（非仅 width=14 的 svg 存在性）+ terminal 面板不渲染 FileIcon 反向用例（须 Read 确认）；用例通过
- **Stage 08 纪律**：`git diff` 中 src/ 生产代码文件（src/__tests__/ 以外）零变更

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run e2e`
