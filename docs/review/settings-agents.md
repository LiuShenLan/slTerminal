# 设置、Agent 域与历史隔离审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`；只审当前工作树，不运行构建、测试或网络。

## 覆盖与局限

- 已读根 `CLAUDE.md`、`slterm_settings/CLAUDE.md`、`slterm_agents/CLAUDE.md`、`slterm_hooks/CLAUDE.md`、`slterm_app/CLAUDE.md`、`app_shell/CLAUDE.md`、`app_root/CLAUDE.md`、`persistence/CLAUDE.md`、`local_control/CLAUDE.md`、`plan_balance/CLAUDE.md`、`theme_library/CLAUDE.md`、`startup/CLAUDE.md`。
- 已读 `docs/devPlan/design/settings-recovery.md`、`agents.md`、`domain-model.md`、`hooks.md`、`architecture.md`；`docs/featureSpec/settings-and-persistence.md`、`workspace/navigation.md`、`sidebar/agent-files.md`、`panels/settings.md`、`cross-cutting.md`、`acceptance.md`；`docs/devPlan/traceability.md` 与 `progress.md` 的设置、Agent、导航、隔离相关章节。
- 已读 `slterm_settings/src` 全部模块；`slterm_agents/src` 全部模块；`slterm_hooks/src`；`slterm_agent_detection` 加载器与部分规则文件；`slterm_app` 中 `settings_store`、`settings_state`、`settings_transfer`、`secrets`、`ai_agents`、`ai_sessions`、`agent_dirs`、`agent_env`、`agent_host`、`agent_root_service`、`history`、`history_quarantine`、`theme_library`、`plan_balance` 及 AppRoot 的 settings、nav、agent files、runtime agent 接线；相关内联测试与 GPUI 测试。
- 未逐条复核 `slterm_agent_detection` 全部正则的语义正确性；未覆盖加密备份密码学、Hooks 编辑页完整渲染、Runtime 线缆细节；这些不属于本范围。
- `slterm_app/src/history.rs` 是 fish 式命令历史，不是导航历史；导航历史/隔离的审查以 `navigation.rs`、`nav.rs`、`history_quarantine.rs`、`ai_sessions.rs` 与 AppRoot 接线为准。
- 按合同未运行 cargo、测试、脚本或 GUI，不读取真实用户数据目录；“确认”均指已读完整代码路径，不指运行时验证。

## 问题

### 阻断｜导航历史、活跃会话与恢复流程没有生产实现

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/nav.rs` 的 `NavRuntime` / `render_nav` / `nav_set_projects`；`slterm_app/src/app_shell/app_root/navigation.rs` 的 `deepest_project_for_cwd`、`resolve_restore_cwd`、`restore_injection_timeout`、`resolve_restore_project`；`docs/featureSpec/workspace/navigation.md` 的「活跃会话」「历史会话」「删除隔离区与撤销」「搜索」「刷新与失败」；`docs/devPlan/traceability.md` 的「导航树」表。
- 现象：导航侧栏只渲染项目行与移除按钮，没有活跃会话行、历史会话节点、未归属节点、历史搜索、多选、恢复、分支、删除、状态徽标或套餐页脚。历史归属与恢复判定全部是带 `#[allow(dead_code)]` 的纯函数，注释明确写“消费点归导航历史视图/恢复任务”，生产没有调用者。`slterm_agents` 不导出 `HistoryRecordKey`，`HistoryAdapter` 没有版本，`ai_sessions::scan` 在自身模块之外无调用者。
- 根因：历史扫描、记录身份、导航视图、恢复任务与删除隔离被留作后续接线，但追溯矩阵与进度已把它们记为自动化验收。
- 影响：用户无法看到或恢复 Claude/Codex 原生历史，无法区分活跃会话，无法搜索或删除历史；`traceability.md` 中 `navigation_history_lexical_deepest_match`、`navigation_unowned_restore_requires_project_confirmation`、`navigation_restore_fallback_and_injection_retry` 等“已自动化”只验证纯判定，不验证任何用户可观察行为。`progress.md` 的「Agent 与 Hooks」行声称历史扫描／恢复已全量自动化，与代码事实不符。
- 证据：`nav.rs` 的 `NavRuntime` 只有项目列表；`navigation.rs` 中恢复相关函数全部未被生产引用；`slterm_agents/src/lib.rs` 的导出清单没有历史记录身份；`ai_sessions.rs` 没有调用点；`agent_host.rs` 的 `AgentHub::observe_screen` 与 `ai_agents` 的屏幕识别同样未接线；`traceability.md` 导航表逐项指向这些纯函数用例。
- 优化方向：把导航视图、历史适配器、记录身份、恢复任务和删除入口作为同一交付面接通；追溯矩阵只登记端到端行为用例，纯判定用例不能支撑“活跃会话/历史会话”验收。

### 阻断｜历史隔离区没有执行面，启动维护只整体阻断

- 置信度：确认
- 位置：`slterm_app/src/history_quarantine.rs` 全部；`slterm_app/src/persistence/startup/maintenance.rs` 的 `MaintenanceRunner::run` 与 `MaintenanceStage::HistoryQuarantine`；`slterm_app/src/persistence/workspace.rs` 的 `quarantine_batches` / `quarantine_items`；`docs/featureSpec/workspace/navigation.md` 的「删除隔离区与撤销」；`docs/devPlan/design/agents.md` 的「历史隔离批次」。
- 现象：隔离批次只有内存状态机与文件复制/撤销函数，生产没有删除入口、撤销入口、到期清理、ZIP 导出或永久删除路径；quarantine 表也没有业务写入或读取。`history_quarantine.rs` 头注释明确说文件复制、来源删除与启动对账“由后续接线消费”。启动维护只要发现隔离表有任意记录就返回 `NeedsAttention { stage: HistoryQuarantine }`，应用进入只读诊断分支，不执行来源/副本对账，也不继续展示普通历史。
- 根因：隔离区被实现为“测试可用的纯状态机”，而不是“导航删除操作的持久事务链”；启动维护把隔离材料当作未知材料统一阻断，与“启动先对账、完整隔离项继续隐藏、未完成项保留或恢复来源”的规格相反。
- 影响：历史删除与撤销在真实应用中不可用；一旦未来按现有隔离函数写入数据库，下一次启动将被整体阻断，而不是恢复或继续隐藏。`quarantine_batches_have_independent_deadlines`、`quarantine_restart_preserves_undo_window`、`quarantine_startup_reconciles_incomplete_batch` 等“已自动化”只验证状态机或内存事实，不覆盖真实文件、SQLite 或重启。
- 证据：生产代码只在 `navigation.rs` 的测试中引用 `HistoryQuarantine`；`quarantine_batch` 与 `restore_quarantine_batch` 没有生产调用点；维护器对隔离表只做 `EXISTS` 查询；`workspace.rs` 只建表，没有业务 SQL；`traceability.md` 导航表仍标“已自动化”。
- 优化方向：把删除、撤销、到期、导出、永久删除与启动对账接到同一持久批次清单；启动维护应注入隔离恢复器并读取清单后对账，而不是把隔离材料升格为启动阻断。

### 阻断｜普通设置导入、导出与恢复默认只有内存纯函数

