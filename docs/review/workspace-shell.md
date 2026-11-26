# 工作区、侧栏、拆分与 PathPolicy 审查

> 审查范围：`slterm_workspace`、`slterm_sidebar`、`slterm_split`、`slterm_path` 及其直接消费的侧栏宿主、项目持久化与路径授权接线；基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`，只读静态审查，未运行 cargo、测试或网络。

## 覆盖与局限

- 已覆盖：四个 crate 的全部源码与测试；工作区壳、侧栏框架、PathPolicy、领域模型、工作区设计文档；追溯矩阵中工作区、侧栏、PathPolicy、导航树相关行；`slterm_app` 中 `SideViewHost`、侧栏宽度压缩、侧栏 ViewState 持久化、项目打开与重定位接线、文档保存与授权代次消费点、路径解析端口。
- 已覆盖的侧栏宿主测试：同槽替换、整栏隐藏、隐藏按钮、跨槽交换、关闭重开与重启的实例和瞬态语义；侧栏 DTO 严格/容错恢复；ViewKey/ViewState 规范编解码属性；ProjectLayout 随机操作属性。
- 未覆盖：导航树内容本身（展开、历史节点、搜索、套餐行）按 `docs/featureSpec/workspace/navigation.md` 归属单独所有者，未按该篇逐条核验；更新安装 handoff 未做全链运行时审查；真实 GPU/桌面焦点与 PTY 进程行为未实机复验；GPUI 全特性测试未执行。
- 局限说明：报告中的“确认”仅表示已读完对应静态代码路径；涉及 OS 重解析点、进程生命周期和持久库恢复结果的结论仍需运行时证据的部分列入“存疑未证”。

## 问题

### 重要｜工作区布局恢复没有未知 ViewKey 过滤与重复视图归一

- 置信度：确认
- 位置：`slterm_workspace/src/project_layout.rs` 的 `ProjectLayout::from_canonical_json`、`ProjectLayoutDto::into_domain`、`PanelGroup::new`；`slterm_workspace/src/project_snapshot.rs` 的 `ProjectSnapshot::recover_with_documents`；`slterm_app/src/persistence/project.rs` 的 `decode_project_snapshot_from`；`slterm_app/src/persistence/startup/recovery.rs` 的 `StartupRecovery::recover`
- 现象：持久布局里出现未登记的 `ViewKey.kind` 时，`ViewKey` 解码返回 `UnknownKind`，被包装为 `LayoutDtoError::ViewKey`，`decode_project_snapshot_from` 直接上抛，`StartupRecovery::recover` 以 `StartupRecoveryError::Project` 失败并进入启动维护失败分支。同一布局内重复的 Settings、Editor、Diff 等完整 ViewKey 由 `PanelGroup::new`/`ProjectLayout::validate` 判为 `DuplicateView`，同样整库拒绝。启动恢复入口 `ProjectSnapshot::recover` 只摘除缺少记录或错项目的 Terminal/Editor/Markdown/Diff 视图，并不识别未知类型，也不做重复视图的确定性归一。
- 根因：工作区侧只有严格写读 DTO，没有与侧栏恢复对应的“容错布局恢复转换”；未知类型与领域冲突被统一压进严格解码错误，应用层也没有按 `UnknownKind` 分类过滤。
- 影响：一个未登记的面板类型或一条重复视图记录会让全部项目的布局恢复失败，工作区进入诊断/重试分支，而不是只过滤坏条目并聚合提示。违背 `docs/featureSpec/workspace/shell.md` “面板类型…未知类型恢复时过滤并聚合警告，不创建替代面板”与 `docs/devPlan/design/workspace.md` “恢复时…未知 ViewKey 过滤并聚合提示”“重复 Settings 或非法引用按确定性归一”。该行为在升级/降级、手工修复或持久库残留场景可实际触发。
- 证据：`ProjectLayout::from_canonical_json` 对任意未知 `kind` 返回 `LayoutDtoError::ViewKey(ViewKeyError::UnknownKind)`；`decode_project_snapshot_from` 无按错误分类的回退；`ProjectSnapshot::recover_with_documents` 的过滤条件只检查 Terminal 记录存在且项目匹配、Document 记录存在；测试 `project_layout_dto_distinguishes_envelope_failures`、`project_layout_dto_rejects_duplicate_fields_and_nested_view_failures` 覆盖的是拒绝而非过滤，`workspace_startup_layout_before_snapshots` 与 `recovery_normalizes_missing_terminal_views_and_orphan_records` 不注入未知 ViewKey 或重复 Settings。
- 优化方向：为持久布局增加独立于严格写 DTO 的容错恢复转换，按封闭注册表过滤未知类型、按确定顺序归一重复视图，并产出可聚合、可本地化的警告；严格 DTO 保留为写入与重放闸门。

### 重要｜显式终端 cwd 只做词法包含校验，未解析真实目标

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commands.rs` 的 `AppRoot::new_terminal_from_request`、`new_terminal_project`、`local_launch`、`launch_metadata`；`slterm_workspace/src/project_snapshot.rs` 的 `ProjectSnapshot::prepare_record`；对照 `slterm_app/src/app_shell/app_root/explorer.rs` 的 `verify_terminal_cwd`
- 现象：通过 `NewTerminal` 命令、第二实例 handover 或 Runtime 传入的显式 cwd，只比较 `cwd.as_path().starts_with(root.as_path())`，随后把该路径直接写入 `TerminalRecord.cwd` 并交给 `LocalTerminalLauncher` 作为 ConPTY 工作目录。路径不存在的唯一后果是启动失败；路径存在但不是目录也没有在启动前显式判定。更关键的是，项目根内任一目录联接或符号链接组件不会被解析为真实目标，词法上位于根内、真实目标在根外的 cwd 会被接受。
- 根因：终端 cwd 的授权链没有接到真实路径解析端口和 `PathPolicy::TerminalCwd`；只有 Explorer 的“在此打开终端”调用 `verify_terminal_cwd`，显式 NewTerminal 链只依赖 `CanonicalPath` 的词法绝对性与 `starts_with`。
- 影响：违反 `docs/featureSpec/path-policy.md` “规范路径解析符号链接、目录联接等重定向，用于包含性与授权判断”“终端 cwd 必须由允许终端的有效根授权”“敏感操作前重新解析真实目标”；项目根内的重解析点可把 PTY 工作目录带到授权根外，且 `runtime`/`handover` 与面板内操作采用不同判定。现有测试 `integration_new_terminal_cwd`、`handover_rejects_unauthorized_cwd` 只覆盖词法越界，不覆盖别名/联接分量。
- 证据：`prepare_record` 只用 `cwd.as_path().starts_with(root.as_path())`；`launch_metadata` 把 `record.cwd().as_path().to_path_buf()` 原样交给 `TerminalLaunch::Local`；`session::local_options` 把该路径赋给 `tty::Options.working_directory`，全程没有再解析；`verify_terminal_cwd` 只在 Explorer 意图链中使用。
- 优化方向：显式 cwd 在进入快照候选前经后台真实路径解析与目录检查，再以解析后的规范路径按 `PathPolicy` 的终端操作判定；保持无 I/O 的 core 纯状态机，真实解析与拒绝留在 application 端口。

