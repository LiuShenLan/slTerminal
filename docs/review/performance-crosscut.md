# 跨模块性能与资源预算审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`，当前分支；只做静态代码与已有测试证据审查，不做基准测试。

## 覆盖与局限

- 已读：`.claude/docs/architecture/overview.md`、`.claude/docs/release/packaging.md`；`docs/devPlan/plan.md`、`progress.md`、`traceability.md`；`docs/devPlan/design/terminal.md`、`document.md`、`markdown.md`、`files-git.md`、`persistence.md`、`runtime.md`、`testing.md`；`docs/featureSpec` 的终端、文档、Markdown、Diff、Explorer、Commit、GitShow、Runtime、设置与持久化、跨域约定、验收。
- 已检查的目标代码：`slterm_terminal` 的 `term`、`grid`、`render`、`event_loop`；`slterm_app` 的 `gpui_terminal`、`app_shell/text_view`、`app_shell/markdown`、`math`、`git`、`persistence`、`local_control`、`app_shell/fs_watch`、`learning`、`history`、`app_shell/app_root` 的 Diff／Explorer／Commit 路径；`slterm_document` 的 Markdown、分页、差分与搜索实现。
- 只核对了测试名、断言路径与测试覆盖关系，未运行 `cargo`、测试、基准、网络或真实 GUI；没有实测吞吐、延迟、内存、启动耗时或 GPU 绘制数据。未读取真实用户数据目录。
- 未覆盖：真实 ConPTY 洪峰时序、真实 GPU 绘制成本、真实大目录、真实大仓库事件风暴、真实启动耗时、真实数据库体积；这些只作为存疑项列出，不作为确认结论。
- 体积预算静态核对：`scripts/tests/package-release.tests.ps1` 的 setup／zip 预算字节与根 profile 钉值一致，`dist` 现有产物低于预算；未发现伪造百分比式性能声明。

## 问题

### 重要｜Diff 每次共享编辑都重建全文并启动不可取消的差分计算

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/diff.rs::DiffView::apply_shared_edit`、`DiffView::schedule_diff`；`slterm_app/src/app_shell/app_root/documents.rs::AppRoot::apply_document_edit`；`docs/featureSpec/panels/diff.md`『差异计算与 HEAD 刷新』；`docs/featureSpec/cross-cutting.md`『后台与可见性』
- 现象：共享 Document 的每次编辑都会广播给所有项目中的 Diff 面板，`schedule_diff` 在当前 GPUI 更新回调里读取完整 Rope 快照、执行 `to_string().into_bytes()`，再新建一个后台 `compute_diff` 任务；前一代任务没有取消，`diff_generation` 只用于丢弃结果。没有可见性判断，也没有任何重算间隔。
- 根因：编辑广播没有按可见性和去抖筛选；每次编辑都构造完整 `DiffInput` 并复制工作区全文；任务只有代次丢弃，没有单飞行、取消令牌或排队合并。
- 影响：大文档输入时每个按键都在前台复制整份正文，并触发最长 2 秒的后台 Myers 计算；隐藏项目和隐藏面板同样消耗 CPU。与 `docs/featureSpec/panels/diff.md` 要求的“停止编辑后按应用设置的重算间隔后台计算，默认 300 毫秒，可配置 100–2000 毫秒”以及“不可见时暂停后台刷新与纯展示重算”不一致。
- 证据：`DiffView::schedule_diff` 的 `snapshot.text.to_string().into_bytes()` 和每次编辑新建的 `background_executor().spawn(compute_diff)`；`AppRoot::apply_document_edit` 遍历全部项目面板广播；`slterm_settings` 与设置字段表中没有 Diff 重算间隔键；现有 `diff_hidden_head_refresh_coalesced` 只覆盖 HEAD 刷新，不覆盖共享编辑计算。
- 优化方向：按可见性门控共享编辑广播，按设置间隔合并连续编辑，保持每个 Diff 视图最多一个在途计算并真正取消旧任务；避免前台全文转换，改为共享不可变文本快照或增量差分输入。

### 重要｜Diff 大文件降级路径缺失

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/diff.rs::DiffView::open_diff`、`DiffView::new`；`slterm_app/src/app_shell/app_root/documents.rs::AppRoot::build_source_content`；`docs/featureSpec/panels/diff.md`『大文件与二进制』；`docs/devPlan/design/files-git.md`『Diff 核心』
- 现象：Diff 工作区侧固定为 `Entity<DocumentTextView>`，`build_source_content` 先要求 `session.text()` 返回 Rope，再只接受 `PanelContent::Editor`；超过 50,000,000 字节的 Paged Document 会直接返回 `MissingPanel`，10,000,001–50,000,000 字节的工作区仍以完整可编辑 Rope 参与差分，没有进入分页只读侧。`DiffView::schedule_diff` 也不区分 10 MiB 阈值，对整侧文本执行转换和差分。
- 根因：Diff 复合视图没有接入 PagedTextView／共享页缓存，大小路由沿用了编辑器阈值而不是 Diff 规格；旧的 `diff_tab` 实现没有生产消费点。
- 影响：规格要求的“任一侧大于 10 MiB 时该侧分页只读”未实现；大文件 Diff 的内存和计算没有降级出口，超过 50 MB 时只能打开失败。与 `docs/featureSpec/panels/diff.md` 的大文件表和设计中的“Diff 任一侧大于 10 MiB 时该侧分页只读”不一致。
- 证据：`DiffView` 字段 `right: Entity<DocumentTextView>`；`open_diff` 对 `build_source_content` 结果做 `PanelContent::Editor` 解构，其他形态返回 `MissingPanel`；`build_source_content` 用 `session.text().is_some()` 过滤 Paged；`diff_tab` 模块没有生产消费点。
- 优化方向：按每侧真实字节数选择 PagedTextSource 只读视图，把差分输入限制在可分页快照上，避免对超限侧建立完整行对。

