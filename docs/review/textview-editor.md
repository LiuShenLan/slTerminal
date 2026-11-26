# 专用文本视图、编辑器与分页审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支）；范围限 `text_view`、`file_editor`、分页读取与相关文档，只读审查，未执行构建、测试或网络访问。

## 覆盖与局限

- 已读 `slterm_app/src/app_shell/text_view/` 全部模块及其 `CLAUDE.md`、`source.rs` 与 `paged_tests.rs`/`editor_tests.rs` 的测试实现，`slterm_app/src/app_shell/file_editor/`、`paged_document.rs`、`document_io.rs`、`text_document.rs`、`documents.rs`、`document_external.rs`，以及 AppRoot 的 `documents.rs`、`document_external.rs`、`editor_git.rs`、`diff.rs`、`git_show.rs`、`commands.rs` 的恢复与打开接线。
- 已读 `docs/devPlan/design/document.md`、`docs/devPlan/traceability.md` 的 Document／Editor 行、`docs/devPlan/plan.md`、`docs/devPlan/progress.md`、`docs/featureSpec/panels/editor.md`、`docs/featureSpec/panels/document.md`、`docs/featureSpec/panels/gitshow.md`，以及 core 的 `encoding.rs`、`paged.rs`、`external.rs`、`replace.rs` 与相关 `CLAUDE.md`。
- 已核对：Rope／Paged 可见行物化、Unicode 字素与显示列、软换行索引与检查点、布局缓存键、选区与 IME 虚拟输入、ViewKey 恢复、HTML 系统打开、Git 行标投影、分页页边界／LRU／复制／键盘续动、外部重载与形态转换。
- 未覆盖：GPUI 真实帧调度、真实 OS IME 候选窗与像素级对齐、Markdown／Diff 内部实现（仅按文本视图消费者阅读）、真实 Windows 桌面与文件系统竞态。测试仅阅读源码判断语义，未运行。
- 实时正文：视图持 Rope 或分页页缓存，未发现编辑路径可写入的第二份长驻正文；跨域 Diff 的前景正文转换见「跨域观察」。

## 问题

### 阻断｜Editor 面板规定能力未实现，追溯与进度却标记已验收

- 置信度：确认
- 位置：`docs/featureSpec/panels/editor.md` 的「面板与编辑能力」「查找、替换与折叠」「语言与显示」「编码与行尾」「只读与大文件」；`slterm_app/src/app_shell/text_view/editor.rs` 的 `DocumentTextView`；`slterm_app/src/app_shell/text_view/mod.rs` 的 `key_bindings`；`docs/devPlan/traceability.md` 的 Editor 表；`docs/devPlan/progress.md` 的「当前进度」。
- 现象：编辑器只提供移动、选择、普通编辑、撤销／重做、剪贴板、软换行和语言选择。查找、替换、折叠、行号、跳转行、缩进、括号匹配、当前行、文档内补全、状态条（语言／编码／BOM／行尾／只读原因）、行尾切换、编码重开、大文件提示、字号设置均不存在；`Ctrl+F`、`Alt+Z`、`Ctrl+Alt+G`、`Ctrl+滚轮` 未绑定，`Tab` 未绑定为缩进。
- 根因：交付停留在基础编辑文本视图，featureSpec 的 Editor 命令面与状态条未接线；追溯矩阵与进度在没有对应面板实现的情况下给出了完成结论。core 的 `replace_request`/`editor_replace_all_single_undo` 只证明替换计划与单次撤销语义，不能替代面板入口、确认和可观察结果。
- 影响：featureSpec 的 Editor 验收场景无法成立；用户无法执行规定操作；`Ctrl+滚轮` 当前只滚动，`Ctrl+F`/`Alt+Z` 无响应；`Ctrl+S` 之外的命令目录与键位合同不完整。
- 证据：`key_bindings` 的封闭 action 集合不含查找、折叠、行定位或字号动作；`DocumentTextView::render` 底栏仅有换行按钮与语言按钮；`EditorViewState` 仅保存滚动、换行与语言覆盖，没有查找／折叠状态；`slterm_app/src/app_shell` 内没有 `LineEnding` 切换调用，core 也没有行尾修改入口。
- 优化方向：按 featureSpec 将缺失命令接入统一命令目录与瞬态视图状态，或按既有纪律登记正式延期并修正追溯矩阵与进度；验收应落到面板可观察结果，而不是 core 单元语义。

