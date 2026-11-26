# 应用壳生命周期与工作区命令审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），只审当前工作树；范围为 AppRoot 所有权、焦点、布局、统一关闭、启动与单实例移交、侧栏与页签。

## 覆盖与局限

- 已读合同：根 `CLAUDE.md`、`slterm_app/CLAUDE.md`、`slterm_app/src/app_shell/CLAUDE.md`、`slterm_app/src/app_shell/app_root/CLAUDE.md`、`slterm_app/src/gpui_terminal/CLAUDE.md`、`slterm_workspace/CLAUDE.md`、`slterm_app/src/persistence/CLAUDE.md` 及 `persistence/startup/CLAUDE.md`、`docs/devPlan/plan.md`、`progress.md`、`traceability.md`、`design/workspace.md`、`design/lifecycle.md`、`docs/featureSpec/workspace/shell.md`、`cross-cutting.md`、`sidebar/framework.md`、`.claude/docs/testing/exemptions.md`。
- 已读代码：`app_root` 的 `mod`、`focus_manager`、`command_registry`、`commands`、`application_close`、`close_coordinator`、`project_close`、`terminal_close`、`batch_close`、`tabs`、`layout`、`navigation`、`side_view`、`side_view_host`、`sidebar`、`panel_host`、`events`、`render`、`task_service`、`projects`、`project_view`、`nav`、`hooks_page`、`settings_overlay`（关闭与 Hooks 相关面）、`runtime_snapshot`（关闭与写意图面）、`documents`（打开 FIFO 与面板安装面）；`app_shell` 的 `global_close`、`startup_diagnostics`、`crash_recovery`、`shell_events`、`mod`、`windowing`、`wallpaper`、`theme`、`assets`；`app_shell/document_close.rs`；`slterm_app/src/lib.rs` 与 `local_control/startup`、`client`（单实例移交面）。
- 已读测试与追溯：`app_root/tests` 中关闭、局部关闭回执、项目关闭、批量关闭、Runtime 关闭、页签、布局、导航、侧栏、Handover、终端恢复与 `settings_overlay` 用例；`persistence/startup/recovery` 的布局归一用例；按测试例名核对追溯矩阵工作区壳／布局／关闭／焦点／启动行。
- 为核对键位派发顺序，只读查看了本机 Cargo git checkout 中钉定版本的 gpui `Window::dispatch_key_event` 与 `DispatchTree::dispatch_key` 实现；未修改依赖。
- 局限：未执行 cargo、构建、测试、脚本门禁、GUI 或网络；未逐行审查 `documents.rs` 的正文语义、Diff/Explorer/Commit 内部、SQLite schema 与 writer 线程实现，只审与关闭、焦点、所有权交叉的面；真实 IME、真实 Alt+F4、原生确认框与真实 ConPTY 行为依赖运行时证据，未实测。

## 问题

