# 架构与跨模块合同审查

> 基线 HEAD 53c2a1504a287917f78e58f580a7bd4b3e752060（当前分支），只审当前工作树。

## 覆盖与局限

- 已读文档：根 `CLAUDE.md`、各 crate 与关键子目录 `CLAUDE.md`、`.claude/docs/architecture/overview.md`、`provenance.md`、`.claude/docs/testing/exemptions.md`、`docs/devPlan/plan.md`、`progress.md`、`traceability.md` 的相关段落、`docs/devPlan/design/architecture.md`、`domain-model.md`、`dependencies.md`、`migration.md`、`testing.md`、`agents.md`、`docs/featureSpec/cross-cutting.md`、`path-policy.md`、`README.md`、`acceptance.md`、`backup-and-recovery.md`、`workspace/navigation.md`。
- 已核对配置与门禁：根 `Cargo.toml`、全部成员 `Cargo.toml`、`rust-toolchain.toml`、`architecture/dependencies.toml`、`architecture/file-budgets.txt`、`scripts/check_architecture.py`、`scripts/architecture/*.py`、`scripts/tests/test_gpui_pins.py`、`scripts/tests/test_architecture_governance.py`、`Cargo.lock` 抽样。
- 已读代码路径：`atomic_file`、`audit`、`settings_store`、`settings_state`、`slterm_app/src/app_shell/documents.rs`、`app_root/documents.rs`、关闭三链、`gpui_terminal/registry.rs`、`session.rs`、`event_mailbox.rs`、`slterm_terminal` 的 `event_loop` 与 `tty/windows/blocking.rs`、`platform/dirs.rs`、`platform/project_paths.rs`、`persistence/project.rs`、`encrypted_backup`、`update_check/update_download/handoff`、`plan_balance`、`ai_hook` 宿主与安装器、`agent_host`、`notify`、`nav`、`side_view`、`slterm_workspace` 的 `view_key`/`project_layout`、`slterm_completions/src/file.rs`、`slterm_settings` 的 `try_load_at`/`reset`/`diagnostics`、`tray`、`fs_watch`、`side_panel/search/worker.rs`。
- 局限：未执行 cargo、构建、测试、GUI 或网络；未逐行读完所有源码，结论来自完整读取目标路径与全局符号检索。`check_architecture.py` 未实际运行；文件预算用同一行数规则人工核对。GPUI fork 与 vendor 补丁内容无法离线对照上游。

## 问题

### 阻断｜AI hook 宿主管道从未启动，已安装的 CLI hooks 事件被静默丢弃

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `spawn_gpui_server`；`slterm_app/src/ai_hook.rs` 的再导出；`slterm_app/src/agent_host.rs` 的 `AgentHub::dispatch_lifecycle`；`slterm_app/src/notify.rs` 的 `from_accepted_hook`；`slterm_app/src/app_shell/shell_events.rs` 的 `SltermShellEvent`；`slterm_app/src/ai_hook/e2e_tests.rs`。
- 现象：生产启动链不创建命名管道服务，也不把 hook 事件接入壳事件通道；`spawn_gpui_server` 只被 `ai_hook::e2e_tests` 直接调用。安装器仍会把 `slterm-hook` 写入 claude/kimi/codex 配置；helper 按设计只认 `SLTERM_NOTIFY_PIPE`，找不到管道即无声 exit 0，因此事件在真实产品中永远到不了宿主。`AgentHub::dispatch_lifecycle` 与 `notify::from_accepted_hook` 同样没有生产调用方。
- 根因：hook 传输层、解析层、pane 路由、通知缝合都已实现并有单独的端到端测试，但宿主启动装配被遗漏；测试自建服务器屏蔽了“生产没有入口”这一事实。
- 影响：AI CLI 回合完成、权限确认、会话身份、attention 状态与托盘/通知联动全部失效；用户开启 hooks 后看到的是静默无效的配置写入。这是对外可见的核心功能缺失，不是测试缺口。
- 证据：`spawn_gpui_server` 的调用点仅在 `ai_hook::e2e_tests`；`AgentHub::dispatch_lifecycle` 与 `from_accepted_hook` 的调用点仅在测试；`SltermShellEvent` 没有 hook 事件变体；`ai_hook.rs` 顶部注释声称“事件通道已接线”，但启动链中无对应装配；`slterm_app/CLAUDE.md` 对 hook 事件的描述与生产入口不一致。
- 优化方向：把宿主管道与事件排空做成启动链中唯一、有所有权、可关闭的服务，事件经有界通道进入 `AgentHub` 与通知漏斗；验收必须从真实启动入口（而非测试自建服务器）观察事件到达。

### 阻断｜多项已验收产品面只有引擎态，没有生产入口

