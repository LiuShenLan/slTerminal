# 测试用例质量与覆盖独立审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`；只读审查当前工作树，未执行 cargo、测试、构建或网络访问。

## 覆盖与局限

- 已读：根级项目指令、`.claude/docs/testing/strategy.md`、`exemptions.md`、`.claude/docs/operations/commands.md`、`docs/devPlan/design/testing.md`、`docs/devPlan/traceability.md`、`plan.md`、`progress.md`、`.github/workflows/ci.yml`、全部 workspace 成员的 `Cargo.toml`、`architecture/dependencies.toml`、脚本门禁与发布脚本、`assets/runtime-control-api.md`、`docs/devPlan/design/runtime.md`。
- 已读并抽查：各 crate 的测试模块与关键用例体，覆盖路径、Document、Workspace、Terminal、AppRoot 虚拟窗口、Runtime、Settings、Sidebar、Hook、加密备份、更新链、打包与门禁脚本。
- 已读：`slterm_terminal/tests/pty_integration_tests.rs`、`slterm_app/tests/windows_console_startup.rs`、`slterm_app/tests/i18n_contract.rs`、`tools/slterm-i18n-contract` 的清单与构建脚本、`scripts/tests/package-release.tests.ps1`、`scripts/tests/windows-runtime.tests.ps1`、`scripts/generate-asset-previews.ps1`。
- 未执行：任何 cargo、测试、构建、安装器、真实 GUI、真实 PTY、网络或脚本；因此“测试当前是否全绿”不在本报告结论内。未逐行阅读全部测试体，采用符号、清单、门禁调用链与代表性用例交叉核对。
- 未审查历史 diff；只按当前工作树与合同要求判断。

## 问题

### 重要｜Runtime schema 合同测试不在任何固定门禁内

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol_tests.rs` 的 `generated_schema_matches_the_packaged_asset`、`schema_generator_recreates_missing_output_file`、`schema_generator_replaces_existing_output_file`、`schema_generator_failure_preserves_existing_output_file`、`schema_accepts_discovery_and_rejects_sensitive_fields`、`schema_accepts_real_requests_and_rejects_unknown_methods`；`slterm_runtime_protocol/Cargo.toml` 的 `[features] schema` 与 `[[bin]] runtime-schema`；`.claude/docs/operations/commands.md`；`.github/workflows/ci.yml`；`assets/runtime-control-api.md` 的“操作”段；`scripts/build-windows-product.ps1` 的构建参数；`docs/devPlan/plan.md` 的 Runtime v2 退出证据与 Windows 发布行
- 现象：上述用例全部以 `#[cfg(feature = "schema")]` 门控，而全量测试、core release、虚拟窗口、clippy 与发布构建均未启用 `schema`；`runtime-schema` 因 `required-features = ["schema"]` 也不会被 `cargo clippy --all-targets` 或发布构建编译。`assets/runtime-control-api.md` 给出了定向测试命令，但该命令没有进入命令矩阵、CI 或发布门禁。
- 根因：可选 feature 的 schema 生成与资产比对被当成人工操作；治理测试只钉 core release 命令，没有钉 schema 命令；发布清单把 `runtime-api-v2.schema.json` 当静态资产收录，未要求重新生成并比较。
- 影响：随包 schema 可与 DTO 生成器漂移而不被任何自动化发现；hello／describe 携带的 schema 指纹由静态资产字节计算，漂移会让客户端按错误结构校验；schema 对 discovery 中敏感字段的拒绝、未知方法拒绝、生成器失败保留旧文件等行为都没有回归保护。发布门禁全绿不代表 Runtime 合同一致，`plan.md` 要求的“schema 漂移测试通过”无法由命令矩阵产出。
- 证据：`slterm_runtime_protocol/src/lib.rs` 中 `#[cfg(feature = "schema")] pub mod schema;`；schema 用例实现逐字节读取 `assets/runtime-api-v2.schema.json` 并调用生成器；`ci.yml` 与命令矩阵无 `--features schema`；`build-windows-product.ps1` 只构建 `slterm` 与 `slterm-hook`；`release-checklist.md` 无 schema 重新生成或比对项；`plan.md` 的 Runtime v2 退出证据要求 schema 漂移测试通过，Windows 与发布行的交付要求核验 schema。
- 优化方向：把 `cargo test -p slterm_runtime_protocol --test lib_tests --features schema -- --test-threads=1` 纳入命令矩阵、CI 与发布前门禁，并对 `runtime-schema` 二进制单独执行构建或 clippy；若继续保持人工，必须在豁免表登记根因与兜底，不能把它当作已自动化合同。

### 重要｜Runtime 双进程 conformance 缺失，Runtime v2 退出证据没有对应测试

