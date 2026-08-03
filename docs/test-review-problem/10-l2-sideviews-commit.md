# 10 L2 侧栏视图+Commit 测试 Review

## 元信息

- **领域**：侧栏视图系统（`src/features/sideViews/` + `src/stores/sideBar.ts`）+ Commit 视图（`src/features/commit/`）
- **测试文件数**：8
- **用例数**：180（sideBarState 50 + sideViewRegistry 7 + sideBar 19 + activityBar 29 + sideBarArea 14 + workspace-sideviews 13 + commit-view 35 + commit-context-menu 13）
- **覆盖率概况**：纯函数/状态机（`sideBarState`）覆盖较完整；ActivityBar 拖拽的 `index` 落点参数与 zone 边界未真正锁定；Commit 的去重/错误分支/右键菜单交互有缺口；`sideBar.ts` 的 `cancelPendingSave` 零覆盖。
- **审查日期**：2026-08-04

## 覆盖率缺口

按业务风险分级：

### 核心逻辑零覆盖/断言缺失

- `src/__tests__/activityBar.test.tsx` 全部 drop 用例：仅断言 `moveButton(id, zone)` 的 `zone`，从未断言 `index` 参数——`computeDropTarget` 的落点索引实质无守卫。
- `src/stores/sideBar.ts:143-149`：`cancelPendingSave()` 零覆盖；App 关闭钩子依赖此函数冲掉待保存状态。
- `src/features/commit/useCommitStatus.ts:88-108`：debounce timer 的卸载清理、200ms 内多次 fs-event 只刷最后一次未覆盖。
- `src/features/commit/openCommitFile.ts:47,65,112,122`：未知状态直接 return、rootPath 推导失败、`addPanel` 异常 catch、`recomputeTitles` 更新标题 4 条路径无测试。

### 边界分支未覆盖

- `src/features/sideViews/ActivityBar.tsx:93-99`：`resolveTargetZone` 用容器垂直中点判定，测试用 clientY=20/400 远离边界，把中点改到 1/3 或 2/3 高度，现有 zone 用例仍绿。
- `src/features/sideViews/sideBarState.ts:67-70` / `src/stores/sideBar.ts:58-61`：`clamp` 的 `!Number.isFinite(value)` 分支（NaN/Infinity）未覆盖。
- `src/features/sideViews/SideBarArea.tsx:79`：`onChange` 的 `total <= 0` 除零守卫未覆盖。
- `src/features/commit/CommitFileList.tsx:130,133,253`：菜单项 hover 背景、renamed 无 `oldPath` 的 `?? undefined` 回退未测。
- `src/features/commit/commitContextMenu.ts:76,85`：删除操作（added/untracked）的异常 catch 分支疑似未覆盖（覆盖率报告显示 0/2）。
- `src/features/commit/useCommitStatus.ts:49-56`：`rootPath` 缺失 + `gen` 检查组合路径未直接覆盖。

### 低风险/可接受缺口

- `src/features/sideViews/sideViewDefs.ts:38`：注册副作用文件被多数测试 mock，实际注册由 Workspace 集成/E2E 覆盖，L2 不重复属合理。
- `src/features/sideViews/ActivityBar.tsx:108,110,115,202`：active/hover/indicator 的部分视觉分支风险低。

## 问题清单

### P1 [断言有效性/严重] ActivityBar 拖拽测试不校验 `moveButton` 的 `index` 参数

- **风险等级**：🔴 高
- **位置**：`src/__tests__/activityBar.test.tsx:260-495`（多个 drop 用例）
- **问题描述**：所有 drop 用例仅 `expect(moveSpy.mock.calls[0][1]).toBe("top"/"bottom")`，从未校验 `calls[0][2]`（insert index）。`computeDropTarget` 的落点索引逻辑（按钮上半/下半/数组末尾）完全无守卫。
- **代码片段**：
  ```ts
  expect(moveSpy).toHaveBeenCalledWith(
    expect.any(String),
    "bottom"
    // index 未断言
  );
  ```
- **改进建议**：每个 drop 用例追加 `expect(moveSpy.mock.calls[0][2]).toBe(expectedIndex)`；新增同按钮上/下半区插入位置差异用例。
- **变异推演**：若把 `computeDropTarget` 改为恒返回 `{ zone, index: 0 }`，SB-19.12/17/19-22/24 仍绿。