### 重要｜Markdown 渲染态在共享编辑时每键全量解析，未按可见性合并或取消

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs::MarkdownView::apply_edit_snapshot`、`MarkdownView::schedule_parse`；`slterm_app/src/app_shell/app_root/documents.rs::AppRoot::apply_document_edit`；`docs/featureSpec/panels/markdown.md`『两态与共享 Document』；`docs/featureSpec/cross-cutting.md`『后台与可见性』
- 现象：渲染态 Markdown 面板（包括其他项目中的隐藏面板）在共享 Document 每次编辑时都会 `adopt_snapshot`，随后 `schedule_parse` 把完整 Rope 转成 `String` 并新建一次全量 `MarkdownDocument::parse` 后台任务；`parse_task` 被替换但运行中的同步解析无法中止，也没有去抖或可见性判断。
- 根因：编辑广播没有只面向可见视图；解析门控只看形态和大小，不看面板可见性；每个 revision 都独立启动解析任务，没有单飞行或取消协议。
- 影响：5 MB（显式授权后上限 50 MB）文档按每个按键复制全文并启动全文解析，连续输入会堆积后台任务并占满共享 background executor，隐藏 Markdown 面板持续消耗 CPU；与“源码编辑时不做不可见渲染”和隐藏暂停界面工作的约定不一致。
- 证据：`schedule_parse` 中 `self.text.to_string()` 与 `background_executor().spawn(MarkdownDocument::parse)`；`apply_document_edit` 对全部项目广播；`parse_task` 只保存最新 `Task`，没有取消信号；现有 Markdown 共享正文用例只验证文本一致，不验证隐藏解析与任务合并。
- 优化方向：按可见性和 revision 去抖解析，复用不可变正文快照，单人飞行并在新 revision 到达时取消旧解析。

### 重要｜Markdown 滚动调度每次复制全文并清空全部已渲染结果

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs::MarkdownView::new` 的滚动回调 → `MarkdownView::schedule_blocks`；`slterm_app/src/app_shell/markdown/progressive.rs::ProgressiveMarkdownRenderer::schedule`
- 现象：列表每次滚动事件都调用 `schedule_blocks`，其中执行 `self.text.to_string()` 生成全文，再交给 `ProgressiveMarkdownRenderer::schedule`；后者推进代次、清空 `pending`、`inflight` 和 `outcomes`，然后把可见块源码再次复制为任务。已在视口外的旧渲染结果随之失效。
- 根因：滚动驱动的是“全文重新调度”，不是可见区间增量；调度器把完成结果存储和视口调度绑定在同一次清空里。
- 影响：大文档滚动时每个滚轮步进都在 GPUI 前台复制全文；连续滚动会让已渲染块反复重建并丢弃成果，形成渲染饥饿和前台抖动。列表需要重建可见项时，`render_document` 会克隆完整 `MarkdownBlock`（包括代码块源码和行内运行），`render_block` 对代码块再次复制源码并重建整块 `StyledText`，成本随块大小放大。
- 证据：`schedule_blocks` 的 `&self.text.to_string()`；`ProgressiveMarkdownRenderer::schedule` 无条件清空待调度、在途与完成结果集合并递增 `render_generation`；`render_document` 的 `view.renderer.outcome(...).cloned()` 与 `render_block` 的 `code.source().to_owned()`；现有 `progressive_renderer_scopes_viewport_and_rejects_stale_results` 只验证任务范围和代次，不验证全文复制或缓存保留。GPUI 列表对已测量项有高度缓存，但这次查找未取得可直接引用的项目内证据，因此“可见项重建”频率仍需实机确认。
- 优化方向：持有共享源码字符串／Rope，只对滚动窗口的增量块调度；已完成的块按 revision 保留，滚动时不重置整批代次。

