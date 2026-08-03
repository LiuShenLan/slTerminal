# L2 前端测试审查报告：资源管理器 + 侧栏

## 审查范围

- **测试文件**：`src/__tests__/` 下资源管理器 16 文件（213 用例）+ 侧栏 1 文件（38 用例），共 17 文件 / 251 用例。
- **被测源码**：`src/features/explorer/*`（ExplorerPanel / FileTree / useFileTree / FileIcon / activeExplorer / keyboard）+ `src/features/sidebar/SidebarTree.tsx`。
- **方法**：纯静态审查，未运行测试；结合 `docs/test-review-problem/coverage/frontend/coverage-final.json` 提取未覆盖语句/分支/函数，交叉核对源码与测试断言。

## 总体结论

- 资源管理器 + 侧栏领域测试密度较高，核心数据流（rootPath 切换清空、reloadPreservingExpanded 保留展开、FileViewerRegistry 分派、选中模型、键盘命令派发）均有回归用例守卫。
- 但存在若干**用户高频操作路径覆盖缺失**或**测试断言与源码语义不一致**的问题，主要集中在：右键菜单「在终端中打开」、CRUD 成功后文件树刷新、fullRefresh 路径、焦点/悬停交互、图标扩展名分支、错误降级分支。
- 共识别 **14 项问题**，其中高优先级 2 项、中优先级 8 项、低优先级 4 项。

---

## 高优先级问题

### H1. ExplorerPanel「在终端中打开」功能完全无测试覆盖

- **源码位置**：`src/features/explorer/ExplorerPanel.tsx:251-262`
- **覆盖数据**：语句/分支/函数均为 0 命中；函数 `(anonymous_26)@249` 未覆盖。
- **问题描述**：`handleOpenInTerminal` 是文件/文件夹右键菜单的重要入口，负责在 Dockview 中创建终端面板并传入 `cwd`。该函数从未被任何测试调用。若未来出现以下回归，L2 无法发现：
  - `cwd` 计算错误（如使用 `path.dirname` 或反斜杠未规范化）；
  - panelId 格式不符合 `terminal-{pageId}-{seq}` 约定；
  - 未设置 `renderer: "always"` 导致页签切换后终端白屏；
  - 传入的 `params` 字段缺失或类型错误。
- **建议**：在 `explorer-file-viewer.test.tsx` 或新增 `explorer-open-in-terminal.test.tsx` 中补充：
  - 文件/文件夹右键菜单含「在终端中打开」项；
  - 点击后 `__dockviewApi.addPanel` 被调用，参数 `component="terminal"`、`params.cwd` 正确、`renderer` 为 `"always"`；
  - panelId 前缀为 `terminal-` 且含 pageId。

### H2. CRUD 成功路径未验证文件树刷新与状态重置

- **源码位置**：`src/features/explorer/ExplorerPanel.tsx:321,335,347,349-350`
- **覆盖数据**：上述语句行未命中。
- **问题描述**：当前 `explorer-delete.test.tsx` / `explorer-root-contextmenu.test.tsx` 等文件只测试了**失败**场景（错误横幅显示）。成功分支中 `refresh()`、`setRenamingPath(null)`、`setNewFileName(null)` 等关键副作用未被断言。典型风险：
  - 某次重构将 `await deleteSelected()` 后的 `refresh()` 误删除，文件树在删除后仍显示旧文件；
  - 重命名成功后未清空 `renamingPath`，输入框不消失；
  - 新建文件成功后未清空 `newFileName`，再次右键新建会残留旧值。
- **建议**：为每个 CRUD 操作补充成功路径断言：
  - `deleteEntry`/`rename`/`writeFile`/`createDir` resolve 后，`mockReadDir` 第二次调用（refresh）；
  - 重命名成功后 DOM 中输入框消失且旧文件名不再存在；
  - 新建成功后输入框消失且新文件名出现在列表中。

---

## 中优先级问题

### M1. `useFileTree.fullRefresh` 实际未被调用，且 F8 测试命名误导

