# L2 前端测试质量审查报告：Workspace/Layout/Tabs + Startup/Shutdown

## 1. 审查范围与方法

- **测试文件**：16 个，约 212 条用例
  - Workspace/布局：`workspace*.test.tsx`、`layout*.test.ts`、`default-layout-format.test.ts`、`panel-registry.test.ts`、`title-manager.test.ts`、`open-hooks-config-panel.test.ts`
  - 启动/关闭：`bootstrap.test.ts`、`startup-restore.test.ts`、`close-handler.test.ts`
- **关键源码**：`src/workspace/Workspace.tsx`、`PageDockviewHost.tsx`、`layoutSerde.ts`、`titleManager.ts`、`pageApis.ts`、`src/panelRegistry.ts`、`src/App.tsx`、`src/main.tsx`、`src/ipc/window.ts`
- **执行结果**：`npm test` 全量通过，117 文件 / 2020 用例通过
- **覆盖率**：使用 `@vitest/coverage-v8` 针对目标文件提取

| 文件 | Stmts | Branch | Funcs | Lines | 未覆盖行号 |
|------|-------|--------|-------|-------|-----------|
| All targeted | 71.96% | 64.65% | 69.23% | 73.70% | — |
| `src/App.tsx` | 85.71% | 62.50% | 100% | 87.23% | 76-84, 166, 183-186 |
| `src/main.tsx` | 100% | 80.00% | 100% | 100% | 27-43（catch 分支） |
| `src/ipc/window.ts` | 57.89% | 50.00% | 33.33% | 57.14% | 16-21, 42-43 |
| `src/workspace/PageDockviewHost.tsx` | 44.82% | 12.19% | 43.75% | 46.43% | 120-326, 343-353 等大量行 |
| `src/workspace/Workspace.tsx` | 68.31% | 57.89% | 72.22% | 70.64% | ...38, 154-158, 192 等 |
| `src/workspace/layoutSerde.ts` | 97.61% | 97.36% | 100% | 97.56% | 51 |
| `src/workspace/pageApis.ts` | 42.22% | 28.57% | 55.55% | 43.40% | 45-92 等 |
| `src/workspace/titleManager.ts` | 98.64% | 95.83% | 100% | 98.98% | 121, 161 |
| `src/panelRegistry.ts` | 100% | 100% | 100% | 100% | — |

> 注：`src/workspace/CLAUDE.md` 因 V8 覆盖率工具无法解析 Markdown 被自动排除，不影响代码覆盖率数字。

## 2. 分维度审查发现

### 2.1 断言有效性

- **`workspace-defaulttab.test.tsx` 测试的是手写 MockDefaultTab，而非 `PageDockviewHost.tsx` 中的真实 `DefaultTab`**。虽然事件结构回归断言有效，但无法保证生产组件在 Dockview 真实参数变化时行为一致，存在“用例绿、生产挂”的漂移风险。
- **`workspace-switch-order.test.tsx` 的“时序契约”用例是手动模拟顺序**（先调 mock `setProjectRoot` 再调 `setActivePage`），并非真正驱动 `Workspace.tsx` 的 `switchToPage` 或 `pageApis.ts` 的 `switchToPageShared`，因此对 DBG-5/DBG-9 核心契约的回归防护较弱。
- **`startup-restore.test.ts` 验证了加载状态流转，但未断言 `setProjectRoot` 必须在 `setActivePage` 之前执行**（DBG-6），也未覆盖 `__slterm_e2e_projectPending` 跳过逻辑。
- `close-handler.test.ts`、`title-manager.test.ts`、`layout-serde.test.ts`、`panel-registry.test.ts` 的断言与源码语义一致，未发现明显无效断言。

### 2.2 测试覆盖

- **重大缺口：`PageDockviewHost.tsx` 仅 44.82% 语句 / 12.19% 分支覆盖**。真实的 `DefaultTab`、右键菜单、`Watermark`、`RightHeader`、`handleReady`、`rebuildAndRecomputeTitles`、Save-As 处理等均未在 L2 中被直接测试。
- **重大缺口：`pageApis.ts` 仅 42.22% 语句覆盖**，只有 `openHooksConfigPanel` 被覆盖；`switchToPageShared`、`switchToPageAndFocus` 等页面切换核心函数无 L2 测试。
- **`App.tsx` 启动恢复路径中 `project_root` 回写循环（76-84）与 `requestUserAttention` 失败 catch（183-186）未覆盖**，启动顺序的关键防御没有回归用例。
- **`src/ipc/window.ts` 的 `onFocusChanged`、`setFocus` 未覆盖**（仅 `registerCloseHandler` 被测），窗口焦点生命周期缺少 L2 守卫。
- `Workspace.tsx` 中 `handlePageLayoutChange`、`onDeletePage`、Allotment `onChange` 等分支覆盖不足。
- `titleManager.ts`、`layoutSerde.ts` 覆盖较好，剩余少量未覆盖行属于边界容错分支。

