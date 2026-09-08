# Review 04 · 前端架构（CP-016/017/018/019/020/021/022/031/036/037/039/042）

## 问题清单

### 1. useFileTree 挂载即向注册表空提交，恢复完成前卸载则展开态丢失
- **CP**: CP-016
- **位置**: `src/features/explorer/useFileTree.ts:501-503`（commit effect）与 `:458-478`（loadRoot→restoreExpanded）
- **问题**: 每次挂载后首个 commit，`[rootNodes]` effect 恒运行一次，以空树派生 `expandedPaths: []` 覆盖 sideViewRegistry 状态槽。当前挂载自愈（restore 读的是 render 期 ref 快照 `:81-83`，早于 effect），但存在窗口：切换视图重建后，若在 `loadRoot` 完成前再次切走，注册表槽已被空集覆盖，快照随卸载丢弃——再切回时 `restoreExpanded` 读到空集，用户展开态丢失。根因 = 恢复窗口开启前不做提交抑制。
- **证据**: useFileTree.ts:501 `useEffect(() => { commitViewState(); }, [rootNodes])` 在挂载后首个 commit 即上呼 `{rootPath, expandedPaths: []}`；restoringRef 只在 `restoreExpanded()`（loadRoot resolve 后）才置 true，此前无任何提交抑制。explorer/CLAUDE.md 契约声称「槽位切换/换区重建后由回填恢复」，未覆盖该窗口。
- **建议**: 首次 loadRoot 完成（或 restore 窗口开启）前抑制 commitViewState 上呼。

### 2. panelRegistry 白名单 "settings" 裸字面量，与其余面板类型命名常量形态不一致
- **CP**: CP-017
- **位置**: `src/panelRegistry.ts:118`（及 `:86` PANEL_TYPES）
- **问题**: terminal/htmlviewer/markdownviewer 均有导出常量（PANEL_TERMINAL/PANEL_HTML_VIEWER/PANEL_MARKDOWN_VIEWER，:17-23），settings 以裸 `"settings"` 字面量两处出现。同文件双形态，后续重命名/引用无单点。
- **证据**: panelRegistry.ts:17-23 三常量；:86 `"settings",`；:118 `type === "settings"`。
- **建议**: 补 `PANEL_SETTINGS` 常量并替换两处字面量。

### 3. LargeFileViewer 行文本色取模块级 import 期快照，与 CP-039 响应式方向相悖
- **CP**: CP-022
- **位置**: `src/panels/editor/largeFileViewer/LargeFileViewer.tsx:27`
- **问题**: `LINE_TEXT_COLOR = schemeRegistry.getActive().editor.overrides.plainText` 在模块加载时求值一次，全生命周期冻结。editor/CLAUDE.md 的例外登记以 mdPreviewStyle「每次渲染现取」为先例，但实现并未照先例——是模块级常量。CP-039 刚消灭 editorTheme 模块级常量（为运行期方案切换铺路），此处重新引入同类冻结面；运行期切换路径落地后查看器颜色即掉队。
- **证据**: LargeFileViewer.tsx:27 模块级求值；对照 mdPreviewStyle.ts「每次渲染取 active 方案现拼」与 overrides.ts `getEditorTheme()` 函数形；editor/CLAUDE.md:35 例外登记自述「mdPreviewStyle 先例」。
- **建议**: 行文本色改渲染期经 `schemeRegistry.getActive()` 现取（或登记为已知滞后项）。

### 4. largeFile.sizeBytes 用 UTF-16 文本长度近似字节数，信息条文件大小对非 ASCII 文件系统性偏小
- **CP**: CP-022
- **位置**: `src/panels/editor/useCodeMirror.ts:358-364`；`src/panels/editor/largeFileViewer/LargeFileViewer.tsx:152`
- **问题**: `sizeHint = doc.length`（JS UTF-16 code units）作为 `sizeBytes` 上送，信息条以 `formatSize(fileSizeBytes)` 展示「只读浏览（X MB）」。CJK 文件 3 字节/字符，12MB UTF-8 文件 length≈4M → 展示「4.0MB」。useLineIndex 侧已显式 void 该值（:89），但信息条展示仍消费它。gitshow/diff 同样传 `headContent.length`（DiffPanel.tsx:732,751）。
- **证据**: useCodeMirror.ts:358 `const sizeHint = doc.length`、:364 `setLargeFile({ filePath, sizeBytes: sizeHint })`；LargeFileViewer.tsx:152 `只读浏览（{formatSize(fileSizeBytes)}）`。
- **建议**: 展示用大小改经 `new TextEncoder().encode(doc).length` 或后端 stat 值。

### 5. blockCache 无文件变更失效，浏览期间外部修改产生新旧块混合视图
- **CP**: CP-022
- **位置**: `src/panels/editor/largeFileViewer/blockCache.ts:27`；`src/panels/editor/largeFileViewer/LargeFileViewer.tsx`（全文无 fs-event 订阅）
- **问题**: 块缓存按 `{filePath, blockIndex}` 键入，无 mtime/大小校验、无 fs-event 失效、无重载路径。文件在浏览中被外部修改（编辑器打开期间正是 watcher 活跃场景）后：已缓存块保持旧内容，新触发补载的块读到新内容——同一文档新旧块拼接，且无任何提示。LRU 逐出后补载反而加剧不一致面。
- **证据**: blockCache.ts 全文无失效机制；LargeFileViewer/useLineIndex 无 `onFsEvent` import（grep 零命中）；对比 useCodeMirror 有外部修改重载链路（useCodeMirror.ts:549 重载失败 toast 登记）。
- **建议**: 打开时记录文件大小/mtime，块读取响应与记录不符即整表失效重扫（或登记为显式接受的近似口径）。

### 6. 注入产物「不得含 `</script>`」拼接纪律无测试锁
- **CP**: CP-031
- **位置**: `src/__tests__/doc-viewer-injection.test.ts:24-28`；`src/panels/docViewer/buildInjectedScript.ts:14-16`
- **问题**: 拼接纪律 #1（注入段自身不得输出 `</script>` 字面量——宿主内容不转义后此为唯一防线）无显式断言。parse-only 用非贪婪正则 `<script>([\s\S]*?)<\/script>` 取体，若某段未来误含提前闭合标签，捕获截断到第一段即仍通过 parse——纪律失守静默。
- **证据**: doc-viewer-injection.test.ts:25 非贪婪正则；全文件无 `toHaveLength(1)`/not.toContain("</script>") 类断言（grep 确认）。
- **建议**: 补一条 `expect(out.match(/<\/script>/gi)).toHaveLength(1)` 守卫。

## 界外观察

- `useCodeMirror` 的 10MB 检查发生在 `doc = await fs.readFile(filePath)` 全量读入之后（useCodeMirror.ts:354-359）——目标文件 10MB 与否都先全量进内存再拒绝/引导。CP-022 未触及该路径（其读取改走 fs_read_file_range 的是 LargeFileViewer），但可编辑域上限的「内存保护」语义实际只保护 CM 建实例一步，不保护读盘一步。属存量形态，非本次修复引入。