- 置信度：确认
- 位置：`docs/devPlan/design/testing.md` 的“测试层次与隔离”与“Runtime”行；`docs/devPlan/design/runtime.md` 的“真实 GUI／CLI conformance”；`docs/devPlan/plan.md` 的 Runtime v2 交付、退出证据与集成检查点；`scripts/tests/test_runtime_retirement.py`；`slterm_app/tests/`；`slterm_app/src/local_control/server_tests/`
- 现象：设计把“双进程 conformance”列为 Runtime 域的主要证据，要求启动真实 GUI 与 CLI、使用临时 AppPaths、隔离数据根与假凭据；计划把“真实 GUI／CLI 的握手、断线重查、重复 mutation、事件缺口、非交互关闭、关闭中拒绝和 schema 漂移”作为 Runtime v2 退出证据，并把“Runtime 双进程断线”列为集成检查点。当前 `slterm_app/tests/` 只有 i18n 合同与 console 启动目标；`test_runtime_retirement.py` 反而断言 `slterm_app/tests/runtime_conformance.rs` 不存在。现有 Runtime 测试是进程内真实 loopback 服务与 AppRoot 派发测试，不是真实 GUI 进程 + 真实 CLI 客户端的双进程矩阵。
- 根因：旧 Runtime 的 conformance target 随旧栈删除后，没有以新形态重建；追溯矩阵把 Runtime 场景映射到进程内 server 与 AppRoot 用例，未登记双进程证据缺口。
- 影响：握手、发现 CLI、跨进程凭据轮换、事件续传与断线、非交互关闭、关闭中拒绝、GUI 首帧后开放控制面与 v1 拒绝没有在真实进程边界上验证；计划与设计要求的 Runtime v2 退出证据无法由现有自动化产出，而 `progress.md` 已把 Runtime v2 记为已验收。进程内测试不能替代双进程的发现、认证、生命周期与断线行为。
- 证据：`slterm_app/tests/` 文件集；`test_runtime_retirement.py` 的 `test_legacy_runtime_implementation_is_removed` 断言 `runtime_conformance` 不存在；`local_control/server_tests` 与 `app_root/tests/runtime_*` 均为进程内测试；设计文档明确要求“双进程 conformance”；计划退出证据明确要求真实 GUI／CLI 矩阵；豁免表没有该缺口的条目。
- 优化方向：以真实 GUI 进程与真实 CLI 客户端重建 Runtime conformance target，覆盖设计列出的方法族与事件断线矩阵，并纳入 CI 或发布门禁；若暂时不实现，必须登记豁免、说明根因与人工兜底，不能把进程内测试写成双进程验收。

### 重要｜发布钉测试接线断点：安装器参数测试缺失，运行时钉测试未执行

- 置信度：确认
- 位置：`scripts/release.ps1` 的钉测试循环；`scripts/tests/` 中缺失的 `installer.tests.ps1`；`scripts/tests/windows-runtime.tests.ps1`；`.github/workflows/ci.yml`；`scripts/CLAUDE.md`；`.claude/docs/release/packaging.md`；`.claude/docs/operations/commands.md`；`scripts/installer.iss`；`scripts/build-installer.ps1`
- 现象：`release.ps1` 只遍历 `installer.tests.ps1` 与 `package-release.tests.ps1`，并用 `Test-Path` 静默跳过不存在的文件；`installer.tests.ps1` 在仓库中不存在。`windows-runtime.tests.ps1` 虽然存在于 `scripts/tests/`，但既未被 `release.ps1` 调用，也未被 CI 调用。命令矩阵与打包文档都把它列为钉测试。
- 根因：测试层用“存在时强制执行”的软接线，缺文件不报红；治理测试没有钉住钉测试文件的存在性与调用关系；安装器参数化没有落地为可执行测试文件，或用例文件在迁移中缺失。
- 影响：安装器 AppId、夹具 GUID、AppUserModelId、输出名、版本注入与夹具／生产隔离没有自动化验证；`installer.iss` 中的 `#ifdef AcceptanceFixture` 分支只能人工目测。运行时 DLL 的复用、修复、架构拒绝与损坏归档拒绝同样没有进入 CI 或发布链。发布可以全绿而安装器身份或运行时选择错误，影响续装、升级与卸载。
- 证据：`release.ps1` 的循环与 `Test-Path` 条件；`scripts/tests` 文件集中无 `installer.tests.ps1`；`commands.md` 与 `packaging.md` 列出 `windows-runtime.tests.ps1`，但 `ci.yml` 与 `release.ps1` 均无调用；`package-release.tests.ps1` 的自测只检查 profile、清单、体积、新鲜度与 `installer.iss` 的局部行，不解析夹具 GUID／AppId 隔离；`build-installer.ps1` 的 `-AcceptanceFixture` 无测试调用。
- 优化方向：恢复安装器参数化测试，或把夹具 GUID／AppId／输出名／版本参数化纳入 `package-release.tests.ps1` 的自测；把 `windows-runtime.tests.ps1` 接入 CI 或 release，并为其准备固定归档夹具；钉测试文件缺失时门禁必须红而不是跳过。若确属人工面，须在豁免表写明根因、兜底与核验记录。

