# 实施计划

> 按依赖闭合组织命名阶段、集成检查点和提交证据，不要求中间形成完整可用产品。

## 决策与红线

### 通用交付规则

- 每阶段先建立类型、状态机和假端口测试，再接真实 I/O 与 GPUI；不得先写不可测试的界面流程再补领域规则。
- 下方“执行拆分”的每个行为行对应一个独立提交，交付单元同时包含实现、自动化测试和必要的所有者文档；合同未变化时无需为形式同步文档。已有实现只作迁移、扩展或接线，不按阶段标题重建；已有证据只作回归锚，不虚构重新实现。
- 行为行只登记前置与交付验收锚，不携带状态、日期或提交 hash。真实测试产生后同步对应追溯场景；跨域集成在所涉能力阶段核销，不积压到最终验收。
- 命名阶段按本文排列顺序推进；入口依赖提前满足不授权跨阶段并行或前移。测试假端口不得变成生产占位面板、伪成功入口或第二套领域通道。
- 每阶段列出的退出条件是进入下一阶段的依赖保证，不是允许跳过其他门禁的理由。所有提交持续执行架构、文档、禁名、clippy、fmt 和串行测试。
- 删除旧实现时同步删除或重写其测试、资源、文档和发布资产；不保留编译关闭的旧功能、兼容适配或占位入口。
- 阶段状态只在对应证据真实产生后更新到 [进度记录](progress.md)。未完成不得以“代码已存在”标记已验收。

### 依赖基线与删减

**入口**：目标设计与依赖裁决已完成。

**交付**：新 workspace 骨架与依赖方向；固定依赖和 grammar 清单；GPUI 补丁审计；删除 AI Providers／Assistant、Merge、终端数学、旧 Runtime、旧主题包和其他最终排除实现。

**退出证据**：所有 grammar 在 Windows x64 通过编译、ABI 与 query 加载；许可与 notice 完整；全量门禁在无旧模块引用的情况下全绿；记录新体积实测基线。

### 领域基础

**入口**：依赖基线与删减已验收。

**交付**：AppPaths、稳定 ID、PathPolicy、SettingsRegistry、主题 token、typed i18n、工作区库与学习库、ViewState DTO、Operation／ManagedOperationJournal、原子文件和审计端口。

**退出证据**：临时数据根下的数据库事务、损坏分流、授权能力并集、设置单字段回退、原子写和日志对账测试通过；无模块自行读取数据根环境变量。

### 工作区与生命周期

**入口**：领域基础已验收。

**交付**：AppRoot、ProjectView、PanelHost、SideViewHost、泛型布局、CommandRegistry、FocusManager、TaskService、Startup／CloseCoordinator、LocalControlServer 与单实例移交。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| 布局命令与 GPUI 接线 | 既有 `ProjectLayout` 转换对与稳定身份 | 把拆分、合并、移动、尺寸调整接入统一命令与 AppRoot；核销“布局操作保留实例与焦点”场景，不重建布局核心。 |
| 页签溢出与完整列表 | 布局命令接线 | 实现横向溢出和完整列表面板，验证无 ViewKey 丢失、焦点与激活项一致。 |
| 多项目运行态集成 | 既有 ProjectView 与 TerminalRegistry | 只补项目切换的跨域证据，确认实例、PTY 与隐藏输出不重建；核销 `workspace_project_switch_preserves_instances`。 |
| 侧栏显隐跨域集成 | 既有 SideViewHost、TaskService | 复用现有实例语义补齐可见性集成证据；核销 `integration_sidebar_visibility_semantics`，不重写侧栏生命周期。 |
| 项目重命名、排序与查重失败 | 既有 `ProjectRegistry`、真实路径端口 | 接通剩余项目命令；真实路径查重失败保持原关系，核销 `project_realpath_dedupe_failure_preserves_relation`。 |
| 组内批量关闭 | 既有 `CloseCoordinator`、布局命令 | 完成关闭其他／全部、空组清理与失败保留，核销 `integration_batch_close`；共享 Document 的真实守卫留 Document 阶段扩展同一协调器。 |
| 退出放弃的旧状态提示 | 既有全局退出与知情放弃链 | 补足最终提交失败后放弃的实际提示与测试证据；核销 `close_abandon_warns_about_old_state`，不改变既有提交顺序。 |
| 面板循环与关闭还焦 | 既有 FocusManager、PanelHost | 补齐 `Ctrl+Tab`／反向循环及剩余关闭落点，验证终端和编辑器不能截获全局路由。 |
| 任务结果代次与可见性复核 | AppRoot 所有权树、既有 TaskService | 完成隐藏后合并复核、过期结果丢弃和显式操作结果查询，核销 `visibility_coalesces_refresh_and_drops_stale`、`operation_result_survives_view_hidden`。 |
| 启动布局与快照对账骨架 | 布局持久化、既有 MaintenanceRunner | 保证布局先于快照且非法引用确定归一；只使用测试假端口，核销 `workspace_startup_layout_before_snapshots`、`workspace_unrecoverable_isolated`，真实快照恢复器留 Document 阶段。 |
| 单实例显式路径复核 | 既有 LocalControlServer 与 handover | 补齐显式 cwd 越权拒绝且不改投项目，核销 `handover_rejects_unauthorized_cwd`；`OpenFile` 不在此阶段接占位链。 |
| 无项目设置浮层宿主 | AppRoot、既有 Settings 视图 | 保证项目激活不销毁浮层、独立 ViewKey 与重复入口聚焦，核销 `app_overlay_does_not_close_on_project_activation`；Hooks 草稿生命周期留 Agent 与 Hooks 阶段。 |

