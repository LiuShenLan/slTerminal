# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

编辑器面板封装 CodeMirror 6 生命周期与普通文件编辑能力（打开/保存/外部改动检测/diff gutter/语言切换/自动换行/字体缩放）。`gitshow` / `diff` / `JsonMode` 等场景复用本模块导出的语言扩展、大文件阈值与主题扩展，避免各自维护一份 CM6 配置。

## 关键约束与决策

### Compartment 热切换语言与自动换行

文件扩展名 → `getLanguageExtension()` 返回对应 CM6 语言扩展。语言扩展通过 `Compartment.reconfigure()` 热切换，不丢失文档状态。`Alt+Z` 触发 `editor.toggleWordWrap`，通过 `wrapCompartment` 热切换 `EditorView.lineWrapping`，同样不丢失文档状态。默认关闭，每实例独立，不持久化（同 VS Code 行为）。

### docViewer 面板选项（S3 扩展，EditorPanel 默认行为零变化）

`UseCodeMirrorOptions` 新增三项，供 htmlviewer/markdownviewer 的编辑形态使用（消费点见 docViewer 家族文档）：

- **`initialDoc`**：有值（含空串）则跳过磁盘读取直接以快照建缓冲（草稿回填免二次 IPC）。只在 effect 执行瞬间消费（经 initialDocRef）——**不入 effect deps**：重建由 container 变化驱动（形态切换卸载/重挂），面板读盘完成回填 doc 若入 deps 会触发无谓重建（闪烁+光标重置）。
- **`onDocContent(text, source)`**：三源回传（init = 缓冲建立 / edit = 每次 docChanged / reload = 外部修改重载成功），经 ref 转发防闭包过期；**仅在传入时挂载**——EditorPanel 不传则 updateListener 零额外开销（无 toString 成本）。
- **`gitGutterEnabled`**：默认 true（EditorPanel/DiffPanel 现状）；docViewer 预览面板传 false——不加载 diff gutter 扩展、不读 git、保存后不刷新 gutter。

**容器 ref 时序红线**：hook 消费方在 render 阶段读 ref 得 null（commit 后赋值）——edit 容器首挂需 commit 后 bump 重渲染（HtmlPanel/MarkdownPanel bumpFrame 桥接，DiffPanel 先例），否则 view 永不创建。

### 大文件四层防线（FE-31 登记 + CP-022 第四层，D3 关闭）

CodeMirror 6 不支持部分文档模型，大文件编辑**不虚拟化**——此事实仍属实。防线分两层域：

**可编辑域（≤10MB）三层防线削峰（D3 方案，语义不变）**：

- **Channel 分块削峰（BE-03）**：`fs_read_file` 后端按 256KB 块经 `onChunk` Channel 推送，消除单次 IPC 大 payload 峰值。
- **10MB 可编辑上限**：`MAX_FILE_SIZE_BYTES = 10_000_000`（`useCodeMirror.ts` 导出单点）——读盘前 `fs_stat` 预检（FE-08）超限零读盘不再可编辑；读后 `doc.length > MAX` 复核保留为 TOCTOU 防线（stat 与读盘间文件长大场景）。
- **1MB 警告**：`LARGE_FILE_WARN_BYTES = 1_000_000`，超限 `confirmDialog` 弹窗警告（确认=继续/取消=中止），取消清 `filePathRef`；文案用 stat 真实字节数（FE-08，原 UTF-16 码元近似对 CJK 文件系统性偏小）。

**>10MB 只读分片浏览（CP-022 第四层，超限引导语义）**：`fs_stat` 预检 `> MAX_FILE_SIZE_BYTES` 不再以拒绝文案替换全文——`useCodeMirror` 返回 `largeFile: { filePath } | null` 信号（文件切换渲染期自动清空；FE-04 起仅 filePath，真实大小由查看器挂载 `fs_stat` 自取），宿主面板检测后切换 `largeFileViewer/LargeFileViewer`（同面板内形态切换，不经 panelRegistry）：

- 固定行高虚拟化行窗口（`LARGE_FILE_LINE_HEIGHT=20`，overscan 上下各 20 行，参照 FileTree 手实现先例）；行内经 `useLineIndex` 按需扩展索引、`blockCache`（LRU 32 块上限）缓存块文本，块经新命令 `fs_read_file_range` 按 256KB 区间读取（后端无 10MB 上限、头尾对齐字符边界——逐块拼接即原文，契约见 src-tauri/src/fs/mod.rs 命令注释）。
- `filePathRef.current = undefined` 清空保留（防误保存覆盖原文件——大文件形态无 CM 编辑实例，Ctrl+S 无保存目标）。
- 行文本前景色 = active 方案 `editor.overrides.plainText`，渲染期 state + `schemeRegistry.onDidChange` 订阅响应式取色（FE-03，editorThemeSlot 先例；colors.ts facade 无编辑器正文 token——**硬约束 #6 新增例外登记，随本组件生效**）；字体复用 `EDITOR_FONT_SPEC`。
- 外部修改失效：fs-event Modify + `fs_stat` 比对（mtime/size 变化 → 缓存整表失效 + 行索引复位重扫），静默重载（只读语义）；事件丢失残余窗口为已知边界（同 editor 域 fs-event 依赖）。