### 阻断｜恢复失败分支跳过局部关闭段，已收集的 Document 保存决定被丢弃

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/application_close.rs` 的 `start_application_close_with_decisions`；`slterm_app/src/app_shell/close_coordinator.rs` 的 `start_close_flush`；`slterm_app/src/app_shell/windowing.rs` 的 `should_open_initial_project` 与 `open_initial_window`；`slterm_app/src/app_shell/app_root/tests/application_close_receipts.rs` 的 `application_recovery_failure_flushes_without_close_intent`、`application_recovery_failure_uses_global_close_without_intent`；`slterm_app/src/app_shell/app_root/tests/close.rs` 的 `close_without_writer_and_with_recovery_failure_uses_memory_path`。
- 现象：`project_snapshot_recovery_failed` 为真时，关闭入口先收集并校验 Document 决定，随后直接进入全局 flush 分支；`application_close` 不登记，`CloseCoordinator` 不启动，Save 阶段与 StopPty 阶段都不会执行。`collect_application_documents` 返回的对象被丢弃，窗口收到“关闭已开始/已完成”的回执。
- 根因：恢复失败分支把“只冲刷既有写屏障”实现成只调用 `start_close_flush`，绕过了统一关闭主干；Document 决定、PTY 停止与最终状态都依赖被跳过的局部段。
- 影响：该会话仍可工作——显式目录启动且恢复失败时 `should_open_initial_project` 仍会打开 cwd 项目并创建终端，handover 也可打开项目与文件。此后用户编辑的脏正文在关闭时被静默丢弃；若自动保存开启（默认关闭但可配置）或 Runtime `app.close` 显式给出 Save 策略，调用方还会得到“关闭已推进”的成功结果。运行中的 PTY 也不经显式 join 停止。现有恢复失败用例只覆盖空工作区，未暴露该路径。
- 证据：`start_application_close_with_decisions` 在 writer 校验后按 `project_snapshot_recovery_failed` 直接返回 `start_close_flush`，其前面的 `documents` 变量不再使用；`start_close_flush` 只调用 `submit_final_states` 与 `client.flush`；`start_resolved_document_open` 的类型拒绝只覆盖拒绝打开，不阻止在恢复失败会话里打开项目。`app_root/CLAUDE.md` 的「应用退出局部关闭」称恢复失败“只冲刷既有写屏障且不登记关闭意图”，但 `shell.md` 的关闭状态机要求应用退出执行保存与守卫；此处至少需要裁决，现状是丢弃已收集决定且不回执拒绝。
- 优化方向：恢复失败分支要么保留关闭意图并执行 Save／StopPty（至少对显式 Save 决定），要么在存在 Save 需求时拒绝关闭并保持窗口；无论哪种，都不能在跳过保存后给调用方成功回执，PTY 停止也需要明确所有权与兜底。

### 阻断｜Hooks 草稿未进入应用退出与面板关闭守卫，在途载入／保存任务也不随面板取消

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/application_close.rs` 的 `start_application_close_with_decisions` 与 `collect_application_documents`；`slterm_app/src/app_shell/app_root/batch_close.rs` 的 `begin_views_close`、`detach_panels`、`destroy_closed_batch`；`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `request_hooks_draft`、`update_hooks_draft_text`、`save_hooks_draft_for`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `close_settings_overlay`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `start_runtime_panel_close`。
- 现象：统一关闭的守卫集合只覆盖 Document 与终端。应用退出、单面板关闭、批量关闭与设置浮层关闭都不检查 `hook_drafts`；只有项目移除与 Runtime 的 `project.remove`／`app.close` 接受显式 Hooks 策略。关闭项目 Settings 面板后，草稿仍以旧 `PanelId` 留在 `HooksDraftHost`，重开面板得到新 `PanelId`，旧草稿永远不会再被显示；应用退出后内存草稿直接消失。关闭页面时 `HooksDraftLoad`／`HooksDraftSave` 任务也不取消，载入回执在面板已不存在时仍会插入草稿。
- 根因：Hooks 草稿生命周期被实现为 Settings 视图的私有编辑态，没有登记到统一关闭对象，也没有面板销毁时的最终处置；草稿以 `PanelId` 为键使实例销毁后键失去可见入口。
- 影响：用户未保存的 Hooks JSON 编辑在面板关闭或应用退出时静默丢失，违反 `shell.md`“Hooks 草稿按保存／丢弃／取消守卫处理”与守卫集合要求；项目移除在发起前有草稿守卫，但关闭期间仍可继续在 Hooks 编辑器输入（输入层不受关闭冻结），StateCommitted 的 `clear_project` 会把这些新编辑静默清掉。
- 证据：`application_close.rs`、`batch_close.rs`、`terminal_close.rs` 没有 `hook_drafts` 引用；`project_close.rs` 只在发起前 `project_has_drafts` 与 StateCommitted 调 `clear_project`；`close_settings_overlay` 只做还焦与注销；`update_hooks_draft_text` 无关闭检查；`request_hooks_draft` 的回执在 `hook_drafts.get(key).is_none()` 时无条件插入；`TaskId::HooksDraftLoad`／`HooksDraftSave` 不在 `detach_panels`／`destroy_closed_batch` 的取消清单。
- 优化方向：把 Hooks 草稿与在途载入／保存纳入统一关闭决定与面板销毁前结算；面板销毁、项目移除与退出都必须取得 Save／Discard 决定或明确拒绝，不得让草稿键悬挂或让迟到回执复活。

### 阻断｜项目移除的 Save 阶段没有代次重收集，失配后永久失败并阻断退出

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/project_close.rs` 的 `LocalProjectClose`、`remove_project_with_decisions`、`start_project_save`、`finish_project_save`、`retry_project_close`；对照 `batch_close.rs` 的 `recollect_batch_close` 与 `application_close.rs` 的 `recollect_application_close`；`slterm_app/src/app_shell/document_close.rs` 的 `prepare_document_save_stage`、`validate_document_close_generations`；`slterm_workspace/src/close.rs` 的 `CloseCoordinator::retry`、`recollect`。
- 现象：项目移除在收集时把每个待保存 Document 的 `text_revision` 固定进关闭意图。保存失败后重试只调用 `CloseCoordinator::retry`，复用同一意图；`prepare_document_save_stage` 先要求当前 revision 等于意图代次，`validate_document_close_generations` 再在写后复核同一代次。只要文档在收集后发生编辑或外部重载（包括保存期间的一次键入），重试会立即或持续返回 `RevisionMismatch`，协调器永远停在 `Failed(Save)`。失败重试入口也不消费调用方新提供的决定，无法绕开旧意图。
- 根因：项目关闭只实现“同阶段重试”，缺少 Document 代次失配所需的 Collect 重收集与全新意图提交；批量关闭与退出关闭都实现了该分流。
- 影响：项目移除永久失败。`project_closes` 非空使 `ensure_project_snapshot_command_allowed` 拒绝终端、布局与项目命令，`start_application_close_with_decisions` 也直接拒绝应用关闭；用户只能强杀进程，其他未保存内容随之丢失。该状态没有超时、取消或放弃入口。
- 证据：`LocalProjectClose` 没有 `needs_recollect` 或等价标志；`retry_project_close` 只调用 `coordinator.retry()`；`remove_project_with_decisions` 对已存在的 `Failed` 关闭直接转重试，本次传入的 `document_decisions` 被丢弃；`CloseCoordinator::retry` 的注释是“关闭意图保持不变”，`recollect` 的注释要求“内容代次失效时重新进入 Collect”；批量关闭与退出关闭均有重收集实现，项目关闭没有；项目关闭测试集中没有保存失败后的重试用例。
- 优化方向：为项目关闭补齐与批量关闭同构的代次失配重收集、新意图与新指纹提交，并让显式重试能够带入新的 Save／Discard 决定；重收集无法取得决定时应回到用户可见的守卫，而不是永久停在 Failed。