### 2.3 测试设计质量

- **优点**：`title-manager.test.ts`、`layout-serde.test.ts`、`panel-registry.test.ts`、`default-layout-format.test.ts` 采用纯函数/状态断言，分支覆盖扎实，维护性高。
- **不足**：
  - Workspace 系列测试大量依赖 mock（Dockview、xterm、FitAddon、WebGL），虽然必要，但 `workspace-multi-instance.test.tsx` 仅通过 CSS `display` 和文本正则断言多实例存活，**没有真正验证 H6“终端跨页面存活”**（该保障实际由 L4 E2E 覆盖）。
  - `close-handler.test.ts` 验证了内部 flush 顺序，但**未验证 `registerCloseHandler` 是否真的会阻止默认关闭、是否调用了 Tauri 窗口销毁**。
  - `workspace-header-actions.test.tsx` 测试的是工厂函数，不是真实 RightHeader 渲染，对 `PageDockviewHost.tsx` 的集成行为覆盖有限。

### 2.4 Mock 使用合理性

- **`layout-serde.test.ts` 将 `isValidPanelType` mock 为仅允许 `terminal/editor/htmlviewer` 三种类型**，与当前真实的 `PANEL_TYPES`（含 `gitshow`、`diff`、`hooksConfig` 共 6 种）不一致。这是过时的 mock，导致新版面板类型的布局白名单过滤在 L2 中完全未验证。
- `close-handler.test.ts` 中 `TerminalRegistry.getAll` 的返回值使用 `{ term, sessionId, webglAddon, fitAddon }` 形状，与真实 `RegisteredTerminal` 不完全一致；当前测试只消费 `sessionId`，暂时安全，但存在 mock 漂移隐患。
- Workspace 测试中对 `xterm`、`webgl`、`ResizeObserver` 的 mock 属于必要隔离，使用合理。

### 2.5 稳定性风险

- `workspace-switch-order.test.tsx` 使用 `waitFor(..., { timeout: 3000 })`，在 CI 资源紧张时存在偶发失败风险。
- `workspace-multi-instance.test.tsx` 使用 `not.toMatch(/terminal-/)` 这类间接断言，若 Watermark 文案或页面 ID 生成规则变化，容易产生误报。
- `close-handler.test.ts` 使用 fake timers 且包含 `neverResolve` 场景，当前清理逻辑正确，但后续新增异步分支时容易引入悬挂 timer。
- 全量 2020 用例目前稳定通过，未发现 flaky 用例。

### 2.6 结构与可维护性

- 测试文件命名基本统一为 kebab-case，符合项目约定。
- `panel-registry.test.ts` 与 `workspace-file-panel-types.test.ts` 对 `FILE_PANEL_TYPES`、`isAlwaysRenderPanel` 存在重复断言，可合并或明确分工。
- `workspace-defaulttab.test.tsx` 将生产组件逻辑复制到测试文件中，维护负担较大；若 `DefaultTab` 图标渲染规则变化，测试与实现需要同步改两处。

## 3. 变异思想实验

对核心行为进行“如果源码发生以下有意或无意的改动，现有 L2 能否拦截”的推演：

| 变异点 | 预期现有 L2 结果 | 说明 |
|--------|-----------------|------|
| `titleManager.getTerminalTitle` 去掉 `counter++`、始终返回 `terminal-0` | `title-manager.test.ts` 多面板标题用例失败 | 覆盖有效 |
| `titleManager.findExistingEditor` 忽略 `suffix` | suffix 去重用例失败 | 覆盖有效，B10 受保护 |
| `layoutSerde.patchLegacyLayout` 移除 `activeGroup` 填充 | `layout-serde.test.ts` 用例 3、5、18 失败 | 覆盖有效 |
| `panelRegistry.PANEL_TYPES` 漏掉 `hooksConfig` | `panel-registry.test.ts` 失败 | 覆盖有效 |
| `pageApis.switchToPageShared` 把 `await setProjectRoot` 与 `setActivePage` 顺序对调 | **L2 无法拦截**（workspace-switch-order 是手动模拟） | 重大缺口 |
| `pageApis.switchToPageAndFocus` 把轮询上限从 50 改为 5 | **L2 无法拦截**（无此函数测试） | 重大缺口 |
| `PageDockviewHost.DefaultTab` 把 `event.tabIcon` 误写成 `event.params.tabIcon` | **L2 无法拦截**（测的是 MockDefaultTab） | 重大缺口 |
| `App.tsx` 启动恢复时不再 await `setProjectRoot` | **L2 无法拦截**（startup-restore 未断言顺序） | 重大缺口 |
| `src/ipc/window.ts` `onFocusChanged` 被删除 | **L2 无法拦截**（无测试） | 中等缺口 |

结论：结构良好的纯函数/状态测试具备较强的变异拦截能力；**集成路径和真实组件渲染是当前 L2 的明显短板**。