### 重要｜项目根重定位链与 ProjectRootStatus 在生产不可达

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/projects.rs` 的 `AppRoot::relocate_project`；`slterm_workspace/src/project_registry.rs` 的 `ProjectRegistry::relocate_project`；`slterm_workspace/src/project_registry.rs` 的 `ProjectRootStatus`；`slterm_app/src/persistence/project.rs` 的状态编解码；`slterm_app/src/app_shell/app_root/tests/relocation.rs` 的 `relocate` 辅助函数；`slterm_app/src/app_shell/app_root/tests/terminal_restore.rs` 的 `backup_cross_machine_requires_relocation`
- 现象：`AppRoot::relocate_project` 标注 `#[allow(dead_code)]`，除测试外没有任何命令目录、菜单、导航入口或 handover 分支调用它。测试辅助函数直接给 `relocate_project` 传入已解析的 `CanonicalPath`，没有经过 `ProjectPathResolver`，也没有真实路径查重、目录校验或失败重试。`ProjectRegistry::relocate_project` 只做词法 `CanonicalPath` 相等比较。`ProjectRootStatus` 的 `Inaccessible`/`RelocationRequired` 在生产中没有任何写入点；`set_root_status` 与 `RelocationRequired` 的赋值只出现在测试和持久库测试夹具中。
- 根因：重定位的实现停留在候选快照层的内部函数，缺少从用户/移交入口到后台真实解析、冲突检查、Hooks 守卫和原子提交的完整接线；根状态枚举有读取方和持久字段，但没有生命周期生产者。
- 影响：`docs/featureSpec/workspace/shell.md` 的“根重定位只按 PathPolicy 改关联”与 `docs/featureSpec/path-policy.md` 的“新增项目或重定位根时同时检查词法唯一性与真实目标查重；无法完成真实路径查重时拒绝操作”在重定位一侧没有生产路径。跨机恢复无法把项目置为 `RelocationRequired`，`commands.rs`、`explorer.rs`、`git_show.rs` 对该状态的过滤实际永不生效。追溯矩阵把 `integration_project_relocation` 标为已自动化，但该用例绕过真实解析端口，不能证明生产入口。
- 证据：全仓 `relocate_project` 的生产调用点为空；`set_root_status` 仅在 `project_registry_tests`、`terminal_restore` 与 `persistence/project` 测试中出现；`backup_cross_machine_requires_relocation` 在测试里手工调用 `set_root_status(..., RelocationRequired)` 再验证恢复行为，没有经过备份恢复或重定位生产链。
- 优化方向：把重定位接到与项目打开同一 FIFO 的后台真实解析与冲突链，状态枚举由同一命令在成功/失败/跨机恢复时推进，并补重定位真实路径查重失败与别名冲突的端到端证据。