### 重要｜Markdown 查找为前台 O(匹配数×块数) 且匹配结果无上限

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs::MarkdownDocument::find_literal`；`slterm_app/src/app_shell/markdown/view/search.rs::MarkdownView::refresh_search`；`docs/featureSpec/panels/markdown.md`『大纲、查找与阅读位置』；`docs/featureSpec/cross-cutting.md`『外观与资源』
- 现象：每次输入变化都在 GPUI 前台调用 `find_literal`，它对 `visible_text` 做 `match_indices`，再对每个匹配线性遍历 `blocks` 找所属块，并把全部匹配收集到 `Vec`；没有结果上限、后台执行或取消。
- 根因：按匹配逐个线性查找块，没有利用可见文本区间或块边界索引；搜索在视图回调内同步完成。
- 影响：大文档和常见短查询会让前台时间随匹配数×块数放大，可能长时间冻结输入；匹配结果数量也没有内存上界。
- 证据：`find_literal` 的 `self.visible_text.match_indices(query)` 与内层 `self.blocks.iter().find(...)`；`refresh_search` 由输入事件同步调用；现有测试只覆盖小语料下的匹配正确性。
- 优化方向：为块可见区间建立索引或二分查找，限制返回数量，把查找移入可取消的后台任务。

### 重要｜终端回滚搜索在 PTY worker 内同步全量扫描，并被每帧重发

- 置信度：确认
- 位置：`slterm_terminal/src/term/search.rs::Term::search_literal`；`slterm_terminal/src/event_loop/mod.rs::EventLoop::drain_recv_channel` 的 `Msg::SearchLiteral` 分支；`slterm_app/src/gpui_terminal/view.rs::TerminalView::process_event` 的 `TermEvent::Frame` 分支与 `slterm_app/src/gpui_terminal/view/search.rs::TerminalView::refresh_terminal_search`；`docs/featureSpec/panels/terminal.md`『只读回滚搜索』
- 现象：`search_literal` 从顶部到当前画面底部遍历全部网格单元并执行 DFA；该调用在 worker 的 `drain_recv_channel` 内同步完成，期间不读取 PTY、不处理输入。搜索视图打开时，每个新帧都会再次调用 `refresh_terminal_search`，每次输入也立即发送新的 `SearchLiteral`；命令队列只有固定容量，旧搜索无法取消，满载时输入和 resize 的 `try_send` 返回 `QueueFull`，调用方只记录日志。
- 根因：搜索作为阻塞命令塞进解析 worker，没有独立取消令牌或“只保留最新查询”的替换协议；新帧驱动搜索刷新，形成输出与搜索互锁。
- 影响：搜索扫描期间 ConPTY 输出队列填满并对子进程施加背压，实际改变了前台程序状态，违反“搜索只读、不改变前台程序状态”；输入可能因 `QueueFull` 丢失；输出活跃时搜索代次不断失效，扫描仍重复执行。
- 证据：`search_literal` 遍历 `topmost_line..bottommost_line`；`drain_recv_channel` 在通道循环内直接调用；`TerminalView::process_event` 每帧调用 `refresh_terminal_search`；`Registry::send` 使用 `try_send`，失败仅由调用方 `log::warn!`；现有 `terminal_search_is_read_only`／`integration_terminal_search` 只验证匹配与不写 PTY，不覆盖长回滚和队列满。
- 优化方向：把搜索放入独立可取消通道或快照工作器，只保留最新查询；不要在每帧重复发起全量扫描；队列满必须成为用户可见状态而不是静默丢输入。

### 重要｜终端搜索匹配无上限，绘制每帧克隆并遍历全部匹配

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/search.rs::TerminalSearchState` 的 `matches`；`slterm_terminal/src/term/search.rs::Term::search_literal`；`slterm_app/src/gpui_terminal/element.rs::TerminalElement` 的 `Element::paint` 与 `terminal_search_runs`
- 现象：搜索返回当前画面与全部回滚的所有匹配 `Vec<Match>`；视图保存整个结果；每次绘制先 `search.matches.clone()`，再逐匹配调用 `terminal_search_runs` 生成运行区间，未命中当前视口的匹配也被遍历。
- 根因：搜索 API 只提供全量 `Vec`，视图没有可见窗口计数、分页或懒加载接口；绘制路径直接复制结果集。
- 影响：回滚达到配置上限、查询为常见单字符时，匹配量可随网格单元数线性膨胀，造成搜索内存和每帧克隆／遍历开销；搜索持续打开会拖慢所有重绘。
- 证据：`search_literal` 的 `RegexIter::collect`；`TerminalElement::paint` 的 `search.matches.clone()` 和 `for (index, matched) in search_matches.iter().enumerate()`；`terminal_search_runs` 对每个匹配单独构造 `Vec<CellRun>`；无任何容量断言或上限测试。
- 优化方向：返回总数与按视口查询的游标，限制单次结果；绘制只消费可见范围的匹配，避免整表克隆。

### 重要｜Runtime 终端与 Git 分页读取每页重建全量结果

