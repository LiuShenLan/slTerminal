# 审查问题修复状态

> 本文件只记录 `docs/review/` 台账问题的修复状态，防止重复修复。状态词：待修 / 修复中 / 已修（附提交 hash 与测试例名）/ 需裁决（合同冲突类，等待所有者选择一侧）。

## 应用壳、关闭与生命周期

| 问题 | 状态 |
|---|---|
| 交互式关闭没有保存/丢弃/取消决定入口 | 待修 |
| Commit 永久删除确认把任意失败升级为绕过执行体的强删 | 已修：永久删除经 `plan_delete_permanent` + `execute` 复用同一身份/父目录/链接/授权复核；`RecycleUnavailable` 单独分类，只有回收站失败可升级确认（提交 cd140797）；测试 `permanent_retry_is_offered_only_for_recycle_unavailable`、`permanent_delete_never_bypasses_the_shared_operation_executor`、`recycle_unavailable_is_distinct_from_other_delete_failures` |
| 未暂存删除回滚删除任意同内容未跟踪文件 | 已修：镜像候选限定同父目录且位于项目写根内，删除改经 `plan_delete`/`execute` 复核身份与授权（提交 f87962bf）；测试 `unstaged_delete_rollback_never_removes_same_content_files_outside_the_parent`、`unstaged_delete_rollback_keeps_mirrors_outside_the_write_root`、`commit_unstaged_rename_rollback` |
| 文件树 watcher 未接线 | 待修 |
| 外部冲突与另存为没有生产落点 | 待修 |
| 恢复失败分支跳过局部关闭段，已收集决定被丢弃 | 待修 |
| Hooks 草稿未进入应用退出与面板关闭守卫 | 待修 |
| 项目移除 Save 阶段没有代次重收集，失配后永久失败 | 已修：`needs_recollect` 标记 RevisionMismatch，显式重试经 `recollect_project_close` 以最新代次和新决定重建意图（提交 4bfe0605）；测试 `project_remove_save_revision_mismatch_recollects_with_new_decisions` |
| 关闭保存授权漏掉 AgentRoot 能力并集 | 已修：`document_close_grants` 复用 `document_grants`，关闭保存叠加可写 AgentRoot（提交 599debed）；测试 `agent_root_capability_union_and_revoke` |
| 页签缺少脏标记、CLI 状态与单面板关闭入口 | 已修：页签投影共享 Document 脏标记与 AgentHub 状态（工作=实心圆／阻塞=实心方／完成=空心圆），每页签带关闭按钮；新增 `CloseActivePanel`（`ctrl-w`）复用批量关闭链（提交 e9c59e61 及形状区分补充）；测试 `tab_bar_projects_dirty_status_and_closes_a_single_panel`、`close_active_panel_command_closes_a_focused_editor` |
| Runtime 关闭策略覆盖非最后引用文档 | 已修：`last_reference_decisions` 只为关闭集合内最后引用的脏文档生成 Save／Discard，共享文档不产生决定（提交 dfc8330e）；测试 `runtime_panel_close_ignores_documents_shared_outside_the_closing_view` |
| 无项目浮层 Hooks 页没有草稿键 | 已修：`HooksDraftKey` 所有者改为 `HooksDraftOwner::{Panel,Overlay}`，浮层以 Overlay 键载入 User 层草稿，关闭浮层释放其草稿与文档；测试 `app_overlay_hooks_page_uses_overlay_draft_and_releases_it_on_close` |
| IME 组合期间全局命令仍经 GPUI 键位层派发 | 已修：组合态同步至全局 `ImeComposition`，AppRoot 的 keystroke interceptor 在绑定派发前拦截（提交 420d0044）；测试 `marked_text_toggles_the_ime_command_gate`、`ime_composition_blocks_global_command_dispatch`、`marked_text_keeps_all_keys_off_the_wire` |
| 布局与侧栏恢复警告只写日志 | 已修：`AppRootPersistence` 聚合恢复跳过／修复计数（快照、侧栏、Editor/Markdown/Diff/Settings/SideView ViewState），AppRoot 首帧经 `notify_workspace_repairs` 以 typed i18n 提示，提示不含路径；测试 `side_view_load_marks_corrupt_state_write_barrier`、`side_view_load_restores_existing_private_state`、`workspace_repair_notice_states_the_skipped_item_count` |
| 批量/项目关闭遗留 pane 级任务栏与 AgentHub 清理 | 已修：新增 `AppRoot::forget_panel_runtime`，单终端、批量摘除、批量销毁与项目移除共用同一 AgentHub／任务栏清理入口；测试 `integration_batch_close`、`project_remove_commits_intent_before_destroy` |
| 活动栏缺少固定底部配置按钮 | 已修：活动栏固定底部 `app-root-settings-button` 复用 `OpenSettings` 命令与唯一浮层，不参与视图组织（提交 d72c614a）；测试 `activity_bar_settings_button_opens_the_settings_overlay` |
| 浮层已打开时设置入口改开项目面板 | 已修：`settings_overlay` 存在时入口只还焦浮层，项目面板只在浮层关闭后的下一次入口使用（提交 44112ce5）；测试 `integration_settings_singleton` |
| Commit/Explorer 数据根切换时在途状态读取被丢弃或错投 | 待修 |
| 项目重定位与导航历史恢复没有生产入口 | 待修 |
| 启动恢复对缺失/不可解码的干净 Document 不建面板 | 已修：新增 `DocumentRecoveryKind::Unavailable` 占位与 typed 文案，未知 Document 不再 `continue` 丢面板，可由页签关闭入口移除；测试 `missing_clean_document_restores_unavailable_placeholder` |
| Agent 根路径变更（同 id）沿用旧 ViewState | 已修：`agent_files_sync` 把「同 id 不同路径」视为移除并重登记，旧持久分区、展开目录与当前行一并清理；测试改走真实同步入口 `agent_root_path_change_resets_state_and_authorization` |
| 授权代次在生产路径恒为零 | 已修：新增进程级 `app_shell::authorization` 代次单源，项目移除／重定位与 Agent 根变更推进代次；Document 保存、关闭保存、Explorer 重命名、Git 回滚/删除与各 PathPolicy 判定均读取真实代次，执行前重读最新值；测试 `project_relocation_advances_the_authorization_generation`、`agent_root_path_change_resets_state_and_authorization` |
| GitShow 非法 UTF-8 的合同冲突 | 需裁决 |