**退出证据**：布局属性测试、GPUI 焦点与 IME 测试、启动维护崩溃序列、统一关闭守卫和双实例移交通过；第二实例连接失败不会创建第二窗口。

### Document 与文本视图

**入口**：工作区与生命周期已验收。

**交付**：DocumentRegistry／DocumentSession、Rope 编辑与撤销、编码与行尾、分页只读、保存与外部修改、崩溃快照、LanguageRegistry、Tree-sitter、TextSource 和自定义 GPUI 文本视图。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| Document 聚合与真实打开 | 工作区 PanelHost、PathAuthority | 收编现有编辑入口为唯一 DocumentRegistry／DocumentSession，按文件身份去重并保证打开失败不留空 Panel；核销 `integration_document_global_identity`、`path_file_identity_deduplicates_document`。 |
| 编辑事务与撤销清脏 | DocumentSession | 实现 `EditTransaction`、`text_revision`、`TextStateId`、合并与预算；以 Rope 属性测试覆盖 Unicode、撤销／redo 和清脏。 |
| 编码与内容路由 | 真实文件读取端口、DocumentSession | 完成 BOM、编码、行尾、末尾换行、NUL、非法编码和大小阈值分流；核销 `editor_encoding_and_binary_routing`。 |
| 并发原子保存 | 编辑事务、原子文件端口 | 后台编码与写前复核，保存期间编辑保持当前脏态；覆盖 ACL、不可表示字符和 rename 失败，核销 `path_capabilities_union_for_save`、`path_rechecks_redirect_before_write`。 |
| 另存为与外部变化 | 并发保存、文件身份与授权代次 | 完成目标占用拒绝、外部修改／删除、共享冲突与位置重绑；核销 `document_save_as_rejects_open_target`、`document_location_change_does_not_rebuild`、`document_retains_content_after_read_revoke`。 |
| 应用级自动保存 | 外部变化处理、Editor 引用关系 | 按停止修改、全部编辑视图失焦、最后引用和退出触发；核销 `document_autosave_waits_for_all_views_to_blur`、`settings_document_autosave_is_global` 的 Document 部分。 |
| 最后引用与批量关闭守卫 | DocumentRegistry、既有 CloseCoordinator | 扩展同一关闭主干处理共享引用、保存／丢弃决定和部分失败；核销 `document_last_reference_close_guard`、`document_discard_respects_external_reference`、`document_batch_close_partial_save`、`workspace_batch_save_failure_cancels_close`、`integration_last_reference_close`。 |
| Paged 只读与外部增长 | 编码路由、DocumentContent | 实现源范围分页、页边界和 LRU；干净文档可降级、脏文档保留保存出口，核销 `editor_large_file_dirty_preserves_buffer`。 |
| 后台查找、复制与替换全部 | Rope／Paged TextSource | 实现代次取消、完整复制、正则捕获和单撤销替换；核销 `editor_replace_all_single_undo`。 |
| 崩溃快照写入与配额 | TextStateId、原子文件与独立写队列 | 实现自描述快照、flush、配额和删除时机；核销 `snapshot_quota_does_not_evict_dirty`、`integration_snapshot_quota`，并覆盖快照删除边界崩溃。 |
| 快照启动恢复器 | 快照写入、既有 MaintenanceRunner | 注册真实快照恢复器，区分有主、无主与根外授权；核销 `startup_reconciles_layout_before_snapshot`、`outside_snapshot_restores_without_disk_read`、`orphan_snapshot_requires_explicit_target`、`outside_document_restart_requires_authorization`、`integration_session_grant_restore`。 |
| 语言注册与增量语法 | 编辑事务、既有 grammar 资产 | 完成封闭 LanguageRegistry、真实查询映射和 SyntaxWorker 中止；覆盖隐藏释放、revision 丢弃和重新可见重建。 |
| 专用文本视图与 IME | DocumentSession、FocusManager | 收编现有视图为 Rope／Paged `TextSource`，完成可见行、布局缓存、选区、Unicode 显示宽度与 IME；禁止第二份实时 String 缓冲。 |
| Editor 视图与身份集成 | 专用文本视图、PanelHost | 完成 ViewState、改名身份保持、普通视图去重、HTML 系统打开与跨项目共享；核销 `editor_view_state_restore`、`editor_external_rename_keeps_view_type`、`editor_html_system_open_does_not_save`、`document_rename_preserves_identity_and_view`、`document_duplicate_view_focuses_existing`、`document_shared_across_projects`。 |
| 真实 OpenFile 移交 | Editor 视图集成、既有 LocalControlServer | 将 `instance.handover` 的 `OpenFile` 接入真实 Document 与 Editor 落点；核销 `integration_cross_project_views`，越权或打开失败不得创建空 Panel。 |

