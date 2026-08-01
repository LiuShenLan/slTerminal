# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

Claude Code 历史会话查询与恢复——历史区 UI 与数据层。宿主为 agent 侧栏视图（`AgentStatusView` 三下拉框：活跃会话 + 当前项目历史会话 + 全部项目历史会话），数据经 `src/ipc/claudeHistory` 的三命令（`claude_history_scan` / `claude_history_delete` / `claude_history_rename`）与后端 `src-tauri/src/claude_history` 交互。需求规格见 `docs/claude-history-view/README.md`（v1.1）。

## 架构决策

### 双行式行（FE-07）

`HistorySessionRow` 为纯受控展示组件（不碰 IPC）：行 1 = 粗体标题（`title` 为 null 时显示 sessionId 前 8 位）+ 右上角相对时间（`formatRelativeTime` 灰字）；行 2 = 首条 prompt 预览单行截断。状态标记：`active` → ⚡（运行中）、`orphan` → ✗（cwd 非 null 且 `cwdExists=false`）、`noCwd` → 不显示 ✗（恢复类操作禁用）。单击选中高亮 `EXPLORER_SELECTION_BG`（照 explorer 选中模型），双击/右键均回调 props 委托。配色全部 `theme/colors.ts` token（硬约束 #6）。

### 三下拉框结构（FE-08，宿主在 agentStatus 模块）

`AgentStatusView` 持有三区展开 state（默认：活跃展开、两历史区收起），经 props 传给 `ClaudeHistorySections`（受控区）。活跃区逻辑零改动（`useAgentStatus` + `AgentStatusRow` 不动）。**E2E 兼容红线**：`data-e2e="agent-status-view"` 根容器、`data-e2e="agent-status-row"`、标题栏 "AGENT STATUS"、空态文案「选择一个项目」「无运行中的 claude 会话」逐字保留。

### 数据流与刷新时机（FE-04，规格 4.3.5/4.5）

`useClaudeHistory` 状态机 `idle | loading | ready | error`（初始 idle 未扫描）：

- `scan()` 由历史区**首次展开**与手动刷新按钮（`agent-history-refresh`）触发；generation 防竞（`genRef`，照 `useFileTree` 模式）
- `removeLocal` / `updateLocalTitle` 纯本地即时刷新，**不触发重扫**（删除/重命名 IPC 由调用方先执行，成功后调本函数同步 UI）
- `activeIds` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算 ⚡ 集合），**不重扫**；卸载清理订阅
- `rootPath` 推导：activePageId → 所属 project（照 `useCommitStatus` 先例）；rootPath 变化**不自动重扫**（历史区数据与项目弱相关，仅影响「当前项目」过滤——`isCurrentProject` 重算即可）

### 纯函数模型（FE-05，`historyModel.ts`）

零 React 依赖，全部展示派生集中于此（供 UI 与 L2 测试共用）：

- `isCurrentProject(cwd, rootPath)`：`normalizePath`（反斜杠→`/`）+ `toLowerCase()` 后**精确相等**（决策 24）；任一侧 null/空串 → false
- `groupByCwd(sessions)`：分组键 = 规范化 cwd（同目录不同写法归一组）；无 cwd 归 `UNKNOWN_CWD_KEY`（null）组（展示文案「(未知目录)」由 UI 层负责）；组内 mtimeMs 降序，组间按组内最大 mtimeMs 降序
- `matchesSearch`：标题 + firstPrompt 大小写不敏感 includes；query 空白 → 恒 true
- `formatRelativeTime`：六档——刚刚（<1min）/ N 分钟前 / N 小时前 / N 天前（<7d）/ 同年 `MM-DD` / 跨年 `YYYY-MM-DD`；mtimeMs ≤ 0 → 「-」（口径 = 文件 mtime，决策 26）
- `deriveActiveSessionIds()`：`TerminalRegistry.getAll()` 条目 → `claudeSession?.transcriptPath` basename 去 `.jsonl` → Set

### ⚡ 派生机制与其局限（FE-05，规格 4.1 两区关系）

