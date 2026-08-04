# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

Claude Code 历史会话查询与恢复——历史区 UI 与数据层。宿主为 agent 侧栏视图（`AgentStatusView` 三下拉框：活跃会话 + 当前项目历史会话 + 全部项目历史会话），数据经 `src/ipc/claudeHistory` 的两命令（`claude_history_scan` / `claude_history_delete`）与后端 `src-tauri/src/claude_history` 交互。**重命名功能已整体移除**（问题 7 修复：前端 UI/菜单、IPC wrapper、后端命令全链路删除，官方 `/rename` 是 custom-title 唯一写入方）。需求规格文档已随 `docs/` 清理删除，规格要点由本文件架构决策与 `src-tauri/src/claude_history/CLAUDE.md` 承载。

## 架构决策

### 四态同源（问题 2 修复）

历史区行状态与活跃区**同源**（TerminalRegistry）：hook 事件到达时 `useXterm` 经 `setClaudeSession({ sessionId, transcriptPath, status })` 写入四态（`eventToStatus` 结果；null 状态不传，undefined 保留旧值）；`deriveActiveSessionStatuses()` 纯函数派生 `Map<sessionId, ClaudeStatus>`（sessionId 优先，回退 transcriptPath basename 去 `.jsonl` 兼容旧数据；matchedCommand-only 会话无两者可定位——文档化局限）。`useClaudeHistory.activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算，不重扫）；`HistorySessionRow` 按 `status` 渲染 `STATUS_EMOJI`（⚡🟡✅❌，null → 无标记），与活跃区 `getStatusIcon` 展示一致。

### 双行式行（FE-07）与三级字号层级（问题 1/4 修复）

- `HistorySessionRow`：行1 = 四态标记 + 粗体标题（12px）+ 右上角相对时间（`formatRelativeTime` 灰字）；行2 = 首条 prompt 预览单行截断（11px）。状态标记：`status` 非 null → 对应 emoji；`orphan` → ✗（cwd 非 null 且 `cwdExists=false`）；`noCwd` → 不显示 ✗（恢复类操作禁用）。配色全部 `theme/colors.ts` token（硬约束 #6）。
- `AgentStatusRow`（活跃区）：行1 = 四态图标 + 标题（12px 粗体）；行2 = 用量条 + 百分比 + 相对时间（11px 灰，缩进对齐图标列）。时间口径与历史区统一（`formatRelativeTime`，问题 1 修复——旧为 toLocaleTimeString 同行挤压导致窄侧栏遮挡）。
- **三级字号递减**（问题 4）：折叠框名（区块标题）13px 粗体 > 会话标题 12px 粗体 > 第二行 11px 灰。树形引导线：区块内容 `paddingLeft: 12px` + 左侧 1px 竖线（`SIDEBAR_COLORS.treeGuide`）；全部区组内容再缩进 12px + 二级竖线。

### 三下拉框结构（FE-08，宿主在 agentStatus 模块）

`AgentStatusView` 持有三区展开 state（默认：活跃展开、两历史区收起），经 props 传给 `ClaudeHistorySections`（受控区）。**E2E 兼容红线**：`data-e2e="agent-status-view"` 根容器、`data-e2e="agent-status-row"`、标题栏 "AGENT STATUS"、空态文案「选择一个项目」「无运行中的 claude 会话」逐字保留。

### 数据流与刷新时机（FE-04，规格 4.3.5/4.5；问题 6 修复）

`useClaudeHistory` 状态机 `idle | loading | ready | error`（初始 idle 未扫描），**上提至 AgentStatusView 单实例**（问题 6 修复——活跃区标题与历史区数据同源）：

- `scan()` 由历史区**首次展开**与手动刷新按钮（`agent-history-refresh`）触发；generation 防竞（`genRef`，照 `useFileTree` 模式）
- `removeLocal` 纯本地即时刷新，**不触发重扫**（删除 IPC 由调用方先执行，成功后调本函数同步 UI）
- `activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算四态映射），**不重扫**；卸载清理订阅
- `rootPath` 推导：activePageId → 所属 project（照 `useCommitStatus` 先例）；rootPath 变化**不自动重扫**（历史区数据与项目弱相关，仅影响「当前项目」过滤——`isCurrentProject` 重算即可）
- **活跃区标题覆盖**：`AgentStatusView` 渲染活跃行前经 `titleBySessionId`（sessions 中 title 非 null 者）覆盖 `row.title`——`/rename` 写 transcript custom-title 后点刷新，scan 结果即为新标题，活跃区自动同步；hook 事件 setRows 的 resolveTitle 值被视图层覆盖。无匹配 sessionId 或标题为 null → 回退行原标题（dockview 面板标题）