- 置信度：确认
- 位置：`slterm_app/src/settings_transfer.rs` 的 `SettingsExport`、`export_settings`、`import_settings`；`slterm_settings/src/reset.rs` 的 `restore_default_settings_at`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的设置页入口；`docs/featureSpec/settings-and-persistence.md` 的「设置导入、导出与恢复默认」；`docs/devPlan/traceability.md` 的设置与持久化行。
- 现象：普通设置导出只是把当前 `RawSettings` 中的白名单键投影成内存 `BTreeMap`，没有文件格式、序列化、原子写、系统文件选择器、导入预览、确认、上一代有效备份或导入提交。`restore_default_settings_at` 只被自身测试与 `settings_transfer` 测试调用；生产设置页没有恢复默认入口。`SettingsStore::save_explicit` 同样只在测试中使用。
- 根因：实现了纯投影与校验函数，却没有所有者的冷路径文件读、写与 UI 流程；追溯矩阵把纯函数用例当作行为验收。
- 影响：用户无法导入、导出或恢复默认设置。`settings_import_rejects_unknown_or_credentials` 与 `settings_import_and_reset_preserve_secrets` 虽然通过，但只覆盖内存对象；规格要求的“任一预检失败零写入、保留上一代有效备份、原子提交、预览影响域”没有实现。
- 证据：`settings_transfer` 只被 `lib.rs` 声明，无其它生产引用；`settings_overlay` 的字段提交路径只有 `apply_settings_field` 与 `poll_settings_save`，没有文件 IO；`SettingsExport` 没有 serde 序列化或反序列化实现；`progress.md` 将设置主题阶段标为已验收。
- 优化方向：为普通导入/导出建立独立的文件信封与显式动作链，接入系统文件选择器、完整校验、预览确认、备份与原子提交；恢复默认应经同一显式动作链，而不是公开一个核心直写 API 后无 UI。