- 置信度：确认
- 位置与现象：
  - `slterm_app/src/update_check.rs` 的 `start_gpui`、`slterm_app/src/update_download.rs` 的 `hydrate`/`begin`、`slterm_app/src/update_download/handoff.rs` 的 `schedule`/`apply_scheduled`、`slterm_app/src/app_shell/app_root/update_install.rs` 的 `begin_update_install`：检查、下载、预约安装均无生产调用方；`begin_update_install` 以 `#[allow(dead_code)]` 标注“消费点 = 更新提示 UI 的后台准备回执接线”。更新链没有 UI 或命令入口。
  - `slterm_app/src/plan_balance` 的 `PlanBalanceService`/`PlanBalanceClient`：只在自身模块与测试中被引用；`background_tasks::PLAN_BALANCE` 的执行体为 `None`，没有订阅方把真实服务接入调度。
  - `slterm_app/src/cleanup.rs` 的 `CleanupService` 与 `slterm_app/src/diagnostics.rs` 的 `DiagnosticReport`：除清理模块自测与 `acceptance` 测试外无调用方；设置页与 CLI 都没有诊断/清理入口。
  - `slterm_app/src/ai_sessions.rs`：无生产引用。
  - `slterm_app/src/history_quarantine.rs` 与 `slterm_app/src/app_shell/app_root/navigation.rs`：隔离批次与历史恢复落点只被测试调用；`SideViewId::Nav` 的实际视图 `app_root/nav.rs` 只渲染项目行，没有活跃/历史会话行、删除隔离区或套餐页脚。
  - `slterm_app/src/settings_transfer.rs`：导入/导出函数无生产调用方；`slterm_settings` 的 `restore_default_settings_at` 无生产调用方，`settings_store::with_save_lock` 只被测试使用。
  - `slterm_app/src/theme_library/preferences.rs`：模块整体 `#![allow(dead_code)]`，注释声称生产应用路径经 `SettingsPane::try_persist_checked`，但该符号在仓库中不存在；自定义主题导入/应用没有设置页入口。
  - `slterm_app/src/encrypted_backup` 的 `stage_restore`/`BackupSelection`：除启动维护恢复器与测试外无入口；导出/恢复 UI 不存在，而 `slterm_app/src/encrypted_backup/CLAUDE.md` 仍把“真实备份/恢复 UI 流程由构建产物人工验收”作为兜底。
  - `slterm_app/src/tray.rs` 的 `update`、`background_tasks` 的 `AGENT_SCREEN_WATCHDOG` 订阅、`AgentHub::observe_screen`：无生产调用方；托盘菜单拿不到 agent 快照，屏幕兜底识别从不运行。
- 根因：按引擎/模块切片开发并以模块内用例验收；产品编排、设置页、导航视图与启动装配被推迟，但 `progress.md` 已把相关阶段标为“已验收”，`traceability.md` 把模块内自测写成“已自动化”集成证据。
- 影响：featureSpec 中的套餐余量、历史会话与撤销、诊断与清理、设置导入导出与恢复默认、自定义主题、加密备份导出/恢复、更新检查与安装、托盘 agent 菜单在真实产品中不可用；大量 `#[allow(dead_code)]` 与“消费点归后续”注释掩盖了这一状态。
- 证据：上述符号的全局调用点仅测试或模块内部；`slterm_app/src/lib.rs` 多处模块注释自述“尚无壳侧调用方”“消费点归后续”；`progress.md` 的“全部阶段验收完成”与 `traceability.md` 的对应“已自动化”行和代码入口不一致；`docs/devPlan/plan.md` 明确“测试假端口不得变成生产占位面板、伪成功入口或第二套领域通道”且“未完成不得以代码已存在标记已验收”，而上述模块正以引擎测试维持完成状态；`slterm_app/src/update_download/handoff.rs`、`theme_library/preferences.rs`、`platform/update_installation.rs`、`app_shell/app_root/documents.rs` 存在模块级 `#![allow(dead_code)]`，使未接线代码不被静态门禁发现。
- 优化方向：验收判据改为“从真实启动、命令或 UI 入口可观察”，模块引擎测试不能替代；未接线功能要么完成编排，要么从已验收状态与文档中移除，不得以 dead_code 豁免维持“完成”。

### 重要｜授权代次在生产路径恒为零，`PathAuthority` 不存在