- **源码位置**：`src/features/explorer/useFileTree.ts:191-206`
- **覆盖数据**：函数 `(anonymous_18)@191` 未命中；`fullRefresh` 语句行 192-203 未覆盖。
- **问题描述**：`fullRefresh` 是 hook 对外暴露的「need_rescan 全量刷新」入口，但 17 个测试文件均未调用它。`explorer-git-status.test.tsx` 中的 F8 用例名为「fullRefresh 调用 gitStatus 加载状态」，实际断言的是**初始 mount 时 `gitStatus` 被调用**，并非 `result.current.fullRefresh()` 的执行结果。该命名会掩盖 `fullRefresh` 本身的回归风险（如未处理 `gitStatus` 失败、未清空旧 gitStatusMap 等）。
- **建议**：
  - 重命名 F8 用例为「初始加载调用 gitStatus」；
  - 在 `use-file-tree.test.ts` 或 `explorer-git-status.test.tsx` 中新增独立用例：手动调用 `result.current.fullRefresh()`，验证其重新调用 `loadRoot` + `gitStatus`，且 `gitStatus` 失败时 `gitStatusMap` 被清空。

### M2. ExplorerPanel 焦点/失活/悬停交互与 activeExplorer 设置链路覆盖不足

- **源码位置**：`src/features/explorer/ExplorerPanel.tsx:91-157`
- **覆盖数据**：`activate`/`deactivate`、hover enter/leave、error banner dismiss 定时器等多处未命中。
- **问题描述**：
  - `explorer-focus.test.tsx` 仅 3 用例，验证容器 `tabIndex` 与 `usePanelFocus` 被调用，但未验证 `focusin` 后 `setActiveExplorer(explorerActions)`、`focusout` 后 `clearActiveExplorer` 被调用；
  - hover 时非选中行背景变化、选中行 hover 不覆盖选中色等交互无覆盖；
  - 错误横幅 3 秒自动 dismiss 的 `setTimeout` 清理逻辑无覆盖。
- **建议**：
  - 增加 `explorer-focus.test.tsx` 用例，通过 spy `setActiveExplorer` / `clearActiveExplorer` 验证完整焦点链路；
  - 增加 hover 样式断言；
  - 增加错误横幅自动消失断言（使用 fake timers）。

### M3. FileIcon 多个扩展名分支无测试覆盖

- **源码位置**：`src/features/explorer/FileIcon.tsx:34,47,50,57`
- **覆盖数据**：switch 分支中 `.pyw`、`.markdown`、`.less`、`.scss`、`.gitattributes` 等走向命中为 0。
- **问题描述**：`file-icon.test.tsx` 覆盖了常见扩展名（ts/js/rs/json/md/html/无扩展名/未知扩展名），但未覆盖 `.pyw`、`.markdown`、`.less`、`.scss`、`.gitattributes` 等。这些分支若被误删或合并，测试不会发现图标变化。
- **建议**：在 `file-icon.test.tsx` 中为上述扩展名各增加一个渲染断言，验证返回特定 Unicode 图标或至少文本非空。

### M4. FileTree 内联输入框边界分支覆盖不足

- **源码位置**：`src/features/explorer/FileTree.tsx:190,194-195,291,295,333,337,379,479,487,579,586,618,626`
- **覆盖数据**：大量菜单项 hover、输入框 Escape 取消、blur 空值、新文件/新文件夹/重命名输入框的 onChange 等分支未命中。
- **问题描述**：
  - 已覆盖：根级新建文件回车创建、blur 非空创建、Escape 取消；
  - 未覆盖：文件夹级新建文件/新建文件夹的 Escape 取消与 blur 空值、重命名输入框的 Escape 取消与 blur 空值、输入框 `onChange` 回调、菜单项 hover 高亮。
- **建议**：补充 FileTree 独立组件测试，覆盖三种内联输入（rename/newFile/newFolder）的 Escape/blur/onChange 全分支。

### M5. `useFileTree` 竞态与清理分支覆盖不完整

- **源码位置**：`src/features/explorer/useFileTree.ts:65-66,139-140,232,241`
- **覆盖数据**：
  - `loadRoot` 中 `rootPath` 为 null 且 `gen` 过期时的清空分支未命中；
  - `reloadPreservingExpanded` 中 `rootPathRef.current` 为 null 的分支未命中；
  - `onFsEvent` 去抖清理中的 `if (debounceRef.current)` 判断未命中；
  - `slterm:file-saved` 中 `savedPath` 为 falsy 时仍调用 `refreshExpanded` 的分支未命中。