### 一般｜授权代次在生产恒为零，撤销代次未接线

- 置信度：疑似（代码事实已确认；用户可见后果仍需运行时证据）
- 位置：`slterm_app/src/app_shell/app_root/documents.rs` 的 `AppRoot::save_document`；`slterm_app/src/app_shell/documents.rs` 的 `DocumentOpenService::load`、`DocumentSaveService::save`；`slterm_app/src/app_shell/app_root/document_external.rs` 的 `external_location_authorized`；`slterm_app/src/app_shell/app_root/explorer.rs` 的 `verify_terminal_cwd`；`slterm_path/src/policy.rs` 的 `PathPolicy::authorize`；`slterm_workspace/src/project_registry.rs` 的根撤销路径
- 现象：生产代码中的 `PathRequest.expected_authorization_generation`、`DocumentSaveService::save` 的当前代次、`DocumentRegistry::begin_save` 的授权代次、`DocumentExternalRequest` 的授权代次、`DocumentWriteContext::new` 的授权代次全部传入字面量 `0`。应用没有按项目根或 Agent 根集合维护授权代次，也没有在删除/重定位根时递增。于是 `PathDenial::GenerationMismatch` 在正常产品流程中不可达，文档保存请求与外部刷新请求捕获的授权代次永远与当前代次相等。
- 根因：PathPolicy 与 Document 会话实现了授权代次校验，但 application 未提供代次来源；撤销能力依赖每次操作前重新计算 grants，而不是代次失效。
- 影响：`docs/featureSpec/path-policy.md` “删除根立即撤销其授权，并取消该根后续自动读取、刷新与写入”和领域模型“删除根立即撤销其能力并增加授权代次”缺少运行的代次机制。新操作多数可通过重新计算 grants 被拒，但已捕获 grants 的后台保存/外部读刷新不会因代次失配而失效；具体是否已越过撤销边界需要运行时用例证明。
- 证据：`save_document` 调用 `begin_save(document_id, 0)` 与 `save(request, &grants, 0)`；`external_location_authorized` 调用 `PathPolicy::authorize(..., 0)`；`DocumentWriteContext::new(..., 0, 0, ...)`；全仓没有在根注册/撤销/重定位时推进的授权代次字段或函数；`save_rejects_changed_authorization_generation_without_writing` 只构造代次不一致的单元场景。
- 优化方向：由 application 的唯一根授权快照持有代次，在根加入、移除、改路径和重定位提交时递增，并让保存、外部刷新、文件操作和终端 cwd 请求统一携带捕获代次与当前代次；补“捕获后撤销根”的运行时拒绝证据。