### P2 [测试覆盖度/严重] `sideBar.ts` 的 `cancelPendingSave` 零覆盖

- **风险等级**：🔴 高
- **位置**：`src/stores/sideBar.ts:143-149`
- **问题描述**：该函数在 App 关闭钩子中被调用，用于清除待 flush 的 debounce timer、防止关窗竞态写盘。无任何用例验证它。
- **改进建议**：在 `sideBar.test.ts` 中触发一次状态变更产生待保存 timer，调用 `cancelPendingSave()` 后推进 fake timers，断言 `saveSettings` 不再被调用。
- **变异推演**：把 `cancelPendingSave` 实现清空，当前全部测试仍绿。

### P3 [测试覆盖度/严重] `useCommitStatus` 的 debounce 清理与去抖未覆盖

- **风险等级**：🔴 高
- **位置**：`src/features/commit/useCommitStatus.ts:88-108`
- **问题描述**：`onFsEvent` 的 200ms debounce 与 unmount 时 `clearTimeout(timerRef.current)` 均未被测试。快速连续文件事件可能触发多次 `gitStatus`，卸载时未清理 timer 会导致 React 状态更新警告。
- **改进建议**：新增用例：
  1. 连续触发 3 次 fs-event，推进 50ms/100ms/250ms，验证 `gitStatus` 仅被调用 1 次；
  2. 激活 timer 后卸载 hook，验证 `clearTimeout` 被调用。
- **变异推演**：把 `FS_EVENT_DEBOUNCE_MS` 改为 0 或删除 cleanup 中的 `clearTimeout`，现有测试仍绿。

### P4 [测试覆盖度/中高] `openCommitFile` 异常与守卫路径大量未覆盖

- **风险等级**：🟡 中高
- **位置**：`src/features/commit/openCommitFile.ts:47,65,112,122`
- **问题描述**：
  - `if (!dispatch) return;`（未知 status）未测；
  - `const rootPath = project?.rootPath; if (!rootPath) return;` 未测；
  - `try { api.addPanel(...) } catch { return; }` 未测；
  - `recomputeTitles(api, pageId)` 更新标题未断言。
- **改进建议**：新增 4 条用例：status="ignored" 不调 `addPanel`；project 无 `rootPath` 时 return；mock `addPanel` throw 不抛到外层；同名冲突场景验证 `setTitle` 被调用。
- **变异推演**：删除 `if (!dispatch) return;`，对非 git 状态调用会崩溃，但当前无此用例。

### P5 [测试覆盖度/中] `resolveTargetZone` 边界阈值未被真正锁定

- **风险等级**：🟡 中
- **位置**：`src/features/sideViews/ActivityBar.tsx:93-99`
- **问题描述**：zone 判定基于容器垂直中点 `rect.top + rect.height / 2`。测试中容器 height=600，用例 clientY 取 20/400/350/30 等，远离中点 300。把判定阈值改到 1/3 或 2/3 高度，现有 zone 用例仍会通过。
- **改进建议**：新增 clientY 恰等于 midpoint、midpoint-1、midpoint+1 的边界用例。
- **变异推演**：把 `rect.height / 2` 改为 `rect.height / 3`，SB-19.17/22/23 仍绿。

### P6 [测试覆盖度/中] `sideBarState.moveButtonPure` 的 R7 场景覆盖不充分

- **风险等级**：🟡 中
- **位置**：`src/features/sideViews/sideBarState.ts:105-151`
- **问题描述**：现有 R7 用例中目标区为空。若代码错误地在跨区未打开时也替换目标区已有视图，测试无法发现。
- **改进建议**：新增用例：目标区 bottom 已打开 projects，将未打开的 explorer 拖入 bottom，预期 bottom 仍保持 projects（open 不变），仅 zones 归属变化。
- **变异推演**：把 R7 实现改成与 R6 相同（总是 set targetZone），当目标区非空时现有测试不会红。

### P7 [测试覆盖度/中] `SideBarArea` 的 `total <= 0` 守卫与极端 splitRatio 未覆盖

