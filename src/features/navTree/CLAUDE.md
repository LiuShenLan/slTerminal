# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

统一导航树（NAV-01/02/03/04/09，UI 重设计 Stage 06）——侧栏 `nav` 视图组件（sideViews 三槽之一，NAV-05 注册）。四级层级：项目 → 页面 → 活跃会话；历史会话折叠节点挂项目下。兼并原 SidebarTree（项目/页面 CRUD）与 agent-status 视图（活跃会话）双重职责（NAV-06/08 承接约定）。

## 关键约束与决策

### 层级与数据源

- **层级**：项目 → 页面 → 活跃会话（`NavSessionRow`）；历史折叠节点挂项目下（`NavHistoryNode` → `NavHistoryRow`）。
- **数据源全部只读引用既有数据层，零新增订阅**（`useNavTree`）：
  - 项目/页面树：`useProjects`
  - 活跃会话：`useAgentStatus`
  - 历史会话：`useAgentHistory`
- **归属规则**：
  - 活跃会话挂页面下（`row.pageId`——`useAgentStatus` 内部已按 `parseTerminalPageId` 解析）。
  - 历史会话挂项目下：`session.cwd` 前缀匹配项目 `rootPath`（规范化 + 忽略大小写 + 段边界守卫），最深前缀命中；无归属项目不展示。

### 展开/折叠与搜索（NAV-01/04/10）

- 展开状态组件内维护（`expanded` / `expandedHist` 两个 Set），**默认空 = 全部收起**。
- **NAV-10 契约**：历史节点常驻项目下（不随项目展开态隐藏），计数 pill 与历史行入口恒可见；外包 `childrenStyle` 容器与操作页面同级缩进，位于页面容器之后恒置最下方。
- 搜索：query 子串不区分大小写过滤项目/页面/会话名；父节点因子命中而显示；查询非空时命中链自动展开。

### FE-16 历史归属索引

原 `isCwdUnderProject` 纯函数 O(N×M) 逐会话前缀匹配已删除。现 `rootPathIndex` Map 规范化 rootPath → projectId 一次建表，`projectIdForCwd` 沿 cwd 逐级上溯查表，最深前缀命中（嵌套项目归子项）。`useMemo` 依赖精确化——只依赖 sessions + 索引，项目页增删不重算归组。

### FE-19 历史扫描时机

- 挂载即扫描历史一次（计数 pill 首屏可见）。
- 展开历史节点不重复 scan——BE-19 后端 `(目录 mtime, 文件数)` 缓存命中复用不重复读盘。
- 刷新钮 = `history.scan(true)`（force=true 绕过后端缓存强制重扫，空结果永久命中场景必须 bypass）。

### CRUD 迁移承接（NAV-06）与入口唯一化

- 添加项目：`dialog.open` 选文件夹 → `addProject`（建项目 + 默认空布局页面）。
- 新建页面：`handleNewPage`（默认名 + `makeEmptyLayout()` 空布局，不自动切换）。
- 删除项目/页面/历史会话统一走 `confirmDialog`（OV-02，替换 `window.confirm`）。
- `makeEmptyLayout()` 迁自 SidebarTree（NAV-06 承接约定），`restoreSession` 等消费点改引用本导出。
- 右键菜单删除「打开 Hooks 配置」项（决策 4 入口唯一化——配置钮移至活动栏底部）。

### 行结构契约

- 行高 28（会话行 30）、圆角 5、每级左缩 15px + 发丝引导线。
- fg 层级映射：fg-1 = `SIDEBAR_FG` / fg-2 = `SIDEBAR_COLORS.fg` / fg-3 = `DIM_FG` / fg-4 = `PLACEHOLDER_FG`。
- 项目行：500 字重 + 彩色文件夹图标（六色盘蓝，**硬编码例外**，IC-04/ NAV-09）+「当前」pill + 页面计数 pill。
- 页面行：`IconPage` 14px fg-3 图标；chevron 点击仅切换会话展开；行点击 = 切换页面 + 切换展开；选中 = 活跃页面。
- 活跃会话行：`StatusDot`（F3 四态）+ CLI logo 14px（按 `row.cliId` 查 `profile.iconSrc`）+ 标题 + 迷你用量条 pill + 百分比；点击行聚焦对应终端页签。
- 历史行：`StatusDot` 恒渲染（运行中按实际态，无运行状态 → done 灰档）+ logo + 标题 + 相对时间 + `title` tooltip；双击恢复三分支（运行中 → SessionActionDialog / 孤儿无 cwd → 无操作 / 普通 → `restoreHistorySession(..., { fork: false })`）。

## 硬约束

- **#6 配色单点**：全部颜色引用 `theme/colors.ts` token；唯一例外 = 项目行彩色文件夹图标六色盘蓝（NavProjectRow 内登记，IC-04）。
- **#1 前端不碰 OS**：IPC（dialog.open / deleteHistorySession / 写剪贴板）全部经 `src/ipc/` 调用。
- **数据 hook 不自建订阅**：活跃会话/历史数据一律经 `useAgentStatus` / `useAgentHistory` 获取；运行中会话反查经 `workspace/pageApis` 调用，本组件不再直接引用 `TerminalRegistry`。

## 测试模式

- L2 测试：`nav-tree.test.tsx` + `nav-tree-history.test.tsx`。
- **NAV-10 契约辅助**：测试按点击展开驱动（`expandTo`），断言基于最终渲染而非内部状态。
- **数据属性契约（写死）**：容器 `data-e2e="nav-tree"`；行 `data-e2e="nav-row-project"` / `"nav-row-page"` / `"nav-row-session"`；历史节点 `data-e2e="nav-history-node"`。

## 运行

```bash
npx vitest run nav-tree
npm test
```