## 设置、主题、导航与 Agent

| 问题 | 状态 |
|---|---|
| 设置中心页面集合与导航结构未按规格实现 | 待修 |
| 设置中心缺少导入、导出、备份、诊断、更新、Runtime、AgentRoot 与套餐凭据入口 | 待修 |
| 自定义主题库没有设置界面，主题选择只列内置主题 | 待修 |
| 主题文档仍强制承载排版、卡片几何、背景图与效果字段 | 已修：`ThemeDefinition` 删除 typography/layout/effects，文档只收颜色与 ANSI；旧区段按未知字段拒绝，应用桥只写主题键（提交 40b4a59b）；测试 `pure_color_documents_are_accepted_without_non_color_sections`、`generated_documents_carry_only_colors_and_ansi`、`legacy_non_color_sections_are_rejected_as_unknown_fields`、`applying_a_theme_writes_only_theme_keys_and_keeps_personal_appearance` |
| 最终排除项仍在设置页暴露 | 待修 |
| WCAG 对比度校验与提示残留，合同互相矛盾 | 已修：删除 `validate_wcag`/阈值/建议色/`wcag_violations`/内置对比度测试与 i18n 承诺；源扫描守卫 `theme_sources_do_not_validate_contrast`，模块文档同步为“不做对比度校验或提示” |
| 设置页大量控件没有生产消费方 | 待修 |
| 设置保存状态与失败不可见；文本/颜色控件跨视图不同步且缺少路径校验 | 待修 |
| 根重定位实现为 dead code，缺少真实路径查重与决定守卫 | 待修 |
| 重复 Settings 恢复不按规格保留首个并提示 | 部分修复：宽容解码按首次出现保留重复视图，整库不再因重复 Settings 解码失败；测试 `lossy_recovery_filters_unknown_views_collapses_empty_groups_and_dedupes`、`startup_filters_unknown_and_duplicate_views`。恢复提示面仍待接线 |
| 主题下拉选择不清除 custom_theme | 已修：`theme_field_followups` 在选择内置主题时清除 `custom_theme`；测试 `theme_field_followups_clear_only_for_builtin_theme_selection`、`builtin_theme_selection_clears_custom_theme_override` |
| Runtime v2 快照没有设置界面，快照测试未覆盖资源边界 | 待修 |
| 测试主要覆盖引擎函数与 DTO，未覆盖页面场景 | 待修 |
| Hooks 页只实现 Claude JSON 子集，缺少 GUI、事件目录、外部监听、托管只读与保存提示 | 待修 |
| 浮层不持久化 ViewState，失效页回退未接线 | 待修 |
| 真实目录查重漏掉内置根，生产路径绕过 Registry | 待修 |
| 改路径沿用旧根能力 | 已修：`AgentRoot::with_path` 把新根重置为默认只读；测试 `agent_root_path_change_requires_new_identity`、`agent_root_path_change_service_semantics` |
| agent.start 注入 slug 而不是 AgentCatalog 内置命令 | 已修：改注入 `kind.catalog().builtin_command`；测试 `runtime_agent_start_injects_catalog_builtin_command` |
| Claude 无原生会话 ID 时仍生成 `--continue` | 已修：无原生会话 ID 一律返回 None；测试 `resume_commands_require_catalog_capability_and_safe_identity` |
| 撤销状态在恢复成功前被置为 Undone | 待修 |
| quarantine_batch 不先持久化清单，部分失败丢失材料身份 | 待修 |
| 主题身份不是 ThemeId/UUIDv7，导入冲突被自动改名 | 已修：自定义主题身份改为 `ThemeId`（UUIDv7），显示名与身份分离；内置主题无可持久身份；同身份/同名导入明确拒绝且零写入（提交 de837f97）；测试 `theme_identity_is_uuid_v7_and_independent_from_display_name`、`import_rejects_a_display_name_conflict_without_writing`、`import_rejects_an_identity_that_already_exists`、`fork_assigns_a_uuid_v7_identity_independent_from_the_name` |
| ANSI 16–255 被持久化成第二份色表 | 已修：主题只保存 ANSI 0–15，16–255 统一经 `xterm_indexed_color` 派生，文档不再写 `terminal.indexed`（提交 997bdcc6）；测试 `extended_indexed_colors_are_derived_not_persisted`、`indexed_colors_are_derived_from_the_single_xterm_formula`、`resolve_indexed_falls_back_to_cube_and_grayscale` |
| 设置损坏分流缺“高版本”和“权限/IO 只读重试” | 部分修复：新增 `settings_version` 字段，高版本读取标记 `unsupported_version` 且写通道拒绝覆盖（不隔离）；测试 `higher_settings_version_is_read_only_and_never_overwritten`、`settings_writes_stamp_the_current_version`。权限/IO 回退默认仍保留启动可用性，用户可见反馈归“设置写失败/损坏无用户可见反馈”条目 |
| 设置写失败、损坏与单字段回退没有用户可见反馈 | 待修 |
| 恢复默认旁路统一设置写通道、审计与保存锁 | 已修：`settings_store::restore_default_settings_at` 共用保存锁、审计事件与 1MB 上限；测试 `settings_import_and_reset_preserve_secrets` |
| AgentCatalog 适配器声明未被消费，CLI 名单多源 | 待修 |
| Hooks 事件目录远小于 featureSpec 的 Claude 事件集合 | 已修：`HookEvent` 扩为 31 项全表，编辑器校验与 `hook_events()` 使用 `CLAUDE_EVENT_CATALOG`，安装器仍只注入 10 项订阅子集；测试 `legal_catalog_events_save_even_when_not_installed`、`claude_event_catalog_covers_the_full_feature_spec`（提交 21eb93f7） |
| 设置热应用在 GPUI 前台同步读取自定义主题文件 | 已修：主题定义预载缓存 + `ThemeLoad` 后台任务，启动/显式提交走冷路径读盘，字段提交后的快照重建只查缓存，库写入/删除主动失效（提交 b83926fe）；测试 `hot_snapshot_uses_the_cached_custom_theme_definition_without_disk_io`、`custom_theme_hot_apply_uses_the_background_preloaded_cache`、`cache_hits_only_the_installed_identity_and_clears_on_invalidate` |
| 套餐凭据目标不含本地根稳定散列 | 已修：`credential_target(local_root)` 绑定 `stable_path_hash`；测试 `plan_balance_targets_are_scoped_to_the_local_data_root` |
| 普通设置导入、导出与恢复默认只有内存纯函数 | 待修 |
| Agent 基础配置、AgentRoot 管理与 Agent 文件操作面未实现 | 待修 |

