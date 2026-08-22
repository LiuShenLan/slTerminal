# L2 域B（features 域）测试 Review 报告

## 问题清单

### [高] explorer-virtualization.test.tsx 在 jsdom 中模拟视口与计数方式不可靠
- **维度**: 断言有效性
- **证据**: `explorer-virtualization.test.tsx:75-84`（`mockClientHeight` 覆盖 `HTMLElement.prototype.clientHeight`）、`98-109`（1000 节点窗口化断言）、`111-115`（clientHeight=0 全量兜底断言）、`renderedRowCount:66-69`（按 `getAllByText(/^f\d+\.ts$/)` 计数）
- **证据类型**: 静态推断
- **问题**: jsdom 无真实布局，测试通过覆盖原型属性模拟 clientHeight；`renderedRowCount` 用 `screen.getAllByText` 匹配文件名，React StrictMode 双渲染时同一行会返回两个 span，计数虚高。`1000 节点仅渲染 <100 行` 的断言可能永远满足，即使生产代码虚拟化失效。`clientHeight=0` 全量兜底断言 `toBe(120)` 同样可能因双渲染而变成 240，但当前未触发 StrictMode 时碰巧通过。
- **建议**: 给 `TreeNodeRow` 加唯一 `data-testid` 并在测试内查询容器直接子元素；显式控制 StrictMode；全量兜底改为断言可见行数等于输入节点数且每个节点有唯一 key。

### [高] sideBar / sideBarArea / activityBar 测试共享 sideViewRegistry 单例，side-effect import 可能混入真实视图
- **维度**: 隔离性
- **证据**: `sideBar.test.ts:31-32,72-73,87-89`（`_reset` 后注册 stub 视图）、`sideBarArea.test.tsx:59-65,134-146`（同前）、`activityBar.test.tsx:40-44,67-71,161-164`（同前）
- **证据类型**: 静态推断
- **问题**: `sideViewDefs.ts` 为 side-effect import，若测试文件或实现链中导入该模块，真实 `nav/explorer/commit` 视图会在 `_reset` 后重新注册。测试仅注册 stub 视图，导致 `reconcileZones` / `getAll()` 混入真实定义，持久化恢复与排序断言可能偏离预期。
- **建议**: 在测试 setup 中屏蔽 `sideViewDefs` 的 side-effect（如通过 vi.mock），或每次 `beforeEach` 在 `_reset()` 后断言 `getAll().length === 0` 再注册 stub。

### [高] explorer-sandbox-race.test.tsx 标题为“沙箱竞态”却未构造真实并发竞态
- **维度**: 覆盖率
- **证据**: `explorer-sandbox-race.test.tsx:1-6`（文件头声明验证 DBG-10 时序）、`257-290`（"先启动 setProjectRoot pending 再加载 useFileTree" 用例）、`292-315`（setProjectRoot 先于 readDir 完成用例）
- **证据类型**: 静态推断
- **问题**: DBG-10 回归故障 A 是“React effect 子先于父执行 → readDir 在 setProjectRoot 前到达后端被拒”。但测试仅验证 `useFileTree` 不等待 `setProjectRoot`，调用顺序为测试代码手动编排的顺序执行，没有真正让 `setProjectRoot` 与 `readDir` 并发交织，也未验证后端拒绝或 readDir 调用被沙箱拦截的后果。
- **建议**: 构造并发场景：在 `setProjectRoot("/proj")` pending 期间触发 `ExplorerPanel` mount 并设置 `rootPath="/proj"`，断言 `readDir` 实际调用时刻晚于 `setProjectRoot` resolve，或验证后端返回拒绝后的降级行为。

### [中] explorer-race-cleanup.test.tsx G3 用例对初始加载完成时机假设过强
- **维度**: 稳定性与确定性
- **证据**: `explorer-race-cleanup.test.tsx:132-161`（G3 fs-event 去抖清理）
- **证据类型**: 静态推断
- **问题**: 用例在触发 fs-event 前使用 `await vi.advanceTimersByTimeAsync(0)` 并立即断言 `rootNodes.length === 1`，但初始 `readDir` + `gitStatus` 的微任务链未必在 0ms 内完成；若未完成，后续 `triggerFsEvent()` 的定时器清理断言失去意义。
- **建议**: 改用 `waitFor(() => expect(result.current.rootNodes.length).toBe(1))` 等待初始加载完成，再触发 fs-event，并断言去抖前 `readDir` 未被调用。

