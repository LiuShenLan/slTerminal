# 应用壳内容域审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），审查范围：Document、Editor、Diff、GitShow、Explorer、Commit、文件操作、watcher、粘贴。

## 覆盖与局限

已覆盖：

- 代码：`slterm_app/src/app_shell/app_root/` 的 `documents`、`document_external`、`diff`、`editor_git`、`git_show`、`git_changes`、`commit`、`explorer`、`agent_files`、`navigation`、`nav`、`projects`、`markdown_links`、`batch_close`、`project_close`、`application_close`、`tabs`、`commands`、`event` 接线；`slterm_app/src/app_shell/` 的 `documents`、`doc_tabs`、`diff_tab`、`document_external`、`document_autosave`、`document_close`、`watcher_service`、`file_operation_service`、`fs_watch`、`paste_service`、`paste_journal`、`file_drop`、`side_view`、`text_view/view_state`；相关 core 侧 `slterm_document` 的注册/保存/外部变化/diff、`slterm_workspace` 的 `collect_document_close` 与快照校验、`slterm_path` 的 `PathPolicy`；`slterm_app/src/git` 的 `show`、`system_open`、`repository`；`slterm_app/src/local_control/git`。`app_root/relocation.rs` 不存在，项目重定位实现位于 `app_root/projects.rs::relocate_project` 与 `tests/relocation.rs`。
- 文档：根 `CLAUDE.md`、`slterm_app/CLAUDE.md`、`app_shell/CLAUDE.md`、`app_root/CLAUDE.md`、`file_editor/CLAUDE.md`、`git/CLAUDE.md`；`docs/devPlan/design/document.md`、`files-git.md`、`workspace.md`；`docs/featureSpec/panels/document.md`、`editor.md`、`diff.md`、`gitshow.md`、`sidebar/explorer.md`、`sidebar/commit.md`、`workspace/navigation.md`、`path-policy.md`、`acceptance.md`；`docs/devPlan/traceability.md` 的 Document／Editor／Diff／GitShow／Explorer／Commit 行；`docs/devPlan/progress.md`；`.claude/docs/testing/exemptions.md`。
- 测试：`app_root/tests/` 的 documents、documents/external、documents/links、documents/open_file、batch_close、commit、explorer、gitshow、diff、relocation、agent_files、local_close、application_close_receipts；`app_shell/document_autosave.rs`、`document_external.rs`、`documents.rs`、`diff_tab.rs`、`doc_tabs.rs`、`code_tab.rs`、`paste_service_tests.rs`、`paste_journal_tests.rs`、`file_operation_service_tests.rs`、`watcher_service_tests.rs`、`fs_watch.rs` 的单元测试；`git/show_tests.rs`、`git/status_tests.rs`、`local_control/git_tests.rs`。

未覆盖／局限：

- 按合同未运行 cargo、测试、构建或网络，未启动真实 GUI；涉及真实 GPUI 焦点、notify 时序、真实回收站/链接的结论只到代码路径与测试内容层面。
- 未逐行审查 DocumentTextView／MarkdownView／PagedTextView／SettingsView 的渲染实现；只在它们与共享 Document、焦点、关闭链交界处取样。
- 未审 JSON-RPC 协议定义与其他审查范围（终端、工作区壳、设置、备份）的内部实现。

## 问题

### 阻断｜交互式关闭没有保存／丢弃／取消决定入口，脏文档无法关闭

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/application_close.rs::start_application_close`；`batch_close.rs::close_group_from_request`／`begin_views_close`；`tabs.rs::tab_list_command_item`；`nav.rs::SideView::nav_remove`；`events.rs::notify_application_close_decision_required`；i18n 键 `close_title`／`save`／`discard`／`cancel`／`save_all_failed_title`／`save_all_failed_detail`。
- 现象：关窗、组内“关闭其他／全部”、导航“移除项目”都只向关闭链提交空的 `document_decisions`。`collect_document_close` 返回 `MissingDecision` 后，组内命令仅在 `tab_list_command_item` 中 `log::warn`；关窗路径只投递一条无动作按钮的 `Notification::ProcessAlert`；导航移除对错误 `let _`。窗口、面板或项目保持原样，且没有任何保存／丢弃／取消对话框。页签栏也没有单页签关闭入口，`close_view_from_request` 只有 Runtime 调用方。
- 根因：交互路径只实现了统一关闭协调器的服务端，没有实现 featureSpec 要求的文档决定收集 UI 与命令；带决定关闭只存在于 Runtime／测试直调。
- 影响：有未保存正文时，用户无法关窗、无法关闭文档页签、无法移除项目，也没有丢弃通道；只有开启自动保存或手工逐文件保存后才能退出。即使文档干净，也没有单页签关闭入口，只能借“关闭其他／全部”整组处理。Document「关闭最后视图才提供保存／丢弃／取消」与工作区壳「批量保存失败取消关闭并保留 Panel」的用户验收不成立。i18n 中这些关闭文案无源码引用，进一步证明交互面缺失。
- 证据：`start_application_close` 调用 `start_application_close_with_decisions(Default::default(), true, …)`；`tab_list_command_item` 构造 `CommandParameters::CloseGroup { document_decisions: Default::default() }` 并只记录日志；`nav_remove` 丢弃 `handle_nav_remove` 结果；`notify_application_close_decision_required` 使用 `Notification::ProcessAlert`（无 action）；`close_title`／`save`／`discard`／`cancel`／`save_all_failed_*` 在源码中零引用。
- 优化方向：交互路径必须与 Runtime 共用同一 `CloseCoordinator`，但拥有自己的决定收集、提交、失败展示与重试入口；页签、项目行、关窗入口都要暴露失败而不是静默保持。

### 阻断｜Commit 永久删除确认把任意失败升级为绕过文件操作执行体的强删

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commit.rs::finish_commit_delete`、`commit_force_delete_row`、`commit_request_delete`；`git_changes.rs::delete_change` 的 `permanent` 分支。
- 现象：删除失败后 `retry_permanent = result.is_err() && !permanent`，因此 `Precondition`、`Unauthorized`、`Failed` 等所有错误都被呈现为“回收站不可用，按 Delete 永久删除”。用户在行上再次按 Delete 后，`delete_change(permanent=true)` 只做状态形态与 `target_path.starts_with(write_root)` 的纯词法检查，然后 `std::fs::remove_file(target_path)`；非永久分支则经过 `plan_delete`／`execute` 的真实身份、父目录、链接与授权复核。
- 根因：`GitChangeError` 的失败原因在 UI 层被压成布尔值；永久删除分支没有复用 `FileOperationService`，也没有同等的重解析与授权步骤。
- 影响：链接或父目录在预检后重定向、写授权已失效、目标内容在失败后被替换时，第一次安全路径会拒绝，第二次“确认”却能强删实际目标，属于数据丢失与文件操作授权红线违例。
- 证据：`finish_commit_delete` 的 `retry_permanent` 判定不区分错误种类；`delete_change` 的永久分支直接 `std::fs::remove_file`，而同一函数的回收站分支调用 `file_operations.plan_delete`／`execute`；`commit_untracked_permanent_delete_requires_confirmation` 只注入回收站失败，未覆盖 `Precondition`／`Unauthorized` 升级路径。
- 优化方向：失败按种类分流并保留原因；永久删除也必须经过统一计划/执行体或等价的身份、父目录与链接重授权；不得用同一次失败自动进入强删确认。

