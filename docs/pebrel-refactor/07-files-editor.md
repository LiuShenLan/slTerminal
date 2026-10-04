# pebrel 重构优化 spec · 分片 07：文件与编辑

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

pebrel 侧文件与编辑体系 = 三块：

1. **内置编辑器** `nebula_app/src/gpui_shell/file_editor/`（`TextFileView` 双模态文本编辑器，源码/渲染两态，markdown 默认渲染态打开、显式切源码编辑）：`live_edit`/`inline_edit`/`inline_selection`/`live_commands`/`live_navigation`/`live_selection` 的所见即所得块编辑内核（源位置投影、格式标记留源码、IME 稳定），`edit_history` 跨表示统一撤销历史，`outline`/`outline_view`/`details`/`info` 大纲导航与详情面板，`preview`/`activity` 虚拟化块渲染与非活跃页资产释放，`block_structure`/`block_inline`/`structure_view`/`structure_inline_view`/`structure_commands` 结构化块（表格/列表/代码/数学）编辑，`input_rules` 提交式 Markdown 输入规则，`code_actions` 代码块语言选择与复制，`images`/`image_cache` 文档图片压帧与文档级缓存，`document`/`document_io` 快照与有界读取，`chrome`/`reader_presentation` 工具条与几何常量族；`source.rs` 的 Local/Remote 双适配中 Remote 随 SSH 砍。
2. **tab 容器与打开路由** `gpui_shell/code_tab.rs`（普通代码文件走可保存文本编辑器，Git 冲突走三栏合并器）+ `gpui_shell/doc_tabs.rs`（图片 tab + 共享编辑入口 + `openable_in_app` 双击路由）。
3. **文件浏览器** `gpui_shell/workspace/file_tree.rs` + `file_tree/path_bar.rs`（侧栏文件树渲染与路径栏导航），行模型在 `display/side_panel/`（`enumerate` 目录枚举与过滤索引、`gitignore`、`vcs` git 状态、`search`），文件操作在 `display/file_operations.rs`（回收站）、`display/file_dialog.rs`（系统文件对话框），文件树→终端拖放在 `gpui_shell/file_drop.rs`。

slTerminal 侧现状 = WebView 技术栈的文件/编辑全家：`src/panels/editor/`（CodeMirror 6 编辑器面板：语言 Compartment 热切换、git gutter、外部修改三模式同步、`repaintGuard` 字形防复发、大文件四层防线 + `largeFileViewer` 分块只读浏览）；预览类面板 `src/panels/docViewer/`（iframe 沙箱宿主页 + 消息桥 + 注入脚本 + 缩放/滚动运行时，承载 `htmlviewer`/`markdownviewer`）、`src/panels/gitshow/`（HEAD 只读）、`src/panels/diff/`（HEAD↔工作区双栏）；`src/features/explorer/`（虚拟化文件树 + CRUD + git 着色 + fs-event 增量刷新，`FileTreeExplorer` 共享树组件）、`src/features/navTree/`（项目→页面→会话导航）、`src/features/fileViewers/`（扩展名→面板策略注册表）、`src/features/agentFiles/`；后端 `src-tauri/src/fs/`（沙箱内读写、keyset 分页目录、256KB 分块读取、CRLF 检测、`fs_read_file_range`）、`src-tauri/src/git/`、`src-tauri/src/preview.rs`（预览自定义协议域）。键位侧：slTerminal `src/features/shortcuts/`（ShortcutRegistry + 命令目录 + 保留键）对照 pebrel `nebula_settings` 键位表 + `workspace/keyboard_bindings.rs` + `display/keymap.rs`。

裁定背景：全局已定「内置编辑器 + 文件树照抄」，slTerminal 自有面板并入 GPUI 面板体系；两进程 IPC 沙箱文件层随单进程消亡；CM6 与 iframe 沙箱两大 WebView 技术栈无 GPUI 对应物，其承载功能由 pebrel 原生形态替代或重定。本节功能点粒度裁定。