- 置信度：确认
- 位置：`slterm_path/src/policy.rs` 的 `PathPolicy::authorize` 与 `PathRequest.expected_authorization_generation`；`slterm_app/src/app_shell/documents.rs` 的 `DocumentOpenService::resolve_with_grants`；`slterm_app/src/app_shell/app_root/documents.rs` 的 `save_document`；`slterm_app/src/app_shell/app_root/batch_close.rs`、`application_close.rs`、`project_close.rs` 的保存阶段；`slterm_app/src/app_shell/app_root/explorer.rs`；`slterm_app/src/git/repository.rs` 与 `git/show.rs`；`slterm_app/src/persistence/document_snapshot/recovery.rs`。
- 现象：所有生产授权调用都传 `0`（请求的期望代次与当前代次同时为 0）；保存、关闭、Git 状态/绑定、Diff/GitShow、Explorer 与恢复读取均如此。仓库中没有任何代码递增授权代次，也没有设计文档所述的 application `PathAuthority` 类型持有根授权与代次。
- 根因：授权代次被设计为可撤销性闸门，但 application 侧从未建立其所有者；各调用点自行拼装 `PathGrant`，代次只是形式的参数。
- 影响：`GenerationMismatch` 分支在生产中不可达；“删除根立即撤销其能力并增加授权代次”的合同失效。任何在根撤销前捕获 grant 与代次的在途请求（保存、外部刷新、Git 写）都无法被代次识别，只能依赖调用点是否恰好重算 grants。
- 证据：`resolve_with_grants` 同时写入 `expected_authorization_generation: 0` 并以 `0` 调用 `PathPolicy::authorize`；`save_document` 调用 `begin_save(document_id, 0)`；批量、应用与项目的关闭链调用 `prepare_document_save_stage(..., 0)`；`explorer` 的 `PathRequest` 为 0；`GitFacade::status/repository_binding` 的调用点均传 0；`document_snapshot/recovery` 的请求为 0；全局检索无代次递增点，无 `PathAuthority` 定义。
- 优化方向：建立单一 application 授权所有者，持有根 grant 集合、会话 grant 与单调代次；所有打开、保存、删除、回滚、粘贴、恢复在发起与执行前都从该所有者重取 grant 与代次，而不是携带固定零。

### 重要｜未知 ViewKey 类型会使整库布局恢复失败，而非过滤并聚合提示

- 置信度：确认
- 位置：`slterm_workspace/src/view_key.rs` 的 `ViewKey::from_canonical_json`；`slterm_workspace/src/project_layout.rs` 的 `PanelGroupDto::into_domain` 与 `ProjectLayout::from_canonical_json`；`slterm_app/src/persistence/project.rs` 的 `decode_project_snapshot` 与 `ProjectStoreError::Layout`/`CorruptRecord`；合同见 `docs/featureSpec/README.md` 与 `docs/devPlan/design/domain-model.md`。
- 现象：内嵌 ViewKey 的 `kind` 未登记时 `ViewKeyKind::parse` 返回 `None`，`ViewKey::from_canonical_json` 返回 `UnknownKind`；`PanelGroupDto::into_domain` 用 `?` 直接上抛，整个 `ProjectLayout` 解码失败，项目读取返回布局错误/损坏记录，启动恢复失败并进入诊断/阻断分支。没有“丢弃未知视图并继续恢复其余面板”的转换。
- 根因：严格持久 DTO 与容错恢复共用同一解码入口；只区分了结构损坏与未知类型，却没有为未知类型提供过滤层。
- 影响：任一未知面板类型（新版本写入、回滚、未来扩展）都会让整个工作区无法加载，而不是仅丢失该面板；与 featureSpec“未知类型恢复时过滤并聚合提示，不创建兼容替代类型”和 domain-model“结构非法与未知类型分开处理”直接冲突。
- 证据：`ViewKeyKind::parse` 的 `None` 分支；`PanelGroupDto::into_domain` 对 `active` 与每个 `views` 元素调用 `ViewKey::from_canonical_json` 并短路；`ProjectLayout::from_canonical_json` 将其包为 `LayoutDtoError::ViewKey`；`decode_project_snapshot` 直接传播；`SideViewId` 的恢复链有独立未知过滤，而 ViewKey 链没有。
- 优化方向：在持久布局解码中把“未知 kind”与“结构损坏/领域非法”分离，先按类型过滤未知视图并聚合警告，再对剩余视图执行严格领域校验。

### 重要｜合同冲突：加密备份类别在 featureSpec 与 crate 文档间不一致，实现仍走旧 `session.json`