- 置信度：确认
- 位置：`slterm_app/src/local_control/service.rs::LocalControlService::start_with` 的 `on_read`／`on_git_diff`；`slterm_app/src/local_control/server.rs::respond_read`、`respond_git_diff`；`slterm_app/src/gpui_terminal/view.rs::TerminalView::runtime_text_snapshot`；`slterm_terminal/src/term/text_snapshot.rs::Term::scrollback_text`；`slterm_app/src/local_control/git.rs::git_diff_snapshot`；`docs/devPlan/design/runtime.md`『关闭与读取安全』
- 现象：`terminal.read`／`agent.read` 的每个分页请求（包括携带游标的续页）都重新向 worker 发 `ReadText`，worker 在终端锁内执行 `scrollback_text` 生成完整回滚字符串，然后才由 `read_page` 切出 384 KiB。`git.diff` 的每个分页请求都重新读取 HEAD、工作区文件并执行 `compute_diff`／`render_diff`，而不是复用上一页的不可变结果。
- 根因：分页游标只绑定 revision，没有绑定可复用的不可变快照缓存；快照在每次请求时重新物化。
- 影响：读取成本随总正文×页数增长；终端 worker 在每页请求时长时间持锁，期间 PTY 解析和输入停顿；Git diff 每页重新启动子进程和差分，连接线程耗时可达硬预算量级，续页容易超时。与“有界读取”和“不可变缓冲快照＋分页游标”的设计意图不一致。
- 证据：`on_read` 每次调用 `try_send_runtime_read` 后等待 `worker_receipt`；`respond_read` 在 `read_page` 前先调用 `(runtime.read)(target)`；`respond_git_diff` 在切片前先调用 `(runtime.git_diff)`；`git_diff_snapshot` 重新执行 `head_file`、工作区读取和 `compute_diff`；`server_tests/runtime.rs::runtime_terminal_read_pages_snapshot_and_rejects_stale_cursor` 使用合成快照，未覆盖每页重物化。
- 优化方向：以资源代次＋revision 为键缓存不可变快照，游标直接在快照上切片；Git diff 结果按 revision 缓存或流式生成页面，禁止每页重算子进程与差分。

### 重要｜Explorer 目录无分批与上限，树行全量构建和渲染

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/explorer.rs::read_directory`、`render_explorer`；`slterm_sidebar/src/file_tree.rs::FileTreeModel::apply_batch`、`FileTreeModel::sort_children`、`FileTreeModel::visible_rows`；`docs/featureSpec/sidebar/explorer.md`『树根与加载』
- 现象：`read_directory` 对目录一次性读取全部条目，不设单目录上限；`apply_batch` 为全部条目建节点；`sort_children` 在每次比较中对两侧名称执行 `to_lowercase()`；`visible_rows` 每次调用克隆全部可见行；`render_explorer` 在每次渲染中为全部行创建元素、`format!` 选择器和标签，未使用虚拟列表。
- 根因：Explorer 的目录读取和行投影没有采用 `side_panel::enumerate` 已有的分批／行数上限，也没有接入可见区域渲染；排序键在比较器内临时分配。
- 影响：包含大量文件的目录（典型如依赖目录）会造成内存、排序分配和整树元素构建；滚动或刷新时前台开销随目录大小增长。与“目录按需分批读取”“大目录采用分批与可见行渲染”不一致。
- 证据：`read_directory` 的 `for entry in std::fs::read_dir(...)` 没有上限；`sort_children` 的 `left_node.name.to_lowercase().cmp(&right_node.name.to_lowercase())`；`render_explorer` 的 `let rows = runtime.tree.visible_rows()` 后逐行 `list.child(row_element)`；现有 Explorer 用例均为小目录。
- 优化方向：复用单目录条目上限与分批读取协议，提前计算小写排序键，按可见区间虚拟化行元素。

### 重要｜Editor／Paged 行布局缓存没有容量上限

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/layout.rs::LayoutCache::activate/get/insert`；`slterm_app/src/app_shell/text_view/editor.rs::DocumentTextView::text_canvas`；`slterm_app/src/app_shell/text_view/paged.rs::PagedTextView` 的 `text_canvas`；`.claude/docs/testing/exemptions.md`『目标态设计声明的人工验收面』中的超大文件滚动条目
- 现象：`LayoutCache` 是只插入不淘汰的 `HashMap<usize, ShapedLine>`；只有布局键变化时才 `clear`。布局键包含横向首列、主题、字体、DPI、语言 revision，但不包含垂直滚动行；因此纵向滚过新行时，每个新行的 `ShapedLine` 会永久驻留，视图销毁前不回收。
- 根因：缓存按“全文行历史”而非“视口窗口”设计，没有 LRU／按行窗口上限。
- 影响：大文件滚动、多个视图同时打开时内存随已经过的逻辑行数增长；50 MB 可编辑文档可积累大量带文本和字形布局的行缓存。与跨域约定中的虚拟化／只读降级目标不一致。
- 证据：`LayoutCache::insert` 无淘汰；`activate` 只在 `TextLayoutKey` 不等时清空；`TextLayoutKey` 无 `scroll_line` 维度；`editor_tests.rs::layout_cache_reuses_lines_and_invalidates_text_and_theme` 只验证复用和失效，不验证容量上限；豁免表把超大文件滚动标为“目标态待实现登记”。
- 优化方向：布局缓存改为围绕视口的固定窗口或字节上限 LRU，滚动时释放远离视口的行布局。

### 一般｜PTY 创建在 GPUI 前台同步执行，并经过全局 spawn 锁