### 重要｜合同冲突：本地编码检测与编码重开在实际打开链中不存在

- 置信度：确认
- 位置：`slterm_document/src/encoding.rs` 的 `DocumentContent::decode`；`slterm_document/CLAUDE.md` 的「编码与内容路由」；`slterm_app/src/app_shell/file_editor/document.rs` 的 `Document::load`；`slterm_app/src/app_shell/documents.rs` 的 `DocumentOpenService::load`；`slterm_app/src/app_shell/paged_document.rs` 的 `detect_encoding`；`docs/featureSpec/panels/editor.md` 的「编码与行尾」。
- 现象：自动识别只接受 BOM 或合法 UTF-8；GBK／GB18030 只能由调用方显式传入编码。实际打开链一律传 `None`，并且应用内没有“选择编码打开／以编码重新打开”的入口。GBK 文本、超限 GBK 文件都会在解码阶段失败，无法建立编辑或分页视图。
- 根因：core 决策要求 GBK 显式指定，而 featureSpec 要求先本地编码检测、无法判定时提供编码重开；应用层没有实现显式指定的 UI/命令，形成合同冲突且两边都未兜底。
- 影响：featureSpec 的「GBK 文本分别按检测编码编辑、提供编码重开」无法通过；未知编码文件没有恢复路径；`editor_encoding_and_binary_routing` 的名称与实际场景不一致。
- 证据：`DocumentContent::decode` 将 `requested=None` 的非法 UTF-8 映射为 `UnknownEncoding`；`Document::load` 对 `Unavailable` 直接返回错误；`DocumentOpenService::load` 对可编辑与分页分支都没有传入编码；`PagedDocument::detect_encoding` 只有 BOM/UTF-8 分支。`editor_encoding_and_binary_routing` 只对 core 直接传入 `Gbk` 做断言，打开链只断言非法与二进制失败。
- 优化方向：由所有者裁决 core 显式编码决策与 featureSpec 检测／重开要求的优先级；若保留 core 决策，必须提供唯一的显式编码打开／重开入口并覆盖分页；若扩展检测，必须继续严格拒绝非法序列。验收需真实 GBK 文件与编码重开两种场景。

### 重要｜追溯矩阵的自动化证据层级与场景不一致

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 Document／Editor 行；`slterm_workspace/src/project_snapshot_tests.rs` 的 `ordinary_open_does_not_deduplicate_diff_or_gitshow`；`slterm_app/src/app_shell/document_external.rs` 的 `editor_large_file_dirty_preserves_buffer`；`slterm_app/src/app_shell/text_view/paged_tests.rs` 的 `paged_view_restores_scroll_state_without_transients`；`slterm_app/src/app_shell/text_view/editor_tests.rs` 的极端滚动用例。
- 现象：Diff／GitShow 不参与普通去重只由纯工作区快照用例证明，没有 AppRoot 面板级可观察断言；Editor 的大文件脏缓冲只由保存服务与 core 用例证明，AppRoot 的脏外部增长场景只有 Markdown 用例；分页恢复用例手工调用 `persist_view_state` 后只断言 `wrap` 布尔值与选区，未断言渲染真正采用恢复状态或最终 DTO 不含瞬态；极端恢复滚动用例只验证不 panic。
- 根因：追溯行以底层状态机用例充当面板验收锚，未核对“前置条件、动作、可观察结果”三者是否落在同一层。
- 影响：`已自动化` 结论高估了面板接线覆盖；Diff/GitShow 并存、共享正文的脏增长保留和分页恢复渲染等回归可以在矩阵全绿时漏过。
- 证据：`ordinary_open_does_not_deduplicate_diff_or_gitshow` 只操作 `ProjectSnapshot` 布局与 `with_opened_document`，未创建 AppRoot 面板或视图实体；`editor_large_file_dirty_preserves_buffer` 只断言 Session 文本与 `begin_save`；分页恢复用例没有绘制帧或持久 JSON 断言。
- 优化方向：为这些场景补面板级断言或明确标注证据层级；恢复用例应断言绘制行与持久 DTO，而不只断言内存字段。

