# L2 前端测试质量评审：Editor / Diff / GitShow 面板域

## 元信息

- **评审日期**：2026-08-04
- **评审人**：Claude Code
- **评审性质**：静态走查，未运行测试/构建
- **工作目录**：`D:\data\learn\code\slTerminal`
- **输出文件**：`docs/test-review-problem/07-l2-editor-diff.md`

## 范围与基线

### 评审范围

| 测试文件 | 用例数 | 对应源码 |
|---|---|---|
| `src/__tests__/use-code-mirror.test.ts` | 34 | `src/panels/editor/useCodeMirror.ts` |
| `src/__tests__/git-gutter.test.ts` | 28 | `src/panels/editor/gitGutter.ts` |
| `src/__tests__/language-mapping.test.ts` | 23 | `src/panels/editor/useCodeMirror.ts::getLanguageExtension` |
| `src/__tests__/editor-confirm.test.ts` | 11 | `src/panels/editor/useCodeMirror.ts`（fs-event 监听） |
| `src/__tests__/editor.test.tsx` | 9 | `src/panels/editor/EditorPanel.tsx` |
| `src/__tests__/editor-font.test.ts` | 8 | `src/panels/editor/useCodeMirror.ts` |
| `src/__tests__/editor-keyboard.test.ts` | 7 | `src/panels/editor/keyboard.ts`、`activeEditor.ts` |
| `src/__tests__/active-editor.test.ts` | 5 | `src/panels/editor/activeEditor.ts` |
| `src/__tests__/diff-alignment.test.ts` | 16 | `src/panels/diff/alignment.ts` |
| `src/__tests__/diff-panel.test.tsx` | 30 | `src/panels/diff/DiffPanel.tsx` |
| `src/__tests__/gitshow-panel.test.tsx` | 19 | `src/panels/gitshow/GitShowPanel.tsx` |
| **合计** | **190** | — |

### 基线文档

- `src/panels/CLAUDE.md`
- `.claude/test-inventory.md`

### 覆盖数据基线

通过 `node docs/test-review-problem/coverage/extract-uncovered.cjs` 获取：

| 源码文件 | 行覆盖 | 未覆盖行 | 未覆盖函数 |
|---|---|---|---|
| `src/panels/editor/useCodeMirror.ts` | 90.8% | 122, 152, 195, 237, 261-262, 265, 268-270, 274-275, 280, 282, 302, 321, 367-368, 404, 417, 452 | — |
| `src/panels/editor/gitGutter.ts` | 87.8% | 52, 54, 59, 126-127, 262, 271, 292-293, 295, 318, 325 | `toDOM_4`（DeletedMarker）、`updateDiffGutter`、`clearDiffGutter`、`updateHeadDiffGutter`、`clearHeadDiffGutter` |
| `src/panels/diff/DiffPanel.tsx` | 63.9% | 大量未覆盖行 | — |
| `src/panels/diff/alignment.ts` | 100.0% | 无 | —（但分支未完全覆盖） |
| `src/panels/gitshow/GitShowPanel.tsx` | 88.9% | 107, 123, 134-135, 175, 187-190, 195, 205-206 | — |

## 评审方法

1. 逐文件阅读 11 个测试文件，对照源码与 `src/panels/CLAUDE.md` 验收要求。
2. 结合覆盖率脚本输出定位未覆盖代码路径。
3. 对 20% 左右核心用例做“变异测试”思想实验：假设源码关键行被删除/交换，当前测试能否发现。
4. 评估断言真实性、mock 合理性、稳定性风险、可维护性。

## 问题清单

### 红色（严重）

#### R1. `diff-panel.test.tsx` 的“保存链”用例有名无实