- **风险等级**：🟡 中
- **位置**：`src/features/sideViews/SideBarArea.tsx:75-82`
- **问题描述**：`onChange` 中的 `if (total <= 0) return;` 除零守卫未覆盖；splitRatio 进入 effect 重置 0.5 的边界也未测。
- **改进建议**：mock Allotment `onChange` 触发 sizes=[0,0]、sizes=[-100,100]、sizes=[1,0]，验证不抛且 store 不被写入。
- **变异推演**：删除 `if (total <= 0) return;`，传入 [0,0] 会 `NaN` 写 store，但当前测试仍绿。

### P8 [测试覆盖度/中] `CommitFileList` 右键菜单交互与 oldPath 回退未覆盖

- **风险等级**：🟡 中
- **位置**：`src/features/commit/CommitFileList.tsx:130,133,253`
- **问题描述**：菜单项 `mouseEnter`/`mouseLeave` 无断言；renamed 无 `oldPath` 时 `entry.oldPath ?? undefined` 未测；右键菜单位置仅断言 left/top 存在。
- **改进建议**：断言菜单项 hover 背景色 token 变化；新增 renamed 状态但 `oldPath` 为 null 的用例，验证 `params.oldPath` 为 `undefined`。
- **变异推演**：把 `entry.oldPath ?? undefined` 改为 `entry.oldPath`，renamed 无 oldPath 时 diff 面板会拿到 `null`，但当前无此用例。

### P9 [测试覆盖度/中] `commitContextMenu` 删除失败 catch 未覆盖

- **风险等级**：🟡 中
- **位置**：`src/features/commit/commitContextMenu.ts:76,85`
- **问题描述**：覆盖率报告显示 added/untracked 删除的 `catch` 分支 0/2。删除操作异常时若不 catch 会抛到 UI。
- **改进建议**：新增用例：mock `deleteEntry`/`gitUnstage` reject，验证 `console.error` 被调用且菜单 action 不抛。
- **变异推演**：删除 catch 块，删除失败时测试会抛错但当前无此用例。

### P10 [Mock 合理性/中] ActivityBar 拖拽测试高度理想化，无法回归真实 hit-test 问题

- **风险等级**：🟡 中
- **位置**：`src/__tests__/activityBar.test.tsx:106-130`（`installRectSpy`）
- **问题描述**：测试完全 mock `getBoundingClientRect`，且给 zone div 高度=0。真实浏览器中 zone div 零高度会导致 `dragover`/`drop` 无法触发（CLAUDE.md 明确这是外层容器统一处理的根因）。mock 下无法回归该架构决策。
- **改进建议**：保留现有 L2 单元测试；L4 E2E 已覆盖拖拽，无需重复。但应在报告中标注此风险由 E2E 兜底。
- **变异推演**：把 `onDragOver`/`onDrop` 绑定从外层容器改回 zone div，当前 L2 测试仍绿，但真实浏览器/E2E 会失败。

### P11 [断言有效性/中] `workspace-sideviews` 对 `SideBarArea` props 仅做 `typeof` 检查

- **风险等级**：🟡 中
- **位置**：`src/__tests__/workspace-sideviews.test.tsx:284-300`
- **问题描述**：仅断言 `typeof props.switchToPage === "function"`，未验证函数身份。任意函数都能通过。
- **改进建议**：spy Workspace 传入的 `switchToPage`/`onDeletePage` 引用，断言 SideBarArea 收到的就是同一函数。
- **变异推演**：SideBarArea 收到无关函数，测试仍绿。

### P12 [用例设计/低] commit-view 中 B10 反向用例实际测的是 `titleManager` 而非 `openCommitFile`

- **风险等级**：🟢 低
- **位置**：`src/__tests__/commit-view.test.tsx:488-497`
- **问题描述**：用例标题为“同文件不同 suffix 不误聚焦”，但直接调用 `titleManager.findExistingEditor`，未经过 `openCommitFile` 的 suffix 传递路径。
- **改进建议**：改为通过 `openCommitFile` 打开普通 editor 后再用 modified 调用，验证 `addPanel` 仍触发。
- **变异推演**：在 `openCommitFile` 中删除 suffix 传递，此用例仍绿。

### P13 [稳定性风险/中] commit-view rootPath 切换用例混用 fake timers 与 `waitFor`

- **风险等级**：🟡 中
- **位置**：`src/__tests__/commit-view.test.tsx:532-642`
- **问题描述**：该用例在 fake timers 下用 `waitFor` 等待 DOM 文本，可能因 timer 推进不同步导致 flaky。
- **改进建议**：统一使用 fake timers 推进到稳定态后再断言，或纯用 `waitFor` 不用 fake timers。
- **变异推演**：删除 generation 检查逻辑，该用例在时序巧合下仍可能绿。