### 重要｜合同冲突：分页只读视图不实现换行，且与软换行 Editor 共用滚动坐标单位

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/paged.rs` 的 `PagedTextView` `wrap` 字段、`from_parts`、`persist_view_state` 与 `render`；`slterm_app/src/app_shell/text_view/CLAUDE.md` 的 Rope 软换行边界；`docs/featureSpec/panels/editor.md` 的「只读与大文件」「语言与显示」；`docs/featureSpec/panels/gitshow.md` 的「文本、查找与显示」。
- 现象：分页视图恢复并持久化 `wrap`，但渲染固定传 `soft_wrap: false`，从不使用该字段；分页正文没有语法高亮、行号、查找、折叠和大文件／高亮降级提示。`EditorViewState.scroll_line` 在软换行 Editor 中是显示行，在分页视图中是逻辑行；同一 ViewKey 在 Editor、Markdown 源码与 Paged 之间转换时，该字段被直接套用，恢复位置会落在错误行。
- 根因：模块决策只把软换行接入 Rope Editor、Paged 仍用既有逻辑行索引，但 featureSpec 与 GitShow 规范要求只读浏览支持换行、行号、查找与折叠；共享 ViewState 没有携带坐标单位或视图形态，转换侧也没有换算。
- 影响：只读浏览验收不成立；分页大文件不能换行或查找；`Wrap` 开启的 Editor／Markdown 在外部增长切换为 Paged 后阅读位置错误，切回时又把逻辑行当显示行。
- 证据：`PagedTextView::render` 的 `TextLayoutKey` 固定 `soft_wrap: false`；视图结构没有 `DocumentSyntax`、查找状态或行号绘制；`PagedTextView::persist_view_state` 直接复用 `EditorViewState::new(self.scroll_line, ...)`；`EditorViewState` 没有形态字段。模块 `CLAUDE.md` 的边界句确认这是未登记的延后，而追溯矩阵已把相关大文件行标为已自动化。
- 优化方向：实现分页只读规范面或登记正式延期；若保留共享 DTO，必须明确坐标单位并在形态转换时换算（或让 Paged 真正支持换行），并补恢复／转换测试。

### 重要｜分页页读取失败后永久停留 Loading，无错误与重试入口

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/paged.rs` 的 `PagedTextView::apply_page_result`、`failed_pages`、`claim_pages`、`visible_line_windows` 的 `VisibleRow::Loading` 分支；`docs/featureSpec/panels/gitshow.md` 的区段读取失败与重试要求。
- 现象：任一页读取错误或缓存拒绝都会写入 `failed_pages`；该集合只在 `relocate_reader` 与 `apply_snapshot` 中清空。视图没有错误态、提示或重试命令，失败行永远显示 Loading；滚动、重绘和再次进入视图都不会重试；GitShow 对相同 blob 的刷新也不会重建分页视图。
- 根因：把“不得逐帧自动重试”实现成了永久抑制，没有提供显式重试与可见失败状态。
- 影响：一次瞬时 I/O 失败会让大文件或 GitShow 固定版本的部分内容永久不可见，用户只能关闭面板重开；没有原因提示，违反 GitShow 的“保留已加载内容并提供重试”。
- 证据：`apply_page_result` 的错误分支只插入 `failed_pages`；`claim_pages` 对失败页直接过滤；渲染只按缓存命中与否产出 Text／Loading；`GitShowView::apply_snapshot` 在 blob 未变时直接返回，不替换分页实体，因此相同固定版本的再次回投也没有重建路径。
- 优化方向：引入显式的失败／重试状态与用户命令，区分临时读取失败和源已变化；补充失败后重试与恢复用例。