### 纯函数模型（FE-05，`historyModel.ts`）

零 React 依赖，全部展示派生集中于此（供 UI 与 L2 测试共用）：

- `isCurrentProject(cwd, rootPath)`：`normalizePath`（反斜杠→`/`）+ `toLowerCase()` 后**精确相等**（决策 24）；任一侧 null/空串 → false
- `groupByCwd(sessions)`：分组键 = 规范化 cwd（同目录不同写法归一组）；无 cwd 归 `UNKNOWN_CWD_KEY`（null）组（展示文案「(未知目录)」由 UI 层负责）；组内 mtimeMs 降序，组间按组内最大 mtimeMs 降序
- `matchesSearch`：标题 + firstPrompt 大小写不敏感 includes；query 空白 → 恒 true
- `formatRelativeTime`：六档——刚刚（<1min）/ N 分钟前 / N 小时前 / N 天前（<7d）/ 同年 `MM-DD` / 跨年 `YYYY-MM-DD`；mtimeMs ≤ 0 → 「-」（口径 = 文件 mtime，决策 26）
- `deriveActiveSessionStatuses()`：`TerminalRegistry.getAll()` 条目 → `Map<sessionId, ClaudeStatus>`（sessionId 优先 / transcriptPath basename 去 `.jsonl` 回退 / 双无跳过 / status 为 null 不产出键——与活跃区 null 无图标语义一致）

### 组默认收起 + 计数（问题 3 修复）

`HistorySessionList` 全部项目区组折叠用 **expandedGroups 白名单**（初始空 = 默认收起；黑名单模型已废弃）；组标题 = `basename(cwd)` + `(N)` 计数（含「(未知目录)」组）。

### 四步恢复编排（FE-06，`restoreSession.ts`，决策 6/25）

`restoreHistorySession(session, { fork })` 全部复用既有原语，**零后端/workspace/stores 改动**：

1. **项目入列**：`useProjects.getState()` 查 rootPath 与 `session.cwd` 规范化相等（决策 24 同款）的项目，无则 `addProject`（字段形状照 `SidebarTree.handleAddProject` 现值）
2. **页面保障**：项目 `pages` 为空则 `addPage`（`页面-{Date.now()%10000}` + `makeEmptyLayout()` 空布局，照 `handleNewPage` 模式）
3. **页面切换**：`switchToPageShared(pages[0].pageId)`（`workspace/pageApis`；setProjectRoot 前置 await 由其内部保证，DBG-5）
4. **终端恢复**：轮询 `getPageApi`（100ms×50，照 `openHooksConfigPanel`）→ `addPanel({ id: "terminal-{pageId}-{Date.now()}", component: "terminal", title: "claude", params: { panelId, cwd }, renderer: "always" })` → 轮询 `TerminalRegistry.get(panelId)` → `pty.write(entry.sessionId, panelId, "claude --resume <id>" + (fork ? " --fork-session" : "") + "\r")`

- **防重入**：模块级 `restoring` 标记，进行中再次调用直接返回（快速双击同一行）
- **失败**：任何步骤异常 → `sendToastNotification("恢复会话失败", ...)` + console.error，不中断其他流程（场景 10）
- **前置拦截**：孤儿（`cwdExists=false`）/无 cwd 行的禁用判定由调用方（菜单/双击分派）负责；函数内 cwd 为 null 仍防御性 throw

### 操作矩阵（FE-07，规格 4.4，`historyContextMenu.ts`；重命名已移除）

