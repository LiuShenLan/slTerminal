# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

统一导航树（NAV-01/02/03/04/09，UI 重设计 Stage 06）——侧栏 `nav` 视图组件（sideViews 三槽之一，NAV-05 注册）。**四级层级**（决策 5，UI-303）：项目 → 页面 → 活跃会话；历史会话折叠节点挂项目下。**兼并原 SidebarTree（项目/页面 CRUD）与 agent-status 视图（活跃会话）双重职责**（NAV-06/08 承接约定）；项目/页面 CRUD、页面切换导航迁自 SidebarTree 且行为不变。

## 架构决策

### 层级与数据源（NAV-01/02/03/04，决策 5）

- **层级**：项目 → 页面 → 活跃会话（`AgentSessionRow`）；历史折叠节点挂项目下（`NavHistoryNode` → `NavHistoryRow` 单行）
- **数据源全部只读引用既有数据层，零新增订阅**（`useNavTree`）：
  - 项目/页面树：`useProjects`（照原 SidebarTree 订阅形态）
  - 活跃会话：`useAgentStatus`（rows——panelId/pageId/projectId/cliId/title/status/usage）
  - 历史会话：`useAgentHistory`（sessions + activeStatuses + scan/removeLocal）
- **归属规则**：活跃会话挂页面下（`row.pageId`——useAgentStatus 内部已按 `parseTerminalPageId` 解析）；历史会话挂项目下（`isCwdUnderProject`：规范化 + 忽略大小写 + 去尾部斜杠后 cwd === rootPath 或为其子路径（前缀 + `/` 防同前缀目录误归属）；无归属项目（孤儿目录）→ 导航树不展示）

### 展开/折叠与搜索（NAV-01/04/10）

- 展开状态**组件内维护**（不进 store）：`expanded`（项目/页面）与 `expandedHist`（历史节点）两个 Set，**默认空 = 全部收起**
- **NAV-10 契约**：历史节点**常驻项目下**（不随项目展开态隐藏）——计数 pill 与历史行入口恒可见；测试辅助 `expandTo` 按点击展开驱动
- **搜索**：query 子串不区分大小写过滤项目/页面/会话名；父节点因子命中而显示（match 链）；**查询非空时命中链自动展开**（searching 覆盖手动展开态）
- **挂载即扫描历史**（NAV-10：历史计数 pill 首屏可见）+ 历史节点**展开时重扫**（照原 agentHistory 历史区展开刷新语义；`useAgentHistory` generation 防竞兜底）

### 行结构契约（UI-501/502/503，navStyles.ts）