### 重要｜外部快照与撤销／重做把滚动位置重置到顶部并覆盖持久值

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/editor.rs` 的 `DocumentTextView::apply_snapshot`、`travel_history`、`ensure_cursor_visible`、`persist_view_state`；`slterm_app/src/app_shell/app_root/documents.rs` 的 `AppRoot::apply_document_edit` 的 `Travel` 分支；`docs/featureSpec/panels/document.md` 的外部重载阅读位置要求；`docs/featureSpec/panels/editor.md` 的滚动恢复与历史瞬态要求。
- 现象：`apply_snapshot` 把选区设为 0 后调用 `ensure_cursor_visible`，可见视图随即滚到顶部；滚动变化又经 `persist_view_state` 写回 ViewState。撤销、重做和干净外部重载都会走这条路径。权限变化等仅元数据变化的外部 `Reload` 也会触发同样的归零。
- 根因：把滚动位置当作快照重置的瞬态处理，没有在无增量快照或历史移动时保留／重映射阅读锚点；契约只要求复位选区、组合与光标类瞬态，没有要求清零持久滚动。
- 影响：用户在文档中段撤销一次就会跳回顶部，并把新的 0 滚动位置写盘；外部干净重载丢失阅读位置，违反“尽量保留各自阅读位置”，也让关闭重开恢复到错误位置。
- 证据：`apply_snapshot` 将 `selection` 置零后调用 `ensure_cursor_visible`；后者在滚动变化时调用 `persist_view_state`；`travel_history` 对来源视图执行同样的归零路径，AppRoot 的 `Travel` 广播对其余视图调用 `apply_snapshot`。现有撤销测试只断言文本与选区，没有断言滚动或持久 DTO。
- 优化方向：在无增量快照与历史移动前捕获逻辑锚点，按新旧文本重映射滚动；文本未变时保留原滚动。选区／IME 的复位继续按现合同执行，并补撤销与外部重载的滚动断言。

### 重要｜Git 行标丢失增改删语义，颜色与越界防护不满足合同

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/editor_git.rs` 的 `EditorGitState`、`finish_editor_git`、`schedule_editor_git`；`slterm_app/src/app_shell/text_view/editor.rs` 的 `set_git_marks`、`git_markers`；`slterm_app/src/app_shell/file_editor/CLAUDE.md` 的 git gutter；`slterm_app/src/app_shell/editor_theme.rs` 的 `EditorThemeTokens`；`docs/featureSpec/panels/editor.md` 的「Git 行标」。
- 现象：跨视图边界只传 `Vec<usize>` 行号，新增／修改／删除／重命名／冲突语义全部丢失；绘制统一使用 `cx.theme().accent` 的 4 像素竖条，没有形状区分，也没有引用 `EditorThemeTokens` 的 `gutter_*` 槽。只读 Editor 仍会调度行标；越界行号在普通布局会被外框裁掉，在软换行布局会进入 `Rope::line_to_byte_idx`，越界时 panic。
- 根因：设计把行标简化为“行号投影”，没有携带语义枚举；模块文档与主题 token 决策描述的 gutter 语义没有进入新文本视图。
- 影响：featureSpec 要求颜色与形状同时区分语义，当前无法区分；只读文档出现无意义的编辑域行标，与 file_editor 决策冲突；若后台注入越界行号，软换行绘制会 panic 而非安全跳过。
- 证据：`set_git_marks` 形参为行号集合；`git_markers` 只读 `self.git_marks` 并固定 `px(4.0)`／`px(20.0)`；`finish_editor_git` 把 `diff_change_locations` 的 worktree 行号压平成集合；`install_panel` 只按 `ViewKey::Editor` 调度行标，不检查 `session.is_read_only()`；`git_markers` 的软换行分支直接调用 `self.text.line_to_byte_idx(line, ...)` 且没有行数上界检查。
- 优化方向：以语义行标类型穿越视图边界，映射到 `EditorThemeTokens` 的 gutter 语义槽并用形状区分；绘制前夹紧或跳过越界行；只读文档不调度行标。补删除／修改形状、只读与越界注入用例。

