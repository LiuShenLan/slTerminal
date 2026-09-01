# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

Agent 历史会话查询与恢复（CLI 无关聚合，MC-310 泛化）。**宿主 = 导航树历史折叠节点（NAV-03）**：`NavTree` 经 `useNavTree` 内建历史聚合（`useAgentHistory` 数据 + 按项目 cwd 归属归组），历史行 = `NavHistoryRow` 单行式。数据经 `src/ipc/agentHistory` 两命令（`agent_history_scan` / `agent_history_delete`）与后端 `src-tauri/src/agent_history` 交互。

**重命名功能已整体移除**（前端 UI/菜单、IPC wrapper、后端命令全链路删除，官方 `/rename` 是 custom-title 唯一写入方）。**`AgentHistorySections` / `HistorySessionList` / `HistorySessionRow` 已删除（NAV-08/FE-25）**——原三区结构随 `AgentStatusView` 退役，历史区迁入导航树。

## 关键约束与决策

### 四态同源（MC-313）

历史区行状态与活跃区同源（`TerminalRegistry`）：hook 事件到达时 `useXterm` 经 `setAgentSession` 写入四态；`deriveActiveSessionStatuses()` 派生 `Map<cliId|sessionId, AgentStatus>`。

- **复合键构造/解析唯一口径 = `keyOf(cliId, sessionId)` 单点**（ZQ-1/ZQ-7）：cliId 缺省回退 `CLAUDE_CLI_ID`；两侧竖线转义后拼接。
- sessionId 优先，回退 `usageSourcePath` basename 去 `.jsonl` 兼容旧数据；双无则跳过（matchedCommand-only 会话的文档化局限）。
- `useAgentHistory.activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随，不重扫。
- `NavHistoryRow` 按 `status` 渲染 `StatusDot`（运行中四态，无运行状态 → **恒渲染 done 灰档**——mockup `.dot.idle` 契约，NAV-10）。

### 历史行单行化（NAV-03）

导航树历史行 = 单行 30px：`StatusDot` + CLI logo 14px（按 `session.cliId` 查 `profile.iconSrc`，未注册不报错）+ 标题 + 右侧相对时间。prompt 预览 → 原生 `title` tooltip（双行式行2 退役）。

### 数据流与刷新时机（FE-04/FE-19/NAV-10，F12 订阅化）

`useAgentHistory` 状态机 `idle | loading | ready | error`（初始 idle 未扫描）——**sessions/state 真值源已上移 `backgroundTaskScheduler`（F12）**：本 hook 只订阅 `sessionRefresh` 任务快照（`TaskSnapshot<AgentHistorySession[]>`），状态机语义不变：

- **触发时机**：首个订阅者出现 → 立即执行一轮（接管「挂载即扫」语义）+ 按配置频率（`backgroundTasks.sessionRefresh.intervalSec`）定时刷新；最后订阅者退订 → 停 interval（调度器全局单例与 UI 解耦，NavTree 卸载无碍，ADR-0001）。
- **手动刷新** = `triggerNow()`（刷新钮）——与 tick 共用同一扫描执行体（规格 §1 单一执行体），仅失败处理策略不同（manual 失败置 error）。
- **force 恒 true**：扫描执行体（`sessionRefreshTask.ts`）遍历全部已注册 history provider 逐个 `scanAgentHistory(cliId, true)` 聚合——后端 `(目录 mtime, 文件数)` 缓存对进行中会话不敏感，手动与定时必须同一口径绕过缓存（空结果永久命中场景必须 bypass）。
- **scan 已退役**：`scan(force?)` 从 hook 返回面移除（无参导出早于 F12 已删），历史引用全部改 `triggerNow()`。
- `removeLocal` 经调度器 `applyLocal` 透传（删除会话后本地移除列表项，不重扫）。
- `activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随。
- `rootPath` 推导：activePageId → 所属 project；rootPath 变化不自动重扫。

### 纯函数模型（FE-05，`historyModel.ts`）

零 React 依赖，展示派生集中：

- `isCurrentProject(cwd, rootPath)`：规范化 + 忽略大小写后精确相等。
- `groupByCwd(sessions)`：规范化 cwd 分组；无 cwd 归 `UNKNOWN_CWD_KEY`；组内/组间 mtimeMs 降序。
- `matchesSearch`：标题 + firstPrompt 大小写不敏感 includes。
- `formatRelativeTime`：六档相对时间；mtimeMs ≤ 0 → 「-」。
- `keyOf(cliId, sessionId)`：复合键构造单点（回退 + 转义）。
- `deriveActiveSessionStatuses()`：`TerminalRegistry.getAll()` → `Map<cliId|sessionId, AgentStatus>`。

