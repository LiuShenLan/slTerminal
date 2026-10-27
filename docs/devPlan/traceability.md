# 需求—设计—测试追溯

> 将 featureSpec 所有者与验收场景映射到目标设计和测试锚；目标例名是实施计划，不代表测试已经存在或通过。

## 决策与红线

- 每条场景保留独立核销行；实现后填入真实测试例名、执行证据或已登记豁免，不能以全量测试全绿替代逐条核销。
- 状态取 `待实现`、`已自动化`、`人工豁免`、`延期`。状态锚为 2026-10-24：除用户明确延期的 Mermaid 最终能力外均为 `待实现`，本文件不提供虚假完成证据。
- 场景名使用领域语言，不引入阶段号、条目号或测试分层编号；源码引符号名，测试引例名。
- 设计链接是实现约束，需求链接是行为真值；若二者冲突，先报告并取得用户裁决，不静默修改任一方。
- Mermaid 的静态渲染是 featureSpec 的最终能力，但本轮按用户裁决不实现；其场景保留为 `延期`，普通代码块测试不能冒充最终验收。

## 验收与测试

- 每次阶段验收检查本表涉及场景是否有真实例名和新鲜输出；计划锚不得直接改写成通过结论。
- 最终核销同时检查排除项、敏感数据边界、跨域单点和人工豁免；任一未决场景都阻塞全量验收。
- 进度记录只引用本表已核验条目，不在这里追加流水账或问题清单。

## 操作

### 所有者索引

| featureSpec 所有者 | 目标设计 |
|---|---|
| [规格总览](../featureSpec/README.md) | [架构](design/architecture.md)、[领域模型](design/domain-model.md)、[实施计划](plan.md) |
| [跨域验收](../featureSpec/acceptance.md) | [测试设计](design/testing.md)、本矩阵 |
| [PathPolicy](../featureSpec/path-policy.md) | [架构](design/architecture.md)、[领域模型](design/domain-model.md)、[文件与 Git](design/files-git.md) |
| [跨域约定](../featureSpec/cross-cutting.md) | [架构](design/architecture.md)、[工作区](design/workspace.md)、[测试设计](design/testing.md) |
| [设置与持久化](../featureSpec/settings-and-persistence.md) | [持久化](design/persistence.md)、[设置与恢复](design/settings-recovery.md)、[生命周期](design/lifecycle.md) |
| [备份与恢复](../featureSpec/backup-and-recovery.md) | [设置与恢复](design/settings-recovery.md)、[持久化](design/persistence.md) |
| [Runtime](../featureSpec/runtime.md) | [Runtime](design/runtime.md)、[生命周期](design/lifecycle.md) |
| [系统集成](../featureSpec/system-integration.md) | [Windows](design/windows.md)、[生命周期](design/lifecycle.md) |
| [工作区壳](../featureSpec/workspace/shell.md) | [工作区](design/workspace.md)、[生命周期](design/lifecycle.md) |
| [导航树](../featureSpec/workspace/navigation.md) | [Agent](design/agents.md)、[工作区](design/workspace.md)、[Windows](design/windows.md) |
| [Document](../featureSpec/panels/document.md) | [Document](design/document.md)、[领域模型](design/domain-model.md) |
| [Editor](../featureSpec/panels/editor.md) | [Document](design/document.md)、[文件与 Git](design/files-git.md) |
| [Markdown](../featureSpec/panels/markdown.md) | [Markdown](design/markdown.md)、[Document](design/document.md) |
| [Diff](../featureSpec/panels/diff.md) | [文件与 Git](design/files-git.md)、[Document](design/document.md) |
| [GitShow](../featureSpec/panels/gitshow.md) | [文件与 Git](design/files-git.md) |
| [Terminal](../featureSpec/panels/terminal.md) | [终端](design/terminal.md) |
| [Settings](../featureSpec/panels/settings.md) | [设置与恢复](design/settings-recovery.md)、[Hooks](design/hooks.md)、[工作区](design/workspace.md) |
| [侧栏框架](../featureSpec/sidebar/framework.md) | [工作区](design/workspace.md) |
| [Explorer](../featureSpec/sidebar/explorer.md) | [文件与 Git](design/files-git.md)、[工作区](design/workspace.md) |
| [Commit](../featureSpec/sidebar/commit.md) | [文件与 Git](design/files-git.md) |
| [Agent 全局文件](../featureSpec/sidebar/agent-files.md) | [Agent](design/agents.md)、[文件与 Git](design/files-git.md) |
| [资产与发布](../featureSpec/assets/README.md) | [依赖](design/dependencies.md)、[Windows](design/windows.md) |