### 阻断｜Agent 基础配置、AgentRoot 管理与 Agent 文件操作面未实现

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/agent_files.rs` 的 `AgentFilesRuntime`、`agent_files_roots`、`agent_root_grants_for_path`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的静态字段表；`slterm_app/src/agent_root_service.rs` 的 `AgentRootRegistry`；`slterm_app/src/agent_dirs.rs`；`docs/featureSpec/panels/settings.md` 的「Agent 基础配置页」「AgentRoot 管理」；`docs/featureSpec/sidebar/agent-files.md`。
- 现象：设置中心没有 Agent 基础配置页，没有 CLI 别名编辑、展示模式、自定义名单或运行时文件清单；没有 AgentRoot 添加、改名、改路径、删除或能力开关。生产只在首次打开文件树时读取原始自定义根列表，内置根与自定义根直接拼成一棵树；`AgentRootRegistry`（真实目录查重、改路径撤销、能力并集）没有生产调用者。Agent 文件视图本身只有展开、选择、搜索过滤、打开 Document 与读取目录，没有新建、重命名、删除、复制、剪切、粘贴、在终端中打开或过滤配置。规格禁止的盘符根、Windows 目录、System32 校验也不存在。
- 根因：AgentRoot 与 Agent 文件被实现为展示层加核心纯函数，设置编辑面、文件操作面和真实路径查重被测试注入端口取代。
- 影响：用户无法添加或管理自定义 AgentRoot，无法为根开启写入或终端 cwd，无法在 Agent 树中执行文件操作；`agent_root_duplicate_and_revocation`、`agent_files_default_capabilities`、`agent_files_cross_root_copy_and_cut` 等用例只验证服务或核心剪贴板，不验证生产入口。`progress.md` 的「Agent 与 Hooks」行声称 Agent 文件树全量自动化，与代码事实不符。
- 证据：`settings_controls` 的字段集合只来自 `SettingsRegistry` 的标量键，没有 AgentRoot 条目；`save_agent_roots` 只有测试调用；`AppRoot` 仅在 `ensure_agent_roots_loading` 中 `load_agent_roots` 并把结果交给 `custom_agent_roots`；Agent 文件测试大量使用 `replace_custom_agent_roots_for_test`；文件视图没有 `FileOperationService` 或剪贴板引用。
- 优化方向：把 Agent 基础配置页与 AgentRoot 管理作为设置中心的正式编辑面，所有增删改都经同一个真实路径解析与授权服务；Agent 文件操作复用 Explorer 的 `FileOperationService` 与剪贴板，不以测试注入替代生产接线。

### 重要｜真实目录查重漏掉内置根，生产路径绕过 Registry

- 置信度：确认
- 位置：`slterm_settings/src/agent_roots.rs` 的 `AgentRoot` 与 `agent_roots_from_segment`；`slterm_app/src/agent_root_service.rs` 的 `AgentRootRegistry::register`、`add_builtin`、`change_path`；`slterm_app/src/app_shell/app_root/agent_files.rs` 的 `agent_files_roots` 与 `agent_root_grants_for_path`；`docs/featureSpec/sidebar/agent-files.md` 的「节点与能力」；`docs/featureSpec/panels/settings.md` 的「AgentRoot 管理」。
- 现象：`register` 只对既有自定义根做真实目录查重，不检查 `builtin` 根集合；`add_builtin` 只对自定义根查重，不检查已登记内置根。`change_path` 反而检查 `builtin`，说明规则本应覆盖内置根。生产不使用 Registry：`agent_files_roots` 直接把内置目录与原始自定义根拼接，`agent_root_grants_for_path` 对每个词法前缀命中的根分别授权，可能在同一直实目录上形成两个节点或多份授权。`load_agent_roots` 的错误在 AppRoot 被 `unwrap_or_default()` 吞掉，段结构损坏或任一元素非法会让全部自定义根静默消失。`agent_roots_from_segment` 本身也不校验身份唯一性。
- 根因：查重与解码规则分散在“服务 API”与“生产加载路径”两处，生产路径只读取值模型，不经过服务。
- 影响：违反“同一真实目录只允许一个 AgentRoot，包括内置根与不同词法别名”；用户可以手写配置制造重复根，或通过链接别名绕过查重；损坏配置被当成空配置，表现为根与授权静默消失。
- 证据：`register` 的重复检查只遍历 `self.roots`；`add_builtin` 只检查 `self.roots`；`agent_files_roots` 直接 `extend` 自定义列表；`ensure_agent_roots_loading` 对 `load_agent_roots` 使用 `unwrap_or_default`；`agent_files_roots` 对无效词法路径 `filter_map` 静默跳过。
- 优化方向：让生产加载与注册共用同一真实路径服务，并把内置根纳入查重与能力计算；段解码错误必须作为显式错误态保留，不得降级为空根集合。

### 重要｜改路径沿用旧根能力，违背“不把旧路径能力延续到不同目录”

- 置信度：确认
- 位置：`slterm_settings/src/agent_roots.rs` 的 `AgentRoot::with_path`；`slterm_app/src/agent_root_service.rs` 的 `AgentRootRegistry::change_path`；测试 `agent_root_path_change_service_semantics`；`docs/featureSpec/sidebar/agent-files.md` 的「节点与能力」。
- 现象：`with_path` 克隆 `display_name` 与 `capabilities`，`change_path` 接受调用方传入的新 `AgentRoot` 并直接替换；服务测试明确断言“新 id 继承能力”，把可写能力从旧目录带到新目录。
- 根因：改路径被建模为“同能力换目录”，而规格要求“撤销旧根＋登记新根”，新根应重新从默认只读开始或至少重新取得用户显式授权。
- 影响：一次改路径可能把写入或终端 cwd 授权静默扩大到另一个真实目录；若未来接线直接复用该服务，这是权限扩张而不是配置变更。
- 证据：`with_path` 返回 `capabilities: self.capabilities`；`change_path` 不重置 `new_root.capabilities()`；测试 `agent_root_path_change_service_semantics` 断言新根 `allows_write()`。
- 优化方向：新根按默认只读登记，能力必须由用户在新路径上重新显式开启；旧根撤销与新根登记分两步提交，失败保留旧根。

### 重要｜`agent.start` 注入 slug 而不是 AgentCatalog 声明的内置命令

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `RuntimeMutation::AgentStart` 与 `start_agent_terminal`；`slterm_agents/src/catalog.rs` 的 `AgentCatalogEntry::builtin_command`；测试 `runtime_agent_start_resume_and_fork_create_terminals`。
- 现象：`AgentStart` 解析出 `AgentKind` 后把 `kind.slug().to_owned()` 交给 `start_agent_terminal`；`AgentCatalogEntry.builtin_command` 除 catalog 自身测试外没有消费者。`Cursor` 的 slug 是 `cursor`，catalog 声明的内置命令是 `agent`；`Antigravity` 的 slug 是 `antigravity`，catalog 声明的是 `agy`。两者注入的入口名与目录声明不一致。
- 根因：启动命令没有从 AgentCatalog 取得，而是把跨域身份 slug 当成可执行名；测试只验证终端数量，不验证注入命令内容。
- 影响：对 Cursor/Antigravity 等 slug 与入口名不同的 CLI，`agent.start` 会注入错误命令，终端创建成功但 CLI 不启动；AgentCatalog 的“唯一命令事实”被实际绕开。
- 证据：`start_agent_terminal` 的唯一调用方向是 `kind.slug()`；catalog 测试 `catalog_builtin_command_is_declared_alias` 只验证内置命令属于别名；`runtime_agent_start_resume_and_fork_create_terminals` 只对 `claude` 调用 start 并断言终端计数，没有断言注入文本。
- 优化方向：start 命令只能从 AgentCatalog 的内置命令构造，并对每个 AgentKind 增加命令内容断言；slug 只作身份，不作可执行名。

### 重要｜Claude 无原生会话 ID 时仍生成 `--continue`，恢复身份不成立

- 置信度：确认
- 位置：`slterm_agents/src/session.rs` 的 `AgentSession::resume_command`；`slterm_app/src/agent_host.rs` 的 `AgentHub::session_agent`；`slterm_app/src/app_shell/app_root/commands.rs` 与 `terminal_events.rs` 的恢复/快照接线；测试 `resume_commands_require_catalog_capability_and_safe_identity`。
- 现象：没有 hook 会话身份的 pane，快照会生成 `AgentSession { source, session_id: None, session_file: None }`；`resume_command` 对 Claude 返回 `claude --continue`。该命令恢复 cwd 下最近一次会话，不是被快照的那次会话；多个 Claude pane 在同一 cwd 重启时会争用同一“最近会话”。
- 根因：把“没有可验证身份”降级成了“恢复最近会话”，与“未知来源、无恢复能力或异形身份一律不生成命令”的身份纪律相反；测试把该降级写成期望。
- 影响：重启后 Claude 面板可能接续错误对话；多 pane 场景尤其明显，且用户无法从界面判断恢复了哪一次会话。
- 证据：`session_agent` 在无 hook 身份时用程序名构造无 ID 的 `AgentSession`；`resume_command` 的 `None if agent == AgentKind::Claude` 分支返回 `claude --continue`；测试 `resume_commands_require_catalog_capability_and_safe_identity` 断言这一行为；`schedule_agent_resume` 直接消费 `record.agent()`。
- 优化方向：无原生 ID 的会话不生成恢复命令，或改为显式的用户选择；`--continue` 只能作为独立命令，不得冒充基于会话身份的恢复。

### 重要｜撤销状态在恢复成功前被置为 Undone，失败不可重试且状态不持久

- 置信度：确认
- 位置：`slterm_app/src/history_quarantine.rs` 的 `HistoryQuarantine::try_begin_undo`、`complete_batch`、`from_persisted`、`persisted_windows`；`restore_quarantine_batch`；`docs/featureSpec/workspace/navigation.md` 的「删除隔离区与撤销」；`docs/devPlan/design/agents.md` 的「历史隔离批次」。
- 现象：`try_begin_undo` 在返回 `Allowed` 前就把批次状态改成 `Undone`；真正的文件恢复在另一个函数中进行，可以返回 `Blocked` 或 `Err`。没有任何 API 把状态改回 `Completed` 或 `NeedsAttention`，因此撤销预检失败或恢复异常后再次点击会得到 `AlreadyUndone`，与“保留完整隔离数据、标记待处理并提供重试”不符。`persisted_windows` 只保存批次身份与完成时间，`from_persisted` 把所有带完成时间的批次一律恢复为 `Completed`，已撤销或待处理状态在重启后丢失。
- 根因：撤销被拆成“状态转换”和“文件副作用”两个独立步骤，状态机没有“执行中/失败待重试”状态，持久 DTO 也不携带状态或错误。
- 影响：撤销失败会伪装成已撤销；用户无法重试；重启后待处理批次可能重新出现在可撤销集合中，而普通历史列表仍隐藏来源，形成状态不一致。
- 证据：`try_begin_undo` 在检查期限后直接写 `BatchStatus::Undone`；`restore_quarantine_batch` 是独立纯文件函数；`BatchStatus` 没有失败态；`persisted_windows` 的返回类型只有 `(id, Option<SystemTime>)`；测试 `quarantine_undo_rechecks_deadline` 只覆盖 Allowed/Expired/AlreadyUndone，不覆盖恢复失败后的重试。
- 优化方向：把状态推进与文件恢复合并成可回滚事务，失败进入显式待处理态并保留重试；持久事实必须包含状态、错误类别与批次清单版本，而不是只有完成时间。

### 重要｜`quarantine_batch` 不先持久化清单，部分失败会丢失材料身份

- 置信度：确认
- 位置：`slterm_app/src/history_quarantine.rs` 的 `quarantine_batch`、`QuarantineBatchMaterial`、`QuarantineEntry`；`docs/featureSpec/workspace/navigation.md` 的「删除隔离区与撤销」；`docs/devPlan/design/agents.md` 的「历史隔离批次」。
- 现象：函数先逐个读取来源、写副本、校验副本，再复核来源指纹并逐个删除来源；只有全部成功后才返回材料对象。任何一步失败都以 `Err` 返回，已写入的副本、已删除的来源以及逐项失败原因都不在返回材料中，也没有先落盘的批次清单。`QuarantineEntry` 只保存来源、副本与一个指纹，没有原始位置、列表顺序、来源身份或前后指纹。
- 根因：把“隔离批次”实现成一次性内存函数，没有按规格先持久化清单，也没有把部分失败建模为批次事实。
- 影响：若接入生产，来源删除中途失败会导致部分来源已消失、部分仍在，但没有可恢复的批次清单；已写入副本成为孤儿；重启对账无法知道哪些项属于该批、顺序如何、失败原因是什么。
- 证据：`quarantine_batch` 的 `entries` 是局部向量，失败路径不返回它；来源删除循环在第一个 `remove_file` 错误处 `?` 直接返回；`QuarantineEntry` 字段不含顺序、原始位置或失败状态；函数参数只有 `(source, fingerprint)`。
- 优化方向：先写入并校验不可变批次清单，再逐项复制与删除；部分失败必须返回逐项结果与已产生材料，批次进入待处理态，供撤销、重试、导出与启动对账消费。

### 重要｜主题值模型夹带排版、布局、效果与媒体，并写回用户设置

- 置信度：确认
- 位置：`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition`、`ThemeTypography`、`ThemeLayout`、`ThemeEffects`；`slterm_app/src/theme_library/document.rs` 的 `from_definition`、`definition`；`slterm_app/src/theme_library/preferences.rs` 的 `custom_theme_updates`；`docs/devPlan/design/settings-recovery.md` 的「主题与 ANSI」；`slterm_settings/CLAUDE.md` 的「主题语义合同」；`docs/featureSpec/cross-cutting.md` 的「主题」。
- 现象：`ThemeDefinition` 除颜色外还包含字体族、字号、行高、字距、字重、连字、UI 字体、卡片圆角/卡缝/投影/分界线、内边距、透明度、模糊材质、压暗层、光标形状、壁纸路径、壁纸 fit/alignment/铺满 flag。主题文档把这些字段序列化；应用主题时 `custom_theme_updates` 把它们写入 `font_family`、`font_size`、`pane_card_*`、`opacity`、`blur`、`cursor_shape`、`background_image*` 等用户设置。`ThemeEffects::background_image` 接受任意本地或 UNC 路径，仅拒绝控制字符，随后由壁纸加载器读取。
- 根因：主题模型把“外观预设”与“颜色主题”混在一起，应用桥进一步把非颜色字段投影到设置，违反了主题只承载语义颜色与 ANSI 映射的边界。
- 影响：导入一个主题会静默改变用户的字体、卡几何、透明度、材质、光标和壁纸，而不是只改变配色；主题文件成为任意本地/网络图片引用的载体，可能触发非预期文件读取或 SMB 网络访问，违反“危险外部引用一律拒绝”。`slterm_settings` 内另有一套只含颜色的 `ThemeTokens`（含重复字段拒绝）却未被生产使用，说明值权威存在双模型。
- 证据：`ThemeDefinition` 字段与 `from_definition` 的 `typography`/`layout`/`effects` 段；`custom_theme_updates` 逐项写入非颜色设置；`validate_effects` 只做长度/控制字符/枚举校验；`wallpaper` 直接把 `settings.background_image` 交给图片加载器；`ThemeTokens` 无生产消费者。
- 优化方向：主题值模型收敛为完整语义颜色 token 与 ANSI 16 映射；字体、几何、材质、壁纸与效果继续由用户设置拥有，不在主题导入/应用路径中改写；主题文档不得携带外部资源引用。

### 重要｜主题身份不是 ThemeId/UUIDv7，导入冲突被自动改名而非拒绝

- 置信度：确认
- 位置：`slterm_app/src/theme_library/document.rs` 的 `ThemeDocument::id`、`validate_id`、`with_id_and_revision`；`slterm_app/src/theme_library/store.rs` 的 `unique_id`、`unique_name`、`import_internal`；`slterm_settings/src/identity.rs` 的 `ThemeId`；测试 `names_and_ids_are_disambiguated_on_import`、`json_file_import_uses_the_existing_store_path`；`docs/devPlan/design/settings-recovery.md` 的「主题与 ANSI」；`docs/devPlan/design/domain-model.md` 的「身份类型」。
- 现象：自定义主题 id 是显示名派生的 slug（测试期望 `my-nord`、`nord-2`、`external`），不是 UUIDv7；`ThemeId` 类型只存在于设置 crate 的身份测试，没有用于主题文档或存储。`ThemeDefinition` 没有 `theme_id` 或版本字段，只有文档信封的 `schema_version` 与 `revision`。导入时同名/同 id 不拒绝，而是自动生成 `Nord (2)`、`nord-3` 并创建新主题。
- 根因：主题身份被当成文件名 slug，而不是跨重启、不可变、与显示名无关的稳定身份；导入被设计成“总是新增”，没有冲突拒绝语义。
- 影响：主题身份随名称生成，重名/改名/导入会产生不可预测的 id 与重复主题；无法用稳定身份做引用、去重、冲突检测或跨机器恢复；规格要求的“导入冲突拒绝”未实现，用户会得到静默复制而不是明确冲突。
- 证据：`validate_id` 只允许字母数字、`_`、`-`；`unique_id` 从 `slug(name)` 派生并追加序号；测试 `names_and_ids_are_disambiguated_on_import` 断言 `Nord (2)`/`nord-3`；`ThemeDefinition` 无 id/version 字段；`ThemeId` 只在 `identity.rs` 测试中使用。
- 优化方向：主题文档使用 application 注入的 UUIDv7 `ThemeId`，显示名与身份分离；导入遇到相同身份/冲突时明确拒绝并零写入，正常导入不自动启用。

### 重要｜自定义主题的选择、导入、重命名与删除没有生产入口

- 置信度：确认
- 位置：`slterm_app/src/theme_library/store.rs` 的 `fork_builtin`、`import`、`delete`；`slterm_app/src/theme_library/preferences.rs` 全模块；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的主题字段；测试 `theme_import_complete_or_zero_write`、`integration_custom_theme`；`docs/featureSpec/panels/settings.md` 的「主题与界面」。
- 现象：主题库已经实现条件保存、导入、fork、删除与 revision 前置，但这些方法在 `store.rs` 之外没有生产调用者；`preferences.rs` 以 `#![allow(dead_code)]` 声明 `custom_theme_updates` 等桥接 API。设置页的 `theme` 控件只列出内置目录，没有自定义主题列表、导入、改名、删除或当前主题更换入口。`resolve_theme_deletion` 只在测试中被调用；`ThemeLibraryStore::delete` 本身也不知道当前启用主题。
- 根因：主题库被实现为独立存储层，但设置页没有接入其管理面；删除当前使用主题的“必须先另选”只是纯判定函数。
- 影响：用户无法导入或选择自定义主题，无法改名或删除；删除使用中主题的约束没有执行点，未来接入若直接调用 `delete` 会绕过约束。`theme_import_complete_or_zero_write` 只验证 store，不验证设置页行为。
- 证据：`import`/`fork_builtin`/`delete` 除 `store.rs` 测试外无调用；`preferences` 模块头有 `#![allow(dead_code)]`；`settings_controls` 的 `theme` 选项来自 `ThemeName::BUILTIN_NAMES`；`resolve_theme_deletion` 只出现在测试。
- 优化方向：设置页提供自定义主题列表与管理动作，所有选择/改名/删除都经主题库与设置写通道；使用中删除必须先完成另选并提交，删除入口不得直接调用存储删除。