### 阻断｜未暂存删除回滚会删除内容相同的任意未跟踪文件

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/git_changes.rs::rollback_unstaged` 的 `mirror` 查找与删除分支。
- 现象：回滚一个未暂存删除时，代码遍历整个仓库状态中的全部未跟踪条目，取第一个字节与索引 blob 完全相同的路径并 `std::fs::remove_file(&mirror)`。它不要求该路径位于同一写根、同一项目根、同一父目录，也不确认它确为被删除文件的重命名目标。
- 根因：用“内容相等”替代重命名事实；没有把新路径约束到已登记的改名别名或同一目录，也没有复用 `FileOperationService` 的授权模型。
- 影响：仓库内（包括项目根外）任何未跟踪的同内容文件可能被误删；正常删除回滚也会触发该搜索，而不仅是未暂存重命名。`commit_unstaged_rename_rollback` 只覆盖了唯一镜像的理想场景。
- 证据：`snapshot.entries.iter().filter(index == '?').find_map(...)` 覆盖全部未跟踪条目；唯一前置授权检查针对 `target`（旧路径），不覆盖 `mirror`；随后直接 `remove_file`。
- 优化方向：只允许删除能由同一根内事实确认的新路径（同目录/已观测改名目标），否则只恢复旧路径并报告部分成功；删除前走统一写授权与身份复核。

### 阻断｜文件树 watcher 未接线，Explorer／Commit／AgentFiles 不会自动刷新

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/document_external.rs::reconcile_fs_watch`、`poll_fs_watch`；`slterm_app/src/app_shell/watcher_service.rs::WatcherService`；`fs_watch.rs::FsWatchHandle::retain`；`app_shell/CLAUDE.md` 的“文件树 watcher 调度”决策。
- 现象：生产唯一的 `FsWatchHandle` 只登记打开 Document 的精确文件，`retain` 的树根参数恒为空；`poll_fs_watch` 只消费 `files` 与 `renames`，`FsPending::dirs` 从未被读取。`WatcherService` 没有任何生产构造、注册、注册根或 `poll` 调用，只有 `watcher_service_tests.rs`。Explorer／Commit／AgentFiles 的根可见性、200ms 合并、1000ms 上限、隐藏暂停与重新可见合并复核全部停留在未使用类型。
- 根因：目录树监听调度和视图合并回投没有接入 AppRoot 的同一事件泵；`FsWatch` 具备树根能力，但应用层从未登记。
- 影响：磁盘变化不会刷新 Explorer 树、Commit 列表或 Agent 文件树；Commit 也没有手动刷新按钮／F5。featureSpec Explorer「视图可见时监听当前根、200ms 合并」、Commit「文件事件尾去抖并自动刷新」以及验收判据「不可见期间多次事件、重新可见只合并复核一次」不成立；`.claude/docs/testing/exemptions.md` 中“外部改文件后树刷新”的人工兜底也没有对应实现。
- 证据：`reconcile_fs_watch` 调用 `watch.retain(Vec::new(), files)`，没有其他调用点以非空树根调用 `retain`；`WatcherService` 只被其测试引用；`explorer_hidden_refresh_coalesced`、`agent_files_visibility_controls_watchers` 均只测这个未接线的服务。
- 优化方向：把可见 `FileTreeRoot` 的登记／注销与 `WatcherService` 的合并回投接入 AppRoot 事件泵，由 Explorer／Commit／AgentFiles 消费复核结果；未接线前不得以该类型测试或人工兜底声明验收。

### 阻断｜外部冲突与另存为没有生产落点，冲突中的本地修改无法保存

