# 审查问题修复状态

> 本文件只记录 `docs/review/` 台账问题的修复状态，防止重复修复。状态词：待修 / 修复中 / 已修（附提交 hash 与测试例名）/ 需裁决（合同冲突类，等待所有者选择一侧）。

## 应用壳、关闭与生命周期

| 问题 | 状态 |
|---|---|
| 交互式关闭没有保存/丢弃/取消决定入口 | 已修：新增 AppRoot 模态关闭决定面（保存／丢弃／取消），窗口关闭、组内关闭其他／全部、单面板关闭、项目移除与设置浮层关闭在存在最后引用脏 Document 或脏 Hooks 草稿时先取得显式处置，再回到既有 `start_application_close_with_decisions`／批量关闭／`remove_project_with_decisions` 链；Hooks 草稿只能丢弃或取消，保存仍须在 Hooks 页显式完成；新增 core `last_reference_dirty_documents` 与 `collect_document_close` 共用引用与脏态判据；测试 `application_close_saves_or_cancels_through_the_decision_surface`、`integration_last_reference_close`、`dirty_hooks_drafts_block_panel_close`、`dirty_hooks_drafts_block_overlay_close`、`project_remove_requires_hooks_drafts_resolved`、`last_reference_decisions_skip_shared_and_clean_documents` |
| Commit 永久删除确认把任意失败升级为绕过执行体的强删 | 已修：永久删除经 `plan_delete_permanent` + `execute` 复用同一身份/父目录/链接/授权复核；`RecycleUnavailable` 单独分类，只有回收站失败可升级确认（提交 cd140797）；测试 `permanent_retry_is_offered_only_for_recycle_unavailable`、`permanent_delete_never_bypasses_the_shared_operation_executor`、`recycle_unavailable_is_distinct_from_other_delete_failures` |
| 未暂存删除回滚删除任意同内容未跟踪文件 | 已修：镜像候选限定同父目录且位于项目写根内，删除改经 `plan_delete`/`execute` 复核身份与授权（提交 f87962bf）；测试 `unstaged_delete_rollback_never_removes_same_content_files_outside_the_parent`、`unstaged_delete_rollback_keeps_mirrors_outside_the_write_root`、`commit_unstaged_rename_rollback` |
| 文件树 watcher 未接线 | 已修：AppRoot 按已打开的 Explorer／Commit／AgentFiles 登记 `WatcherService` 根，侧栏不可见时暂停 OS watch；`FsPending::dirs` 经 200ms／1s 合并后回投，Explorer 只重读受影响目录（含父目录），Commit 重取状态，Agent 根重新同步；测试 `explorer_directory_events_refresh_the_visible_tree`、`explorer_watcher_follows_sidebar_visibility`、`watcher_burst_coalesces_for_two_hundred_milliseconds` |
| 外部冲突与另存为没有生产落点 | 已修（提交 de8ec095）：显式保存返回 Changed 时打开唯一冲突决定面（覆盖／另存为／重载磁盘／保留本地／取消）；覆盖复用原子保存链但跳过磁盘字节基线比较并复核真实身份与权限，成功后关闭冲突；重载经后台重读真实文件、core 强制替换并清空撤销历史；保留本地只清除冲突标记；另存为经 WindowsIntegration 在后台弹出原生 `IFileSaveDialog`，前台按 Registry 复核目标占用，写前再次解析授权目标，成功后同一 Document 重绑位置并广播全部引用视图；外部刷新回投的授权代次改读进程级单源。测试 `explicit_save_conflict_opens_decision_and_overwrite_writes`、`save_conflict_cancel_keeps_draft_and_disk`、`save_conflict_reload_discards_local_draft`、`save_as_writes_selected_target_and_rebinds`、`save_as_rejects_target_owned_by_open_document`、`explicit_overwrite_accepts_content_change_but_not_identity_replacement`、`conflict_reload_discards_local_draft_and_history`、`keep_local_after_conflict_preserves_dirty_text`、`successful_save_clears_external_conflict` |
| 恢复失败分支跳过局部关闭段，已收集决定被丢弃 | 已修：恢复失败会话复用 `ApplicationClose` 状态机，`persist_intent=false` 时仍执行 Save、StopPty、Cleanup 与全局 flush，只跳过持久关闭意图；测试 `recovery_failure_app_close_saves_documents_and_stops_terminals` |
| Hooks 草稿未进入应用退出与面板关闭守卫 | 已修：应用退出、单面板／批量关闭、项目移除与设置浮层关闭遇到脏草稿先进入统一关闭决定面（丢弃或取消，保存须在 Hooks 页显式完成）；面板/所有者销毁取消在途载入／保存，迟到回执按所有者存活复核且不复活草稿；Runtime 面板关闭映射 `decision_required`；测试 `dirty_hooks_drafts_block_panel_close`、`dirty_hooks_drafts_block_overlay_close`、`dirty_hooks_drafts_block_application_close`、`hooks_load_receipt_never_resurrects_a_destroyed_owner` |
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
| Commit/Explorer 数据根切换时在途状态读取被丢弃或错投 | 已修（提交 93ca5cf5）：两者在途读取期间的新请求只保留合并位，`finish_*_status` 统一匹配当前项目、根与 generation，旧回执丢弃后立即重投当前根；Explorer 运行态新增 generation 并随根切换推进，状态只写入同代次视图；测试 `commit_runtime_defers_reload_to_inflight_status_result`、`explorer_runtime_advances_generation_and_defers_status`、Commit／Explorer UI 既有用例 |
| 项目重定位与导航历史恢复没有生产入口 | 已修（提交 21df6df8、1f7b72d2、9c7cdeae）：导航项目行菜单接入重定位（后台原生目录选择与真实解析、junction 归一查重、Hooks 决定面丢弃／取消、项目快照事务回执后采纳、授权代次推进、取消与失败零改根）；生产启动链在 GPUI 前探测持久根（缺失→RelocationRequired、不可访问→Inaccessible、别名归一、真实重复根进诊断链，测试虚拟根走通用加载入口）；导航视图后台扫描 CLI 会话并按真实 cwd 词法归属，已归属行双击经统一项目/终端/注入链恢复，cwd 失效回退项目根，未归属行先显示目标根确认、确认后才建项目与终端；重定位、恢复 cwd/项目判定、注入超时与 Shell 分类均成为生产唯一入口；测试 `nav_project_menu_relocates_to_picked_real_directory`、`nav_project_menu_relocation_cancel_keeps_root`、`relocation_entry_discards_hooks_drafts_through_the_decision_surface`、`relocation_entry_cancel_keeps_root_and_drafts`、`root_status::tests`、`missing_project_root_is_marked_relocation_required_at_startup`、`nav_history_scan_attributes_sessions_to_projects`、`nav_history_restore_opens_terminal_with_resume_command`、`unowned_history_requires_explicit_confirmation`、`unowned_history_confirm_creates_project_and_terminal`、`backup_cross_machine_requires_relocation` |
| 启动恢复对缺失/不可解码的干净 Document 不建面板 | 已修：新增 `DocumentRecoveryKind::Unavailable` 占位与 typed 文案，未知 Document 不再 `continue` 丢面板，可由页签关闭入口移除；测试 `missing_clean_document_restores_unavailable_placeholder` |
| Agent 根路径变更（同 id）沿用旧 ViewState | 已修：`agent_files_sync` 把「同 id 不同路径」视为移除并重登记，旧持久分区、展开目录与当前行一并清理；测试改走真实同步入口 `agent_root_path_change_resets_state_and_authorization` |
| 授权代次在生产路径恒为零 | 已修（提交 55339b4a）：新增进程级 `app_shell::authorization` 代次单源，项目移除／重定位与 Agent 根变更推进代次；重命名与排序不改变根路径，不再误推进代次或打断在途保存；Document 保存、关闭保存、Explorer 重命名、Git 回滚/删除与各 PathPolicy 判定均读取真实代次，执行前重读最新值；测试 `project_relocation_advances_the_authorization_generation`、`project_rename_and_reorder_keep_the_authorization_generation`、`agent_root_path_change_resets_state_and_authorization` |
| GitShow 非法 UTF-8 的合同冲突 | 已裁决为显式编码错误：非法 UTF-8 不生成替换字符伪文本，只显示错误与系统打开；featureSpec 与 git 模块合同同步 |

## 设置、主题、导航与 Agent