- 置信度：疑似（调用路径确认；真实停顿未测）
- 位置：`slterm_app/src/gpui_terminal/view/startup.rs::TerminalView::launch`、`TerminalView::restore_launch`、`TerminalView::prepare`；`slterm_app/src/gpui_terminal/session.rs::spawn_local`；`slterm_terminal/src/tty/windows/mod.rs::new`；`slterm_terminal/src/tty/windows/spawn.rs::spawn_locked`；`slterm_terminal/src/tty/windows/conpty.rs::ensure_extracted`；`.claude/docs/architecture/overview.md` 的 GPUI 前台上限
- 现象：新建终端的命令路径在 GPUI 更新内调用 `TerminalView::launch`，随后同步执行 `spawn_local` → `tty::new` → `spawn_locked`；`spawn_locked` 持有进程级 `SPAWN_LOCK` 完成 `CreatePseudoConsole` 与 `CreateProcessW`。首次 bundled ConPTY 路径还会在 `ensure_extracted` 中同步建目录并写 `conpty.dll`／`OpenConsole.exe`。恢复可见终端也走同一同步启动链。
- 根因：终端启动工厂是同步 API，没有把创建与加载移交 background executor；能力探测与提取只在启动前做了一次 shell 选择，未覆盖 ConPTY 资源提取。
- 影响：首次创建终端、批量恢复或首次运行时的前台会停顿在进程创建与文件写入上；`SPAWN_LOCK` 会让并发创建串行化，停顿叠加。缺的证据是真实机器上的创建耗时与首启提取耗时。
- 证据：`TerminalView::launch` 的 `Self::prepare(..., true, ...)` 直接返回启动结果；`spawn_local` 同步调用 `spawn`／`spawn_reconnect`；`spawn_locked` 的 `static SPAWN_LOCK` 包住 `conpty::new`；`conpty::new` 在 Win10 捆绑分支调用 `ensure_extracted`。`TerminalView` 的启动链没有任何 background executor 包裹。
- 优化方向：把 PTY 创建与首次资源提取放到后台执行体，完成后再回前台挂载视图；保持创建容量与代次的原子预检。

### 一般｜终端帧快照按读块重建，逐格分配文本，选区全文随帧重算，隐藏会话也执行

- 置信度：确认
- 位置：`slterm_terminal/src/render.rs::RenderSnapshot::capture`、`TerminalFrame::capture`；`slterm_terminal/src/event_loop/mod.rs::EventLoop::publish_frame`、`EventLoop::pty_read_until`；`slterm_app/src/gpui_terminal/view.rs::TerminalView::set_output_visible`
- 现象：每次 PTY 读块处理完后都会捕获完整可视帧；每个非空格单元都新建一个 `String`，`TerminalFrame::capture` 还会无条件调用 `selection_to_string` 复制整个选区。帧在事件信箱里可被合并为最新，但捕获成本已经发生；隐藏面板只阻止 `cx.notify()`，不阻止捕获和发布。
- 根因：帧发布与读取块绑定，没有“捕获前先合并／按需捕获”；单元格文本使用独立 `String`；选区文本不是懒计算。
- 影响：输出洪峰下每个读块都产生大量分配和完整帧拷贝；隐藏项目或面板仍承担同样的 worker 成本；大选区会让每帧成本再乘上选区大小。
- 证据：`RenderSnapshot::capture` 的 `let mut text = String::new(); text.push(c)`；`TerminalFrame::capture` 的 `selection_text: term.selection_to_string()`；`pty_read_until` 每次处理后 `publish_frame`；`set_output_visible` 只改视图标志；`terminal_hidden_output_backpressure` 只验证隐藏后显示最新帧，不验证捕获成本。
- 优化方向：把帧捕获推迟到消费者真正取帧时，按 damage／revision 去重；用单元格共享文本或线段表示，选区文本按需生成。

### 一般｜终端绘制逐格克隆与塑形，搜索高亮每帧全量处理

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/element.rs::TerminalElement` 的 `Element::paint`；`terminal_search_runs`；`slterm_app/src/gpui_terminal/view/search.rs::TerminalSearchState`
- 现象：绘制对每个可见非空格单元执行 `SharedString::from(cell.text.clone())` 和一次 `shape_line`，并逐格克隆字体；搜索高亮在每次绘制前克隆全部匹配、遍历全部匹配并逐匹配构造运行区间。
- 根因：按单元格而非按文本段复用塑形结果；搜索匹配没有视口过滤接口。
- 影响：前台绘制成本随视口格数和搜索结果数增长，搜索打开时尤其明显；有大量搜索匹配时会持续分配和哈希。
- 证据：`paint` 内的 `SharedString::from(cell.text.clone())`、`window.text_system().shape_line(...)`；`search.matches.clone()`；`terminal_search_runs(std::slice::from_ref(matched), ...)`；没有绘制性能预算或缓存断言。
- 优化方向：按连续文本段或行复用塑形结果，绘制只遍历视口内匹配；避免每帧克隆整个匹配集合。

### 一般｜Commit 列表全量构建，未按可见区域渲染

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commit.rs::render_commit`、`CommitRuntime::visible_rows`；`docs/featureSpec/sidebar/commit.md`『列表、过滤与排序』
- 现象：每次渲染先 `visible_rows().into_iter().cloned()` 复制全部行，再为所有分组行构建元素和 `format!` 标签；`visible_rows` 返回的是过滤后的全部行，不是屏幕可见行。状态输出受 Git 读取上限约束，但列表本身仍可达到大量条目。
- 根因：Commit 列表没有虚拟列表或可见窗口切片。
- 影响：大仓库或大量未跟踪文件时，每次重绘都构建整表元素并分配标签，造成滚动与刷新卡顿；与“大列表按可见区域渲染”不一致。
- 证据：`render_commit` 的 `runtime.visible_rows().into_iter().cloned()` 与逐行 `list.child(...)`；`CommitRuntime::visible_rows` 遍历全部 `rows`；现有 Commit 用例使用小仓库。
- 优化方向：按可见区间取行并虚拟化元素构建，缓存行标签和分组计数。

