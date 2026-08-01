# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **E2E 红线（FE-08 硬约束）**：`src/features/agentStatus/AgentStatusView.tsx` grep 全部命中——`data-e2e="agent-status-view"`、`data-e2e="agent-status-row"`（或该行组件 `AgentStatusRow.tsx` 中原样保留）、`AGENT STATUS`、`选择一个项目`、`无运行中的 claude 会话`。
- **FE-08**：`AgentStatusView.tsx` 含三个可展开/收起区块（活跃会话 + 当前项目历史会话 + 全部项目历史会话；Read 确认三标题文案）；默认态 = 活跃展开、两历史区收起（Read 确认初始 state）；历史区首次展开调用 `scan()`（Read 确认）；活跃区仍使用 `useAgentStatus` + `AgentStatusRow`（Read 确认 import 未变——活跃逻辑零改动）。
- **FE-07**：`ClaudeHistorySections.tsx` / `HistorySessionList.tsx` / `HistorySessionRow.tsx` / `historyContextMenu.ts` / `InputDialog.tsx` 五文件存在；搜索框位于两个历史下拉框之上（Read 组件结构确认）；`HistorySessionList` 全部项目区为二级折叠、组标题 basename + title 悬停完整路径（Read 确认）；`HistorySessionRow` 双行式（行1 标题+时间、行2 prompt 单行截断；Read 确认结构与 `text-overflow: ellipsis` 或等效）；`historyContextMenu.ts` 的 `getHistoryContextMenuItems` 签名与 stages.md 契约段逐字一致（Read 对照）。
- **操作矩阵（语义式）**：菜单与双击分派中——孤儿行（orphan）恢复/分支恢复禁用；无 cwd 行（noCwd）恢复/分支恢复禁用；运行中行（active）删除禁用；复制命令与重命名全行可用（Read `historyContextMenu.ts` 与双击 handler 逐一确认四个禁用分支）；复制命令格式：有 cwd = `cd '<cwd>' && claude --resume <id>`、无 cwd = `claude --resume <id>`（Read 字符串构造确认）。
- **FE-09**：空态文案 grep 命中——`该项目暂无历史会话`、`暂无历史会话`、`无活跃项目`（存在于 Sections/List 文件）；搜索无结果有提示文案（Read 确认）。
- **FE-11**：本 Stage 新增/修改的 UI 文件中不存在硬编码色值（grep `#[0-9a-fA-F]{3}\b|#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{8}\b|rgb\(` 零命中）；颜色引用 `theme/colors`（grep `from` 行命中）；选中高亮使用 `EXPLORER_SELECTION_BG`（grep 命中）。
- **FE-12**：grep 命中全部 data-e2e 属性——`agent-history-search`、`agent-history-refresh`、`agent-history-section-current`、`agent-history-section-all`、`agent-history-group`、`agent-history-row`、`agent-history-menu`、`agent-history-input-dialog`。
- **组件契约一致（意图断言）**：`HistorySessionRow.tsx` 的 props 接口与 `InputDialog.tsx` 的 props 接口同 stages.md「跨 Stage 契约」段逐字一致（Read 对照，防两 agent 各自漂移）。
- **barrel**：`src/features/claudeHistory/index.ts` 存在且导出视图所需公共 API（Read 确认）。
- **测试**：`claude-history-row.test.tsx`、`claude-history-input-dialog.test.tsx`、`claude-history-view.test.tsx` 三新文件存在；`agent-status-view.test.tsx` 已更新适配三下拉框且断言活跃区行为（Read 确认）；view 测试含菜单可用性矩阵（普通/孤儿/⚡/无 cwd × 4 操作）与展开触发 scan 断言（Read 核对）。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