- 置信度：确认
- 位置：`slterm_app/src/app_shell/documents.rs::DocumentSaveService::save_as`、`ExistingTargetPolicy`；`app_shell/app_root/document_external.rs::apply_document_external_result`；`app_shell/app_root/documents.rs::save_document`；`slterm_document/src/diff.rs::DiffInput::conflict`。
- 现象：`save_as` 与 `ExistingTargetPolicy::OverwriteConfirmed` 只在测试中被调用，没有任何命令、Runtime 方法或视图事件；外部冲突只写入 `DocumentRegistry.external_conflict`，没有“覆盖、另存为、取消、重载、保留本地、查看差异”的入口；`DiffInput::conflict` 零调用。冲突后的手动保存固定走 `SaveError::Changed`，自动保存暂停，用户没有保存路径。
- 根因：Document 的冲突与另存为只落地了服务层原语，没有接入命令目录、Runtime 或视图交互；i18n 也没有另存为/覆盖提示键。
- 影响：脏 Document 遇到外部修改后，本地正文不能写回磁盘，也不能另存为；用户只能手工复制文本。featureSpec Document 的保存冲突与另存为验收、PathPolicy 的“根外另存为授权”不成立。`document_save_as_rejects_open_target` 只是服务级测试，动作不经过用户可见入口。
- 证据：`save_as`／`OverwriteConfirmed` 调用点只在 `documents.rs` 的测试与 `tests/`；`apply_document_external_result` 只处理 `Relocated`／`Reloaded`／`ReadRevoked`，冲突分支不产生视图或交互；`DiffInput::conflict` 无生产调用。
- 优化方向：把冲突决定与另存为纳入统一命令/事件链，写前维持“目标已关联则拒绝、未打开目标需确认覆盖”的合同，并把决定结果广播到全部引用视图。

### 重要｜关闭保存授权漏掉 AgentRoot 能力并集

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/batch_close.rs::document_close_grants`；`application_close.rs::start_application_save`；`project_close.rs::start_project_save`；对照 `agent_files.rs::document_grants`。
- 现象：批量、项目与应用关闭保存都只从项目根生成 grants，不叠加 `agent_root_grants_for_path`；而普通 Ctrl+S、自动保存与 Runtime `panel.save` 使用 `document_grants`，包含可写 AgentRoot。文档经可写 AgentRoot 打开时，逐文件保存可行，批量/项目/应用关闭保存却返回 `Unauthorized` 并取消关闭。
- 根因：关闭保存与普通保存各自派生授权，没有复用同一能力并集入口。
- 影响：能力并集在关闭路径失效；用户必须逐文件保存后才能关闭或移除项目，验收「根外只读范围与另一可写根同时覆盖工作区，保存按能力并集处理」不成立。
- 证据：`document_close_grants` 只映射 `PathGrant::project_root`；`document_grants` 额外叠加 `agent_root_grants_for_path`；关闭相关测试只使用项目根授权。
- 优化方向：关闭保存复用 `document_grants` 的同一授权派生，并补 AgentRoot 可写/撤销场景。

### 重要｜Diff 冲突快照缺失

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/diff.rs::open_diff`／`schedule_diff`；`slterm_document/src/diff.rs::DiffInput::conflict`；`text_view/view_state.rs::DiffViewState`。
- 现象：Diff 只支持 `DiffInput::git`；没有磁盘版本与当前缓冲的只读冲突快照，没有“查看差异”触发，没有不可保存/不自动刷新/源销毁清理的生命周期。
- 根因：冲突快照只定义了 core 构造函数，未接入视图、命令或 ViewKey 之外的临时面板类型。
- 影响：featureSpec Diff「冲突横幅的查看差异创建只读快照」与 Document「脏外部修改进入共享冲突，可查看差异」不可用；`diff_conflict_snapshot_is_immutable` 在源码中不存在。
- 证据：`DiffInput::conflict` 零调用；`DiffViewState::new` 强制包含 `DiffBinding`；`document_shared_across_projects`／`integration_git_document` 只注入冲突并断言状态，不验证快照视图。
- 优化方向：为冲突/大文件快照引入独立只读视图与临时生命周期，不进入普通 Diff 去重和持久布局。

### 重要｜Diff 大文件与二进制降级缺失，大文档 Diff 直接打不开

- 置信度：确认
- 位置：`app_shell/app_root/documents.rs::build_source_content`；`app_root/diff.rs::open_diff`；`app_root/commit.rs::open_commit_diff_path`／`flush_pending_commit_diff`。
- 现象：Diff 工作区侧要求 `session.text().is_some()`；分页（超过可编辑阈值）Document 直接返回 `MissingPanel`。Commit 对 modified 大文件调用 `open_diff` 失败后，`open_commit_diff_path`／`flush_pending_commit_diff` 用 `let _ =` 丢弃错误，用户看不到原因。没有按侧大于 10 MiB 的分页只读、NUL 二进制占位、差异不可用错误条。
- 根因：Diff 右侧只实现可编辑 Rope 一侧，未实现快照/分页一侧；打开失败没有回投。
- 影响：featureSpec Diff「任一侧超限或含 NUL 时按侧降级」「差异不可用时先显示正文」与 Git 集成验收不成立。
- 证据：`build_source_content` 的 `filter(|session| session.text().is_some())`；Commit 打开链对 Diff 结果不作处理；`diff_large_and_binary_side_degradation` 不存在。
- 优化方向：按真实字节选择 Rope／只读快照／分页，并让 Diff 打开失败成为可见、可重试的错误。

### 重要｜Diff 展示参数只持久化不消费