- 置信度：确认
- 位置：`slterm_app/src/encrypted_backup.rs` 的 `BackupCategory`、`category_root`、`collect_from`；`slterm_app/src/encrypted_backup/maintenance.rs` 的 `stage_restore`；`slterm_app/src/update_download/handoff.rs` 的 `apply_scheduled_at`；`slterm_app/src/session.rs` 的 `save_final`/`load_from`；合同见 `docs/featureSpec/backup-and-recovery.md`。
- 现象：备份类别是 `Settings`、`Session`、`Themes`；`Session` 从本地根读取 `session.json`，而当前持久化权威是 `workspace.db`，`session.rs` 的 `save_final`/`save_to` 没有生产调用方，应用不会写 `session.json`。featureSpec 要求的工作区与界面状态类别（项目元数据、布局、统一 ViewKey 状态、侧栏、终端编号/标题/cwd、窗口几何）没有任何收集实现。更新 handoff 的 `apply_scheduled_at` 仍通过 `crate::session::load_from(root/session.json)` 构造恢复快照，文件缺失即返回“缺少可恢复的工作区快照”并放弃预约安装。
- 根因：工作区从旧 session schema 迁移到 SQLite 后，备份与更新 handoff 的收集面没有随迁移重写；测试用伪造 `session.json` 维持了旧类别。
- 影响：用户导出的加密备份不含当前项目、布局、视图状态与窗口几何，跨机恢复无法还原工作区；自动安装链在真实数据目录上永远缺少可恢复快照。该行为与备份合同的“工作区与界面状态”类别直接不符。
- 证据：`collect_from` 的 `BackupCategory::Session` 分支只 `add_file(root, category, "session.json", ...)`；无逻辑工作区导出来源；`session.rs` 的写入口无生产调用；`handoff::apply_scheduled_at` 读取 `session.json` 并以其为空即失败；备份/恢复测试显式创建 `session.json` 作为夹具，`backup_workspace_category_exclusions` 以“workspace.db 不得进入备份”为前提只断言旧 `session.json` 入包，等于把错误类别固定成合同；`docs/featureSpec/acceptance.md` 把获准类别限定为“应用设置与主题”和“工作区与界面状态”，代码却以设置、会话、主题三个可选类别表示，并以旧会话文件代表工作区；`slterm_app/src/encrypted_backup/CLAUDE.md` 仍把“会话归本地根”写成当前类别合同，与 featureSpec 的工作区类别冲突；`docs/devPlan/plan.md` 要求“完成类别选择、永久排除、逻辑工作区导出和跨机几何标记”，与当前物理旧文件收集相反。
- 优化方向：备份收集与更新 handoff 都改为从当前工作区权威（SQLite 逻辑导出或受控快照）取数据；旧 `session.json` 读取通道应删除，而不是继续被生产链引用。

### 重要｜合同冲突：core 纯边界与 crate 文档不一致，补全/设置执行真实 I/O 并存在第二写盘通道

- 置信度：确认
- 位置：`slterm_completions/src/file.rs` 的 `complete_rec`、`dirs_next_home`、`expand_home`；`slterm_app/src/completion/paths.rs` 的生产调用；`slterm_settings/src/lib.rs` 的 `RawSettings::try_load_at`；`slterm_settings/src/reset.rs` 的 `restore_default_settings_at`；`slterm_settings/src/diagnostics.rs` 的 `DIAGNOSTIC_SCOPES`。
- 现象：补全 core 直接 `path.read_dir()` 枚举目录，并读取 `HOME`/`USERPROFILE`/`HOMEDRIVE`/`HOMEPATH` 解析 `~`；生产补全路径直接调用该 API。设置 core 的 `RawSettings::try_load_at` 直接读文件，被 `settings_store` 与 `ai_hook/local/config_guard` 生产使用；`restore_default_settings_at` 直接读/写/备份/rename，使用自有临时名与备份名，不经 `atomic_file`/`audit`，且无生产调用方；`slterm_settings::diagnostics` 持有进程级 thread-local 可变作用域栈。
- 根因：core 边界在文档中规定为“纯数据/纯状态机，文件与 OS 访问经 application 端口”，但 crate 级 `CLAUDE.md` 又允许“文件读写 API 显式接收路径”，两套纪律并存；实现选择了后者。
- 影响：补全在 core 内执行未受 `PathPolicy` 约束的目录读取，形成第二读取通道，无法被 application 的授权/审计统一管理；设置 core 持有读写硬机制，`reset` 的原子写语义（ACL、WRITE_THROUGH、审计）弱于 `settings_store`，一旦接线即成为第二写盘通道；core 的纯测试前提被破坏。
- 证据：`file.rs` 中 `read_dir` 与 `dirs_next_home` 位于非 `#[cfg(test)]` 代码；`completion/paths.rs` 调用 `slterm_completions::file::expand_home`/`complete_literal_with_cancel`；`lib.rs` 中 `try_load_at` 的 `std::fs::read`；`reset.rs` 的 `fs::rename`/备份写与公开导出；`settings_store.rs` 与 `config_guard.rs` 调用 `try_load_at`；`diagnostics.rs` 的 `thread_local!`；`.claude/docs/architecture/overview.md` 和 `docs/devPlan/design/architecture.md` 的 core 纯边界；`slterm_settings/CLAUDE.md` 与 `slterm_completions/CLAUDE.md` 的边界表述与实现不一致。
- 优化方向：把补全的文件枚举与 home 解析移到 application 端口（或注入目录读取接口），core 只消费已获得的条目；设置读取改为 application 读字节后交 core 解析；删除 core 内的恢复默认写通道，或将其完全收归 `settings_store`/`audit`。

