# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

CodeMirror 6 编辑器面板——文件查看/编辑（普通编辑器 + 大文件阈值）。本模块承载编辑器能力专属实现：`EditorPanel` 面板组件、`useCodeMirror` 生命周期 hook、`gitGutter` diff gutter 扩展、快捷键命令工厂（`keyboard.ts`）、聚焦编辑器指针（`activeEditor.ts`）。gitshow / diff / JsonMode 等复用本模块导出（语言扩展、大文件阈值、字体/主题扩展，见下）。

## 架构决策

### Compartment 模式切换语言

文件扩展名 → `getLanguageExtension()` 返回对应 CodeMirror 语言扩展。语言扩展通过 `Compartment.reconfigure()` 热切换，不丢失文档状态。

### Compartment 模式切换自动换行（Alt+Z）

`Alt+Z` 触发 `editor.toggleWordWrap` 命令，通过 `wordWrapRef`（`useRef<boolean>`）跟踪当前状态、`wrapCompartment`（`Compartment`）热切换 `EditorView.lineWrapping` 扩展，不丢失文档状态。默认关闭，每编辑器实例独立，不持久化（同 VS Code 行为）。handler 经 `EditorActions.toggleWordWrap()` → `getActiveEditor()` 派发到聚焦编辑器。

### 大文件不虚拟化（FE-31 决策登记，D3 关闭）

CodeMirror 6 不支持部分文档模型（虚拟化），大文件编辑**不虚拟化**——按 D3 方案以三层防线削峰（决策登记见 ADR 豁免表）：

- **Channel 分块削峰（BE-03）**：`fs_read_file` 改后端按 256KB 块经 `onChunk` Channel 推送（见 `src/ipc/CLAUDE.md` fs.ts 行），消除单次 IPC 大 payload 峰值——大文件读取不再一次载入内存
- **10MB 硬上限**：`MAX_FILE_SIZE_BYTES = 10_000_000`（`useCodeMirror.ts` 导出单点）——`sizeHint` 预检（UTF-8 文本 length 近似字节数）超限**直接拒绝**：doc 置错误提示 + 清 `filePathRef` 防误保存覆盖原文件
- **1MB 警告**：`LARGE_FILE_WARN_BYTES = 1_000_000`，超限 `confirmDialog` 弹窗警告（确认=继续/取消=中止，取消同样清 filePathRef）
- **阈值复用**：gitshow 面板经 `useCodeMirror` 导出复用同一阈值——禁止新造数值（既有约束）

### 滚动委托 CM .cm-scroller

旧方案外层 div `overflow: auto` 是实际滚动容器，`.cm-scroller` 无溢出（`.cm-editor` `height: auto`=内容高 → `.cm-scroller` `height: 100%`=内容高 → 无溢出 → 无滚动条）。横向滚动条在外层 div 底部，长内容时需垂直滚到底才能看到。

修复分两层：
- **容器** `overflow: clip`（非 `hidden`）：`hidden` 是 CSS 滚动容器→吸收鼠标滚轮事件不传递。`clip` 裁剪但不创建滚动容器→滚轮穿透到 `.cm-scroller`
- **`.cm-editor` 高度**：`EditorView.theme({ "&": { height: "100%" } })` 给予明确高度 → `.cm-scroller` `height: 100%` 约束为视口高度 → 内容溢出 → 滚动条出现。CM6 base theme 已设 `.cm-scroller { overflowX: auto }`，CSS 规范强制 `overflowY: auto`

### CM6 主题扩展与层叠

CM6 编辑器主题来源 = **`editorTheme`**（active 方案 `editor.theme`，linear 为 oneDark 透出）+ **`editorColorOverrides()`**（active 方案 `editor.overrides`，lint/searchMatch/background/正文行号覆盖）+ **`editorSyntaxHighlight()`**（active 方案 `editor.overrides.syntax`，9 组 tag 语法高亮——**消费点须置于 `editorTheme` 之前**，数组顺序决胜，ACC-05），经 `../../theme` barrel 引用，四处消费点：useCodeMirror / GitShowPanel / DiffPanel ×2 / JsonMode。**层叠规则与特异性守卫（ACC-05 实证）见 @../../theme/CLAUDE.md「editorColorOverrides 的 CM6 层叠」**——改动覆盖规则前必读：`@codemirror/view` `mountStyles()` reverse 注入使先声明主题恒胜，竞争选择器必须保持 `.cm-editor` 前缀形态。

### Ctrl+S 迁入 ShortcutRegistry

