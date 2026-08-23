# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

编辑器面板封装 CodeMirror 6 生命周期与普通文件编辑能力（打开/保存/外部改动检测/diff gutter/语言切换/自动换行/字体缩放）。`gitshow` / `diff` / `JsonMode` 等场景复用本模块导出的语言扩展、大文件阈值与主题扩展，避免各自维护一份 CM6 配置。

## 关键约束与决策

### Compartment 热切换语言与自动换行

文件扩展名 → `getLanguageExtension()` 返回对应 CM6 语言扩展。语言扩展通过 `Compartment.reconfigure()` 热切换，不丢失文档状态。`Alt+Z` 触发 `editor.toggleWordWrap`，通过 `wrapCompartment` 热切换 `EditorView.lineWrapping`，同样不丢失文档状态。默认关闭，每实例独立，不持久化（同 VS Code 行为）。

### 大文件不虚拟化（FE-31 登记，D3 关闭）

CodeMirror 6 不支持部分文档模型，大文件编辑**不虚拟化**——按 D3 方案以三层防线削峰：

- **Channel 分块削峰（BE-03）**：`fs_read_file` 后端按 256KB 块经 `onChunk` Channel 推送，消除单次 IPC 大 payload 峰值。
- **10MB 硬上限**：`MAX_FILE_SIZE_BYTES = 10_000_000`（`useCodeMirror.ts` 导出单点）——`sizeHint` 预检超限直接拒绝，doc 置错误提示并清 `filePathRef` 防误保存覆盖原文件。
- **1MB 警告**：`LARGE_FILE_WARN_BYTES = 1_000_000`，超限 `confirmDialog` 弹窗警告（确认=继续/取消=中止），取消同样清 `filePathRef`。

阈值由 `useCodeMirror` 导出单点，`gitshow` 等面板禁止新造数值。

### 滚动委托 `.cm-scroller`

旧方案外层 div `overflow: auto` 是实际滚动容器，`.cm-scroller` 无溢出，横向滚动条沉底。修复：

- 容器设 `overflow: clip`（非 `hidden`）：`hidden` 会创建滚动容器并吸收鼠标滚轮事件；`clip` 裁剪但不创建滚动容器，滚轮穿透到 `.cm-scroller`。
- `.cm-editor` 高度 `100%`：使 `.cm-scroller` `height: 100%` 约束为视口高度 → 内容溢出 → 滚动条出现在 `.cm-scroller`。

### CM6 主题扩展与层叠（ACC-05）

CM6 编辑器主题来源 = `editorTheme`（active 方案 `editor.theme`）+ `editorColorOverrides()`（active 方案 `editor.overrides`）+ `editorSyntaxHighlight()`（active 方案 `editor.overrides.syntax`）。消费点须注意顺序：

- `@codemirror/view` 的 `mountStyles()` 会把 styleModule facet 数组 **reverse()** 后挂载。
- 扩展数组 `[editorTheme, editorColorOverrides(), editorSyntaxHighlight()]` 编译后 oneDark 规则排在 overrides 之后——同特异性下后声明者胜，导致覆盖全失效。
- 解决方案：覆盖选择器带 `&.cm-editor` 前缀提升特异性，使胜负与数组顺序无关。

改动覆盖规则前必读 `@../../theme/CLAUDE.md`「editorColorOverrides 的 CM6 层叠」。

### Ctrl+S 迁入 ShortcutRegistry

`editor.save`（Ctrl+S）不再走 CodeMirror keymap。命令在 `App.tsx` 一次性注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发到聚焦编辑器；`useCodeMirror` 经 `usePanelFocus("editor", container, activate, deactivate)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CM 内部 keymap。`save` 动作用 `handleSaveRef` 保持最新引用。

## 外部坑/红线

- **CM6 `mountStyles()` reverse 注入**：扩展数组顺序会被反转，覆盖规则必须靠特异性（`&.cm-editor` 前缀）取胜，不能依赖声明顺序。
- **容器 `overflow` 选择**：编辑器面板容器必须用 `overflow: clip`。`hidden` 会吸收滚轮事件；`auto`/`scroll` 会把外层 div 变成滚动容器，导致横向滚动条沉底。
- **Compartment 不可跨 view 共享**：每个 `Compartment` 绑定到特定 `EditorState`，`diff` 左右栏必须各自独立创建 font/wrap Compartment。
- **保存后抑制 fs-event**：`justSavedRef` 用 `Set<string>` 按路径去重，避免把自己的写入误判为外部改动、执行全量文档替换从而清空 diff gutter 标记。
- **大文件拒绝必须清 `filePathRef`**：超限拒绝或警告取消后若不清理 filePathRef，后续保存会覆盖原文件。

## 测试模式

- 测试文件位于 `src/__tests__/`，命名规则 `editor*.test.ts(x)`。
- `editor.test.tsx` 为浅层组件定位：仅验证 prop 透传与容器 `overflow: clip` 样式契约，真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖。
- `use-code-mirror.test.ts` 覆盖字体扩展、Compartment reconfigure、保存链路、大文件拒绝/警告。
- `git-gutter.test.ts` 覆盖 StateEffect → RangeSet 映射、GutterMarker DOM、SpacerMarker 宽度一致性。
- `language-mapping.test.ts` 覆盖扩展名→语言扩展全表。
- `editor-keyboard.test.ts` / `active-editor.test.ts` 覆盖命令派发与聚焦指针竞态。