### 一般｜启动恢复同步读取全部恢复文档与快照，无启动预算

- 置信度：疑似（代码路径确认；实际启动耗时未测）
- 位置：`slterm_app/src/app_shell/app_root/side_view.rs::AppRootPersistence::load`、`load_with_previous_run`；`slterm_app/src/persistence/startup/recovery.rs::StartupRecovery::recover`；`slterm_app/src/persistence/document_snapshot/recovery.rs::DocumentSnapshotRecoverer::recover_documents`、`DocumentSnapshotRecoverer::read`；`slterm_app/src/persistence/database.rs::Database::validate_storage`
- 现象：窗口创建前，启动链打开工作区库并执行 `integrity_check` 与 `foreign_key_check`；随后扫描快照目录并解码、校验全部快照正文；对每个布局里的 Document 重新读取磁盘（可编辑上限内读到完整文件，超限时 `PagedDocument::open` 建立完整页索引）。这些都发生在 `application.run` 和首窗之前。
- 根因：恢复流程以完整性和正文对账为先，缺少按大小／时间预算的分级或后台化。
- 影响：恢复大量打开的大文件或大量快照时，首窗出现前会阻塞在磁盘读取、哈希和解码上；当前没有实测启动预算或降级策略。
- 证据：`run_shell_once` 在 `application.run` 前调用 `AppRootPersistence::load_with_previous_run`；`load` 内同步执行 `StartupRecovery::recover`；`recover_documents` 调用 `scan_for_recovery` 和 `self.read`；`read` 对 `EDITABLE_LIMIT_BYTES` 内文件 `read_to_end`；`Database::open` 路径调用 `validate_storage`。缺的证据是大工作区的实际启动耗时和读取字节量。
- 优化方向：先恢复元数据并开窗，正文／快照校验放到后台可取消任务；为启动读取设立契约预算和超限降级。

### 一般｜文件监听脏集合没有上限

- 置信度：疑似（事件规模未测）
- 位置：`slterm_app/src/app_shell/fs_watch.rs::WatchState` 的 `dirty_files`／`dirty_dirs`／`dirty_renames`、`classify_event`、`FsWatchHandle::take_pending`；`docs/featureSpec/cross-cutting.md`『后台与可见性』
- 现象：notify 回调对每个事件路径直接插入脏集合，直到前台 `take_pending` 才整体清空；没有按根合并、数量上限或丢弃策略。后台线程只负责分类，溢出没有背压到前台。
- 根因：监听事件与前台消费之间只靠无界集合累积。
- 影响：大仓库 checkout、批量删除或编译产生的事件风暴在界面繁忙时会持续占用内存；没有可见的丢弃或合并策略。缺的证据是真实树规模与事件量。
- 证据：`classify_event` 的 `dirty_files.insert`／`dirty_dirs.insert`／`record_rename`；`take_pending` 只 `drain`；现有 `fs_watch_tests` 覆盖根、文件和关闭语义，不覆盖事件数量上限。
- 优化方向：按根维护合并后的脏标记和上限，超限时退化为“整根复核”而不是继续累积路径。

### 一般｜性能契约测试只做关键词扫描，且有作用域盲区

- 置信度：确认
- 位置：`slterm_app/src/acceptance.rs::performance_claims_have_conditions`；`docs/featureSpec/cross-cutting.md`『外观与资源』；`docs/featureSpec/acceptance.md`『验收规则』；`docs/devPlan/traceability.md` 的性能数值条目
- 现象：该测试只检查行内是否出现测量单位和条件词；`实测`、`约` 等词即可放行，任意日期一旦出现，后续行都被视为有日期锚。它扫描 `.claude/docs`、`docs/devPlan`、`release-notes`、`CONTEXT.md`、`README.md`，不扫描 `docs/featureSpec`。不校验硬件、Windows 版本、数据规模、测量方法、预热或统计口径。
- 根因：用文本启发式代替结构化性能声明校验；扫描范围没有覆盖行为真值的 featureSpec。
- 影响：`docs/devPlan/traceability.md` 把该能力标为已自动化，但性能声明可以在没有条件的情况下通过；本次审查发现的多个性能路径也没有对应测量门禁。
- 证据：`CONDITION_WORDS` 包含 `实测`、`约`、`不得`；`dated_section` 在文件内一旦置位不再复位；`scan_tree` 的目录列表缺少 `docs/featureSpec`；测试只断言 `violations.is_empty()`。
- 优化方向：对性能声明要求结构化字段或显式契约标记，扫描全部所有者文档，并拒绝无环境条件的时延／吞吐承诺。

### 一般｜GitShow 固定版本把整对象读入内存，并存在未写入契约的上限