## 4. 问题清单

| 编号 | 问题 | 严重度 |
|------|------|--------|
| R1 | `PageDockviewHost.tsx` 真实组件覆盖严重不足（44.82% 语句 / 12.19% 分支），DefaultTab/Watermark/RightHeader/handleReady/onSaveAs 等关键行为在 L2 中未测 | 🔴 |
| R2 | `pageApis.ts` 仅覆盖 `openHooksConfigPanel`，`switchToPageShared`/`switchToPageAndFocus` 无 L2 测试，DBG-5/DBG-9 页面切换顺序缺乏回归防护 | 🔴 |
| R3 | `App.tsx` 启动恢复路径中 project_root 回写与 `requestUserAttention` catch 未覆盖，且 `startup-restore.test.ts` 未断言 `setProjectRoot` 先于 `setActivePage` | 🔴 |
| R4 | `src/ipc/window.ts` 的 `onFocusChanged`、`setFocus` 未覆盖，窗口焦点生命周期仅靠 E2E 兜底 | 🔴 |
| Y1 | `workspace-defaulttab.test.tsx` 测试手写 MockDefaultTab 而非生产组件，存在实现漂移风险 | 🟡 |
| Y2 | `workspace-switch-order.test.tsx` 的时序契约是手动模拟，未真正验证 `switchToPageShared` 的 await/setActivePage 顺序 | 🟡 |
| Y3 | `layout-serde.test.ts` mock 的 `isValidPanelType` 仅允许 3 种面板，与真实 6 种不一致，白名单过滤未覆盖新类型 | 🟡 |
| Y4 | `close-handler.test.ts` 未验证 `registerCloseHandler` 阻止默认关闭与真实窗口销毁行为 | 🟡 |
| Y5 | `workspace-multi-instance.test.tsx` 仅通过 CSS display 断言，未真正验证 H6 终端跨页面存活 | 🟡 |
| Y6 | `main.tsx` 的 bootstrap 错误 catch 分支未覆盖 | 🟡 |
| Y7 | `titleManager.ts`、`layoutSerde.ts` 残留少量未覆盖容错行 | 🟡 |
| G1 | `default-layout-format.test.ts` 仅测 `makeEmptyLayout` 函数，未验证新建页面确实使用该空布局 | 🟢 |
| G2 | `panel-registry.test.ts` 与 `workspace-file-panel-types.test.ts` 对 `FILE_PANEL_TYPES` 的断言重复 | 🟢 |

## 5. 改进建议

1. **补测 `PageDockviewHost.tsx`**：至少新增真实 `DefaultTab` 渲染测试（验证 `params.tabIcon` 的 img/span 分支、参数变化事件）、`RightHeader` 与 `Watermark` 的 addPanel 路径、`handleReady` 不兜底创建终端、`onSaveAs` 重算标题。
2. **补测 `pageApis.ts` 核心函数**：`switchToPageShared` 必须断言 `setProjectRoot` 在 `setActivePage` 之前且等待完成；`switchToPageAndFocus` 验证轮询聚焦与超时降级；`openHooksConfigPanel` 现有测试可保留。
3. **补测 `App.tsx` 启动恢复顺序**：在 `startup-restore.test.ts` 中断言 `setProjectRoot` 在 `setActivePage` 之前被调用，并覆盖 `__slterm_e2e_projectPending` 跳过分支。
4. **补测 `src/ipc/window.ts`**：为 `onFocusChanged` 与 `setFocus` 增加契约测试，确保 window focus 相关回调在启动/关闭链路中可用。
5. **修正 `layout-serde.test.ts` 的 mock**：让 `isValidPanelType` 对齐真实 `PANEL_TYPES`，并增加 `gitshow`/`diff`/`hooksConfig` 面板类型的白名单过滤用例。
6. **合并/精简重复断言**：将 `workspace-file-panel-types.test.ts` 中与 `panel-registry.test.ts` 重复的 `FILE_PANEL_TYPES` 断言集中到 `panel-registry.test.ts`，前者只保留与 `isAlwaysRenderPanel` 相关的用例。
7. **减少手动模拟式测试**：`workspace-switch-order.test.tsx` 中“手动验证时序”的用例建议改为直接调用 `pageApis.switchToPageShared` 或 `Workspace.switchToPage` 的集成测试。

## 6. 结论

Workspace/Layout/Tabs 领域的纯函数与状态管理测试（`titleManager`、`layoutSerde`、`panelRegistry`）质量较高，断言有效、分支覆盖扎实。但 **真实组件渲染与页面切换集成路径覆盖严重不足**：`PageDockviewHost.tsx`、`pageApis.ts`、`App.tsx` 启动顺序、`src/ipc/window.ts` 的多个关键行为在 L2 中基本处于“裸奔”状态，主要依赖 L4 E2E 兜底。建议优先补测 R1-R4 四项，以降低回归修复成本。
