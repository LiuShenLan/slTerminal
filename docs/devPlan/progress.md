# 开发进度

> 记录命名阶段的当前实施状态与已验收证据，作为长期开发的唯一进度入口。

## 决策与红线

- 状态只取 `未开始`、`进行中`、`已验收`、`阻塞`；不使用主观百分比或“基本完成”。
- 当前实施位置和阶段状态变化时更新；集成检查点验收后也更新，不记录逐提交流水账。
- `已验收` 必须同时带验收日期、commit hash 和实际测试证据。只有代码存在、定向测试曾通过或计划已写完均不足以验收。
- `阻塞` 只记录已经用户知悉并影响继续实施的事实；新发现的问题先按协作纪律报告，不把本文件变成问题清单。
- 不记录临时想法、评审过程、会议摘要或第二份待办。尚未开始的命名阶段本身即表示剩余范围。
- 设计、测试和豁免链接必须指向真实文件与例名；进度不得与 [追溯矩阵](traceability.md) 的核销状态矛盾。

## 验收与测试

- 每次阶段验收时核对 commit 工作树、验收日期和测试输出，再更新本文件。
- 最终交付前检查所有阶段均有有效证据锚；无锚点的完成措辞视为未验收。
- 文档策略检查不得把本文件中的日期、commit hash 或契约数值误判为过程编号。

## 操作

### 当前进度

状态锚：**2026-11-26 的最近阶段验收记录；命名阶段的历史验收不表示发布就绪。**

当前实施位置：**评审修复与未接线产品面补齐进行中；安装器与发布包实机项仍待发布机核验。**