## Hooks 与 hook 桥

| 问题 | 状态 |
|---|---|
| 生产环境没有 hook 事件入口 | 待修 |
| 打开 Hooks 页留下无法清除的草稿，使项目移除/根重定位永久报 HooksDraftPending | 已修：`project_has_drafts` 只统计绑定该项目且 `dirty()` 的草稿，干净草稿不阻塞（提交 bbf3f69b）；测试 `hooks_project_guard_ignores_user_layer_and_clears_project_drafts`、`navigation_relocation_guards_and_reassigns_history`、`project_remove_requires_hooks_drafts_resolved` |
| 管道事件不校验 pane 归属与 PTY 代次 | 待修 |
| 注入/卸载没有 Journal 与启动恢复器 | 待修 |
| 危险命令确认没有生产链路 | 待修 |
| GUI 编辑模式与事件树/handler 表单不存在 | 待修 |
| Hooks 编辑保存绕过审计与 PathPolicy | 已修：新增 `hook_editor_write` 审计包装（主文件与备份均审计，提交 eb1c71e4）；保存前经真实规范位置包含校验与 `HooksWrite` 精确文件 grant 的 `PathPolicy` 判定（提交 3346cdf9）；测试 `saving_hooks_emits_audit_event_without_content`、`hook_editor_write_emits_audit_event_without_content`、`save_rejects_target_outside_the_authorized_root_without_writing` |
| 加载路径不检测重复键 | 已修：加载/保存重读共用严格解析，重复键拒绝编辑；测试 `load_rejects_duplicate_keys_instead_of_silently_overwriting` |
| config_guard 没有生产调用点 | 待修 |
| AgentHooks 开关状态按 AgentHook::ALL 顺序索引 AGENTS 行 | 已修：行开关按 `AGENTS` 自身顺序读取设置键；测试 `agent_row_states_follow_agents_order_not_all_order` |
| 保存进行中继续编辑会被保存回执静默覆盖 | 已修：`HooksDraft::finish_save` 以提交文本比对结算，期间编辑保留正文与脏态、只推进基线；任务身份按完整草稿键隔离（提交 29094491）；测试 `save_receipt_keeps_edits_made_while_the_save_was_in_flight`、`hooks_page_loads_saves_external_guard_and_discards` |
| Hooks 层默认与切换错误 | 已修：项目面板重建视图恒回 User 层，`available_hooks_layers` 按视图身份恒给三层，User↔Project 可往返（提交 94ca8256）；测试 `hooks_page_loads_saves_external_guard_and_discards` |
| Hooks 草稿任务身份只含面板 | 已修：`TaskId::HooksDraftLoad/Save` 改携完整 `HooksDraftKey`（提交 5a9fb9e1）；测试 `hooks_draft_task_identity_includes_layer_and_agent` |
| statusline 用量桥接与信号通道整体缺失 | 待修 |
| 追溯矩阵与进度把未接线的生产链记为已自动化/已验收 | 待修 |
| Hooks 页面错误文案硬编码中文 | 已修：编辑域返回封闭 `HooksEditError`，页面包装 `HooksPageError`，渲染经 `hooks_error_text` 查询 typed `Message`，新增中英文诊断键（提交 5ab9a661）；测试 `save_rejects_unknown_events_and_invalid_handlers_without_writing`、`hooks_save_backup_failure_keeps_original_untouched`、`hooks_page_loads_saves_external_guard_and_discards` |
| 项目根不可访问时未禁用 Project/Local | 已修：载入与保存前在后台校验真实层根目录，缺失时 Project／Local pill 禁用、显示原因并拒绝写盘（提交 e971c4ae）；测试 `hooks_project_root_loss_disables_layers_and_never_recreates_config`、`hooks_missing_user_config_dir_is_reported_without_creating_it` |
| Hooks 外部修改监听与重新加载缺失 | 待修 |