### [中] explorer-crud-success.test.tsx 的 `rowBackground` 在虚拟化 DOM 中可能取错行元素
- **维度**: 断言有效性
- **证据**: `explorer-crud-success.test.tsx:103-111`（`rowBackground` 实现）、`146-162`（删除后选中态清空断言）、`173-212`（重命名输入框断言）
- **证据类型**: 静态推断
- **问题**: `rowBackground` 找到文本为文件名的 span 后取 `closest("div")`。在 `FileTree` 虚拟化结构中，内容包裹在带 `paddingTop/paddingBottom` 的 div 内，span 的最近 div 可能不是 `TreeNodeRow`，导致背景色/选中态断言失真。
- **建议**: 给 `TreeNodeRow` 增加 `data-testid` 并据此限定行容器；或直接断言 `selectedPath` 状态变化。

### [中] commit-view-status.test.ts 旧请求丢弃测试未等待状态稳定
- **维度**: 稳定性与确定性
- **证据**: `commit-view-status.test.ts:233-295`（"rootPath 变化后旧请求结果不覆盖新状态"）
- **证据类型**: 静态推断
- **问题**: `resolveOld([...])` 后仅 `await vi.advanceTimersByTimeAsync(10)` 就断言 `commitView?.textContent` 不含 `old.ts`。React 重渲染与状态更新可能尚未完成，断言可能偶然通过但无法捕获旧数据闪屏。
- **建议**: `resolveOld` 后用 `waitFor` 多次轮询检查 `textContent`，确保在合理超时内持续不含 `old.ts`。

### [中] activityBar.test.tsx 全局 mock `HTMLElement.prototype.getBoundingClientRect` 过于宽泛
- **维度**: mock 合理性
- **证据**: `activityBar.test.tsx:137-157`（`installRectSpy` 全局覆盖原型）、`269-296,300-314,377-429` 等拖拽用例
- **证据类型**: 静态推断
- **问题**: 测试期间所有 HTMLElement 的尺寸测量都被 mock，React Testing Library 与组件内部任何依赖真实几何的计算都走同一返回值；ActivityBar 真实布局在 jsdom 下本就无法验证，测试退化为对 `computeDropTarget` 纯函数的间接调用验证。
- **建议**: 组件级测试只验证事件委托、dataTransfer 设置、`computeDropTarget` 被调用参数；将落点算法留给 `dropTarget.ts` 的纯函数测试。

### [中] explorer-delete.test.tsx E6 键盘 Del 测试未覆盖真实焦点链路
- **维度**: 覆盖率
- **证据**: `explorer-delete.test.tsx:682-779`（E6 组键盘 Del 删除）
- **证据类型**: 静态推断
- **问题**: 测试直接 `window.dispatchEvent(new KeyboardEvent("keydown"))` 并手动 `pushContext("explorer")`，未验证 ExplorerPanel 容器中 `focusin → pushContext("explorer") + setActiveExplorer` 的真实链路。若 `usePanelFocus` 或容器 `tabIndex` 配置错误，Delete 键在生产中不会触发删除，但本测试仍通过。
- **建议**: 增加集成用例：渲染 `ExplorerPanel`、点击文件行、再按 Delete，断言 `deleteSelected` 被调用。

### [中] nav-tree / nav-tree-history 大量依赖样式与 data-e2e 的同步断言存在 flaky 风险
- **维度**: 稳定性与确定性
- **证据**: `nav-tree.test.tsx:422-453`（hover/选中态颜色断言）、`668-715`（pill 色值断言）、`876-933`（历史节点展开与位置断言）；`nav-tree-history.test.tsx:195-250`（历史计数 pill 断言）
- **证据类型**: 静态推断
- **问题**: 多次在 `waitFor` 后紧跟同步的 `style.color` / `style.backgroundColor` 断言，React 状态更新后的重渲染可能尚未稳定；历史节点展开后文本断言也可能因异步 scan 结果而抖动。
- **建议**: 将样式与文本断言也包入 `waitFor`；对历史节点使用稳定的 `data-testid` 而非深层 DOM 遍历。

### [中] explorer/commit/nav-tree 测试未统一重置所有 Zustand stores
- **维度**: 隔离性
- **证据**: `commit-view.test.tsx:68-104`（仅重置 projects/layout/titleManager）、`nav-tree.test.tsx:228-262`（同前）、`explorer-crud-success.test.tsx:113-125`（同前）
- **证据类型**: 静态推断
- **问题**: 测试只重置自己使用的 store，但 `useSideBar`、`useKeybindings`、`useProjects` 等共享单例。某个测试修改的 store 状态可能在下一个测试的 `beforeEach` 之前影响其初始渲染。
- **建议**: 统一调用 `helpers/workspace-setup.ts` 中的 `resetStore()` 或等效函数，重置所有相关 Zustand stores。