- 置信度：确认
- 位置：`slterm_app/src/git/show.rs::GitFacade::show`、`SHOW_OUTPUT_LIMIT`、`GitShowSnapshot::bytes`、`classify_content`；`docs/featureSpec/panels/gitshow.md`『大文件与二进制』；`docs/devPlan/design/files-git.md`『GitShow 固定版本』
- 现象：`show` 通过一次 `cat-file blob` 把最多 `200_000_000 + 64 KiB` 的输出完整读入 `Vec`，再转为 `Arc<[u8]>` 常驻快照；超过该上限直接返回 `OutputLimit`，而不是按规格分页。超过 10 MB 只进入 `PagedSource`，但源字节仍全量保留。
- 根因：分页索引建立在完整对象内存上，没有流式读取或外部页存储；上限值不在行为规格中登记为契约值。
- 影响：打开很大的固定版本时产生 200 MB 级常驻内存，多个视图叠加更明显；超过阈值时是失败而不是降级分页。与规格“大于 10 MB 分页简化只读”之间缺少明确上限语义。
- 证据：`SHOW_OUTPUT_LIMIT` 与 `run_repository_with_limit(cat_file, SHOW_OUTPUT_LIMIT)`；`GitShowSnapshot { bytes: Arc<[u8]>, ... }`；`classify_content` 对 `bytes.len() > PAGED_CONTENT_BYTES` 才建页；featureSpec 表没有 200 MB 上限。
- 优化方向：流式建立页索引并按页持有，或明确登记内存上限为契约值并给出超限降级。

### 提示｜Runtime 页面预算按原始字节计算，JSON 转义可能突破响应上限

- 置信度：确认
- 位置：`slterm_app/src/local_control/reads.rs::PAGE_BUDGET_BYTES`、`read_page`；`slterm_app/src/local_control/server.rs::write_json`、`RESPONSE_BUDGET`；`slterm_app/src/local_control/read_page_tests.rs::reads_are_budget_bounded_and_never_split_utf8`
- 现象：分页预算按原始 UTF-8 字节取 384 KiB，注释假设 JSON 转义最坏翻倍；但合法文件文本中的 C0 控制字符（如 U+0001）在 JSON 中会变成 `\u00XX`，单字节可膨胀为六字节。`write_json` 在编码后检查 1 MiB，超限返回 IO 错误并断开连接。
- 根因：分页预算与响应编码预算分离，没有按转义后长度或字符类别计算。
- 影响：含大量控制字符的 Git diff 或终端文本页面可能触发响应超限，客户端得不到结构化错误而是连接失败；`read_page` 的“低于 1 MiB”论证不成立。
- 证据：`PAGE_BUDGET_BYTES` 的注释只考虑引号、反斜杠和换行；`write_json` 的 `serde_json::to_vec` 后 `encoded.len() > RESPONSE_BUDGET`；测试只覆盖原始字节切分与 UTF-8 边界，不覆盖 JSON 编码长度。
- 优化方向：按编码后长度计算分页预算或先转义再切分，并把超限映射为结构化读取错误。

### 提示｜Git status 与仓库根缓存持全局锁跨越子进程

- 置信度：确认
- 位置：`slterm_app/src/git/mod.rs::GitFacade::status`、`status_cached_with`、`repository_root_cached_with`、`store_repository_root`
- 现象：`GitFacade::status` 持有状态缓存互斥锁，同时执行可能持续 3 秒的 `git status` 子进程；`status_cached_with` 与 `repository_root_cached_with` 同样在全局 mutex 内调用读取函数。除 status 缓存外，仓库根缓存是所有项目共享的单一 map。
- 根因：缓存锁与 I/O 临界区没有分离。
- 影响：多个项目、Explorer、Commit、gutter 与 Runtime 的 Git 读取会相互等待；一次慢仓库读取会阻塞其他后台读取线程，放大后台任务延迟。
- 证据：`let mut cache = ...lock()` 后调用 `super::status`／`reader`；`STATUS_CACHE_TTL` 与 `REPOSITORY_ROOT_CACHE_TTL` 只有 500 毫秒，但锁持有时间由子进程决定；没有单飞行或按 root 分段锁。
- 优化方向：锁外执行 I/O，锁内只做命中检查与结果发布；按 root 或键拆分在途请求。

### 提示｜`file_hunks` 无超时且无界捕获 stdout，违反 Git 有界输出合同

- 置信度：确认
- 位置：`slterm_app/src/git/mod.rs::file_hunks`；调用方 `slterm_app/src/app_shell/file_editor/gutter.rs::read_for_path`；`docs/devPlan/design/files-git.md`『GitFacade 与状态事实』
- 现象：`file_hunks` 使用 `Command::output()` 执行 `git diff --unified=0 HEAD -- <path>`，没有超时，也没有 stdout 上限；解析只使用 hunk 头，但完整差异正文会全部读入内存。`read_for_path` 在生产路径没有调用者，只有测试引用。
- 根因：该历史路径没有走 `run_repository` 的 `run_cancellable` 有界双通道。
- 影响：当前是潜伏路径；一旦接回 gutter／编辑器行标，大文件差异可能造成大量内存和长时间后台阻塞，违反“所有调用有界 stdout／stderr”的合同。
- 证据：`file_hunks` 的 `.output()` 与 `parse_hunks(&String::from_utf8_lossy(&output.stdout))`；`read_for_path` 的引用只在 `gutter_tests`；`run_repository` 已存在带 3 秒和 1 MiB 预算的入口未被使用。
- 优化方向：只经 `run_repository`／流式读取 hunk 头，限制输出和时间，删除或隔离未接线路径。