⚡ 标记 = 历史区（磁盘扫描）与活跃区（TerminalRegistry）的重叠。`claudeSession` 存在即运行中（二态模型），取 `transcriptPath` 的 basename 定位对应历史行。**已知局限（文档化）**：仅本应用 spawn 且有 `transcriptPath` 的会话可标记——matchedCommand-only 会话（无 transcriptPath）无法定位 transcript 文件，⚡ 覆盖不到；外部终端开的 claude 完全不感知。历史区与活跃区数据同源重叠是设计语义（规格 4.1），非 bug。

### 四步恢复编排（FE-06，`restoreSession.ts`，决策 6/25）

`restoreHistorySession(session, { fork })` 全部复用既有原语，**零后端/workspace/stores 改动**：

1. **项目入列**：`useProjects.getState()` 查 rootPath 与 `session.cwd` 规范化相等（决策 24 同款）的项目，无则 `addProject`（字段形状照 `SidebarTree.handleAddProject` 现值）
2. **页面保障**：项目 `pages` 为空则 `addPage`（`页面-{Date.now()%10000}` + `makeEmptyLayout()` 空布局，照 `handleNewPage` 模式）
3. **页面切换**：`switchToPageShared(pages[0].pageId)`（`workspace/pageApis`；setProjectRoot 前置 await 由其内部保证，DBG-5）
4. **终端恢复**：轮询 `getPageApi`（100ms×50，照 `openHooksConfigPanel`）→ `addPanel({ id: "terminal-{pageId}-{Date.now()}", component: "terminal", title: "claude", params: { panelId, cwd }, renderer: "always" })` → 轮询 `TerminalRegistry.get(panelId)` → `pty.write(entry.sessionId, panelId, "claude --resume <id>" + (fork ? " --fork-session" : "") + "\r")`

- **防重入**：模块级 `restoring` 标记，进行中再次调用直接返回（快速双击同一行）
- **失败**：任何步骤异常 → `sendToastNotification("恢复会话失败", ...)` + console.error，不中断其他流程（场景 10）
- **前置拦截**：孤儿（`cwdExists=false`）/无 cwd 行的禁用判定由调用方（菜单/双击分派）负责；函数内 cwd 为 null 仍防御性 throw

### 操作矩阵（FE-07，规格 4.4，`historyContextMenu.ts`）

`getHistoryContextMenuItems(session, opts)` 策略函数（照 `commitContextMenu.ts` 策略模式），**差异**：本策略不直接做 IPC——四项操作的 action 由调用方（`HistorySessionList`）经 opts 回调注入（onCopy/onFork/onDelete/onRename），策略层只负责禁用态判定与菜单项构造：

| 操作 | 普通行 | 孤儿行 ✗ | 运行中行 ⚡ | 无 cwd 行 |
|------|--------|---------|------------|----------|
| 复制恢复命令（`cd '<dir>' && claude --resume <id>`，无 cwd 仅命令；`buildResumeCommand`） | ✓ | ✓ | ✓ | ✓ |
| 分支恢复（四步 + `--fork-session`） | ✓ | ✗ 禁用 | ✓ | ✗ 禁用 |
| 删除（`dialog.ask` 确认 → `deleteHistorySession` → `removeLocal`） | ✓ | ✓ | ✗ 禁用（句柄占用 + 幽灵文件） | ✓ |
| 重命名（`InputDialog` → `renameHistorySession` → `updateLocalTitle`） | ✓ | ✓ | ✓（追加写无冲突） | ✓ |

