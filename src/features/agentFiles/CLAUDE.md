# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

「Agent 全局文件」侧栏视图（F1）：agent 节点（注册序，仅声明 `globalFiles` 能力的 profile）→ 展开为该 agent 全局配置目录的文件浏览器。树交互复用 `features/explorer/FileTreeExplorer` 共享组件（功能与文件浏览器完全相同）；展示内容经 settings `agentGlobalFiles` 段配置（全部/自定义模式 + 「显示运行时文件」开关）。

## 关键约束与决策

### 分层

- `filtering.ts`：过滤/名单校验/净化纯函数（硬约束 #12——store 不存业务逻辑；参数注入，零 store/React/IPC 依赖）。匹配语义统一：大小写不敏感（NTFS 语义一致）、仅根层完全同名（名称非路径）。
- `AgentFilesPanel.tsx`：视图组件（agent 节点 + 展开 FileTreeExplorer + pinned watch 生命周期 + 打开守卫）。`AgentSection` 子组件按 agent 承载 watch 与过滤派生（hooks 规则禁在 map 回调内用 hook）。
- 状态在 `src/stores/agentGlobalFiles.ts`（纯透传段，ADR-0014 先例——后端不设 DTO，settings 白名单段直存）。

### pinned watcher（ADR-0024）

展开且目录存在 → `startWatch(dir, { pinned: true })`；折叠/卸载 → `stopWatch(dir)`。pinned 条目被 `pause_all_except` 跳过（与项目 watcher 互不暂停）、`evict_lru` 避让。目录不存在（`exists=false`）→ 展开显示「目录不存在」占位，不启动监听。

### 过滤链路

- `rootFilter`（根层三点：loadRoot 首帧+续页 / loadDirectory 根 / refreshSubtreeAt 根）← `shouldShowAtRoot(name, config, runtimePaths)`。
- `eventPathFilter`（fs-event 二次过滤，根前缀过滤之后）← `isRootEventRelevant(absPath, rootPath, config, runtimePaths)`——取相对根首段按同语义判定；根外/不可算 → false（防御）。
- store 配置变更 → `config` 对象引用变化 → `rootFilter` 换引用 → useFileTree「过滤器变化即 refreshExpanded」effect 即时生效。**红线：store 缺配置回退缺省必须 useMemo 稳定引用**（`defaultConfig()` 每次新对象，直用会令过滤器每渲染换引用触发刷新死循环）。

### 打开文件守卫

双击文件：无活跃页面/无 dockview 宿主 → `toast.show("warning", "请先创建项目")`；否则 `openFileInPage({ activePageId, dockApi, rootPath: null, projectRootPath: null }, path)`——agent 文件在项目外，标题 = basename、跳过相对路径重算（既有根外语义）。

### 跨挂载状态（CP-016 槽位契约）

viewState = `{ expandedAgents: string[], trees: Record<cliId, FileTreeViewState> }`——agent 展开集 + 每棵树展开态均经注册表状态槽存活（FE-21 槽位切换卸载不丢）。挂载消费一次快照（ref），toggle/子树提交合并上呼。

### testId 前缀

FileTreeExplorer `testIdPrefix="agent-files-<cliId>"`——同页多实例（explorer + agent 视图同开）防选择器撞名；面板自身 data-e2e：`agent-files-panel` / `agent-files-node-<cliId>` / `agent-files-missing-<cliId>`。

## 测试模式

- `filtering.ts` 纯函数全分支直测（profiles 数组参数注入，无需注册表）。
- 面板：mock `../ipc/agentDirs` + `../ipc/fs` + `../ipc/git` + `../ipc/notify`（断言 `startWatch(path, {pinned:true})`）+ `../workspace/openFile`（spy 断言项目外打开形态）+ `../lib`（toast）；注册 fake profile（globalFiles 能力可选）；store `setState` 种子。
- **无 globals 配置，RTL 自动 cleanup 不生效**：afterEach 手动 `cleanup()`；data-e2e 查询用 `container.querySelector`（document 全局查会命中前例残留 DOM）。