### 重要｜`reveal_line` 在软换行下把逻辑行当显示行

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/editor.rs` 的 `DocumentTextView::reveal_line`；`slterm_app/src/app_shell/app_root/diff.rs` 的 `DiffView::reveal_worktree_line`、`reveal_change`；`slterm_app/src/app_shell/text_view/CLAUDE.md` 的「普通与软换行布局共用同一逻辑行定位」。
- 现象：`reveal_line` 直接把传入行号写入 `scroll_line` 并把 `scroll_column` 置零；软换行渲染把 `scroll_line` 解释为显示行。Diff 的右侧工作区视图可以按 ViewState 恢复为软换行，此时差异块导航会滚到更早的内容；若换行索引尚未就绪，也没有延迟定位。
- 根因：单一滚动字段在两种布局中语义不同，`reveal_line` 没有经过换行索引换算，也没有待 reveal 机制。
- 影响：Git 行标与 Diff 差异导航在软换行下定位错误；验收“定位对应差异块”只在未换行时成立。
- 证据：`reveal_line` 只做 `line.min(逻辑行数 - 1)`；`reveal_worktree_line` 直接调用它；`DiffViewState.source` 可持久化 `wrap = true`，Diff 构造时经共享 DTO 恢复；现有 Diff 导航测试不检查滚动目标。
- 优化方向：用软换行索引把逻辑行换算为显示行，索引未就绪时记录待 reveal；Rope 与 Paged 布局共用同一逻辑定位语义并补换行导航测试。

### 重要｜未知扩展名的文本文件在读取前被拒绝，不能进入 Editor

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/documents.rs` 的 `AppRoot::prepare_document_open`；`slterm_app/src/app_shell/code_tab.rs` 的 `viewable_file`；`docs/featureSpec/panels/document.md` 的「打开与视图路由」；`docs/featureSpec/panels/editor.md` 的「语言与显示」。
- 现象：`prepare_document_open` 只接受 Markdown 或 `LanguageRegistry` 能映射扩展名的文件，其他路径在真实读取前返回“文件类型不支持应用内文本编辑”。未知扩展名的 UTF-8 文本（如普通脚本、日志、数据文本）无法在应用内打开，文件树也不会进入 Editor。
- 根因：把“语言注册表有映射”当成“是文本文件”的判定，混淆了高亮语言与内容类型；规范要求未知语言使用纯文本、其他文本进入 Editor。
- 影响：大量合法文本文件无法编辑；NUL／编码检测从未有机会参与未知扩展名文件的分类；与 Document 打开路由和 Editor“未知类型使用纯文本”冲突。
- 证据：`viewable_file` 以 `LanguageRegistry::for_path(...).is_some()` 为真值；`prepare_document_open` 用它做前置拒绝；core 的 `LanguageRegistry` 对未登记扩展名返回 `None`，而规范要求未知类型用 `PlainText`。
- 优化方向：用真实读取、NUL 探测与编码检测决定文本与否，未知语言回落 `PlainText`；补未知扩展名 UTF-8 文件与二进制文件的打开用例。

### 重要｜分页恢复占位是无出口面板，重新打开只会再次聚焦占位

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commands.rs` 的 `AppRoot::restore_project_panels` 的 Paged 分支；`slterm_app/src/app_shell/app_root/document_recovery.rs` 的 `DocumentRecoveryView`；`slterm_app/src/app_shell/app_root/documents.rs` 的 `install_opened_document` 的既有 Panel 聚焦分支；`slterm_app/src/app_shell/app_root/tests/documents.rs` 的 `paged_document_restart_has_explicit_placeholder`。
- 现象：恢复到分页 Document 时只安装 `DocumentRecoveryKind::Paged` 文本占位，不构建读取端口或分页视图。占位视图没有按钮、命令或重试动作。用户从文件树重新打开同一文件时，`install_opened_document` 先按完整 ViewKey 找到这个占位面板，只还焦而不替换内容；在用户手动关闭面板之前，分页正文无法重新出现。
- 根因：启动恢复把分页内容定义为“需要专用只读视图”，但没有提供进入专用视图的路径；占位没有显式重开动作，面板查重又把占位当成已安装视图。
- 影响：异常恢复后的分页面板成为死端；用户必须关闭标签再打开，且占位文案只说明需要视图，不提供操作。验收“恢复失败保留面板或授权占位”没有配套的可恢复性。
- 证据：恢复分支只调用 `restore_recovery_panel(..., DocumentRecoveryKind::Paged, ...)`；`DocumentRecoveryView::render` 只绘制文案；`install_opened_document` 在 `panel_for_view` 命中后直接聚焦并提交快照，不比较内容形态。
- 优化方向：恢复时按真实授权与文件身份重建分页读取端口并安装分页视图，或为占位提供显式重开／重试动作并让打开链替换占位；补“恢复后重新打开可得到正文”的 AppRoot 用例。

### 一般｜Git 行标显隐会改变左侧留白并触发换行重排

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/editor.rs` 的 `DocumentTextView::render` 的 `git_gutter_width` 与 `adopt_snapshot`。
- 现象：只有 `git_marks` 非空时才给正文加 gutter 宽度，正文容器随行标显隐在 0 与固定宽度之间切换；每次文本快照都会清空行标，后台结果返回后又恢复。正文水平位移，窗口宽度跨过整列边界时软换行索引被废弃并重建。
- 根因：gutter 宽度由瞬态行标集合决定，而不是由编辑域形态决定的稳定布局；行标清空是快照对账的一部分。
- 影响：编辑时正文左右抖动，软换行重排和大文件换行索引重建；行标短暂消失还会让点击目标移动。
- 证据：`git_gutter_width` 以 `git_marks.is_empty()` 为唯一判据并用于 `.pl(git_gutter_width)`；`adopt_snapshot` 在每次编辑快照时 `git_marks.clear()`；`TextLayoutKey` 与 `SoftWrapIndex` 的列数都取自实际窗口宽度。
- 优化方向：编辑域保留稳定的 gutter 宽度，行标只在该槽位内绘制；补编辑前后正文宽度与换行列数不抖动的用例。