### 重要｜交互式关闭缺少保存／丢弃／取消决定入口，批量关闭与导航移除对脏文档静默无效

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/tabs.rs` 的 `tab_list_command_item`；`batch_close.rs` 的 `close_group_from_request`、`begin_views_close`；`slterm_app/src/app_shell/app_root/nav.rs` 的移除按钮回调；`application_close.rs` 的 `start_application_close`；`events.rs` 的 `notify_application_close_decision_required`；`tests/close.rs` 的 `application_close_requires_decision_when_autosave_is_disabled`。
- 现象：UI 没有任何收集“保存／丢弃／取消”决定的模态或操作面。完整页签列表的关闭其他／全部固定传入空决定；遇到最后引用的脏 Document 时 `collect_document_close_with_autosave` 返回 `MissingDecision`，调用点只记录日志，菜单先收合、界面无任何反馈。导航树移除项目同样把错误丢弃。窗口关闭遇到缺决定只投递一个带脏文档数量的通知并保持窗口，用户无法选择丢弃或取消本次退出。`interaction.document_autosave` 默认关闭，因此这是常规路径。
- 根因：关闭主干只实现了“带决定的执行”，没有实现“收集决定”的交互层；UI 入口把领域错误当作日志事件吞掉。
- 影响：`shell.md` 的统一关闭交互验收不可达：用户无法在批量关闭、项目移除或窗口关闭时选择保存、丢弃或取消，也无法在失败后“再次明确丢弃”。窗口关闭在有脏文档时只能靠用户逐个手动保存，否则无法退出。
- 证据：`tab_list_command_item` 使用 `Default::default()` 构造 `CloseGroup` 并在出错时只 `log::warn!`；`handle_nav_remove` 在渲染回调里 `let _ =` 丢弃结果；`start_application_close` 的 Err 分支只调用 `notify_application_close_decision_required`；`tests/close.rs` 把“通知并保持窗口”写入期望行为；`CloseDecision` 的非空实例只由 Runtime 与测试构造。
- 优化方向：在关闭入口生成关闭对象清单并弹出守卫，收集每文档决定后复用同一关闭链；所有入口在缺决定时必须发起守卫或明确可见地拒绝，禁止静默吞错；追溯矩阵需要新增对应的交互守卫场景。

### 重要｜页签缺少脏标记、CLI 状态与单面板关闭入口，非终端视图只能整组关闭

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/tabs.rs` 的 `render_tab_bar`、`tab_title`；`app_root/command_registry.rs` 的 `CommandId` 目录；`batch_close.rs` 的 `close_view_from_request`；`slterm_app/src/app_shell/doc_tabs.rs`。
- 现象：页签只渲染标题文本；没有共享 Document 脏标记、CLI 状态或关闭入口。命令目录只有终端专用的 `CloseActiveTerminal`，没有关闭当前面板的命令；单视图关闭入口只由 Runtime `panel.close` 与测试调用。Editor、Markdown、Diff、GitShow 与 Settings 面板没有鼠标或键盘的单面板关闭路径，只能通过“关闭其他／全部”整组关闭，而该路径又被缺决定问题阻塞。
- 根因：页签渲染与命令目录没有接入各视图的状态投影与关闭动作；单面板关闭链只完成了领域侧实现。
- 影响：`shell.md` 的“页签显示共享 Document 脏标记、CLI 状态和关闭入口”与“所有核心流程均有键盘路径”不成立；用户无法只关闭一个编辑器页签，也无法从页签判断未保存状态。
- 证据：`render_tab_bar` 只构造 `Tab::new().label(...)`，没有状态徽标或关闭控件；`CommandRegistry` 的关闭命令仅 `CloseActiveTerminal` 且 `execute` 要求聚焦终端；`close_view_from_request` 的调用点只在 `runtime_snapshot` 与测试；`doc_tabs` 仅被 Markdown 视图类型判定引用，不参与 AppRoot 页签。
- 优化方向：页签从面板实体投影脏标记与 CLI 状态，提供关闭动作与完整列表项；在统一目录新增单面板关闭命令并复用批量关闭链，保证指针与键盘入口都可用。