`getHistoryContextMenuItems(session, opts)` 策略函数（照 `commitContextMenu.ts` 策略模式），**差异**：本策略不直接做 IPC——三项操作的 action 由调用方（`HistorySessionList`）经 opts 回调注入（onCopy/onFork/onDelete），策略层只负责禁用态判定与菜单项构造：

| 操作 | 普通行 | 孤儿行 ✗ | 运行中行 | 无 cwd 行 |
|------|--------|---------|----------|----------|
| 复制恢复命令（`cd '<dir>' && claude --resume <id>`，无 cwd 仅命令；`buildResumeCommand`） | ✓ | ✓ | ✓ | ✓ |
| 分支恢复（四步 + `--fork-session`） | ✓ | ✗ 禁用 | ✓ | ✗ 禁用 |
| 删除（`dialog.ask` 确认 → `deleteHistorySession` → `removeLocal`） | ✓ | ✓ | ✗ 禁用（句柄占用 + 幽灵文件） | ✓ |

### 双击分派（问题 5 修复）与动作弹窗

`HistorySessionList` 双击三分支：普通行 → 恢复四步；孤儿/无 cwd → 无操作；**运行中（status 非 null）→ `SessionActionDialog` 弹窗**（「切换到该会话操作页面」/「取消」；**分支恢复仅保留在右键菜单**——Tauri 原生 dialog 无法三按钮，自绘模态照 InputDialog 模式，`data-e2e="agent-history-action-dialog"`）。「切换到该会话操作页面」= 反查 `TerminalRegistry.getAll()`（`claudeSession.sessionId` 精确匹配，回退 transcriptPath basename）→ `parseTerminalPageId(panelId)` → `switchToPageAndFocus(pageId, panelId)`（内部：activePageId 相同则直接聚焦）；反查不到 → `sendToastNotification` 提示（会话已结束）。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export（照 `commit/index.ts` 模式）：组合件/列表/行/弹窗/数据 hook/恢复编排/菜单策略/模型纯函数 |
| `ClaudeHistorySections.tsx` | 历史区组合件（**受控组件**——问题 6 修复：useClaudeHistory 上提至 AgentStatusView，数据/回调经 props 注入）：搜索框（`agent-history-search`）+ 刷新按钮 + 两历史区（受控展开）；空态文案（当前项目无历史→「该项目暂无历史会话」/全部无→「暂无历史会话」/无活跃项目→「无活跃项目」/搜索无结果→「无匹配的会话」）；历史区首次展开触发 scan()；区块标题 13px 粗体 + 内容缩进引导线 |
| `HistorySessionList.tsx` | 列表：current 平铺（`isCurrentProject` 过滤，mtime 降序）/ all 分组（`groupByCwd` 二级折叠，组标题 = basename + (N) 计数，**组默认收起**——expandedGroups 白名单，问题 3）；双击三分派（运行中 → SessionActionDialog）+ 右键菜单调用方（删除完成回调 removeLocal 经 props 注入） |
| `HistorySessionRow.tsx` | 双行式行组件（纯受控展示）：**四态 status**（⚡🟡✅❌，问题 2 同源）/✗ 标记、12px 粗体标题、11px prompt、单击选中、双击/右键回调委托 |
| `SessionActionDialog.tsx` | 动作弹窗（问题 5 新建，照 InputDialog 样式）：标题 + 消息 + 竖排动作按钮 + 取消；Esc/遮罩点击取消 |
| `historyContextMenu.ts` | 右键菜单策略：`getHistoryContextMenuItems`（禁用态矩阵，重命名项已移除）+ `buildResumeCommand`（复制命令构造） |
| `historyModel.ts` | 纯函数模型：`isCurrentProject` / `groupByCwd` / `matchesSearch` / `formatRelativeTime` / `deriveActiveSessionStatuses` + `UNKNOWN_CWD_KEY` |
| `useClaudeHistory.ts` | 数据 hook（上提至 AgentStatusView）：状态机 + scan/removeLocal + TerminalRegistry 订阅四态映射 + rootPath 推导（照 `useCommitStatus`） |
| `restoreSession.ts` | 四步恢复编排：`restoreHistorySession(session, {fork})` + `waitFor` 轮询（100ms×50）+ 防重入 + 失败 toast |