## 采纳点

### 内置编辑器内核（file_editor 照抄面）

1. **照抄：`TextFileView` 双模态编辑器总体形态**（`gpui_shell/file_editor/mod.rs` 的 `TextFileView` / `new_with_source` / `save` / `request_reload` / `toggle_preview` / `tab_title`）。markdown 默认渲染态打开、编辑显式切源码（`ToggleSource`）、dirty/saving/loading 三态合同、tab 标题脏标记「•」、重开同路径保留草稿与撤销历史、外部修改 reload 脏确认弹窗——编辑器面板产品语义全套照抄，`nebula_*` → `slterm_*`。
2. **照抄：`Document` 快照与条件原子保存**（`file_editor/document.rs` 的 `Document` / `Document::load` / `Document::save`、`SaveError` 的 `Changed`/`ReadOnly`/`Io`）。保存前快照比对（磁盘已变 = `Changed` 拒绝覆盖）、只读文件禁写、BOM/CRLF/invalid_encoding/truncated/read_only 元数据全保留、写盘 I/O 后台执行——slTerminal 保存链路的严格超集。
3. **照抄：`read_prefix` 有界前缀读取**（`nebula_app/src/document_io.rs` 的 `read_prefix`）。内存上限先于解码/解析生效、超限截断标志 + UTF-8 边界截断——「大文件读多少」的合同单点。
4. **照抄：`EditHistory` 跨表示统一撤销历史**（`file_editor/edit_history.rs` 的 `EditHistory` / `record` / `record_rope` / `changed_rope_span`）。源码态与渲染态共享一份历史（输入实体寿命不决定可撤销性）、8MB 上限、rope 差量提取不物化全文。
5. **照抄：`Outline` 块大纲四索引同源**（`file_editor/outline.rs` 的 `Outline` / `Outline::prepare` / `edit_block_at` / `visible_headings`）。mdast 解析出 headings/blocks/source_ranges/structures 同一份 AST、512KB 预览上限 + 32KB 块上限、源码导航与预览块跳转同源——「大纲即渲染块清单」的模型照抄。
6. **照抄：虚拟化块渲染 + 非活跃页资产释放**（`file_editor/preview.rs` 的 `render_markdown_preview`、`file_editor/activity.rs` 的 `set_render_active`、`file_editor/mod.rs` 的 `apply_outline` / `schedule_preview`）。预览块经 `ListState` 虚拟化（屏外块不物化）、250ms 防抖重排大纲、切走 tab 即清内联视图/图片激活/滚动状态但草稿与历史存活——slTerminal `renderer="always"` 恒挂载白名单问题的原生世界答案。
7. **照抄：`LiveEdit`/`Projection` 所见即所得块编辑内核**（`file_editor/live_edit.rs` 的 `LiveEdit` / `decorations`、`file_editor/inline_edit.rs` 的 `Projection` / `Marks` / `changed_span`）。聚焦块用稳定原生输入、内联格式从源位置投影、格式标记留源码、未改叶子与链接目标逐字节不动、IME 组合期不重排输入实体——本片最核心的照抄资产。
8. **照抄：跨输入边界的键盘导航与选择同步**（`file_editor/live_navigation.rs` 的 `move_live_edge`、`file_editor/live_selection.rs` 的 `sync_live_selection`、`file_editor/inline_selection.rs` 的 `inline_selected_text`）。方向键跨越原生输入生命周期不离开格式化文档、行内源码可见性跟随光标（不改文档/不加撤销项）、相邻内联视图共享段落复制——WYSIWYG 交互完成度靠这三件。
9. **照抄：`BlockStructure` 源支撑可编辑部件模型**（`file_editor/block_structure.rs` 的 `StructureNode` / `EditPart` / `PartKind`、`file_editor/block_inline.rs` 的 `InlineRun` / `contains_object` / `build`）。容器拥有布局、输入只改内容 span（管道/列表符/围栏永不进输入）、嵌入对象切分内联 run、病理块回退字面量 + 单输入有界。
10. **照抄：结构化块渲染与结构化编辑命令**（`file_editor/structure_view.rs` 的 `render_structured_block`、`file_editor/structure_inline_view.rs` 的 `render_inline_parts`、`file_editor/structure_commands.rs` 的 `commit_structure_edit` / `table_cell_source`）。表格/列表/引用保持挂载于虚拟列表、仅聚焦单元格拥有原生输入、结构化命令走文档事务共享撤销/保存——「结构编辑 = 普通编辑」的统一事务语义。
11. **照抄：`input_rules` 提交式 Markdown 输入规则**（`file_editor/input_rules.rs` 的 `block_prefix` / `enter_block`）。已提交的打字与粘贴可引入结构（`- `→列表、`# `→标题、围栏、分割线、`$$`→数学块入口）、IME 组合更新不触发——打字即排版。数学块入口后的公式编译/栅格化管线归分片 08（见交界登记）。
12. **照抄：`code_actions` 代码块语言选择与复制**（`file_editor/code_actions.rs` 的 `CodeLanguage` / `CodeSpec`）。可见块级语言选择器 + 代码复制动作 + 复制反馈。
13. **照抄：文档图片压帧与文档级缓存**（`file_editor/images.rs` 的 `rewrite_doc_images` / `flatten_local_image_url`、`file_editor/image_cache.rs` 的 `DocumentImageCache`）。本地 GIF/动图 WebP 压单帧 PNG 进缓存（网络图走 HTTP 压帧；不压则 gpui 播 GIF 且多图共用 element id 越界 panic）、文档自有解码图 LRU 64MB/128 项、逐出同时释放像素缓冲与图集条目、非活跃文档 `set_active(false)`——「虚拟行不得污染进程级常驻资产图」的缓存纪律照抄。
14. **参考：`reader_presentation` 几何常量族单点**（`file_editor/reader_presentation.rs` 的 `PAGE_WIDTH` / `OUTLINE_WIDTH` / `DETAILS_MIN_WIDTH` / `clamp_details_width` / `heading_size`）。阅读器全部几何常数单文件集中、消费方零字面量；具体数值实现期按 slTerminal 视觉重定。
15. **参考：工具条/详情侧栏/文件信息面**（`file_editor/chrome.rs` 的 `render_toolbar`、`file_editor/details.rs` 的 `DocumentDetails` / `DocumentSection`、`file_editor/info.rs` 的 `render_info_content`）。面包屑工具条、大纲/信息双节侧栏（工作区托管共享右侧栏的形态）、详情宽度拖拽钳制、字符/行数/编码/行尾/修改时间统计——产品语义照抄，几何细节参考。