| 命名阶段 | 状态 | 验收日期 | commit hash | 测试证据／阻塞事实 |
|---|---|---|---|---|
| 依赖基线与删减 | 已验收 | 2026-10-26 | `e4f4d88b` | `javascript_grammar_loads_with_real_highlights`、全量 workspace、GPUI／ConPTY 与脚本门禁通过；[真机证据](../../.claude/docs/architecture/provenance.md)与[许可及体积基线](../../.claude/docs/release/packaging.md)已核验。 |
| 领域基础 | 已验收 | 2026-10-26 | `c38267b2` | 临时双根下的数据库事务与损坏分流、授权能力并集、设置单字段回退、原子写及审计对账用例通过；全量 workspace、GPUI、脚本与静态门禁全绿；数据根环境读取仅限启动入口。 |
| 工作区与生命周期 | 已验收 | 2026-11-22 | `ed4da6df` | `workspace_project_switch_preserves_instances`、`integration_batch_close`、`workspace_startup_layout_before_snapshots`、`handover_rejects_unauthorized_cwd` 等验收与回归例通过；GPUI 全特性、全量 workspace、core release、脚本及静态门禁全绿。 |
| Document 与文本视图 | 已验收 | 2026-11-22 | `97cb2b3c` | `integration_document_global_identity`、`document_autosave_waits_for_all_views_to_blur`、`integration_snapshot_quota`、`ime_commits_utf16_edits_through_the_shared_document` 等主责锚通过；全量 workspace、GPUI 全特性、core release、脚本及静态门禁全绿。 |
| 终端、补全与学习 | 已验收 | 2026-11-23 | `88e3b4a9c61f5cf6a53922257d0c1fc4d583a833` | `integration_terminal_restore`、真实 ConPTY 自动／懒启动与失效 cwd 回退、`terminal_restore_visibility_and_exit_state`、`integration_pty_limit` 等回归锚通过；全量 workspace、core release、脚本及静态门禁全绿。 |
| Markdown 与安全资源 | 已验收 | 2026-11-23 | `a6eb9f56b2483a56a0c3017c3be21051af5c78e1` | `markdown_source_render_share_document`、`markdown_shared_document_autosave`、`markdown_panel_restores_persistent_reading_state`、真实拖选／查找与退出守卫回归通过；全量 workspace、GPUI 全特性、core release、脚本及静态门禁全绿。 |
| 文件与 Git | 已验收 | 2026-11-24 | `0c92d3f5` | Explorer、Commit、Diff、GitShow 全部验收锚（含 `explorer_repo_root_does_not_expand_tree_root`、`integration_commit_rollback`、`commit_staged_added_delete_sequence`、`diff_hidden_head_refresh_coalesced`、`gitshow_fixed_commit_after_head_move`）通过；全量 workspace、GPUI 全特性、core release、脚本及静态门禁全绿。 |
| Agent 与 Hooks | 已验收 | 2026-11-25 | `aadca3fa` | 能力目录、历史扫描／恢复／隔离批次与撤销、Agent 文件树；hook 小进程桥经真实命名管道用例 `pipe_server_shutdown_joins_the_accept_thread`、`real_pipe_prompt_submit_reaches_the_owning_pane_host`、`real_pipe_foreign_payload_is_dropped_and_turn_done_is_owned_by_pane` 核验；Hooks 编辑页 `hooks_page_loads_saves_external_guard_and_discards`，AgentHooks 页 `agent_hooks_page_renders_status_and_toggles_through_injected_ports`；提交后清理失败由 `agent_hooks_cleanup_failure_offers_cleanup_only_retry` 与 `cleanup_reports_leftover_materials_instead_of_swallowing_errors` 覆盖只重试清理；全量 workspace、GPUI 全特性、core release 与脚本门禁全绿。 |
| 设置、主题与恢复 | 已验收 | 2026-11-25 | `d9fba527` | `settings_single_queue_across_views`、`settings_overlay_controls_write_runtime_values_and_queue`、`theme_import_complete_or_zero_write`、`backup_tamper_rejected_zero_write`、`staged_restore_is_applied_at_startup_and_rotates_store_epoch`、`interrupted_apply_is_rolled_back_on_restart`、`backup_cross_machine_requires_relocation`、`maintenance_restore_leaves_credentials_and_unlisted_files_untouched` 通过；DPAPI 恢复点与维护期世代轮换已落地；全量 workspace、GPUI 全特性、core release 与脚本门禁全绿。 |
| Runtime v2 | 部分验收 | 2026-11-28 | `b47e9b92` | 进程内合同：全部线缆方法族（含 Git 破坏写、面板打开/保存/关闭、App 聚焦/非交互关闭、项目移除策略）、单实例同构、凭据边界与诊断 token 白名单锚（`runtime_token_absent_from_materials`）通过；全量 workspace、GPUI 全特性、core release 与脚本门禁全绿。真实 GUI 进程 + 真实 CLI 客户端的双进程 conformance 与事件断线矩阵按豁免表登记，留发布前临时双数据根人工核验，未标自动化。 |
| Windows 与发布 | 已验收 | 2026-11-26 | `4a82ca15` | 平台接入（WindowsIntegration、窗口几何、套餐调度、通知与任务栏、i18n 热切换、更新守卫与启动对账、诊断白名单、清理）切片全绿；发布钉测试（profile、清单、体积预算、随包运行时）PASS；debug 构建产物真实运行矩阵（单实例窗口、第二实例移交、关窗退出、`clean_shutdown` 落盘、无残留进程）通过；安装器与发布包实机项按豁免留发布机。 |
| 全量验收 | 已验收 | 2026-11-26 | `f8c42a32` | 跨域验收锚自动化核销；全量 workspace、GPUI、core release、两套 clippy、架构／文档策略／禁名门禁与脚本自测全绿；许可清单按锁文件原子重建并复核通过；敏感材料扫描仅命中假值占位。 |

### 更新方式

- 阶段开始只更新当前实施位置和该阶段状态；阶段完成填写日期、完整或足以唯一定位的 commit hash 与证据。
- 证据优先引用 [追溯矩阵](traceability.md) 中已核销场景及实际测试命令；实机人工项注明对应豁免登记与核验记录。
- 若提交修正了已验收阶段的行为，重新验证受影响证据，不沿用旧验收结论。