### 一般｜侧栏恢复的聚合警告只写日志，没有用户可见提示

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/side_view.rs` 的 `AppRootPersistence::load`；`slterm_app/src/persistence/sidebar.rs` 的 `WorkspaceDatabase::sidebar_state`；`slterm_sidebar/src/sidebar_state/recovery.rs` 的 `SidebarRecovery`、`SidebarRecoveryWarning`
- 现象：启动时侧栏组织状态经 `SidebarRecovery::from_json` 恢复，未知视图、重复按钮/打开记录、同槽冲突、非法宽度/比例/显隐字段被字段级修复并汇总为 `warnings`；应用只调用 `log::warn!` 记录警告数量，不产生启动通知、通知条或本地化提示。
- 根因：恢复警告没有进入壳事件或 i18n 通道，`SidebarRecovery.warnings` 只被日志消费。
- 影响：`docs/featureSpec/sidebar/framework.md` 的“未知视图…聚合提示”“缺失的已注册视图…合并不警告，非法宽度、比例或按钮位置只回退对应字段并聚合提示”以及验收“布局包含未知 SideViewId、重复记录和非法宽度…聚合提示”对用户不可见；用户只看到侧栏布局变化，不知道记录被改写。
- 证据：`AppRootPersistence::load` 对 `recovery.warnings` 只做 `log::warn!`；全仓生产代码没有 `SidebarRecoveryWarning` 的 i18n 消息或通知投递；`integration_sidebar_visibility_semantics` 不涉及恢复警告。
- 优化方向：把侧栏恢复警告作为启动恢复通知的封闭类别聚合投递，文案只含修复类别和数量，不复述持久内容；保持无警告时不提示。

### 一般｜启动恢复专用 ProjectSnapshot::recover 的冲突分流缺自动化证据

- 置信度：确认
- 位置：`slterm_workspace/src/project_snapshot.rs` 的 `ProjectSnapshot::recover_with_documents`；`slterm_workspace/src/project_snapshot_tests.rs` 的 `recovery_normalizes_missing_terminal_views_and_orphan_records`、`document_recovery_detaches_missing_views_and_drops_orphan_records`；`slterm_app/src/persistence/startup/recovery.rs`
- 现象：生产启动只经 `recover_with_documents` 归一失效引用，但核心测试仅覆盖“缺失 Terminal 记录”“孤儿 Terminal 记录”“缺失 Document 视图”“孤儿 Document 记录”。没有覆盖：Terminal 视图对应记录存在但属于另一项目；保留记录之间终端编号重复；保留记录编号达到或超过项目计数器；Document 记录重复；以及这些结构损坏在归一后仍必须拒绝。
- 根因：测试集中在正常归一分支，未覆盖 `recover` 与 `validate` 的交互；`restore` 的冲突测试不能替代 `recover`，因为 `recover` 会先摘除部分视图/记录再校验。
- 影响：启动恢复是唯一生产恢复入口；上述冲突若回归，可能被归一顺序掩盖或导致整库启动失败，而现有测试不会失败。与 `docs/featureSpec/workspace/shell.md` “身份重复、编号冲突及其他结构损坏仍拒绝”的承诺缺少对应证据。
- 证据：`recover_with_documents` 先摘除 detached 视图、再 `retain` 引用终端、再 `retain_referenced_documents`、最后 `validate`；现有恢复测试没有构造错项目 Terminal 记录、重复编号或超计数器记录；`project_snapshot_rejects_duplicate_terminal_records_and_numbers` 等用例走 `restore`，不经过摘除路径。
- 优化方向：为 `recover_with_documents` 增加上述冲突矩阵，并断言错误分类稳定、原持久输入不被静默改写。

### 一般｜追溯矩阵的 PathPolicy 文件身份用例名不存在

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` PathPolicy 节“硬链接或不同词法路径按文件身份复用 Document”行；目标例名 `path_file_identity_deduplicates_document`；实际覆盖 `slterm_app/src/app_shell/documents.rs` 的 `integration_links_and_file_identity`
- 现象：矩阵声明的 `path_file_identity_deduplicates_document` 在全仓不存在。实际验证硬链接复用的例名为 `integration_links_and_file_identity`，且只使用同一目录下的硬链接，不覆盖“不同词法路径”或目录联接别名。
- 根因：用例改名或矩阵未同步；测试覆盖范围也比行描述窄。
- 影响：追溯矩阵的自动化证据不可定位，验收状态与真实例名漂移；验收场景中的“不同词法路径”没有直接证据。
- 证据：全仓检索不到该例名；`integration_links_and_file_identity` 的内容是 `std::fs::hard_link` 后两次打开，断言同一 `DocumentId` 与同一正文，没有别名路径/联接分量。
- 优化方向：统一矩阵例名与真实例名，并补充不同词法路径或目录联接别名命中同一 Document 的例，或在矩阵中准确缩小声明范围。