### 四步恢复编排（FE-06，`restoreSession.ts`）

`restoreHistorySession(session, { fork })` 全部复用既有原语，零后端/workspace/stores 改动：

1. 项目入列：`useProjects` 查 rootPath 与 `session.cwd` 规范化相等的项目，无则 `addProject`。
2. 页面保障：项目 pages 为空则 `addPage`（默认名 + `makeEmptyLayout()` 空布局）。
3. 页面切换：`switchToPageShared(targetPageId)`（setProjectRoot 前置 await 由其内部保证，DBG-5）。
4. 终端恢复：轮询 `getPageApi`（100ms×50）→ `addPanel(terminal, ...)` → 轮询 `TerminalRegistry.get(panelId)` → `pty.write` 注入 `profile.history.buildRestoreInput(session, { fork })`（MC-315 委托）。

其他约束：

- 初始标题 = `session.title ?? session.sessionId.slice(0, 8)`（人工验证问题 3）。
- **B14**：panelId 经生成单点 `makeTerminalPanelId`（`terminal-{pageId}-{seq}`，模块级每页计数与 `PageDockviewHost` 共享）——旧格式含 Date.now 数字段破坏解析（visible 恒 false 黑屏 + 幽灵页面导航）。
- **FE-27 可取消**：`waitFor` 接受 `AbortSignal`；模块级 `restoreAbortRef` Controller，新恢复发起时 abort 上一轮在途恢复。
- 防重入：模块级 `restoring` 标记。
- 失败：任何步骤异常 → `sendToastNotification("恢复会话失败", ...)` + console.error。
- 孤儿/无 cwd 行的禁用判定由调用方负责。

### 操作矩阵（FE-07，`historyContextMenu.ts`）

`getHistoryContextMenuItems(session, opts)` 策略函数不直接做 IPC，菜单 action 由调用方（`NavTree`）经 opts 回调注入：

| 操作 | 普通行 | 孤儿行 | 运行中行 | 无 cwd 行 |
|------|--------|--------|----------|----------|
| 复制恢复命令 | ✓ | ✓ | ✓ | ✓ |
| 分支恢复 | ✓ | 禁用 | ✓ | 禁用 |
| 删除 | ✓ | ✓ | 禁用 | ✓ |

- 复制恢复命令委托 `profile.history.buildResumeCommand`（无 history 能力 → 空串降级）。
- 分支恢复：依赖 `profile.history.supportsFork`，缺省 false → 不展示菜单项（MC-316）。
- 删除：`confirmDialog` 确认 → `agent_history_delete(cliId, sessionId)` → `removeLocal` 不重扫。

### 双击分派与动作弹窗

历史行双击三分支（`NavTree` 内实现）：普通行 → 恢复四步；孤儿/无 cwd → 无操作；运行中 → `SessionActionDialog` 弹窗（「切换到该会话操作页面」/「取消」；分支恢复仅保留在右键菜单）。

「切换到该会话操作页面」= 反查 `TerminalRegistry.getAll()`（`findPanelForSession`：复合键精确匹配）→ `findPageIdForPanelId`（B14：先按 `useProjects` 已知页面集合前缀匹配，兜底 `parseTerminalPageId`）→ `switchToPageAndFocus(pageId, panelId)`。

### 已知限制（MC-318）

1. **历史区相对时间无 ticker**：`formatRelativeTime` 在渲染时计算，历史区无定时重渲染机制——挂起的历史区相对时间文本不自动刷新，直至其他状态变更触发重渲染（视为可接受，不修）。

## 测试模式

- 纯函数全分支（含复合键构造/解析、四态派生）。
- 状态机、订阅首轮自动执行（挂载即扫语义）、triggerNow、removeLocal、subscribe 驱动 activeStatuses。
- 四步编排、可取消、防重入、失败 toast、无 history 能力防御。
- SessionActionDialog 弹窗行为。
- 两命令 × 四维契约验证，经 `helpers/ipc-contract.ts` 共享工厂。
- 导航树历史节点契约（NAV-03/10）。

## 运行

```bash
npx vitest run agent-history nav-tree-history
npm test
```