### 跨域验收

设计：[测试设计](design/testing.md)、[架构](design/architecture.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 所有者判据可观察、可复现且不依赖内部状态 | `acceptance_user_contract_observable` | 待实现 |
| 最终排除项没有隐藏入口或迁移占位 | `acceptance_excluded_surface_absent` | 待实现 |
| 跨域集成不存在第二缓冲、授权或关闭通道 | `integration_single_authority_channels` | 待实现 |
| 平台或第三方能力缺失按边界降级或失败 | `integration_platform_boundary_failure` | 待实现 |
| 不可自动化面有豁免、兜底和人工记录 | `acceptance_exemptions_registered` | 待实现 |

### 跨域集成场景

| 验收场景 | 目标设计 | 目标测试例名 | 状态 |
|---|---|---|---|
| Document 全局身份 | [领域模型](design/domain-model.md)、[Document](design/document.md) | `integration_document_global_identity` | 待实现 |
| 跨项目视图 | [工作区](design/workspace.md)、[Document](design/document.md) | `integration_cross_project_views` | 待实现 |
| 视图身份 | [领域模型](design/domain-model.md)、[工作区](design/workspace.md) | `integration_view_identity_reopen` | 待实现 |
| 侧栏显隐 | [工作区](design/workspace.md) | `integration_sidebar_visibility_semantics` | 待实现 |
| PathPolicy 能力并集 | [架构](design/architecture.md)、[文件与 Git](design/files-git.md) | `integration_path_capability_union` | 待实现 |
| 链接与文件身份 | [架构](design/architecture.md)、[文件与 Git](design/files-git.md)、[Document](design/document.md) | `integration_links_and_file_identity` | 待实现 |
| 会话级授权恢复 | [生命周期](design/lifecycle.md)、[持久化](design/persistence.md)、[Document](design/document.md) | `integration_session_grant_restore` | 待实现 |
| 根重定位 | [工作区](design/workspace.md)、[领域模型](design/domain-model.md)、[文件与 Git](design/files-git.md) | `integration_project_relocation` | 待实现 |
| 最后引用关闭 | [生命周期](design/lifecycle.md)、[Document](design/document.md) | `integration_last_reference_close` | 待实现 |
| 批量关闭 | [生命周期](design/lifecycle.md)、[工作区](design/workspace.md) | `integration_batch_close` | 待实现 |
| 项目移除与退出 | [生命周期](design/lifecycle.md)、[工作区](design/workspace.md) | `integration_project_remove_and_exit` | 待实现 |
| 崩溃恢复 | [生命周期](design/lifecycle.md)、[持久化](design/persistence.md) | `integration_crash_recovery` | 待实现 |
| 快照配额 | [持久化](design/persistence.md)、[Document](design/document.md) | `integration_snapshot_quota` | 待实现 |
| 终端恢复 | [终端](design/terminal.md)、[生命周期](design/lifecycle.md) | `integration_terminal_restore` | 待实现 |
| PTY 上限 | [终端](design/terminal.md)、[工作区](design/workspace.md) | `integration_pty_limit` | 待实现 |
| 新建 cwd | [终端](design/terminal.md)、[工作区](design/workspace.md) | `integration_new_terminal_cwd` | 待实现 |
| 单实例与 Runtime | [Runtime](design/runtime.md)、[生命周期](design/lifecycle.md)、[工作区](design/workspace.md) | `integration_single_instance_runtime_parity` | 待实现 |
| Runtime 非交互关闭 | [Runtime](design/runtime.md)、[生命周期](design/lifecycle.md) | `integration_runtime_noninteractive_close` | 待实现 |
| Runtime 凭据 | [Runtime](design/runtime.md)、[设置与恢复](design/settings-recovery.md) | `integration_runtime_credential_boundaries` | 待实现 |
| 补全可信边界 | [终端](design/terminal.md) | `integration_completion_trust_boundary` | 待实现 |
| 本地学习数据 | [终端](design/terminal.md)、[持久化](design/persistence.md) | `integration_local_learning_data` | 待实现 |
| 终端搜索 | [终端](design/terminal.md) | `integration_terminal_search` | 待实现 |
| Commit 回滚 | [文件与 Git](design/files-git.md) | `integration_commit_rollback` | 待实现 |
| Git 与 Document | [文件与 Git](design/files-git.md)、[Document](design/document.md) | `integration_git_document` | 待实现 |
| 历史删除撤销 | [Agent](design/agents.md)、[生命周期](design/lifecycle.md) | `integration_history_delete_undo` | 待实现 |
| 套餐凭据 | [Windows](design/windows.md)、[设置与恢复](design/settings-recovery.md) | `integration_plan_credentials` | 待实现 |
| 套餐调度 | [Windows](design/windows.md)、[生命周期](design/lifecycle.md) | `integration_plan_schedule` | 待实现 |
| 自定义主题 | [设置与恢复](design/settings-recovery.md) | `integration_custom_theme` | 待实现 |
| Markdown 与 Mermaid | [Markdown](design/markdown.md)、[Document](design/document.md) | `integration_markdown_mermaid_final` | 延期 |
| 设置单例 | [设置与恢复](design/settings-recovery.md)、[工作区](design/workspace.md)、[Hooks](design/hooks.md) | `integration_settings_singleton` | 待实现 |
| 加密备份 | [设置与恢复](design/settings-recovery.md)、[持久化](design/persistence.md)、[生命周期](design/lifecycle.md) | `integration_encrypted_backup` | 待实现 |
| 更新 | [Windows](design/windows.md)、[生命周期](design/lifecycle.md) | `integration_update` | 待实现 |
| 通知与任务栏 | [Windows](design/windows.md) | `integration_notification_taskbar` | 待实现 |
| 语言 | [Windows](design/windows.md) | `integration_language` | 待实现 |
| 诊断与清理 | [Windows](design/windows.md)、[持久化](design/persistence.md) | `integration_diagnostics_cleanup` | 待实现 |

### 加密备份与恢复

设计：[设置与恢复](design/settings-recovery.md)、[持久化](design/persistence.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 仅选工作区类别时排除设置及永久排除内容 | `backup_workspace_category_exclusions` | 待实现 |
| 条目篡改或口令错误时整包拒绝且零写入 | `backup_tamper_rejected_zero_write` | 待实现 |
| 恢复中断后可由恢复点回滚 | `backup_interrupted_restore_rolls_back` | 待实现 |
| 恢复后修改对象时撤回保留冲突项 | `backup_undo_fingerprint_conflict` | 待实现 |
| 跨机恢复未完成重定位前不读正文或启动 PTY | `backup_cross_machine_requires_relocation` | 待实现 |
| 目标机缺凭据时不从包内提取凭据 | `backup_restore_does_not_restore_credentials` | 待实现 |

### 跨域约定

设计：[架构](design/architecture.md)、[工作区](design/workspace.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 文件树与补全重绑后命令一致且终端中断保留 | `command_registry_reserved_terminal_interrupt` | 待实现 |
| 非模态操作栏不偷焦且可键盘到达 | `focus_nonmodal_no_steal` | 待实现 |
| 模态触发控件销毁后按链还焦 | `focus_modal_fallback_after_owner_destroy` | 待实现 |
| 重新可见只合并复核且丢弃过期结果 | `visibility_coalesces_refresh_and_drops_stale` | 待实现 |
| 隐藏后显式保存或文件操作结果仍可查询 | `operation_result_survives_view_hidden` | 待实现 |
| 日志、错误、备份和诊断不含敏感正文或凭据 | `sensitive_material_excluded_from_outputs` | 待实现 |
| 性能数值具有测量条件或属于契约数值 | `performance_claims_have_conditions` | 待实现 |

### Runtime v2

设计：[Runtime](design/runtime.md)、[生命周期](design/lifecycle.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| v1 方法、旧资源和旧发现格式明确拒绝 | `runtime_v1_rejected` | 待实现 |
| 脏关闭未提供策略时返回决定错误且不弹模态 | `runtime_close_requires_decision` | 待实现 |
| 显式 cwd 越权时不创建空 Panel 或改投活跃项目 | `runtime_cwd_does_not_fallback` | 待实现 |
| 破坏操作状态校验失败时不执行写入 | `runtime_destructive_precondition` | 待实现 |
| 结果不明时可按操作标识查询而不盲目重放 | `runtime_outcome_queryable` | 待实现 |
| 端点、设置、备份和诊断不含 Runtime token | `runtime_token_absent_from_materials` | 待实现 |
| 隐藏或切项目仍收合法事件且销毁后无过期事件 | `runtime_events_visibility_and_generation` | 待实现 |

### 设置与持久化

设计：[持久化](design/persistence.md)、[设置与恢复](design/settings-recovery.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 多项目与浮层修改后关闭视图不取消全局队列 | `settings_single_queue_across_views` | 待实现 |
| 权限、损坏、高版本和字段错误严格分流 | `settings_damage_classification` | 待实现 |
| 状态库及副本均不可恢复时隔离并明确提示 | `workspace_unrecoverable_isolated` | 待实现 |
| 快照配额不足时不驱逐打开脏 Document | `snapshot_quota_does_not_evict_dirty` | 待实现 |
| 异常启动先布局后对账且不重复创建 Panel | `startup_reconciles_layout_before_snapshot` | 待实现 |
| 无主快照无活跃项目时保留并等待显式处置 | `orphan_snapshot_requires_explicit_target` | 待实现 |
| 最终提交失败后放弃状态退出有旧状态提示 | `close_abandon_warns_about_old_state` | 待实现 |
| 普通设置导入拒绝未知版本或凭据字段 | `settings_import_rejects_unknown_or_credentials` | 待实现 |

### PathPolicy

设计：[领域模型](design/domain-model.md)、[文件与 Git](design/files-git.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 硬链接或不同词法路径按文件身份复用 Document | `path_file_identity_deduplicates_document` | 待实现 |
| 只读根与可写根覆盖时保存按能力并集 | `path_capabilities_union_for_save` | 待实现 |
| 链接重定向后写前重新校验并拒绝旧授权 | `path_rechecks_redirect_before_write` | 待实现 |
| 根外文件重启后只显示重新授权占位 | `outside_document_restart_requires_authorization` | 待实现 |
| 根外快照异常恢复不自动重读磁盘或恢复授权 | `outside_snapshot_restores_without_disk_read` | 待实现 |
| 失去读授权后保留内容并停止刷新 | `document_retains_content_after_read_revoke` | 待实现 |
| 项目根重定位不改绑既有 Diff 与 GitShow | `repository_binding_survives_project_relocation` | 待实现 |
| 真实路径查重失败时保留原项目关系 | `project_realpath_dedupe_failure_preserves_relation` | 待实现 |

### Diff

设计：[文件与 Git](design/files-git.md)、[Document](design/document.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 从 Commit 打开脏 Document 时直接使用共享文本 | `diff_uses_shared_document_from_commit` | 待实现 |
| Diff 工作区编辑在 Editor 与 Markdown 中一致 | `diff_edit_shared_document` | 待实现 |
| 根重定位后不读取新仓库同路径 HEAD | `diff_does_not_rebind_after_relocation` | 待实现 |
| 根外只读与另一可写根覆盖时仍可保存 | `diff_save_uses_capability_union` | 待实现 |
| Diff 展示参数恢复且搜索与临时提示清除 | `diff_view_state_restore_clears_transients` | 待实现 |
| 冲突快照不随源文本自动更新且不可保存 | `diff_conflict_snapshot_is_immutable` | 待实现 |
| 隐藏期间 HEAD 变化重新可见只合并复核 | `diff_hidden_head_refresh_coalesced` | 待实现 |
| 任一侧超限或含 NUL 时按侧降级 | `diff_large_and_binary_side_degradation` | 待实现 |

### 系统集成

设计：[Windows](design/windows.md)、[生命周期](design/lifecycle.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 托盘启用时关窗与托盘退出都走统一关闭 | `windows_tray_close_uses_close_coordinator` | 待实现 |
| 移除显示器后窗口夹回可见工作区 | `windows_geometry_clamped_to_work_area` | 待实现 |
| 架构或哈希不符的更新资产不安装不执行 | `update_asset_validation_rejects_mismatch` | 待实现 |
| 更新安装前先完成脏 Document 关闭守卫 | `update_install_uses_close_guard` | 待实现 |
| 桌面通知与任务栏闪烁独立判定 | `notification_and_taskbar_switches_independent` | 待实现 |
| 错误优先且工作进度不伪造百分比 | `taskbar_priority_and_indeterminate_progress` | 待实现 |
| 切换语言不重建工作区且缺译回退英文 | `i18n_switch_preserves_workspace` | 待实现 |
| 清理排除活跃恢复材料且审计独立确认 | `cleanup_excludes_active_recovery_material` | 待实现 |

### 工作区壳

设计：[工作区](design/workspace.md)、[生命周期](design/lifecycle.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 多项目切换只改变可见性且实例与进程不重建 | `workspace_project_switch_preserves_instances` | 待实现 |
| 共享 Document 只有最后引用关闭才触发内容守卫 | `document_last_reference_close_guard` | 待实现 |
| 批量保存失败取消关闭并保留全部 Panel 与 PTY | `workspace_batch_save_failure_cancels_close` | 待实现 |
| 项目移除先完成守卫与删除意图再销毁资源 | `project_remove_commits_intent_before_destroy` | 待实现 |
| 激活项目不关闭无项目浮层或其 Hooks 草稿 | `app_settings_overlay_keeps_hooks_draft` | 待实现 |
| 移交显式 cwd 未授权时拒绝且不改投项目 | `handover_rejects_unauthorized_cwd` | 待实现 |
| 最终退出提交失败后允许知情放弃状态 | `close_final_commit_failure_allows_explicit_abandon` | 待实现 |
| 异常启动先布局后对账且不重复 Panel 或 PTY | `workspace_startup_layout_before_snapshots` | 待实现 |

### Document

设计：[Document](design/document.md)、[领域模型](design/domain-model.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 跨项目 Editor 与 Diff 共享文本、撤销和冲突 | `document_shared_across_projects` | 待实现 |
| 同项目普通视图去重而 Diff 与 GitShow 独立 | `document_duplicate_view_focuses_existing` | 待实现 |
| 文件跨扩展名改名不自动转换 Panel 类型 | `document_rename_preserves_identity_and_view` | 待实现 |
| 另存为已打开目标时拒绝且两个 Document 不变 | `document_save_as_rejects_open_target` | 待实现 |
| 自动保存等待全部编辑视图失焦 | `document_autosave_waits_for_all_views_to_blur` | 待实现 |
| 批量关闭部分保存失败时保留所有 Panel | `document_batch_close_partial_save` | 待实现 |
| 关闭集合外仍有引用时不丢弃 Document | `document_discard_respects_external_reference` | 待实现 |
| 根重定位或外部改名不重建 Document 或视图 | `document_location_change_does_not_rebuild` | 待实现 |

### Editor

设计：[Document](design/document.md)、[文件与 Git](design/files-git.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| Editor 与 Diff 编辑保存共享文本和基线 | `editor_diff_share_document` | 待实现 |
| HTML 系统打开磁盘版本且不隐式保存 | `editor_html_system_open_does_not_save` | 待实现 |
| GBK、非法编码和 NUL 文件分别按规则路由 | `editor_encoding_and_binary_routing` | 待实现 |
| 全部替换确认后一次撤销回滚整次操作 | `editor_replace_all_single_undo` | 待实现 |
| 跨大文件阈值时干净切只读、脏保留缓冲 | `editor_large_file_dirty_preserves_buffer` | 待实现 |
| 语言、换行和滚动按 ViewKey 恢复且瞬态清除 | `editor_view_state_restore` | 待实现 |
| 外部改名只更新位置与显示而不换视图 | `editor_external_rename_keeps_view_type` | 待实现 |
| Git 行标打开同 Document Diff 并定位差异 | `editor_git_marker_opens_shared_diff` | 待实现 |

### Terminal

设计：[终端](design/terminal.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 新终端未给 cwd 使用项目根且非法 cwd 拒绝 | `terminal_new_uses_project_root` | 待实现 |
| 重启恢复可见、隐藏和已退出终端的不同运行态 | `terminal_restore_visibility_and_exit_state` | 待实现 |
| 存活 PTY 达上限时拒绝新启动且记录不丢 | `terminal_live_pty_limit` | 待实现 |
| 隐藏期间持续输出重新可见仍连续且只显示新画面 | `terminal_hidden_output_backpressure` | 待实现 |
| 不可信镜像不补全不采集且接受候选不自动执行 | `terminal_trusted_line_completion_gate` | 待实现 |
| cwd 变化丢弃旧补全且单来源失败不阻塞其他来源 | `completion_generation_and_source_failure` | 待实现 |
| 清空学习同时取消待写并清除内存 | `learning_clear_cancels_pending_writes` | 待实现 |
| 回滚搜索只读且不向 PTY 写搜索字节 | `terminal_search_is_read_only` | 待实现 |
| 非聚焦或隐藏项目 OSC 52 未授权时不改剪贴板 | `terminal_osc52_authorization` | 待实现 |

### 导航树

设计：[Agent](design/agents.md)、[Windows](design/windows.md)、[工作区](design/workspace.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 导航移除项目经统一守卫与删除意图 | `navigation_project_remove_uses_workspace_guard` | 待实现 |
| 根重定位检查冲突与 Hooks 且保持既有 PTY cwd | `navigation_relocation_guards_and_reassigns_history` | 待实现 |
| 嵌套或暂不可访问根仍按词法最深匹配归属 | `navigation_history_lexical_deepest_match` | 待实现 |
| 未归属历史恢复新项目前先确认目标根 | `navigation_unowned_restore_requires_project_confirmation` | 待实现 |
| 历史 cwd 失效可回退且注入失败不重复建终端 | `navigation_restore_fallback_and_injection_retry` | 待实现 |
| 连续删除的隔离批次期限彼此独立 | `quarantine_batches_have_independent_deadlines` | 待实现 |
| 撤销点击时重新检查绝对期限与批次状态 | `quarantine_undo_rechecks_deadline` | 待实现 |
| 删除后重启仍保留未到期撤销窗口 | `quarantine_restart_preserves_undo_window` | 待实现 |
| 完成提交前异常退出先对账来源与副本 | `quarantine_startup_reconciles_incomplete_batch` | 待实现 |
| 撤销目标外部新增或修改时不覆盖且保留材料 | `quarantine_undo_rejects_external_target` | 待实现 |
| 套餐只使用专用 Windows 凭据且材料不含 token | `plan_balance_uses_dedicated_credential` | 待实现 |
| 导航隐藏时套餐继续调度且退出后停止 | `plan_balance_runs_while_navigation_hidden` | 待实现 |
| 错过完整周期后才标记套餐结果过期 | `plan_balance_stale_after_full_period` | 待实现 |
| 导航瞬态清除而未到期隔离批次按规则恢复 | `navigation_transient_and_quarantine_restore` | 待实现 |

### 侧栏框架

设计：[工作区](design/workspace.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 同槽替换销毁旧实例并取消任务 | `sideview_same_slot_replacement_destroys_instance` | 已自动化 |
| 隐藏整个侧栏再显示保留实例与运行态 | `sideview_hide_preserves_instance` | 已自动化 |
| 隐藏按钮不移除内容或销毁实例 | `sideview_hide_button_preserves_instance` | 已自动化 |
| 跨槽交换保留双方实例而同槽替换才销毁 | `sideview_cross_slot_swap_preserves_instances` | 已自动化 |
| 关闭重开与重启共享 ViewKey 状态并清除瞬态 | `sideview_close_and_restart_share_view_state` | 已自动化 |
| 未知、重复和非法布局字段分别归一与回退 | `sideview_restore_normalizes_invalid_layout` | 已自动化 |
| 激活项目不关闭无项目设置浮层 | `app_overlay_does_not_close_on_project_activation` | 待实现 |
| 临时窗口压缩不覆盖用户保存的侧栏宽度 | `sidebar_temporary_compression_does_not_overwrite` | 待实现 |
| 视图状态写失败保留内存并在后续重试 | `view_state_save_failure_retains_memory` | 已自动化 |

### Settings 面板

设计：[设置与恢复](design/settings-recovery.md)、[Hooks](design/hooks.md)、[Agent](design/agents.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 浮层 Hooks 草稿与全局队列在激活项目后保留 | `settings_overlay_keeps_draft_and_queue` | 待实现 |
| 选中页按 ViewKey 恢复而搜索与录制态清除 | `settings_page_view_state_restore` | 待实现 |
| 多 Settings 视图同步生效值且关闭不取消队列 | `settings_views_share_global_queue` | 待实现 |
| 终端中断与文件树复制按上下文独立绑定 | `keybinding_reserved_contexts` | 待实现 |
| 完整主题可用、缺项零写入且使用中删除需另选 | `theme_import_complete_or_zero_write` | 待实现 |
| Editor、Markdown、Diff 遵循同一 Document 自动保存 | `settings_document_autosave_is_global` | 待实现 |
| 套餐不读取 CLI 凭据且专用 token 不回显 | `settings_plan_token_not_cli_credentials` | 待实现 |
| 后台周期提交后按新周期运行且隐藏仍查询 | `settings_plan_schedule_commit_and_visibility` | 待实现 |
| 自定义根真实目录重复拒绝且删除立即撤销授权 | `agent_root_duplicate_and_revocation` | 待实现 |
| AgentRoot 默认只读且写与终端分别拒绝 | `agent_root_default_capabilities` | 待实现 |
| Hooks 外部修改时不覆盖文件或静默丢草稿 | `hooks_external_modification_guard` | 待实现 |
| 注入提交后清理失败显示已提交与重试 | `hooks_post_commit_cleanup_failure_is_visible` | 待实现 |
| 导入拒绝凭据／高版本且恢复默认不删凭据 Hooks | `settings_import_and_reset_preserve_secrets` | 待实现 |

### Commit

设计：[文件与 Git](design/files-git.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| staged 与 unstaged 并存时回滚保留 staged 差异 | `commit_rollback_preserves_staged_changes` | 待实现 |
| 纯 staged、staged rename 和 conflict 禁用通用回滚 | `commit_rollback_disabled_shapes` | 待实现 |
| 未暂存 deleted 从索引版本恢复而非 HEAD | `commit_deleted_restores_index_version` | 待实现 |
| 未暂存 rename 恢复旧路径并清除新路径且不改索引 | `commit_unstaged_rename_rollback` | 待实现 |
| 回滚前完成脏 Document 守卫且取消不写盘 | `commit_rollback_document_guard` | 待实现 |
| Commit 打开已有脏 Document 的 Diff 使用共享文本 | `commit_opens_shared_document_diff` | 待实现 |
| GitShow 创建后不随 HEAD 或项目根变化 | `commit_gitshow_fixed_version` | 待实现 |
| staged added 删除先取消暂存再进回收站 | `commit_staged_added_delete_sequence` | 待实现 |
| 无回收站时永久删除必须再次明确确认 | `commit_untracked_permanent_delete_requires_confirmation` | 待实现 |
| RepoRoot 只读与另一可写根按能力并集判定 | `commit_rollback_capability_union` | 待实现 |
| 菜单打开后状态变化会在写前重检并中止 | `commit_rechecks_state_before_rollback` | 待实现 |
| 折叠、ignored 和滚动恢复且过滤与错误清除 | `commit_view_state_restore` | 待实现 |

### Markdown

设计：[Markdown](design/markdown.md)、[Document](design/document.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 源码修改切渲染不读盘且共享最新 Document | `markdown_source_render_share_document` | 待实现 |
| 形态、大纲、缩放和阅读位置按同一 ViewKey 恢复 | `markdown_view_state_restore` | 待实现 |
| 危险 HTML、远程字体和外部 SVG 不执行不加载 | `markdown_unsafe_html_svg_blocked` | 待实现 |
| 有效与无效 Mermaid 的静态渲染及错误边界 | `markdown_mermaid_static_final_capability` | 延期 |
| 远程图片点击前零请求且请求不带凭据或 Referer | `markdown_remote_image_click_authorization` | 待实现 |
| 根外文档的相邻图片不因文档可读而获批 | `markdown_outside_document_resource_not_authorized` | 待实现 |
| 大文档降级优先于持久渲染形态 | `markdown_large_document_degradation` | 待实现 |
| Markdown 共享 Document 按统一自动保存触发 | `markdown_shared_document_autosave` | 待实现 |

### Explorer

设计：[文件与 Git](design/files-git.md)、[工作区](design/workspace.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 另一项目已打开脏文件时再次打开复用 Document | `explorer_open_reuses_document` | 待实现 |
| 树加载后链接重定向会在写前重检并拒绝 | `explorer_rechecks_links_before_mutation` | 待实现 |
| 只读 AgentRoot 到可写 ProjectRoot 可复制而反向拒绝 | `explorer_copy_capability_union` | 待实现 |
| 跨根剪切和目录粘贴到后代逐项拒绝 | `explorer_rejects_cross_root_cut_and_descendant_paste` | 待实现 |
| 显式 cwd 失效时在终端打开拒绝且不留空 Panel | `explorer_terminal_open_rejects_invalid_cwd` | 待实现 |
| 过滤与多选清除而展开、滚动和忽略开关恢复 | `explorer_view_state_restore` | 待实现 |
| 关闭视图不自动提交未确认内联编辑 | `explorer_inline_edit_not_committed_on_destroy` | 待实现 |
| 粘贴冲突取消零修改且部分失败不回滚成功项 | `explorer_paste_conflict_and_partial_success` | 待实现 |
| 不可见期间文件事件重新可见只合并复核 | `explorer_hidden_refresh_coalesced` | 待实现 |
| 项目根为仓库子目录时 Git 读取不扩大树根 | `explorer_repo_root_does_not_expand_tree_root` | 待实现 |

### Agent 全局文件

设计：[Agent](design/agents.md)、[文件与 Git](design/files-git.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| 新 AgentRoot 默认可读但写入和终端分别拒绝 | `agent_files_default_capabilities` | 待实现 |
| 开启写入不隐含终端 cwd 能力 | `agent_files_write_does_not_imply_terminal` | 待实现 |
| 链接别名指向同一真实目录时不形成重复根 | `agent_root_real_directory_dedup` | 待实现 |
| 删除根后 Document 保留且既有 PTY 不被强杀 | `agent_root_delete_preserves_document_and_pty` | 待实现 |
| 多根覆盖同一文件时保存按能力并集与撤销更新 | `agent_root_capability_union_and_revoke` | 待实现 |
| 跨根复制逐项执行而跨根剪切拒绝 | `agent_files_cross_root_copy_and_cut` | 待实现 |
| 根层过滤不读取被隐藏内容且不是授权 | `agent_files_filter_is_not_authorization` | 待实现 |
| 从 Agent 树打开同一文件仍复用共享 Document | `agent_files_open_reuses_document` | 待实现 |
| 展开、当前行和滚动恢复而多选与错误清除 | `agent_files_view_state_restore` | 待实现 |
| 根改路径后旧授权与相对视图状态不套用 | `agent_root_path_change_resets_state_and_authorization` | 待实现 |
| 不可见或根收起时释放监听且无过期结果回填 | `agent_files_visibility_controls_watchers` | 待实现 |

### GitShow

设计：[文件与 Git](design/files-git.md)。

| 验收场景 | 目标测试例名 | 状态 |
|---|---|---|
| HEAD 前进或根重定位后仍显示创建时版本 | `gitshow_fixed_commit_after_head_move` | 待实现 |
| 同一固定版本重复打开聚焦且不重读 | `gitshow_duplicate_view_focuses` | 待实现 |
| 手动刷新只重读同一固定对象 | `gitshow_refresh_reads_same_object` | 待实现 |
| 固定对象不可用时保留错误且不回退工作区 | `gitshow_missing_object_preserves_error` | 待实现 |
| 大文件或二进制按阈值降级且系统打开不写工作区 | `gitshow_large_and_binary_degradation` | 待实现 |
| 固定身份与布局恢复而搜索、滚动和焦点清除 | `gitshow_view_state_restore` | 待实现 |