- **位置**：`src/__tests__/diff-panel.test.tsx:169-194`
- **源码**：`src/panels/diff/DiffPanel.tsx:351-391`
- **问题描述**：该用例注释声称验证 `writeFile → gitDiff → updateDiffGutter`，但实际只验证了 `mockWriteFile` 与 `mockGitDiff` 两个 mock 函数“已定义”。从未触发 `handleSave`，也未断言保存后 `gitDiff` 被再次调用、双侧 gutter 被更新、占位被刷新。
- **风险**：`DiffPanel.tsx:351-391` 的保存后刷新链（`fs.writeFile` → `gitDiff` → `updateDiffGutter`/`updateHeadDiffGutter` → `refreshPlaceholders` → 自定义事件）无任何回归防护。若未来有人删除 `refreshPlaceholders()` 调用或 gutter 刷新逻辑，测试仍通过。
- **引用代码**：
  ```ts
  // diff-panel.test.tsx:185-193
  // 这里只验证 IPC 调用链：writeFile 成功后 gitDiff 再次被调
  // handleSave 由 usePanelFocus mock 捕获不到——改为验证组件挂载时 IPC 调用

  // 验证 writeFile + gitDiff 在 import 中的存在（编译期保证）
  expect(mockWriteFile).toBeDefined();
  expect(mockGitDiff).toBeDefined();
  ```
- **建议**：补一条真实触发保存的用例：通过 mocked `usePanelFocus` 的 `activate` 回调获取 `editorActions`，调用 `save()`，然后断言 `mockWriteFile` 被调用、保存完成后 `mockGitDiff` 再次被调用，且 `updateDiffGutter`/`updateHeadDiffGutter` 效果被 dispatch。

#### R2. `DiffPanel.tsx` 整体行覆盖仅 63.9%，关键路径大面积缺失

- **位置**：`src/panels/diff/DiffPanel.tsx`
- **问题描述**：190 个用例中 Diff/GitShow 占 65 个，但 `DiffPanel.tsx` 行覆盖最低。大量与验收直接相关的路径未覆盖：
  - **占位刷新同步**：`refreshPlaceholders`（275-302）在 hunks 为空/有值两条分支、左右 view 同时 dispatch 仅在组件挂载后通过 `setTimeout 50ms` 触发，测试未直接断言结果。
  - **保存后刷新**：右侧保存后调用 `updateDiffGutter`/`clearDiffGutter` + `updateHeadDiffGutter`/`clearHeadDiffGutter` + `refreshPlaceholders` 的路径未覆盖（同 R1）。
  - **左侧 `.git` 变更刷新**：`onFsEvent` 检测 `/.git/` 并重新 `gitFileAtHead` 后更新左侧文档（483-501）无任何测试。
  - **外部修改脏文件确认**：右侧外部修改时 `dirtyRef.current` 为 true 的分支（457-468）未覆盖，仅覆盖净态自动重载（295-320）。
  - **滚动同步重新绑定**：视图重建（`state.kind`/`headContent`/`workdirContent` 变化导致 effect 重新执行）后，`useEffect:306-347` 需要重新绑定 scroll listener；当前测试只验证初始绑定。
  - **大文件拒绝/警告**：239-250 对 HEAD 和工作区的大文件处理分支未覆盖。
- **风险**：这是本期新增的最复杂面板之一，但 L2 测试深度不足，关键交互链只能依赖 L4 E2E 兜底；一旦 E2E 环境波动，回归难以在 L2 阶段发现。
- **建议**：按源码分支补全 L2 用例，优先覆盖保存链、占位刷新、`.git` 刷新、脏文件确认。

#### R3. `useCodeMirror.ts` 大文件处理与保存失败路径缺乏直接回归用例

- **位置**：`src/panels/editor/useCodeMirror.ts:150-210`、`259-271`、`273-276`
- **问题描述**：
  - 大文件拒绝分支（`sizeHint > MAX_FILE_SIZE_BYTES`，259-262）和大文件警告分支（263-271）仅在 `gitshow-panel.test.tsx` 中间接以 mock 方式验证，**编辑器自身没有直接测试**。
  - `fs.writeFile` 失败分支（173-178）无测试。当前 `use-code-mirror.test.ts` 的保存用例均走成功路径。
  - `window.confirm` 大文件警告的“取消”分支（268-270）无测试。
