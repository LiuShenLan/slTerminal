# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

Agent 历史会话查询与恢复——历史区 UI 与数据层（CLI 无关聚合，MC-310 泛化）。**宿主 = 导航树历史折叠节点（NAV-03，UI 重设计）**：`NavTree` 经 `useNavTree` 内建历史聚合（`useAgentHistory` 数据 + 按项目 cwd 归属归组），历史行 = `NavHistoryRow` 单行式；数据经 `src/ipc/agentHistory` 的两命令（`agent_history_scan` 无参聚合 / `agent_history_delete(cliId, sessionId)`）与后端 `src-tauri/src/agent_history` 交互。**重命名功能已整体移除**（问题 7 修复：前端 UI/菜单、IPC wrapper、后端命令全链路删除，官方 `/rename` 是 custom-title 唯一写入方）。**`AgentHistorySections` 已删除（NAV-08）**——原三区结构（活跃 + 当前项目历史 + 全部项目历史）随 `AgentStatusView` 退役，历史区迁入导航树。聚合层 CLI 无关——扫描结果由各 provider 打标 `cliId`，行 logo/恢复命令/恢复注入按 `session.cliId` 查 profile 策略委托（MC-311/315/316）；claude 数据路径语义保留在 claude provider（后端）+ `profiles/claude/` 策略（前端）。

## 架构决策

### 四态同源（问题 2 修复，复合键 MC-313）

历史区行状态与活跃区**同源**（TerminalRegistry）：hook 事件到达时 `useXterm` 经 `setAgentSession({ sessionId, usageSourcePath, status, cliId })` 写入四态（经 `profile.hooks.eventToStatus` 结果；null 状态不传，undefined 保留旧值）；`deriveActiveSessionStatuses()` 纯函数派生 **`Map<cliId|sessionId, AgentStatus>`**（复合键——sessionId 优先，回退 usageSourcePath basename 去 `.jsonl` 兼容旧数据；matchedCommand-only 会话无两者可定位——文档化局限；旧数据 agentSession 无 cliId → 按 `CLAUDE_CLI_ID` 常量回退，非字面量）。**复合键构造/解析唯一口径 = `keyOf(cliId, sessionId)` 单点**（ZQ-1/ZQ-7——cliId 缺省回退 + 竖线转义一处生效，生产/消费同函数；现状 cliId/sessionId 均不含竖线，转义对存量键零变化，纯防御未来）。`useAgentHistory.activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算，不重扫）；`NavHistoryRow` 按 `status` 渲染 `StatusDot` 圆点（working 绿/attention 黄/done 灰/error 红，null → **恒渲染 done 灰档**——mockup `.dot.idle` 契约，NAV-10）。

### 历史行单行化（NAV-03，决策 6）

导航树历史行 = **单行 30px**（原 `HistorySessionRow` 双行式行2 已退役）：`StatusDot`（运行中四态，无运行状态 → done 灰档）+ CLI logo 14px（`cliProfileRegistry.get(session.cliId)?.iconSrc`——**按行 cliId 查 profile**（MC-311），未注册 cliId → 无 logo 不报错）+ 标题（fg-1）+ 右侧相对时间（11px fg-4，`formatRelativeTime` 与历史区口径统一）。**prompt 预览 → 原生 `title` tooltip**（决策 6——双行式行2 改为 title 属性）。标题 null → sessionId 前 8 位。配色全部 `theme/colors.ts` token（硬约束 #6）。

**`HistorySessionList` / `HistorySessionRow` 已删除（FE-25）**：两组件原为 NAV-08 后「保留但无生产消费方」的退役组件，本次修复直接删除，`agentHistory/index.ts` 已不再导出两者；生产历史区由导航树 `NavHistoryRow` 承担，原行级测试语义迁移至 `nav-history-row.test.tsx`。

### 数据流与刷新时机（FE-04，规格 4.3.5/4.5；NAV-10 契约调整）

`useAgentHistory` 状态机 `idle | loading | ready | error`（初始 idle 未扫描），**消费方 = `useNavTree`**（导航树内建聚合，单实例）：

- **scan(force?) 触发时机（NAV-10 + FE-19）**：导航树**挂载即扫描**（历史折叠节点常驻项目下，计数 pill 与历史行首屏可见——计数需要数据；`useAgentHistory` generation 防竞兜底重复扫描）；**展开不再重复 scan（FE-19）**——BE-19 后端 (目录 mtime, 文件数) 缓存命中复用不重复读盘；手动刷新钮（导航头「刷新」）= `scan(true)` **已实现** force=true 强制重扫通道
- **单 CLI 扫描（契约断链接线修复）**：`scan(force?)` 调 `scanAgentHistory(CLAUDE_CLI_ID, force)`——后端 provider REGISTRY 当前仅 claude，单 cliId 扫描即全量（MC-312 聚合语义收敛到后端 cliId 分发，前端不再无参聚合；第二后端 provider 接入时重评估）；generation 防竞（`genRef`，照 `useFileTree` 模式）；**BE-19 缓存语义**：后端按 (目录 mtime, 文件数) 进程内缓存——目录内会话文件增删不影响根键（不触发自动重扫），由前端显式刷新（force=true）兜底
- `removeLocal` 纯本地即时刷新，**不触发重扫**（删除 IPC 由调用方先执行，成功后调本函数同步 UI——`NavTree` 历史行右键菜单删除走此路径）
- `activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算四态映射），**不重扫**；卸载清理订阅
- `rootPath` 推导：activePageId → 所属 project（照 `useCommitStatus` 先例）；rootPath 变化**不自动重扫**（历史区数据与项目弱相关）