- 行结构：chevron 12px fg-3 + 图标 + 名称 + 右侧 11px fg-4 元数据；**行高 28（会话行 30）**、圆角 5、hover #222227（`SIDEBAR_COLORS.hover`）；选中行 accent-dim 底（`ACTIVE_SELECTION_BG`，hover → `SELECTION_HOVER_BG`）+ fg-1
- 层级缩进：每级左缩 15px + 1px 发丝引导线（`SIDEBAR_COLORS.treeGuide`）
- **fg 层级映射**（final-mockup 契约）：fg-1 = `SIDEBAR_FG` / fg-2 = `SIDEBAR_COLORS.fg` / fg-3 = `DIM_FG` / fg-4 = `PLACEHOLDER_FG`
- **项目行**（NAV-09/UI-505）：500 字重 fg-1 + 彩色文件夹图标（六色盘蓝 `#7fa8e8`——**硬编码例外，NAV-09 写死**，与 FileIcon.tsx 六色盘同源规格，IC-04 契约）+「当前」pill（active 项目，accent-dim 底 + ACCENT_FG 字 10px）+ 页面计数 pill（SIDEBAR_BG 底 + fg-4）
- **页面行**（UI-501）：chevron 点击仅切换会话展开（stopPropagation），**行点击 = 切换页面 + 切换展开**（照原 SidebarTree 语义）；选中 = 活跃页面；收起且含活跃会话时右侧 meta = 最近会话标题；内联重命名（右键菜单入口，Enter 确认/Esc/blur 取消/空白或同名取消）
- **活跃会话行**（NAV-02/UI-504，决策 6 单行化）：StatusDot（F3 四态，null 不渲染）+ CLI logo 14px（按 `row.cliId` 查 `profile.iconSrc`，未注册无 logo 不报错）+ 标题 fg-1 + 右侧**迷你用量条 32×3 pill 档** + 百分比 11px fg-4（`computeUsagePercent` 经 `profile.hooks` 委托，四档分级 ≥90/≥70/≥50 照 AgentStatusRow 口径；无数据 → `--`）；页面级最近事件行 `active` = accent-dim 选中底（设计 6.3）；**点击行 → 聚焦对应终端页签**（B14：先按已知页面集合前缀匹配 panelId 属主，兜底 `parseTerminalPageId`）
- **历史行**（NAV-03，决策 6 单行化）：StatusDot（运行中四态；无运行状态 → **恒渲染 done 灰档**，mockup `.dot.idle` 契约）+ logo 14px + 标题 fg-1 + 右侧相对时间 11px fg-4（`formatRelativeTime`）；**prompt 预览 → 原生 `title` tooltip**（双行式行2 退役）；双击恢复三分支（运行中 → SessionActionDialog / 孤儿无 cwd → 无操作 / 普通 → `restoreHistorySession(session, { fork: false })`）+ 右键菜单沿用 `historyContextMenu` 策略（复制/分支恢复/删除——删除经 `confirmDialog` 确认 → `agent_history_delete` → `removeLocal` 不重扫）

### CRUD 迁移承接（NAV-06）与入口唯一化（决策 4）

- **添加项目**：`open({ directory: true })` 选文件夹 → `addProject`（建项目 + 默认空布局页面，行为照原 SidebarTree.handleAddProject 不变）
- **新建页面**：`handleNewPage`（`页面-{Date.now()%10000}` + `makeEmptyLayout()` 空布局，不自动切换）
- **删除项目**：`window.confirm` 确认 → `removeProject`；**删除页面**：委托 props `onDeletePage`（Workspace 编排）或兜底 `removePage`
- **`makeEmptyLayout()` 迁自 SidebarTree**（NAV-06 承接约定——`restoreSession` 等消费点改引用本导出）
- **右键菜单删除「打开 Hooks 配置」项**（决策 4 入口唯一化——配置钮移至活动栏底部，`openHooksConfigFromActivityBar`）；删除确认弹窗统一 `confirmDialog`（OV-02）

### 空态（UI-806/GL-05）