- **风险**：大文件处理是 `useCodeMirror.ts` 的关键安全路径（避免 10MB+ 文件拖垮内存），但编辑器测试未直接锁定；保存失败时 `window.alert` 路径也无回归防护。
- **建议**：在 `use-code-mirror.test.ts` 中补：
  - 打开 >10MB 文件时文档被替换为拒绝文案且 `filePathRef.current` 被清空。
  - 打开 >1MB 文件时 `window.confirm` 返回 false 的取消路径。
  - `fs.writeFile` mock reject 时弹出 `window.alert` 且不派发保存事件。

#### R4. `gitshow-panel.test.tsx` 大文件警告与参数切换用例断言薄弱

- **位置**：`src/__tests__/gitshow-panel.test.tsx:269-279`、`319-387`
- **问题描述**：
  - 大文件用例仅断言“渲染容器”，未验证顶部是否附加了大文件警告 header（`GitShowPanel.tsx:134-135`）。
  - “params 变化后先显示 loading 态”用例（351-387）使用 `container.querySelector('div[style*="overflow: clip"]')` 作为 content 存在的断言，但 error 态和 content 态的容器样式都包含 `overflow: clip`（centerStyle 不含，content div 含），loading 态是 `span`。该断言实际无法区分 content 与旧 content 残留，且未验证旧 EditorView 是否被销毁。
- **风险**：大文件警告回归、参数切换时 CM6 视图泄漏或 loading 态未正确切换，均无法在 L2 发现。
- **建议**：大文件用例断言 `displayText` 包含警告文案；参数切换用例断言 `mockEditorViewDestroy` 在 rerender 后被调用，且 loading `span` 真实出现后再消失。

### 黄色（中等）

#### Y1. `gitGutter.ts` 的 dispatch wrapper 函数未被直接测试

- **位置**：`src/panels/editor/gitGutter.ts:261-328`
- **问题描述**：`updateDiffGutter`、`clearDiffGutter`、`updateHeadDiffGutter`、`clearHeadDiffGutter` 四个 wrapper 函数在覆盖率报告中显示为未覆盖函数。`git-gutter.test.ts` 仅测试了 `buildRangeSet`/`buildHeadRangeSet` 的 RangeSet 映射和 `diffGutter()` 扩展装配，未测试这些 wrapper 如何 dispatch `StateEffect`。
- **风险**：wrapper 是 `useCodeMirror.ts`、`DiffPanel.tsx`、`GitShowPanel.tsx` 中实际调用的 API。若有人误将 `setDiffMarkers.of(hunks)` 改为 `setHeadDiffMarkers.of(hunks)`，当前测试不会发现。
- **建议**：补 4 条轻量用例：创建真实 EditorView（带 `diffMarkersField`/`headDiffMarkersField`），调用 wrapper 后断言 StateField 值变化。

#### Y2. `alignment.ts` 分支覆盖不足

- **位置**：`src/panels/diff/alignment.ts:38`、`44`
- **问题描述**：虽然行覆盖 100%，但未覆盖 `key >= 0` 守卫的 false 分支。所有测试用例的 `newStart` 均 >=1，未验证 `newStart=0` 这种非法/边界输入。
- **风险**：非法 diff hunk 输入时 `computeAlignment` 行为未锁定；若未来代码去掉 `if (key >= 0)` 保护，测试仍通过。
- **建议**：补一条 `newStart=0` 的用例，断言结果 Map 为空或行为符合预期。

#### Y3. `diff-panel.test.tsx` 滚动同步测试依赖固定延时，存在 flakiness

- **位置**：`src/__tests__/diff-panel.test.tsx:198-291`
- **问题描述**：三个滚动同步用例在断言前 `await new Promise((r) => setTimeout(r, 200))`，等待 `useEffect:306-347` 的 100ms setTimeout 绑定 listener。该固定延时在慢机器或 CI 负载高时可能不稳定。
- **风险**：测试间歇性失败会削弱对滚动同步的信任。
- **建议**：将等待条件改为轮询 listener 是否绑定（例如 mock `addEventListener` 并等待被调用），或等待 `state.kind === "ready"` 后再等待一个 microtask。