## Document、Markdown 与文本视图

| 问题 | 状态 |
|---|---|
| SVG 清洗重建 XML 时未转义属性值 | 已核验无需修复：`push_attribute` 接收 `(&str, &str)` 时由 quick-xml 的 `From` 转换转义属性值；报告引用的 FIXME 位于底层 `push_attr`，调用路径不经过它。新增回归守卫 `attribute_values_are_escaped_so_reparsing_preserves_the_attribute_set` |
| 同一规范路径的外部重载在身份变化时覆写其他 Document 身份索引 | 已修：外部重载与普通保存提交前经 `identity_owner` 拒绝其他聚合已占用的身份，`refresh_identity` 只补建本聚合索引、绝不改写其他 owner；测试 `external_reload_rejects_identity_owned_by_another_document`、`in_place_save_rejects_identity_owned_by_another_document` |
| 可编辑 Document 缺行尾切换、编码重开与手动只读活体通道 | 待修 |
| 查找与替换只实现字面量查找和全部替换，且没有生产接线 | 待修 |
| 本地图片路径在授权前被解析，UNC 可自动触发 SMB 认证 | 已修：`PathGrant::lexically_covers` 纯词法预检先于解析，未授权目标（含 UNC）不触碰文件系统；测试 `lexical_precheck_covers_subtree_and_exact_scope_without_io`、`authorization_precedes_resolution_and_unc_never_resolves_without_grant` |
| 行内公式与行内图片被移出文本流 | 待修 |
| 引用、列表与脚注内的块结构被压平 | 待修 |
| 脚注引用不渲染、不可点击 | 待修 |
| 安全 HTML 白名单只做文本降级 | 待修 |
| 程序化滚动与恢复位置不走视口调度 | 待修 |
| 每次滚动清空全部已渲染 outcome | 待修 |
| 高 DPI 下公式位图被放大后再按 1:1 逻辑像素显示 | 已修：`logical_formula_size` 按窗口 scale 除回逻辑尺寸，块级与行内公式均设显式逻辑宽度（zoom 保留在位图内）；测试 `formula_logical_size_undoes_window_scale_but_keeps_reading_zoom` |
| 已加载的远程图片再次点击仍发起新请求 | 已修：已加载或有在途任务时重复点击不再重启请求；测试 `repeated_image_click_does_not_restart_requests` |
| 渲染查找在前台按匹配数乘块数扫描 | 待修 |
| 每次滚动在前台复制整篇文档文本 | 待修 |
| 大纲无层级且常驻挤占内容区 | 待修 |
| 渲染态缺少键盘滚动、大纲与链接激活 | 待修 |
| 安全与授权关键路径只有服务/投影测试 | 待修 |
| Editor 面板规定能力未实现 | 待修 |
| 本地编码检测与编码重开在实际打开链中不存在 | 待修 |
| 追溯矩阵的自动化证据层级与场景不一致 | 待修 |
| 分页只读视图不实现换行，且与软换行 Editor 共用滚动坐标单位 | 待修 |
| 分页页读取失败后永久停留 Loading | 已修：失败页显示底部提示条与显式重试，点击清空失败抑制并重新读取同一页（提交 2a991217）；测试 `paged_view_explicit_retry_reruns_failed_pages`、`paged_view_does_not_retry_failed_pages_on_every_frame` |
| 外部快照与撤销/重做把滚动位置重置到顶部 | 已修：`apply_snapshot` 只复位选区与瞬态范围，滚动行/列保留，越界由 `adopt_snapshot` 夹紧（提交 9971ced8）；测试 `external_snapshot_keeps_the_current_scroll_position` |
| Git 行标丢失增改删语义，颜色与越界防护不满足合同 | 已修：`GitMark{line,kind}` 保留增／改／删，新增与修改使用 success／warning 语义色、删除用细条，绘制前过滤越界行；测试 `diff_marks_classify_insert_replace_and_delete`、`out_of_range_git_marks_are_ignored_during_painting`、`editor_git_marks_coalesce_latest_text` |
| reveal_line 在软换行下把逻辑行当显示行 | 已修：软换行下 `reveal_logical_line` 经 wrap 索引换算显示行，索引未就绪登记待揭示行，索引完成回投消费（提交 c8a844d9）；测试 `reveal_line_targets_the_display_row_under_soft_wrap`、`reveal_line_waits_for_the_wrap_index_before_scrolling` |
| 未知扩展名的文本文件在读取前被拒绝 | 已修：`viewable_file` 仅拒绝已知二进制扩展名，未知/无扩展名按纯文本尝试，NUL 与编码在真实读取后裁决（提交 8a3bb0de）；测试 `viewable_file_maps_known_source_extensions_case_insensitively`、`openable_in_app_routes_known_binaries_and_accepts_unknown_text`、`unknown_extension_utf8_opens_as_plain_text_and_binary_is_rejected` |
| 分页恢复占位是无出口面板 | 已修：重新打开命中 `PanelContent::Recovery` 时按真实文件重读分页正文并用同一内容构造器替换占位（复用 PanelId/ViewKey）；已有分页 Document 缺读取端口时重新读取 Paged 正文；测试 `paged_document_restart_has_explicit_placeholder` |
| 布局缓存没有容量或可见窗口回收 | 已修：`LayoutCache` 超过容量后按插入行保留 ±512 行窗口，远距离滚动淘汰旧行；测试 `layout_cache_pruning_keeps_only_the_window_around_the_visible_row` |