### P14 [测试覆盖度/低] `sanitizeSideBar` 的 NaN/Infinity 分支未覆盖

- **风险等级**：🟢 低
- **位置**：`src/features/sideViews/sideBarState.ts:67-70`、`src/stores/sideBar.ts:58-61`
- **问题描述**：`clamp` 中 `!Number.isFinite(value)` 分支未覆盖。
- **改进建议**：新增用例传入 `NaN`/`Infinity`/`-Infinity`，验证返回 min。
- **变异推演**：把 `!Number.isFinite(value)` 改为 `false`，当前测试仍绿。

### P15 [结构与可维护性/低] `commit-view.test.tsx` 过长且职责混合

- **风险等级**：🟢 低
- **位置**：整个文件
- **问题描述**：单文件 850+ 行，混合状态机、列表渲染、双击分派、右键菜单、fs-event 刷新、STATUS_PANEL_MAP 等。新增用例时容易破坏既有 setup。
- **改进建议**：按关注点拆分为 `commit-view.test.tsx`（状态机/列表）、`commit-open-file.test.ts`（分派与去重）、`commit-context-menu-ui.test.tsx`（右键菜单交互）。
- **变异推演**：不适用。

## 变异推演核心用例清单

对约 25% 核心行为用例执行思想实验，结果如下：

| 核心行为 | 变异内容 | 当前测试能否发现 | 说明 |
|---------|---------|----------------|------|
| `toggleViewPure` R1/R2 | 把 `open[zone] === id` 改为 `!==` | ✅ 能 | R2 关闭测试会变红 |
| `moveButtonPure` R6 | 跨区移动时不清空源区 open | ✅ 能 | S3/S4 测试会变红 |
| `moveButtonPure` R7 | 未打开跨区时也替换目标区 | ❌ 不能 | 现有 R7 目标区为空 |
| `computeDropTarget` | 按钮中间线判定由 `<` 改 `<=` | ✅ 能 | SB-19.24  exact mid 会变红 |
| `computeDropTarget` | 恒返回 `index: 0` | ❌ 不能 | 无 index 断言 |
| `resolveTargetZone` | 中点改为 1/3 高度 | ❌ 不能 | 用例远离边界 |
| `deriveLayout` | 交换 single-top/single-bottom | ✅ 能 | sideBarArea 测试会变红 |
| `sanitizeSideBar` clamp | 删除 NaN/Infinity 回退 | ❌ 不能 | 无对应用例 |
| `useCommitStatus` debounce | 删除 cleanup 中的 clearTimeout | ❌ 不能 | 无卸载/去抖用例 |
| `openCommitFile` | 删除 `if (!dispatch) return;` | ❌ 不能 | 无 unknown status 用例 |
| `openCommitFile` | `addPanel` 异常未 catch | ❌ 不能 | 无异常用例 |
| `commitContextMenu` | 删除删除操作的 catch | ❌ 不能 | 无删除失败用例 |
| `STATUS_PANEL_MAP` | conflict 映射错为 editor | ✅ 能 | commit-context-menu 测试会变红 |
| `commitContextMenu` ROLLBACK/DELETE | 互换两个状态集合 | ✅ 能 | 现有菜单项断言会变红 |
| `cancelPendingSave` | 实现清空 | ❌ 不能 | 零覆盖 |

## 总体评价

- **优势**：`sideBarState` 纯函数状态机（50 用例）覆盖完整，包含 S1-S6 场景序列；`computeDropTarget` 自身边界测试较好；Commit 状态机四态与 STATUS_PANEL_MAP 策略表测试到位；测试使用真实 Zustand store + `vi.hoisted()` 共享 mock 状态，架构符合项目规范。
- **主要短板**：
  1. **ActivityBar 拖拽的 index 参数零断言**——核心交互的落点精度无守卫；
  2. **`cancelPendingSave` 零覆盖**——关窗数据一致性风险；
  3. **`useCommitStatus` debounce 清理与去抖未覆盖**——稳定性隐患。
- **建议优先级**：先补 P1、P2、P3；再补 P4、P6、P7、P8、P9；P5、P10-P15 可在后续迭代处理。