#### Y4. `editor-confirm.test.ts` 未覆盖 `justSavedRef` Set 的多实例并发路径

- **位置**：`src/panels/editor/useCodeMirror.ts:143`、`365-369`
- **问题描述**：源码注释明确说明使用 `Set<string>` 而非 boolean 是为了“多编辑器同时保存时各自路径独立，不互相影响”。但测试只验证了单实例下自己保存后跳过自动重载，未覆盖多实例场景。
- **风险**：若有人将 `Set` 改回 boolean，当前测试不会发现。
- **建议**：补一条思想实验用例：实例 A 保存路径 `a.ts`、实例 B 收到 `a.ts` 的 Modify 事件，应仍对 B 执行自动重载；或直接用两个 `useCodeMirror` 实例验证 Set 隔离。

#### Y5. `gitshow-panel.test.tsx` 未验证 `editorFontSize` 变化后的 reconfigure

- **位置**：`src/panels/gitshow/GitShowPanel.tsx:172-180`
- **问题描述**：用例 18（`CM6 creation effect does not recreate view on fontSize change`）只验证了 `createEditorFontExtension` 被调用，但 `fontCompartment.current.reconfigure` 发生在真实 CM6 中，mock 的 `Compartment.of` 无法验证 reconfigure 效果。
- **风险**：若 `editorFontSize` effect 依赖项写错导致 view 被销毁重建，当前测试无法捕获。
- **建议**：mock `fontCompartment.current.reconfigure` 或捕获 `view.dispatch` 调用，断言字号变化时 dispatch 了 reconfigure effect 且未重新 `EditorState.create`。

### 绿色（提示/低优先级）

#### G1. `editor.test.tsx` 仅做浅层 prop 透传验证

- **位置**：`src/__tests__/editor.test.tsx`
- **问题描述**：该文件完全 mock 了 `useCodeMirror`，只验证容器样式和 prop 传递，未验证任何真实编辑器行为。
- **建议**：保留作为组件集成契约测试，但应明确标注其定位；若要增强，可补充挂载真实 hook 的用例。

#### G2. `active-editor.test.ts` 与 `editor-keyboard.test.ts` 功能正确但较薄

- **位置**：`src/__tests__/active-editor.test.ts`、`src/__tests__/editor-keyboard.test.ts`
- **问题描述**：active pointer 模式的核心价值在于多实例下“后设置的覆盖先前的”和“clear 仅匹配时生效”，这两处已覆盖。但未测试 `setActiveEditor`/`clearActiveEditor` 与 `usePanelFocus` 的集成时序。
- **建议**：可接受当前覆盖，作为底层工具函数足够。

#### G3. `language-mapping.test.ts` 表驱动完整

- **位置**：`src/__tests__/language-mapping.test.ts`
- **评价**：扩展名映射覆盖较全，未发现明显问题。

## 覆盖分析

### 已覆盖较好的区域

- `getLanguageExtension` 扩展名映射。
- `computeAlignment` 纯函数主路径。
- `buildRangeSet`/`buildHeadRangeSet` 的 RangeSet 映射逻辑。
- `useCodeMirror` 正常打开/保存/fs-event 自动重载路径。
- `GitShowPanel` 三态渲染、oldPath 优先、readOnly 配置、卸载销毁。

### 明显缺口

| 源码区域 | 未覆盖内容 | 建议优先级 |
|---|---|---|
| `DiffPanel.tsx` | 保存后刷新链、占位刷新、`.git` 刷新、外部修改脏确认、滚动重新绑定、大文件分支 | P0 |
| `useCodeMirror.ts` | 大文件拒绝/警告、保存失败、另存为后事件、多编辑器 `justSavedRef` 隔离 | P1 |
| `gitGutter.ts` | 四个 wrapper dispatch 函数 | P1 |
| `alignment.ts` | `key >= 0` false 分支 | P2 |
| `GitShowPanel.tsx` | 大文件警告 header、参数切换时 view 销毁重建、字号变化 reconfigure | P1 |