### 一般｜新增项目经真实目录别名去重没有端到端证据

- 置信度：确认
- 位置：`slterm_app/src/platform/project_paths.rs` 的 `system_resolver_normalizes_directory_junction`；`slterm_app/src/app_shell/app_root/tests/handover.rs` 的 `handover_open_project_duplicate_root_reactivates_same_project`、`project_realpath_resolution_preserves_request_order`、`project_realpath_dedupe_failure_preserves_relation`；`slterm_workspace/src/project_registry.rs` 的 `ProjectRegistry::add_project`
- 现象：解析器层有目录联接归一测试，Agent 根有 `agent_root_real_directory_dedup` 的真实 junction 去重测试，但项目层没有“经 junction 别名打开同一真实目录，复用既有 ProjectId 且不新增项目/终端”的端到端用例。`handover_open_project_duplicate_root_reactivates_same_project` 用的是完全相同的词法根；失败用例使用假的 `FailingProjectPaths`。
- 根因：真实路径去重的成功路径被拆在解析器单测与词法重复单测两端，未在项目打开链上闭合。
- 影响：`docs/featureSpec/path-policy.md` “新增项目或重定位根时同时检查词法唯一性与真实目标查重”在新增项目一侧缺少自动化证据；若 `add_project` 或未来打开链对规范化结果的比较发生回归，现有测试不足以发现别名导致的重复项目。
- 证据：`ProjectRegistry::add_project` 只比较 `existing.root == root`；生产链依赖 `ProjectPathResolver` 先规范化；现有项目级测试没有调用 `mklink /J` 创建别名后打开项目。
- 优化方向：在真实临时目录上建立 junction 别名，走 OpenProject/handover 生产链，断言返回既有项目、项目总数、活跃项、布局与 PTY 均不变化；失败路径继续由假解析器覆盖。

### 提示｜PathRequest.root_hints 是无生产消费的死字段

- 置信度：确认
- 位置：`slterm_path/src/policy.rs` 的 `PathRequest.root_hints`；全部生产 `PathRequest` 构造点
- 现象：`root_hints` 的文档说明为“词法路径与根提示由 application PathResolver 消费”，但生产代码全部构造为 `Vec::new()`，没有任何读取方；`PathPolicy::authorize` 也不使用该字段。
- 根因：请求模型承载了未实现的根提示能力，解析端口只接收词法路径。
- 影响：API 面暗示存在多提示解析能力，实际不存在；后续调用方可能误以为填充该字段会改变解析或授权，形成错误预期。
- 证据：全仓 `root_hints` 只有构造点为空向量，`ProjectPathResolver`/`DocumentPathResolver::resolve` 签名不接收该字段。
- 优化方向：要么删除该字段并保持请求最小化，要么把根提示纳入统一解析端口的显式输入与校验。