### 重要｜验收与追溯大量使用模块内自测或源码字符串断言，不能证明产品行为

- 置信度：确认
- 位置：`slterm_app/src/acceptance.rs` 的 `acceptance_excluded_surface_absent`、`integration_single_authority_channels`、`acceptance_exemptions_registered`；`slterm_app/src/background_tasks.rs` 的 `integration_plan_schedule` 与 `plan_balance_runs_while_navigation_hidden`；`slterm_app/src/cleanup.rs` 的 `integration_diagnostics_cleanup`；`slterm_app/src/history_quarantine.rs` 的 `integration_history_delete_undo`；`slterm_app/src/update_download/handoff/tests.rs` 的 `update_install_uses_close_guard`；`docs/devPlan/traceability.md` 与 `progress.md`。
- 现象：`plan_balance_runs_while_navigation_hidden` 只对注册表的 `subscribe`/`run_once` 传一个计数闭包，从未创建 `PlanBalanceService`、凭据或网络请求；`integration_plan_schedule` 同样只驱动空闭包；`integration_diagnostics_cleanup` 直接调用 `CleanupService::execute`，没有从设置页或命令入口进入；`integration_history_delete_undo` 直接调用隔离批次函数，不经过导航视图；`update_install_uses_close_guard` 直接构造守卫，不经过更新提示 UI。`acceptance_excluded_surface_absent` 只扫描三个源文件中的五个禁止子串，因而不覆盖仍读取旧 `session.json` 的 `update_download/handoff.rs`；`integration_single_authority_channels` 断言源码包含某个函数名；`acceptance_exemptions_registered` 只统计豁免表中的表格标记。
- 根因：验收被定义为“模块内行为 + 源码形状”，追溯矩阵把这类用例登记为“已自动化”，没有真实启动/UI 入口的集成证据层。
- 影响：测试全绿与“全部阶段验收完成”不能证明 featureSpec 行为成立；上述未接线功能正是被这种验收方式放过的。
- 证据：上述用例体均不经过生产入口；`traceability.md` 将 `integration_plan_schedule`、`plan_balance_runs_while_navigation_hidden`、`integration_diagnostics_cleanup`、`integration_history_delete_undo`、`update_install_uses_close_guard` 标为已自动化；`progress.md` 的最终“全部阶段验收完成”引用这些证据；`docs/devPlan/plan.md` 要求“每个检查点同时观察内存状态与真实磁盘／进程副作用，不以 DTO 状态推断操作已经成功”，当前验收未满足该要求。
- 优化方向：为每项验收增加从真实入口（启动、命令、设置页、导航视图）驱动并观察外部结果的集成用例；源码扫描只保留给结构性门禁，且扫描面必须覆盖真实风险文件。