### 重要｜Runtime 关闭策略覆盖非最后引用文档，共享 Document 时 panel.close 与 project.remove 被拒

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `start_runtime_panel_close`、`start_runtime_project_remove`；`slterm_workspace/src/document_close.rs` 的 `collect_document_close`。
- 现象：Runtime 把封闭的 Save／Discard 策略直接应用到目标视图或目标项目所引用的全部脏 Document，而不先判断该 Document 是否仅在关闭集合内被引用。若同一 Document 还被同组其他页签、其他组或其他项目引用，`collect_document_close` 会把它判为集合外引用并带决定，返回 `UnexpectedDecision` 或 `ExternalReference`，整个关闭随即失败，即使这次关闭本来无需处理该 Document。
- 根因：决定构造没有遵循与 `collect_document_close` 相同的最后引用规则；策略被当作逐文档无条件决定。
- 影响：共享 Document 场景下，Runtime 显式策略反而使 `panel.close`／`project.remove` 不可用；与 `shell.md`“关闭非最后引用只销毁 Panel，只有最后引用才要求保存／丢弃／取消”的验收直接冲突。Runtime 客户端无法在不了解全局引用的情况下安全关闭。
- 证据：这些关闭入口的 decisions 循环只检查 `session.is_dirty()`，未检查引用数；`collect_document_close` 在 `has_external_reference` 且 supplied 为 Save 或 Discard 时返回错误；`runtime_close.rs` 的策略用例只覆盖独占 Document。
- 优化方向：先按与 core 相同的最后引用语义生成候选，再只对确实需要决定的 Document 应用策略；共享引用文档自动为 `NotRequired`，并把该语义补进 Runtime 关闭用例。