### 重要｜ANSI 16–255 被持久化成第二份色表

- 置信度：确认
- 位置：`slterm_settings/src/custom_theme.rs` 的 `IndexedPalette`、`TerminalThemeColors::palette`；`slterm_app/src/theme_library/document.rs` 的 `from_definition`（`terminal.indexed`）与 `definition`（读取 `indexed`）；测试 `integration_custom_theme`、`indexed_palette_is_complete_and_ansi_is_mirrored`；`docs/devPlan/design/settings-recovery.md` 的「主题与 ANSI」；`slterm_settings/CLAUDE.md` 的「主题语义合同」。
- 现象：`IndexedPalette` 保存 256 色，主题文档把 16–255 逐项写成 `terminal.indexed` 对象并允许导入覆盖；转换回运行时后又逐项写回。`integration_custom_theme` 明确把 `palette.colors[16]` 改成自定义值并断言渲染使用该值。
- 根因：把“派生色”当成主题数据保存，而不是按 xterm 色立方与灰阶公式在读取时生成。
- 影响：同一主题在不同版本/不同渲染器上可能出现不同 16–255 色；主题文件可以覆盖用户提示符使用的扩展色，违反“不保存第二份色表”和“16–255 只按公式派生”。这也使主题文件体积与校验面无谓扩大。
- 证据：`from_definition` 写出 16..255 的 `indexed`；`definition` 接受并应用 `indexed`；`with_indexed` 允许编辑 16..255；`custom_theme` 集成测试断言自定义索引色生效。
- 优化方向：只持久化 ANSI 0–15 与必要语义色，16–255 在读取/渲染时由单一公式派生；删除 `indexed` 持久字段与对应编辑入口。