### tab 容器与打开路由

16. **照抄：`CodeTabView` 普通代码文件 tab + 三栏合并器**（`gpui_shell/code_tab.rs` 的 `CodeTabView` / `new_git_merge` / `MergeEditor` / `matches_git_merge`）。普通文件 = 可保存文本编辑器（与 markdown tab 共享编辑内核）；Git 冲突 = 三栏（左当前/中可编辑结果/右传入，外侧只读），应用结果写回工作树 + `git add`，不另开浮窗——slTerminal `diff` 面板与冲突解决面的归宿。
17. **照抄：扩展名→tree-sitter 语言映射表与可打开判定**（`gpui_shell/code_tab.rs` 的 `language_for_extension` / `viewable_file`、`display/image_viewer.rs` 的 `viewable_file`）。全量映射表单点 + 图片格式判定；映射值实现期与 slTerminal 既有语言值集对账合并。
18. **照抄：`ImageTabView` 图片 tab 与 `ImageView` 几何状态机**（`gpui_shell/doc_tabs.rs` 的 `ImageTabView` / `spawn_decode` / `reload`、`display/image_viewer.rs` 的 `ImageView` / `zoom_by` / `begin_drag`）。缩放 0.25×–8× 步进 1.18、锚点缩放、拖拽平移、后台 BGRA 解码、重开同路径 reload——图片查看产品语义照抄。
19. **照抄：`openable_in_app` 双击路由**（`gpui_shell/doc_tabs.rs` 的 `openable_in_app`）。应用内可读（图片/文档/源码）开 tab，其余交系统处理器——slTerminal `fileViewers` 注册表的产品功能等价物（注册表机制不采纳）。
20. **参考：画布几何事件换算**（`gpui_shell/doc_tabs.rs` 的 `area_tuple` / `on_scroll` / `on_mouse_down`）。绘制不依赖上一帧 bounds、事件换算用当帧 area——事件/绘制解耦的健壮性参考。