### 提示｜resize 命令队列满只记录日志，最终网格／PTY 尺寸可能丢失

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/layout.rs::TerminalView::resize_grid_only`、`commit_viewport`；`slterm_app/src/gpui_terminal/registry.rs::TerminalRegistry::send`；`docs/devPlan/design/terminal.md`『TerminalSession 与 PTY 代次』
- 现象：布局变化通过 `try_send` 提交 `Msg::ResizeGrid` 或 `Msg::Resize`；队列满时只 `log::warn!`，没有保留最新视口或重试。拖拽风暴下 worker 可能正在处理长搜索／读取，最终尺寸命令被丢弃。
- 根因：resize 的“最新值覆盖”合并只发生在 worker 已收到的消息上，入队失败没有应用层待提交状态。
- 影响：窗口／分屏最终尺寸可能没有到达网格或 PTY，界面按新视口裁剪而输入坐标基于旧几何，直到下一次布局变化；与 resize 水位设计的确定性不一致。
- 证据：`resize_grid_only` 与 `commit_viewport` 的错误分支只有日志；`Notifier::on_resize` 的 `try_send` 返回 `QueueFull`；`event_loop/tests.rs::notifier_reports_input_and_resize_backpressure` 只验证错误返回，不验证 UI 侧恢复。
- 优化方向：前台保留最新待提交视口，队列有空位或下一帧时重发；对 resize 使用覆盖而非丢失语义。

## 跨域观察

- **阻断候选（合同冲突，数据丢失）｜崩溃快照写入未接线**：生产启动与 AppRoot 从未构造 `DocumentSnapshotWriter`／`DocumentSnapshotClient`；grep 只能找到测试里的 `DocumentSnapshotWriter::start*` 与 `DocumentSnapshotStore::write`。`run_shell_once` 的启动链只接入恢复器 `DocumentSnapshotRecoverer`，编辑路径 `AppRoot::apply_document_edit` 只做自动保存和视图广播，没有快照入队。`docs/featureSpec/settings-and-persistence.md`『Document 崩溃快照』要求每个脏 Document 只留最新快照、配额与删除时机；`docs/devPlan/plan.md` 的『崩溃快照写入与配额』也在交付范围内；但 `docs/devPlan/progress.md` 已把 Document 交付标为已验收，`traceability.md` 的 `snapshot_quota_does_not_evict_dirty`／`integration_snapshot_quota` 只直接调用 store／writer，不能证明生产写入。结果是异常退出无法恢复新编辑，验收结论与代码事实不一致。附带性能风险：`DocumentSnapshotStore::write` 每次写都会 `scan_entries` 读取并解码目录内全部快照，若接线会成为编辑去抖热路径上的全量扫描。
- **文档完整性问题**：`.claude/docs/testing/exemptions.md` 的『目标态设计声明的人工验收面』引用 `design/panels/editor.md`、`design/panels/diff.md`、`design/ux-framework.md`、`design/platform-windows.md`、`design/panels/terminal.md`，这些路径在 `docs/devPlan/design/` 下不存在；其“超大文件滚动与虚拟化观感”仍标为“目标态待实现登记”，与本次发现的布局缓存无上限、Explorer／Commit 未虚拟化互相印证，说明相关性能面没有可执行的自动化兜底。
- **未接线旧实现**：`slterm_app/src/app_shell/diff_tab.rs` 的 `DiffTabView` 没有生产消费点，`slterm_app/src/app_shell/file_editor/gutter.rs::read_for_path` 只在测试中调用，`slterm_terminal/src/term/redraw_anchor.rs::Term::set_redraw_anchor_enabled` 也只在测试中调用。它们包含大文件／分页／阅读锚点相关逻辑，容易让性能与验收证据指向未运行路径。

## 存疑未证

- 终端帧捕获与逐格绘制的实际分配率和延迟未测；需要真实输出洪峰下的分配计数、帧率与 UI 延迟数据。
- Explorer 大目录的实际耗时和内存未测；需要包含大量文件的真实临时目录、不同展开深度和排序样本。
- Markdown 代码块在 GPUI 列表中的实际复用／重排行为未证实；需要确认 `gpui::list` 是否缓存可见项，否则 `render_document` 的逐帧块克隆和整块 shaping 会放大。
- 启动恢复的实际耗时未测；需要在大工作区（多个大文件、快照目录非空）上采集 `SLTERM_BOOT_TRACE` 与磁盘读取量。
- PTY 创建与首次 bundled ConPTY 提取在真实机器上的前台停顿未测；需要首启和批量恢复的耗时数据。
- 文件监听脏集合在 checkout／批量删除风暴下的实际增长未测；需要真实事件量与内存数据。
- 100,000 行回滚下逐帧 reflow 的实际成本未测；代码注释称客户端 reflow 便宜，但没有测量条件和数据。
- Git 缓存锁在多个项目／多个消费者并发时的等待未测；需要并发状态读取的时序记录。
- 控制字符密集的 Runtime 分页是否实际触发 1 MiB 响应超限未验证；需要构造含控制字符的 Git diff／终端文本并测量编码后长度。
- 大选区下的每帧 `selection_to_string` 成本未测；需要选区覆盖多行回滚时的帧时间数据。