### 重要｜设置损坏分流缺“高版本”和“权限/IO 只读重试”，纯函数测试覆盖不到

- 置信度：确认
- 位置：`slterm_app/src/settings_store.rs` 的 `LoadResult`、`load_settings_at`、`recover_corrupted`；`slterm_app/src/app_shell/config.rs` 的 `Settings::load`；`slterm_app/src/settings_state.rs` 的测试 `settings_damage_classification`；`docs/featureSpec/settings-and-persistence.md` 的「损坏与不兼容分流」；`docs/devPlan/design/settings-recovery.md` 的「SettingsRegistry 与保存队列」。
- 现象：设置文件没有版本字段或高版本识别机制；更高版本的设置文件会被当作当前版本解析，未知键被保留但已知键按当前默认/校验读取，随后仍可被自动保存重写。权限/锁/IO 读取失败在 `Settings::load` 中记录告警后回默认值，没有“只读并重试”状态。`LoadResult` 只有 `corrupted` 布尔，无法区分缺失、结构损坏、高版本、单字段非法。测试 `settings_damage_classification` 只覆盖缺失、单字段非法与解析损坏，没有权限/锁与高版本分支。
- 根因：损坏分类被压缩成“读失败就默认 + 解析失败就隔离”，缺少版本化信封与只读态；追溯矩阵把测试名当作四类分流的证据。
- 影响：高版本设置可能被旧版本静默重写；权限故障被用户感知为“设置丢失”；`settings_damage_classification` 的“权限、损坏、高版本和字段错误严格分流”验收并未成立。
- 证据：`SettingsKeyDef` 与 `settings_store` 没有版本读取；`Settings::load` 的 `Err` 分支返回默认；`LoadResult` 没有只读/重试字段；`settings_damage_classification` 的断言集合不含权限与高版本；`traceability.md` 仍标“已自动化”。
- 优化方向：为设置文档引入明确版本与高版本只读分支；把权限/锁/IO 失败映射为可重试只读态并保留原数据；测试按五类分流逐一覆盖。

### 重要｜设置写失败、损坏与单字段回退没有用户可见反馈

- 置信度：确认
- 位置：`slterm_app/src/settings_state.rs` 的 `persist_failed`；`slterm_app/src/settings_store.rs` 的 `LoadResult`；`slterm_app/src/app_shell/config.rs` 的 `Settings::load`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `initial_field_values`；`docs/featureSpec/settings-and-persistence.md` 的「应用设置与保存队列」「损坏与不兼容分流」。
- 现象：`SettingsStore::persist_failed` 只被测试读取，生产 UI 没有任何提示；`LoadResult.corrupted` 只在 `Settings::load` 中写日志；`SettingsRegistry::recover` 返回的 `Invalid` 状态在 `initial_field_values` 与设置页字段读取中被丢弃，只留下默认值。没有“本次运行生效、重启可能丢失”的提示，也没有损坏/单字段回退的聚合提示条。
- 根因：失败与回退事实停留在内部标志和日志层，没有接入设置页或壳通知；`LoadResult` 的文档注释假定“设置页据此显示警示条”，但消费点不存在。
- 影响：用户看到设置值变化或恢复默认，却不知道没有落盘或只有部分字段生效；损坏设置被静默恢复默认，违背“明确提示”要求。
- 证据：`persist_failed` 的读取点只在 `settings_state` 测试；`corrupted` 的读取点只有 `Settings::load` 的 `log::warn`；`SettingRecoveryStatus::Invalid` 没有生产消费者；`settings_overlay` 没有失败/损坏警示 UI。
- 优化方向：把写失败、隔离恢复与单字段回退聚合成设置页可见状态，并接入壳通知；提示必须区分“本次运行生效”“重启可能丢失”“已从备份恢复”“部分字段回退”。

### 重要｜恢复默认旁路统一设置写通道、审计与保存锁

- 置信度：确认
- 位置：`slterm_settings/src/reset.rs` 的 `restore_default_settings_at`；`slterm_app/src/settings_store.rs` 的 `with_save_lock`、`save_merged_locked`；测试 `external_writers_share_the_save_lock`；根 `CLAUDE.md` 的「原子写」「设置写只经 settings_store/audit 包装」；`docs/featureSpec/settings-and-persistence.md` 的「持久对象」。
- 现象：`restore_default_settings_at` 在核心 crate 内自行创建备份、临时文件并 `rename`，不经过 `settings_store` 的 `SAVE_LOCK`、`audit::write_settings`、`MAX_PERSIST_BYTES` 或保存队列；它是公开导出的 API。`settings_store::with_save_lock` 的注释要求恢复默认等路径必须包在锁内，但没有任何调用者把二者接起来。测试 `external_writers_share_the_save_lock` 只验证 `with_save_lock` 自身串行，不验证恢复默认路径。
- 根因：恢复默认被放在值权威 crate 内实现，形成与统一写通道并行的第二写入路径。
- 影响：一旦接入 UI，恢复默认会与自动保存并发读-改-写而丢失更新，绕过审计与大小上限；违反“设置写只经 settings_store/audit 包装，写核心函数私有”。
- 证据：`reset.rs` 直接使用 `fs::read`、`OpenOptions`、`fs::rename`；没有调用 `settings_store`；`restore_default_settings_at` 为 `pub` 并被 `slterm_settings/lib.rs` 重导出；`with_save_lock` 的生产调用只围绕保存通道。
- 优化方向：恢复默认只作为显式设置命令交给 application 的写通道执行，核心层只保留删除键集合的纯转换；审计、锁、大小上限与原子写必须与普通保存同源。

### 重要｜AgentCatalog 的适配器声明未被消费，CLI 名单在 UI 与 Hooks 域重复维护