双击分派（`HistorySessionList`）：普通行 → 恢复四步；孤儿/无 cwd → 无操作；⚡ → `dialog.ask`「该会话已在运行中」→ 确认走 fork 恢复。`InputDialog` 为自绘输入弹窗（Tauri 原生 dialog 无输入框），受控 input + Enter 提交 / Esc 取消 / 空输入禁确认。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export（照 `commit/index.ts` 模式）：组合件/列表/行/弹窗/数据 hook/恢复编排/菜单策略/模型纯函数 |
| `ClaudeHistorySections.tsx` | 历史区组合件：搜索框（`agent-history-search`，过滤两区当前展开列表）+ 刷新按钮 + 两历史区（受控展开）；空态文案（当前项目无历史→「该项目暂无历史会话」/全部无→「暂无历史会话」/无活跃项目→「无活跃项目」/搜索无结果→「无匹配的会话」）；历史区首次展开触发 scan() |
| `HistorySessionList.tsx` | 列表：current 平铺（`isCurrentProject` 过滤，mtime 降序）/ all 分组（`groupByCwd` 二级折叠，组标题 = basename + title 悬停完整路径）；双击三分派 + 右键菜单调用方（删除/重命名完成回调 removeLocal/updateLocalTitle 经 props 注入） |
| `HistorySessionRow.tsx` | 双行式行组件（纯受控展示）：⚡/✗ 标记、单击选中、双击/右键回调委托 |
| `InputDialog.tsx` | 自绘输入弹窗：受控 input、Enter/Esc、空输入禁确认 |
| `historyContextMenu.ts` | 右键菜单策略：`getHistoryContextMenuItems`（禁用态矩阵）+ `buildResumeCommand`（复制命令构造） |
| `historyModel.ts` | 纯函数模型：`isCurrentProject` / `groupByCwd` / `matchesSearch` / `formatRelativeTime` / `deriveActiveSessionIds` + `UNKNOWN_CWD_KEY` |
| `useClaudeHistory.ts` | 数据 hook：状态机 + scan/removeLocal/updateLocalTitle + TerminalRegistry 订阅 ⚡ + rootPath 推导（照 `useCommitStatus`） |
| `restoreSession.ts` | 四步恢复编排：`restoreHistorySession(session, {fork})` + `waitFor` 轮询（100ms×50）+ 防重入 + 失败 toast |

## 测试模式

L2 测试位于 `src/__tests__/`，命名规则 `claude-history-*.test.ts(x)` + `ipc-claude-history-contract.test.ts`（8 文件，用例数见 `.claude/test-inventory.md`）：

| 文件 | 覆盖范围 |
|------|---------|
| `claude-history-model.test.ts` | 纯函数全分支：isCurrentProject（大小写/斜杠/null/空串/前缀不匹配）、groupByCwd（排序/未知组/空数组）、matchesSearch（大小写/空白）、formatRelativeTime（六档边界 + 跨年 + mtime=0）、deriveActiveSessionIds（有/无 transcriptPath/空注册表） |
| `claude-history-hook.test.tsx` | 状态机流转、scan 成功/失败、removeLocal/updateLocalTitle 不触发 scan、subscribe 驱动 ⚡ 更新、卸载清理 |
| `claude-history-restore.test.ts` | 四步编排（mock stores/projects、workspace/pageApis、ipc/pty、TerminalRegistry、ipc/notification）：已开项目跳过入列/无页建页/切页/addPanel 参数（cwd/id 格式）/pty.write 内容（普通/fork/`\r` 结尾）/防重入/失败 toast/无 cwd 防御性 throw |
| `claude-history-row.test.tsx` | 双行渲染、⚡/✗、单击选中、双击三分派、右键回调 |
| `claude-history-view.test.tsx` | 三区结构、默认展开态、展开触发 scan、搜索过滤、空态文案、菜单可用性矩阵（普通/孤儿/⚡/无 cwd × 4 操作）、双击分派 |
| `claude-history-input-dialog.test.tsx` | 受控输入/Enter/Esc/空禁确认 |
| `ipc-claude-history-contract.test.ts` | 三命令 × 四维验证（命令名 / 参数结构 camelCase / 正常返回 / 异常传播），照 `ipc-hooks-config-contract.test.ts` 模式 |
| `agent-status-view.test.tsx`（既有，agentStatus 侧） | AgentStatusView 三下拉框适配（保留活跃区断言） |

### 测试模式要点

- **纯函数层零 mock**：`historyModel` 直接调用断言（`deriveActiveSessionIds` 除外——mock `TerminalRegistry` 供读注册表）
- **hook/编排层 mock 边界**：mock 只守 JS 侧形状，真实编排由 L4 E2E 兜底（`claude-history-restore.test.ts` mock 四个模块）
- **E2E 兼容断言**：`claude-history-view.test.tsx` 中保留 E2E 红线（`data-e2e` 属性与空态文案）相关断言

### 运行

```bash
npx vitest run claude-history      # 仅历史会话模块测试
npm test                           # L2 全量
```