超限引导消费点：EditorPanel（editor 形态）、GitShowPanel（sourceLabel="git show"）、DiffPanel（任一侧超限该侧引导、双侧超限分栏各自）——1MB-10MB 警告 header 语义在 gitshow/diff 不变。阈值仍由 `useCodeMirror` 导出单点，各面板禁止新造数值。查看器信息条大小 = `fs_stat` 真实字节（FE-04；stat 失败降级 `…`）；gitshow/diff 的 10MB 判定保留 `text.length` 近似（HEAD blob 无磁盘 stat 通道——FE-04 收窄登记）。

### 滚动委托 `.cm-scroller`

旧方案外层 div `overflow: auto` 是实际滚动容器，`.cm-scroller` 无溢出，横向滚动条沉底。修复：

- 容器设 `overflow: clip`（非 `hidden`）：`hidden` 会创建滚动容器并吸收鼠标滚轮事件；`clip` 裁剪但不创建滚动容器，滚轮穿透到 `.cm-scroller`。
- `.cm-editor` 高度 `100%`：使 `.cm-scroller` `height: 100%` 约束为视口高度 → 内容溢出 → 滚动条出现在 `.cm-scroller`。

### CM6 主题扩展与层叠（ACC-05）

CM6 编辑器主题来源 = `editorThemeSlot` 主题热切换槽（CP-039）：`createEditorThemeSlot()` 一次、每 EditorView 一槽，槽内 `editorThemeBundle()` 单点固化 `[editorSyntaxHighlight(), getEditorTheme(), editorColorOverrides()]` 三项（均随 active 方案响应式取色）。**一个 EditorView 一个槽**（Compartment 不可跨 view 共享——diff 双栏各自建槽）；view 创建后 `slot.bind(view)` 订阅 `schemeRegistry.onDidChange`，方案切换即 Compartment 重配置（编辑器不重建），卸载 cleanup 先调 bind 返回的取消函数再 `view.destroy()`。层叠要点：

- `@codemirror/view` 的 `mountStyles()` 会把 styleModule facet 数组 **reverse()** 后挂载。
- 槽内顺序契约：syntax 必须先于 theme（`[editorSyntaxHighlight(), getEditorTheme(), editorColorOverrides()]`）——`editorSyntaxHighlight` 与 oneDark 的 HighlightStyle 是同机制竞争，只能靠数组顺序决胜；`editorColorOverrides` 则靠 `&.cm-editor` 前缀提升特异性，顺序无关。
- 消费点扩展数组只出现 `themeSlotRef.current.extension` 单槽项——禁止绕过槽裸拼三项（ACC-05 顺序由槽单点锁死）。

改动覆盖规则前必读 `@../../theme/CLAUDE.md`「editorColorOverrides 的 CM6 层叠」。

### Ctrl+S 迁入 ShortcutRegistry

`editor.save`（Ctrl+S）不再走 CodeMirror keymap。命令在 `App.tsx` 一次性注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发到聚焦编辑器；`useCodeMirror` 经 `usePanelFocus("editor", container, activate, deactivate)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CM 内部 keymap。`save` 动作用 `handleSaveRef` 保持最新引用。

## 外部坑/红线

- **WebView2/Chromium 陈旧光栅（GLYPH bug，2026-09-06 取证）**：CM6 文本行跨帧多次独立输入后部分字形「存在但未绘制」（DOM/几何/字色完整，仅像素缺失；选中强制重绘恢复）。取证：GPU 合成/光栅/全软渲染关闭均复现、CSS 层提升与 containment 无效、@codemirror/view 6.43.11 仍复现、面板结构无关（txt 面板同丢）→ 引擎 paint 缓存陈旧缺陷，无上游修复。规避 = `repaintGuard()` 扩展（repaintGuard.ts，全部 CM6 宿主 useCodeMirror/JsonMode/DiffPanel/gitshow 注入）：docChanged 后对光标行 display 往返强制整行重绘（同宏任务，不触碰 text node/selection/装饰结构）。像素防复发锚 = e2e-tests/glyph-repro.e2e.ts（GLYPH_E2E=1，真实 GPU 环境截图字形位判读）。**撤销条件：WebView2/Chromium 引擎修复后复核移除**（测试红线：repaint-guard.test.ts 注入契约用例会红）。
- **CM6 `mountStyles()` reverse 注入**：扩展数组顺序会被反转，覆盖规则必须靠特异性（`&.cm-editor` 前缀）取胜，不能依赖声明顺序。
- **容器 `overflow` 选择**：编辑器面板容器必须用 `overflow: clip`。`hidden` 会吸收滚轮事件；`auto`/`scroll` 会把外层 div 变成滚动容器，导致横向滚动条沉底。
- **Compartment 不可跨 view 共享**：每个 `Compartment` 绑定到特定 `EditorState`，`diff` 左右栏必须各自独立创建 font/wrap Compartment。
- **保存后抑制 fs-event**：`justSavedRef` 用 `Set<string>` 按路径去重，避免把自己的写入误判为外部改动、执行全量文档替换从而清空 diff gutter 标记。
- **大文件超限必须清 `filePathRef`**：>10MB 引导只读浏览（largeFile 信号）或 1MB 警告取消后若不清理 filePathRef，后续保存会覆盖原文件。

## 测试模式

- **浅层组件定位**：仅验证 prop 透传与容器 `overflow: clip` 样式契约，真实编辑器行为由 hook 层测试覆盖。
- **编辑器核心**：字体扩展、Compartment reconfigure、保存链路、大文件拒绝/警告。
- **git gutter**：StateEffect → RangeSet 映射、GutterMarker DOM、SpacerMarker 宽度一致性。
- **语言映射**：扩展名→语言扩展全表。
- **命令派发**：覆盖命令派发与聚焦指针竞态。