- 置信度：确认
- 位置：`slterm_agents/src/catalog.rs` 的 `AgentCatalogEntry`、`HistoryAdapter`、`HooksAdapter`；`slterm_app/src/app_shell/app_root/settings_agents.rs` 的 `AGENT_NAMES`；`slterm_app/src/ai_hook/integrations.rs` 的 `AGENTS`；`slterm_settings/src/agent_hooks.rs` 的 `AgentHook::ALL`；`docs/devPlan/design/agents.md` 的「AgentCatalog 与 AgentRoot」。
- 现象：`history_adapter`、`hooks_adapter` 与 `builtin_command` 除 catalog 自身测试外没有消费者；Hooks 状态页使用独立的 `AGENT_NAMES` 与 `AGENTS` 列表，设置层再维护 `AgentHook::ALL`；`AgentHook` 与 `HooksAdapter` 之间没有映射或一致性测试。HistoryAdapter 没有任何历史扫描消费者，也没有适配器版本。
- 根因：目录表只提供描述性字段，实际 UI/Hooks/History 各自维护可枚举名单，违反“唯一 AgentCatalog，UI、Runtime 和恢复流程不得各自维护 CLI 名单”。
- 影响：新增或改名 CLI 时必须同步多处列表；Hooks 页显示顺序、可配置集合与 catalog 可能漂移；历史适配器版本无法随记录身份演进。
- 证据：`hooks_adapter`/`history_adapter` 在 `slterm_app` 中无引用；`settings_agents` 的 `AGENT_NAMES` 与 `integrations::AGENTS` 各写一份；`AgentHook::ALL` 与 `HooksAdapter` 没有交叉测试；`HistoryAdapter` 没有版本字段。
- 优化方向：让 Hooks 页、历史扫描和恢复都从 AgentCatalog 派生其 CLI 集合与适配器标识；为历史适配器提供版本并把 catalog 映射纳入测试。

### 重要｜Hooks 事件目录远小于 featureSpec 的 Claude 事件集合

- 置信度：确认
- 位置：`slterm_hooks/src/lib.rs` 的 `HookEvent`、`HookEvent::ALL`、`CLAUDE_EVENTS`、`HookEvent::fields`；`slterm_app/src/ai_hook/editor.rs` 对 `CLAUDE_EVENTS` 的使用；`docs/featureSpec/panels/settings.md` 的「GUI 与 JSON」；`docs/devPlan/design/hooks.md` 的「能力目录与适配器」。
- 现象：事件目录只包含 SessionStart、UserPromptSubmit、Notification、PermissionRequest、PreToolUse、PostToolUse、PostToolUseFailure、Stop、StopFailure、SessionEnd。featureSpec 要求 Claude 事件目录覆盖 Setup、PostToolBatch、PermissionDenied、SubagentStart/Stop、TaskCreated/Completed、TeammateIdle、PreCompact/PostCompact、ConfigChange、CwdChanged、FileChanged、InstructionsLoaded、DirectoryAdded、WorktreeCreate/Remove、Elicitation/ElicitationResult 等事件；设计文档要求目录覆盖 featureSpec 的完整集合。安装器只订阅当前目录，Hooks 编辑器也以该目录拒绝/过滤事件。
- 根因：事件目录只实现了生命周期与部分工具事件，未完成 featureSpec 的完整 Claude 事件面；测试 `claude_events_derive_from_catalog` 只断言目录内部自洽，不比对 featureSpec。
- 影响：Hooks 页无法编辑、保存或保留大量合法 Claude 事件；注入器不会安装这些事件；用户既有配置中的这些事件可能被编辑器判为不支持，违反“未知但 schema 合法字段原样保留”和完整能力目录要求。
- 证据：`HookEvent` 枚举与 `CLAUDE_EVENTS` 列表；`editor.rs` 用 `CLAUDE_EVENTS.contains` 拒绝事件；`local.rs` 按 `CLAUDE_EVENTS` 安装；全仓没有 `PostToolBatch`/`SubagentStart`/`PreCompact` 等符号实现。
- 优化方向：按 featureSpec 的事件集合扩展目录、字段矩阵、安装器与编辑器校验；增加目录与 featureSpec 清单的机械对账测试。

### 重要｜设置热应用在 GPUI 前台同步读取自定义主题文件

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `SettingsView::commit_field` 与 `AppRoot::apply_settings_field` / `reload_settings_snapshot`；`slterm_app/src/app_shell/config.rs` 的 `Settings::from_runtime` 与 `resolve_theme`；`slterm_app/src/theme_library/store.rs` 的 `ThemeLibraryStore::load`；`slterm_app/CLAUDE.md` 的「前台纪律」；`theme_library/CLAUDE.md` 的「冷路径同步」。
- 现象：设置控件提交发生在 GPUI 前台；`apply_settings_field` 立即调用 `reload_settings_snapshot`，后者调用 `Settings::from_runtime`；当 `custom_theme` 已设置时，`resolve_theme` 会同步调用 `ThemeLibraryStore::load`，对主题文件执行 `fs::read`、大小检查与 JSON 解析。任意字段修改都会重复一次主题文件读取。
- 根因：主题解析被放在设置快照重建的同步路径里，没有把冷路径文件读取移到后台或缓存已解析定义。
- 影响：自定义主题启用时，设置页每次输入/切换都可能在前台进行磁盘 I/O 与解析；主题文件位于慢盘或被杀软扫描时会卡住界面，违反“GPUI 前台禁止阻塞 I/O”的硬约束。
- 证据：`commit_field` 直接 `root.update(... apply_settings_field)`；`apply_settings_field` 调用 `reload_settings_snapshot`；`Settings::from_runtime` 调 `resolve_theme`；`resolve_theme` 在 `custom_theme` 分支调用 `store.load`；`ThemeLibraryStore::load` 使用 `std::fs`。
- 优化方向：设置热应用只使用已缓存的主题定义；文件读取与解析在后台完成后回投前台，前台不直接 `fs::read`。

### 重要｜追溯矩阵与测试语义不匹配，进度声称已验收

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的「设置与持久化」「导航树」「Settings 面板」表；`docs/devPlan/progress.md` 的「Agent 与 Hooks」「设置、主题与恢复」行；测试 `settings_single_queue_across_views`、`settings_damage_classification`、`settings_import_rejects_unknown_or_credentials`、`settings_import_and_reset_preserve_secrets`、`navigation_*`、`quarantine_*`、`agent_root_*`、`runtime_agent_start_resume_and_fork_create_terminals`、`theme_import_complete_or_zero_write`。
- 现象：多个“已自动化”用例只调用纯函数、内存状态机或测试注入端口，不经过用户可达路径。`settings_single_queue_across_views` 只验证订阅计数与单一 store；`settings_damage_classification` 没有权限/高版本；`settings_import_*` 没有文件、预览或确认；`navigation_*` 只验证纯判定；`quarantine_*` 没有真实文件/SQLite/重启；`agent_root_*` 只验证未接线服务或测试注入根；`runtime_agent_start_resume_and_fork_create_terminals` 只断言终端数量，不检查注入命令。`progress.md` 仍把这些阶段标为已验收。
- 根因：追溯矩阵以“存在同名测试”作为自动化证据，而不是以“前置条件、动作、可观察结果与生产路径一致”为判据；进度验收沿用了追溯矩阵状态。
- 影响：核心功能缺失仍能保持全绿与“已验收”，后续开发会以虚假基线继续；真实失败路径、重启、部分失败、权限拒绝与用户可见提示没有覆盖。
- 证据：上述用例的断言只触及纯函数/内存对象；`progress.md` 的 Agent 与 Hooks 行声称“历史扫描／恢复／隔离批次与撤销、Agent 文件树…全量自动化”；实际生产入口缺失；`settings_single_queue_across_views` 没有两个 Settings 视图；`settings_damage_classification` 的断言集合只有缺失/单字段/解析损坏。
- 优化方向：追溯矩阵按用户可观察行为登记端到端用例；纯函数用例只能支撑算法层，不得支撑导航、隔离、导入导出或设置页验收；进度不得早于真实接线更新。