### 重要｜无项目浮层 Hooks 页没有草稿键，Hooks User 编辑无效果且追溯用例未覆盖真实路径

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `SettingsView::hooks_key`、`request_hooks_draft`、`update_hooks_draft_text`、`render_hooks_page`；`settings_overlay.rs` 的 `SettingsView::new_overlay` 与 `open_settings_overlay_from_request`；`tests/settings_overlay.rs` 的 `app_settings_overlay_keeps_hooks_draft`、`settings_overlay_keeps_draft_and_queue`；`docs/featureSpec/workspace/shell.md` 的设置入口。
- 现象：应用级浮层从不绑定 `PanelId`，而 `hooks_key()` 以 `PanelId` 为必需部分，因此浮层恒无草稿键：载入不触发，输入变化不写入草稿，保存与丢弃按钮为空操作。浮层 Hooks 页仍渲染编辑器与层选择，外观上可编辑，实际完全无效。
- 根因：Hooks 草稿键把面板实例身份当作必要条件，没有为应用级浮层提供稳定作用域；浮层与项目面板共用同一渲染函数但只实现了项目面板的键。
- 影响：`shell.md` 要求无项目浮层提供 Hooks User，该能力不可用；浮层中输入的文本不会进入任何守卫，关闭后无痕丢失。
- 证据：`new_overlay` 传入的视图没有面板身份；`with_state` 仅在项目 `Settings` 键下使用项目层，浮层退到 User 层但仍无面板身份；`hooks_key` 首句 `self.panel_id_value()?` 返回 `None`；追溯用例用 `PanelIdAllocator` 手工插入与浮层无关的草稿，仅断言重新打开浮层不销毁它，未经过浮层的 Hooks 输入、载入、保存或丢弃路径。
- 优化方向：为浮层确立稳定的草稿作用域键（不与项目面板实例共享），或明确移除浮层 Hooks 页并同步修正规格与追溯；验收必须从浮层真实编辑入口产生并保存草稿，不能用测试手工插入的无关草稿替代。

### 重要｜IME 组合期间全局命令仍经 GPUI 键位层派发，组合可能被打断

