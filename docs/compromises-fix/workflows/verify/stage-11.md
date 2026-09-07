# Stage 11 逐项验证断言（唯一真值源）

> stage-11 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：panelId 页前缀协议先行落地+测试，再改消费点。

## 断言清单

- **CP-004-a（多实例消亡）**：`rg "MAX_PAGES|initializedPages|ensurePageInitialized" src/` 零命中；Workspace 渲染单一 `<DockviewReact>`（Read src/workspace/Workspace.tsx 确认无多实例 map）；`src/workspace/CLAUDE.md:15` 笔误已改 ADR-0009（Read 确认）。
- **CP-004-b（页组模型）**：`src/workspace/pageGroups.ts` 存在且含纯函数族（pageGroupId/panelIdInPage/pageOfPanelId/panelsOfPage，Read 确认）；跨页拖拽回迁校验存在（panelBelongsToGroup 或等价，越界即回迁原页组，Read 确认）；panelId 页前缀协议消费点已逐点核改（checklist 冲击面：titleManager/tabClose 判据/TerminalRegistry 键/SEC-08 归属校验/DefaultTab params/findExistingEditor——Read 抽查 ≥3 处）。
- **CP-004-c（布局单点不破）**：layoutSerde 仍是 Dockview toJSON/fromJSON 唯一存取点（`rg "toJSON|fromJSON" src/ --include=*.ts` 命中仅 layoutSerde.ts，硬约束 #7）；`patchLegacyLayout` 旧多实例格式迁移分支存在且有测试（归组成功/脏面板丢弃两分支，随全量覆盖）。
- **CP-004-d（生命周期契约）**：终端面板不随切页卸载（Read 确认 xterm open 一次约束注释保留）；`renderer="always"` 白名单语义不变（isAlwaysRenderPanel 行为回归由全量 npm test 覆盖）；页面删除先 kill 页组内终端再移除页组（Read 确认）。
- **CP-004-e（e2e 适配）**：`e2e-tests/CLAUDE.md:58` 句已改为「跨 spec 状态隔离」口径（Read 确认）；helpers.ts 三 helper 经 getPageApi 页组查询（Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
5. `npm run e2e`（H6 跨页存活、settings 跨页计数用例必须绿）

## 人工验证点

- 布局/面板全场景：开页/关页/跨页拖拽/重启恢复/存量布局迁移；双页各开终端+编辑器，切页 20 次往返，终端输出不丢、编辑器无重建闪屏。