| 问题 | 状态 |
|---|---|
| 设置中心页面集合与导航结构未按规格实现 | 修复中（提交 45a205c7、4032c0dd、689d67c9、91641392、e38ce399、fdc01262）：页面模型收敛为全局组（快捷键、后台定时任务、外观、终端、应用）与 Agent 组（基础配置、Hooks），导航分组／折叠／过滤与稳定 slug 选择器已实现；后台任务页接入注册表启停与热应用；页级与全局恢复默认（跳过授权类键）；快捷键页只读接入统一命令目录并按上下文分组、合并多绑定；导入／导出／预览确认接线且写盘成功才采纳运行值。剩余：快捷键录制／覆盖／冲突与 keybind 持久、后台任务周期文本编辑、页内视图参数与浮层／项目页区分、编辑器与文件树／补全键位并入目录 |
| 设置中心缺少导入、导出、备份、诊断、更新、Runtime、AgentRoot 与套餐凭据入口 | 已修：普通设置导入导出、加密备份（含恢复/撤回）、诊断导出与复制、分类清理、手动检查更新、Runtime 状态面板与套餐凭据/余量入口已接入设置「应用」页；AgentRoot 管理已接入 Agent 基础页（新增/改名/换目录/能力开关/删除，真实目录查重后经统一设置写通道落盘）；测试见相邻行与 `settings_application_page_offers_plan_balance_dialog`、`agent_basic_page_opens_agent_roots_dialog` 系列 |
| 自定义主题库没有设置界面，主题选择只列内置主题 | 已修：外观页页头打开主题库模态对话框，列表并列内置与自定义主题，点击行经 `theme`／`custom_theme` 双键切换；导入经后台原生文件选择与 `import_json_file`（失败零写入），改名经 `ThemeLibraryStore::rename` 的显示名冲突与修订检查，删除前当前主题必须先另选；对话框加载、导入、改名与删除全部后台执行，失败只显示封闭文案；测试 `settings_appearance_page_offers_theme_library_with_builtin_and_custom_rows`、`theme_library_selects_custom_theme_and_marks_it_active`、`theme_library_rejects_deleting_the_active_custom_theme`、`theme_library_deletes_inactive_custom_theme`、`theme_library_renames_custom_theme_and_refreshes_rows`、`theme_library_imports_picked_file_and_rejects_invalid_json`、`rename_rejects_name_conflicts_and_stale_revisions` |
| 主题文档仍强制承载排版、卡片几何、背景图与效果字段 | 已修：`ThemeDefinition` 删除 typography/layout/effects，文档只收颜色与 ANSI；旧区段按未知字段拒绝，应用桥只写主题键（提交 40b4a59b）；测试 `pure_color_documents_are_accepted_without_non_color_sections`、`generated_documents_carry_only_colors_and_ansi`、`legacy_non_color_sections_are_rejected_as_unknown_fields`、`applying_a_theme_writes_only_theme_keys_and_keeps_personal_appearance` |
| 最终排除项仍在设置页暴露 | 已修：设置值与注册表删除 `pane_card_*` 四项、内置主题卡几何与对应常量；密度改为紧凑／标准／宽松三档且默认标准；设置页字段测试直接遍历 `FIELDS` 断言排除项缺席与密度三档，验收源扫描新增 `pane_card_` 禁名；测试 `excluded_appearance_fields_are_absent_and_density_has_three_levels`、`acceptance_excluded_surface_absent` |
| WCAG 对比度校验与提示残留，合同互相矛盾 | 已修：删除 `validate_wcag`/阈值/建议色/`wcag_violations`/内置对比度测试与 i18n 承诺；源扫描守卫 `theme_sources_do_not_validate_contrast`，模块文档同步为“不做对比度校验或提示” |
| 设置页大量控件没有生产消费方 | 已修：默认 Shell 设置已接入启动链（设置值参与真实探测，失效回退默认链）与运行期热应用（变更后后台解析、成功才替换，新建终端使用新 Shell）；`startup_directory` 在无 CLI 根且目录真实存在时作为首个项目根，失效回退当前目录；`accept` 门控 ghost 接受键、`bell` 独立门控视觉通知与系统响声；`background` 背景覆盖色进入终端调色板；`refresh_environment`／`terminal_proxy`／`conpty_input_modes` 经 `LocalOptionsPolicy` 进入 PTY 启动（见「运行时设置未作用到终端核心」）；无消费点的旧上游键（density、cell_width_mode、tab_reveal、tab_close_visible、cjk_bold_regular、vcs_display、font_family_cjk、ui_font_family、ui_font_size、panel_resize、ctrl_wheel_font_zoom、new_tab_position、focus_follows_mouse、dim_inactive_panes、keep_session、restore_session、fetch、ai_toasts）从设置页退休，注册表保留旧值以兼容导入；编辑器字号由主题 `mono_font_size` 令牌驱动、光标闪烁已由 `cursor_blink` 显式值与系统动效偏好消费；测试 `startup_shell_follows_settings_and_restores_original_file`、`settings_shell_hot_applies_to_future_terminals`、`startup_directory_overrides_only_without_cli_root_and_when_present`、`completion_accept_key_right_mode_rejects_tab_acceptance`、`completion_accept_key_right_mode_accepts_right`、`bell_mode_none_suppresses_the_notification_funnel`、`retired_and_excluded_fields_are_absent_from_the_settings_table`、`runtime_values_feed_the_shell_view`、`disabled_refresh_skips_the_environment_snapshot`、`policy_injects_conpty_modes_and_system_proxy_environment` |
| 设置保存状态与失败不可见；文本/颜色控件跨视图不同步且缺少路径校验 | 已修：文本/颜色控件按 `AppRoot.settings_revision` 回填，聚焦中不覆盖、切页强制回填并清非法草稿；状态条显示保存中／已保存／失败并提供显式重试（`SettingsStore::retry_pending`）；`shell`／`startup_directory`／`background_image` 先查绝对路径与扩展名，再经后台真实目标校验，非法值不写运行值；同时修复 FocusManager 把未注册子控件焦点误恢复导致输入框不可键入（提交 3403028f）；测试 `settings_status_reports_saving_then_saved`、`settings_save_failure_offers_explicit_retry`、`settings_text_inputs_follow_runtime_and_page_switch_clears_invalid_text`、`settings_path_fields_require_existing_absolute_targets`、`focus_sync_keeps_focus_inside_rendered_child_controls`、`retry_pending_makes_the_failed_batch_due_again` |
| 根重定位实现为 dead code，缺少真实路径查重与决定守卫 | 已修（提交 21df6df8）：导航项目行右键菜单接入唯一重定位命令，后台经 `WindowsIntegration::pick_folder` 选择目录并由 `ProjectPathResolver` 做真实目录校验与 junction 归一查重；脏 Hooks Project／Local 草稿经唯一决定面丢弃或取消（保存仍归 Hooks 页），有 writer 时等 `commit_project_snapshot` 事务回执成功后才改根并推进授权代次，失败、取消与回执丢失均保持原根；测试 `nav_project_menu_relocates_to_picked_real_directory`、`nav_project_menu_relocation_cancel_keeps_root`、`relocation_entry_discards_hooks_drafts_through_the_decision_surface`、`relocation_entry_cancel_keeps_root_and_drafts`、`root_status::tests` |
| 重复 Settings 恢复不按规格保留首个并提示 | 已修：宽容解码按首次出现保留重复视图，整库不再因重复 Settings 解码失败；恢复警告计数进入 `AppRoot` 首帧后的聚合提示（`notify_workspace_repairs`，只含计数、不含路径）；测试 `lossy_recovery_filters_unknown_views_collapses_empty_groups_and_dedupes`、`startup_filters_unknown_and_duplicate_views`、`workspace_repair_notice_states_the_skipped_item_count` |
| 主题下拉选择不清除 custom_theme | 已修：`theme_field_followups` 在选择内置主题时清除 `custom_theme`；测试 `theme_field_followups_clear_only_for_builtin_theme_selection`、`builtin_theme_selection_clears_custom_theme_override` |
| Runtime v2 快照没有设置界面，快照测试未覆盖资源边界 | 已修：设置「应用」页新增 Runtime 只读面板，展示控制面可用性与发现记录中的非敏感端点（切页时后台刷新）；快照资源边界补齐空工作区、多项目多组与隐藏项目、Document 资源、无 worker 退出终端代次为零、非 Unicode 路径编码失败；测试 `settings_application_page_shows_runtime_status_and_endpoint`、`runtime_snapshot_empty_workspace_has_no_resources`、`runtime_snapshot_covers_hidden_projects_and_split_groups`、`runtime_snapshot_lists_open_documents`、`runtime_snapshot_reports_zero_generation_for_exited_terminals`、`snapshot_path_encoding_rejects_non_unicode_paths` |
| 测试主要覆盖引擎函数与 DTO，未覆盖页面场景 | 待修 |
| Hooks 页只实现 Claude JSON 子集，缺少 GUI、事件目录、外部监听、托管只读与保存提示 | 待修 |
| 浮层不持久化 ViewState，失效页回退未接线 | 已修：浮层注入应用级 `SettingsViewStateStore`，启动恢复按 `ViewKey::AppSettingsOverlay` 预载；构造时经 `SettingsViewState::restore` 做可用页回退并写回，命中回退经 typed 通知提示；浮层 Hooks 已由 Overlay 草稿键承接（见 Hooks 草稿行）；测试 `settings_overlay_page_survives_close_reopen_and_restart`、`settings_page_view_state_restore` |
| 真实目录查重漏掉内置根，生产路径绕过 Registry | 已修（提交 a44e20df）：`AgentRootRegistry` 内置根纳入真实目录查重与能力并集（重复 builtin 收敛为单根，自定义别名冲突拒绝）；生产后台加载用 `build_agent_root_registry` 构建同一 registry，树与授权只按已解析真实目录计算，加载／解析／冲突错误作为显式 `agent_roots_error` 保留并只保留内置根，不再 `unwrap_or_default` 静默清空；测试 `builtin_roots_participate_in_real_path_deduplication`、`production_registry_keeps_builtin_roots_when_custom_root_duplicates`、`agent_root_real_directory_dedup`、`agent_files_open_reuses_document` |
| 改路径沿用旧根能力 | 已修：`AgentRoot::with_path` 把新根重置为默认只读；测试 `agent_root_path_change_requires_new_identity`、`agent_root_path_change_service_semantics` |
| agent.start 注入 slug 而不是 AgentCatalog 内置命令 | 已修：改注入 `kind.catalog().builtin_command`；测试 `runtime_agent_start_injects_catalog_builtin_command` |
| Claude 无原生会话 ID 时仍生成 `--continue` | 已修：无原生会话 ID 一律返回 None；测试 `resume_commands_require_catalog_capability_and_safe_identity` |
| 撤销状态在恢复成功前被置为 Undone | 已修：`check_undo` 只做只读预检，`undo_batch` 合并文件恢复与状态推进，成功才置 `Undone`；阻断或 I/O 失败进入 `NeedsAttention`，保留材料、不限原窗口重试且跨重启保留，`expired` 永不收割待处理批次；测试 `quarantine_undo_rechecks_deadline`、`quarantine_undo_failure_state_survives_restart_and_allows_retry`、`quarantine_undo_rejects_external_target_at_file_level` |
| quarantine_batch 不先持久化清单，部分失败丢失材料身份 | 已修（提交 21bf51da）：`quarantine_batch_with` 经可注入 I/O 端口先原子写入不可变清单（版本／批次／顺序／来源与副本路径／指纹），全部副本校验通过后才逐项删除来源；任一副本失败不删除任何来源，部分删除返回逐项阶段与已产生副本，清单可经 `load_manifest` 由启动对账读取。测试 `quarantine_manifest_is_persisted_before_touching_sources`、`quarantine_manifest_failure_touches_no_source`、`quarantine_copy_failure_keeps_all_sources_and_reports_items`、`quarantine_remove_failure_reports_completed_and_failed_items`、`quarantine_manifest_round_trips_for_startup_reconciliation` |
| 主题身份不是 ThemeId/UUIDv7，导入冲突被自动改名 | 已修：自定义主题身份改为 `ThemeId`（UUIDv7），显示名与身份分离；内置主题无可持久身份；同身份/同名导入明确拒绝且零写入（提交 de837f97）；测试 `theme_identity_is_uuid_v7_and_independent_from_display_name`、`import_rejects_a_display_name_conflict_without_writing`、`import_rejects_an_identity_that_already_exists`、`fork_assigns_a_uuid_v7_identity_independent_from_the_name` |
| ANSI 16–255 被持久化成第二份色表 | 已修：主题只保存 ANSI 0–15，16–255 统一经 `xterm_indexed_color` 派生，文档不再写 `terminal.indexed`（提交 997bdcc6）；测试 `extended_indexed_colors_are_derived_not_persisted`、`indexed_colors_are_derived_from_the_single_xterm_formula`、`resolve_indexed_falls_back_to_cube_and_grayscale` |
| 设置损坏分流缺“高版本”和“权限/IO 只读重试” | 已修：设置文件带 `settings_version`，高版本回退默认值且写通道拒绝覆盖；权限/锁/IO 读取失败保留启动可用性但以 `ReadFailed` 警示，设置页提供后台重试入口，重读值先铺底、本轮待写编辑覆盖其上，写通道仍从磁盘读-合并-写且读失败即拒绝，不覆盖原文件；测试 `higher_settings_version_is_read_only_and_never_overwritten`、`settings_writes_stamp_the_current_version`、`settings_read_failure_offers_retry_and_adopts_loaded_values`、`adopting_loaded_settings_preserves_pending_edits` |
| 设置写失败、损坏与单字段回退没有用户可见反馈 | 已修：启动设置只读一次并把损坏／高版本／读取失败写入 `SettingsLoadState` 全局；设置页聚合加载降级、单字段 `Invalid` 回退计数与持久的写失败标记，渲染可见状态条；`save_failed` 文案明确本次运行生效、重启可能丢失；测试 `settings_status_surfaces_load_fallback_and_save_failure`、`higher_settings_version_is_read_only_and_never_overwritten` |
| 恢复默认旁路统一设置写通道、审计与保存锁 | 已修：`settings_store::restore_default_settings_at` 共用保存锁、审计事件与 1MB 上限；测试 `settings_import_and_reset_preserve_secrets` |
| AgentCatalog 适配器声明未被消费，CLI 名单多源 | 部分修复：`hooks_adapter` 与 `builtin_command` 已有生产消费（设置页 Hooks 名单与 `agents()` 均由 catalog 派生，启动命令走 catalog）；`AgentHook` 增加 slug/label 并对齐集合；测试 `catalog_hooks_match_the_settings_hook_enum`、`agent_row_states_follow_catalog_order_not_all_order`。`history_adapter` 仍待历史扫描生产入口接线 |
| Hooks 事件目录远小于 featureSpec 的 Claude 事件集合 | 已修：`HookEvent` 扩为 31 项全表，编辑器校验与 `hook_events()` 使用 `CLAUDE_EVENT_CATALOG`，安装器仍只注入 10 项订阅子集；测试 `legal_catalog_events_save_even_when_not_installed`、`claude_event_catalog_covers_the_full_feature_spec`（提交 21eb93f7） |
| 设置热应用在 GPUI 前台同步读取自定义主题文件 | 已修：主题定义预载缓存 + `ThemeLoad` 后台任务，启动/显式提交走冷路径读盘，字段提交后的快照重建只查缓存，库写入/删除主动失效（提交 b83926fe）；测试 `hot_snapshot_uses_the_cached_custom_theme_definition_without_disk_io`、`custom_theme_hot_apply_uses_the_background_preloaded_cache`、`cache_hits_only_the_installed_identity_and_clears_on_invalidate` |
| 套餐凭据目标不含本地根稳定散列 | 已修：`credential_target(local_root)` 绑定 `stable_path_hash`；测试 `plan_balance_targets_are_scoped_to_the_local_data_root` |
| 普通设置导入、导出与恢复默认只有内存纯函数 | 已修（提交 e38ce399、fdc01262）：导出经白名单投影生成版本化信封并由后台原生目标选择后原子写；导入后台选择文件、1 MiB 上限读取、解析与版本／登记／导出资格／字段校验，预览确认后经设置存储原子写与 `.bak` 备份落盘，写盘成功才采纳运行值；恢复本页／全部默认接入生产 UI 并同步平台键。测试 `export_json_round_trips_and_rejects_malformed_documents`、`settings_export_writes_versioned_envelope`、`settings_import_previews_and_applies_after_write`、`settings_page_reset_restores_field_defaults`、`settings_restore_all_resets_fields_and_tasks` |
| Agent 基础配置、AgentRoot 管理与 Agent 文件操作面未实现 | 修复中：Agent 基础页新增 AgentRoot 管理对话框，自定义根的新增、改名、换目录、写／终端能力开关与删除都经 `validate_agent_roots` 真实目录查重与统一设置写盘，成功后才采纳运行值并同步文件树；测试 `agent_basic_page_opens_agent_roots_dialog`、`agent_roots_dialog_renames_root_after_save`、`agent_roots_dialog_capability_toggle_advances_authorization`、`agent_roots_dialog_deletes_root`、`agent_roots_dialog_adds_root_and_rejects_duplicate`、`validation_reports_duplicate_kinds_for_edit_surface`。剩余：CLI 别名/展示模式等基础配置字段与 Agent 文件树操作面 |