## 终端核心与 GPUI 终端

| 问题 | 状态 |
|---|---|
| 存活上限预检发生在真实 PTY 创建之后 | 已修：`TerminalRegistry::preflight_launch` 在解析 shell／创建 ConPTY 之前判定重复与额度；`launch`/`restore_launch` 均先预检。测试 `launch_capacity_is_checked_before_spawning_a_real_pty` |
| 空 `Msg::Input` 永久卡住写队列 | 已修：发送端拒绝空输入 + 状态机跳过空项；测试 `empty_input_never_enters_the_command_channel`、`write_state_skips_empty_entries` |
| 重连期间旧 worker 仍可注入退出事件或显示命令 | 待修 |
| 帧唤醒标记与最新帧存储存在丢失竞态 | 已修：最新帧与唤醒位合并为同一 `WakeState` 锁，接收端取帧时原子清除标记，发送端新帧可重新入队；测试 `frame_written_between_take_and_ack_is_not_lost`、`frame_events_coalesce_to_latest_without_losing_semantic_events` |
| 子进程退出时未排空已读输出 | 已修：本地启动开启 `drain_on_exit`，退出边界循环排空队列后才发布最终帧；测试 `local_options_drain_output_before_exit`、`child_exit_drains_queued_output_before_final_frame` |
| 读取线程 EOF 或读取错误不会转成退出/故障事件 | 已修：`EventedPty::output_closed` 暴露读取端关闭，宽限 2 秒内无子进程退出事件即报告 `PtyFailure`；测试 `reader_reports_output_closed_after_eof`、`output_closed_without_child_exit_reports_transport_failure` |
| OSC 原始缓冲无界，嗅探上限在分发后才生效 | 已修：`OscGate` 在进入 VT 解析器前把 OSC 负载限制在 24 MiB（覆盖内联图片 base64 上界），超限丢弃负载、保留终止符；测试 `osc_gate_bounds_unterminated_payload_and_recovers_after_terminator`、`osc_gate_keeps_st_terminator_across_chunk_boundaries` |
| 运行时设置未作用到终端核心 | 待修 |
| 搜索的帧驱动重查会重置导航位置并形成搜索风暴 | 已修：新帧刷新保留选中项且不滚动，在途请求跳过重排；测试 `frame_driven_search_refresh_preserves_selection`（提交 ac25b809） |
| 退出后键盘复制、搜索与回滚全部失效 | 已修：退出后保留复制/搜索/回滚，粘贴与编码路径关闭；测试 `exited_terminal_keeps_copy_and_scroll_but_rejects_input` |
| 一次瞬态操作错误会替换并永久遮住终端画面 | 已修：超限粘贴与 Agent 注入失败的 `notice` 瞬态提示不再替换终端元素，可点击关闭（提交 ba269cdb）；测试 `oversized_paste_keeps_terminal_visible_with_dismissible_notice`、`dangerous_paste_confirmation_does_not_leak_or_write_early` |
| Ctrl+Enter 未发送 LF，行镜像把修饰 Enter 当作提交 | 已修：`encode_for_program` 对 Ctrl+Enter 恒发 LF，行镜像 control 分支已使镜像失效；测试 `ctrl_enter_always_sends_lf` |
| 键盘选区扩展未实现，Shift+PageUp/PageDown/Home/End 语义相反 | 已修：`SelectionCommand::Extend(SelectionMotion)` 由 worker 按光标/既有锚点移动端点，方向键、翻页与 Home/End 不再编码进 PTY；端点自动滚入视口，备用屏夹在当前画面；测试 `keyboard_extension_moves_endpoint_with_inclusive_cells`、`keyboard_extension_in_alt_screen_stays_in_current_screen`、`page_extension_scrolls_endpoint_into_view`、`shift_arrow_extends_selection_without_pty_write`、`shift_pageup_extends_selection_into_scrollback`、`shift_paging_extends_selection_and_alt_screen_stays_local` |
| Ctrl+Shift+A 同时被终端全选与侧栏 Agent 文件占用 | 已修：终端独占 `Ctrl+Shift+A` 全选（`SelectionCommand::SelectAll` 覆盖回滚与屏幕，尾部空白不产生尾随换行，零 PTY 写入）；侧栏 Agent 文件改绑 `Ctrl+Shift+I`，命令目录新增保留键断言；测试 `select_all_covers_scrollback_and_screen`、`ctrl_shift_a_selects_all_terminal_text_without_pty_write`、`side_view_shortcut_bindings_cover_all_registered_views` |
| 光标形状、闪烁与相关设置不符合行为真值 | 待修 |
| 主题、字体、字号、连字与复制/补全外观设置不热应用 | 待修 |
| 真实 ConPTY 验收场景覆盖不足 | 待修 |
| 测试语义与生产路径断裂 | 待修 |
| 项目脚本与 workspace 有界预算缺少真实测试 | 待修 |
| 可信边界与旧代丢弃的验收测试不足 | 待修 |