### [中] file-viewer-registry.test.ts 单例默认行为由测试私有函数恢复，与生产初始化解耦
- **维度**: 断言有效性
- **证据**: `file-viewer-registry.test.ts:204-219`（`restoreDefaultRegistry` 测试私有函数）、`227-262`（默认 html/htm 断言）
- **证据类型**: 静态推断
- **问题**: 测试在 `_reset()` 后用自己的 `restoreDefaultRegistry` 重新注册 html/htm，再断言这些扩展名命中。若生产代码 `FileViewerRegistry.ts` 的预注册逻辑被删除或变更，测试不会发现。
- **建议**: 直接导入生产初始化文件并断言其注册结果；或在测试外通过静态检查确保生产初始化包含 html/htm。

### [低] global-commands.test.ts 与 shortcuts.test.ts 键盘事件构造重复且未统一
- **维度**: 覆盖率
- **证据**: `global-commands.test.ts:80-85,117-127`（各自 `new KeyboardEvent`）、`shortcuts.test.ts:30-41`（`dispatchKeydown` 辅助）
- **证据类型**: 静态推断
- **问题**: 两份文件都构造 `KeyboardEvent`，但默认字段（ctrlKey/shiftKey/code）处理不一致；未来修改键位测试可能漏改一处。
- **建议**: 抽取共享 `dispatchKeydown` helper 到 `helpers/keyboard.ts`，统一默认值与返回值。

### [低] commit-context-menu-ui.test.tsx 右键菜单定位依赖 `position: fixed` 全局选择器
- **维度**: 断言有效性
- **证据**: `commit-context-menu-ui.test.tsx:131-135`（`getMenuEl` 实现）、`167-169`（无菜单项断言）
- **证据类型**: 静态推断
- **问题**: 通过 `'div[style*="position: fixed"]'` 取最后一个元素作为菜单；若页面上同时存在 tooltip、dialog 等固定定位浮层，会误匹配。
- **建议**: 给 `ContextMenu` 组件添加 `data-testid` 并限定查询。

### [低] nav-history-row.test.tsx 导入 cliProfiles/profiles 触发 side-effect 注册且未清理
- **维度**: 隔离性
- **证据**: `nav-history-row.test.tsx:28`（`import "../features/cliProfiles/profiles"`）
- **证据类型**: 静态推断
- **问题**: 该 import 会触发 CliProfileRegistry 注册 claude profile；测试后未 `_reset`，后续测试查询 profile 时可能读到本测试残留状态。
- **建议**: `afterEach` 中调用 `CliProfileRegistry._reset()`，或避免在组件测试中直接导入 profiles。

### [低] commit-open-file.test.ts 设置 `window.__dockviewApi` 未在 afterEach 清理
- **维度**: 隔离性
- **证据**: `commit-open-file.test.ts:55-69`（`mockDockApi` 设置全局属性）
- **证据类型**: 静态推断
- **问题**: 测试在 `beforeEach` 中设置 `window.__dockviewApi`，但未在 `afterEach` 删除。后续测试若依赖该属性可能得到过期 mock。
- **建议**: `afterEach` 中执行 `delete window.__dockviewApi`。

### [低] wire-keybindings.test.ts 仅验证 fake store，未与真实 useKeybindings 集成
- **维度**: 覆盖率
- **证据**: `wire-keybindings.test.ts:11-27`（fake store 工厂）、`29-65`（三用例均使用 fake store）
- **证据类型**: 静态推断
- **问题**: 测试只覆盖纯接线函数，未验证真实 `useKeybindings` 的 `subscribe/getState` 签名与 `wireKeybindings` 实际集成后是否一致。
- **建议**: 增加一个与真实 `useKeybindings` 的轻量集成用例，确保 store 接口漂移时测试变红。

### [低] explorer-input-boundary / explorer-crud-success 用 `document.querySelectorAll('input')` 取最后一个输入框
- **维度**: 稳定性与确定性
- **证据**: `explorer-delete.test.tsx:487-488`（重命名输入框）、`explorer-crud-success.test.tsx:190-191,240-241,284-285`（新建/重命名输入框）
- **证据类型**: 静态推断
- **问题**: StrictMode 双渲染或页面上存在多个 input 时，取最后一个 input 可能拿到错误元素。
- **建议**: 给内联输入框加 `data-testid` 或按父路径限定查询。