### 重要｜追溯矩阵存在无法解析的“已自动化”目标例名，验收证据不可复现

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 PathPolicy、Commit、Agent 全局文件行；`docs/devPlan/progress.md` 的文件与 Git 验收行；`docs/devPlan/plan.md` 的 Document 聚合、通用回滚与 Agent 文件树行为行；实际用例 `slterm_app/src/app_shell/documents.rs` 的 `integration_links_and_file_identity`、`slterm_app/src/agent_root_service.rs` 的 `agent_root_default_capabilities`、`slterm_app/src/app_shell/app_root/tests/commit.rs` 的 `integration_commit_rollback`
- 现象：矩阵、计划与进度记录中的 `path_file_identity_deduplicates_document`、`commit_rollback_preserves_staged_changes`、`agent_files_default_capabilities` 在源码中没有同名测试；`progress.md` 还把 `commit_rollback_preserves_staged_changes` 写成已通过的验收锚。对应场景实际由改名后的用例覆盖，但文档没有更新。
- 根因：测试名核销是人工维护，缺少机械解析；`scripts/check_doc_policy.py` 只检查样式、失效路径与行号，不校验追溯目标例名是否存在；跨域验收锚只检查豁免表外形，不检查追溯矩阵内容。
- 影响：按矩阵逐条核销时无法定位证据，验收记录与源码事实不一致；`plan.md` 明确要求“已验收阶段不得引用不存在的测试例”，当前记录违反该约束。后续重命名会继续积累语义漂移，最终无法用矩阵证明覆盖。
- 证据：`rg` 只能在这些名字的文档位置命中；实际用例分别使用硬链接打开复用、AgentRoot 默认能力、staged／unstaged 回滚组合场景；`integration_commit_rollback` 的断言覆盖 staged 差异保留与未暂存差异放弃，语义与矩阵行一致。
- 优化方向：把矩阵、计划与进度中的目标例名改成真实符号名，或把实际用例改名；增加一个门禁脚本，解析矩阵反引号中的测试名并在源码 `#[test]`／`#[gpui::test]` 集合中解析，失败即红。豁免表与进度记录引用同一解析器。

### 重要｜AppRoot 后台 Agent 根加载不可注入，虚拟窗口用例读取真实用户 home

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/explorer.rs` 的 `sync_side_view_roots`；`slterm_app/src/app_shell/app_root/agent_files.rs` 的 `ensure_agent_roots_loading`、`AgentFilesRoot::new`、`agent_files_begin`、`agent_files_roots`；`slterm_app/src/agent_dirs.rs` 的 `agent_global_dirs`／`resolve_agent_dirs`；`slterm_app/src/platform/dirs.rs` 的 `home_dir`；用例 `slterm_app/src/app_shell/app_root/tests/sidebar.rs` 的 `side_view_shortcut_bindings_cover_all_registered_views` 与 `tests/explorer.rs` 的 `open_explorer`
- 现象：`sync_side_view_roots` 无条件调用 `ensure_agent_roots_loading`，后台任务调用 `agent_global_dirs`，后者经 `home_dir` 读取 `USERPROFILE`／`HOME` 并探测真实 Agent 目录。`AppRoot` 默认 `agent_global_dirs = None`、`agent_roots_loaded = false`；只有 `agent_files.rs` 的测试通过 `replace_agent_global_dirs_for_test` 注入，其余 AppRoot 测试（例如打开 AgentFiles 的侧栏快捷键用例）会让后台任务把真实 home 目录写入根列表。`AgentFilesRoot::new` 默认 `expanded: true`，`agent_files_begin` 会对已展开根发起目录读取。
- 根因：home／Agent 目录解析没有可注入端口；测试夹具没有在 `AppRoot` 构造时标记根快照已加载，也没有默认空根；后台加载任务没有测试构建短路。
- 影响：测试行为依赖开发机的 `.claude`、`.codex` 等目录是否存在及其内容；打开 AgentFiles 的用例会把真实用户目录纳入文件树读取，违反“测试不读写真实用户目录”的隔离纪律。不同机器上根集合、读取耗时与失败路径不同，可能出现只在特定开发机复现的红灯或假绿。
- 证据：`agent_dirs::resolve_agent_dirs` 只接收 home 参数，但公开入口 `agent_global_dirs` 固定使用 `crate::platform::dirs::home_dir()`；`ensure_agent_roots_loading` 的回投代码在 `agent_roots_loaded` 为假时写入真实目录；`replace_agent_global_dirs_for_test` 仅被 `tests/agent_files.rs` 使用；侧栏用例通过 `ctrl-shift-a` 打开 AgentFiles 后执行 `draw_frame`／`run_until_parked`。
- 优化方向：为 AppRoot 增加可注入的 Agent 目录／home 端口，测试默认注入空集或临时 home；让后台加载在测试构建可显式完成或禁用；在 `open_root` 夹具中默认标记根快照已加载，只有 AgentFiles 专用用例再注入真实临时根。

### 一般｜PTY 集成层的条件跳过未登记豁免

- 置信度：确认
- 位置：`slterm_terminal/tests/pty_integration_tests.rs` 的 `default_shell_resolve_chain_osc_marks`、`pwsh_integration_script_osc_semantics`、`bundled_psreadline_first_input_is_not_polluted`；`.claude/docs/testing/exemptions.md` 的“条件跳过用例”段
- 现象：用例在找不到 `pwsh.exe`／`powershell.exe` 时打印跳过信息并直接返回；`default_shell_resolve_chain_osc_marks` 在默认解析回退 `cmd` 时也直接返回。豁免表的条件跳过清单只登记了应用执行别名／ACL 与真实凭据管理器。
- 根因：条件跳过是就地实现的，没有同步到单表真值源；CI 镜像通常带 pwsh，其他环境的空跑不会被发现。
- 影响：没有 pwsh 的机器或镜像会把 OSC 集成、PSReadLine 首输入、DA1 握手相关用例计为通过，但实际未执行；验收记录无法区分“通过”与“跳过”。
- 证据：相关分支均为 `eprintln!` 后 `return`；豁免表“条件跳过用例”没有对应条目；`strategy.md` 要求条件跳过必须登记根因与兜底层级。
- 优化方向：要么在 CI 与开发机保证 pwsh 存在并把缺失改成失败，要么把这几个用例登记进豁免表，写明跳过后仍由哪些真实进程用例兜底。

### 一般｜进程级环境变量与全局端口测试缺少 ENV_LOCK／RAII 守卫

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/local/settings.rs` 的 `uninstall_removes_own_hooks_and_preserves_user_configuration`；`slterm_app/src/ai_sessions.rs` 的 `HomeEnvGuard`；`slterm_app/src/update_download/handoff/tests.rs` 的 `integration_update`；`slterm_app/src/notify.rs` 的 `integration_notification_taskbar`；`slterm_app/src/app_shell/app_root/tests/taskbar.rs`；`slterm_terminal/src/tty/windows/spawn.rs` 的 `spawn_lock_serializes_capacity_check_and_reservation`
- 现象：应用 crate 的测试用例直接 `set_var`／`remove_var` 进程级变量，没有统一 ENV_LOCK；`SLTERM_UPDATE_RESTORE` 与 `CLAUDE_CONFIG_DIR` 都是先设置、执行、再删除，断言失败会跳过恢复。全局 `WindowsIntegration` 端口由多个用例安装／重置；`LIVE_PTY_PROCESSES` 被用例直接改写并在结尾恢复。这些用例的注释明确依赖“全量测试串行”或进程级独占。
- 根因：应用 crate 没有像 `slterm_terminal` 那样的 crate 级 ENV_LOCK；全局端口只有测试专用安装函数，没有按测试上下文隔离；RAII 守卫没有统一。
- 影响：任何未带 `--test-threads=1` 的定向运行、未来并行化或失败清理都会发生污染；失败用例留下的环境变量与静态计数会改变后续用例结果，表现为间歇红灯或把真实缺陷掩盖成环境噪声。
- 证据：`slterm_terminal/src/lib.rs` 定义 `ENV_LOCK`，`shell.rs`、`environment.rs`、`conpty.rs` 的用例都持锁；`slterm_app` 只有 `gpui_terminal/session.rs` 内局部 ENV_LOCK，`ai_hook/local/settings.rs` 与 `update_download/handoff/tests.rs` 无锁；`notify.rs` 与 `tests/taskbar.rs` 直接写全局端口。
- 优化方向：在应用测试支持层提供统一 ENV_LOCK 与 RAII 环境守卫；全局 WindowsIntegration 改为按 `TestAppContext` 注入或提供串行守卫；`LIVE_PTY_PROCESSES` 的用例用 Drop 守卫恢复，避免断言失败后污染。