### 一般｜布局缓存没有容量或可见窗口回收

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/layout.rs` 的 `LayoutCache`；`DocumentTextView` 与 `PagedTextView` 的 `layout_cache`。
- 现象：`LayoutCache` 只在布局键变化时清空 `HashMap`；纵向滚动持续插入新行的 `ShapedLine`，从不移除。分页视图同样按行号累积。
- 根因：把布局缓存当作无界会话缓存，没有按视口窗口或容量淘汰。
- 影响：阅读距离越远，前台内存越大；在允许编辑的大文件或大分页文件上可能造成持续增长，违反前台成本有界。
- 证据：`LayoutCache::activate` 是唯一清空路径；`get`／`insert` 没有淘汰；缓存键变化只覆盖正文、主题、宽度、列与 DPI。
- 优化方向：按可视行窗口淘汰或设置容量上限，并补长距离滚动后的内存或条目上限断言。

### 一般｜遗留第二实现与文档漂移仍留在文本域

- 置信度：确认
- 位置：`slterm_app/src/app_shell/file_editor/gutter.rs` 的 `GitGutter`、`read_for_path`；`slterm_app/src/app_shell/file_editor/CLAUDE.md` 的 git gutter；`slterm_app/src/app_shell/diff_tab.rs`；`slterm_app/src/app_shell/editor_theme.rs` 的消费说明；`slterm_app/src/app_shell/file_editor/document.rs` 的 `Document::from_recovered`；`slterm_app/src/app_shell/text_view/source.rs` 的 `TextSource`；`.claude/docs/testing/exemptions.md` 的 gutter 豁免行。
- 现象：`gutter.rs` 使用 `file_status`／`file_hunks` 的 gutter 没有任何生产消费者，生产行标走 `editor_git.rs` 的 core 差分；`file_editor/CLAUDE.md` 仍把 `gutter.rs` 描述为行标数据源。`diff_tab.rs` 没有生产消费者，`editor_theme.rs` 的 `EditorThemeTokens` 只被该遗留模块与 Markdown 消费。`Document::from_recovered` 无调用者。`TextSource` trait 的 `visible_lines` 只被测试使用，生产渲染分别使用 Rope／Paged 的固有窗口 API，Rope／Paged 的 shaping、选区绘制、命中测试与滚动逻辑独立演化。豁免登记仍以 `gutter_tests` 为兜底。
- 根因：迁移后只换了生产接线，没有同步删除旧实现、旧测试锚与旧文档；统一读取抽象停在了类型层。
- 影响：后续维护可能误接被 AppRoot 决策禁止的 `file_hunks` 行标路径，或更新错误的模块文档；Rope 与 Paged 的文本窗口语义容易漂移；豁免锚指向非生产模块。
- 证据：生产搜索无 `gutter::`、`GitGutter`、`read_for_path`、`diff_tab::` 的消费；`from_recovered` 只有定义；`TextSource` 的 trait 方法只在 `source.rs` 测试中出现；模块级 `allow(dead_code)` 与“生产消费归后续接线”注释仍在已验收状态保留。
- 优化方向：删除或重写遗留模块、未用入口与过期豁免，保留单一的 core 差分行标路径；把可见窗口／布局端口收敛为生产共用的接口，或明确记录 Rope／Paged 分叉的理由。

### 提示｜分页命中测试在缺页时静默丢弃点击且不发起读取

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/paged.rs` 的 `PagedTextView::byte_for_point`；`slterm_app/src/app_shell/text_view/source.rs` 的 `PagedTextSource::missing_pages`。
- 现象：`byte_for_point` 新建 `PagedTextSource` 只读取共享缓存；目标行所在页缺失时返回 `None`，随后丢弃 `missing_pages`，没有像绘制路径那样调用 `request_pages`。点击缺页区域没有任何反馈或后续加载。
- 根因：命中测试没有接入缺页请求通道，也没有挂起点击。
- 影响：页面加载窗口内的鼠标点击被静默吞掉，用户需要再次点击；交互与绘制加载路径不一致。
- 证据：`byte_for_point` 只调用 `visible_line_windows` 并 `next()?`，不读取 `missing_pages`；绘制 prepaint 却显式收集 missing 并调用 `request_pages`。
- 优化方向：让命中测试的缺页进入同一请求通道或保存待续点击；补缺页点击后自动定位的用例。