### 一般｜Kimi/Codex 配置安装写入绕过 `atomic_file` 与审计通道

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/local.rs` 的 `write_atomic`；`local/kimi.rs` 的 `ensure_kimi_hooks_in`、`remove_kimi_hooks_in`；`local/codex_notify.rs` 的 `ensure_codex_notify`、`remove_codex_notify`；对照 `local/managed_files.rs` 与 `local/codex_hooks.rs` 使用 `crate::atomic_file`；`slterm_app/src/audit.rs` 的 `settings_writes_are_centralized_in_the_audit_wrapper`。
- 现象：Kimi `config.toml` 与 Codex `config.toml` 的安装/卸载使用 `write_atomic`：直接 `std::fs::write` 临时文件后 `rename`，不复制目标 ACL、不使用 `MOVEFILE_WRITE_THROUGH`、不产生 `audit` 事件；改前备份用裸 `std::fs::copy`。同目录的 managed 文件与 Codex native hooks 却走 `atomic_file`，形成两套写语义。审计源码扫描只覆盖 claude 的 `ensure_claude_hooks_in`/`remove_claude_hooks_at` 函数体，未覆盖这些旁路。
- 根因：安装器按 CLI 形态分裂，非 claude 配置的写路径没有收编到统一原语；审计范围被定义为 claude settings。
- 影响：安全相关的用户配置修改没有审计留痕；ACL/崩溃语义与产品其他持久化不一致；扫描门禁给出“已集中”的错误信心，未来新 AI CLI 安装器可继续复制该旁路。
- 证据：`write_atomic` 的实现与调用点；`kimi.rs`/`codex_notify.rs` 的备份与写入；`managed_files.rs`/`codex_hooks.rs` 的 `atomic_file` 使用；`audit.rs` 测试只断言 claude 函数体不含旁路。
- 优化方向：所有受支持 CLI 配置写入统一经审计包装与同一原子写原语；若确有例外，需在审计/写盘纪律中登记根因与兜底，并纳入扫描。

### 一般｜应用直接依赖 `vte` 取用类型，绕过 core 类型锚点

- 置信度：确认
- 位置：`slterm_app/Cargo.toml` 的 `vte` 直接依赖与注释；`slterm_app/src/gpui_terminal/view.rs`、`element.rs`、`cursor_painter.rs`、`colors.rs` 的 `use vte::ansi::{...}`；`slterm_terminal/src/lib.rs` 的“解析器不经本 crate 再导出”注释。
- 现象：终端 frame 的公开面（`CursorShape`/`CursorStyle`/`Color`/`NamedColor`/`Rgb`）直接使用 `vte` 类型；core 刻意不重新导出，应用因此必须新增与 core 逐字相同版本/feature 的 `vte` 依赖，并直接命名第三方类型。
- 根因：跨 crate 类型锚点没有覆盖终端 frame 暴露的第三方类型；选择“应用同名钉版”替代“core 公开领域类型或再导出”。
- 影响：应用与 core 之间的类型共享依赖第三方 crate 的 source identity，而不是 core 公开 API；升级 `vte` 时必须同步修改多个 crate 与 feature 集，否则出现两套不兼容类型；违反“跨 crate 共享只经 core 公开 API/类型锚点”的边界。
- 证据：`Cargo.toml` 注释明确承认 core 不再导出、壳侧同名钉版；`view.rs`/`element.rs`/`cursor_painter.rs`/`colors.rs` 的 vte 导入；`slterm_terminal/src/render.rs` 的 frame 字段使用同一批 vte 类型；`slterm_terminal/src/lib.rs` 的“公共面只露领域出口类型”与实际 frame 公共字段不一致。
- 优化方向：由 `slterm_terminal` 定义/再导出应用需要的最小领域类型，或让 frame 使用 core 自有类型；应用不得再直接依赖解析器 crate。

### 一般｜GPUI 启动回调内同步读取设置与自定义主题

- 置信度：确认
- 位置：`slterm_app/src/app_shell/mod.rs` 的 `init`；`slterm_app/src/app_shell/config.rs` 的 `Settings::load` 与 `from_runtime_with_theme_root`；`slterm_app/src/settings_store.rs` 的 `load_settings`；`slterm_app/src/theme_library/store.rs` 的主题读取。
- 现象：`init` 在 `gpui_platform::application().run(...)` 的前台回调内调用 `config::Settings::load()`；该调用同步读取 `settings.json`（含损坏/备份分支），并加载自定义主题文件，随后才创建首窗。文件 I/O 位于 GPUI 前台线程。
- 根因：启动初始化被放进 `application.run` 闭包，而文档要求启动链在 GPUI 创建前完成读盘；设置读取没有走 background executor。
- 影响：违反“GPUI 前台线程禁止阻塞 I/O”；磁盘慢、网络重定向目录或杀软扫描会直接延迟首帧与窗口创建；该路径也无法被前台纪律的静态/运行时检查覆盖。
- 证据：`run_shell_once` 中 `init(cx)` 位于 `application.run` 回调内；`Settings::load` 直接调用 `settings_store::load_settings`，其内部 `std::fs::read` 与 `ThemeLibraryStore` 读取；`slterm_app/CLAUDE.md` 的“前台纪律”与“启动链单点”。
- 优化方向：设置与主题读取应在进入 GPUI 事件循环前完成，或经背景任务读取后以首个快照注入；启动回调内只允许纯内存装配。

### 一般｜合同冲突：WCAG 校验在 featureSpec、架构文档、crate 文档与代码之间互相矛盾

- 置信度：确认
- 位置：`docs/featureSpec/cross-cutting.md` 的“不做对比度校验、提示或自动回退”；`.claude/docs/architecture/overview.md` 的 `slterm_settings` ownership“主题色表与 WCAG 校验”；`slterm_settings/CLAUDE.md` 的主题合同与验收段；`slterm_app/src/theme_library/CLAUDE.md` 的主题合同与操作段；`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition::validate_wcag`、`WCAG_TEXT_CONTRAST`、`wcag_contrast_ratio`；`slterm_settings/src/themes.rs` 的 `meets_wcag`/`wcag_violations`。
- 现象：featureSpec 明确“不做对比度校验、提示或自动回退”；overview 把 WCAG 校验列入 `slterm_settings` 职责；settings crate 同一份文档一处禁止 WCAG、另一处要求“通过语义槽 WCAG 校验”；theme_library 同一份文档一处“不做对比度校验”、操作段又要求“新增主题…通过 WCAG 校验”。代码实现了两套 WCAG API，但生产主题保存路径只调用 `ThemeDefinition::validate`，不调用 `validate_wcag`，校验 API 仅测试消费。
- 根因：主题能力裁剪后没有统一合同；测试仍按旧的 WCAG 验收编写，文档两侧各自保留旧表述。
- 影响：无法判断“主题保存是否应拒绝低对比度”是行为合同还是死代码；未来接线会直接违反 featureSpec，不接线则保留无用公开 API 与测试；文档真值分裂。
- 证据：上述四份文档原文；`validate()` 不含 WCAG 调用，`validate_wcag` 无生产调用点；内置主题测试 `builtin_palettes_pass_semantic_slot_wcag_review` 与 `custom_theme` 的 WCAG 用例只验证 API。
- 优化方向：以 featureSpec 为行为真值裁决：若确实不做对比度校验，删除或降级 WCAG API 与验收条目，并同步 overview/crate 文档；若要做，先修改 featureSpec 并给出用户可见行为与测试。

### 一般｜业务模块与测试中的平台条件编译未按“PTY/平台封装”收敛

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/layout.rs` 的 `drag_gesture_active`（`#[cfg(windows)]` + `GetAsyncKeyState`）；`slterm_app/src/diagnostics.rs` 的 `DiagnosticSnapshot::current`；`slterm_app/src/app_shell/wallpaper.rs` 的 `apply_windows_accent_policy` 与 DWM 调用；`slterm_app/src/gpui_terminal/view/cwd_report.rs` 的 `local_windows_directory_events_keep_drive_and_unc_paths`；`slterm_app/src/app_shell/app_root/tests/terminal_restore.rs` 的 `#[cfg(windows)]`/`#[cfg(not(windows))]` 局部项。
- 现象：Win32 输入探测、系统信息格式化、DWM 材质调用直接出现在视图/壳业务模块中，并以 `#[cfg(windows)]` 分叉；测试用编译期 `#[cfg(windows)]` 隐藏非 Windows 分支，而不是运行时 `cfg!`。
- 根因：平台调用按功能就近放置，未收拢到 `platform/` 或明确的平台封装模块；测试沿用了编译期门控。
- 影响：与根 `CLAUDE.md`“业务 `#[cfg(windows)]` 仅允许出现在 PTY/平台封装模块”和“测试 `#[cfg(windows)]` 原则上改运行时 `cfg!`”冲突；新增平台差异会继续散落，非 Windows 编译面被编译期隐藏，测试在非 Windows 上不执行也不显式跳过。
- 证据：上述符号中的 `#[cfg(windows)]` 与 Win32 调用；`cwd_report` 用例整体编译期门控；`terminal_restore` 用例用 `#[cfg(not(windows))]` 伪造替代值。
- 优化方向：把 Win32 输入/材质/系统信息调用收进平台封装模块，业务层只消费纯结果；测试改为运行时 `cfg!(windows)` 分支并在不满足时显式记录跳过判据。