- 置信度：确认
- 位置：`app_root/diff.rs::DiffView` 与 `render`；`text_view/view_state.rs::DiffViewState`；测试 `diff_view_state_restore_clears_transients`。
- 现象：DTO 含 `mode`、`sync_scroll`、`column_width`、`collapsed`、`swapped`、`collapse_unchanged`，但渲染始终是 HEAD／工作区两栏加计数与上一/下一按钮；`set_view_state_for_test` 只应用 wrap 与滚动；没有模式切换、交换、折叠、列宽、同步滚动、空白选项控件。`DiffViewState` 上明确标注“完整展示控件消费归后续接线”。
- 根因：展示控件未实现，持久字段只有测试和空白计算在消费。
- 影响：featureSpec Diff 的持久状态验收被 DTO 往返测试替代；实际能力缺失。
- 证据：`render` 没有读取这些字段；`set_view_state_for_test` 只调用 `set_wrap`／选区／滚动；测试断言 `view_state_for_test() == expected`。
- 优化方向：以真实控件消费 DTO，并把测试改为断言可见行为；未实现的字段不应以已自动化登记。

### 重要｜GitShow 错误态无重试，手动刷新缺失，刷新失败丢弃已加载内容

- 置信度：确认
- 位置：`app_root/git_show.rs::GitShowView::apply_result`／`render`／`schedule_gitshow_load`。
- 现象：`GitShowBody::Error` 渲染 `Message::GitshowReadFailed` 且无按钮；没有刷新命令、按钮或快捷键；`apply_result(Err)` 会 `close_body`、把 `snapshot` 置空，若已有成功内容则整页退化为错误；对象不可用只能通过重新打开另一面板恢复。
- 根因：固定版本读取只有初次加载入口，错误落点没有重试动作；失败处理未区分初次读取与刷新。
- 影响：featureSpec GitShow「手动刷新只重读同一固定版本」「对象不可用保留面板与明确错误、可重试」「区段读取失败保留已加载内容」不成立；`gitshow_refresh_reads_same_object`／`gitshow_missing_object_preserves_error` 只是服务层测试。
- 证据：错误分支 `open=false` 不生成重试按钮；`schedule_gitshow_load` 只在 `open_gitshow` 调用；`apply_result` 的 `Err` 分支清空 `snapshot`。
- 优化方向：增加同固定对象的显式刷新与重试；刷新失败保留最后一次成功快照并给可重试错误。

### 重要｜Explorer 双击目录启动终端、单击目录展开、多选仍可打开

- 置信度：确认
- 位置：`app_root/explorer.rs::render_explorer` 的鼠标分支、`explorer_activate_selected`、`explorer_activate_at`。
- 现象：双击目录调用 `explorer_open_terminal_at` 并在后台创建新 PTY；单击目录同时选中并执行展开/收起；`explorer_activate_selected` 不检查 `multi_selection`，多选时 Enter 仍打开文件；双击的第一次点击会把多选清空后再打开。
- 根因：渲染层把“激活”与“在终端打开”混在同一鼠标手势，多选状态未参与打开判定。
- 影响：普通浏览动作产生新终端，违反 featureSpec Explorer「单击单选」「目录 Enter 或双击只切换展开」「多选时 Enter 不打开文件，双击也不因第二次点击意外打开」。
- 证据：上述分支；`explorer_open_terminal_at` 从 `render_explorer` 的双击目录臂调用；打开前没有 multi-selection 检查。
- 优化方向：目录双击只切换展开；打开动作走显式菜单/命令；打开前检查多选与当前行。

### 重要｜Explorer／AgentFiles 的文件操作、粘贴与键盘能力基本缺失，PasteService 无生产入口

- 置信度：确认
- 位置：`app_shell/paste_service.rs`、`paste_journal.rs`、`file_operation_service.rs`；`app_root/explorer.rs`、`agent_files.rs`；`side_view.rs` 的 SideView 键盘分支。
- 现象：`plan_create_file`／`plan_create_directory`／`plan_copy` 没有生产调用；`PasteService` 只在 `paste_service_tests.rs` 中构造（`PasteJournalRecoverer` 虽接入启动，但永远不会产生粘贴记录）；Explorer 渲染没有右键/菜单、新建、删除、复制、剪切、粘贴、复制路径、定位入口；键盘只有 F2／Escape／Enter；没有方向键、Home/End、Ctrl+A、F5、Shift 选区；树行不显示 Git 状态字符与颜色；`show_ignored` 只写持久值，不参与任何过滤；Agent 文件树也没有文件操作入口。
- 根因：文件操作/粘贴服务已实现但未按统一命令目录与视图动作接线；渲染层只实现了最小浏览与重命名。
- 影响：featureSpec Explorer／Agent 文件树的大部分验收只由服务级测试冒充，用户不可达；粘贴冲突聚合、跳过/替换/取消、跨根复制与剪切、回收站确认、定位与树内导航全部不在产品路径上。`explorer_copy_capability_union`／`explorer_rechecks_links_before_mutation`／`explorer_paste_conflict_and_partial_success` 分别只是 `file_operation_service_tests`／`paste_service_tests` 的单元用例。
- 证据：上述调用点与渲染函数；`PasteService::new` 无生产构造；`plan_create_*`／`plan_copy` 无生产调用；`show_ignored` 没有消费者；SideView 的 Explorer 键盘分支仅覆盖 F2／Escape／Enter。
- 优化方向：按统一命令目录接入树操作与粘贴，复用现有服务并把用户可见部分作为验收对象；删除未接线的服务测试对追溯状态的替代。