### 文件浏览器

21. **照抄：文件树模型/渲染分层 + 平铺行模型**（`display/side_panel/mod.rs` 的 `file_rows` / `PanelView`、`gpui_shell/workspace/file_tree.rs` 的 `render_file_tree_row` / `on_file_tree_search_event`、`display/side_panel/enumerate.rs` 目录枚举与过滤索引、`gitignore.rs`、`vcs.rs`、`search.rs`）。模型（枚举/gitignore 过滤索引/git 状态/文件名索引随 fs 通知存活）与渲染完全分层、行模型平铺化后渲染只按行号取——slTerminal `useFileTree` 全部前端状态的 Rust 模型化落点。
22. **参考：文件树行几何水洗形态**（`gpui_shell/workspace/file_tree.rs` 的 `ROW_PITCH` / `ROW_WASH_H` / `ROW_WASH_INSET` / `DRAWER_TEXT_INSET`）。行距内画矮一截水洗、行缝来自水洗矮于行距而非 flex gap——连续列表读感细节参考，数值实现期重定。
23. **照抄：右键菜单宿主提升到 workspace 根**（`gpui_shell/workspace/file_tree.rs` 的 `FileTreeContextMenu`）。菜单挂 workspace 根不受抽屉裁剪与推出动画影响。
24. **照抄：`PathEditor` 路径栏导航**（`gpui_shell/workspace/file_tree/path_bar.rs` 的 `PathEditor` / `resolve_directory` / `display_path`）。路径框只拥有草稿、导航经文件树模型单入口、`~` 展开、UNC `\\?\` 前缀规范化显示——slTerminal 无路径栏，新增能力照抄；WSL 来宾路径解析分支随多平台/SSH 砍。
25. **照抄：`send_to_recycle_bin` 回收站删除**（`display/file_operations.rs` 的 `send_to_recycle_bin`）。Windows `SHFileOperationW` + `FOF_ALLOWUNDO` 可撤销删除——删除语义对齐系统回收站。
26. **参考：`display/file_dialog.rs` 系统文件对话框**（`display/file_dialog.rs` 的 `FileFilter` 过滤器族）。Windows 原生打开/保存对话框 + 过滤器表单点；slTerminal 无此能力，新增参考；SSH 私钥过滤器随 SSH 砍。
27. **照抄：`FileTreeDrag` 文件树→终端拖放契约**（`gpui_shell/file_drop.rs` 的 `FileTreeDrag` / `FileDragGhost`）。载荷纯数据（宿主路径 + 写入 PTY 的路径原文 + 名称）、拖动预览独立实体（源行可能已因树刷新消失）——slTerminal 无文件拖放，新增照抄。
28. **参考：`text_preview` 显示用文本边界**（`nebula_app/src/text_preview.rs` 的 `multiline_source` / `single_line_label`）。单行表面（IME 气泡/链接提示）不得把换行喂给字体系统单行 API、多行预览 32 行/4KB 双上限——文本预览有界化细节参考。

### 键位绑定

29. **照抄：`keybind_pairs` 键位表持久化与整表替换语义**（`nebula_settings/src/lib.rs` 的 `keybind_pairs` / `keybind_pairs_from_text` / `persist_keybinds` / `apply_keybinds`）。`combo:Action` 对表、行序即优先级、整表替换（删旧行按序追加）其余内容原样保留——分片 06 已定 JSON 化，语义照抄、载体替换（JSON 键位段）。
30. **照抄：用户覆盖注入与恢复默认的精确收回**（`gpui_shell/workspace/keyboard_bindings.rs` 的 `STATIC_DEFAULT_COMBOS` / `gpui_binding_combo`）。静态默认绑定镜像表（区分「恢复默认」与「Unbind 精确收回」）、存储格式 combo→GPUI 绑定串规范化（大小写/修饰键序/digitN/plus-minus 转义）——combo 格式不变，解析落 `gpui::Keystroke`。
31. **照抄：`EDITABLE_ACTIONS` 可编辑动作表**（`display/keymap.rs` 的 `EDITABLE_ACTIONS` / `editable_row_count` / `display_stored_combo`）。可重绑动作集中常量化（顺序=展示顺序、持久化按 Action 名、重排不动用户数据）、数字系动作与 AI 贴入键只读展示——slTerminal `COMMAND_CATALOG` 的同构物，两表实现期对账合并。
32. **照抄关系裁定：slTerminal `ShortcutRegistry` 四纪律映射 GPUI 键位模型**（`src/features/shortcuts/` 的 `ShortcutRegistry.ts` / `commandCatalog.ts` / `reserved.ts` / `wireKeybindings.ts`）。Command/Keybinding 分离 + 用户覆盖层 + 命令目录单点 + 保留键保护四纪律与 pebrel 键位模型同构，语义保留；active 指针派发/上下文栈/window capture 机制随 WebView 消亡（GPUI `key_context` + `bind_keys` 原生替代）；Ctrl+C 中断透传红线与 Ctrl+Shift+C 复制（产品定位约束）映射为 GPUI 版保留键。

### slTerminal 侧保留资产与归宿映射

33. **归宿：编辑器面板（CM6 栈）消亡，能力由 file_editor + code_tab 承接**（`src/panels/editor/` 的 `EditorPanel.tsx` / `useCodeMirror.ts` / `keyboard.ts`）。打开/保存/外部改动检测/语言切换/自动换行/字体缩放逐项映射进双模态编辑器与 code tab；`gitGutter.ts` 的 git 状态槽语义保留重建（GPUI 世界编辑器 gutter 无 pebrel 对应物，登记实现期新建项）；`repaintGuard.ts` 随 WebView2 GLYPH bug 消亡；`Alt+Z` 换行 / `Ctrl+S` 保存 / `Ctrl+滚轮` 字号映射进 file_editor 键位族。
34. **归宿：大文件防线语义重定——截断 + truncated 提示替代分块只读浏览**（`src/panels/editor/largeFileViewer/` 的 `LargeFileViewer` / `blockCache.ts` / `useLineIndex.ts`）。>10MB 只读分块浏览（固定行高虚拟化 + LRU 块缓存 + `fs_read_file_range`）整体消亡；pebrel 语义 = 上限截断读 + `truncated` 提示横幅（`file_editor/document.rs` / `file_editor/mod.rs` notice），可编辑域上限对齐 pebrel 8MB；1MB 警告弹窗不迁移（截断提示替代）。
35. **归宿：预览类面板消亡，markdown 预览/编辑由 file_editor 承接**（`src/panels/docViewer/` 的 `PreviewFrame.tsx` / `buildInjectedScript.ts` / `previewMessages.ts` / `zoomRuntime.ts` / `scrollRuntime.ts` / `ModeSwitcher.tsx`、`src/panels/markdown/`、`src/panels/html/`、`src-tauri/src/preview.rs` 的 `PREVIEW_SCHEME` / 宿主页）。iframe 沙箱宿主页、消息桥、注入脚本、缩放/滚动运行时、预览自定义协议域整体消亡（GPUI 世界无 WebView）；markdown 的 edit/split/preview 三态由 file_editor 源码/渲染双模态 + 详情侧栏替代并超出；html 渲染预览无 pebrel 对应物，裁定消亡（html 文件 = 源码只读查看）。
36. **归宿：`gitshow`→只读 code tab，`diff`→对照语义重定**（`src/panels/gitshow/`、`src/panels/diff/`）。gitshow（HEAD 内容只读）= code tab 只读形态；diff 面板（HEAD 只读 + 工作区可编辑双栏 + 占位对齐 + 滚动同步）无 pebrel 直接对应——通用 diff 渲染面另行裁决，冲突场景由三栏合并器（采纳点 16）覆盖；占位对齐/滚动同步技艺随 CM6 消亡。
37. **归宿：explorer 能力清单并入 file_tree**（`src/features/explorer/` 的 `ExplorerPanel.tsx` / `FileTreeExplorer.tsx` / `FileTree.tsx` / `useFileTree.ts`）。CRUD、git 状态着色、fs-event 增量刷新、展开态保留、虚拟化行、键盘快捷键——能力语义全数保留，载体换 pebrel 模型/渲染分层（采纳点 21）；`FileTreeExplorer` 共享树组件形态（多宿主复用）映射为 GPUI 树组件复用；`FileIcon` 六色盘登记例外随主题 token 体系重建归分片 06。
38. **归宿：`fileViewers` 注册表消亡，路由由 `openable_in_app` 承接**（`src/features/fileViewers/FileViewerRegistry.ts` 的 `FileViewerRegistry` / `ExtensionBasedViewerStrategy` / `registerDefaultViewers`）。策略链机制不采纳；「扩展名→打开方式」映射值并入 `viewable_file` 判定集（采纳点 19）。
39. **不归本片：`navTree`/`agentFiles` 归工作区侧栏分片**（`src/features/navTree/`、`src/features/agentFiles/`）。项目→页面→活跃/历史会话层级与 Agent 全局文件视图是 slTerminal 独有产品概念（pebrel 无项目/页面模型），GPUI 重建挂 workspace 侧栏骨架（分片 05 登记边界），本片只登记文件树组件的复用关系。
40. **照抄关系裁定：`src-tauri/src/fs/` 命令层消亡，语义资产并入**（`src-tauri/src/fs/mod.rs` 的 `fs_read_dir` / `fs_read_file` / `fs_stat` / `fs_read_file_range`）。单进程后无 IPC 边界，读写 = `std::fs` 直连；保留语义资产：CRLF 行尾检测（64KB 样本判定，并入 text_document 编码检测面）；keyset 分页目录/256KB 分块/10MB 上限随 IPC 消亡（原生世界无 IPC 峰值问题）；沙箱校验消亡（见不采纳点 5）。
41. **照抄关系裁定：`src-tauri/src/git/` 保值为 git 子进程封装**（`src-tauri/src/git/mod.rs` 的 `gitStatus` / `gitDiff` / `gitFileAtHead` 等）。slTerminal 自有 git 命令封装保留；pebrel `display/side_panel/vcs.rs` 的 `--no-optional-locks` 纪律（不碰 index 锁、不污染并发 git 操作）照抄并入；SVN 面不采纳——git 操作集实现期两仓对账。

### 交界登记

42. **登记：数学编辑面与分片 08 的边界**（`file_editor/input_rules.rs` 的 `enter_block`、`file_editor/block_structure.rs` 的 `PartKind::Math`、`file_editor/structure_view.rs` 数学分支）。file_editor 内数学块的编辑结构（`$$` 入口、数学 part 的源 span 切分）归本片；公式编译、栅格化、位图上屏、scientific 资产管线（`gpui_shell/scientific_render.rs` / `gpui_shell/math_view.rs`）归分片 08；本片消费其渲染结果，不重复裁定。
43. **登记：文件→终端拖放与终端分片的边界**（`gpui_shell/file_drop.rs` 的 `FileTreeDrag` 消费点 `drop_text_for_path`）。路径写入 PTY 的规则（控制字符拒绝/含空白加引号/尾随空格）归终端输入分片，本片只照抄拖放载荷与幽灵预览。
44. **登记：快速跳转/命令面板与对应分片的边界**（`gpui_shell/workspace/quick_jump.rs` 的 `rows` 目录投影）。文件相关条目（目录 frecency、tab、AI 会话）的 palette 投影归命令面板分片，本片只登记 file_tree 是其数据源之一。

## 不采纳点

1. **SSH/remote_files 整支，不采纳（已定）**（`gpui_shell/workspace/remote_files/transfers.rs`、`display/sftp_panel.rs`、`file_editor/source.rs` 的 `DocumentSource::Remote` / `new_remote`、`gpui_shell/workspace/remote_files.rs` 的 `drag` / `target`）。SFTP 传输、远端文档适配、远端拖放目标全砍；`DocumentSource` 收敛为本地单态。
2. **WSL 来宾文件枚举，不采纳（已定多平台）**（`display/side_panel/enumerate.rs` 的 `parse_wsl_find_pairs` / `wsl_root_key` / `wsl_guest_join`、`gpui_shell/workspace/file_tree.rs` 的 `wsl_terminal_launch_at`、`path_bar.rs` 的 `resolve_directory` WSL 分支）。宿主 UNC 映射 / guest 路径规范化 / `find` 输出解析整面砍。
3. **CM6 编辑器栈整体，不采纳（载体消亡）**（`src/panels/editor/` 的 `useCodeMirror.ts` / `repaintGuard.ts` / `largeFileViewer/`）。CodeMirror 6、Compartment 热切换、主题槽层叠（ACC-05）、WebView2 GLYPH 防复发、大文件分块浏览（CP-022/BE-03 防线）随 WebView 消亡——承载功能由 file_editor 原生形态替代（采纳点 33/34），GLYPH e2e 防复发锚一并退役。
4. **docViewer iframe 沙箱体系，不采纳（载体消亡）**（`src/panels/docViewer/` 全家、`src-tauri/src/preview.rs`、`src/panels/html/` 与 `src/panels/markdown/` 的 iframe 渲染面）。自定义协议域、宿主桥、注入脚本、缩放/滚动运行时、预览 CSP 域纪律随 WebView 消亡；markdown 渲染由 gpui TextView 原生管线替代；SEC 系列预览红线（CVE-2024-35222 等）的防护对象不复存在。
5. **Tauri 文件沙箱与 IPC DTO 层，不采纳（架构已定）**（`src-tauri/src/state.rs` 的 `validate_path_within_root`、`src/types/fs.ts`、`src/ipc/fs.ts` / `src/ipc/git.ts`）。单进程无 IPC 边界、无 webview 攻击面，路径沙箱校验与 ts-rs DTO 生成物整体消亡；「项目根外访问」产品语义由 path_bar 任意路径导航替代（越出项目根允许，与 pebrel 一致）。
6. **file_dialog 非 Windows 平台分支，不采纳（已定）**（`display/file_dialog.rs` 的 Linux/macOS 平台模块、`display/file_operations.rs` 非 Windows 回退）。仅保留 Windows 原生分支。
7. **双壳兼容包袱，不采纳**（`display/side_panel/render.rs` 等旧 OpenGL 壳渲染、`gpui_shell/workspace/file_tree.rs` 注释中的「旧壳基准」引用链）。pebrel GPUI 壳与旧壳的双写兼容（`legacy-shell` feature、旧壳几何基准数字）不迁移；基准数值仅作参考不进代码。
8. **quick_terminal 键位行，不采纳（已定）**（`display/keymap.rs` 的 `QUICK_TERMINAL_ROW` / `DEFAULT_QUICK_TERMINAL_HOTKEY`）。
9. **send_to_chat 远端安全分层，不采纳（SSH 砍）**（`gpui_shell/workspace/send_to_chat.rs` 的远端 pane 徽标 / `NEBULA_PANE_REMOTE` 环境自查 / `ensure_local_context_allowed` 远端分支）。本地 Agent pane 选区引用工作流保留语义归分片 03；三层安全中的远端两层随 SSH 消亡。
10. **`fileViewers` 策略注册表机制，不采纳**（`src/features/fileViewers/FileViewerRegistry.ts`）。策略链/`_reset()` 注册表家族契约的 WebView 载体消亡；「扩展名→打开方式」值集并入 `viewable_file`（采纳点 19/38）。
11. **navTree/PlanBalance 面板形态，不采纳 pebrel 对照物缺失**（无 pebrel 对应）。pebrel 无项目/页面/会话导航与用量面板概念，不为其寻找 GPUI 对照物；归 workspace 侧栏分片按 slTerminal 产品语义重建（采纳点 39）。
12. **pebrel 亮主题双角色代码，不采纳（已定仅暗色）**（`file_editor/code_actions.rs` 的 `CodeUiColors` 亮/暗双角色表等）。双主题角色收敛为暗色单套。

## 优化方向

文件与编辑在 GPUI 单进程壳内重建为三个 Rust 模块族：slterm_file_editor（双模态编辑器 + live_edit WYSIWYG 全家，照抄改名）、slterm 侧栏文件树（side_panel 模型分层 + file_tree 渲染 + path_bar，照抄改名）、tab 路由（code_tab 三栏合并 + doc_tabs 图片/文档 tab + `openable_in_app` 判定，照抄改名）。CM6 与 iframe 沙箱两大 WebView 栈整体消亡，其承载的打开/保存/外部改动/git gutter/预览能力逐项落位：git gutter 与通用 diff 对照面为实现期新建项（pebrel 无对应物），其余全部由 pebrel 原生形态覆盖并超出。

编辑器方向：TextFileView 双模态（markdown 渲染态默认、显式切源码）为编辑器产品形态基准；live_edit 投影内核 + 结构化块事务 + 输入规则 + 统一撤销历史构成编辑内核；大文件语义统一为 pebrel 截断模型（上限截断 + truncated 提示），slTerminal 分块只读浏览与四层防线退役；外部修改同步走 Rust 侧 fs notify 直连文档模型（无 IPC、无轮询心跳、无保存回声抑制链）。

文件树方向：模型/渲染分层照 pebrel（目录枚举 + gitignore 过滤索引 + git 状态 + 文件名索引随 fs 通知存活），slTerminal explorer 的 CRUD/git 着色/增量刷新/展开态保留能力全数保留为 Rust 模型行为；拖放文件进终端、回收站删除、路径栏导航为新增能力；navTree/agentFiles 挂 workspace 侧栏骨架重建（分片 05 边界）。

键位方向：用户自定义键位表 JSON 化（分片 06 已定），pebrel keybind_pairs 语义（行序即优先级、整表替换、恢复默认精确收回）照抄；GPUI `key_context` 替代窗口捕获 + 上下文栈；Ctrl+C 中断透传与 Ctrl+Shift+C 复制两大产品红线映射为 GPUI 保留键。

测试方向：file_editor 的投影/结构/输入规则/历史/图片缓存纯逻辑全部 Rust 单测随照抄迁移；文档快照/保存冲突/截断读取 round-trip 收敛到 text_document 单点；file_tree 行平铺/gitignore/搜索过滤纯函数化测试；git gutter 与 diff 对照面新建项补全测试。测试体系总体归分片 11。