## Hooks 与 hook 桥

| 问题 | 状态 |
|---|---|
| 生产环境没有 hook 事件入口 | 已修：启动链创建 `PipeServer` 并设置管道/helper 环境，泵线程把有界批次投为 `AiHookEvents` 壳事件；AppRoot 门控后经 `AgentHub` 仲裁、通知漏斗与快照会话身份提交，窗口结束后停管道并 join 泵线程；测试 `ai_hook_events_require_pane_process_ownership` |
| 打开 Hooks 页留下无法清除的草稿，使项目移除/根重定位永久报 HooksDraftPending | 已修：`project_has_drafts` 只统计绑定该项目且 `dirty()` 的草稿，干净草稿不阻塞（提交 bbf3f69b）；测试 `hooks_project_guard_ignores_user_layer_and_clears_project_drafts`、`navigation_relocation_guards_and_reassigns_history`、`project_remove_requires_hooks_drafts_resolved` |
| 管道事件不校验 pane 归属与 PTY 代次 | 已修：前台把内核 `client_pid` 与目标 pane 当前 PTY 的进程身份做进程树归属校验，未证明归属的事件只记丢弃计数；旧代次进程树不再连接到新 PTY，迟到事件自然失效；测试 `descendant_check_binds_the_client_to_the_pane_process_tree`、`ai_hook_events_require_pane_process_ownership` |
| 注入/卸载没有 Journal 与启动恢复器 | 待修 |
| 危险命令确认没有生产链路 | 待修 |
| GUI 编辑模式与事件树/handler 表单不存在 | 待修 |
| Hooks 编辑保存绕过审计与 PathPolicy | 已修：新增 `hook_editor_write` 审计包装（主文件与备份均审计，提交 eb1c71e4）；保存前经真实规范位置包含校验与 `HooksWrite` 精确文件 grant 的 `PathPolicy` 判定（提交 3346cdf9）；测试 `saving_hooks_emits_audit_event_without_content`、`hook_editor_write_emits_audit_event_without_content`、`save_rejects_target_outside_the_authorized_root_without_writing` |
| 加载路径不检测重复键 | 已修：加载/保存重读共用严格解析，重复键拒绝编辑；测试 `load_rejects_duplicate_keys_instead_of_silently_overwriting` |
| config_guard 没有生产调用点 | 已修：`run_shell_once` 在通知初始化后启动 `spawn_config_guard_with`，守卫随 `application.run` 生命周期持有并在 Drop join；失败类别经 `AiHookAlert` 壳事件回前台按 typed 文案呈现（`ai_hook_alert_uses_display_name_and_no_synthetic_command`）；豁免表改为真实 CLI 配置与 watcher 的人工验收边界 |
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
| 可编辑 Document 缺行尾切换、编码重开与手动只读活体通道 | 已修：core 提供 `set_line_ending`／`set_manual_read_only` 与 `saved_encoding` 基线，Editor 底栏按钮经统一事件写入共享 Document 并广播只读/编码元数据；不可解码打开失败进入唯一编码选择面，选定编码后复用同一读取链重开（可编辑与分页均覆盖），二进制内容不进入该面；测试 `line_ending_change_is_a_revertible_document_modification`、`manual_read_only_requires_clean_editable_text_and_blocks_edits`、`editor_line_ending_and_manual_read_only_are_live`、`unknown_encoding_prompts_for_explicit_reopen`、`wrong_encoding_keeps_prompt_and_cancel_leaves_layout_unchanged`、`binary_content_is_rejected_without_encoding_prompt`、`oversized_gbk_file_reopens_as_paged_through_explicit_encoding` |
| 查找与替换只实现字面量查找和全部替换，且没有生产接线 | 待修 |
| 本地图片路径在授权前被解析，UNC 可自动触发 SMB 认证 | 已修：`PathGrant::lexically_covers` 纯词法预检先于解析，未授权目标（含 UNC）不触碰文件系统；测试 `lexical_precheck_covers_subtree_and_exact_scope_without_io`、`authorization_precedes_resolution_and_unc_never_resolves_without_grant` |
| 行内公式与行内图片被移出文本流 | 待修 |
| 引用、列表与脚注内的块结构被压平 | 待修 |
| 脚注引用不渲染、不可点击 | 待修 |
| 安全 HTML 白名单只做文本降级 | 待修 |
| 程序化滚动与恢复位置不走视口调度 | 已修：`scroll_to_block`／锚点／片段跳转和解析恢复都在同一状态转换内调用 `schedule_block_window` 显式调度目标窗口，键盘滚动按当前顶部调度；测试 `markdown_programmatic_jump_and_restore_schedule_target_blocks` |
| 每次滚动清空全部已渲染 outcome | 已修：同一 Document revision 的 `outcomes`/`inflight` 保留且渲染代次不推进，滚动只补排未渲染块；测试 `same_revision_scroll_keeps_rendered_outcomes` |
| 高 DPI 下公式位图被放大后再按 1:1 逻辑像素显示 | 已修：`logical_formula_size` 按窗口 scale 除回逻辑尺寸，块级与行内公式均设显式逻辑宽度（zoom 保留在位图内）；测试 `formula_logical_size_undoes_window_scale_but_keeps_reading_zoom` |
| 已加载的远程图片再次点击仍发起新请求 | 已修：已加载或有在途任务时重复点击不再重启请求；测试 `repeated_image_click_does_not_restart_requests` |
| 渲染查找在前台按匹配数乘块数扫描 | 已修：`find_literal` 用单向前进的块游标把匹配映射到所属块；Markdown 搜索移到后台执行，回执按查询与代次对账，过期结果不覆盖当前输入；测试 `literal_search_maps_each_match_to_its_own_block`、`markdown_rendered_search_navigates_visible_matches` |
| 每次滚动在前台复制整篇文档文本 | 已修：`ProgressiveMarkdownRenderer::schedule` 接收 `&Rope`，按块 `source_range.try_slice` 只取可见块文本；测试 `same_revision_scroll_keeps_rendered_outcomes` 及渐进渲染组 |
| 大纲无层级且常驻挤占内容区 | 已修：大纲按 `MarkdownHeading::depth` 逐级缩进，同级标题对齐；大纲改为贴靠正文右侧的绝对定位浮层，不再作为内容行兄弟列分配宽度；测试 `markdown_outline_is_hierarchical_overlay_without_shrinking_content` |
| 渲染态缺少键盘滚动、大纲与链接激活 | 已修：新增 `Ctrl+Shift+O` 大纲开关、方向键／PageUp／PageDown／Home／End 渲染滚动、`Alt+↑`／`Alt+↓` 链接焦点循环与 `Alt+Enter` 激活；焦点链接以选区色标识，大纲按钮经 Tab 可达并在工具提示展示快捷键；测试 `markdown_render_mode_is_keyboard_operable` |
| 安全与授权关键路径只有服务/投影测试 | 待修 |
| Editor 面板规定能力未实现 | 待修 |
| 本地编码检测与编码重开在实际打开链中不存在 | 已修：保留 core「显式编码」决策，真实打开链在未知/非法编码失败时打开唯一编码选择面，用户选定后经同一 `DocumentOpenService::load` 以显式编码重开，可编辑与超限分页分支都覆盖；二进制内容不提供编码重开。测试 `unknown_encoding_prompts_for_explicit_reopen`、`wrong_encoding_keeps_prompt_and_cancel_leaves_layout_unchanged`、`binary_content_is_rejected_without_encoding_prompt`、`oversized_gbk_file_reopens_as_paged_through_explicit_encoding` |
| 追溯矩阵的自动化证据层级与场景不一致 | 待修 |
| 分页只读视图不实现换行，且与软换行 Editor 共用滚动坐标单位 | 已修（坐标单位）并正式延期分页只读面：`EditorViewState` 新增必填 `scroll_unit`，Editor 与 Paged 均持久化逻辑行；Editor 软换行下由索引在写入时换算显示行→逻辑行，恢复时把逻辑行挂为待揭示目标并在索引完成后换算回显示行；Paged 只接受逻辑行，无法换算的显示行记录退回顶部且不反写。Paged 软换行、行号、查找、折叠与高亮降级以「无有界全文显示行索引」为根因正式延期，解除条件与边界登记在 `text_view/CLAUDE.md`（提交 b093fef0）；测试 `soft_wrap_logical_scroll_is_revealed_after_index_build`、`paged_rejects_display_row_scroll_units`、`paged_view_restores_scroll_state_without_transients`、`editor_view_state_restore`、`editor_wrap_toggle_before_index_ready_persists_safe_origin` |
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
| 重连期间旧 worker 仍可注入退出事件或显示命令 | 已修：`Term` 增加网格显示代次，`begin_reconnect` 递增；旧 worker 在循环、显示命令、resize、PTY 解析与子进程退出边界比对自己的代次，失配即停手；退出事件改经 worker 创建时捕获的代理发送，不再走被替换的 `Term` 代理；测试 `stale_worker_after_reconnect_cannot_mutate_grid_or_reach_new_proxy` |
| 帧唤醒标记与最新帧存储存在丢失竞态 | 已修：最新帧与唤醒位合并为同一 `WakeState` 锁，接收端取帧时原子清除标记，发送端新帧可重新入队；测试 `frame_written_between_take_and_ack_is_not_lost`、`frame_events_coalesce_to_latest_without_losing_semantic_events` |
| 子进程退出时未排空已读输出 | 已修：本地启动开启 `drain_on_exit`，退出边界循环排空队列后才发布最终帧；测试 `local_options_drain_output_before_exit`、`child_exit_drains_queued_output_before_final_frame` |
| 读取线程 EOF 或读取错误不会转成退出/故障事件 | 已修：`EventedPty::output_closed` 暴露读取端关闭，宽限 2 秒内无子进程退出事件即报告 `PtyFailure`；测试 `reader_reports_output_closed_after_eof`、`output_closed_without_child_exit_reports_transport_failure` |
| OSC 原始缓冲无界，嗅探上限在分发后才生效 | 已修：`OscGate` 在进入 VT 解析器前把 OSC 负载限制在 24 MiB（覆盖内联图片 base64 上界），超限丢弃负载、保留终止符；测试 `osc_gate_bounds_unterminated_payload_and_recovers_after_terminator`、`osc_gate_keeps_st_terminator_across_chunk_boundaries` |
| 运行时设置未作用到终端核心 | 已修：`scrollback_lines` 进入 `term_config`，字号默认 14 且注册表只接受 8–32 整数；光标与外观热应用见相邻行；shell 与 `startup_directory` 进入启动链；`refresh_environment`／`terminal_proxy`／`conpty_input_modes` 经 `LocalOptionsPolicy` 进入 `local_options` 的新建与重连两条启动路径，系统代理经 `platform::system_proxy` 单点解析后注入子进程环境；`conpty_input_modes` 域外段由 `slterm_settings` 逐字段宽容解析；测试 `local_options_drain_output_before_exit`、`disabled_refresh_skips_the_environment_snapshot`、`policy_injects_conpty_modes_and_system_proxy_environment`、`conpty_input_modes_segment_is_parsed_field_by_field`、`system_proxy` 解析与 `update_proxy_tests` 全量 |
| 搜索的帧驱动重查会重置导航位置并形成搜索风暴 | 已修：新帧刷新保留选中项且不滚动，在途请求跳过重排；测试 `frame_driven_search_refresh_preserves_selection`（提交 ac25b809） |
| 退出后键盘复制、搜索与回滚全部失效 | 已修：退出后保留复制/搜索/回滚，粘贴与编码路径关闭；测试 `exited_terminal_keeps_copy_and_scroll_but_rejects_input` |
| 一次瞬态操作错误会替换并永久遮住终端画面 | 已修：超限粘贴与 Agent 注入失败的 `notice` 瞬态提示不再替换终端元素，可点击关闭（提交 ba269cdb）；测试 `oversized_paste_keeps_terminal_visible_with_dismissible_notice`、`dangerous_paste_confirmation_does_not_leak_or_write_early` |
| Ctrl+Enter 未发送 LF，行镜像把修饰 Enter 当作提交 | 已修：`encode_for_program` 对 Ctrl+Enter 恒发 LF，行镜像 control 分支已使镜像失效；测试 `ctrl_enter_always_sends_lf` |
| 键盘选区扩展未实现，Shift+PageUp/PageDown/Home/End 语义相反 | 已修：`SelectionCommand::Extend(SelectionMotion)` 由 worker 按光标/既有锚点移动端点，方向键、翻页与 Home/End 不再编码进 PTY；端点自动滚入视口，备用屏夹在当前画面；测试 `keyboard_extension_moves_endpoint_with_inclusive_cells`、`keyboard_extension_in_alt_screen_stays_in_current_screen`、`page_extension_scrolls_endpoint_into_view`、`shift_arrow_extends_selection_without_pty_write`、`shift_pageup_extends_selection_into_scrollback`、`shift_paging_extends_selection_and_alt_screen_stays_local` |
| Ctrl+Shift+A 同时被终端全选与侧栏 Agent 文件占用 | 已修：终端独占 `Ctrl+Shift+A` 全选（`SelectionCommand::SelectAll` 覆盖回滚与屏幕，尾部空白不产生尾随换行，零 PTY 写入）；侧栏 Agent 文件改绑 `Ctrl+Shift+I`，命令目录新增保留键断言；测试 `select_all_covers_scrollback_and_screen`、`ctrl_shift_a_selects_all_terminal_text_without_pty_write`、`side_view_shortcut_bindings_cover_all_registered_views` |
| 光标形状、闪烁与相关设置不符合行为真值 | 已修：终端 Config 固定 `CursorShape::Beam`，闪烁取 `cursor_blink` 显式值或系统减少动效的相反值；启动链在设置快照前写入动画偏好；形状与平滑移动控件及文案移除，旧键保留但不再暴露；测试 `cursor_blink_prefers_the_explicit_setting_over_reduced_motion`、`term_config_uses_a_beam_cursor_with_the_effective_blink`、`every_setting_field_in_displayed_domains_has_exactly_one_control` |
| 主题、字体、字号、连字与复制/补全外观设置不热应用 | 已修：`reload_settings_snapshot` 向全部项目（含隐藏）的既有终端扇出 `apply_settings`；字号/字体/连字更新并触发结构 resize，调色板替换后按亮暗翻转发送 `SetColorScheme`，复制/补全/ghost 字段同步；测试 `terminal_views_hot_apply_settings_snapshot` |
| 真实 ConPTY 验收场景覆盖不足 | 待修 |
| 测试语义与生产路径断裂 | 待修 |
| 项目脚本与 workspace 有界预算缺少真实测试 | 已修：超限夹具改为超过 1 MiB 的合法 JSON 并断言其可解析，证明降级来自字节预算；workspace 发现抽出可注入 `WorkspaceBudget` 与时钟，逐维验证字节／目录／包／截止时间超限均降级为空且中途到期不回填，生产常量由用例锁定；删除无调用者的 `finish_catalog`。测试 `oversized_manifest_and_relative_cwd_fail_closed`、`workspace_budget_limits_degrade_to_empty`、`workspace_deadline_is_checked_mid_scan`、`production_budget_uses_declared_limits` |
| 可信边界与旧代丢弃的验收测试不足 | 修复中：提交命令现在显式取消在途补全并清空候选，测试 `commit_line_cancels_inflight_completion`（移除取消即失败）；备用屏可信边界、旧 key/revision 迟到结果、提示符代次变化取消与 `Request` 代次身份仍需补测／对齐 |