## 持久化、崩溃恢复与加密备份

| 问题 | 状态 |
|---|---|
| Document 崩溃快照只有恢复器，没有生产写入与删除链路 | 待修 |
| 加密备份没有工作区类别，跨机重定位与逻辑工作区导出不存在 | 待修 |
| 加密备份没有生产入口、预览、原子导出、跨重启恢复点与撤回 | 待修 |
| stage_restore 把解密后的备份条目明文写盘 | 待修 |
| 灾难副本从未生成与刷新 | 待修 |
| 备份恢复在 store_epoch 轮换与 Applied 之间崩溃会永久阻断启动 | 待修 |
| 双损隔离部分失败或中断后，重试以空工作区正常启动 | 待修 |
| 备份明文没有 64 MiB 上限 | 已修：密封与解封双向 64 MiB 硬上限，解封在派生密钥前拒绝超限包；测试 `archive_plaintext_is_bounded_to_64_mib` |
| 设置导出使用敏感键黑名单而不是登记字段白名单，Session 原文完全未过滤 | 待修 |
| 恢复点与暂存目录没有当前用户 ACL 限制 | 待修 |
| 暂存明文先于 owner 落盘，清理先清行后删文件 | 待修 |
| 即时恢复在指纹回写失败时已写盘却返回失败 | 待修 |
| 撤回只返回单一错误，逐项结果不可见，成功后恢复点不失效 | 待修 |
| 备份恢复、应用、回滚与撤回没有审计/恢复日志，设置写绕过 settings_store | 待修 |
| 审计成功事件在 Warn 级日志过滤下被丢弃 | 已修：`level_allowed` 让 audit target 恒放行；测试 `audit_records_bypass_the_warn_level_gate` |
| 主日志写入系统临时目录而不是本地数据根 | 已修：`log_file_path` 归本地数据根；测试 `log_file_lives_under_the_local_data_root` |
| 根外 Document 的会话重授权没有生产调用点 | 待修 |
| 布局或快照领域恢复失败时静默以空工作区开窗 | 待修 |
| 未完成关闭意图、NeedsAttention 维护请求与遗留恢复点只有阻断 | 待修 |
| 更高版本数据库先以读写连接打开 | 已修：既有库先经 `SQLITE_OPEN_READ_ONLY` 探测版本，高版本直接保留只读连接（提交 c8290344）；测试 `higher_version_wal_database_is_opened_read_only_without_touching_data_files`、`workspace_higher_version_is_query_only_and_not_downgraded` |
| 设置 IO/权限失败回退默认值，设置文件没有高版本判定 | 部分修复：高版本判定与写拒绝已实现（同 settings_store 版本字段）；IO/权限仍回退默认值以保启动可用，设置页警示面待接线 |
| 追溯矩阵把函数级、未接线或不同场景的测试当成已自动化验收锚 | 待修 |
| 崩溃与边界测试缺少关键窗口 | 待修 |
| 快照存储运行期遇任意损坏文件即阻断全部新写入 | 已修：写入路径忽略旁路损坏文件，留待启动隔离；测试 `unrelated_corrupt_snapshot_does_not_block_writes` |
| 快照文件名与头身份不一致时被当作损坏 | 已修：扫描以信封头身份为准，写成功清除旧位置残留；测试 `snapshot_header_identity_wins_over_file_name` |
| snapshot_index 与历史隔离表没有生产写入 | 待修 |
| 备份归档 DTO 接受未知字段，空类别集也能解封成功 | 已修：DTO 加 `deny_unknown_fields`，空类别集/零条目拒绝；测试 `archive_rejects_unknown_fields_and_empty_selection` |