### 提示｜文档符号、结构与过程编号已与代码漂移

- 置信度：确认
- 位置：根 `CLAUDE.md` 的“面板封闭”与“布局单点”；`.claude/docs/architecture/overview.md` 的 ownership 表、WCAG、数据文件名；`slterm_split/CLAUDE.md` 的“会话布局只经 session schema”；根 `Cargo.toml` 的 release profile 注释；`slterm_app/src/background_tasks.rs` 的 `SETTINGS_KEY` 注释；`slterm_completions/src/lib.rs` 的头部注释；`slterm_app/src/ai_hook/local.rs` 的 statusline 注释；`slterm_runtime_protocol/src/protocol.rs` 的方法族注释。
- 现象：根文档要求 tab 为封闭枚举 `WorkspaceTab`，但仓库中不存在该类型，实际封闭枚举是 `ViewKey`/`ViewKeyKind`；根文档与 split 文档要求布局经“session schema 的 layout 转换对”，实际是 `slterm_workspace::ProjectLayout` 的规范 JSON；overview 的 ownership 表只列部分成员 crate，未覆盖 `slterm_document`、`slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；overview 把 `session.json`/`history.jsonl`/`directory_history.json` 描述为当前数据文件，而当前权威是 `workspace.db`/`learning.db` 且旧文件不再写入；代码注释仍含阶段/文档编号形态的短标识：release profile 注释以阶段名解释热路径清单回填，`SETTINGS_KEY` 注释以两段式文档编号指向消费点，completions 头部带评审条目编号，ai_hook 注释以改造小节编号定位接管点，protocol 方法族以“后续切片”表述占位。
- 根因：迭代后未按“决策就近记录”同步根文档与 crate 文档；禁用短标识规则只覆盖了部分文档，未覆盖代码注释。
- 影响：读者会按不存在的类型或数据文件名理解架构；文档门禁无法发现代码注释中的过程编号；新成员无法从 overview 判断 ownership。
- 证据：全局检索 `WorkspaceTab` 只命中 `diff_tab.rs` 的一行注释；无 `session schema` 类型/模块；overview 表格与 workspace 成员对照；`app_paths.rs` 与 `persistence` 的实际文件路径；上述注释原文。
- 优化方向：以当前代码符号重建根文档的封闭枚举、布局 DTO、ownership 表与数据文件清单；把过程编号从代码注释与配置注释中清除，文档策略门禁扩展到源文件注释。

### 提示｜运行期全局状态与未 join 的后台线程/任务仍存在

- 置信度：确认
- 位置：`slterm_app/src/tray.rs` 的 `STATE`、`GPUI_COMMAND`、`THREAD_STARTED`、`thread_slot`；`slterm_app/src/update_download/handoff.rs` 的 `ACTIVE_RESTORE`；`slterm_app/src/background_tasks.rs` 的 `RUNTIME_STORE`/`RUNTIMES`；`slterm_app/src/app_shell/fs_watch.rs` 的 `FsWatchHandle`（`Drop` 只关闭通道，`_worker` 未 join）；`slterm_app/src/side_panel/search/worker.rs` 的 `EmbeddedFileIndex::new`（线程句柄丢弃，仅 `stopped` 标记）；`slterm_app/src/update_check.rs` 的 `start_gpui`（`.detach()`）；`slterm_app/src/gpui_terminal/view.rs`、`view/layout.rs`、`view/pointer.rs`、`app_shell/wallpaper.rs`、`diff_tab.rs`、`doc_tabs.rs` 的 `.detach()`。
- 现象：运行值（托盘快照/回调、更新恢复票据、任务配置）存放在模块级可变 static，而不是 Entity 或 worker 所有者；若干线程/任务有停止信号但不 join，`Detach` 的 GPUI 任务也没有 TaskService 登记。
- 根因：进程级单例被就近实现为 static；关闭链只保证“发出停止”，没有统一等待归属。
- 影响：与根架构“禁止模块级可变全局状态”“所有 watcher、线程和任务都有明确所有者与停止路径”“TaskService 禁止 detach”不一致；测试之间共享全局状态需要手动 reset；关闭时无法证明线程已退出，迟到回调可能访问已销毁状态。
- 证据：上述 static 与线程创建/丢弃点；`tray` 的 `shutdown` 只 join 托盘线程，搜索/FS watcher 不 join；`fs_watch` 的 `Drop` 注释明确“前台不 join”；`TaskService` 之外存在多处 `detach`。
- 优化方向：把进程级单例收进启动所有者并暴露显式 shutdown/join；后台线程统一登记到单一所有权表；停止信号必须伴随可验证的终止回执。

## 跨域观察

- `slterm_app/src/session.rs` 仍保留旧会话 schema 的 `save_to`/`save_final`/`load_from` 与版本常量，且被备份/更新链引用；该模块自身注释称“生产 AppRoot 不读取”，与实际的跨域引用不一致。此项已并入备份与更新相关条目，但旧 schema 的删除范围超出本范围。
- `slterm_app/src/platform/update_installation.rs` 与 `update_download/handoff.rs` 的模块级 `#![allow(dead_code)]` 使更新安装的未接线状态不被 clippy/门禁发现；若后续接线，需同时验证文件事务与关闭守卫。
- `slterm_app/src/agent_dirs.rs`、`agent_root_service.rs` 的 AgentRoot 管理 API 只有加载与测试消费，自定义根注册 UI 未接入；这属于 Agent 文件树产品面的一部分，已由“引擎态无入口”条目覆盖。