`editor.save`（Ctrl+S）不再走 CodeMirror keymap。命令在 `App.tsx` 一次性注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发到聚焦编辑器；`useCodeMirror` 经 `usePanelFocus("editor", container, activate, deactivate)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CM 内部 keymap（capture/bubble 分阶段共存）。`save` 动作用 `handleSaveRef` 保持最新引用（`handleSave` 依赖 panelId 会变）。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | EditorPanel 导出 |
| `EditorPanel.tsx` | 编辑器面板 React 组件：container `overflow: clip`（裁剪不吸收滚动事件，委托 `.cm-scroller` 管理滚动；`.cm-editor` `height: 100%` 约束 scroller 高度产生溢出）→ useCodeMirror |
| `keyboard.ts` | 编辑器快捷键命令工厂：`createEditorShortcuts()`（无参）经 `commandFromMeta` 生成 `editor.save`、`editor.toggleWordWrap`，App 一次性注册；handler 经 `getActiveEditor()` 派发 |
| `activeEditor.ts` | 模块级"聚焦编辑器"指针：`setActiveEditor`/`clearActiveEditor`（仅匹配时清）/`getActiveEditor` |
| `useCodeMirror.ts` | CodeMirror 6 生命周期 hook：创建 EditorView、`.cm-editor` `height: 100%` theme（约束 scroller 高度产生溢出→滚动条）、语言扩展、字体大小动态调节、自动换行 Compartment 热切换（Alt+Z）、Ctrl+Wheel 监听、Ctrl+S 保存（`usePanelFocus("editor")` + `setActiveEditor`，无路径则另存为）、Tab 缩进/Shift+Tab 反缩进（`keymap.of([indentWithTab])`）、Ctrl+F 搜索/撤销/重做仍归 CM keymap、外部文件改动监听、脏状态跟踪；**大文件阈值单点（FE-31/D3）：`MAX_FILE_SIZE_BYTES`（10MB 拒绝）/`LARGE_FILE_WARN_BYTES`（1MB 警告），供 gitshow 复用，不虚拟化** |
| `gitGutter.ts` | CodeMirror 6 gutter 扩展：DiffHunk → RangeSet<GutterMarker> 映射、setDiffMarkers StateEffect、diffMarkersField StateField、SpacerMarker 固定宽度防光标错位；新增 HEAD 侧 buildHeadRangeSet（old 行号映射）/ headDiffGutter / updateHeadDiffGutter / clearHeadDiffGutter |

## 测试模式

> 本节用例数为快照，最新计数以 `.claude/test-inventory.md` 为准。

测试文件位于 `src/__tests__/`，命名规则 `editor*.test.ts(x)`。

### 编辑器测试

| 文件 | 模式 |
|------|------|
| `use-code-mirror.test.ts`（39 用例） | `EditorState.create` 验证字体扩展；Compartment reconfigure 不重复 dispatch；handleSave（有/无 filePath、另存为、gitDiff 刷新、失败 alert、slterm:file-saved/file-saved-as 事件）；EDF-03 大文件拒绝/警告 |
| `editor.test.tsx`（9 用例） | EditorPanel 组件：mock `useCodeMirror` 返回 stub，验证 panelId/filePath 传递 + 容器 `overflow: clip` 样式。浅层组件定位（DOC-02⑥）——仅 prop 透传与容器样式契约，真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖 |
| `editor-confirm.test.ts`（11 用例） | `renderHook(useCodeMirror)` 真实驱动；mock `onFsEvent` 保留回调引用手动触发 fs-event；覆盖订阅/取消、kind 过滤、路径匹配、脏/净状态分支 |
| `editor-font.test.ts`（8 用例） | 字体 CSS 选择器断言（`.cm-scroller` vs `.cm-editor`） |
| `git-gutter.test.ts`（32 用例） | StateEffect → RangeSet 映射验证；GutterMarker DOM 颜色断言；SpacerMarker 宽度一致性 |
| `language-mapping.test.ts`（23 用例） | 扩展名→语言扩展全表验证（`.js`/`.ts`/`.py`/`.rs`/`.json` 等） |
| `use-code-mirror-reload-error.test.ts`（3 用例，FE-10） | 编辑器外部重载失败可感知（S08 新建）：reload 失败 console.warn + 状态条提示/成功路径无提示/卸载后结果忽略 |

### 键盘与快捷键

| 文件 | 模式 |
|------|------|
| `editor-keyboard.test.ts`（7 用例） | `createEditorShortcuts()` save 经 `getActiveEditor()` 派发；后设置的 active 覆盖先前的；无 active 返回 false |
| `active-editor.test.ts`（5 用例） | active 指针 set/get/覆盖、clear 仅匹配时生效（防竞态） |