**退出证据**：跨项目／跨视图共享 Document、并发保存、撤销清脏、编码失败、超限降级、快照恢复和 IME 测试通过；不存在 InputState 或 String 形式的第二实时缓冲。

### 终端、补全与学习

**入口**：Document 与文本视图已验收。

**交付**：TerminalSession worker、PTY 代次、背压与 resize 水位、ShellLineTracker、Shell 适配器、CompletionService、learning.db、终端搜索和元数据恢复；提前收编应用级 GitFacade 的受控只读 refs／remotes 能力供补全使用。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| TerminalRegistry 与单 worker 所有权 | Document 阶段完成、既有终端记录 | 收紧 TerminalId、PTY 代次、进程身份和有界端点，确保 Panel 与 Runtime 不直持 Term 或子进程句柄。 |
| 真实 ConPTY 与存活上限 | 单 worker 所有权 | 接通真实创建、输入输出、退出保留与失败重试；核销 `terminal_live_pty_limit`、`integration_pty_limit`。 |
| 输出背压与隐藏恢复 | ConPTY worker | 同时限制块数与字节，隐藏只暂停绘制且恢复最新画面；核销 `terminal_hidden_output_backpressure`。 |
| resize 水位与重连代次 | 输出背压 | 完成水位后排版、初始尺寸、重连分隔和旧搜索／读取失效；真实 ConPTY 覆盖洪峰、resize 与重连。 |
| Shell 探测与 cwd 授权 | ConPTY、PathPolicy | 收编显式／默认 Shell、确定性探测和终端 cwd 复核；核销 `terminal_new_uses_project_root`、`integration_new_terminal_cwd`。 |
| ShellLineTracker 与执行确认 | Shell 适配器、PTY 语义事件 | 闭合 PowerShell／cmd 的本地镜像、回显对账、失效条件与执行确认；未闭合前不得开放采集或补全。 |
| 学习写队列与清空 | 可信 Shell 行、独立学习库端口 | 完成可信采集、去重／frecency、容量、损坏暂停和代次清空；核销 `learning_clear_cancels_pending_writes`、`integration_local_learning_data`。 |
| 纯补全方言与编辑计算 | ShellLineTracker | 收编 `slterm_completions` 的方言、匹配和 `CompletionEdit`，覆盖原文指纹与接受后不执行。 |
| GitFacade 只读收编 | 既有 `git_completion` 查询与执行元数据 | 在 application 服务建立语义化 refs／remotes 只读入口，迁移现有消费者；覆盖取消、超时、输出上限、禁止网络与写命令，禁止另建 argv 通道。 |
| CompletionService 调度与来源 | 纯补全核心、GitFacade 只读入口、可信边界 | 完成历史、目录、命令、路径、Git 与静态项目脚本来源及代次取消；核销 `completion_generation_and_source_failure`、`terminal_trusted_line_completion_gate`、`integration_completion_trust_boundary`。 |
| 按键、粘贴与 OSC 授权 | CommandRegistry、终端 worker | 完成保留键、括号粘贴、危险确认、OSC 8／52；核销 `terminal_osc52_authorization`、`command_registry_reserved_terminal_interrupt`、`keybinding_reserved_contexts` 的终端部分。 |
| 只读回滚搜索 | 终端 worker 快照 | 搜索只读当前画面与回滚，新输出使旧结果失效；核销 `terminal_search_is_read_only`、`integration_terminal_search`。 |
| 真实恢复与 CLI 信号 | 终端 worker、Shell 适配器、学习／补全已闭合 | 扩展现有恢复状态到真实 PTY 自动／懒启动、重连、cwd 回退、历史注入与 CLI 标题状态；以 `terminal_restore_visibility_and_exit_state` 为回归锚并核销 `integration_terminal_restore`。 |

**退出证据**：真实 ConPTY 的输入输出、洪峰、resize、退出和重连通过；不可信行不补全不采集，旧候选不回填，清空学习取消待写；64 个存活 PTY 上限按进程计数。

### 文件与 Git

**入口**：终端、补全与学习已验收。