## 变异测试分析

对核心代码做“关键行删除/交换”思想实验，判断当前测试能否发现：

| 变异目标 | 当前测试能否发现 | 说明 |
|---|---|---|
| `alignment.ts`：交换 `left.set` 与 `right.set` | 能 | `diff-alignment.test.ts` 对左右 Map 分别断言。 |
| `alignment.ts`：删除 `if (key >= 0)` 守卫 | **不能** | 无 `newStart=0` 用例。 |
| `gitGutter.ts`：`buildRangeSet` 中把 `ModifiedMarker` 换成 `AddedMarker` | 能 | 测试断言了 marker 的 DOM 颜色/尺寸差异。 |
| `gitGutter.ts`：`updateDiffGutter` 改发 `setHeadDiffMarkers` | **不能** | 未直接测试 wrapper。 |
| `useCodeMirror.ts`：删除 `justSavedRef.current.add(path)` | **不能** | 无保存后立刻触发 fs-event 的用例。 |
| `useCodeMirror.ts`：删除大文件拒绝分支 | **不能**（编辑器侧） | 编辑器测试未直接构造大文件。 |
| `DiffPanel.tsx`：删除 `refreshPlaceholders()` 调用 | **不能** | 无占位行数量/渲染断言。 |
| `DiffPanel.tsx`：保存后删除 `updateHeadDiffGutter(leftView, hunks)` | **不能** | 保存链用例有名无实。 |
| `DiffPanel.tsx`：删除 `.git` 变更刷新 effect | **不能** | 完全未覆盖。 |
| `DiffPanel.tsx`：删除 scroll listener 重新绑定 | **不能** | 仅验证初始绑定。 |
| `GitShowPanel.tsx`：删除大文件警告 header 拼接 | **不能** | 大文件用例只断言容器存在。 |

**结论**：约 30% 的关键变异无法被当前 L2 测试捕获，主要集中在 DiffPanel 保存链、占位刷新、外部修改/`.git` 刷新，以及 useCodeMirror 的大文件/保存失败路径。

## 稳定性风险评估

| 风险点 | 等级 | 说明 |
|---|---|---|
| 固定延时等待 | 中 | `diff-panel.test.tsx` 多处 `setTimeout 200ms` 依赖本地环境，CI 负载高时可能 flaky。 |
| mock 范围过大 | 中 | `gitshow-panel.test.tsx` 完全 mock CM6，无法验证真实 `reconfigure` 与 `destroy` 时序。 |
| 保存链用例名实不符 | 高 | 用例注释与实现不一致，容易造成“已覆盖”的误判。 |
| 覆盖缺口依赖 E2E | 高 | DiffPanel 多条路径只在 L4 验证，L2 快速反馈能力不足。 |

## 总结

本次评审 11 个测试文件、190 条用例，发现 **4 条红色、5 条黄色、3 条绿色** 问题。

**TOP 3 问题**：

1. **`diff-panel.test.tsx` 的保存链用例有名无实**（R1）。这是最严重的用例设计缺陷，注释声称覆盖保存后刷新，实际只验证了函数存在性。
2. **`DiffPanel.tsx` 行覆盖仅 63.9%**，保存链、占位刷新、`.git` 刷新、外部修改脏确认等关键路径缺失 L2 防护（R2）。
3. **`useCodeMirror.ts` 大文件处理与保存失败路径**缺乏直接回归用例（R3），而这是编辑器内存安全与数据完整性的核心路径。

**建议下一步**：优先补齐 DiffPanel 保存链与占位刷新用例、useCodeMirror 大文件/保存失败用例、gitGutter wrapper 直接测试，并修复 `diff-panel.test.tsx` 中名实不符的用例注释或实现。