### 纯函数模型（FE-05，`historyModel.ts`）

零 React 依赖，全部展示派生集中于此（供 UI 与 L2 测试共用）：

- `isCurrentProject(cwd, rootPath)`：`normalizePath`（反斜杠→`/`）+ `toLowerCase()` 后**精确相等**（决策 24）；任一侧 null/空串 → false
- `groupByCwd(sessions)`：分组键 = 规范化 cwd（同目录不同写法归一组）；无 cwd 归 `UNKNOWN_CWD_KEY`（null）组（展示文案「(未知目录)」由 UI 层负责）；组内 mtimeMs 降序，组间按组内最大 mtimeMs 降序。**同目录不同 CLI 同组**——行级 logo 区分（MC-312）
- `matchesSearch`：标题 + firstPrompt 大小写不敏感 includes；query 空白 → 恒 true
- `formatRelativeTime`：六档——刚刚（<1min）/ N 分钟前 / N 小时前 / N 天前（<7d）/ 同年 `MM-DD` / 跨年 `YYYY-MM-DD`；mtimeMs ≤ 0 → 「-」（口径 = 文件 mtime，决策 26）
- `keyOf(cliId, sessionId)`：**复合键构造/解析唯一口径**（ZQ-1/ZQ-7）——cliId 为 null/undefined 回退 `CLAUDE_CLI_ID`（旧数据兼容）；两侧各自 `replaceAll("|", "\\|")` 转义后拼接 `a|b` 形态返回；生产（拼接方）消费（查键/比较方）同函数即口径一致
- `deriveActiveSessionStatuses()`：`TerminalRegistry.getAll()` 条目 → `Map<cliId|sessionId, AgentStatus>`（复合键 MC-313：键经 `keyOf` 构造 / sessionId 优先 / usageSourcePath basename 去 `.jsonl` 回退 / 双无跳过 / status 为 null 不产出键——与活跃区 null 无图标语义一致）

### 四步恢复编排（FE-06，`restoreSession.ts`，决策 6/25）

`restoreHistorySession(session, { fork })` 全部复用既有原语，**零后端/workspace/stores 改动**：