## Windows 平台、更新、套餐、通知与 Runtime

| 问题 | 状态 |
|---|---|
| 更新链在运行态没有入口 | 待修 |
| 套餐余量功能在运行态不存在 | 待修 |
| 诊断与清理没有产品面 | 待修 |
| 通知事件源与托盘快照没有接入 | 待修 |
| 系统关机与注销不走统一关闭链 | 待修 |
| 通知前台状态从未写入 | 待修 |
| 应用内错误不可见，错误类别不进入任务栏错误态 | 待修 |
| 任务栏聚合没有资源身份 | 待修 |
| 更新预约在崩溃或知情放弃关闭时仍保留 | 待修 |
| 启动安装依赖已退役的 session.json，恢复票没有消费方 | 待修 |
| 托盘可用性判定早于建窗，失败后没有恢复节拍 | 待修 |
| 合同冲突：静默启动的触发范围 | 需裁决 |
| 诊断脱敏不覆盖真实 Runtime token 形态 | 待修 |
| 追溯矩阵把服务层或注册表层用例标成运行态集成 | 待修 |
| 套餐周期契约与后台任务注册表不一致 | 待修 |
| Runtime `git.diff` 路径校验放过盘符绝对路径/符号链接，工作区读取越过仓库根 | 已修：协议拒绝冒号/控制字符（盘符、ADS），应用层对规范位置做仓库根包含检查；测试 `git_diff_requires_a_safe_repository_relative_path`、`git_diff_snapshot_rejects_targets_outside_the_worktree` |
| 快照 revision 固定为握手起点，事件续传进入不可恢复 resync 循环 | 待修 |
| 非交互关闭在保存、停止或清理落定前返回 Succeeded | 待修 |
| 事件环移除滞后订阅后，订阅连接保持静默 | 待修 |
| expected_revision 在线上端到端未生效 | 待修 |
| Operation 停在 Queued/Running 后没有恢复路径 | 待修 |
| 项目、布局、面板与终端写 Operation 在持久快照提交前报告成功 | 待修 |
| Hooks 草稿在决定齐备前被丢弃 | 已修：Runtime `app.close`／`project.remove` 先收集文档决定、预检并通过关闭登记后才按 `Discard` 清理草稿；项目移除以显式 `allow_hooks_discard` 跳过交互守卫（提交 c2b50b9d）；测试 `runtime_hooks_discard_waits_for_complete_document_decisions`、`runtime_project_hooks_discard_waits_for_document_decisions`、`runtime_project_remove_honors_discard_and_save_policies` |
| 合同冲突：app.close 回执语义与“保存失败返回失败”冲突 | 需裁决 |
| 合同冲突：非交互关闭缺少运行终端处置策略 | 需裁决 |
| 控制面一次 join 失败被永久重放 | 待修 |
| 随包 schema 未覆盖多数方法的请求结构，method 与 params/result 无关联校验 | 待修 |
| Runtime 合同测试不在任何固定门禁内 | 待修 |
| Runtime 双进程 conformance 与事件断线矩阵缺失 | 待修 |
| 事件面只发布项目激活 | 待修 |
| 路由与能力摘要不按方法注册表作用域执行 | 待修 |
| `GitStatusResponse.root` 返回项目根而不是仓库根 | 待修 |
| 快照不暴露布局树与分割几何，resize 路径不可推导且视口由客户端自报 | 待修 |
| 等待操作不能因客户端断开而取消 | 待修 |
| `runtime_token_absent_from_materials` 锚只覆盖诊断材料 | 待修 |
| 注册表 kind/idempotent 语义与真实副作用不符 | 待修 |
| 握手后的解析失败直接断开连接 | 待修 |
| 分页与响应预算按原始字节估算 | 待修 |
| 分页游标不绑定资源身份，也接受非 UTF-8 字符边界偏移 | 待修 |
| Handover 客户端不核对 hello 协议版本与合同散列 | 待修 |
| git.open_diff/git.open_deleted 超时按失败处理但打开可能仍完成 | 待修 |
| agent.start/resume/fork 在提示符注入前报告成功 | 待修 |
| 封闭文本字段在 DTO 与 schema 中仍是自由字符串 | 待修 |

## 脚本、发布、许可与依赖