### 重要｜套餐凭据目标不含本地根稳定散列，跨数据根共享同一 token

- 置信度：确认
- 位置：`slterm_app/src/plan_balance/mod.rs` 的 `PlanBalanceSource::credential_target` 与 `PlanBalanceCredentials`；测试 `plan_balance_uses_dedicated_credential`；`slterm_app/src/local_control/credentials.rs` 的 `credential_target_at`；`docs/devPlan/design/settings-recovery.md` 的「凭据门面」。
- 现象：套餐来源的凭据目标是固定字面量 `slterm/plan-balance/deepseek` 与 `slterm/plan-balance/kimi`，不含本地根稳定散列；Runtime/handover 的目标则按 `local_dir` 做稳定散列。测试明确断言这两个固定字面量。
- 根因：CredentialFacade 的目标命名规则只在 Runtime/handover 实现，套餐来源另走一套固定命名。
- 影响：同一 Windows 用户下不同数据根（包括测试根或隔离实例）会共享同一套餐 token；测试根无法与本机生产凭据隔离，违背“目标包含用途与本地根稳定散列，测试根相互隔离”。凭据替换/删除也会跨根互相影响。
- 证据：`credential_target` 返回固定字符串；`credential_target_at` 使用 `stable_path_hash(local_root)`；`plan_balance_uses_dedicated_credential` 断言固定目标。
- 优化方向：套餐凭据目标同样由“用途 + 本地根稳定散列”生成，并让测试根注入独立的 local root；凭据替换/删除按目标作用域隔离。

### 重要｜合同冲突：主题 WCAG 校验在行为规格与模块纪律之间互相矛盾

- 置信度：确认
- 位置：`docs/featureSpec/cross-cutting.md` 的「主题」；`docs/featureSpec/panels/settings.md` 的「主题与界面」；`docs/featureSpec/acceptance.md` 的「不提供」；根 `CLAUDE.md` 的「配色单点」；`docs/devPlan/design/settings-recovery.md` 的「主题与 ANSI」；`.claude/docs/architecture/overview.md` 对 `slterm_settings` 的职责描述；`slterm_settings/CLAUDE.md` 的「验收与测试」「操作」；`slterm_app/src/theme_library/CLAUDE.md` 的「决策与红线」「操作」；`slterm_settings/src/custom_theme.rs` 的 `validate_wcag`、`WCAG_TEXT_CONTRAST`、`WCAG_GRAPHICS_CONTRAST`、`foreground_recommendations`；`slterm_settings/src/themes.rs` 的 `ReviewedPalette::wcag_violations`、`meets_wcag`。
- 现象：行为规格、根纪律与设计文档明确“不做对比度校验、提示或自动回退”，并说明“断言没有媒体、ZIP、排版或 WCAG 旁路”；但 `slterm_settings/CLAUDE.md` 把“WCAG 边界”列为必测面并要求新增主题通过语义槽 WCAG 校验，`theme_library/CLAUDE.md` 的「操作」也要求 WCAG 校验，架构总览还把“WCAG 校验”列为该 crate 职责。代码实现了完整的 WCAG 计算、保存拒绝与建议色算法，测试 `save_path_accepts_the_builtin_catalog_and_rejects_low_contrast_pairs`、`builtin_palettes_pass_semantic_slot_wcag_review` 还断言内置目录全部达标、低对比组合必须被拒绝。
- 根因：模块级纪律与行为真值对同一产品红线的表述相反；代码与测试跟随了模块纪律，而不是 featureSpec。
- 影响：按判据层级，代码保留了规格禁止的能力与测试合同；后续维护者无法从文档判断应该删除还是保留 WCAG。当前生产路径未调用 `validate_wcag`，属于“禁止能力的残留 API + 测试合同”，一旦未来接入保存路径会直接改变用户行为。
- 证据：featureSpec/根纪律/设计的禁止表述与模块纪律/架构总览的要求并存；`custom_theme.rs` 的 WCAG API 与 `themes.rs` 的语义槽校验；上述两个测试用例；`ThemeTokens` 的重复字段拒绝未用于生产，说明颜色模型内部也存在选择分歧。
- 优化方向：先按判据层级裁清合同，再删除或保留 WCAG 面；无论取舍，文档、领域模型与测试必须同向，不能让模块纪律覆盖 featureSpec。

### 一般｜主题文档不拒绝重复字段

- 置信度：确认
- 位置：`slterm_app/src/theme_library/document.rs` 的 `ThemeDocument::from_json_bytes` / `from_value` / `validate_and_normalize`；`slterm_settings/src/theme_tokens.rs` 的 `ThemeTokenError::DuplicateToken` 与 `ThemeTokens::from_value`；`slterm_settings/CLAUDE.md` 的「主题语义合同」。
- 现象：主题文档用 `serde_json::from_slice::<Value>` 解析，重复键由 `serde_json::Map` 后写覆盖前写，后续校验看不到重复；`ThemeTokens` 虽然实现了重复字段拒绝，但不在主题文档路径上。
- 根因：主题文档走通用的 `Value` 解析，没有在信封层检测重复键。
- 影响：含重复槽位的主题会被静默接受，用户看到的是最后一个值；违反“重复字段一律拒绝、不生成部分主题”，也使恶意或手误输入无法被诊断。
- 证据：`from_json_bytes` 直接反序列化为 `Value`；`validate_and_normalize` 没有重复键扫描；`ThemeTokens::from_value` 的重复检测未被调用。文档头注释还声称“未知字段保留”，与实际根/段未知字段拒绝也不一致。
- 优化方向：在主题信封解析阶段使用可检测重复键的解析器或保序成员扫描；统一文档注释与实际的未知字段策略。

### 一般｜普通设置导出不校验值，导出的文件可能无法再导入

- 置信度：确认
- 位置：`slterm_app/src/settings_transfer.rs` 的 `export_settings`、`import_settings`；测试 `settings_import_rejects_unknown_or_credentials`；`docs/featureSpec/settings-and-persistence.md` 的「设置导入、导出与恢复默认」。
- 现象：导出使用 `RawSettings::persisted_value` 返回未校验的原始值；如果用户手改出非法值（例如字号为字符串），导出仍会包含它，而导入会因 `InvalidValue` 拒绝整个文件。
- 根因：导出只做键白名单投影，没有复用字段校验或单字段恢复状态。
- 影响：合法用户无法用导出的文件恢复自己的设置；导出/导入不是往返一致的操作；如果未来导出接入，会把内部损坏传播成导入失败。
- 证据：`export_settings` 的 `persisted_value` 无 `registry.validate`；`import_settings` 对同一键执行 `validate` 并在失败时返回 `InvalidValue`。
- 优化方向：导出只包含通过注册表校验的值，或显式把非法字段排除并提示；导出与导入共用同一字段校验。