1. **项目入列**：`useProjects.getState()` 查 rootPath 与 `session.cwd` 规范化相等（决策 24 同款）的项目，无则 `addProject`（字段形状照 `NavTree.handleAddProject` 现值）
2. **页面保障**：项目 `pages` 为空则 `addPage`（`页面-{Date.now()%10000}` + `makeEmptyLayout()` 空布局——makeEmptyLayout 迁自 SidebarTree，NAV-06 后由 navTree 导出）
3. **页面切换**：`switchToPageShared(pages[0].pageId)`（`workspace/pageApis`；setProjectRoot 前置 await 由其内部保证，DBG-5）
4. **终端恢复**：轮询 `getPageApi`（100ms×50，照 `openHooksConfigPanel`）→ `addPanel({ id: makeTerminalPanelId(targetPageId), component: "terminal", title: session.title ?? profile.tabTitle, params: { panelId, cwd }, renderer: "always" })` → 轮询 `TerminalRegistry.get(panelId)` → `pty.write` 注入 `profile.history.buildRestoreInput(session, { fork })`（MC-315 委托——注入内容含 fork 追加与 `\r` 结尾，由各 CLI 的 history 能力实现负责）。**初始标题 = session.title ?? session.sessionId.slice(0, 8)（人工验证问题 3）**——直接用历史会话标题（回退链合成结果），读不到兜底 = sessionId 前 8 位（与 NavHistoryRow/useXterm 同步兜底同口径）；运行中由 SessionStart 异步标题通道保持同步。**B14：panelId 经生成单点 `makeTerminalPanelId`（`terminal-{pageId}-{seq}`，模块级每页计数与 PageDockviewHost 共享）**——旧格式 `terminal-{pageId}-{Date.now}-{seq}` 的 Date.now 数字段破坏两处解析（TerminalPanel 贪婪正则 → visible 恒 false 黑屏；parseTerminalPageId 切分 → 幽灵页面导航空白）。**FE-27：可取消**——`waitFor` 接受 `AbortSignal`（循环前检查 `signal.aborted`，中止后停止轮询并抛错）；四步共享一个模块级 `restoreAbortRef` Controller，**新恢复发起时 abort 上一轮在途恢复**（页面切换后旧轮询不误操作）

- **profile 策略委托（MC-315）**：按 `session.cliId` 查 profile——无 history 能力（含 profile 未注册）→ 防御性失败，走统一失败 toast 路径（能力未声明 = 该域不可用）
- **防重入**：模块级 `restoring` 标记，进行中再次调用直接返回（快速双击同一行）
- **失败**：任何步骤异常 → `sendToastNotification("恢复会话失败", ...)` + console.error，不中断其他流程（场景 10）
- **前置拦截**：孤儿（`cwdExists=false`）/无 cwd 行的禁用判定由调用方（菜单/双击分派）负责；函数内 cwd 为 null 仍防御性 throw

### 操作矩阵（FE-07，规格 4.4，`historyContextMenu.ts`；重命名已移除）

`getHistoryContextMenuItems(session, opts)` 策略函数（照 `commitContextMenu.ts` 策略模式），**差异**：本策略不直接做 IPC——三项操作的 action 由调用方（`NavTree` 历史行右键菜单，原 HistorySessionList（已删））经 opts 回调注入（onCopy/onFork/onDelete），策略层只负责禁用态判定与菜单项构造：

| 操作 | 普通行 | 孤儿行 ✗ | 运行中行 | 无 cwd 行 |
|------|--------|---------|----------|----------|
| 复制恢复命令（`buildResumeCommand` 委托 `profile.history.buildResumeCommand`——无 history 能力 → 空串降级；命令形态含 cwd 单引号路径等 CLI 专属限制，由各 CLI 实现负责） | ✓ | ✓ | ✓ | ✓ |
| 分支恢复（四步 + fork 注入）——`profile.history.supportsFork` 缺省 false（能力未声明）→ **不展示菜单项**（MC-316） | ✓ | ✗ 禁用 | ✓ | ✗ 禁用 |
| 删除（`confirmDialog` 确认（UI-801/803，原 `dialog.ask` 已退役 OV-02）→ `agent_history_delete(cliId, sessionId)` → `removeLocal` 不重扫） | ✓ | ✓ | ✗ 禁用（句柄占用 + 幽灵文件） | ✓ |

### 双击分派（问题 5 修复）与动作弹窗