**交付**：WatcherService、FileOperationService、Explorer、Commit、Diff、GitShow、Editor Git 行标和受限破坏操作；扩展终端阶段同一 GitFacade，以支持仓库绑定、状态、固定版本读取与破坏操作。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| 共享 FileTreeModel | SideViewHost、PathPolicy | 完成分批读取、稳定排序、过滤、根代次与树边界；核销 `explorer_repo_root_does_not_expand_tree_root` 的模型部分。 |
| 根作用域 WatcherService | FileTreeModel、TaskService | 完成事件合并、可见性暂停、重新可见复核与所有者回收；核销 `explorer_hidden_refresh_coalesced`。 |
| OperationPlan 与单项操作 | PathResolver、原子文件与回收站端口 | 完成新建、重命名、单项删除及执行前重解析；核销 `explorer_rechecks_links_before_mutation`、`integration_links_and_file_identity`。 |
| 批量复制、剪切与替换恢复 | OperationPlan、ManagedOperationJournal | 完成冲突聚合、部分成功、跨根规则与专属恢复器；核销 `explorer_paste_conflict_and_partial_success`、`explorer_rejects_cross_root_cut_and_descendant_paste`、`explorer_copy_capability_union`，覆盖真实提交边界崩溃。 |
| GitFacade 状态与仓库绑定 | 终端阶段只读 GitFacade | 扩展同一服务实例，接入 porcelain v2、进程预算、仓库代次与绑定复核；核销 `integration_path_capability_union` 的 Git 部分。 |
| DiffBinding 与差异核心 | Document TextStateId、RepositoryBinding | 完成不可变输入、Myers 行对、空白模式、deadline 和按侧降级；核销 `diff_large_and_binary_side_degradation`、`diff_conflict_snapshot_is_immutable`。 |
| GitShow 固定版本 | GitFacade、Paged TextSource | 完成固定对象读取、对象错误、大文件／二进制降级和系统打开中转；核销 `gitshow_fixed_commit_after_head_move`、`gitshow_duplicate_view_focuses`、`gitshow_refresh_reads_same_object`、`gitshow_missing_object_preserves_error`、`gitshow_large_and_binary_degradation`、`gitshow_view_state_restore`。 |
| Diff GPUI 与 Editor 行标 | Diff 核心、专用文本视图 | 接入共享工作区 Document、HEAD 刷新、ViewState 与行标定位；核销 `diff_uses_shared_document_from_commit`、`diff_edit_shared_document`、`diff_save_uses_capability_union`、`editor_diff_share_document`、`editor_git_marker_opens_shared_diff`、`diff_hidden_head_refresh_coalesced`、`diff_view_state_restore_clears_transients`、`integration_git_document`。 |
| Explorer 完整接线 | 文件操作、WatcherService、DocumentRegistry、TerminalRegistry | 完成内联编辑、打开复用、终端落点和 ViewState；核销 `explorer_open_reuses_document`、`explorer_terminal_open_rejects_invalid_cwd`、`explorer_inline_edit_not_committed_on_destroy`、`explorer_view_state_restore`、`keybinding_reserved_contexts` 的文件树部分。 |
| Commit 模型与完整打开映射 | GitFacade 状态、Diff、GitShow | 完成分组、过滤、ignored、ViewState 和 Document／Diff／GitShow 落点；核销 `commit_opens_shared_document_diff`、`commit_gitshow_fixed_version`、`commit_view_state_restore`。 |
| 通用回滚与写前重检 | OperationPlan、Document 关闭守卫、Journal 恢复器 | 完成脏 Document 决定、当前索引恢复和禁用形态；核销 `commit_rollback_document_guard`、`commit_rechecks_state_before_rollback`、`commit_rollback_preserves_staged_changes`、`commit_rollback_disabled_shapes`、`commit_deleted_restores_index_version`、`commit_rollback_capability_union`。 |
| 未暂存 rename 回滚 | 通用回滚、FileOperationService | 从索引恢复旧路径并清除新路径且不改索引，失败按部分成功报告；核销 `commit_unstaged_rename_rollback`，覆盖双路径提交边界崩溃。 |
| staged added 删除与永久确认 | 通用回滚、回收站策略 | 先取消暂存再进入回收站，无回收站时再次明确确认；核销 `commit_staged_added_delete_sequence`、`commit_untracked_permanent_delete_requires_confirmation`。 |

**退出证据**：链接重定向、能力并集、批量部分成功、porcelain 解析、staged／unstaged 对账、固定仓库绑定和差异预算测试通过；回滚只从当前索引恢复工作区。

### Markdown 与安全资源

**入口**：Document、PathPolicy 和文本视图可用。