- **问题描述**：这些正是并发/竞态场景下的关键防御代码。`explorer-rootpath-clear.test.tsx` 验证了正常清空与 gen 丢弃，但未覆盖 `rootPath=null` 同时旧 `loadRoot` 回调到达的竞态；也未覆盖 fs-event 清理时 `debounceRef` 为空的情况。
- **建议**：
  - 构造 `loadRoot(gen)` 在 `rootPath` 已切为 null 且 `gen` 过期后的回调场景，断言不会抛错；
  - 模拟组件卸载后 fs-event 回调触发，验证去抖 timer 被清理；
  - 测试 `slterm:file-saved` 事件 detail 不含 path 时仍刷新。

### M6. SidebarTree 错误降级与防御分支未覆盖

- **源码位置**：`src/features/sidebar/SidebarTree.tsx:55-56,342,369,484`
- **覆盖数据**：上述语句/分支未命中。
- **问题描述**：
  - `handleAddProject` 的 `catch` 分支（dialog 打开失败或用户取消后的降级）未测试；
  - `handleAddProject` 中将 `result` normalize 后 `if (!dirPath) return` 的防御分支未测试；
  - 项目行「打开 Hooks 配置」入口中 `if (!proj) return` 的防御分支未测试。
- **建议**：补充 sidebar-actions.test.ts 用例：
  - `dialog.open` reject / 返回数组 / 返回 null 时 store 不变且不抛错；
  - 种子数据异常（proj 不存在）时点击项目行菜单不抛错。

### M7. SidebarTree hover 与重命名中点击 stopPropagation 未覆盖

- **源码位置**：`src/features/sidebar/SidebarTree.tsx:96,99,165,168,247-248,251-252,264`
- **覆盖数据**：菜单项/项目行/页面行 hover 效果、重命名时 onClick 的 `stopPropagation` 等分支未命中。
- **问题描述**：这些交互细节与右键菜单稳定性、重命名输入框不被误触发有关。当前测试只验证菜单项存在与点击后的行为，未验证 hover 样式与事件冒泡控制。
- **建议**：补充 hover 样式断言与重命名中点击行不触发 `switchToPage` 的断言。

### M8. ExplorerPanel `handleOpenFile` 防御分支与去重聚焦失败分支未覆盖

- **源码位置**：`src/features/explorer/ExplorerPanel.tsx:185,187,227`
- **覆盖数据**：`activePageId` 缺失、`__dockviewApi` 缺失、`addPanel` catch 等分支未命中。
- **问题描述**：`explorer-file-viewer.test.tsx` 只验证了正常分派与重复打开聚焦。若未来 `handleOpenFile` 中前置守卫被误删或 `addPanel` 抛错未处理，可能导致异常上浮。
- **建议**：补充集成测试：
  - 无 `activePageId` 时双击文件不调用 addPanel；
  - 无 `__dockviewApi` 时不抛错；
  - `addPanel` reject 时显示错误横幅或至少不崩溃。

---

## 低优先级问题

### L1. `explorer-delete.test.tsx` E6 中「无选中 + Del」测试标题与断言矛盾

- **源码位置**：`src/__tests__/explorer-delete.test.tsx:536-547`
- **问题描述**：用例标题为「无选中 + Del → handler 返回 false（deleteSelected 不调用）」，但测试中注释写明「handler 仍调用了 deleteSelected」，且断言 `expect(actions.deleteSelected).toHaveBeenCalledOnce()`。该测试未验证 `handler` 返回值，标题与实际行为不符，容易造成维护者困惑。
- **建议**：
  - 若意图是验证 `deleteSelected` 内部处理 null，则标题改为「无选中 + Del → deleteSelected 被调用但内部不执行删除」；
  - 若意图是验证 shortcut 不派发，则应修改 `keyboard.ts` 或 action 实现并更新断言。

### L2. ExplorerPanel 错误横幅自动 dismiss 未测试