### 一般｜SPAWN_LOCK 串行用例复刻生产逻辑而非调用生产入口

- 置信度：确认
- 位置：`slterm_terminal/src/tty/windows/spawn.rs` 的 `spawn_lock_serializes_capacity_check_and_reservation`，对照生产 `spawn_locked`
- 现象：用例注释写明“复刻 `spawn_locked` 的锁内闭环”，用例体自己加 `SPAWN_LOCK`、自己调用 `ensure_pty_capacity`、自己 `fetch_add`，从未调用 `spawn_locked`。
- 根因：真实 ConPTY spawn 成本高，抽出纯容量函数后，锁作用域与登记顺序没有可复用入口；用例选择复制一段实现。
- 影响：如果生产把容量判定移到锁外、改变锁顺序或把登记移出锁内，这个用例仍然全绿；单元层无法证明“锁内容量判定 + 占用登记”的实际实现。真实进程层有 `integration_pty_limit` 兜底，但两层语义不再对齐。
- 证据：用例体与生产 `spawn_locked` 的步骤相似；用例只验证复制片段。
- 优化方向：把锁内闭环抽成生产与测试共用的闭包／辅助函数，或让测试走 `spawn_locked` 的轻量注入点；断言锁外不可观察的中间态应交给真实 spawn 层。

### 一般｜跨域验收锚多为源码字符串与表头计数，判据范围小于声明场景