## 跨域观察

### 重要｜仍存在第二套布局序列化，且更新安装 handoff 依赖已无生产写入的 session.json

- 置信度：确认
- 位置：`slterm_app/src/session.rs` 的 `LayoutSession`、`Session`、`save_to`、`save_compact_to`、`save_final`；`slterm_app/src/update_download/handoff.rs` 的 `PreparedUpdate::commit`、`apply_scheduled_at`、`restore_ticket_at`；`slterm_app/src/update_download/handoff.rs` 的 `workspace_is_valid`
- 现象：`session.rs` 仍定义并序列化整套 `LayoutSession`（Pane/Split、比例、cwd、launch、Agent），与 `ProjectLayout` 规范 DTO 并存。`update_download::handoff::PreparedUpdate::commit` 把 `Vec<Session>` 写成 `workspace.json`；`apply_scheduled_at` 从本地根读取 `session.json` 再 `commit`；`restore_ticket_at` 又把 `workspace.json` 读回 `Vec<Session>`。但生产启动链已不再调用 `session::save_*`，全仓没有生产写入 `session.json` 的入口，`restore_ticket` 也没有生产消费点。
- 根因：更新 handoff 仍是旧会话 schema 的所有者，未迁移到 `ProjectSnapshot`；旧恢复通道被删除后，handoff 的输入来源未同步替换。
- 影响：与根 `CLAUDE.md` “布局单点”以及 `slterm_app/CLAUDE.md` “禁止旧 session layout”“无旧恢复旁路”、`update_download/CLAUDE.md` “不读取旧 session”冲突。预约安装路径会因缺少 `session.json` 失败；即使存在旧文件，恢复材料也是第二套布局事实，不能保证与 `ProjectSnapshot` 一致。该检查项“禁止第二套布局序列化；持久化恢复只经快照”不成立。
- 证据：`LayoutSession` 仍被 `Session` 字段使用并参与 `serde`；`PreparedUpdate::commit` 序列化 `Vec<Session>`；`apply_scheduled_at` 读取 `session.json`；`session::save_final`/`save_to` 只有测试调用；`restore_ticket_at` 只有测试调用。
- 优化方向：把更新前后工作区移交统一到 `ProjectSnapshot` 的规范 DTO 和启动恢复端口，删除旧布局 schema 的序列化职责，或明确登记为不可自动化的边界并给出唯一权威映射。

### 一般｜导航树运行态与 NavigationState 未接通

- 置信度：确认
- 位置：`slterm_sidebar/src/side_view_state.rs` 的 `NavigationState`、`NavigationProjectState`；`slterm_app/src/app_shell/app_root/nav.rs` 的 `NavRuntime`、`render_nav`；`slterm_app/src/app_shell/app_root/navigation.rs` 的 `navigation_transient_and_quarantine_restore`
- 现象：`NavigationState` 声明了按 ProjectId 分区的 `expanded`、`history_expanded`、`unassigned_expanded` 和 `scroll`，但字段与分区结构均无公开读写方法，模块外只能经原始 JSON 构造；应用 `NavRuntime` 只保存项目行列表，`render_nav` 不消费或更新展开、历史、滚动状态。现有导航测试只验证默认态字段键集合与历史隔离，不验证实际导航树展开/滚动持久化。
- 根因：侧栏 schema 先行声明了导航树持久字段，导航视图运行态仍是项目列表桩。
- 影响：`docs/featureSpec/sidebar/framework.md` 对四种视图“关闭重开与应用重启使用同一份持久状态”的要求对导航树只成立于默认值；`docs/featureSpec/workspace/navigation.md` 的项目/历史展开、树滚动恢复和同槽替换场景无法成立。此项归属导航树所有者，但影响侧栏框架验收。
- 证据：`NavigationState` 无 `pub` 访问器；`nav.rs` 的 `NavRuntime` 只有 `projects`；`navigation.rs` 的测试只构造 `SideViewState::new(SideViewId::Nav)` 并断言对象键为 `projects`、`scroll`、`unassigned_expanded`。
- 优化方向：由导航树所有者提供与 `ExplorerState` 同级的强类型读写接口，并让 `NavRuntime` 的展开/滚动走同一 ViewState 检查点；不在壳层复制第二份导航状态。