- 置信度：疑似
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::on_key_down` 中 `marked_text` 早退；`app_root/command_registry.rs` 的 `CommandRegistry::register_bindings`；`.claude/docs/testing/exemptions.md` 的真实输入法行；钉定版本的 gpui `Window::dispatch_key_event` 与 `DispatchTree::dispatch_key`。
- 现象：组合期间只有终端编码路径被跳过。GPUI 在把按键交给元素 `on_key_down` 前先匹配并派发键绑定；命中的全局命令（侧栏开关、面板循环、关闭、设置等）仍会执行，可能移动焦点、销毁正在组合的控件或触发其它副作用。应用没有注册任何 keystroke interceptor 在组合态拦住命令。
- 根因：“组合期间不派发命令”只在视图编码层实现，而命中了绑定的按键根本不会走到该层。
- 影响：违反 `cross-cutting.md` 的 IME 合同；用户组合 CJK 时按修饰键可能误触命令并打断组合，组合文本存在丢失风险。
- 证据：`TerminalView::on_key_down` 的 `marked_text` 判断位于编码函数内；gpui 的 `Window::dispatch_key_event` 先调用 `dispatch_tree.dispatch_key` 取得 `bindings`，只在未命中时才进入 `dispatch_key_down_up_event`；全仓无 `intercept_keystrokes`；`marked_text_keeps_all_keys_off_the_wire` 只直接调用视图按键链；AppRoot 测试无组合态命令用例；豁免中“组合态零命令匹配的单元层用例”在命令派发层没有对应物。还缺真实 Windows IME 组合期间按下已绑定组合键的事件日志或实机录屏。
- 优化方向：在窗口层用 keystroke interceptor 或组合态闸门屏蔽命令派发，组合结束再恢复；补充从真实窗口派发路径观察命令未执行的用例，真实 IME 部分继续按既有豁免人工验收。

### 一般｜布局与侧栏恢复的归一和修复只写日志，没有聚合用户提示

- 置信度：确认
- 位置：`slterm_workspace/src/project_snapshot.rs` 的 `recover_with_documents`；`slterm_app/src/persistence/startup/recovery.rs`；`slterm_app/src/app_shell/app_root/side_view.rs` 的 `AppRootPersistence::load`；`slterm_app/src/app_shell/app_root/events.rs` 的 `WorkspaceRecovered` 与 `notify_workspace_recovered`。
- 现象：启动恢复会删除缺少记录或项目不匹配的终端视图、清掉失去布局引用的终端与 Document，以及修复侧栏字段与无法解码的视图状态；这些事实只留下 `log::warn!`，没有任何聚合提示或警告列表进入界面。通知通道只承载“从副本恢复”与“双损隔离”，且只在数据库打开阶段产生。
- 根因：恢复入口只返回归一后的快照，不携带归一事实；启动通知没有承载布局／侧栏修复的类别。
- 影响：用户无法知道本次启动丢弃了哪些面板引用、修复了哪些侧栏字段或哪一侧栏视图被写屏障。侧栏框架明确要求未知视图、重复记录与非法布局字段归一时聚合提示；布局归一侧同样没有警告通道，日志对用户不可见。
- 证据：`recover_with_documents` 直接 `close_view` 与 `retain`，无警告返回值；`AppRootPersistence::load` 对侧栏恢复只记录警告数量、对 ViewState 解码失败只记日志；`WorkspaceRecoveryNotice` 只承载副本恢复与隔离，`notify_workspace_recovered` 只映射对应类别；`workspace_startup_layout_before_snapshots` 只断言归一结果，不断言提示。
- 优化方向：让恢复结果携带结构化警告并汇入统一启动提示（只含类型化事实，不含路径与正文），在首帧后聚合展示；日志不能替代用户提示，也不能把字段修复当作无记录处理。

### 一般｜批量关闭与项目移除遗留 pane 级任务栏进度，批量关闭未释放 AgentHub 登记

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/batch_close.rs` 的 `destroy_closed_batch`；`slterm_app/src/app_shell/app_root/project_close.rs` 的 `detach_closing_project`；对照 `terminal_close.rs` 的 `destroy_closed_terminal`；`slterm_app/src/agent_host.rs` 的 `AgentHub::remove`；`slterm_app/src/notify.rs` 的 `clear_taskbar_for`。
- 现象：单终端关闭在销毁面板时同时移除 AgentHub 登记并清除该 pane 的任务栏进度；批量关闭销毁终端面板时两者都没有做，项目移除释放了 AgentHub 登记却没有清任务栏。
- 根因：pane 级清理副作用在单终端、批量与项目关闭路径各写一份，批量与项目路径遗漏。
- 影响：AgentHub 留下指向已销毁 pane 的宿主条目，持续占用内存并参与路由；被关闭终端设置的任务栏进度在窗口存活期间残留，直到其他 pane 覆盖。
- 证据：`destroy_closed_terminal` 调用 `agents.remove` 与 `clear_taskbar_for`；`destroy_closed_batch` 只从项目面板表移除并清 `focused_panel`；`detach_closing_project` 有 `agents.remove` 无任何 taskbar 清理。
- 优化方向：把 pane 级清理收敛为面板销毁的唯一函数，单终端、批量与项目关闭共用，避免遗漏。