历史行双击三分支（`NavTree` 内实现，原 HistorySessionList（已删）语义）：普通行 → 恢复四步；孤儿/无 cwd → 无操作；**运行中（status 非 null）→ `SessionActionDialog` 弹窗**（「切换到该会话操作页面」/「取消」；**分支恢复仅保留在右键菜单**——Tauri 原生 dialog 无法三按钮，自绘模态照 InputDialog 模式，`data-e2e="agent-history-action-dialog"`）。「切换到该会话操作页面」= 反查 `TerminalRegistry.getAll()`（`findPanelForSession`：**复合键 `cliId|sessionId` 精确匹配**（MC-313），键构造两侧均经 `keyOf`（cliId 缺省回退 `CLAUDE_CLI_ID` + 转义，ZQ-1）、sessionId 精确匹配、回退 usageSourcePath basename）→ **`findPageIdForPanelId`（B14：先按 useProjects 已知页面集合前缀匹配——旧格式可靠，兜底 `parseTerminalPageId`；均未命中 toast 不导航）** → `switchToPageAndFocus(pageId, panelId)`（内部：activePageId 相同则直接聚焦）；反查不到 → `sendToastNotification` 提示（会话已结束）。

### 已知限制（MC-318，规格确认不修——决策 6）

1. **组键漂移**：原 HistorySessionList（已删）的 `expandedGroups` 白名单组键漂移问题随其删除不涉及导航树——导航树历史归组键 = 项目 projectId（稳定），无漂移；`groupByCwd` 的未知目录组键漂移仅影响无生产消费方的分组模型（当前仅测试消费）
2. **历史区相对时间无 ticker**：`formatRelativeTime` 在渲染时计算，历史区无定时重渲染机制（活跃区有 60s ticker，见 agentStatus 模块）——挂起的历史区相对时间文本不自动刷新，直至其他状态变更触发重渲染（视为可接受，不修）

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export（照 `commit/index.ts` 模式）：列表/行/弹窗/数据 hook/恢复编排/菜单策略/模型纯函数（`AgentHistorySections` 已删除，NAV-08） |
| `HistorySessionList.tsx` | 历史区列表组件（**已删除，FE-25**——原 NAV-08 后「保留但无生产消费方」）：current 平铺 / all `groupByCwd` 二级折叠（组默认收起）；双击三分派 + 右键菜单调用方（经 props 注入） |
| `HistorySessionRow.tsx` | 双行式行组件（**已删除，FE-25**——原 NAV-08 后「保留但无生产消费方」、props 签名保持兼容；导航树改用单行式 `NavHistoryRow`） |
| `SessionActionDialog.tsx` | 动作弹窗（问题 5 新建，照 InputDialog 样式）：标题 + 消息 + 竖排动作按钮 + 取消；Esc/遮罩点击取消——**生产消费方 = `NavTree`（运行中历史行双击）** |
| `historyContextMenu.ts` | 右键菜单策略：`getHistoryContextMenuItems`（禁用态矩阵，重命名项已移除）+ `buildResumeCommand`（委托 `profile.history.buildResumeCommand`）——**生产消费方 = `NavTree`** |
| `historyModel.ts` | 纯函数模型：`isCurrentProject` / `groupByCwd` / `matchesSearch` / `formatRelativeTime` / `keyOf`（复合键 `cliId\|sessionId` 构造单点——回退 + 转义）/ `deriveActiveSessionStatuses` + `UNKNOWN_CWD_KEY` |
| `useAgentHistory.ts` | 数据 hook（消费方 = `useNavTree`）：状态机 + scan(force?)（`scanAgentHistory(CLAUDE_CLI_ID, force)` 单 CLI 扫描，BE-19 后端缓存命中 / force 透传）/removeLocal + TerminalRegistry 订阅四态映射 + rootPath 推导（照 `useCommitStatus`） |
| `restoreSession.ts` | 四步恢复编排：`restoreHistorySession(session, {fork})`（profile 策略委托）+ `waitFor` 轮询（100ms×50，**FE-27：接受 `AbortSignal`**，模块级 `restoreAbortRef` 新恢复 abort 旧）+ 防重入 + 失败 toast |

## 测试模式

L2 测试位于 `src/__tests__/`，命名规则 `agent-history-*.test.ts(x)` + `ipc-agent-history-contract.test.ts`（用例数见 `.claude/test-inventory.md`；`agent-history-view.test.tsx` 已随 AgentStatusView 退役删除，NAV-08——原「AgentStatusView 集成」用例语义迁入 `nav-tree.test.tsx`）：