## 存疑未证

- GPUI fork（`LiuShenLan/zed` 固定 rev）与 vendor `dav1d-sys`/`tree-sitter-json5`/`tree-sitter-dockerfile` 的实际补丁内容无法离线对照上游；只能确认锁文件 rev、`build.rs` 的 `--vsenv` 与 provenance 审计锚。需要固定 fork 的提交内容与上游 tag 对比才能确认“只做减法”成立。
- 授权代次恒零的实际可利用路径未做运行时验证：代码证明代次机制不前进，但在途保存/外部刷新是否确实携带旧 grants 写入，需构造根删除与在途请求的并发场景。缺该证据。
- 未知 ViewKey 过滤的缺失在真实数据中的触发需要未来版本写入或手工构造布局；当前只证明解码路径会拒绝，未证明线上数据会出现。
- 更新/套餐/诊断等模块的真实 GUI 行为无法在无构建、无运行条件下确认；本报告的“无生产入口”结论来自静态调用图，若存在生成代码或未检索到的动态注册，结论需复核。
- 使用停止标志或通道关闭但不 join 的线程（FS watcher、搜索 worker、更新检查）是否在关闭后立即退出，未做运行期句柄/线程计数验证；当前只能确认其生命周期未被显式证明。
- 文件预算未运行 `check_architecture.py`；人工核对未发现超过 2000 行且无例外的第一方源码，但生成目录排除规则与脚本行为未实测。
- `Cargo.lock` 只抽样核对 GPUI/vendor/tree-sitter；未做完整 `cargo tree`，重复传递依赖与 feature 统一对 binary 体积/行为的影响未证。