### [低] explorer-refresh-preserve.test.tsx R17 未控制 gitStatus 时序
- **维度**: 稳定性与确定性
- **证据**: `explorer-refresh-preserve.test.tsx:455-478`（连续两次 refresh）
- **证据类型**: 静态推断
- **问题**: 连续两次 `refresh()` 会并发触发 `gitStatus`；测试只断言 `rootNodes` 最终状态，未控制 `gitStatus` 返回顺序，可能因异步抖动而 flaky。
- **建议**: 使用 fake timers 或 mock `gitStatus` 为同步 resolved，显式控制两次 refresh 的完成顺序。

### [低] keyboard.test.ts paste 异步测试未显式 reset readText mock
- **维度**: 隔离性
- **证据**: `keyboard.test.ts:124-147`（paste 成功与失败路径）
- **证据类型**: 静态推断
- **问题**: `readTextMock` 在 `beforeEach` 中 `mockClear()` 但未 `mockReset()`，若之前测试用 `mockResolvedValueOnce` 留下队列，会影响当前用例。
- **建议**: 使用 `mockReset()` 或显式在每个用例设置 `mockResolvedValue`/`mockRejectedValue`。

## 审查覆盖声明

- 审阅文件: 51/51（全部按列表逐个阅读；其中 20+ 个关键文件精读并对照实现，其余泛读抓结构与断言模式）
- 执行命令与结果:
  - `npx vitest run src/__tests__/activeExplorer.test.ts src/__tests__/activityBar.test.tsx src/__tests__/command-catalog.test.ts src/__tests__/commit-context-menu-ui.test.tsx src/__tests__/commit-context-menu.test.ts src/__tests__/commit-open-file.test.ts src/__tests__/commit-view-list.test.tsx src/__tests__/commit-view-status.test.ts src/__tests__/commit-view.test.tsx src/__tests__/confirm-dialog.test.tsx` → 10 文件 142 用例全通过
  - `npx vitest run src/__tests__/dir-entry-null.test.tsx src/__tests__/error-boundary.test.tsx src/__tests__/explorer-crud-success.test.tsx src/__tests__/explorer-delete.test.tsx src/__tests__/explorer-error-placeholder.test.tsx src/__tests__/explorer-file-viewer.test.tsx src/__tests__/explorer-focus.test.tsx src/__tests__/explorer-git-status.test.tsx src/__tests__/explorer-input-boundary.test.tsx src/__tests__/explorer-keyboard.test.ts` → 10 文件 127 用例全通过
  - `npx vitest run src/__tests__/explorer-notify.test.tsx src/__tests__/explorer-open-in-terminal.test.tsx src/__tests__/explorer-race-cleanup.test.tsx src/__tests__/explorer-refresh-preserve.test.tsx src/__tests__/explorer-rename-keyboard.test.tsx src/__tests__/explorer-rename-state.test.tsx src/__tests__/explorer-root-contextmenu.test.tsx src/__tests__/explorer-rootpath-clear.test.tsx src/__tests__/explorer-sandbox-race.test.tsx src/__tests__/explorer-selection.test.tsx` → 10 文件 106 用例全通过
  - `npx vitest run src/__tests__/explorer-virtualization.test.tsx src/__tests__/file-icon.test.tsx src/__tests__/file-viewer-registry.test.ts src/__tests__/global-commands.test.ts src/__tests__/keyboard.test.ts src/__tests__/keybindings.test.ts src/__tests__/keystroke.test.ts src/__tests__/language-mapping.test.ts src/__tests__/nav-history-row.test.tsx src/__tests__/nav-tree-history.test.tsx` → 10 文件 201 用例全通过
  - `npx vitest run src/__tests__/nav-tree.test.tsx src/__tests__/notification.test.ts src/__tests__/notifications.test.ts src/__tests__/shortcuts.test.ts src/__tests__/sideBar.test.ts src/__tests__/sideBarArea.test.tsx src/__tests__/sideViewRegistry.test.ts src/__tests__/toast.test.tsx src/__tests__/use-file-tree.test.ts src/__tests__/use-panel-focus.test.ts src/__tests__/wire-keybindings.test.ts` → 11 文件 223 用例全通过
- 重跑验证记录: 基线一次性全绿，未对单个用例做 ≥3 次定向重跑（本次为静态审查为主）。建议对标记“稳定性与确定性”的问题补做定向重跑。