| 问题 | 状态 |
|---|---|
| 内嵌 Maple Mono 字体缺少 OFL 许可与版权文本 | 待修 |
| 发布说明哈希与当前 dist 及打包证据不一致 | 待修 |
| 许可审计根包名与实际 workspace 包名不一致 | 待修 |
| 手工 THIRD-PARTY-NOTICES 缺版权行且引用不存在路径 | 待修 |
| dav1d 原生源从可变 tag 克隆，审计 commit 未进入构建 | 待修 |
| 构建新鲜度摘要漏掉非 .rs/.toml 构建输入 | 待修 |
| 随包 ConPTY 运行时的校验与实际嵌入二进制脱节 | 待修 |
| release.ps1 的钉测试只执行体积与 profile | 待修 |
| “清单即合同”没有机械化全等检查 | 待修 |
| CI 不执行许可清单核验 | 待修 |
| CI 未按发布口径验证（无 --locked、无 release 构建、无 dumpbin） | 待修 |
| 架构门禁未使用 --base | 待修 |
| 版本校验与默认值跨脚本不一致 | 待修 |
| 发布产物无签名链 | 待修 |
| i18n 合同只约束目录占位符 | 待修 |
| 数学语料引用不存在的测试、主题与截图 | 待修 |

## 安全、凭据与依赖

| 问题 | 状态 |
|---|---|
| Git 读取未统一遵守有界执行、禁网与凭据提示约束 | 已修：`command` 统一设置 `GIT_TERMINAL_PROMPT=0`/`GIT_NO_LAZY_FETCH=1`/`GIT_ALLOW_PROTOCOL=file`；`head_file`/`verify_head`/`file_hunks` 改走有界带 deadline 执行体；测试 `every_git_command_disables_prompts_lazy_fetch_and_network` |
| 生产 TLS 栈命中 emSign 弃用中间证书公告 | 待修 |
| 加密备份 AAD 范围与恢复时序合同冲突 | 需裁决 |
| 依赖审计未进入 CI 常驻门禁 | 待修 |
| 诊断脱敏词表与备份敏感键表不一致 | 已修：脱敏键集合与备份敏感键表对齐，授权头额外吞掉方案词后的凭据；测试 `redaction_covers_credential_shapes` |
| 凭据明文副本没有全程清零 | 待修 |
| AI hook 解析在 debug 日志输出整条事件 | 已修：只记录 source/kind/pane，正文与 session_file 不进入日志 |
| 更新下载 .part 打开跟随预置链接 | 已修：先移除既有项再 `create_new` 独占创建 |
| 凭据 Win32 FFI 调用缺少逐块 SAFETY 不变量 | 已修：CredReadW/CredFree/CredWriteW/CredDeleteW 调用点补齐 SAFETY 不变量注释 |

## 测试质量、文档与计划证据

| 问题 | 状态 |
|---|---|
| 追溯矩阵引用不存在的测试例名 | 待修 |
| 部分矩阵例名存在，但覆盖的不是声明场景 | 待修 |
| 全量验收锚以源码字符串和局部状态代替跨域行为 | 待修 |
| 测试豁免表含失效设计路径、目标态待实现项与超范围无障碍项 | 待修 |
| 架构总览与常驻规则仍使用旧栈数据名、类型名与已取消的校验 | 待修 |
| README、CHANGELOG 与发布说明的用户能力清单停留在旧栈 | 待修 |
| 被排除的图片面板实现仍编译存在 | 待修 |
| featureSpec 未覆盖已实现的壁纸与屏幕检测 | 待修 |
| Mermaid 终态在 featureSpec、devPlan 与进度之间构成合同冲突 | 需裁决 |
| devPlan core crate 依赖方向与实际策略相反 | 待修 |
| devPlan Runtime 方法表写错 operation.get/wait | 待修 |
| 进度与追溯文档保留测试通过计数、过期状态锚与计划期自述 | 待修 |
| 模块与设计文档存在第二份源码式复述 | 待修 |
| Runtime v2 验收 hash 指向 Windows 阶段提交 | 待修 |
| “全部阶段验收完成”与安装器、发布包实机待办矛盾 | 待修 |
| 旧实现与最终排除面未清理 | 待修 |
| 进度表用同一提交充当多个阶段锚 | 待修 |
| 追溯矩阵存在同名测试与重复行 | 待修 |
| 计划行为切片没有可核销例名 | 待修 |

## 跨域补充

| 问题 | 状态 |
|---|---|
| 显式终端 cwd 只做词法包含校验，未解析真实目标 | 待修 |
| 工作区布局恢复没有未知 ViewKey 过滤与重复视图归一 | 已修：新增 `ProjectLayout::from_canonical_json_lossy`，恢复链过滤未知视图类型、重复视图保留首个、空组塌缩并重排树；测试 `lossy_recovery_filters_unknown_views_collapses_empty_groups_and_dedupes`、`startup_filters_unknown_and_duplicate_views` |
| AppRoot 后台 Agent 根加载不可注入，虚拟窗口用例读取真实 home | 待修 |
| 共享编辑（Diff/Markdown）缺少可见性门控与取消协议 | 待修 |
| 历史隔离到期清理、ZIP 导出与永久删除未实现 | 待修 |
| 脚本自测依赖共享 %TEMP% | 待修 |
| Git 测试夹具未隔离全局配置 | 待修 |