空项目 → IconEmptyBox 15px fg-4 + 「暂无项目，点击下方「添加项目」开始」；搜索无结果 → IconSearch + 「没有找到匹配的项目 / 页面 / 会话」；历史空 → IconHistory + 「暂无历史会话」。空态规范 = 15px 线性图标 fg-4 + 说明 fg-3 居中。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export：NavTree + makeEmptyLayout + useNavTree + 行组件 + NavContextMenu + 类型 |
| `NavTree.tsx` | 主组件（`nav` 视图，NAV-05）：顶部分组标题「导航」（11px 全大写 0.08em fg-3）+ 刷新钮（重扫历史）+ 搜索框（INPUT_BG 底圆角 5、focus 描边 FOCUS_BORDER）+ 树区 + 底部「添加项目」钮；右键菜单（项目/页面/历史行共用 NavContextMenu）+ 运行中历史会话动作弹窗（SessionActionDialog）；**`makeEmptyLayout()` 导出（迁自 SidebarTree）**；props 可选（switchToPage/onDeletePage 缺省回退 store 级操作，NAV-10 契约：独立渲染 `<NavTree />` 无 props） |
| `useNavTree.ts` | 数据 hook：tree 模型派生（搜索过滤 + 归属归组 + 页面级 active 会话标记）+ expanded/expandedHist 展开集合 + `isCwdUnderProject` 纯函数 |
| `NavProjectRow.tsx` | 项目行（NAV-09/UI-505）：500 字重 + 彩色文件夹图标（六色盘蓝硬编码例外）+「当前」pill + 页面计数 pill |
| `NavPageRow.tsx` | 操作页面行（UI-501）：chevron/名称/meta + 内联重命名（迁自 SidebarTree PageRow） |
| `NavSessionRow.tsx` | 活跃会话行（NAV-02/UI-504）：StatusDot + logo + 迷你用量条 + 百分比，点击聚焦终端 |
| `NavHistoryNode.tsx` | 历史折叠节点（NAV-03/UI-303）：时钟图标 + 「历史」+ 计数 pill；外层容器 data-e2e、子级点击 stopPropagation |
| `NavHistoryRow.tsx` | 历史会话行（NAV-03 单行化）：StatusDot（恒渲染）+ logo + 标题 + 相对时间 + title tooltip；双击/右键回调委托 |
| `NavContextMenu.tsx` | 右键菜单（UI-802）：项 28px 圆角 5/hover SECONDARY_BG/危险项 ERROR_FG/容器 SIDEBAR_BG + 0.09 描边 + contextMenuShadow；点击菜单外关闭 |
| `navStyles.ts` | 共享样式 + fg 层级映射（fg-1~fg-4）+ `rowBaseStyle`/`chevronStyle`/`childrenStyle`/`nameStyle`/`metaStyle`/`countPillStyle`/`currentPillStyle` + `ROW_HEIGHT`(28)/`SESSION_ROW_HEIGHT`(30) |

## 硬约束

- **#6 配色单点**：全部颜色引用 `theme/colors.ts` token（经 navStyles/NavTree 统一）；**唯一例外** = 项目行彩色文件夹图标六色盘蓝（NavProjectRow 内登记，照 FileIcon 硬编码例外规格，IC-04）
- **#1 前端不碰 OS**：纯前端 UI 层；IPC（dialog.open 选目录/deleteHistorySession/写剪贴板）全部经 `src/ipc/` 调用
- **数据 hook 不自建订阅**：活跃会话/历史数据一律经 useAgentStatus/useAgentHistory 获取（NAV-01 数据接入契约），禁止直接订阅 TerminalRegistry/onAgentEvent

## 测试模式

L2 测试位于 `src/__tests__/`：`nav-tree.test.tsx`（27 用例）+ `nav-tree-history.test.tsx`（8 用例）（用例数见 `.claude/test-inventory.md`；原 `sidebar-actions.test.ts` 语义于 NAV-08 迁入）。

- **NAV-10 契约辅助**：测试按点击展开驱动（`expandTo`），断言基于最终渲染而非内部状态
- **数据属性契约（写死）**：容器 `data-e2e="nav-tree"`；行 `data-e2e="nav-row-project"` / `nav-row-page"` / `nav-row-session"`；历史节点 `data-e2e="nav-history-node"`（历史行须嵌套于其内——测试经 `node.querySelectorAll` 定位行内容）
- `nav-tree.test.tsx`（27）：树渲染（项目/页面/会话/历史节点层级）、展开折叠（默认收起/搜索自动展开）、搜索过滤（子串/父节点因子命中显示/无结果空态）、页面切换与选中态、会话行（StatusDot/迷你用量条百分比/logo）、CRUD（添加项目/新建页面/删除项目 confirm/删除页面/内联重命名）、右键菜单（项结构/危险项/「打开 Hooks 配置」项不存在）、空态
- `nav-tree-history.test.tsx`（8）：历史节点常驻/计数 pill/展开重扫/单行行渲染（StatusDot 恒渲染 done 灰档/logo/title tooltip）/双击恢复/右键菜单（复制/分支/删除 confirmDialog）/历史空态

### 运行

```bash
npx vitest run nav-tree        # 仅导航树测试
npm test                       # L2 全量
```