- 置信度：确认
- 位置：`slterm_app/src/acceptance.rs` 的 `acceptance_user_contract_observable`、`acceptance_excluded_surface_absent`、`integration_single_authority_channels`、`acceptance_exemptions_registered`、`performance_claims_have_conditions`、`sensitive_material_excluded_from_outputs`；`docs/devPlan/traceability.md` 的“跨域验收”行
- 现象：多个“全量验收锚”只检查少量源码字符串或文档外形。豁免锚只验证表头、`| 保留 |` 行数与“人工验收”字样；单一权威锚只检查 `git_changes` 的几个符号以及少数文件不含 `std::process::Command`；排除面锚只扫描少数模块；用户契约锚只验证 `clean_shutdown` 标记；性能锚不扫描 `docs/featureSpec`，且 `dated_section` 一旦遇到日期就对该文件后续所有行生效；敏感材料锚只覆盖诊断与审计，不覆盖日志、错误与备份。
- 根因：验收锚以低成本源码／文档检查替代行为验证，没有区分“文档门禁”和“覆盖证明”；没有共享的机器可读覆盖索引。
- 影响：锚可以保持全绿而实际覆盖或文档事实已经漂移；本次发现的追溯例名缺失与豁免悬空就是这种漂移的实例。跨域“全量验收已核销”的结论因此缺少独立支撑。
- 证据：`acceptance_exemptions_registered` 只做 `contains`／`matches`；`performance_claims_have_conditions` 的扫描根为 `.claude/docs`、`docs/devPlan`、`release-notes`、`CONTEXT.md`、`README.md`，不含 `docs/featureSpec`；`performance_claims_have_conditions` 的 `dated_section` 变量在循环外累积；`integration_single_authority_channels` 只检查 `git_changes`、`commit.rs`、`local_control/git.rs`、`runtime_snapshot.rs`、`events.rs`、`workspace_writer.rs`、`application_close.rs` 的字符串。
- 优化方向：把文档／源码门禁与行为验收分离；行为锚断言可观察结果或引用机器可读覆盖索引；性能扫描覆盖 `featureSpec`，日期豁免按节或按段落重置；豁免锚解析每一行并校验兜底测试名存在。

### 一般｜`runtime_token_absent_from_materials` 的实际断言只覆盖诊断材料

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `runtime_token_absent_from_materials`；`docs/devPlan/traceability.md` 的 Runtime 行“端点、设置、备份和诊断不含 Runtime token”
- 现象：测试注释与矩阵声称覆盖端点、设置、备份和诊断，但测试体只构造 `DiagnosticReport`，断言导出文本不含 token、包含 `[redacted]`、预览与导出一致，并检查报告分区全部属于白名单。
- 根因：锚名与注释按最终场景命名，实现只落地了诊断一段；端点材料由 `slterm_app/src/local_control/boundary_tests.rs` 的 `integration_runtime_credential_boundaries` 覆盖，`plan.md` 也把该用例写成端点、设置与备份证据，但 `traceability.md` 只在该行列出诊断用例；设置与备份没有在这个锚里出现。
- 影响：按矩阵核销时，端点／设置／备份的 token 缺席无法从该例名得到证明；设置导出与备份的凭据排除依赖其他用例，覆盖关系没有被记录。
- 证据：测试体只有 `DiagnosticReport` 断言；审计材料断言在 `sensitive_material_excluded_from_outputs`；`boundary_tests.rs` 单独验证端点文件与 discovery JSON；`settings_transfer.rs` 与加密备份用例另行验证凭据字段排除。
- 优化方向：拆成按材料命名的多个用例，或在矩阵中把该场景映射到多个真实锚；测试注释只声明实际断言的范围。

### 一般｜路径与身份域缺少设计指定的属性测试

- 置信度：确认
- 位置：`slterm_path/Cargo.toml`、`slterm_path/src/policy/policy_tests.rs`、`slterm_path/src/app_paths.rs`；`docs/devPlan/design/testing.md` 的“路径与身份”行
- 现象：设计测试矩阵把“属性测试、真实 Windows 文件测试”列为路径与身份域的主要证据，但 `slterm_path` 没有 proptest／quickcheck 依赖，也没有 `proptest!` 用例；现有测试是手工列举的词法、包含、授权与能力并集样例。
- 根因：属性测试没有随域实现落地；现有样例足以覆盖当前已知分支，因此缺口没有暴露。
- 影响：词法路径、canonical 形式、组件边界、能力并集、链接重定向等安全边界的组合空间没有生成式覆盖；新增路径形态或 Windows 短名／UNC 变体可能绕过样例。
- 证据：`slterm_path/Cargo.toml` 只有零依赖契约；`policy_tests.rs` 的用例均为固定输入；workspace／document 域有 proptest，路径域没有。
- 优化方向：为路径词法、包含关系、能力集合与文件身份增加属性测试；至少覆盖并集幂等、只读不否决可写、包含关系组件感知与路径往返。

### 一般｜缺少反复创建／销毁后的线程、句柄与队列增长断言