### 提示｜PathPolicy 的 Hooks 与历史集成能力没有生产消费点

- 置信度：疑似（代码事实已确认；是否构成越权取决于 Hooks/历史模块的自有守卫）
- 位置：`slterm_path/src/policy.rs` 的 `PathOperation::HooksRead`/`HooksWrite`/`HistoryRead`/`HistoryQuarantine`/`HistoryRestore`、`PathGrant::hooks_*`、`PathGrant::history_*`；`slterm_app/src/ai_hook*`、`slterm_app/src/history*.rs`
- 现象：内置集成类操作、能力位和授权构造器只在 `slterm_path` 单测中使用，应用侧 Hooks 与历史模块没有 `PathPolicy` 调用。运行期实际消费的路径操作集中在 Read/Write/ResourceRead/TerminalCwd/AutomaticRead/RestoreTerminalCwd。
- 根因：PathPolicy 的“内置 CLI 声明目标”能力模型已定义，但对应功能篇的接线未落到该模块；Hooks/历史可能以固定路径构造与自身守卫执行。
- 影响：如果 Hooks/历史模块以固定路径自行决定读写而不经能力清单，`docs/featureSpec/path-policy.md` “内置 CLI 声明目标必须收敛到固定能力清单和最小路径集合”“不另建越界例外”缺少唯一执行点；能力位成为死 API。需要 Hooks/历史所有者确认其守卫是否等同。
- 证据：全仓生产代码无 `PathGrant::hooks_*`/`history_*` 调用，`PathPolicy` 对 Hooks/历史操作的唯一引用在 `policy_tests`。
- 优化方向：确认 Hooks/历史写入目标是否全部经固定能力清单；若独立守卫已覆盖同等语义，应在文档中登记分工，否则接入 PathPolicy 的集成操作与精确文件授权。

## 存疑未证

- `SideViewHost::drop`/`close_all` 只清空实例表，不调用 `SideViewControl::close`；生产 AppRoot/窗口销毁时侧栏私有任务与最终 ViewState 检查点依赖 `SideView` 的 `Drop` 以及没有其他强实体引用。现有 GPUI 测试通过显式 `shutdown` 验证，未覆盖真实窗口销毁与 writer 关闭顺序。需要运行时证据：在 GPUI 窗口关闭、writer shutdown 后重启，确认每个已打开视图的最终 ViewState 恰好落盘一次且任务取消。
- `ProjectSnapshot` 对 Document 位置/文件身份的唯一性只在 `with_relocated_document` 内校验，`with_opened_document` 与 `validate` 不阻止两个不同 `DocumentId` 落在同一规范位置或同一文件身份；应用通常先经 `DocumentRegistry::existing` 去重，但 core 聚合本身可表示重复位置。需要确认这是有意的域边界，还是应在快照层补一致性约束；若应用绕过 Registry 或恢复材料出现两个文档同址，启动恢复的 `RecoveryTargetOccupied` 行为需实机核对。
- 删除可写 Agent 根或重定位项目根时，已捕获 grants 的在途文档保存/外部刷新是否仍会落盘，当前只有静态代次为零与重新计算 grants 的证据，缺少“捕获后撤销根”的运行时用例；该用例决定授权代次问题是否已产生真实写入缺口。
- 目录联接与文件名大小写归一目前由 `dunce::canonicalize` 与 Windows 真实句柄身份承担；`CanonicalPath` 自身不强大小写一致，`PathGrant::covers` 使用 `Path::starts_with` 做字节级组件比较。静态上同一解析端口产出的路径应一致，但混合来源（手工构造的 `CanonicalPath`、持久库旧值、不同大小写入口）未做端到端验证，需要真实存储与别名场景确认不会产生错误拒绝或错误放行。