### 一般｜屏幕规则引擎与用户覆盖热加载没有生产消费

- 置信度：确认
- 位置：`slterm_app/src/ai_agents.rs` 的 `detect`、`identify`、`refresh_overrides_if_needed`、`build_cache`；`slterm_app/src/agent_host.rs` 的 `AgentPaneHost::observe_screen`、`AgentHub::observe_screen`；`slterm_agent_detection` 全部规则资源。
- 现象：`detect`/`identify` 的唯一生产候选调用是 `AgentPaneHost::observe_screen`，而该函数与 `AgentHub::observe_screen` 都带 `#[allow(dead_code)]`，注释写“生产消费归 Agent 屏幕看门狗阶段”；全仓没有看门狗或其它调用点。规则资源、mtime 热加载与结构判据只在测试中运行。
- 根因：屏幕兜底被实现为完整规则引擎，但没有接入 Terminal 事件泵或可见性调度。
- 影响：没有 hook 的 CLI pane 无法从屏幕 chrome 建立身份或状态，活跃会话/徽标只能依赖 hook；用户覆盖规则永远不会生效。模块文档声称补齐 hook 覆盖不到的空档，生产事实不成立。
- 证据：`observe_screen` 的 `#[allow(dead_code)]` 与注释；`AgentHub` 只在测试中调用 `observe_screen`；`BUNDLED` 与 `override_path` 的生产调用都从 `detect` 开始，而 `detect` 无生产入口。
- 优化方向：把屏幕看门狗接入 Terminal 事件与可见性调度，或删除未接线的规则引擎与用户覆盖承诺；不要让测试覆盖度冒充功能可用。

### 一般｜`ai_sessions` 扫描无生产消费者，测试使用进程级环境变量

- 置信度：确认
- 位置：`slterm_app/src/ai_sessions.rs` 的 `scan`、`scan_at`、`scan_claude`、`scan_codex`；测试 `native_scan_ignores_the_retired_session_registry` 与 `HomeEnvGuard`。
- 现象：`ai_sessions` 模块没有生产调用者，只有自身测试；测试通过 `unsafe { std::env::set_var }` 修改 `USERPROFILE`/`HOME` 进程级环境，并依赖测试串行，另外还改写进程本地数据根中的 `ai_sessions.json`。
- 根因：旧命令面板会话扫描被保留为死模块；测试用全局环境模拟 home，而不是通过显式路径端口注入。
- 影响：模块维护成本与误导性覆盖；测试若与并行用例或真实启动同进程运行会互相污染环境；没有为导航历史提供任何记录身份或 cwd 归属。
- 证据：`scan` 在模块外无引用；`HomeEnvGuard` 使用 `std::env::set_var`；测试断言本地数据根位于临时目录；导航历史审查确认没有 `HistoryRecordKey`/adapter 版本。
- 优化方向：删除或收编为历史适配器的有界读取层，测试只经显式注入的 home/数据根；不得依赖进程级环境变量。

### 一般｜真实 Windows 凭据管理器用例默认静默跳过

- 置信度：确认
- 位置：`slterm_app/src/secrets.rs` 的 `credential_three_operations_round_trip_through_windows_manager`；`docs/featureSpec/settings-and-persistence.md` 的「凭据」相关验收；`plan_balance/CLAUDE.md` 的测试要求。
- 现象：该用例在未设置 `SLTERM_TEST_CREDENTIAL_STORE` 时打印跳过信息并直接 `return`，测试仍报告通过；默认测试命令不会执行真实 Windows 凭据管理器往返。
- 根因：用环境变量短路代替显式条件测试/豁免登记，测试结果不能区分“已验证”与“未运行”。
- 影响：凭据门面的真实后端写入、覆盖、删除与幂等路径在默认门禁中没有证据；`sk-test` 假值测试只覆盖内存 mock；凭据故障分类可能回归而全绿。
- 证据：用例的环境变量 early return；`plan_balance`/`local_control` 测试使用注入 store；豁免表未登记该条件跳过；追溯矩阵对凭据的“已自动化”只指向 mock 或假服务。
- 优化方向：把真实凭据库用例登记为条件豁免并明确兜底层级，或在 CI 可用环境中真正执行；不得以打印跳过的方式让空断言通过。

## 跨域观察

- 设置面板的页面集合与 featureSpec 的“无项目浮层提供快捷键、后台任务、外观、终端、应用、Agent 基础配置和 Hooks User”不一致：当前页面枚举只有 Appearance、Scrolling、Interaction、Session、System、AgentHooks、Hooks，没有快捷键与后台任务页；套餐页脚也没有导航视图承载。属设置视图与导航所有者，但会影响本范围的设置验收。
- `plan_balance` 已有调度、解析与凭据客户端，却没有导航页脚与设置入口；真实 token 配置、查询、冷却与过期展示无法从用户路径触发。属导航/设置视图所有者。
- `agent_env` 的 PATH 去重只按大小写不敏感字面比较，不归一 `/` 与 `\`、尾分隔符或短名；嵌套启动时可能保留重复目录。属终端环境所有者，影响有限。
- `theme_library` 的 `ThemeTokens` 是一个更贴近规格的颜色-only 完整模型，但生产没有使用；主题文档最终走 `ThemeDefinition` 的混合模型。属主题所有者，和本报告的主题边界问题同源。

## 存疑未证

- Cursor/Antigravity 的 slug 与 catalog 内置命令不一致已在代码中确认；但 `cursor`/`antigravity` 是否在某些安装环境中也能启动对应 CLI，需要实机 CLI 验证。缺失证据：两个 CLI 的真实入口名与 `agent.start` 注入后的终端输出。
- Claude `--continue` 的无 ID 恢复可能是产品刻意的“接续最近会话”语义；当前只有代码与测试，没有 featureSpec 的明确允许表述。缺失证据：产品所有者对该分支的确认。
- 内置 AgentRoot 是否应允许写入/终端开关，featureSpec 说“每个 AgentRoot”有独立能力，代码与 app 注释固定内置只读；需要所有者裁定。缺失证据：设置页 AgentRoot 能力矩阵的设计稿或验收例。
- 主题导入“冲突拒绝”是否仅指 ThemeId 冲突；当前实现自动生成新 id 与显示名。缺失证据：设计文档对“导入冲突”的定义与用户预期。
- 启动维护对隔离表的整体 `NeedsAttention` 是否只是“恢复器接入前”的临时策略；代码注释与 startup 纪律把它写成固定规则，但 featureSpec 要求对账后继续。缺失证据：隔离恢复器是否计划在后续阶段注入。
- 屏幕规则引擎是否属于未完成阶段；模块注释写“生产消费归 Agent 屏幕看门狗阶段”，但 `progress.md` 已把 Agent 阶段标为验收。缺失证据：看门狗切片是否被延期的明确记录。
- 真实 Windows 凭据管理器用例在 CI 的实际执行环境；需要确认 `SLTERM_TEST_CREDENTIAL_STORE` 是否在发布门禁中设置。缺失证据：CI 配置或发布检查单。
- 设置导出/导入的最终文件格式与版本字段；当前只有内存 DTO，没有可核验的磁盘契约。缺失证据：实现该功能时的格式提案与示例文件。