## 跨域观察

- Diff 面板在共享 Document 干净外部重载或正文形态变化时没有同步：`AppRoot::apply_document_location_to_views` 只收集 Editor／Markdown／Paged，`convert_external_panel_kinds` 也不处理 `PanelContent::Diff`。结果为 Diff 右侧 `DocumentTextView` 仍显示旧正文，Document 已切 Paged 时其编辑入口也保持“可编辑”外观。涉及 `slterm_app/src/app_shell/app_root/document_external.rs`、`diff.rs`。这与“干净外部修改后全部视图共享新文本”及“Editor 与 Diff 继续引用同一 Document”不一致。
- `DiffView::schedule_diff` 在 GPUI 前台把整份 Rope `to_string()` 成字节再交给后台差分；大文档每次共享编辑都会在前台完成一次全文转换。涉及 `slterm_app/src/app_shell/app_root/diff.rs`。
- `GitShowView` 的文本正文经 `DocumentTextView::fixed` 构造，固定传 `syntax: None`，因此固定版本文本没有共享语言高亮；GitShow 规范要求行号、共享高亮、查找、换行与字号控制，这些能力同样未由文本视图提供。只读 `Ctrl+S` 只被吞掉，没有提示。
- 外部重读／解码失败只在 `AppRoot` 记录日志，没有把原因与重试回投到面板；与 Editor 规范“既有 Document 重读或解码失败保留面板、路径、原因与重试”不符。冲突状态也没有横幅或解决入口。
- 新文本视图与分页视图使用 `cx.theme()` 的组件库主题，而不是 `Settings.editor_theme`；自定义主题与 `EditorThemeTokens` 的颜色不会作用于 Editor／Paged。行标也未使用 `gutter_*` 语义槽。
- 分页／编辑器布局缓存与 `LayoutCache` 的正文存活期没有和 Document 关闭／形态转换显式对账；`paged_documents` 在 Paged→Editable 转换后不会立即移除旧 `Arc<PagedDocument>`，只能等文档失去全部引用时回收。

## 存疑未证

- 软换行索引对超大单文件的全量构建延迟与首帧空白时长未实测；若索引构建期间视图不提供任何回退，用户会看到空白编辑区。需要真实大文件与帧预算测量。
- Git 行标越界注入目前没有观察到生产生产者；`Vec<usize>` 的越界崩溃路径是否会被未来的差分源或竞态触发，需要运行时证据。
- 分页 LRU 在真实内存压力与同身份原地改写之间的行为未实测；`PagedDocument` 的页读取复核身份与长度，但不比对页内容指纹。
- 真实 OS IME 候选窗定位、组合态出字与像素级行标／选区对齐按豁免登记，虚拟窗口断言不能代表真实桌面；`.claude/docs/testing/exemptions.md` 的 IME 与 gutter 条目仍引用旧设计路径与旧 `gutter_tests`，需要所有者确认兜底范围。
- 分页换行、查找、行号与语法是否属于本轮正式延期，模块 `CLAUDE.md` 与 featureSpec／追溯矩阵给出不同信号，需要所有者裁决后才能判断是缺陷还是登记延期。