- **源码位置**：`src/features/explorer/ExplorerPanel.tsx:73-74`
- **覆盖数据**：`showError` 中的 `setTimeout` 清理分支未命中。
- **问题描述**：错误横幅 3 秒后自动清除，该定时器清理逻辑无覆盖。若定时器未清理或重复设置，可能导致内存泄漏或状态异常。
- **建议**：使用 `vi.useFakeTimers()` 验证错误横幅在 3 秒后消失，且卸载时定时器被清理。

### L3. 部分测试命名与计数口径不一致

- **问题描述**：
  - `sidebar-actions.test.ts` 中存在两个编号为「16」的用例（添加项目流程与项目删除确认），导致按 `it(` 计数时容易混淆；
  - `explorer-delete.test.tsx` E6 组与文件其他组编号不连续（E1-E5 后直接接 ShortcutRegistry 测试）。
- **建议**：统一用例编号，避免重复编号；或改用描述性命名而非纯数字编号。

### L4. `FileViewerRegistry` 单例恢复依赖测试代码，未验证真实启动 side-effect

- **源码位置**：`src/__tests__/file-viewer-registry.test.ts:198-227`
- **问题描述**：测试在 `afterEach` 中手动 `_reset()` 并重新注册 `html`/`htm`，以确保单例隔离。但未覆盖真实应用启动时 `FileViewerRegistry.ts` 模块级 side-effect 注册是否正确执行（虽然该逻辑简单，但属于「测试替代了真实环境」的潜在盲区）。
- **建议**：可考虑增加一个轻量集成测试，直接导入 `src/features/fileViewers` 单例并验证默认注册存在，不依赖测试中的手动恢复。

---

## 覆盖数据摘要

| 文件 | 行覆盖率 | 主要未覆盖区域 |
|------|----------|----------------|
| `src/features/explorer/ExplorerPanel.tsx` | 67.5% | `handleOpenInTerminal`、CRUD 成功 refresh、焦点/失活/hover、错误 dismiss、前置防御分支 |
| `src/features/explorer/FileTree.tsx` | 86.8% | 菜单项 hover、输入框 Escape/blur/onChange 边界 |
| `src/features/explorer/useFileTree.ts` | 88.5% | `fullRefresh`、rootPath=null 竞态分支、fs-event 清理分支 |
| `src/features/explorer/FileIcon.tsx` | 80.0% | `.pyw`、`.markdown`、`.less/.scss`、`.gitattributes` 等扩展名分支 |
| `src/features/sidebar/SidebarTree.tsx` | 89.3% | 错误 catch/防御分支、hover/重命名中 stopPropagation |

---

## 推荐后续行动

1. **立即补充**：`handleOpenInTerminal` 与 CRUD 成功路径刷新断言（高优先级）。
2. **修复测试命名**：`explorer-delete.test.tsx` E6 矛盾用例、`explorer-git-status.test.tsx` F8 用例。
3. **补充竞态/清理测试**：`useFileTree` 的 `fullRefresh`、过期 gen + rootPath=null、fs-event 卸载清理。
4. **补充 UI 交互测试**：hover 样式、焦点链路、错误横幅自动 dismiss、输入框边界。
5. **补充图标扩展名分支测试**：覆盖 `FileIcon.tsx` 中未命中的 switch 分支。
6. **补充侧栏错误降级测试**：dialog 异常返回值、`handleAddProject` catch、proj 不存在防御。

---

## 附：审查文件清单

- 测试文件（17）：`explorer-delete.test.tsx`、`explorer-file-viewer.test.tsx`、`explorer-keyboard.test.ts`、`explorer-rootpath-clear.test.tsx`、`explorer-refresh-preserve.test.tsx`、`explorer-root-contextmenu.test.tsx`、`explorer-notify.test.tsx`、`explorer-selection.test.tsx`、`explorer-git-status.test.tsx`、`explorer-sandbox-race.test.tsx`、`explorer-rename-state.test.tsx`、`explorer-rename-keyboard.test.tsx`、`explorer-focus.test.tsx`、`use-file-tree.test.ts`、`activeExplorer.test.ts`、`file-icon.test.tsx`、`sidebar-actions.test.ts`。
- 源码文件：`src/features/explorer/ExplorerPanel.tsx`、`FileTree.tsx`、`useFileTree.ts`、`FileIcon.tsx`、`activeExplorer.ts`、`keyboard.ts`；`src/features/sidebar/SidebarTree.tsx`。