### 重要｜Commit 破坏操作无确认、脏 Document 守卫静默、Runtime 绕过守卫

- 置信度：确认
- 位置：`app_root/commit.rs::commit_rollback`／`commit_rollback_selected`／`commit_delete_selected`／`start_commit_rollback`／`start_commit_delete`；`local_control/git.rs::execute_git_write`。
- 现象：Ctrl+R 与 Delete 直接派发回滚/删除，没有确认框（完整路径、将丢弃的未暂存变化、默认取消）；目标存在脏 Document 时 `start_commit_rollback`／`start_commit_delete` 直接 `return`，既不弹保存/丢弃/取消，也不给任何提示；Runtime `git.rollback`／`git.delete` 完全不检查脏 Document。
- 根因：确认与 Document 守卫只存在于 featureSpec，面板与 Runtime 都只实现了状态重检；脏 Document 被当成“不可操作”而不是“需要决定”。
- 影响：破坏性操作没有用户确认；脏正文可能被 Runtime 回滚或删除造成的磁盘变化转为外部冲突，甚至与不查看 Doc 的删除叠加。`commit_rollback_document_guard` 断言“静默不写盘”，把弱行为固化成验收。
- 证据：`commit_rollback`／`commit_delete_row` 无二次确认；`is_dirty` 分支只返回；`execute_git_write` 直接调用 `rollback_unstaged`／`delete_change`；无对应 i18n 确认文案。
- 优化方向：统一守卫入口（保存/丢弃/取消）覆盖面板与 Runtime；Runtime 无法交互时必须返回需决定的结构化错误而不是隐式跳过。

### 重要｜Commit／Explorer 数据根切换时在途状态读取被丢弃或错投

- 置信度：确认
- 位置：`app_root/commit.rs::start_commit_status`／`sync_commit_root`／`commit_apply_status`；`app_root/explorer.rs::start_explorer_status`／`finish_explorer_status`。
- 现象：`TaskId::CommitStatus` 在途时，新数据根的 `LoadStatus` 被 `start_commit_status` 直接丢弃；旧回执因 generation 不匹配被 `commit_apply_status` 丢弃且不再重试。新 Commit 运行态已被置 `loading=true`，列表为空，之后没有任何加载触发（也无 F5），永久停留在“加载中”。Explorer 的状态回执只检查项目存在，不检查回执对应的项目与根，旧项目的 `repo_root`／条目数可能写入新根视图。
- 根因：单任务身份没有合并位与重试；回执身份校验缺少项目/根（Commit 有 generation 但没有重投，Explorer 连 generation 都没有）。
- 影响：切换项目后 Commit 可能空列表且永远加载；Explorer 显示上一项目的 Git 汇总。
- 证据：`start_commit_status` 的已登记早返回；`commit_apply_status` 的 generation 早返回；`sync_commit_root` 先设置 loading；`finish_explorer_status` 只 `project(project_id).is_none()` 检查；`explorer_set_status` 直接覆盖。
- 优化方向：为读取请求保留 pending 合并位并在回执后重投；回执同时校验项目、根与 generation；测试覆盖切换期在途读取。

### 重要｜追溯矩阵与测试名不一致，部分验收由服务级或未接线组件测试冒充

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 Document／Editor／Diff／GitShow／Explorer／Commit 行及对应测试文件。
- 现象：以下目标例名在源码中不存在：`document_discard_respects_external_reference`、`ordinary_open_does_not_deduplicate_diff_or_gitshow`、`document_rename_preserves_identity_and_view`、`explorer_rejects_cross_root_cut_and_descendant_paste`、`commit_rollback_preserves_staged_changes`、`diff_conflict_snapshot_is_immutable`、`diff_large_and_binary_side_degradation`。另有语义漂移：`explorer_hidden_refresh_coalesced`／`agent_files_visibility_controls_watchers` 只测未接线的 `WatcherService`；`explorer_paste_conflict_and_partial_success` 只测无生产入口的 `PasteService`；`explorer_copy_capability_union`／`explorer_rechecks_links_before_mutation` 只测 `FileOperationService`；`commit_rollback_document_guard` 断言静默 no-op；`diff_view_state_restore_clears_transients` 只通过 `DiffViewState` DTO 往返断言展示参数；`agent_root_path_change_resets_state_and_authorization` 实际调用 `agent_files_remove_root`，没有走真实路径变更。`.claude/docs/testing/exemptions.md` 还把死代码 `diff_tab_tests` 当作真实 Diff 对齐的兜底。
- 根因：追溯矩阵按设计与服务层用例核销，没有验证测试是否经过生产入口；阶段验收以“全绿 + 例名存在”代替行为可达性。
- 影响：全量测试全绿不能证明功能可用；`docs/devPlan/progress.md` 的“全部阶段验收完成”与上述可达性缺口矛盾。
- 证据：上述例名 grep 为空或测试函数体只调用服务；`WatcherService`／`PasteService`／`DiffTabView` 无生产调用方。
- 优化方向：逐条改名、补真实视图测试或登记豁免；不存在的场景不得标为已自动化；服务级测试不得核销用户动作场景。

### 重要｜项目重定位与导航历史恢复没有生产入口