**交付**：Comrak 扁平块投影、锚点、查找、安全 HTML／SVG、图片授权与缓存、公式、渐进渲染和阅读状态。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| Comrak 唯一块投影 | DocumentSession、Comrak | 完成 GFM、front matter、块模型、标题锚点、大纲、查找和链接事实；Mermaid 仅保留普通代码块。 |
| 安全 HTML 白名单投影 | Comrak 投影、html5ever TreeSink | 完成有界白名单与危险语料，禁止 HTML／WebView 输出；核销 `markdown_unsafe_html_svg_blocked` 的 HTML 部分。 |
| SVG 与本地图片安全链 | PathAuthority、图片解码端口 | 完成 SVG 清洗、静态光栅化、逐资源授权、解码预算和缓存；核销 `markdown_outside_document_resource_not_authorized` 及 SVG 安全部分。 |
| 远程图片与链接分派 | 资源授权键、有界 HTTP 与系统打开端口 | 完成逐 URL 单次授权、srcset 独立判定、无重定向／凭据／Referer 和本地链接授权；核销 `markdown_remote_image_click_authorization`。 |
| 本地公式与位图缓存 | 既有公式能力、主题 revision | 收编 OpenType MATH 布局、后台位图和失败降级，验证缓存键与零外部请求。 |
| 虚拟化渐进渲染 | 块投影、公式／图片服务 | 完成视口分片、取消、单块失败与大文档强制降级；核销 `markdown_large_document_degradation`。 |
| Markdown 双态与阅读状态 | GPUI 块视图、Editor 文本能力 | 源码／渲染共享 Document，接入统一自动保存和 ViewState；核销 `markdown_source_render_share_document`、`markdown_shared_document_autosave`、`markdown_view_state_restore`。 |

**退出证据**：危险输入语料无脚本、危险协议、未授权网络或外部 SVG 引用；源码与渲染共享 Document；远程图片只经单次点击授权；`mermaid` 仅为普通代码块。

### Agent 与 Hooks

**入口**：终端、文件操作、设置注册表和侧栏框架可用。

**交付**：AgentCatalog、活跃会话关联、原生历史扫描、导航恢复／分支、隔离批次、AgentRoot、Hooks 草稿与注入、命名管道和 hook 小进程桥。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| AgentCatalog 收编 | 既有 `slterm_agents`、SettingsRegistry | 统一 AgentKind、别名、会话 ID 和 start／resume／fork 构造器，删除 UI 或 Runtime 的平行名单。 |
| AgentRoot 注册与撤权 | PathAuthority、设置持久化 | 完成真实目录查重、独立能力、改路径和删除撤权；核销 `agent_root_duplicate_and_revocation`、`agent_root_default_capabilities`、`agent_root_real_directory_dedup`、`agent_root_path_change_resets_state_and_authorization`。 |
| 活跃会话事件关联 | TerminalRegistry、AgentCatalog | 由终端事件派生关联，覆盖 PTY 代次失效、重复目标与纯 Shell 排除，不建立第二持久映射。 |
| 原生历史适配与可见扫描 | 本地文件读取、NavigationTree | 接入 Claude／Codex 有限元数据、来源失败保留和可见性调度；核销 `navigation_history_lexical_deepest_match`。 |
| 历史恢复与分支 | AgentCatalog、终端恢复注入 | 完成归属、未归属项目确认、cwd 回退、Shell 就绪和去重重试；核销 `navigation_unowned_restore_requires_project_confirmation`、`navigation_restore_fallback_and_injection_retry`。 |
| 历史隔离副本与来源删除 | ManagedOperationJournal、原子文件 | 建立批次与逐项清单，完成副本校验、指纹复核和部分失败；核销 `quarantine_batches_have_independent_deadlines`，覆盖副本与来源删除边界崩溃。 |
| 历史隔离启动恢复器 | 隔离批次、既有 MaintenanceRunner | 在真实维护顺序注册恢复器，对账未完成批次且先于历史展示；核销 `quarantine_startup_reconciles_incomplete_batch`、`quarantine_restart_preserves_undo_window`、`navigation_transient_and_quarantine_restore`。 |
| 全批撤销预检与恢复 | 隔离启动恢复器 | 重新检查绝对期限、状态、副本和目标指纹，任一失败零恢复；核销 `quarantine_undo_rechecks_deadline`、`quarantine_undo_rejects_external_target`、`integration_history_delete_undo`。 |
| 隔离到期清理与 ZIP 导出 | 隔离批次状态机 | 到期只处理已完成批次，失败可重试或原子导出；导出不清除材料，覆盖清理提交边界崩溃。 |
| 隔离永久删除确认 | 隔离清理与导出 | 独立实现显式永久删除及失败保留，不把导出当删除，不扩大诊断或备份边界。 |
| Agent 文件树复用 | AgentRoot、FileTreeModel、FileOperationService | 完成根过滤、跨根操作、打开复用、ViewState 与监听回收；核销 `agent_files_default_capabilities`、`agent_files_write_does_not_imply_terminal`、`agent_files_cross_root_copy_and_cut`、`agent_files_open_reuses_document`、`agent_root_delete_preserves_document_and_pty`、`agent_root_capability_union_and_revoke`、`agent_files_filter_is_not_authorization`、`agent_files_view_state_restore`、`agent_files_visibility_controls_watchers`。 |
| Hooks 目录与 JSON 子树草稿 | 既有 `slterm_hooks`、Settings 视图宿主 | 收编能力目录、适配声明、重复键／未知字段保留和三层草稿；不把普通输入草稿写入全局设置队列。 |
| Hooks 外部修改与统一关闭守卫 | Hooks 草稿、HookService、CloseCoordinator | 完成写前重读、外部修改、项目移除和根重定位守卫；核销 `hooks_external_modification_guard`、`navigation_project_remove_uses_workspace_guard`、`navigation_relocation_guards_and_reassigns_history`、`repository_binding_survives_project_relocation`、`diff_does_not_rebind_after_relocation`、`integration_project_relocation`。 |
| Hooks 子树原子保存 | PathPolicy 受限能力、原子文件 | 完成锁、备份、子树替换和代次前置，失败保留草稿且不覆盖外部修改。 |
| Hooks 注入与卸载恢复 | HookService、ManagedOperationJournal | 完成危险命令确认、注入／卸载恢复器和提交后清理可见；核销 `hooks_post_commit_cleanup_failure_is_visible`，覆盖真实提交边界崩溃。 |
| hook 小进程事件桥 | Agent 活跃关联、Windows 命名管道端口 | 收编零依赖桥、白名单事件和显式生命周期；与 Runtime 传输保持隔离并验证无孤儿进程。 |