## 持久化、崩溃恢复与加密备份

| 问题 | 状态 |
|---|---|
| Document 崩溃快照只有恢复器，没有生产写入与删除链路 | 已修：启动链在 GPUI 外启动 `DocumentSnapshotWriter` 并把 client 与本次 `run_id` 注入 AppRoot；编辑事务落定后按会话脏态排队最新正文（保护集 = 当前打开且脏的 Document），保存清脏、撤销回干净基线与关闭 Save 结算后排队删除，关闭意图越过 `StateCommitted` 后按 Intent 删除最后引用快照，最终冲刷在窗口销毁后由启动作用域 shutdown；测试 `document_edits_write_and_clear_crash_snapshots` 及既有写线程/恢复器用例 |
| 加密备份没有工作区类别，跨机重定位与逻辑工作区导出不存在 | 已修：`BackupCategory::Workspace` 以 `workspace.json` 逻辑投影承载项目、规范布局 JSON、终端元数据、Document 位置、ViewState 持久 JSON、侧栏与按机器几何，导出不复制 SQLite 物理库；维护期经唯一快照写通道恢复且跨机几何不应用，根缺失由既有启动探测进入 `RelocationRequired`；测试 `projection_uses_canonical_dtos_without_physical_files`、`workspace_only_selection_excludes_settings_credentials_and_physical_files`、`staged_workspace_restore_replaces_projects_sidebar_and_geometry` |
| 加密备份没有生产入口、预览、原子导出、跨重启恢复点与撤回 | 已修：设置「应用」页提供加密备份对话框（类别选择、固定排除说明、口令校验）；导出经唯一写线程收集持久事实与运行态设置白名单投影，后台密封、选择目标并原子写；恢复选择文件、口令解密、类别预览与关闭预检后经唯一写线程登记维护请求，启动维护期按恢复点原值回滚工作区与文件；应用成功后回填恢复后指纹并保留 `Applied` 恢复点，对话框列出可撤回点并登记 `backup.undo`，维护期按指纹逐项撤回，冲突项保留当前内容与材料；测试 `settings_application_page_offers_backup_dialog`、`backup_export_rejects_short_passphrase_and_writes_archive`、`backup_restore_previews_and_stages_maintenance_request`、`backup_undo_lists_applied_point_and_stages_request`、`staged_undo_restores_originals_and_removes_recovery_point`、`staged_undo_keeps_conflicting_target_and_recovery_point`、`backup_cross_machine_workspace_requires_relocation_before_runtime` |
| stage_restore 把解密后的备份条目明文写盘 | 已修：staged 材料先序列化为 `Zeroizing` 明文再经 DPAPI CurrentUser 保护，磁盘与指纹只绑定密文；维护期解密后应用，解密失败阻断且不落盘；测试 `staged_restore_is_dpapi_protected_and_carries_no_plaintext` |
| 灾难副本从未生成与刷新 | 已修：`refresh_disaster_copy` 先 Online Backup 到新临时库再原子替换正式副本；正常关闭在最终冲刷成功后刷新，启动维护成功后刷新，失败只清理临时材料并保留旧副本；测试 `workspace_writer_shutdown_refreshes_the_disaster_copy` |
| 备份恢复在 store_epoch 轮换与 Applied 之间崩溃会永久阻断启动 | 已修：新增 `complete_maintenance_apply`，在同一 Immediate 事务内核对 run_id 与旧世代、轮换 `store_epoch` 并推进 `Applied`；任一冲突都不改写世代或状态；测试 `apply_completion_rotates_and_transitions_atomically`、`staged_restore_is_applied_at_startup_and_rotates_store_epoch` |
| 双损隔离部分失败或中断后，重试以空工作区正常启动 | 已修：隔离目录先写 `.incomplete` 标记再改名，空库打开成功后才移除；启动先完成遗留改名并返回 Isolated 通知，原位与隔离副本并存明确阻断；测试 `interrupted_isolation_is_completed_before_opening_the_empty_workspace`、`interrupted_isolation_with_duplicate_files_blocks_startup`、`workspace_unrecoverable_isolated` |
| 备份明文没有 64 MiB 上限 | 已修：密封与解封双向 64 MiB 硬上限，解封在派生密钥前拒绝超限包；测试 `archive_plaintext_is_bounded_to_64_mib` |
| 设置导出使用敏感键黑名单而不是登记字段白名单，Session 原文完全未过滤 | 已修：设置类别改为 `SettingsRegistry` 导出白名单投影，未知段/未登记/敏感字段在导出与导入复验两侧一致拒绝；删除 Session／Themes 类别，主题并入设置类别，Session 原文与学习历史不再存在入包通道；所有收集目标要求根内普通文件；测试 `collection_uses_registry_export_allowlist`、`import_validation_rejects_unknown_or_sensitive_settings`、`backup_excludes_local_workspace_and_credentials`、`collection_rejects_empty_selection_unparseable_or_non_file_settings` |
| 恢复点与暂存目录没有当前用户 ACL 限制 | 已修：新增 `atomic_file::create_dir_current_user`，恢复点目录/文件与 `maintenance/`、请求目录/staged 文件全部收敛为当前用户显式 DACL 并切断继承；测试 `current_user_directory_cuts_inheritance_and_has_one_ace`、`recovery_point_is_encrypted_and_round_trips`、`staged_restore_is_dpapi_protected_and_carries_no_plaintext`（真实 Win32 DACL 断言） |
| 暂存明文先于 owner 落盘，清理先清行后删文件 | 已修：暂存改为先登记 Prepared owner（指纹绑定 DPAPI 密文）再原子写材料，写失败经 `discard_prepared_maintenance_request` 撤销且只允许 Prepared；清理改为先删恢复点与 staged 材料、再清恢复点登记，删除失败保留阻断事实；测试 `discard_prepared_request_never_removes_started_material`、`staged_restore_is_dpapi_protected_and_carries_no_plaintext` |
| 即时恢复在指纹回写失败时已写盘却返回失败 | 已修：整条运行期即时恢复通道删除，落盘、恢复点采样与回滚只在启动维护期执行，不再存在写完目标后补记指纹的失败分支 |
| 撤回只返回单一错误，逐项结果不可见，成功后恢复点不失效 | 已随运行期即时撤回通道删除而消除；撤回产品能力仍待接线，实现时必须走维护请求并逐项回报、按结局失效材料（归入生产入口条目） |
| 备份恢复、应用、回滚与撤回没有审计/恢复日志，设置写绕过 settings_store | 已修：暂存、逐项应用、恢复点、提交、回滚与清理只经 `audit` 记录 operation/path/source/result；设置条目经 `settings_store` 合并写并保留本机未知字段，回滚设置经审计包装且受 1 MiB 上限；设置条目超过上限在归档校验期拒绝；测试 `backup_restore_emits_audit_events_for_each_security_write`、`settings_restore_merges_exported_values_and_preserves_local_fields`、`oversized_settings_entry_is_rejected_before_staging` |
| 审计成功事件在 Warn 级日志过滤下被丢弃 | 已修：`level_allowed` 让 audit target 恒放行；测试 `audit_records_bypass_the_warn_level_gate` |
| 主日志写入系统临时目录而不是本地数据根 | 已修：`log_file_path` 归本地数据根；测试 `log_file_lives_under_the_local_data_root` |
| 根外 Document 的会话重授权没有生产调用点 | 已修（提交 4cf56c76）：授权占位提供真实“重新授权”入口；前台只取记录位置与精确文件会话授权，真实读盘走后台，回执复核目标未偏离记录位置后经启动恢复器结算并一次替换该 Document 的全部授权占位，分页正文切专用占位，失败保留占位并可重试；测试 `outside_document_reauthorization_replaces_placeholder`、`failed_outside_document_reauthorization_keeps_placeholder`、`diff_authorization_placeholder_reauthorizes_into_real_views`、`integration_session_grant_restore` |
| 布局或快照领域恢复失败时静默以空工作区开窗 | 已修（提交 ae2b28e4）：快照记录位置与布局漂移不再让整次恢复失败，正文归同一 Document、重绑当前位置并按冲突恢复；`AppRootPersistence` 产出封闭 `DomainRecoveryFailure`（数据库/布局/快照），生产组合回收 writer 与转发线程后进入同一只读诊断与重试链，不再以空工作区开窗；测试 `snapshot_location_drift_restores_draft_with_conflict_instead_of_failing`、`project_recovery_failure_is_not_an_empty_workspace`、`abort_startup_stops_writer_and_joins_forwarder`、`diagnostic_messages_are_localized_and_sanitized` |
| 未完成关闭意图、NeedsAttention 维护请求与遗留恢复点只有阻断 | 已修（提交 cf6364a7）：诊断窗对五类维护阻断提供二次确认的显式放弃；先 Online Backup 整库到本地根 `recovery-abandon/`，归档失败零删除，再按阶段删除未完成 Journal／维护请求／恢复点／关闭意图／快照索引／历史隔离行（Journal 关联 Operation 只解绑，Completed 关闭历史保留），审计 `maintenance_abandon`；成功后关闭诊断窗并由外层重走同一启动链；测试 `abandon_unfinished_close_intent_archives_then_retries`、`close_intent_abandon_archives_whole_database_and_keeps_completed_history`、`maintenance_request_abandon_clears_requests_and_recovery_points`、`journal_abandon_removes_unfinished_journal_and_steps`、`snapshot_and_history_stages_clear_their_own_blocking_tables` |
| 更高版本数据库先以读写连接打开 | 已修：既有库先经 `SQLITE_OPEN_READ_ONLY` 探测版本，高版本直接保留只读连接（提交 c8290344）；测试 `higher_version_wal_database_is_opened_read_only_without_touching_data_files`、`workspace_higher_version_is_query_only_and_not_downgraded` |
| 设置 IO/权限失败回退默认值，设置文件没有高版本判定 | 已修：高版本判定与写拒绝已实现；IO/权限读取失败回退默认值只为保启动可用，`ReadFailed` 进入设置页警示并提供后台重试，重读成功采用磁盘值且保留待写编辑，读失败时写入通道不产生任何覆盖（见“设置损坏分流”行） |
| 追溯矩阵把函数级、未接线或不同场景的测试当成已自动化验收锚 | 待修 |
| 崩溃与边界测试缺少关键窗口 | 待修 |
| 快照存储运行期遇任意损坏文件即阻断全部新写入 | 已修：写入路径忽略旁路损坏文件，留待启动隔离；测试 `unrelated_corrupt_snapshot_does_not_block_writes` |
| 快照文件名与头身份不一致时被当作损坏 | 已修：扫描以信封头身份为准，写成功清除旧位置残留；测试 `snapshot_header_identity_wins_over_file_name` |
| snapshot_index 与历史隔离表没有生产写入 | 已修（提交 2271f990）：按审查允许的终态删除 `snapshot_index`、`quarantine_batches`、`quarantine_items` 三张无生产写入的表与 `MaintenanceStage::Snapshots`／`HistoryQuarantine` 空维护阶段，维护顺序收敛为 Journal、维护请求／恢复点、关闭意图；删表不影响既有级联与主键约束覆盖，i18n 诊断阶段键与 devPlan 设计表清单同步删除；历史隔离持久清单留待该功能接线时随真实写入点建立；测试 `workspace_text_primary_keys_reject_null`、`workspace_recovery_metadata_is_not_implicitly_cascaded`、`every_material_table_blocks_without_modifying_facts`、`stage_order_short_circuits_before_querying_later_tables` |
| 备份归档 DTO 接受未知字段，空类别集也能解封成功 | 已修：DTO 加 `deny_unknown_fields`，空类别集/零条目拒绝；测试 `archive_rejects_unknown_fields_and_empty_selection` |