- 置信度：确认
- 位置：`app_root/projects.rs::relocate_project`（`#[allow(dead_code)]`）；`app_root/navigation.rs::deepest_project_for_cwd`／`resolve_restore_cwd`／`resolve_restore_project`／`restore_injection_timeout`；`app_root/nav.rs`；`persistence/project.rs` 的 `set_root_status` 与 `RelocationRequired`。
- 现象：`relocate_project` 只有 `tests/relocation.rs` 调用；生产没有任何命令、导航菜单或 Runtime 方法触发根重定位；`RelocationRequired` 只有持久化解码与测试手工设置，没有生产写入点。导航历史恢复的全部纯函数没有生产调用，导航视图只有项目行。`backup_cross_machine_requires_relocation` 通过手工构造 `RelocationRequired` 快照验证“不读正文/不启动 PTY”，但真实跨机恢复流程不会产生该状态。
- 根因：导航历史视图与重定位命令/状态迁移未实现；只有纯函数和服务层测试。
- 影响：用户无法重定位项目根；跨机恢复不会进入“先重定位与授权、再读正文/启动 PTY”的合同；featureSpec navigation 的历史恢复与 PathPolicy 的根重定位验收不成立。
- 证据：上述符号调用点；`set_root_status` 生产无调用；`integration_project_relocation`／`navigation_relocation_guards_and_reassigns_history` 直接调用内部 API。
- 优化方向：接入导航历史视图与项目重定位命令/Runtime 入口；跨机恢复必须真实标记 `RelocationRequired`；用同一守卫链，并在真实启动链验证。