- 置信度：确认
- 位置：`docs/devPlan/design/testing.md` 的“性能与资源断言”；`slterm_terminal/tests/pty_integration_tests.rs` 的 `assert_handles_return`／`bundled_conpty_resources_return_after_close_and_failed_spawn`；`slterm_app/src/background_tasks.rs` 的 `background_task_tests`；`slterm_app/src/app_shell/fs_watch.rs` 与 `watcher_service_tests`；`slterm_app/src/local_control/service_tests.rs`
- 现象：设计要求反复创建／销毁视图、PTY、watcher、Runtime 客户端和后台任务后检查线程、句柄、队列和内存不持续增长，并分别验证隐藏与销毁；当前只有 PTY 侧有真实句柄计数回归，后台任务有订阅归零与失败续跑，watcher 有关闭停报，Runtime 有活跃连接 shutdown，但没有对视图／侧栏／后台任务／Runtime 客户端做成批创建销毁后的线程、句柄、队列与内存增长断言。
- 根因：各域分别验证了“能停止”，没有统一的生命周期压力夹具；线程与句柄计数是平台相关能力，未抽成共享测试端。
- 影响：孤儿线程、句柄泄漏、队列残留或内存增长可能在单次用例中不可见；根级“线程与生命周期”硬约束缺少自动化兜底，只能依赖代码审查与人工 smoke。
- 证据：`assert_handles_return` 只覆盖真实 PTY；`unsafe GetProcessHandleCount` 仅出现在 PTY 集成测试；视图与 watcher 用例多以“无通知／无错误”判断停止，没有线程或句柄计数；设计文档把该族列为资源断言要求。
- 优化方向：在应用测试层提供线程／句柄／队列计数的共享探针，对视图、侧栏、watcher、Runtime 客户端与后台任务做成批创建销毁与隐藏／销毁对比；无法自动化的平台计数按豁免登记。

### 一般｜测试命名违反项目约定，`test_` 前缀与语义缺失并存

- 置信度：确认
- 位置：`slterm_terminal/src/tty/windows/mod.rs` 的 `test_escape`、`test_cmdline`；`slterm_terminal/src/grid/tests.rs` 的 `test_iter`；另有多个上游迁移的短名用例
- 现象：`strategy.md` 要求测试函数使用“对象_行为_场景”的裸 snake_case，明确禁止 `test_` 前缀；上述用例仍保留前缀，且名称不含对象与场景。上游迁移的 `rotate`、`indexing`、`multibyte_unicode` 等短名也不符合约定。
- 根因：上游迁移的终端测试未按本项目命名规范重命名；没有机械检查测试名。
- 影响：用例检索与追溯矩阵的可读性下降；按名称定位场景时容易误判；命名约定无法作为门禁执行。
- 证据：`test_` 前缀用例位于 `#[cfg(test)]` 模块；`strategy.md` 的“命名约定”条目；短名用例在同域测试模块中大量存在。
- 优化方向：重命名现有用例为对象_行为_场景；可在脚本门禁中增加对 `fn test_` 的拒绝规则，避免回流。

### 一般｜Git 测试依赖全局 Git 配置，缺少环境隔离

- 置信度：确认
- 位置：`slterm_app/src/git/status_tests.rs` 的 `repository_with_commit`；`slterm_app/src/git/repository_tests.rs` 的 `init_repository`；`slterm_app/src/app_shell/app_root/tests/gitshow.rs` 的 `repository`；`slterm_app/src/app_shell/app_root/tests/commit.rs` 的 `staged_and_unstaged_repository`；对照 `slterm_app/src/git_completion.rs` 的 `repository`
- 现象：Git 测试仓库只设置本地 `user.name`／`user.email`；status 与 repository 夹具没有设置 `core.autocrlf=false`，所有夹具都没有清空 `GIT_CONFIG_GLOBAL`／`GIT_CONFIG_SYSTEM`，也没有设置 `commit.gpgsign=false` 或 `core.hooksPath`。`gitshow` 夹具设置了 `core.autocrlf=false`，`git_completion` 夹具显式设置 `commit.gpgsign=false`，但仍未隔离全局配置来源。
- 根因：测试假设 CI／开发机没有全局签名、hooks 与换行转换；共享的 `crate::git::command` 只加 `--no-optional-locks` 与 `-C`，不隔离配置来源。
- 影响：启用 commit 签名、全局 hooks 或 autocrlf 的机器可能让 `git commit` 失败、挂起或改变字节内容，导致测试结果依赖开发机全局状态；这与“临时双根、无真实用户全局状态”的隔离纪律冲突。
- 证据：`repository_with_commit`、`init_repository`、`gitshow::repository` 的配置步骤；`gitshow::repository` 的 `core.autocrlf=false`；`git_completion` 的 `-c commit.gpgsign=false`；`git/mod.rs` 的 `command` 构造。
- 优化方向：在共享 Git 测试辅助中设置 `GIT_CONFIG_NOSYSTEM=1`、指向临时文件的 `GIT_CONFIG_GLOBAL`、`commit.gpgsign=false`、空 `core.hooksPath` 与 `core.autocrlf=false`；让所有 Git 夹具复用同一隔离入口。

### 一般｜CI 门禁没有机械钉住全量串行与 GPUI 特征测试步骤