## Windows 平台、更新、套餐、通知与 Runtime

| 问题 | 状态 |
|---|---|
| 更新链在运行态没有入口 | 已修：启动检查经壳事件进入统一提示面（下载、校验、安装预约、稍后提醒、跳过版本），设置「应用」页提供手动检查入口并按结果开提示或给应用内反馈；测试 `update_available_opens_prompt_and_skip_suppresses_version`、`manual_update_check_result_routes_to_prompt_and_notices`、`settings_application_page_offers_manual_update_check` |
| 套餐余量功能在运行态不存在 | 已修：AppRoot 构造期启动唯一 `PlanBalanceRuntime`（订阅 `PLAN_BALANCE`、前台 1 秒调度拍、按已配置且到期来源后台查询、凭据代次丢弃迟到回执）；设置「应用」页提供掩码 Token 保存/删除、手动刷新与两来源只读汇总，Token 只经凭据域与 `SecretBytes`；测试 `settings_application_page_offers_plan_balance_dialog`、`plan_balance_token_save_and_delete_round_trip`、`plan_balance_manual_refresh_fetches_configured_sources`、`plan_balance_rejects_empty_token`、`plan_balance_fetch_failure_is_closed` |
| 诊断与清理没有产品面 | 已修：设置「应用」页提供诊断对话框（默认环境、健康、脱敏日志尾与端点发现四类白名单；完整路径须显式勾选才加入并声明 `full-paths`；复制与原子导出共用同一 DTO，健康快照含自检状态）与清理对话框（旧日志、更新下载残留、超过 7 天且不被布局引用的快照三类候选；当前日志、当前下载会话与布局引用由所有者排除；普通缓存与审计日志分开确认，后台逐项执行并只经审计留痕）；测试 `settings_application_page_offers_diagnostics_dialog`、`diagnostics_export_respects_full_path_selection`、`cleanup_previews_and_removes_owned_candidates`、`cleanup_candidates_skip_current_log_and_unrelated_files`、`cleanup_candidates_skip_current_and_reserved_installers`、`stale_snapshot_candidates_skip_referenced_and_fresh_files` |
| 通知事件源与托盘快照没有接入 | 已修：生产启动链已持有 hook 命名管道服务与批次泵，`on_ai_hook_events` 经所有权门把接纳事件交给 `AgentHub::dispatch_lifecycle` 与通知漏斗；补齐 BEL／OSC 9 → `TerminalViewEvent::Notification` → 按 pane 投递唯一漏斗；`poll_tray` 由 AgentHub 与项目快照派生托盘清单并发布，应用级用例锁定快照内容（提交 2b7c468f）；测试 `bell_and_osc9_feed_the_notification_funnel`、`tray_snapshot_publishes_accepted_agent_sessions`、既有 hook 路由与通知漏斗用例 |
| 系统关机与注销不走统一关闭链 | 已修：主窗口经 `SetWindowSubclass` 拦截 `WM_QUERYENDSESSION`，返回 FALSE 取消本次会话结束并投递 `SessionEndRequested`，复用托盘退出的统一关闭链保存 Document／停止 PTY／冲刷终局状态；保存完成进程退出后由用户重新发起关机，关闭被用户取消时保持登录（提交 eaabf3e5）；原生投递登记人工验收；测试 `only_query_end_session_is_deferred_to_the_close_chain`、`session_end_uses_the_same_close_chain_as_tray_quit` |
| 通知前台状态从未写入 | 已修：`AppRoot::observe_focus` 经 `observe_window_activation` 把真实窗口激活态写入 `notify::set_foreground`，注册时先同步一次；测试 `window_activation_updates_notification_foreground` |
| 应用内错误不可见，错误类别不进入任务栏错误态 | 已修：`Notification::ProcessAlert` 增加 `AlertSeverity`，错误与警告先进入进程级应用内队列，桌面开关与前台门控只影响系统 toast／闪烁；AppRoot 事件泵排出后顶部去重展示，只有显式关闭才移除；`Error` 映射任务栏 `ErrorRaised`，`Info`／`Warning` 不再发送清除信号；运行期错误改走非启动汇总投递（提交 4dcb8999）；测试 `process_alert_error_maps_to_failure_and_taskbar_error_only`、`in_app_notices_survive_disabled_desktop_toasts`、`process_errors_stay_in_app_until_dismissed`、`process_alert_uses_the_brand_title_and_is_not_ai_sourced` |
| 任务栏聚合没有资源身份 | 已修：`TaskbarSignal` 携带 `PaneId`，`TaskbarState` 以集合分别维护 busy/attention/error，完成与清除只移除自身 pane；窗口聚焦只清全部等待态，错误保留；测试 `pane_completion_only_clears_its_own_signals`、`integration_notification_taskbar`、`clear_edges_only_touch_their_own_source_pane` |
| 更新预约在崩溃或知情放弃关闭时仍保留 | 已修：预约拆为 `install-pending.json` 待定态与 `install-next.json` 确认态；`arm` 只写待定，统一关闭 `Completed` 才原子提升，Abandoned／失败／无 writer／取消删除待定，启动清理待定且只消费确认；测试 `update_install_uses_close_guard`、`pending_update_reservation_is_not_consumed_at_startup` |
| 启动安装依赖已退役的 session.json，恢复票没有消费方 | 已修（提交 9167eb0b）：update handoff 的 commit 不再保存 `workspace.json`，`apply_scheduled` 不再以 `session.json` 作为启用安装的前置条件；删除无生产消费方的 restore ticket/acknowledge 与 `SLTERM_UPDATE_RESTORE` 注入，工作区由新版本启动链从 `workspace.db` 恢复；测试 `integration_update`、`handoff_file_machine_commits_and_cancels_without_installation_authority`、`pending_update_reservation_is_not_consumed_at_startup` |
| 托盘可用性判定早于建窗，失败后没有恢复节拍 | 已修（提交 bfbe7f28）：可用性改为「已启用 ∧ 托盘窗口已创建 ∧ 图标 `NIM_ADD` 成功」三态，建窗/挂载失败或线程退出都会复位；静默启动在端口有界等待真实就绪，超时回退可见窗口；壳事件泵约 1 Hz 派生 `TrayAgent` 快照并发布，启用态每次节拍重投挂载请求，成为真实重试者；测试 `availability_requires_a_mounted_icon`、`enabling_the_tray_reposts_mount_requests_until_mounted`、`manual_launch_never_starts_hidden`、`silent_start_falls_back_to_a_visible_window_without_a_usable_tray` |
| 合同冲突：静默启动的触发范围 | 已裁决为仅登录自启：CLI 增加隐藏 `--autostart` 标记，安装器启动项与 `set_launch_at_login` 写入该参数；手工启动即使设置开启也显示窗口；测试 `autostart_flag_is_the_only_silent_start_intent`、`manual_launch_never_starts_hidden`、`silent_start_falls_back_to_a_visible_window_without_a_usable_tray` |
| 诊断脱敏不覆盖真实 Runtime token 形态 | 已修：`redact` 增加恰好 64 位小写 hex 的裸凭据识别（更长 hex 段不截断），验收 token 改为真实 64 位 hex 假值并覆盖键值、Bearer、裸值三种形态；测试 `runtime_token_absent_from_materials`、`redaction_covers_credential_shapes` |
| 追溯矩阵把服务层或注册表层用例标成运行态集成 | 待修 |
| 套餐周期契约与后台任务注册表不一致 | 已修（提交 1146217c）：新增 `PLAN_BALANCE_INTERVAL_MIN_SEC/DEFAULT_SEC/MAX_SEC` 单源，注册表元数据与运行时默认值、`plan_balance::service` 的 `MIN/DEFAULT/MAX_PERIOD` 全部引用同一常量；注册表锁定测试跨层断言服务常量与注册表边界一致；测试 `registry_metadata_is_locked`、`plan_balance_settings_commit_and_visibility` |
| Runtime `git.diff` 路径校验放过盘符绝对路径/符号链接，工作区读取越过仓库根 | 已修：协议拒绝冒号/控制字符（盘符、ADS），应用层对规范位置做仓库根包含检查；测试 `git_diff_requires_a_safe_repository_relative_path`、`git_diff_snapshot_rejects_targets_outside_the_worktree` |
| 快照 revision 固定为握手起点，事件续传进入不可恢复 resync 循环 | 已修：`RuntimeCallbacks` 增加事件环末尾 revision 回调，`EventHub` 原子维护最新发布 revision，`runtime.snapshot` 返回该值；测试 `event_hub_latest_revision_tracks_published_events`、`runtime_snapshot_returns_injected_resources_with_hello_generation` |
| 非交互关闭在保存、停止或清理落定前返回 Succeeded | 已修：`app.close` 回执改由 `ApplicationClose.runtime_completion` 持有，最终冲刷成功后才发送；保存失败回 NotReady 并保持现场，未知结果放弃回执；测试 `runtime_app_close_reports_save_failure_and_keeps_scene`、`integration_runtime_noninteractive_close` |
| 事件环移除滞后订阅后，订阅连接保持静默 | 已修：订阅事件写线程在事件源结束（滞后移除／发送端关闭／写失败）置完成标记，连接线程 1 秒读超时内检测并关闭连接，客户端收到 EOF；测试 `runtime_subscription_closes_when_the_event_source_ends` |
| expected_revision 在线上端到端未生效 | 已修：service 将线缆 `expected_revision` 与事件环当前末尾 revision 原样投影到 `DispatchRequest`，派发器在登记前拒绝失配；测试 `wire_expected_revision_reaches_the_dispatcher_projection` |
| `publish_discovery` 发布失败时双清理只报一个错误 | 已修：server 与事件环回收都必定尝试，错误逐项并入发布根因信封并保留发布 kind；测试 `publish_failure_combines_every_cleanup_error`、`service_publish_failure_reports_real_server_cleanup_failure` |
| `runtime discover --json` 是手工 JSON，不在随包 schema／DTO | 已修：协议新增 `DiscoverOutput`（端点记录 + Runtime 凭据），Debug 脱敏，CLI 只经该 DTO 序列化；schema 资产包含字段合同；测试 `discover_output_round_trips_and_redacts_debug`、`packaged_schema_includes_the_discover_output_contract` |
| 合同冲突：local_control 与设计文档保留旧能力、旧错误与旧命名 | 已修：删除“仅开放握手”“OpenFile 仍拒绝”“不存在 unknown_operation”“事件环接入前”等过时真值；设计方法表统一为 `runtime.operation_get`／`runtime.operation_wait`，snapshot revision 与事件环同源 |
| 合同冲突：写请求参数散列没有客户端值 | 已裁决为服务端推导：线缆不携带客户端散列，服务端按解析后的封闭意图生成方法级规范投影与散列，同键同散列返回原 Operation，同键异散列 `mutation_conflict`；设计文档与协议 CLAUDE.md 同步 |
| Operation 停在 Queued/Running 后没有恢复路径 | 已修：状态机新增 `Queued → Failed`；启动维护在 Journal 与维护请求后调用 `reconcile_interrupted_operations`，当前世代 Queued 收敛 Failed、Running 收敛 OutcomeUnknown，旧世代与终态不改写；测试 `interrupted_operations_are_reconciled_only_in_the_current_epoch`、`maintenance_reconciles_interrupted_operations` |
| 项目、布局、面板与终端写 Operation 在持久快照提交前报告成功 | 已修：新增 `WorkspaceWriterClient::commit_project_snapshot` 立即事务命令与容量 1 回执；改变项目／布局／终端记录的同步写意图及 `panel.open_file`／`agent.start|resume|fork` 的成功回执推迟到事务提交，输入／聚焦／Git 外部写／关闭链保持原语义；事件泵轮询回执，事务失败、写线程停止或关闭中回 `NotReady`／`Closing`（提交 5e873a52）；测试 `runtime_mutation_success_waits_for_the_project_snapshot_commit`、`runtime_mutation_fails_when_the_snapshot_writer_is_stopped`、`immediate_project_snapshot_commit_acknowledges_after_the_transaction`、`runtime_panel_open_file_reuses_the_document_open_chain`、`runtime_agent_start_injects_catalog_builtin_command` |
| Hooks 草稿在决定齐备前被丢弃 | 已修：Runtime `app.close`／`project.remove` 先收集文档决定、预检并通过关闭登记后才按 `Discard` 清理草稿；项目移除以显式 `allow_hooks_discard` 跳过交互守卫（提交 c2b50b9d）；测试 `runtime_hooks_discard_waits_for_complete_document_decisions`、`runtime_project_hooks_discard_waits_for_document_decisions`、`runtime_project_remove_honors_discard_and_save_policies` |
| 合同冲突：app.close 回执语义与“保存失败返回失败”冲突 | 已裁决为完成语义：成功回执只在全部保存、PTY 停止与本地持久冲刷落定后发送，保存失败回 NotReady 并保持窗口与脏正文，结果不明放弃回执映射 OutcomeUnknown；`app.close` 派发等待上限 120 秒；测试 `runtime_app_close_reports_save_failure_and_keeps_scene`、`integration_runtime_noninteractive_close` |
| 合同冲突：非交互关闭缺少运行终端处置策略 | 已裁决为终端不进入非交互策略：线缆只保留 Document／Hooks 决定，关闭按作用域停止目标 PTY，等待／保留／取消只由交互界面提供；设计、featureSpec、随包 API、协议与应用文档同步 |
| 控制面一次 join 失败被永久重放 | 已修：资源已回收后的重复 `shutdown` 只记录历史 join 错误并返回成功，首次错误仍在首次关闭返回；测试 `local_control_shutdown_replays_worker_join_failure` 更新为断言重复关闭成功 |
| 随包 schema 未覆盖多数方法的请求结构，method 与 params/result 无关联校验 | 已修：schema 生成器遍历方法注册表，把每个方法的 `params`／`result` schema 汇入根 `$defs`，并生成 `$defs.MethodContracts` 方法名到合同的封闭映射；`RuntimeRequest` 用逐方法 `oneOf` 绑定方法名与参数结构；测试 `packaged_schema_covers_every_registered_method_contract`、`packaged_schema_binds_each_method_to_its_params` 逐方法核对并拒绝错误配对 |
| Runtime 合同测试不在任何固定门禁内 | 已修：CI 与 `release.ps1` 前置强制执行 `cargo test -p slterm_runtime_protocol --features schema --test lib_tests`，逐字节核对随包 schema 资产、生成器重建与真实请求校验；命令矩阵与协议 CLAUDE.md 同步 |
| Runtime 双进程 conformance 与事件断线矩阵缺失 | 待修 |
| 事件面只发布项目激活 | 已修（提交 581f7257，按审查允许路径降级）：协议封闭枚举只保留有真实发布点的 `project.activated`，删除 `panel.activated`／`panel.closed`／`terminal.output`，不再宣称不存在的事件能力；客户端以快照与 `terminal.read`／`terminal.wait` 的 revision 对齐状态；随包 schema 重新生成，featureSpec／设计／协议与壳文档同步降级；测试 `event_ring_assigns_monotonic_revisions_and_resumes_without_gap`、`runtime_events_visibility_and_generation`、`generated_schema_matches_the_packaged_asset` |
| 路由与能力摘要不按方法注册表作用域执行 | 已修：server 路由以注册表 `lookup(...).scopes` 复核 Runtime／Handover 方法，hello 能力与 describe 按已认证作用域经 `for_scope` 收窄；测试 `handover_hello_capabilities_are_scoped`、`runtime_hello_capabilities_exclude_handover`、`runtime_describe_returns_registry_and_contract_hashes` |
| `GitStatusResponse.root` 返回项目根而不是仓库根 | 已修：状态 DTO 根取 `StatusSnapshot.worktree_root`，缺省才回退项目根；测试 `git_status_reports_the_repository_worktree_root` |
| 快照不暴露布局树与分割几何，resize 路径不可推导且视口由客户端自报 | 已修：快照新增 `ProjectLayoutResource`／`LayoutNodeResource` 分割树（方向、比例、组身份，空布局为 null），resize 布尔路径可从树推导；split／resize 线缆删除客户端视口，AppRoot 使用前台真实内容视口校验最小空间；测试 `snapshot_layout_tree_round_trips_and_rejects_unknown_nodes`、`runtime_snapshot_returns_workspace_resources_and_identity`、`panel_methods_validate_layout_identities_and_bounds` |
| 等待操作不能因客户端断开而取消 | 已修：`terminal.wait`／`agent.wait` 与 `operation.wait` 每轮非阻塞探测连接 EOF，断开即返回 Closing 并停止轮询；测试 `runtime_operation_wait_cancels_when_the_client_disconnects` |
| `runtime_token_absent_from_materials` 锚只覆盖诊断材料 | 已修：追溯行改为三个证据锚——`integration_runtime_credential_boundaries` 覆盖端点文件、设置文件与凭据域，`runtime_token_shape_outside_registered_fields_never_enters_backup` 覆盖备份归档与密文，`runtime_token_absent_from_materials` 覆盖诊断导出；三处都使用真实 64 位 hex 假值形态 |
| 注册表 kind/idempotent 语义与真实副作用不符 | 已修：新增 `MethodKind::Connection`，`events.subscribe`／`events.unsubscribe` 登记为连接级且不可重试，`instance.handover` 取消幂等声明；describe 与 schema 同步暴露；测试 `connection_methods_and_handover_do_not_claim_retry_safety`、`describe_lists_the_method_registry_with_contract_hashes` |
| 握手后的解析失败直接断开连接 | 已修：认证后协议解析失败在可取到 `id` 时返回封闭 `InvalidRequest` 并保留连接，无法解析 `id` 才断开；首包认证仍不返回协议预言机；测试 `local_control_authenticated_parse_failure_keeps_the_connection` |
| 分页与响应预算按原始字节估算 | 已修：分页预算降至 160 KiB，按 `serde_json` 控制字符最坏六倍转义保证低于 1 MiB；所有 Runtime 响应编码后复核预算，超限返回封闭 `ResponseTooLarge` 并保留连接，无界快照／状态不再静默断开；测试 `runtime_terminal_read_control_characters_stay_within_response_budget`、`runtime_snapshot_over_budget_returns_structured_error_and_keeps_connection` |
| 分页游标不绑定资源身份，也接受非 UTF-8 字符边界偏移 | 已修：`ReadCursor` v2 绑定资源身份散列（终端＋agent 身份、Git 项目＋路径），偏移落在 UTF-8 续字节立即失效；测试 `cursors_are_bound_to_resource_identity_and_character_boundaries`、`stale_generation_or_revision_invalidates_the_cursor` |
| Handover 客户端不核对 hello 协议版本与合同散列 | 已修：客户端比较 hello 的 `protocol_sha256`／`schema_sha256` 与本实例合同，失配在发送 handover 前返回 `ContractMismatch`；发现记录协议版本仍由 `DiscoveryRecord::validate` 核对；测试 `client_rejects_a_contract_hash_mismatch_before_handover` |
| git.open_diff/git.open_deleted 超时按失败处理但打开可能仍完成 | 已修：打开事件投递后回执超时或断连返回 `GitOpenOutcome::Unknown`，派发器记为可查询 `OutcomeUnknown`，只有落定回执才成功；测试 `git_open_timeout_reports_unknown_outcome`、`git_open_unknown_outcome_is_not_reported_as_failure` |
| agent.start/resume/fork 在提示符注入前报告成功 | 已修：Agent 启动写意图延迟为 `RuntimeMutationDispatch::Deferred`，完成回调只在真实 PTY 写入成功或明确失败时回执，取消／销毁不回写并按 OutcomeUnknown 处理；服务端 Agent 写回执上限扩至 12 秒覆盖 10 秒提示符超时；测试 `runtime_agent_start_injects_catalog_builtin_command`、`explicit_agent_injection_reports_failure_without_a_worker` |
| 封闭文本字段在 DTO 与 schema 中仍是自由字符串 | 已修：`TerminalResource.run_intent`、`AgentResource.status`、`ViewResource.kind`、`RuntimeEvent.kind` 与 Git XY 状态改为协议枚举并在 schema 中给出封闭值；`AgentResource.kind` 明确为可先于目录登记的来源 slug，保留开放文本；测试 `closed_resource_text_fields_reject_unknown_values`、`event_ring_assigns_monotonic_revisions_and_resumes_without_gap` |