## 测试模式

L2 测试位于 `src/__tests__/`，命名规则 `claude-history-*.test.ts(x)` + `ipc-claude-history-contract.test.ts`（7 文件，用例数见 `.claude/test-inventory.md`）：

| 文件 | 覆盖范围 |
|------|---------|
| `claude-history-model.test.ts` | 纯函数全分支：isCurrentProject（大小写/斜杠/null/空串/前缀不匹配）、groupByCwd（排序/未知组/空数组）、matchesSearch（大小写/空白）、formatRelativeTime（六档边界 + 跨年 + mtime=0）、deriveActiveSessionStatuses（sessionId 优先/basename 回退/双无跳过/status null 不产出/四态透传/空注册表） |
| `claude-history-hook.test.tsx` | 状态机流转、scan 成功/失败、removeLocal 不触发 scan、subscribe 驱动 activeStatuses 更新、卸载清理 |
| `claude-history-restore.test.ts` | 四步编排（mock stores/projects、workspace/pageApis、ipc/pty、TerminalRegistry、ipc/notification）：已开项目跳过入列/无页建页/切页/addPanel 参数（cwd/id 格式）/pty.write 内容（普通/fork/`\r` 结尾）/防重入/失败 toast/无 cwd 防御性 throw |
| `claude-history-row.test.tsx` | 双行渲染、**四态标记**（working→⚡/attention→🟡/done→✅/error→❌/null 无标记）、✗ 孤儿标记、**字号断言**（行1 12px 粗体/行2 11px）、单击选中、双击/右键回调 |
| `claude-history-view.test.tsx` | **受控 props 注入**（sections 不再内部调 useClaudeHistory）、搜索过滤、**组默认收起+计数**、**双击运行中 → SessionActionDialog（切换反查 → switchToPageAndFocus 参数/反查不到 toast/取消关闭，无分支恢复）**、菜单矩阵（3 项，无重命名）、AgentStatusView 集成（**标题覆盖**：同 sessionId 磁盘标题覆盖 row.title/无匹配回退/标题 null 不覆盖；区块标题 13px 粗体 + 引导线样式）、空态文案、E2E 红线 |
| `claude-history-action-dialog.test.tsx` | SessionActionDialog：标题/消息/动作按钮渲染、action 回调、取消（按钮/Esc/遮罩）、空 actions 防御 |
| `ipc-claude-history-contract.test.ts` | 两命令 × 四维验证（命令名 / 参数结构 camelCase / 正常返回 / 异常传播），经共享工厂 `describeIpcContract`（`helpers/ipc-contract.ts`，IHE-06）声明式驱动——8 条用例，mockIPC 盲区声明见 `src/ipc/CLAUDE.md` |

> 另有 `agent-status-view.test.tsx`（agentStatus 侧）覆盖 AgentStatusRow 双行布局与 AgentStatusView 三区结构；`agent-status-hook.test.ts` 覆盖 useAgentStatus sessionId 字段；`terminal-registry.test.ts` / `use-xterm-lifecycle.test.ts` 覆盖 claudeSession sessionId/status 存储与 hook 事件写入（HUK1-9）。

### 测试模式要点

- **纯函数层零 mock**：`historyModel` 直接调用断言（`deriveActiveSessionStatuses` 除外——mock `TerminalRegistry` 供读注册表）
- **hook/编排层 mock 边界**：mock 只守 JS 侧形状，真实编排由 L4 E2E 兜底（`claude-history-restore.test.ts` mock 四个模块）
- **受控组件测试**：ClaudeHistorySections 受控后测试直接注入 props（sessions/activeStatuses/rootPath/scan/removeLocal），scan 经 props mock 断言
- **E2E 兼容断言**：`claude-history-view.test.tsx` 中保留 E2E 红线（`data-e2e` 属性与空态文案）相关断言

### 运行

```bash
npx vitest run claude-history      # 仅历史会话模块测试
npm test                           # L2 全量
```