- 置信度：确认
- 位置：`scripts/tests/test_architecture_governance.py` 的 `test_ci_runs_core_tests_in_release_mode`；`.github/workflows/ci.yml`
- 现象：治理测试只断言 CI 中存在 core release 的精确命令；全量 workspace 的 `--test-threads=1`、GPUI 特征的 `lib_tests` 步骤、schema 特征的 Runtime 测试都没有对应断言。CI 删掉 GPUI 步骤或去掉串行参数时，脚本自测仍全绿。
- 根因：治理测试只覆盖了最初的一个 CI 不变量；后续新增的测试层命令没有同步进门禁自测。
- 影响：承诺的“全量回归串行”“虚拟窗口全特性”可以静默退化；全量测试的并行误红风险与 UI 覆盖缺失都不会被门禁发现。
- 证据：`test_architecture_governance.py` 只拼出 release 命令并 `assertIn`；CI 中另有多条 `cargo test`／`cargo clippy` 步骤未被断言；`strategy.md` 把全量串行列为硬红线。
- 优化方向：在治理测试中列出所有必需的 CI 测试命令（全量串行、GPUI 特征、schema 特征、脚本自测）并逐条断言；把命令矩阵作为单一来源，避免文档与 CI 漂移。

### 提示｜`reporting_without_a_capture_scope_falls_back_to_stderr_without_panicking` 未验证 stderr 回退

- 置信度：确认
- 位置：`slterm_settings/src/diagnostics.rs` 的同名用例
- 现象：用例只调用 `report_unknown_field`、`report_deprecated_field`、`report_invalid_value`，没有任何 stderr 捕获或输出断言；名称中的 “falls back to stderr” 没有被观察。
- 根因：诊断报告模块没有可注入的回退接收端；测试以“不 panic”作为唯一判据。
- 影响：如果回退路径静默丢弃报告、写错级别或写错目标，用例仍然通过；诊断可观察性没有回归保护。
- 证据：测试体只有调用、没有 stderr 观察；没有 `gag`／管道／自定义 writer。
- 优化方向：为诊断回退提供可注入的 sink，或在子进程／管道中捕获 stderr 并断言字段与级别。

### 提示｜极端滚动用例无显式断言，仅以不 panic 作为判据

- 置信度：确认
- 位置：`slterm_app/src/app_shell/text_view/editor_tests.rs` 的 `extreme_restored_scroll_does_not_overflow_editor_painting_or_hit_testing`；`slterm_app/src/app_shell/text_view/paged_tests.rs` 的 `extreme_restored_scroll_does_not_overflow_paged_hit_testing`
- 现象：这些用例只做视图构造、绘制或刷新，并调用一次 `view.byte_for_point` 后丢弃返回值；没有断言命中点、布局矩形或滚动偏移在有限范围内。
- 根因：属性是“不溢出／不 panic”，在虚拟窗口中缺少可断言的几何边界。
- 影响：命中测试返回错误坐标、布局变成 NaN 或滚动计算错误但未 panic 时，用例仍然通过；只能防止崩溃，不能防止语义回归。
- 证据：这些用例体都无 `assert`；`byte_for_point` 返回值被 `let _ =` 丢弃。
- 优化方向：断言返回字节位于文档范围、布局边界的宽高有限且非负、命中点映射在预期行附近；把“不 panic”降为附带属性。

### 提示｜测试专用生产分支的覆盖边界未登记

- 置信度：确认
- 位置：`slterm_app/src/platform/machine.rs` 的 `machine_key`／`host_name`；`slterm_app/src/platform/startup/first_frame.rs` 的 `present_then_show`；对照 `.claude/docs/testing/exemptions.md`
- 现象：`machine_key` 在 `cfg(test)` 下固定返回 `"test-machine"`，真实 `GetComputerNameExW` 路径在 `cfg(all(windows, not(test)))` 下被排除，`machine_key_is_stable_within_the_test_build` 断言的是这个测试分支。`present_then_show` 在虚拟窗口没有原生 HWND 时，`cfg(test)` 直接调用 `on_ready()`，真实 `WM_PAINT` 同步与 `activate_window` 路径没有自动化覆盖。豁免表没有以这些符号或启动呈现路径单独登记。
- 根因：真实主机名与原生窗口呈现缺少可注入端口；测试构建用固定值／短路维持虚拟窗口用例。
- 影响：测试可以通过测试专用分支全绿，而生产分支（主机身份解析、present-then-show）只在人工 smoke 中覆盖；覆盖边界没有被显式登记，后续容易被误认为已自动化。
- 证据：`machine_key` 的 `#[cfg(test)]` 返回值与 `host_name` 的 `not(test)` 门控；`present_then_show` 的无 HWND 分支；豁免表没有以这些符号单独登记。
- 优化方向：为主机名与呈现操作提供可注入端口，让单元层验证真实组合；若确属真实桌面边界，按豁免规则登记并写明构建产物人工核验步骤。

### 提示｜资产预览集合硬编码，新增单色资产不会触发校验失败

- 置信度：确认
- 位置：`scripts/generate-asset-previews.ps1` 的 `$cliPreviewNames`、`Get-ExpectedPreviewFiles`；`docs/featureSpec/assets/README.md`
- 现象：预览源集合是脚本内固定的 CLI 名称列表；`Get-ExpectedPreviewFiles` 只遍历该列表，`-Check` 只比较预期集合与实际集合。新增一个单色 CLI SVG 时，若没有同时手工加入列表，预览不会被要求生成，`-Check` 仍然通过。
- 根因：预览资格没有从资产清单／源目录元数据推导；README 也声明不手工镜像完整资产清单。
- 影响：文档预览可能与资产集合漂移；新增 CLI 在文档中缺预览而 CI 无感。
- 证据：脚本中的固定数组；`package-release.tests.ps1` 只检查所有 `cli-svg/*.svg` 进包，不检查每个单色资产有预览；CI 仅执行 `-Check`。
- 优化方向：用显式清单或可判定的元数据（颜色空间／单色标记）驱动预览集合；`-Check` 同时断言“每个应有预览的源资产都存在对应预览，或列在显式排除表”。