## 脚本、发布、许可与依赖

| 问题 | 状态 |
|---|---|
| 内嵌 Maple Mono 字体缺少 OFL 许可与版权文本 | 已修：新增 `licenses/LICENSE-MAPLE-MONO`（上游 OFL 1.1 全文与版权行）与 `LICENSE-CONPTY`、`LICENSE-SQLITE` 随包；`check_licenses.py` 增加内嵌字体/运行时资产清单校验（文件、非空许可、版权持有者） |
| 发布说明哈希与当前 dist 及打包证据不一致 | 已修：新增 `sync-release-notes.ps1` 由 `dist/SHA256SUMS` 派生版本说明 SHA256 块并原子替换，发布链末尾执行；当前 0.3.0 说明已同步到 dist 哈希；生产包测试覆盖派生与临时文件清理 |
| 许可审计根包名与实际 workspace 包名不一致 | 已修：`ROOT_PACKAGES` 改为真实 `slterm_hook`，缺少任一根包即报错；单测夹具改用真实包名并新增缺根用例 |
| 手工 THIRD-PARTY-NOTICES 缺版权行且引用不存在路径 | 已修：补 Microsoft／Tabby 版权行与 Maple Mono、ConPTY、SQLite 许可落点；Herdr 归因改指 `slterm_agent_detection/*.toml`，删除已不存在的 SSH Bash 段落与 `prepare-windows-runtime` 哈希声明 |
| dav1d 原生源从可变 tag 克隆，审计 commit 未进入构建 | 已修：`build.rs` 固定 `32cf02af50f32af108a3b281c452788dccdac648`，缓存命中审计 commit 时离线复用，clone/fetch 后强制 `rev-parse HEAD` 校验；tag 漂移或缓存被替换即构建失败 |
| 构建新鲜度摘要漏掉非 .rs/.toml 构建输入 | 已修：摘要与 mtime 回退改为对 `SourceRoots` 全部文件（除 `.git`）取哈希，新增 vendor tree-sitter、`vendor/conpty`、内嵌 schema 与工具链登记；自测新增非 `.rs` 资产变更拒绝用例 |
| 随包 ConPTY 运行时的校验与实际嵌入二进制脱节 | 已修：新增 `check_bundled_runtime.py` 固定 `vendor/conpty` 两个二进制 SHA256 与 x64 PE，并校验许可；CI 与发布链常驻；`windows-runtime.tests.ps1` 无归档时显式打印 SKIP 并登记豁免 |
| release.ps1 的钉测试只执行体积与 profile | 已修：发布链构建前运行 `package-release.tests.ps1 -SelfTest`，构建后运行产物钉测试；删除不存在的 `installer.tests.ps1` 探测；新增发布说明哈希自测 |
| “清单即合同”没有机械化全等检查 | 已修：清单自测解析 `installer.iss` `[Files]`（含通配展开、DestDir 映射）并与共享清单 `PackagePath` 集合全等比较，新增许可文件同步进两处 |
| CI 不执行许可清单核验 | 已修：CI 新增 `check_licenses.py` 与 `check_bundled_runtime.py` 步骤；根门禁清单与命令矩阵同步 |
| CI 未按发布口径验证（无 --locked、无 release 构建、无 dumpbin） | 已修：CI 全部 cargo 步骤加 `--locked`；新增 release 构建 `slterm`／`slterm-hook` 并调用 `Assert-Dav1dStatic` 审计 `target/release/slterm.exe` |
| 架构门禁未使用 --base | 已修：CI 检出完整历史，PR 用 `base.sha`、push 用 `before` 传入 `check_architecture.py --base`，零基线回退全量；测试 `test_ci_architecture_gate_compares_against_the_change_base` |
| 版本校验与默认值跨脚本不一致 | 已修：`package-release.ps1` 与 `build-installer.ps1` 改为缺省回读 `slterm_app/Cargo.toml` 且只接受三数值段，与 `release.ps1`／`verify-release.ps1` 同一契约；删除 `unreleased` 默认 |
| 发布产物无签名链 | 已裁决为不签名：首发不提供 Authenticode，`verify-release` 只验证资产名、体积、PE 魔数与 SHA256；签名边界与 SmartScreen／仅哈希风险写入打包决策与发布清单，不把哈希校验描述成签名 |
| i18n 合同只约束目录占位符 | 已修：生成器输出 `Message::placeholders()` 与 `Message::ALL`；`format` 在 debug 构建校验参数名集合完全一致，合同测试用声明占位符遍历全部消息与语言并断言无残留 `{name}`；测试 `every_message_formats_with_its_declared_placeholders` |
| 数学语料引用不存在的测试、主题与截图 | 已修：语料开头改用 Linear／Nord 与真实选区测试锚（`fresh_terminal_derives_cursor_and_selection_from_palette_slots` 等），删除不存在的 `native_reader_selection_preview`、`SLTERM_SELECTION_QA_*`、MintLight 与截图引用 |