**退出证据**：能力目录与命令构造测试、历史来源故障、逐批崩溃对账、全批撤销预检、Hooks 外部修改和注入提交边界通过；不形成第二长期正文库。

### 设置、主题与恢复

**入口**：所有可移植设置与工作区状态已稳定。

**交付**：完整设置中心、自定义主题 JSON、凭据门面、普通导入导出、BackupPayload、加密导出、DPAPI 恢复点、维护期恢复与跨机重定位。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| 完整设置注册与单写队列 | 既有 SettingsRegistry、设置原子写 | 补齐字段元数据、运行值、自动／显式保存和失败生效语义；核销 `settings_single_queue_across_views`、`settings_views_share_global_queue`、`settings_damage_classification`。 |
| 设置中心与页内状态 | AppSettingsOverlay、Hooks 草稿宿主 | 生成完整页面，完成项目／浮层单例、独立 ViewState、失效页回退和瞬态清除；核销 `settings_page_view_state_restore`、`settings_overlay_keeps_draft_and_queue`、`app_settings_overlay_keeps_hooks_draft`、`integration_settings_singleton`。 |
| 普通设置导入导出与恢复默认 | 设置队列、完整字段元数据 | 独立于加密备份实现白名单操作，未知版本／命名空间和敏感字段严格分流；核销 `settings_import_rejects_unknown_or_credentials`、`settings_import_and_reset_preserve_secrets`。 |
| 自定义主题完整 JSON | 既有 ThemeTokens、设置存储 | 收编完整 token／ANSI、UUIDv7、冲突、revision 改名和使用中删除；核销 `theme_import_complete_or_zero_write`、`integration_custom_theme`，零写入失败且不重载 Document／PTY。 |
| CredentialFacade 统一隔离 | Windows 凭据管理器端口 | 收编套餐、Runtime 与 handover 专用目标、秘密清零和设置页不回显；以 `settings_plan_token_not_cli_credentials` 为后续集成锚。 |
| BackupPayload 逻辑类别投影 | 设置、主题与工作区 DTO 稳定 | 完成类别选择、永久排除、逻辑工作区导出和跨机几何标记；核销 `backup_workspace_category_exclusions`，不复制物理库或任意文件。 |
| 加密导出与整包鉴权 | BackupPayload、密码学依赖 | 以目标 `SLTMBK01` 格式替换旧实现，完成 AAD、大小边界、原子导出和篡改零写入；核销 `backup_tamper_rejected_zero_write` 的导出／鉴权部分。 |
| DPAPI 恢复点与关闭预检 | 加密备份、CredentialFacade、CloseCoordinator | 完成受控暂存、恢复预览、恢复点和关闭取消零材料；覆盖恢复点提交边界崩溃，核销 `backup_restore_does_not_restore_credentials`。 |
| 维护期应用与回滚 | 恢复点、既有 MaintenanceRunner | 注册维护请求恢复器，按类别应用、失败全回滚并更换 store_epoch；核销 `backup_interrupted_restore_rolls_back`、`integration_encrypted_backup`，旧代次写与事件全部失效。 |
| 撤回冲突与跨机重定位 | 维护期恢复、PathAuthority | 完成指纹冲突保留、撤回恢复器和 `RelocationRequired`；核销 `backup_undo_fingerprint_conflict`、`backup_cross_machine_requires_relocation`，未授权前不读正文或启动 PTY。 |

**退出证据**：设置队列、主题零写入失败、备份篡改、类别排除、恢复中断、指纹撤回和 store_epoch 更换通过；任何产物均不含真实凭据或永久排除数据。

### Runtime v2