| 文件 | 覆盖范围 |
|------|---------|
| `agent-history-model.test.ts` | 纯函数全分支：isCurrentProject（大小写/斜杠/null/空串/前缀不匹配）、groupByCwd（排序/未知组/空数组/同目录不同 CLI 同组）、matchesSearch（大小写/空白）、formatRelativeTime（六档边界 + 跨年 + mtime=0）、deriveActiveSessionStatuses（**复合键 cliId\|sessionId**/sessionId 优先/basename 回退/双无跳过/status null 不产出/四态透传/旧数据无 cliId → CLAUDE_CLI_ID 回退/空注册表） |
| `agent-history-hook.test.tsx` | 状态机流转、scan 成功/失败、removeLocal 不触发 scan、subscribe 驱动 activeStatuses 更新、卸载清理 |
| `agent-history-restore.test.ts` | 四步编排（mock stores/projects、workspace/pageApis、ipc/pty、TerminalRegistry、ipc/notification）：已开项目跳过入列/无页建页/切页/addPanel 参数（cwd/id 格式、title = profile.tabTitle）/pty.write 内容（`profile.history.buildRestoreInput` 输出——普通/fork/`\r` 结尾）/防重入/失败 toast/无 cwd 防御性 throw/无 history 能力 profile 防御性失败 |
| `agent-history-row.test.tsx` | 双行渲染（保留组件兼容）、**四态标记**（working→绿/attention→黄/done→灰/error→红/null 无标记）、✗ 孤儿标记、**行 logo 按 session.cliId 查 profile**（注册/未注册 cliId 无 logo 不报错）、单击选中、双击/右键回调 |
| `agent-history-action-dialog.test.tsx` | SessionActionDialog：标题/消息/动作按钮渲染、action 回调、取消（按钮/Esc/遮罩）、空 actions 防御 |
| `ipc-agent-history-contract.test.ts` | 两命令 × 四维验证（命令名 / 参数结构 camelCase 含 cliId / 正常返回 / 异常传播），经共享工厂 `describeIpcContract`（`helpers/ipc-contract.ts`，IHE-06）声明式驱动——mockIPC 盲区声明见 `src/ipc/CLAUDE.md` |
| `nav-tree-history.test.tsx` | **导航树历史节点（NAV-03/10 契约）**：历史折叠节点常驻项目下/计数 pill/展开触发重扫/单行行渲染（StatusDot 恒渲染 done 灰档/logo/title tooltip）/双击恢复/右键菜单（复制/分支/删除 confirmDialog）/空态 |

> 另有 `agent-status-hook.test.ts`（agentStatus 侧）覆盖 useAgentStatus 行建模；`nav-tree.test.tsx` 覆盖活跃会话区集成；`terminal-registry.test.ts` / `use-xterm-lifecycle.test.ts` 覆盖 agentSession sessionId/cliId/status 存储与 hook 事件写入（HUK1-9）；`cli-profile-claude.test.ts` 覆盖 history 策略输出（buildResumeCommand/buildRestoreInput 与迁出源逐字一致）；`mock-cli-profile.test.tsx` 覆盖 mock CLI（cliId="mockcli"）全链路（历史区条目 + 行 logo + 恢复注入桩输出）。

### 测试模式要点

- **纯函数层零 mock**：`historyModel` 直接调用断言（`deriveActiveSessionStatuses` 除外——mock `TerminalRegistry` 供读注册表）
- **hook/编排层 mock 边界**：mock 只守 JS 侧形状，真实编排由 L4 E2E 兜底（`agent-history-restore.test.ts` mock 四个模块）
- **导航树集成**：nav-tree-history.test.tsx 经 `useNavTree` 驱动（真实 useAgentHistory + TerminalRegistry）
- **E2E 兼容断言**：导航树行 `data-e2e="nav-row-session"` / 历史节点 `data-e2e="nav-history-node"` 等契约断言

### 运行

```bash
npx vitest run agent-history nav-tree-history   # 仅历史会话模块 + 导航树历史测试
npm test                                         # L2 全量
```
