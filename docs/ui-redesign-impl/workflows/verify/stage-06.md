# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **NAV-01**：`src/features/navTree/` 目录存在（NavTree.tsx + 行组件 + hook）；Read 确认层级恰为 项目→页面→会话 三级、行高 28/30px、每级左缩 15px、引导线/选中/hover 均经 token 引用
- **NAV-02**：Read 确认活跃会话行 = StatusDot + logo 14px + 标题 + 32x3 迷你用量条 + 百分比；行归属经 panelId→pageId（Read 取值来源，不接受 cwd 猜测）；点击聚焦逻辑自既有跳转迁移
- **NAV-03**：Read 确认历史折叠节点挂项目下（cwd 前缀匹配项目 rootPath）、计数 pill、展开行为单行历史行、prompt 预览在 title 属性
- **NAV-04**：grep `搜索项目 / 页面 / 会话` 命中 navTree 源文件；Read 确认过滤逻辑（子串不区分大小写、父节点因子显示）
- **NAV-05**：`src/features/sideViews/sideViewDefs.ts` 恰 3 条注册（nav/explorer/commit）；grep `ACTIVITY_BAR_SIZE = 46` 命中 sideBarState.ts；ActivityBar 底部配置钮（grep `activity-btn-config` 命中）且不入注册表
- **NAV-06**：`grep -r "SidebarTree" src/ --include=*.ts --include=*.tsx` 零命中（组件已删除且代码引用清零；CLAUDE.md 文档字样归 Stage 09）；grep `打开 Hooks 配置` 在 `src/`（`--include=*.ts --include=*.tsx`）零命中；「打开 Hooks 配置」公共函数存在且被配置钮调用（Read 确认调用链）
- **NAV-07**：Read `src/stores/sideBar.ts` 确认 open 字段指向未知 id 时置 null（sanitize/reconcile 语义覆盖 open）；对应 L2 用例存在
- **NAV-08**：`grep -r "AgentStatusView\|AgentStatusRow\|AgentHistorySections" src/ --include=*.ts --include=*.tsx` 零命中（被删组件引用清零；useAgentStatus.ts 保留不受此断言约束）；grep `agent-status` 在 `src/`（`--include=*.ts --include=*.tsx`）与 `e2e-tests/` 零命中（视图 id 已退役）
- **NAV-09**：Read 确认「当前」pill（ACCENT_FG 字 10px）与计数 pill 存在
- **NAV-10**：`npm test` 与 `npm run e2e` 通过（依测试 agent 结果）；grep `nav-tree\|nav-row-session` 命中 e2e-tests 用例文件
- **NAV-11（人工验证点，verify agent 标注 skipped-manual）**：实机验证三级树展开折叠/会话跳转/历史恢复/搜索/配置钮/旧 settings.json（含 projects/agent-status 段）启动不崩——不纳入 allFixed 判定，details 注明人工验证

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
7. `npx vite build`
8. `npm run e2e`