**入口**：领域命令、Operation、LocalControlServer 和维护期恢复已验收。

**交付**：Serde 协议、生成 schema、全部资源方法、凭据作用域、发现 CLI、持久幂等、分页读取、事件重放和 Runtime conformance。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| 方法注册与完整 schema 骨架 | 既有 `slterm_runtime_protocol`、hello／handover DTO | 扩展方法注册表、封闭错误和生成器，使路由、capabilities、describe 与 schema 同源；只登记真实可用能力，不添加空方法。 |
| Runtime 作用域与传输约束 | 既有 LocalControlServer、CredentialFacade | 扩展首包认证、双作用域、连接／请求限制和关闭约束；核销 `runtime_v1_rejected`，不另开服务器。 |
| 发现 CLI 与凭据轮换 | 双作用域认证、既有发现记录 | 完成非敏感端点、唯一 token 输出和 Runtime／handover 隔离轮换；核销 `runtime_token_absent_from_materials`、`integration_runtime_credential_boundaries`。 |
| describe 与不可变快照 | 方法注册表、资源快照端口 | 返回真实能力、协议散列和同一代次资源快照；同步 DTO、生成资产和 GUI／CLI conformance。 |
| Operation 持久派发 | 既有 Operation／Journal、store_epoch | 完成参数散列、expected_revision、断线重查和 OutcomeUnknown；核销 `runtime_outcome_queryable`，不可证明副作用的操作不得自动重发。 |
| RuntimeHub 与事件续传 | 资源代次、store／stream epoch | 完成有界事件环、客户端队列、续传三元组和缺口 resync；核销 `runtime_events_visibility_and_generation`。 |
| 有界读取与分页游标 | RuntimeHub、不可变资源快照 | 统一终端、Git diff 与 Agent 读取的页面预算和代次失效，不把截断伪装为完整结果。 |
| Project／PanelGroup／Panel 方法族 | CommandRegistry、Operation、Document 真实打开 | 逐方法接入布局、面板与路径语义，同步 DTO、schema 和 conformance；核销 `runtime_cwd_does_not_fallback`。 |
| 非交互关闭方法族 | CloseCoordinator、Operation | 为 app、project 与 panel 关闭提供显式处置策略和 `decision_required`；核销 `runtime_close_requires_decision`、`integration_runtime_noninteractive_close`。 |
| Terminal 与 Agent 方法族 | 终端、Agent、分页读取 | 接入可信 execute、原始 input、代次目标和恢复／分支；同步 schema 与 conformance，无法证明的输入副作用进入 OutcomeUnknown。 |
| Git 方法族与破坏前置 | GitFacade、FileOperationService、Operation | 接入状态、Diff、固定版本与受限破坏操作；核销 `runtime_destructive_precondition`、`integration_commit_rollback` 的 Runtime 部分。 |
| handover 同构与完整 conformance | 各资源方法族、真实 OpenFile 链 | 复核单实例与 Runtime 共用授权、命令和关闭，不重新实现 OpenFile；核销 `integration_single_instance_runtime_parity`，完成真实 GUI／CLI 全方法与事件断线矩阵。 |

**退出证据**：真实 GUI／CLI 的握手、断线重查、重复 mutation、事件缺口、非交互关闭、关闭中拒绝和 schema 漂移测试通过；v1 与 token 环境变量无执行路径。

### Windows 与发布

**入口**：统一关闭、凭据和 Runtime 已可用。