### 提示｜i18n contract 目标为隐式自动发现，相关注释已过时

- 置信度：确认
- 位置：`slterm_app/Cargo.toml`（无 `[[test]] name = "i18n_contract"`）；`slterm_app/tests/i18n_contract.rs` 的 payload 预算用例注释；`tools/slterm-i18n-contract/Cargo.toml`
- 现象：`i18n_contract` 测试 target 未在清单中声明，依赖 Cargo 自动发现；payload 预算用例注释称“合同未被 workspace 全量命令覆盖到”，但根 workspace 的测试目标里实际存在自动发现的 `i18n_contract` 构建产物。独立 `tools/slterm-i18n-contract` workspace 没有进入 CI，`catalog_builder` 的负例测试在 CI 中只因为被自动 target 一并编译才运行。
- 根因：测试从应用 crate 迁到独立工具 workspace 时，清单与注释没有同步；显式 target 纪律没有覆盖这个文件。
- 影响：当前覆盖存在，但依赖 Cargo 自动发现这一隐式行为；一旦加入 `autotests = false` 或调整目录，i18n 合同与构建器负例会静默消失。注释中的过时事实会误导后续核销。
- 证据：`slterm_app/Cargo.toml` 的显式 target 只有 `lib_tests` 与 `windows_console_startup`，没有 `i18n_contract`；根 `target/debug/deps` 存在 `i18n_contract` 构建产物；`tools/slterm-i18n-contract` 有自己的 `[workspace]` 且未被 CI 调用。
- 优化方向：为 `i18n_contract` 增加显式 `[[test]]` 声明；删除或更新过时注释；决定是否把 tools workspace 纳入 CI，或删除重复的 lib target。

### 提示｜崩溃子进程夹具在普通测试列表中是无断言空跑

- 置信度：确认
- 位置：`slterm_app/src/app_shell/crash_recovery.rs` 的 `crash_child_close_commit_boundary`
- 现象：该 `#[test]` 在没有 `SLTERM_TEST_CRASH_ROOT` 时直接 `return`，没有任何断言或行为；父测试 `close_commit_crash_reopens_with_exact_persistent_stage` 才会以子进程形式驱动它。
- 根因：Rust 子进程夹具必须保留为测试目标，才能在 `--exact` 下被父进程调用。
- 影响：全量运行时会留下永远通过的测试条目；如果只按名称筛选父测试或子夹具没有被父测试调用，子夹具仍会给出假绿，掩盖夹具未真正执行。
- 证据：函数体开头的 `let Some(root) = env::var_os(ROOT_ENV) else { return; }`；父测试的 `Command::new(current_exe)` 调用。
- 优化方向：在夹具被独立运行时显式失败或打印并标记为辅助；或在用例名／注释中说明它只能由父测试驱动，并让父测试的存在性由门禁或文档核销。

## 跨域观察

- `docs/devPlan/progress.md` 的验收行写入具体通过测试数（含 GPUI、core release 与脚本门禁的计数），与 `.claude/CLAUDE.md` 的“快照数字禁令”冲突；测试数可由运行日志自证，不应固化在进度文档。证据：`progress.md` 的 Agent 与 Hooks、全量验收等行；文档规范条目“快照数字禁令”。
- 工作树内存在未跟踪的旧项目目录 `.claude/worktrees/agent-*`，包含另一套 `tests`、`CLAUDE.md` 与历史脚本；它未入 git，但会污染本地全文检索、代码审查与工具遍历，建议清理或移出工作树。
- `assets/runtime-control-api.md` 给出了 Runtime schema 的定向测试命令，但 `.claude/docs/operations/commands.md` 没有收录，形成“资产文档有命令、命令矩阵无门禁”的第二真值源。

## 存疑未证

- 当前 `assets/runtime-api-v2.schema.json` 是否已经与生成器漂移：未运行 `runtime-schema` 或 schema 测试；可确认的是没有自动门禁能发现漂移。
- AppRoot 测试是否在所有路径上都实际执行了真实 Agent 目录枚举：home 探测路径已确认；目录读取取决于 AgentFiles 是否打开与后台任务完成时机，未运行验证。
- 全局 Git 配置对当前开发机／CI 的实际影响：代码未隔离已确认；具体签名、hooks 或 autocrlf 是否会触发失败未实测。
- 安装器夹具 GUID／AppId 当前是否与生产隔离正确：未构建或解析安装器；可确认的是没有自动测试覆盖。
- `windows-runtime.tests.ps1` 在无归档且需要网络时的行为：脚本会通过 `prepare-windows-runtime.ps1` 下载固定 NuGet 包；未在隔离环境运行。
- `cargo test --workspace` 是否在所有环境都实际运行自动发现的 `i18n_contract` target：根 target 目录存在构建产物，推断已运行；未执行命令确认。