## 安全、凭据与依赖

| 问题 | 状态 |
|---|---|
| Git 读取未统一遵守有界执行、禁网与凭据提示约束 | 已修：`command` 统一设置 `GIT_TERMINAL_PROMPT=0`/`GIT_NO_LAZY_FETCH=1`/`GIT_ALLOW_PROTOCOL=file`；`head_file`/`verify_head`/`file_hunks` 改走有界带 deadline 执行体；测试 `every_git_command_disables_prompts_lazy_fetch_and_network` |
| 生产 TLS 栈命中 emSign 弃用中间证书公告 | 已修：`rustls-webpki` 升到 0.103.16、`webpki-roots` 升到 1.0.10；同时升级 `cookie` 0.18.3、`ctutils` 0.4.4、`url` 2.5.9，`cargo audit` 漏洞数为零（仅保留告警） |
| 加密备份 AAD 范围与恢复时序合同冲突 | 已裁决：AAD 统一为完整头部（魔数｜盐｜nonce），旧版仅魔数 AAD 包明确拒绝，测试 `legacy_magic_only_aad_packet_is_rejected`；删除运行期即时 `restore`／`restore_selected`／`undo` 写通道，恢复只经 `stage_restore` 与启动维护期，撤回实现必须复用同一维护请求时序 |
| 依赖审计未进入 CI 常驻门禁 | 已修：CI 安装固定 `cargo-audit 0.22.2` 并运行 `cargo audit`，漏洞阻断、告警放行；新增 `audit.toml` 声明忽略登记格式；当前审计漏洞为零 |
| 诊断脱敏词表与备份敏感键表不一致 | 已修：脱敏键集合与备份敏感键表对齐，授权头额外吞掉方案词后的凭据；测试 `redaction_covers_credential_shapes` |
| 凭据明文副本没有全程清零 | 已修：控制面服务端／客户端读取缓冲、帧与 1 KiB 分块改为 `Zeroizing`；套餐请求 `Authorization` 头在 `Zeroizing<String>` 中组装，解析后随作用域释放清零 |
| AI hook 解析在 debug 日志输出整条事件 | 已修：只记录 source/kind/pane，正文与 session_file 不进入日志 |
| 更新下载 .part 打开跟随预置链接 | 已修：先移除既有项再 `create_new` 独占创建 |
| 凭据 Win32 FFI 调用缺少逐块 SAFETY 不变量 | 已修：CredReadW/CredFree/CredWriteW/CredDeleteW 调用点补齐 SAFETY 不变量注释 |