**交付**：套餐余量、托盘、登录启动、材质、窗口几何、通知、任务栏、更新、诊断、清理和发布链。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| WindowsIntegration 统一接入 | 既有托盘、启动与统一关闭 | 收编平台端口和假实现，对账登录启动实际状态与托盘可用性；复用既有关闭链，不新增托盘退出分支。 |
| 按机器窗口几何与能力降级 | WindowsIntegration、几何持久化 | 完成离屏夹回、DPI／显示器变化和材质能力说明；核销 `windows_geometry_clamped_to_work_area`，真实桌面行为按豁免验收。 |
| 固定来源套餐客户端 | CredentialFacade、有界 HTTP 端口 | 收编 DeepSeek／Kimi 请求与解析，覆盖固定路径、无重定向、超时、响应预算和 token 隔离；核销 `plan_balance_uses_dedicated_credential`、`settings_plan_token_not_cli_credentials`、`integration_plan_credentials`。 |
| PlanBalanceService 调度 | 固定来源客户端、TaskService、假 Clock | 完成防重入、冷却、隐藏调度、过期和凭据代次失效；核销 `plan_balance_runs_while_navigation_hidden`、`plan_balance_stale_after_full_period`、`settings_plan_schedule_commit_and_visibility`、`integration_plan_schedule`。 |
| NotificationCenter 与系统通知 | Agent／Hooks 状态事件、WindowsIntegration | 收编类别去重、桌面开关和应用内反馈；核销 `notification_and_taskbar_switches_independent`。 |
| 任务栏闪烁与不确定进度 | NotificationCenter | 完成失焦闪烁、错误优先级和进度清理；核销 `taskbar_priority_and_indeterminate_progress`、`integration_notification_taskbar`。 |
| 运行期语言切换 | 既有 typed i18n | 收编目录替换与全局重绘，不重建工作区；核销 `i18n_switch_preserves_workspace`、`integration_language`。 |
| 更新检查与资产下载 | 有界 HTTP、发布元数据 | 收编精确资产匹配、`.part`、长度／PE／哈希和安装形态限制；核销 `update_asset_validation_rejects_mismatch`。 |
| 更新关闭与 UpdateHandoff | 更新下载、CloseCoordinator | 扩展统一关闭进入 AwaitingClose／Handoff，最终提交失败取消；核销 `update_install_uses_close_guard`、`integration_update` 的应用内部分，覆盖 handoff 提交边界崩溃。 |
| 退出后安装与启动对账 | UpdateHandoff、真实安装器 | 完成旁置锁、PID＋创建时间、旧版回退和独立日志；新版本启动后才标记成功，覆盖退出后中断与重开对账。 |
| DiagnosticService 白名单 | 结构化健康快照、原子导出 | 收编复制／导出同一 DTO、脱敏预览和异步自检；核销 `sensitive_material_excluded_from_outputs` 的诊断部分。 |
| CleanupService 所有者清理 | 各恢复材料所有者、诊断 DTO | 完成计划／执行、活跃排除代次、逐项结果和审计独立确认；核销 `cleanup_excludes_active_recovery_material`、`integration_diagnostics_cleanup`。 |
| 发布包与实机核验 | 全部发布功能、Runtime v2 资产 | 收编安装器／便携包、许可、体积基线和发布清单；核验 schema、版本、架构、更新回退与无孤儿进程。 |

**退出证据**：固定来源查询与凭据隔离、更新校验和 handoff 对账、诊断白名单、清理活跃排除、安装器与便携包验收通过；更新安装不得绕过关闭守卫。

### 全量验收

**入口**：全部功能阶段已验收。

**交付**：追溯矩阵核销、全量串行测试、真实 Windows 场景、人工豁免证据、发布清单和最终进度状态。

#### 执行拆分

| 行为切片 | 前置 | 交付与验收锚 |
|---|---|---|
| 所有者与跨域追溯核销 | 各功能实现及测试已存在 | 逐场景核对真实例名、执行输出或豁免，修正设计漂移；核销 `acceptance_user_contract_observable`、`acceptance_exemptions_registered`，延期项不冒充完成。 |
| 单一权威与完整崩溃集成 | 各领域恢复器与真实提交测试 | 复核无第二缓冲、授权、执行或关闭通道，并补齐跨域崩溃序列；核销 `integration_single_authority_channels`、`integration_crash_recovery`。 |
| 全量门禁与资源回收 | 追溯差异已处理 | 串行执行全部自动化门禁、故障注入和重复创建／销毁检查，登记实际失败、跳过与资源证据。 |
| 真实 Windows 场景矩阵 | 真实可执行文件和发布包 | 验证双实例、ConPTY、备份维护中断、Runtime 双进程、更新移交与安装包；核销 `integration_platform_boundary_failure`，人工项按既有豁免留证。 |
| 最终排除与交付状态 | 所有未决场景已核销或登记延期／豁免 | 核销 `acceptance_excluded_surface_absent`、`sensitive_material_excluded_from_outputs`、`performance_claims_have_conditions`，复核敏感材料、许可和发布内容后更新最终进度与验收 hash。 |

**退出证据**：每个 featureSpec 场景都有实际测试或登记豁免；最终排除项无隐藏入口；发布包内 Runtime、许可和数据边界符合合同。

## 验收与测试

- 集成检查点至少覆盖：工作区关闭崩溃序列、跨视图 Document 编辑、真实 ConPTY 洪峰与 resize、历史隔离崩溃对账、备份维护期中断、Runtime 双进程断线和更新移交。
- 每个检查点同时观察内存状态与真实磁盘／进程副作用，不以 DTO 状态推断操作已经成功。
- 阶段内新增不可自动化面时，先登记 `.claude/docs/testing/exemptions.md` 的根因和兜底层级，再允许对应提交进入验收。
- 进度状态与追溯矩阵必须一致；已验收阶段不得引用不存在的测试例或未执行的实机证据。

## 操作

- 全量测试、定向测试和静态门禁使用 [.claude 命令矩阵](../../.claude/docs/operations/commands.md)，所有 cargo 与脚本钉测试串行执行。
- 每个提交前运行与改动面相符的定向门禁；阶段验收和最终交付运行全部门禁，不能用前一次阶段结果替代新证据。
- 阶段状态变化后更新 [进度记录](progress.md)，只登记已验证事实和当前阻塞，不补写流水账。