### 一般｜活动栏缺少固定底部配置按钮，设置只有键盘入口

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/sidebar.rs` 的 `render_activity_bar`；`slterm_app/src/app_shell/app_root/render.rs` 的根渲染；`command_registry.rs` 的 `OpenSettings`。
- 现象：活动栏只渲染上按钮组、弹性空白与下按钮组，没有固定底部配置按钮；设置入口只有 `Ctrl+,` 键位。
- 根因：配置按钮未接入活动栏与命令目录。
- 影响：`sidebar/framework.md` 要求活动栏包含固定底部配置按钮，且配置按钮与 `Ctrl+,` 使用同一设置入口；`shell.md` 的设置入口因此缺少指针路径，纯鼠标用户无法打开设置。
- 证据：`render_activity_bar` 的子元素只有上下按钮组与 spacer；全仓没有设置图标按钮或配置按钮命令；`OpenSettings` 只在键位与请求入口出现。
- 优化方向：在活动栏固定底部渲染配置按钮并复用 `OpenSettings` 命令，同时保持它不参与视图排序、换区或隐藏。

### 一般｜浮层已打开时激活项目，设置入口改开项目面板而非聚焦既有浮层

- 置信度：疑似
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `open_settings_from_request`；`docs/featureSpec/workspace/shell.md` 的设置入口；`tests/settings_overlay.rs` 的 `integration_settings_singleton`。
- 现象：只要存在活跃项目，设置入口就打开或聚焦项目 Settings 面板；它不检查应用级浮层是否已经打开。因此浮层打开后激活项目，再按 `Ctrl+,` 会同时出现浮层与项目设置面板。
- 根因：入口按活跃项目优先分支，浮层只在无项目时被考虑。
- 影响：`shell.md` 规定浮层打开期间继续共享应用设置并保留 Hooks User 草稿，“用户关闭后，下次打开才使用当前项目的 Settings 面板”；现状把“关闭后”的前提绕过，浮层与项目设置面板并存，浮层中的草稿被留在后台。
- 证据：`open_settings_from_request` 在判断 `active().is_none()` 后立即进入项目面板路径，整条路径没有读取 `self.settings_overlay`；`integration_settings_singleton` 只断言浮层未被销毁与项目面板复用，没有覆盖浮层存在时的再次入口目标。还缺产品侧对“用户关闭后”这一顺序语义的确认。
- 优化方向：浮层存在时设置入口先还焦浮层；只有在浮层关闭或用户明确选择项目设置时才打开项目面板，并补充浮层存在时重复入口的用例。

## 跨域观察

- 设置域的待写队列与退出链没有显式同步：`poll_settings_save` 只由壳事件泵驱动，`submit_final_states` 不包含 `SettingsStore` 待写项，而设置写盘走独立的阻塞保存路径；正常退出可能在去抖窗口未到期时关闭 writer 与窗口。该问题属设置与持久化域，但直接影响统一关闭的“必要的应用设置待写状态”守卫，建议由设置所有者确认是否存在末次编辑丢失。
- `AppRootPersistence::drop` 与 `AppRoot::drop` 在正常关闭后仍可能尝试向已被全局关闭线程 shutdown 的 writer 补交终态（终态通道或 final project snapshot），失败只记日志。若确认正常退出顺序必然经过该路径，会产生误导性诊断；属持久化与关闭编排交叉面。
- `start_close_flush` 的返回类型是 `bool`，但实现的所有路径都返回 `false`，调用方的 `Completed` 分支不可达；签名与实现不一致，掩盖了“同步完成”语义的缺失。

## 存疑未证

- `pending_application_close` 在途文档工作期间没有任何超时或取消入口：一旦在途保存或外部刷新长期不结束，`close_in_progress` 会持续为真，命令全部被冻结且窗口无法关闭。需要真实挂起 I/O（网络盘、锁竞争）的运行时证据来确认可达性与严重度。
- `add_project` 与 `activate_project` 在 `restore_project_panels` 之后执行焦点激活；若该步失败，新增项目的 `ProjectView` 可能被丢弃，而恢复阶段已向 `TerminalRegistry` 与 `AgentHub` 登记的 worker／pane 不会被回收。现有故障注入只存在于测试特性且未被用于该组合；需要发布路径可复现的焦点失败证据。
- 侧栏分隔线的无障碍最大值固定为 `SidebarState::MAX_WIDTH`／`MAX_RATIO`，而实际可用宽度会随窗口压缩变小；需要真实辅助技术读取上限后的行为证据，确认这是否属于“无障碍数值也夹紧”的合同缺口。
- 真实 Alt+F4／系统菜单关闭、原生放弃确认框与真实 IME 组合键的事件时序未实机验证；本报告对 IME 的判断基于钉定版本 gpui 的派发顺序与仓内缺失拦截器，仍需运行时日志或人工录屏闭环。