## 测试质量、文档与计划证据

| 问题 | 状态 |
|---|---|
| 追溯矩阵引用不存在的测试例名 | 已修：8 个失效锚改为真实例名（`integration_encrypted_backup_round_trips_without_touching_disk`、`interrupted_apply_is_rolled_back_on_restart`、`maintenance_restore_leaves_credentials_and_unlisted_files_untouched`、`integration_commit_rollback`、`agent_root_default_capabilities`、`path_file_identity_deduplicates_document` 等），硬链接别名补真实 AppRoot 用例 `path_file_identity_deduplicates_document`；新增 `scripts/check_traceability.py` 门禁与自测 `test_traceability.py`，已自动化行锚点必须解析到源码唯一测试函数 |
| 部分矩阵例名存在，但覆盖的不是声明场景 | 待修 |
| 全量验收锚以源码字符串和局部状态代替跨域行为 | 待修 |
| 测试豁免表含失效设计路径、目标态待实现项与超范围无障碍项 | 已修：失效例名改为真实锚（`spawn_lock_serializes_capacity_check_and_reservation` 等），删除已退役 restore ticket 与不存在数学用例；观感段改用现行 featureSpec／design 路径与真实兜底例名，删除范围外的读屏无障碍行；追溯矩阵新增“人工豁免场景”分区引用豁免行 |
| 架构总览与常驻规则仍使用旧栈数据名、类型名与已取消的校验 | 已修：根规则改指真实类型 `PanelContent`/`ViewKeyKind`/`ProjectLayout`；overview ownership 表补齐全部 workspace 成员，数据文件改为实际的 `settings.json`／`workspace.db`／`workspace.backup.db`／`learning.db`／`themes/` 等 |
| README、CHANGELOG 与发布说明的用户能力清单停留在旧栈 | 已修：中英 README、CHANGELOG 0.3.0 与中英发布说明按当前能力重写（项目/面板组/共享 Document/Runtime v2/纯颜色主题/背景图），删除标签页、窗格缩放、图片面板、只读双栏 Diff、WindowState 与旧设置页清单；版本状态改为 2026-10-18 首发 |
| 被排除的图片面板实现仍编译存在 | 已修：删除 `ImageTabView`、图片几何/解码、`image_viewable` 与 `openable_in_app`，`doc_tabs` 只保留 Markdown 判定；`acceptance_excluded_surface_absent` 改为递归扫描 `src/` 生产段并覆盖图片面板与旧 Diff/拖放符号 |
| featureSpec 未覆盖已实现的壁纸与屏幕检测 | 已修：外观页登记本地背景壁纸的适应/对齐/不透明度与有界解码边界；终端规格把“状态信号”明确为 hooks 未给权威状态时的画面特征兜底，且不得猜测未知 CLI |
| Mermaid 终态在 featureSpec、devPlan 与进度之间构成合同冲突 | 已裁决为延期：featureSpec 与验收场景明确当前只显示普通代码块，静态图实现另行评审；traceability 的延期状态不再与 featureSpec 冲突 |
| devPlan core crate 依赖方向与实际策略相反 | 已修：`architecture.md` 的允许依赖改为与 `architecture/dependencies.toml` 一致（workspace 依赖 path／document／split／agents；agents 依赖 path／settings；sidebar 依赖 path／settings／workspace／agents） |
| devPlan Runtime 方法表写错 operation.get/wait | 已核验无需修复：设计方法表已使用 `runtime.operation_get`／`runtime.operation_wait`，全 devPlan 无点号形式残留 |
| 进度与追溯文档保留测试通过计数、过期状态锚与计划期自述 | 已修：进度表删除 GPUI／core／脚本通过数与“八个锚”计数，失效例名改为真实锚（`integration_commit_rollback`、`interrupted_apply_is_rolled_back_on_restart`、`maintenance_restore_leaves_credentials_and_unlisted_files_untouched`），删除不存在用例的撤回证据；追溯首段改为“已自动化必须解析到唯一真实测试”，状态锚更新为 2026-11-27 |
| 模块与设计文档存在第二份源码式复述 | 待修 |
| Runtime v2 验收 hash 指向 Windows 阶段提交 | 已修：Runtime v2 行改用 `b47e9b92`（Runtime 单实例同构验收），不再引用诊断白名单提交 |
| “全部阶段验收完成”与安装器、发布包实机待办矛盾 | 已修：进度状态锚改为“命名阶段的历史验收不表示发布就绪”，当前实施位置明确评审修复与未接线产品面补齐进行中、安装器与发布包实机项待发布机核验 |
| 旧实现与最终排除面未清理 | 已修：删除 `diff_tab`（旧双栏 `InputState`）、`file_drop`（无消费拖放载荷）、`code_tab::language_for_path` 第二语言路由与图片查看器；壁纸保留并补入 featureSpec，主题非颜色字段此前已删除；排除面验收改为全源码生产段扫描 |
| 进度表用同一提交充当多个阶段锚 | 已修：每个阶段改用各自的验收／实现提交（`e4f4d88b`、`c38267b2`、`ed4da6df`、`97cb2b3c`、`0c92d3f5`、`aadca3fa`、`d9fba527`、`b47e9b92`、`4a82ca15`、`f8c42a32`），工作区/Document、Agent/设置、Windows/全量不再共用 hash |
| 追溯矩阵存在同名测试与重复行 | 已修：删除跨域重复行与设置／工作区重复锚；服务层同名例改名为 `registry_aliases_resolve_to_the_same_document_by_file_identity`、`paged_reload_is_read_only_for_clean_and_conflicts_for_dirty`、`snapshot_recovery_runs_after_layout_reconciliation`、`outside_snapshot_recovery_marks_requires_authorization`、`application_exit_coordinator_allows_abandon_after_final_commit_failure`、`final_commit_failure_sets_old_state_abandon_warning` 与三个 crate 的 `*_stable_id_errors_describe_the_rejection`；门禁强制全仓测试例名唯一 |
| 计划行为切片没有可核销例名 | 已修：布局、页签溢出、项目重排、面板循环、编辑事务、语言/语法、文本视图与 IME、终端 worker/resize/ShellLine、补全/Git 来源、Comrak/HTML/公式、FileTreeModel、仓库绑定、AgentCatalog/活跃会话/Hooks/桥、Runtime 方法族、WindowsIntegration 与 DPAPI 恢复点均补真实例名；隔离到期/导出/永久删除与备份工作区类别、撤回能力明确标注尚未实现，待接线后核销 |

## 跨域补充

| 问题 | 状态 |
|---|---|
| 显式终端 cwd 只做词法包含校验，未解析真实目标 | 已修：Runtime `terminal.create` 与 handover `NewTerminal` 在连接线程经 `resolve_terminal_path` 解析真实目录（拒绝文件/不存在/非 Unicode），联接解析为目标真实路径后再由 AppRoot 与项目根比较；测试 `terminal_paths_resolve_real_targets_and_reject_non_directories` |
| 工作区布局恢复没有未知 ViewKey 过滤与重复视图归一 | 已修：新增 `ProjectLayout::from_canonical_json_lossy`，恢复链过滤未知视图类型、重复视图保留首个、空组塌缩并重排树；测试 `lossy_recovery_filters_unknown_views_collapses_empty_groups_and_dedupes`、`startup_filters_unknown_and_duplicate_views` |
| AppRoot 后台 Agent 根加载不可注入，虚拟窗口用例读取真实 home | 已修：虚拟窗口测试根构造时预置空 Agent 根快照并置 `agent_roots_loaded`，后台加载在测试中显式禁用；专用用例继续经 `replace_agent_global_dirs_for_test` 注入临时根；测试 `test_root_preloads_empty_agent_roots` |
| 共享编辑（Diff/Markdown）缺少可见性门控与取消协议 | 已修：共享 Document 编辑只广播给活跃项目内各组的 active Markdown／Diff；隐藏面板保留最后可见正文并丢弃解析／差分任务（Task 替换即取消），可见转换在快照提交后对账，新可见面板只采纳一次最新正文并重新调度；Rope Editor 仍接收全部编辑（提交 6072ff26）；测试 `hidden_markdown_defers_shared_edits_until_visible`、`diff_edit_shared_document`、`document_shared_across_projects`、`integration_git_document`、`markdown_mode_toggle_uses_shared_draft_without_disk_reload` |
| 历史隔离到期清理、ZIP 导出与永久删除未实现 | 待修 |
| 脚本自测依赖共享 %TEMP% | 已修：`check_licenses.license_files` 只检查 `.git` 所在 checkout 根，不再向文件系统祖先遍历拾取无关 LICENSE；新增 `test_git_package_ignores_licenses_outside_the_checkout_root` 回归用例 |
| Git 测试夹具未隔离全局配置 | 已修：测试构建的 `git::command` 经环境注入 `GIT_CONFIG_NOSYSTEM=1`、空全局配置、`commit.gpgsign=false`、空 `core.hooksPath` 与 `core.autocrlf=false`，不改 argv 前缀；local_control、补全、gutter、文件树夹具统一改走同一入口 |