### 重要｜启动恢复对干净但文件缺失/不可解码的 Document 不建任何面板

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/recovery.rs::recover_documents`；`app_shell/app_root/commands.rs::restore_project_panels`；`app_shell/app_root/render.rs::render_group`。
- 现象：无崩溃快照的 Document 在读盘返回 `NotFound`／`InvalidData`／`Unavailable` 时，既不恢复正文，也不加入 `requires_authorization`／`identity_replaced`；`restore_project_panels` 的授权／身份替换／Paged 占位分支都不命中时直接 `continue`。布局仍保留该 ViewKey，渲染时找不到 PanelHost，出现空白页签/空面板；相关命令随后以 `MissingPanel` 失败。
- 根因：恢复分类只有授权、身份替换、Paged 三种占位，没有“缺失/不可解码的干净文档”；恢复循环把未知情况当成不需要面板。
- 影响：文件在应用关闭期间被删除、变为二进制或不可解码时，重启后留下不可操作的空白面板，违反“恢复失败保留面板或授权占位，不创建重复视图”。
- 证据：`Err(NotFound)`／`Err(InvalidData)` 分支只在 `snapshot.is_some()` 时恢复；`Unavailable` 且无快照时 `continue`；`restore_project_panels` 最后 `continue`；`render_group` 无内容时返回空元素；测试只覆盖授权/身份替换/Paged 占位。
- 优化方向：为“缺失或不可解码的干净文档”增加明确恢复占位或确定性清理，并补启动测试。

### 重要｜Agent 根路径变更（同 id）沿用旧 ViewState

- 置信度：确认
- 位置：`app_root/agent_files.rs::AgentFilesRuntime::set_roots`／`restore`、`SideView::agent_files_sync`、`agent_files_remove_root`。
- 现象：`agent_files_sync` 只对从配置消失的 `root_id` 调 `remove_root`；同一根 id 改路径时 `set_roots` 新建模型但保留旧持久分区，随后 `restore` 把旧的展开目录、根展开标志和当前行按相对路径套到新根。`agent_root_path_change_resets_state_and_authorization` 通过显式调用 `agent_files_remove_root` 绕开了真实路径变更。
- 根因：持久分区只按 `root_id` 判定，未把路径作为身份部分；路径变化没有触发清理。
- 影响：改路径后旧视图状态错误套用到新目录，可能选中/展开同名但无关的目录；与 traceability 声明相反。
- 证据：`set_roots` 的 reusable 条件要求 id 与 path 都相同，否则新建；`restore` 无条件按 root_id 读取持久状态；`agent_files_sync` 的 removed 只比较 id 集合。
- 优化方向：以“根 id + 路径”作为持久分区身份，路径变化时清理对应状态；测试走真实 `agent_files_sync` 路径。

### 重要｜授权代次在 AppRoot 生产路径恒为 0，文件操作计划可跨授权撤销生效

- 置信度：确认（机制）；撤销时序为推断
- 位置：`app_shell/file_operation_service.rs::OperationPlan::authorization_generation`／`execute`／`authorize_with`；`app_root/explorer.rs::start_explorer_rename`；`app_root/git_changes.rs`；`app_shell/paste_service.rs`；`document_close.rs`；`documents.rs::save_document`。
- 现象：所有生产调用把 `expected_authorization_generation` 与 `current_authorization_generation` 传 0；`PathPolicy::authorize` 只比较二者相等。`OperationPlan` 捕获的 grants 切片在执行时原样复用，AppRoot 中没有授权代次生成或推进的来源（项目移除、AgentRoot 撤销、设置修改都不改代次）。
- 根因：授权代次只有相等检查，没有真实代次源；计划/执行未重新派生授权。
- 影响：计划创建后授权被撤销或项目被移除，执行仍可通过恒 0 代次与旧 grants 检查，违反“执行前重新判定授权、计划不携带可沿用授权”。重命名/删除计划在项目移除期间仍有写盘路径。
- 证据：所有 `authorization_generation` 传参为 0；`PathGrant` 无代次字段；`execute` 只比较 `plan.authorization_generation != current_authorization_generation`；`start_explorer_rename` 在后台闭包中使用前台捕获的 `grants`。
- 优化方向：引入真实授权代次源并在每个计划/执行点传递；撤销与项目移除必须推进代次，使旧计划失败；为撤销期在途操作补测试。

### 重要｜GitShow 非法 UTF-8 的合同冲突

- 置信度：确认
- 位置：`slterm_app/src/git/show.rs::classify_content`；`app_root/git_show.rs::render`；`slterm_app/src/git/CLAUDE.md` 的“GitShow 内容路由”条目；`docs/featureSpec/panels/gitshow.md` 的“文本、查找与显示”。
- 现象：featureSpec 要求“非 UTF-8 字节用替换字符并显示正文外提示”；实现把非法 UTF-8 归类为 `GitShowContent::InvalidUtf8`，UI 显示“无效编码”错误并只提供系统打开；模块 CLAUDE 明确“非法 UTF-8 不生成伪文本”。
- 影响：两份判据不能同时成立，GitShow 的非法编码验收取决于哪份是行为真值；也影响 diff 与只读内容的一致性。
- 证据：上述两处文本原文；`classify_content` 直接返回错误枚举，`render` 的 `InvalidUtf8` 分支不生成文本视图。
- 优化方向：由用户裁决统一合同，并在追溯矩阵记录最终选择；若保留“替换字符”，需要明确复制/查找行为。

### 一般｜Diff 隐藏时仍随每次编辑重算

- 置信度：确认
- 位置：`app_root/diff.rs::apply_shared_edit`／`apply_shared_snapshot`／`schedule_diff`。
- 现象：任何共享 Document 编辑都会无条件广播给所有 Diff 面板并启动后台差分，没有项目可见性/组激活/视图隐藏检查。
- 根因：展示重算没有复用 `ViewTaskOwner` 可见性票据；只有 HEAD 读取被可见性门控。
- 影响：隐藏项目中的大 Diff 持续占用差分预算，违反 featureSpec Diff「不可见时暂停后台刷新与纯展示重算」。
- 证据：`apply_shared_edit` 直接 `schedule_diff`；没有 `set_view_visible` 或 owner 检查。
- 优化方向：把纯展示重算纳入同一可见性合并位，隐藏时丢弃并只保留合并标记。

### 一般｜页签标题不符合合同

- 置信度：确认
- 位置：`app_root/tabs.rs::tab_title`。
- 现象：Editor／Markdown 只取文件名，同项目同名文件没有路径消歧，根外文件不显示规范绝对路径；Diff／GitShow／Paged／Settings 返回 `kind_str`，GitShow 没有 `文件名(git delete)`，Diff 没有实际文件名。
- 根因：标题只按视图类型最小实现，未按位置与视图集合重算。
- 影响：违反 featureSpec Editor「页签默认文件名、同项目同名用相对路径消歧、根外绝对路径」与 GitShow「标题为文件名(git delete)」。
- 证据：`tab_title` 的 Editor/Markdown 分支与 `_ => view.kind_str()`。
- 优化方向：按视图集合与位置推导标题，标题不作为身份。

### 一般｜Commit 读取失败无错误态、错误类别丢失、项目根外行可操作但静默失败

- 置信度：确认
- 位置：`app_root/commit.rs::finish_commit_status`／`commit_apply_status`／`render_commit`／`start_commit_rollback`／`start_commit_delete`。
- 现象：`finish_commit_status` 把 `GitError` 映射为 `()`；`commit_apply_status` 在空列表上失败时既不保留 stale 也不显示错误；渲染没有错误分支。项目根外的行（RepoSibling）仍显示为可回滚/可删除，但执行体因 `starts_with(project_root)` 静默返回，UI 不给出禁用或原因。
- 根因：Commit 运行态缺少错误状态与写能力列；回执错误被丢弃。
- 影响：非仓库、Git 缺失、权限不足、仓库损坏无法区分；用户可能以为行可操作但没有任何效果。
- 证据：`map_err(|_| ())`；`stale` 只在 `!rows.is_empty()` 时置位；`start_commit_rollback`／`start_commit_delete` 的项目根检查静默返回。
- 优化方向：保留分类错误与旧列表/重试；对当前写能力不允许的行在 UI 禁用并说明。

### 一般｜死代码与第二套实现

- 置信度：确认
- 位置：`app_shell/doc_tabs.rs::ImageTabView`／`openable_in_app`；`app_shell/diff_tab.rs::DiffTabView`／`compute_diff_hunks`；`app_shell/file_drop.rs`；`app_root/navigation.rs`；`app_shell/watcher_service.rs`。
- 现象：`ImageTabView`、`DiffTabView`、`FileTreeDrag`、`openable_in_app` 没有生产调用；`DiffTabView` 自建 LCS 行差分与只读双栏，与 `slterm_document::compute_diff` 的单点冲突；`navigation.rs` 全部纯函数只有测试调用；`WatcherService` 未接线。`.claude/docs/testing/exemptions.md` 仍以 `diff_tab_tests` 作为真实 Diff 对齐的兜底。
- 根因：旧壳迁移残留未删除，测试与豁免表把它当成在用的真值源。
- 影响：维护者可能把第二套差分/读取当事实源；豁免兜底无效；测试提供假覆盖。
- 证据：上述符号无生产引用；`diff_tab.rs` 顶部注释自述“预留臂”。
- 优化方向：删除或纳入正式注册点；同步更新豁免表与追溯矩阵。

### 一般｜Explorer 树隐藏全部链接/重解析点，与链接删除合同冲突

- 置信度：确认
- 位置：`app_root/explorer.rs::read_directory`；`docs/featureSpec/sidebar/explorer.md` 的“删除”与 `docs/devPlan/design/files-git.md` 的“FileOperationService”。
- 现象：`read_directory` 只接受普通文件与目录，`is_symlink`／重解析点条目被直接跳过。featureSpec 与设计要求符号链接/目录联接作为树条目被删除且只处理链接项本身、写前重解析。
- 影响：链接条目无法在 Explorer 中操作，相关验收只能在服务层模拟；与“链接重定向写前拒绝”的用户场景不一致。
- 证据：`read_directory` 的 `continue` 分支；`explorer_rechecks_links_before_mutation` 是 `FileOperationService` 测试。
- 优化方向：树层展示链接条目并以链接自身为操作对象，写前保持真实路径与授权重检。

### 一般｜批量/应用/项目关闭保存失败无聚合用户提示

- 置信度：确认
- 位置：`batch_close.rs::finish_batch_save`；`application_close.rs::finish_application_save`；`project_close.rs::finish_project_save`。
- 现象：组内批量保存失败只写日志并置失败态；应用退出与项目移除的保存失败用通用 `Io` 文本通知，具体失败 Document 与原因被折叠。
- 根因：`DocumentSaveFailure` 已聚合但未进入用户可见文案；缺少关闭场景的错误汇总。
- 影响：用户无法知道哪些文件未保存、为何取消关闭；与 featureSpec「保存失败保留脏状态并聚合提示」不符。
- 证据：`finish_batch_save` 的失败分支；`finish_application_save`／`finish_project_save` 的 `SaveError::Io(io::Error::other("…保存失败"))`。
- 优化方向：按 Document 聚合并经统一通知/i18n 呈现场景与失败项。

### 一般｜Explorer 过滤与选择边界未实现

- 置信度：确认
- 位置：`app_root/explorer.rs::explorer_set_filter`／`render_explorer`；`slterm_sidebar::FileTreeModel::set_filter`。
- 现象：过滤无专用空态与清除入口；当前行在过滤或刷新后消失时不迁移到下一个/上一个可见同级；`multi_selection` 不随过滤裁剪（只清 tree.selected）；`show_ignored` 不参与文件树过滤。
- 影响：过滤态的选择/当前行可能指向不可见项，违反 featureSpec Explorer「过滤变化裁剪不可见选择并修复当前行」「无匹配专用空态」。
- 证据：`explorer_set_filter` 只调用 `tree.set_filter`；`FileTreeModel::set_filter` 只清 selected；`multi_selection` 独立保存；`show_ignored` 无消费者。
- 优化方向：过滤变化时统一裁剪选择/当前行并渲染空态；忽略开关接入实际状态投影。

### 提示｜其他具体改进点

- 置信度：确认
- 位置：`app_shell/app_root/documents.rs::build_paged_content`／`document_external.rs`；`documents.rs::open_document_in_system`；`app_root/render.rs`。
- 现象与方向：
  - Paged 文本请求使用魔法常量 `text_request(document_id, 9)`，请求代次语义不明；应来自视图/请求身份。
  - `open_document_in_system` 后台 `opener(path)` 的结果被 `open.await;` 丢弃，失败没有任何日志或用户反馈；系统打开失败应有可见提示。
  - `render_group` 在布局视图缺少 PanelHost 时静默渲染空内容；应对缺失面板显式处理，避免空白页签（见启动恢复问题）。

## 跨域观察

- 导航树（`app_root/nav.rs`）只实现项目行；添加项目、活跃会话、历史会话、搜索、刷新、内联重命名、拖拽排序、套餐页脚均不存在。交互式添加项目也没有入口（只有初始 cwd 与单实例 handover），这超出本审查主范围，但直接影响“导航恢复”和项目生命周期的可用性。
- `.claude/docs/testing/exemptions.md` 的 fs-notify 条目把“外部改文件后树刷新”作为人工兜底，双栏 diff 条目把死代码 `diff_tab_tests` 作为单元兜底；两条豁免的前提在当前实现下不成立，需要重审。
- `slterm_app/src/side_panel` 旧壳模块仍保留 `drop_text_for_path` 等旧合同，`file_drop.rs` 引用它但没有生产消费者；旧栈残留与“单点执行体”纪律并存。

## 存疑未证

- AgentRoot 以别名或大小写差异配置时，`agent_root_grants_for_path` 用配置原文前缀与规范化的 Document 路径比较，可能授权不覆盖真实路径。需要以 junction/大小写变体做真实打开测试确认。
- MarkdownView 内部编辑器的焦点句柄是否等于 `PanelHost::focus_handle`，决定自动保存“全部编辑视图失焦”的判定是否准确。需要在虚拟或真实窗口中验证实际焦点句柄。
- 项目关闭的 Save/Discard 路径没有像批量/应用关闭那样等待在途 Document 保存或外部重定位（`start_project_save` 无 in-flight 检查，Discard 直接进入提交）。需要注入“保存/重定位在途 + 项目移除”时序测试确认是否会出现保存先落盘或读写交错。
- 同一路径的旧 Document（如外部删除后以新文件重建）仍存活时再次打开会产生两个同路径 Document；`commit.rs::document_id_at` 取 `snapshot.documents()` 的首项，可能命中旧冲突缓冲而不是新文件。需要运行场景确认。
- `FsWatch` 的改名配对依赖源路径已在 `state.files`；若 Document 刚打开、watcher 尚未登记完成时发生外部改名，可能漏报并让会话停留在旧位置。需要时序注入验证。
- Diff 恢复时若布局保留 Diff ViewKey 但 ViewState 缺失/损坏，`restore_diff_panel` 只报 `MissingPanel`，没有占位面板；需要真实损坏记录验证空白页签表现与后续命令失败范围。
