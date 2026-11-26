# Hooks 域与 hook 小进程桥审查

> 审查基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支）当前工作树；只读审查，未运行 cargo、测试、构建或网络。

## 覆盖与局限

- 已覆盖：`slterm_app/src/ai_hook/` 的模块根、`draft`、`draft_host`、`editor`、`payload`、`protocol`、`ordering`、`windows`、`lifecycle`、`event`、`native_events`、`installation` 及其 `notify` 子模块、`integrations`、`bridges`、`local` 及其 `codex_hooks`／`codex_notify`／`config_guard`／`cursor`／`extended`／`kimi`／`managed_files`／`settings`，以及 `tests`、`native_tests`、`e2e_tests`、`lifecycle/tests`；`slterm_hook/src` 与 `slterm_hook/CLAUDE.md`；`slterm_hooks/src`；`slterm_app/src/agent_host.rs`；`app_root` 的 `hooks_page`、`settings_agents`、`settings_overlay`、`projects`、`project_close`、`runtime_snapshot`、`commands`、`task_service`、`application_close`、`batch_close` 及 Hooks 相关 GPUI 测试；`atomic_file`、`settings_store`、`audit` 的调用面。
- 已覆盖文档：根与各层 `CLAUDE.md`、`docs/devPlan/plan.md`、`progress.md`、`traceability.md`、`design/hooks.md`、`design/agents.md`、`design/settings-recovery.md`、`featureSpec/panels/settings.md`、`.claude/docs/testing/exemptions.md`。
- 局限：没有真实 CLI 安装、真实命名管道双进程链路、真实 ConPTY 环境传播和真实 GPU/桌面会话；各 provider 的配置格式与版本行为只能据仓内生成器与测试推断，未能对真机核验。未运行任何门禁，因此不判断构建或测试是否真实通过。

## 问题

### 阻断｜生产环境没有 hook 事件入口：命名管道服务、事件泵与桥接环境只存在于测试

- 置信度：确认
- 位置：`slterm_app/src/ai_hook.rs` 的 `spawn_gpui_server` 重导出与 `PIPE_ENV`／`HOOK_EXE_ENV`；`slterm_app/src/ai_hook/windows.rs` 的 `spawn_pipe_server`；`slterm_app/src/agent_host.rs` 的 `AgentHub::dispatch_lifecycle`；`slterm_app/src/ai_hook/CLAUDE.md`「命名管道 = hook 事件唯一权威通道」；`docs/devPlan/progress.md` 的 Agent 与 Hooks 行。
- 现象：管道服务只在测试内启动。生产启动链既不创建 `PipeServer`，也不设置 `SLTERM_NOTIFY_PIPE` 与 `SLTERM_HOOK_EXE`，更没有把接收端排水到 `AgentHub` 的前台泵。安装到各家 CLI 的 `slterm-hook` 在生产环境只会得到「无管道闸门」，按 helper 自身契约静默退出；opencode／Pi 插件同样因 `SLTERM_HOOK_EXE` 缺失而惰性返回。
- 根因：`ai_hook.rs` 仍保留「管道服务仅由独立集成测试启动；生产接入归后续 Agent 协调器」的临时裁决，但计划与进度已把该域记为验收完成；`AgentHub::dispatch_lifecycle` 只有测试调用方，AppRoot 对 `agents` 只做 register／seed／remove 与命令边界更新。
- 影响：hook 注入的生产效果为零。Agent 状态、会话身份回填、恢复注入、完成／等待通知、后台任务守卫、事件门与流排序全部拿不到 hook 事实；`ai_hook` 模块文档描述的「命名管道线程 → 进程内 mpsc → GPUI 前台排水 → AgentActivity」链路在运行时不成立。核心验收「命名管道为唯一权威通道」不成立。
- 证据：`spawn_gpui_server` 在模块根被 `#[cfg(test)]` 限定；仓内没有生产代码调用 `spawn_pipe_server` 或 `std::env::set_var(PIPE_ENV, …)`；`AgentHub::dispatch_lifecycle` 的调用点只在 `e2e_tests` 与 `agent_host` 测试；`res/hooks/opencode.js` 与 `res/hooks/pi.ts` 都在启动时检查 `process.env.SLTERM_HOOK_EXE` 并在缺失时直接返回。
- 优化方向：把管道服务、桥接环境发布与前台事件泵纳入唯一启动链（必须先于首个 PTY spawn 发布环境并持有服务句柄到退出），并用真实 helper→宿主进程的端到端用例或生产接线探针钉住「已接线」事实，而不是只钉两半各自的行为。

### 阻断｜打开 Hooks 页会在进程内留下无法清除的草稿，使项目移除与根重定位永久报 `HooksDraftPending`

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/draft_host.rs` 的 `project_has_drafts`；`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `request_hooks_draft`、`discard_hooks_draft_for`、`bind_settings_panel`、`hooks_layer_path`；`projects.rs` 的 `relocate_project`；`project_close.rs` 的 `remove_project_with_decisions`；测试 `navigation_relocation_guards_and_reassigns_history` 与 `project_remove_requires_hooks_drafts_resolved`。
- 现象：项目 Settings 面板绑定后若当前页是 Hooks，或用户进入 Hooks 页，即会为 Project 层创建草稿；即使没有任何修改，`project_has_drafts` 也视为「未解决」。用户点「丢弃」后 `discard_hooks_draft_for` 立即调用 `request_hooks_draft` 重新载入并再次插入同键草稿。切页、关闭面板、切换层级都不移除该草稿；重启后若 Settings 的选中页仍持久为 Hooks，恢复面板时会再次载入草稿。
- 根因：守卫按「存在草稿」而不是「存在脏草稿」判决，`dirty_drafts` 没有被守卫使用；丢弃路径没有「结束编辑」状态，必然重新载入；草稿归属于 AppRoot 的 `HooksDraftHost` 而非面板生命周期，面板销毁没有对应清理。
- 影响：任何项目只要打开过 Hooks 页（或恢复在该页），交互式移除与根重定位就持续失败，错误提示要求「保存或丢弃」，但两条 UI 路径都无法消除草稿；恢复材料先于项目删除，用户只能通过切换选中页并重启等非显然操作绕开。测试用 `clear_project` 直接消键才得以通过，掩盖了生产路径不可解。
- 证据：`HooksDraftHost::project_has_drafts` 只看 `key.layer.project()`，不看 `dirty()`；`discard_hooks_draft_for` 在移除后立刻 `request_hooks_draft`；`bind_settings_panel` 在选中页为 Hooks 时请求草稿；`relocate_project`／`remove_project_with_decisions` 均在构造候选前返回 `HooksDraftPending`；`clear_project` 的生产调用点位于守卫之后的清理阶段与 Runtime 显式丢弃路径，交互路径没有入口。
- 优化方向：守卫只对未保存修改生效，或引入显式的「结束草稿」状态并让丢弃/切层/关面板都能真正释放；把草稿生命周期绑定到面板实例，面板销毁时清理，避免不可见草稿成为全局生命周期闸门。

### 阻断｜交互式关闭不处理 Hooks 草稿：应用退出静默丢失未提交编辑，面板关闭留下孤儿草稿

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/application_close.rs` 的 `start_application_close_with_decisions`；`batch_close.rs`；`runtime_snapshot.rs` 的 `start_runtime_application_close`；`docs/devPlan/design/settings-recovery.md`「关闭预检先处理所有脏 Document、Hooks 草稿和终端决定」；`docs/featureSpec/panels/settings.md`「保存与关闭」。
- 现象：只有 Runtime 的 `app.close` 接受 `CloseHooksPolicy` 并丢弃草稿；OS 关窗、托盘退出、项目/面板关闭路径完全不检查 `hook_drafts`。带脏 Hooks 草稿退出会直接销毁窗口和 AppRoot，草稿不留任何提示；关闭 Settings 面板则把草稿留在 AppRoot、键仍指向已销毁的 PanelId，之后既不可见也无法丢弃。
- 根因：Hooks 草稿没有接入统一关闭协调器，只在 Runtime 分支单独处理；面板关闭没有按 `PanelContent::Settings` 清理 `HooksDraftHost`／`HooksEditorState`。
- 影响：未提交的 hook 编辑丢失，直接违反「关闭、切层…经保存／丢弃／取消守卫」和关闭预检合同；孤儿草稿还会继续参与 `project_has_drafts`，叠加前述永久阻断。
- 证据：`application_close.rs` 与 `batch_close.rs` 中没有 `hook_drafts`／`HooksDraft` 引用；`start_runtime_application_close` 是唯一出现 `CloseHooksPolicy` 的关闭路径；`AppRoot.hook_drafts` 在面板关闭时无清理调用。
- 优化方向：把 Hooks 脏草稿纳入统一关闭预检，提供保存／丢弃／取消三态；面板销毁时同步释放该面板的面板级草稿与文档，Runtime 的显式丢弃语义复用到交互路径。

### 重要｜管道事件不校验 pane 归属与 PTY 代次，迟到或伪造事件可改写活跃 pane 状态

- 置信度：确认
- 位置：`slterm_app/src/agent_host.rs` 的 `AgentHub::dispatch_lifecycle`；`slterm_app/src/ai_hook/event.rs` 的 `AiHookEvent`（`pane`／`client_pid`／`agent_pid`）；`slterm_app/src/ai_hook/windows.rs` 的 `dispatch`；`docs/devPlan/design/hooks.md`「application HookEventService 校验事件来源、AgentKind、TerminalId、PTY 代次和事件代次」。
- 现象：宿主仅按信封里的 `pane` 数字查找现存 pane 宿主，然后把事件交给 `AgentActivity`。内核给出的 `client_pid` 只被换算成 `agent_pid` 写入事件，未与目标 pane 的 PTY／终端登记做任何比对；事件结构里根本没有 PTY 代次。任何能连接管道的同用户进程都可声明任意活跃 pane 号并改变该 pane 的 Working／Blocked／Done、会话身份与 attention。
- 根因：pane 路由被当作身份校验，`TerminalRegistry` 的进程归属与 PTY 代次没有进入 hook 通道；设计要求的来源、TerminalId 与代次校验被降为一个可伪造的 u64 字段。
- 影响：迟到事件（旧 PTY 代次）或同机伪造事件会污染错误 pane 的 Agent 状态、恢复会话与通知；与「非法或过期事件只丢弃」相反，这里会接受并外显。
- 证据：`dispatch_lifecycle` 的 `target` 只由 `event.pane`／焦点兜底决定；`dispatch` 把 `client_pid` 注入事件并计算 `agent_pid`，但 `AiHookEvent` 无 PTY 代次字段，`AgentPaneHost` 也不持有 pane 的进程身份；`PIPE_ENV` 只是端点名，不承载每 pane 凭证。
- 优化方向：在事件进入仲裁器前把 `client_pid`／祖先链与目标 pane 的 PTY 进程树、PTY 代次绑定校验；把代次纳入事件合同与流键；未证明归属的事件只记受限健康计数并丢弃。

### 重要｜注入/卸载没有 Journal 与启动恢复器，提交边界崩溃会留下不可卸载残留；验收锚只是纯枚举映射

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/installation.rs` 的 `HooksJournalKind`、`HooksRecoveryFacts`、`hooks_recovery`；`slterm_app/src/persistence/journal.rs` 的 `ManagedOperationJournal`（Hooks 域无消费者）；`local/codex_hooks.rs` 的 `install`／`remove`；`local/codex_notify.rs`；测试 `hooks_post_commit_cleanup_failure_is_visible`、`hooks_recovery_decides_commit_rollback_or_attention`；`docs/devPlan/traceability.md` 的对应行。
- 现象：Claude 单文件注入是原子写，但 Codex 的 `hooks.json`、归属 marker 与 `config.toml` features、Kimi 与 Cursor 的多形态写入都没有 Journal 记录准备材料、前后指纹或清理步骤。`hooks_recovery` 与 `HooksJournalKind` 没有任何生产调用方；`ManagedOperationJournal` 未被 Hooks 使用。Codex 安装按 hooks → marker → config 的顺序写，若在 hooks 写完后崩溃，marker 缺失会使 `installed_at` 返回未安装，而 `remove` 因「无 marker」直接跳过卸载，残留的 helper hook 无法由正常卸载清除。
- 根因：计划要求的 HookService／ManagedOperationJournal 注入事务没有落地，只有状态枚举与纯函数被测试；安装器各自直接写文件，缺少跨文件提交边界与恢复对账。
- 影响：崩溃或部分失败后用户配置里留下不可见的残留注入；UI 报「未安装」但 CLI 仍会调用 helper。`hooks_post_commit_cleanup_failure_is_visible` 只断言 `hook_retry_action` 的枚举映射，不能证明任何提交后清理事实；追溯矩阵的「已自动化」据此失真。
- 证据：`hook_retry_action` 的测试只输入手工构造的 `HookCommitState`；`hooks_recovery` 只有 `installation_tests` 调用；`ai_hook` 目录内没有 `ManagedOperationJournal` 引用；`codex_hooks::install` 的三个 `write_changed` 直接连续执行，没有幂等恢复记录；`remove` 在 marker 不存在时返回未改动。
- 优化方向：以 ManagedOperationJournal 记录注入/卸载的准备材料、文件基线与前后指纹，在启动维护阶段按指纹对账提交、回滚或待处理；安装器只作为 Journal 的步骤执行体，测试必须覆盖提交边界崩溃与部分失败后的真实文件状态。

### 重要｜危险命令确认没有生产链路：确认原文 API 无调用方，命中后只得到通用失败

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/installation.rs` 的 `InstallConfirmation`／`install_gate`；`slterm_app/src/ai_hook/integrations.rs` 的 `statusline_confirmation`、`claude_statusline_confirmation_at`、`apply_enabled_confirmed`、`apply_claude_confirmed_at`；`slterm_app/src/app_shell/app_root/settings_agents.rs` 的 `set_agent_hook_enabled`；`docs/featureSpec/panels/settings.md`「注入与卸载」。
- 现象：`set_agent_hook_enabled` 只调用 `apply_enabled`，`apply_enabled_confirmed` 与两个 `statusline_confirmation` 入口没有任何 UI 调用方。Claude 的 statusLine 命中可疑命令时，安装被判 `Suspended`，UI 最终只看到全局「安装失败」标记；命令原文从未展示，也没有「确认并重试」入口。Codex `hooks.json` 里用户原有的可疑命令则连 `install_gate` 都不经过，注入照常进行。
- 根因：审查门与确认重试只实现了纯函数和应用层 API，没有接入 AgentHooks 页面的交互；`apply_with_confirmation` 在 `Suspended` 时只记审计事件，随后由通用检查失败收尾。
- 影响：需要确认的安装路径对用户不可完成；合法用户被卡在无解释的失败上，而另一类注入路径绕过了合同要求的「未确认前设置文件零写入」。命令原文也不会出现在确认界面，因为界面不存在。
- 证据：`install_gate` 只在 `installation_tests` 中被调用；`integrations::apply_enabled` 是 `settings_agents` 的唯一安装入口；`apply_claude_at_with` 把 `suspicious_command` 拼进 `io::Error` 文本后由 `finish_agent_hook_apply` 丢弃错误内容；`set_agent_hook_enabled` 没有确认态参数。
- 优化方向：把「待确认原文 + 确认重试」做成 AgentHooks 页的真实状态与操作，并让 hooks 数组命令复用同一确认门；原文只经确认界面显示，确认前保持零写入。

### 重要｜能力目录缺省于 featureSpec 的事件集合，含合法事件的配置无法保存

- 置信度：确认
- 位置：`slterm_hooks/src/lib.rs` 的 `HookEvent::ALL`、`CLAUDE_EVENTS`、`HookHandlerType`；`slterm_app/src/ai_hook/editor.rs` 的 `validate_editable`；`docs/featureSpec/panels/settings.md`「Hooks 页 · GUI 与 JSON」；`docs/devPlan/design/hooks.md`「能力目录与适配器」。
- 现象：目录只覆盖 SessionStart、UserPromptSubmit、Notification、PermissionRequest、PreToolUse、PostToolUse、PostToolUseFailure、Stop、StopFailure、SessionEnd 这组生命周期事件；featureSpec 明确列出的 Setup、PostToolBatch、MessageDisplay、SubagentStart／Stop、TaskCreated／Completed、PreCompact／PostCompact、ConfigChange、CwdChanged、FileChanged、InstructionsLoaded、DirectoryAdded、WorktreeCreate／Remove、Elicitation 等均不在目录。加载时 `HooksDocument` 不校验事件名，因此这些事件会进入可编辑 JSON；保存时 `validate_editable` 以 `CLAUDE_EVENTS` 拒绝整个文档，导致用户即使只想修改无关字段也无法保存。
- 根因：事件目录按当前安装器订阅白名单收缩，而不是按 featureSpec 的完整编辑集合定义；保存校验与安装订阅共用同一份窄表，无法区分「未知非法」与「合法但未订阅」。
- 影响：能力目录不是 GUI、schema、保存校验与注入识别的完整单一来源；合法的 Claude 配置被当成非法内容，未知但 schema 合法的字段/事件无法「原样保留」。示例 `hook_event_catalog_is_closed_and_unique` 只验证目录自洽，没有对照 featureSpec 契约。
- 证据：`HookEvent::ALL` 与 `CLAUDE_EVENTS` 是同一组生命周期名；`validate_editable` 对每个顶层事件执行 `CLAUDE_EVENTS.contains` 并返回「非法 hook 事件名」；`HooksDocument::load_at` 加载时不拒绝未列出的事件。
- 优化方向：把编辑目录与安装订阅拆成同一目录上的两个声明（完整编辑集合 vs 安装集合），保存校验按完整目录，未知事件仍按 JSON 语法与 shape 规则保留；补一条以 featureSpec 事件表为输入的契约测试。

### 重要｜GUI 编辑模式与事件树/handler 表单不存在，字段矩阵与能力目录双源

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `render_hooks_page`；`slterm_app/src/ai_hook/editor.rs` 的 `HandlerField`、`type_fields`、`COMMON_FIELDS`、`field_applies`、`load_field_text`、`load_flag`、`insert_json_field`；`docs/featureSpec/panels/settings.md`「GUI 与 JSON」。
- 现象：Hooks 页只渲染一个多行 JSON `Input`、层级 pill、保存与丢弃；没有 JSON/GUI 模式切换，没有事件树、matcher 组、handler 表单，没有「添加事件只列未配置项」，没有完整诊断入口。`HandlerField` 及其字段矩阵只有定义与单元测试，没有任何渲染消费方。保存校验用 `HookHandlerType::required_fields`，表单字段适用性用 `type_fields`／`COMMON_FIELDS` 的独立表，二者不是同一目录生成。
- 根因：GUI 模式停留在 scaffolding，页面只接上了 JSON 文本路径；schema 与字段声明没有收敛到 `slterm_hooks`。
- 影响：核心规格「默认 JSON，可切 GUI；两种模式编辑同一草稿」未实现；非法模型只显示首条诊断，无法定位到事件/matcher/handler；能力目录与表单字段长期双源，新增字段必须两处修改。
- 证据：`render_hooks_page` 仅构造 `Input::new` 与草稿文本；`type_fields`／`load_field_text`／`insert_json_field` 在 `editor.rs` 外无引用；`validate_editable` 只读 `required_fields`。
- 优化方向：先让能力目录声明每种 handler 的完整字段与事件能力，再由同一声明生成 GUI 表单与保存校验；页面补齐模式切换、事件树与完整诊断入口。

### 重要｜Hooks 编辑保存绕过审计与 PathPolicy，和安装器的「唯一写路径」声明冲突

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/editor.rs` 的 `write_editable_at`；`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `hooks_layer_path`；`slterm_app/src/ai_hook/local.rs` 的 `ensure_claude_hooks_in` 与「user 层 settings.json 写入唯一路径 = 审计包装」；`docs/devPlan/design/hooks.md`「所有 Hooks 写入经 HookService 的受限 PathPolicy 能力」。
- 现象：Hooks 编辑器的保存直接对目标路径调用 `atomic_file::write`（外加自建 `.bak`），没有 `audit` 事件；项目层路径由 `project.root().join(".claude/...")` 直接推导，未取得任何 PathPolicy 授权。安装器的 Claude 写入则经 `audit::write_user_settings` 并声明为唯一路径；Codex／Kimi／Cursor／插件安装器也各自直接写外部配置，只有 Claude 有审计。
- 根因：设计中的 HookService 单点没有落地，编辑器、安装器与卸载器形成了多套写通道与多套备份命名（`.bak` 与 `.slterm-bak`）。
- 影响：同一个 `~/.claude/settings.json` 存在审计覆盖不一致的两条写路径；用户/第三方 hook 编辑没有任何安全写入留痕，违反审计通道与「所有 Hooks 写入经 HookService」的约束；PathPolicy 授权边界被前端路径拼接绕过。
- 证据：`write_editable_at` 只调用 `atomic_file::write` 与 `backup_path`；`ensure_claude_hooks_in` 明确注释 user 层写入恒走 `crate::audit::write_user_settings`；`hooks_page` 直接以项目根拼路径后交给后台 `HooksDocument::load_at`／`save_editable`；Hooks 写入没有 `slterm_path` 授权调用。
- 优化方向：把 Hooks 文件目标收编为受限能力（PathPolicy grant + HookService 写包装 + 统一备份代次），编辑保存与安装/卸载共用同一审计与锁语义。

### 重要｜加载路径不检测重复键，UI 可静默编辑重复键文件

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/editor.rs` 的 `HooksDocument::load_at`、`read_root_text`；`slterm_app/src/ai_hook/draft.rs` 的 `parse_hooks_json`；`docs/featureSpec/panels/settings.md`「范围与草稿」；测试 `hooks_draft_detects_duplicate_keys`。
- 现象：严格重复键解析只在 `HooksDraft::set_text` 处理编辑器文本时使用；打开文件走 `serde_json::from_str`，重复键按最后的值为准被静默接受。用户打开含重复键的 `settings.json` 时不会看到拒绝，编辑保存会保留 hooks 以外的重复键，或在 hooks 子树内悄悄合并重复项。
- 根因：严格解析器与文档加载器是两条通道，`load_at` 没有复用 `draft::parse_hooks_*`；`parse_hooks_json` 的重复键单测没有覆盖 UI 加载入口。
- 影响：违反「解析器必须检测重复键；重复键、坏 JSON、非法 UTF-8、权限和锁失败都拒绝」；重复键文件可被部分改写，用户原始结构不可预期地丢失。
- 证据：`load_at` 调用链为 `read_root_text` → `serde_json::from_str`；`hooks_draft_detects_duplicate_keys` 直接调用 `parse_hooks_json`；`load_at` 没有重复键诊断。
- 优化方向：让文件加载与文本编辑共用同一严格解析器；对重复键、非法 UTF-8、根形态在进入草稿前分类拒绝并保留原文件。

### 重要｜`config_guard` 没有生产调用点：默认启用不生效，自愈与失败提示从不运行

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/local/config_guard.rs` 的 `spawn_config_guard_with`、`heal_all`；`slterm_settings/src/agent_hooks.rs` 的 `AgentHook::DEFAULT`；`slterm_app/src/app_shell/app_root/settings_agents.rs` 的 `set_agent_hook_enabled`；`.claude/docs/testing/exemptions.md` 的「config_guard 后台线程启动可达性」。
- 现象：进程内没有任何地方调用 `spawn_config_guard_with`。默认启用的 Claude／Codex 授权只在 AgentHooks 页显示为开，启动时不会安装；外部改写已安装配置不会自愈；安装失败不会产生约定的 `Install`／`Settings` 通知，`on_alert` 也没有消费方。豁免条目把问题描述为「线程启动无法在虚拟窗口断言」，但真实缺口是生产调用点不存在；兜底用例只覆盖失败门、冷却与纯逻辑。
- 根因：设置域启动链没有挂接 config guard；AgentHooks 页面成为唯一安装入口，默认授权与运行的注入状态脱节。
- 影响：首次使用的默认开启状态是「显示已授权但未安装」；失败后只能手动反复切换开关，`failed → retry` 语义不成立；外部修改注入不被修复，也没有用户可见的失败提示。
- 证据：仓内 `spawn_config_guard_with` 无调用点；`set_agent_hook_enabled` 只入队设置并调用 `apply_enabled`，失败只置 `agent_hooks_error = true` 并重新探测；`heal_enabled` 与失败提示逻辑没有任何生产驱动。
- 优化方向：在设置域启动链挂接 config guard 并定义它的 shutdown 生命周期；让默认授权的首次落盘与失败重试/提示有真实生产路径；修正豁免登记，区分「不可测线程启动」与「从未接线」。

### 重要｜AgentHooks 开关状态按 `AgentHook::ALL` 顺序索引 `AGENTS` 行，多个 CLI 显示与点击语义错位

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/integrations.rs` 的 `AGENTS`；`slterm_settings/src/agent_hooks.rs` 的 `AgentHook::ALL`；`slterm_app/src/app_shell/app_root/settings_agents.rs` 的 `render_agent_hooks_page`／`AGENT_NAMES`；测试 `agent_hooks_page_renders_status_and_toggles_through_injected_ports`。
- 现象：开关值数组由 `AgentHook::ALL.map(...)` 生成，渲染却按 `AGENTS`（展示顺序）逐行取同一索引。两个顺序不同：Cursor、Kimi、Pi、Copilot、Grok 行显示的是另一个 agent 的启用状态；点击时把正确 agent 的授权设置为「错误基准值的反值」。
- 根因：展示表与应用表被声明为「语义不同」，但渲染代码仍以位置隐式关联，没有按 `agent` 值查找。测试只覆盖 Claude 与 Codex 行，两者恰好位于两个顺序的前部，未暴露错位。
- 影响：多个 CLI 的开关呈冻结或反向：用户看到的是他人状态，点击可能无效或与意图相反；显式授权键被写成与界面意图不一致的值，安装/卸载方向也可能与用户判断不符。
- 证据：`enabled = AgentHook::ALL.map(...)`；遍历为 `AGENTS.into_iter().zip(AGENT_NAMES).enumerate()` 后取 `enabled[index]`；`AgentHook::ALL` 与 `AGENTS` 的成员顺序在源码中明显不同；UI 测试只检查 claude/codex 的选择器。
- 优化方向：以 agent 身份为键构造状态映射，或统一两表顺序；补一条覆盖顺序差异后半段行的渲染与点击测试。

### 重要｜保存进行中继续编辑会被保存回执静默覆盖

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `save_hooks_draft_for`；`slterm_app/src/ai_hook/draft_host.rs` 的 `HooksDraft::mark_saved`；`settings_overlay.rs` 的 `apply_hooks_sync`。
- 现象：保存任务捕获提交时的文本后进入后台；回执到达时用 `reloaded.editable_json()` 作为新文本调用 `mark_saved`，并递增 `hooks_generation`。若用户在保存期间继续输入，`update_hooks_draft_text` 已写入的新文本会被回执整体替换，编辑器也在帧间同步中被改回已保存内容。
- 根因：保存没有「在途编辑」隔离或冲突检测；`mark_saved` 把磁盘回读文本无条件当成用户当前文本，而不是仅前移基线。
- 影响：保存期间的用户编辑静默丢失，且没有提示或失败路径；长文本或慢盘上更容易发生。
- 证据：`save_hooks_draft_for` 在后台完成 `save_editable` 与 `reload` 后执行 `draft.mark_saved(baseline, text, model)`，其中 `text` 来自重载文档；`mark_saved` 同时覆盖 `text` 与 `model`；没有保存中标志或提交代次比对。
- 优化方向：保存只推进基线与已保存快照，不覆盖保存开始后的新编辑；或以提交代次判定，检测到期间编辑时保留草稿并提示重新保存。

### 重要｜Hooks 层默认与切换错误：项目面板以 Project 打开，点 User 后层选择器只剩 User

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `with_state`；`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `available_hooks_layers`；测试 `hooks_page_loads_saves_external_guard_and_discards`；`docs/featureSpec/panels/settings.md`「状态与验收」；`docs/devPlan/design/settings-recovery.md`「当前 Hooks 层级和 GUI／JSON 模式不持久化，重建后回到 User 与 JSON」。
- 现象：项目 Settings 创建时层级被初始化为 `Project(project_id)`，测试也以此为准；规格要求重建视图（关闭重开与重启）回到 User。更严重的是 `available_hooks_layers` 以「当前层级」决定可选集合：从 Project 点到 User 后，Project/Local 两个 pill 消失，无法回到项目层，只能重建整个 Settings 视图。
- 根因：层级选择器按当前值生成选项，而不是按面板能力生成固定集合；初始层级硬编码为 Project，与瞬态状态合同相反。
- 影响：文档与页面行为不一致；项目根重定位/移除守卫本来就依赖 Project/Local 草稿，而层级切换不可达会放大前述阻断；无项目浮层的 User-only 行为正常，但项目面板无法稳定返回三层。
- 证据：`with_state` 的 `hooks_layer` 对 `ViewKey::Settings` 分支固定为 Project；`available_hooks_layers` 的 `HooksDraftLayer::User => vec![User]`；测试在 Hooks 页断言首载草稿来自项目层文件。
- 优化方向：按面板类型固定提供三层（浮层仅 User），当前层级只作为选中值；初始层级按规格回 User 与 JSON，并补关闭重开/重启与层切换往返测试。

### 重要｜Hooks 草稿任务身份只含面板，不同层级的载入/保存互相取消或静默失败

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/task_service.rs` 的 `TaskId::HooksDraftLoad(PanelId)`、`TaskId::HooksDraftSave(PanelId)`；`hooks_page.rs` 的 `request_hooks_draft`、`save_hooks_draft_for`、`HooksEditorState.loading`。
- 现象：同一面板的 User／Project／Local 三个草稿键共用同一后台任务 id。初始层载入未完成时切层，新载入的 `register` 返回 `Duplicate`，代码只移除 loading 并写日志；已经 spawn 的任务仍会执行并以同一 id 调用 `tasks.complete`，可能移除前一层在途任务的登记。用户看到层 pill 已切换但编辑区仍是上一层文本，后续输入写入一个不存在的草稿键而静默丢弃。不同层级的保存同理，第二个保存请求不会执行，也没有错误提示。
- 根因：任务身份粒度与草稿键粒度不一致；载入/保存失败分支只记日志，不进入用户可见错误；没有为每个草稿键维护任务或串行队列。
- 影响：层级切换、快速保存、加载重试会出现无反馈的丢失与非确定性取消，状态与磁盘事实不一致；测试都在 `run_until_parked` 后切层，未覆盖并发。
- 证据：`TaskId` 变体只携带 `PanelId`；`request_hooks_draft` 在注册失败时 `loading.remove` 后仅 `log::warn`；`save_hooks_draft_for` 注册失败同样只记日志；任务完成回执按同一面板 id 结算。
- 优化方向：任务身份包含完整草稿键或改为每面板的单一串行队列；注册失败、取消与覆盖必须回到可见错误并保留草稿。

### 重要｜statusline 用量桥接与信号通道整体缺失，注入/卸载契约不完整

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/CLAUDE.md`「statusline 用量信号分通道保留」；`docs/devPlan/design/hooks.md`「注入与卸载事务」；`slterm_app/src/ai_hook/local.rs` 的 `review_statusline_command`；`docs/featureSpec/panels/settings.md`「卸载只移除托管内容…按备份恢复或移除状态行桥接」。
- 现象：仓内只有对既有 `statusLine.command` 的可疑词审查与 Suspended 返回，没有任何地方写入 statusline 桥接脚本、备份原命令、透传原 statusline、写入用量信号或卸载时恢复。`ContextUsage` 等消费类型在当前树中不存在。安装器因此只覆盖 hooks 数组与 Codex notify，不覆盖合同声明的「脚本和状态行桥接」。
- 根因：statusline 通道随旧栈裁剪后没有按 CLAUDE.md 的「分通道保留」重新落地；审查门被单独保留，桥接与信号消费缺失。
- 影响：声明的注入/卸载能力不完整；依赖 statusline 的用量信号永远拿不到；卸载恢复器的状态行恢复分支无对象，后续接入缺少脚本与备份身份。
- 证据：`statusLine` 相关代码只出现在审查、错误文案与测试中；没有脚本模板、备份文件或信号文件写入；`review_statusline_command` 只在安装前读取原命令。
- 优化方向：把 statusline 桥接作为注入事务的显式材料（脚本、原命令备份、信号文件路径与恢复步骤）补齐，或由用户在文档层面明确砍除并同步 CLAUDE.md／design 合同。

### 重要｜追溯矩阵与进度把未接线的生产链记为「已自动化／已验收」

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 Settings 面板 Hooks 行；`docs/devPlan/progress.md` 的「Agent 与 Hooks」行；`docs/devPlan/plan.md` 的「Hooks 注入与卸载恢复」「hook 小进程事件桥」切片。
- 现象：追溯矩阵把 `hooks_post_commit_cleanup_failure_is_visible` 标为已自动化，但该例只验证枚举映射；`hook 小进程事件桥` 与 `config_guard` 没有生产接线却被整体记为已验收；进度没有区分「测试锚存在」与「生产入口存在」。
- 根因：验收口径以测试名存在为准，没有核验生产调用点、启动接线与真实端到端链路；计划切的交付语义被测试 scaffolding 替代。
- 影响：后续开发和发布判断建立在错误完成事实上；评审合同以 featureSpec 与 devPlan 为真值时，会与代码现状直接冲突。
- 证据：`progress.md` 的 Agent 与 Hooks 行宣称 hook 小进程桥锚全量自动化；`traceability.md` 的 Hooks 行引用上述纯映射用例；`ai_hook.rs` 与 `config_guard.rs` 的生产调用点缺失。
- 优化方向：把「生产接线」列为独立验收锚（启动链、管道服务生命周期、事件泵、自愈线程），验收证据需指出真实调用点与端到端测试，而不是只有两半各自的单元成例。

### 重要｜Hooks 页面错误文案硬编码中文，绕过 typed i18n 硬合同

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/editor.rs`（`load_user`、`load_at`、`save_editable`、`validate_editable`、`write_editable_at` 的错误字符串）；`slterm_app/src/app_shell/app_root/hooks_page.rs`（载入／保存错误、`hooks_layer_path` 错误）；`slterm_app` 层 `CLAUDE.md`「i18n 合同」。
- 现象：Hooks 页把 `HooksDocument` 与 `hooks_page` 返回的中文字符串直接显示在界面上；英文界面下同样显示中文。错误文本没有 typed `Message` 键，也没有经过 `language.format`。
- 根因：编辑域的错误类型是 `String`，页面直接回显；i18n 目录只为部分 Hooks 控件登记了消息，没有为文档加载/保存/校验诊断建立键。
- 影响：违反「运行时只经 Message 查询，禁止第二翻译通道」；用户看到不可本地化的安全配置错误，且错误文本成为临时拼装内容，难以稳定做可访问性与文案校验。
- 证据：`request_hooks_draft`、`save_hooks_draft_for`、`hooks_layer_path` 把 `Err(String)` 原样插入 `hooks_editor.errors`，`render_hooks_page` 直接 `child(message)`；`editor.rs` 的错误文本是硬编码中文；`diagnostic_text` 只覆盖三种解析诊断。
- 优化方向：为文档加载、保存、锁冲突、备份失败、外部修改与校验失败建立 typed 消息与参数槽，编辑域返回封闭错误类别而不是 UI 文案。

### 一般｜项目根不可访问时未禁用 Project/Local，保存可在失效根下重新创建配置

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `hooks_layer_path`；`slterm_app/src/ai_hook/editor.rs` 的 `HooksDocument::load_user`（无调用方）与 `load_at`；`docs/featureSpec/panels/settings.md`「无项目浮层只提供 User；项目根不可访问时 Project／Local 禁用并显示原因，User 仍可用」。
- 现象：Project/Local 路径由注册表项目根直接拼接，不检查根目录是否存在或可访问；`HooksDocument::load_at` 对不存在的文件返回空配置与无基线，保存时 `atomic_file::write` 会 `create_dir_all` 父目录，从而在已删除或不可用的项目根下重建 `.claude` 配置。带目录存在性守卫的 `load_user` 没有生产调用方。
- 根因：层级可用性只按当前层级生成 pill，没有按项目根真实可达性禁用；文档加载器没有区分「文件不存在」与「根不可访问」。
- 影响：用户可在失效项目根下写入新的 CLI 配置，违反项目根授权与「项目根不可访问时禁用」的行为合同；也可能误导后续历史归属。
- 证据：`hooks_layer_path` 只做 `registry().project(...)` 取根，无 `is_dir` 检查；`load_at` 在 `NotFound` 时返回空对象；`HooksDocument::load_user` 是唯一带目录检查的入口且未被引用。
- 优化方向：层级可用性由项目根真实访问结果驱动（不可用则禁用并显示原因），加载器对「根不存在/不可访问」与「文件不存在」分别分类，保存不得在失效根上 scaffolding。

### 一般｜Hooks 文件外部修改监听与「重新加载」入口缺失，注入完成后也不重读 User

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `request_hooks_draft`／`discard_hooks_draft_for`；`settings_agents.rs` 的 `finish_agent_hook_apply`；`docs/devPlan/design/hooks.md`「当前页可见且所属项目活跃时监听当前层文件…变化只显示『已外部修改』和重新加载」；`docs/featureSpec/panels/settings.md`「注入完成…若存在 User 草稿，重读前仍走外部修改守卫」。
- 现象：没有任何 Hooks 层文件 watcher，也没有「重新加载」操作；`request_hooks_draft` 在草稿已存在时提前返回，外部修改只能在下一次保存时被发现，错误文案却要求「重新加载」。AgentHooks 的安装/卸载成功后只重新探测安装状态，不重读 Hooks User 文档或提示草稿已过期。
- 根因：外部修改只实现为保存时的基线比较，没有实现可见性驱动的监听与显式重载；安装完成回执没有与 Hooks 编辑域汇合。
- 影响：用户看不到「已外部修改」的持续状态，只能尝试保存后失败；唯一能重新载入的「丢弃」会丢失草稿，且随后重新插入草稿（叠加前述阻断）。注入后打开 Hooks 页看到的是旧基线，保存被拒。
- 证据：`hooks_page` 与 `settings_agents` 中没有 watcher 或 reload 调用；`request_hooks_draft` 的提前返回使已有草稿无法刷新；`finish_agent_hook_apply` 只调用 `request_agent_hooks_inspect` 与 `reload_settings_snapshot`。
- 优化方向：在页面可见且项目活跃时监听目标层文件，显示外部修改状态并提供非破坏性的重新载入；安装/卸载完成后对相关 User 文档执行同一外部修改守卫与重读。

### 一般｜保存 UX 缺 User 任意命令警告、重启提示与成功后草稿清除

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `render_hooks_page`、`save_hooks_draft_for`；`slterm_app/src/ai_hook/draft_host.rs` 的 `mark_saved`；`docs/featureSpec/panels/settings.md`「保存与外部修改」「保存成功后清除草稿并提示重启对应 CLI 会话生效」。
- 现象：User 层保存前没有「hooks 可执行任意命令」的明确警告；保存成功后草稿被标记为干净但不清除，编辑区和层 pill 继续显示该层草稿；没有任何「重启对应 CLI 会话生效」的提示；保存过程没有「保存中／已保存／失败」状态，只有失败文本。
- 根因：保存回执只更新草稿快照，页面没有保存阶段状态；规格的警告与生效提示没有对应 UI。
- 影响：安全提示缺失，用户可能在不理解风险的情况下把任意命令写入用户级配置；保存成功后的状态与规格不符，且干净草稿继续参与项目生命周期守卫（与阻断问题互为因果）。
- 证据：`save_hooks_draft_for` 成功分支调用 `draft.mark_saved` 后只递增 generation；`render_hooks_page` 无警告/重启文案；测试 `hooks_page_loads_saves_external_guard_and_discards` 明确断言保存后草稿仍在且 `!dirty()`。
- 优化方向：User 保存前加入显式警告确认；保存成功后按规格清除草稿（或明确改为「保留干净草稿」并解除其对生命周期的影响）并显示重启提示；补齐保存中/已保存/失败与重试状态。

### 一般｜`draft::save_hooks_draft` 是未使用的第二套保存入口（无锁、无备份）

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/draft.rs` 的 `save_hooks_draft`；`slterm_app/src/ai_hook/editor.rs` 的 `write_editable_at`；测试 `hooks_draft_save_refuses_external_modification`、`hooks_draft_save_writes_only_with_matching_baseline`。
- 现象：生产保存只走 `HooksDocument::write_editable_at`；`save_hooks_draft` 不取操作锁、不写 `.bak`、不做子树替换，仅比较完整文件指纹后写整份字节。它只被自身测试调用。
- 根因：草稿解析模块保留了一条早期保存路径，未随编辑器写路径收编。
- 影响：形成第二套写语义，若未来被误用会绕过锁与备份；测试覆盖的是一段不存在的生产行为，给「已覆盖保存」造成错觉。
- 证据：仓内 `save_hooks_draft` 无生产调用方；`write_editable_at` 实现锁、基线、认领组保留、`.bak` 与子树替换；两条路径的失败分类与备份命名不同。
- 优化方向：删除未使用入口或收编到唯一保存服务，避免保留一条无锁的第二写通道。

### 一般｜托管 marker 契约未落地，只读/卸载归属依赖命令形态猜测

- 置信度：确认
- 位置：`slterm_hooks/src/lib.rs` 的 `MANAGED_MARKER_VALUE`、`is_managed_marker`；`slterm_app/src/ai_hook/local.rs` 的 `is_our_claude_hook`；`slterm_app/src/ai_hook/editor.rs` 的 `group_is_installer_owned`；`docs/devPlan/design/hooks.md`「含 slTerminal 托管标识的 handler 在 GUI 中只读」。
- 现象：注入条目只写 `type`／`command`／`timeout`／`args`，没有写入版本化 marker 字段；`is_managed_marker` 在应用侧无调用。只读与卸载归属完全由命令是否为合法 helper 可执行路径决定，命令文本中出现 helper 名字的其他形态只能靠 shell 解析规则排除。
- 根因：能力目录声明了托管标识，但安装器、编辑器与卸载器没有采用；归属判据仍是从命令形态反推。
- 影响：未来修改命令形态或迁移 helper 路径时无法用稳定标识区分自有条目；用户手动复制/改名 helper 命令的条目可能被误认领为托管，或在升级后失去只读与可卸载性。
- 证据：`MANAGED_MARKER_VALUE` 与 `is_managed_marker` 只在 `slterm_hooks` 测试中使用；`install_into` 生成的条目没有 marker 键；`is_our_claude_hook` 读取 `command`/`args` 做判断。
- 优化方向：让安装器写入并让编辑器/卸载器读取同一版本化 marker，命令形态降为兜底；读取未知 marker 版本时保留只读而非猜测。

### 一般｜Hooks CLI 名单与适配声明多源，违反 AgentCatalog 单点

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/integrations.rs` 的 `AGENTS`、`AGENT_NAMES`、`aliases`；`slterm_settings/src/agent_hooks.rs` 的 `AgentHook`；`slterm_agents` 的 `AgentKind`／AgentCatalog；`docs/devPlan/design/agents.md`「唯一 AgentCatalog…UI、Runtime 和恢复流程不得各自维护 CLI 名单」。
- 现象：Hooks 域同时维护 `AgentHook` 枚举、`AGENTS` 展示表、`AGENT_NAMES` 显示名表、`aliases` 可执行名表与 `capabilities_for` 的 source 字符串表；这些与 `AgentKind`／AgentCatalog 不是同一来源。`integrations.rs` 注释也承认身份枚举的收编尚未完成、落地后本表应改由身份单源供给。
- 根因：Hooks 域先于 AgentCatalog 收编落地，后续统一没有完成。
- 影响：新增 CLI 或改别名需要多处登记，容易出现顺序/身份错位（已由 AgentHooks 开关错位证明）；设计中的「适配器声明」也不存在单一清单，无法机械校验。
- 证据：`AGENTS` 与 `AgentHook::ALL` 顺序不同；`aliases` 自带可执行名与 Cursor/Grok 的路径归属规则；`capabilities_for` 以自由字符串开关；`slterm_hooks` 没有 CLI 适配器声明。
- 优化方向：由 AgentCatalog 提供 Hooks 适配声明与显示顺序，Hooks 域只消费身份与能力；删除本地平行表或用编译期/测试断言强制同源。

### 一般｜codex/kimi 写入使用本地 `write_atomic`，不复用 `atomic_file` 的 ACL 保留与失败处理

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/local.rs` 的 `write_atomic`；`local/codex_notify.rs` 的 `ensure_codex_notify`／`remove_codex_notify`；`local/kimi.rs` 的 `ensure_kimi_hooks_in`／`remove_kimi_hooks_in`；`slterm_app/src/atomic_file.rs` 的 `write_with_acl`。
- 现象：Codex `config.toml` 与 Kimi `config.toml` 用本地 `write_atomic`（固定进程号临时名 + rename），没有 `sync_all`、没有复制目标 ACL、没有 `atomic_file` 的临时文件清理与身份返回。`atomic_file::write` 已实现这些机制并被 Claude／Codex hooks／Cursor／managed files 使用。
- 根因：两处安装器保留了早期本地原子写，未收编到统一 `atomic_file`。
- 影响：替换用户配置后 ACL 可能被临时文件继承的父目录 DACL 改写；崩溃窗口缺少 fsync 保证；失败清理语义与其他安装器不一致，增加排查面。
- 证据：`write_atomic` 只做 `fs::write` + `fs::rename`；`atomic_file::write` 明确 `sync_all`、`copy_acl` 与临时文件清理；Kimi/Codex notify 走的是本地版本。
- 优化方向：统一走 `atomic_file::write`（或同等 ACL/同步语义的唯一原子写实现），删除本地变体。

### 一般｜helper 与宿主的载荷上限边界不一致，恰好压线的载荷被静默丢弃

- 置信度：确认
- 位置：`slterm_hook/src/lib.rs` 的 `read_payload`、`envelope`；`slterm_app/src/ai_hook/windows.rs` 的 `serve`、`dispatch`；`docs/devPlan/design/hooks.md`「单条输入上限 1 MiB」。
- 现象：helper 允许载荷字节数恰好等于上限，但随后追加协议头；宿主把「整条缓冲区」超过上限判定为超限并丢弃。于是 helper 认为合法的最大载荷在宿主侧必然被拒绝，且无侧信道以外的事后线索。
- 根因：两侧对「上限」计量对象不一致：helper 计量原始载荷，宿主计量信封（头+载荷）。
- 影响：边界值事件静默丢失，违背两侧共享的有界合同；测试只分别覆盖 helper 原始上限与宿主超限，没有跨进程边界值用例。
- 证据：`read_payload` 在 `len > MAX_PAYLOAD_BYTES` 时返回 `None`，等于上限则放行；`envelope` 在载荷前加头；`serve` 在 `buf.len() > (1 << 20)` 时停止读取，`dispatch` 对同条件返回 `Oversized`。
- 优化方向：统一上限计量为整条信封，或给协议头预留固定预算并在 helper 侧按同一预算截断/拒绝；补边界值端到端用例。

### 一般｜hooks 相关端到端、崩溃与部分失败测试缺口

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/e2e_tests.rs`、`slterm_app/src/ai_hook/tests.rs`、`lifecycle/tests.rs`、`slterm_hook/src/lib.rs` 的 `lifetime_tests`；`app_root/tests/settings_overlay.rs`；`installation_tests`。
- 现象：现有用例分别覆盖「宿主管道服务 + 模拟 helper 写」与「真实 helper 进程 + 哑接收端」，但没有真实 helper 进程连接宿主 `PipeServer` 的跨进程链路；没有注入提交边界崩溃、Codex 多文件部分失败、备份失败（草稿路径已有，安装路径没有）、锁冲突后的恢复对账；没有 Hooks 草稿在应用关闭/面板关闭/并发保存/层切换加载中的用例；Hooks 页测试没有 GUI 模式、层切换往返与 User 保存警告。
- 根因：测试按两半分别 mock/模拟，验收锚以存在为准，没有把合同声明的失败面映射为可执行故障注入。
- 影响：本报告中的生产接线、日志恢复、草稿生命周期、层级状态与边界值问题都不会被现有门禁发现；`hooks_post_commit_cleanup_failure_is_visible` 这类纯映射用例反而给出绿色信号。
- 证据：`e2e_tests.rs` 用 `std::fs` 直接写管道模拟 helper；`lifetime_tests` 只验证 helper 对哑接收端的退出；`installation_tests` 的恢复用例只调纯函数；Hooks UI 测试没有关闭守卫、并发或层切换中途用例。
- 优化方向：为每条合同失败面建立真实故障注入（进程崩溃点、部分写、锁占用、慢保存/并发编辑、关闭预检），并让端到端用例跑真实 helper→宿主链路。

### 合同冲突｜危险命令审查门的覆盖范围在 design 与模块 CLAUDE 之间不一致

- 置信度：确认
- 位置：`docs/devPlan/design/hooks.md`「注入与卸载事务」与 `slterm_app/src/ai_hook/CLAUDE.md`「决策与红线 · 审查门」。
- 现象：design 写「用户原有可疑下载或任意执行命令命中时先暂停并显示确认条，未确认前设置文件零写入」，没有限定只看 statusLine；模块 CLAUDE 写「审查门只覆盖 statusline 键，hooks 数组的 command 字段由 marker/归属指纹兜底」。同一行为在两份合同里覆盖范围不同。
- 根因：旧栈审查门从 statusline 单点扩展到 hooks 数组的合同没有同步；实现选择了窄口径，但 design 保留宽口径。
- 影响：无法判断 `install_gate` 未接线究竟是缺陷还是合同允许；后续实现可能在同一路径上重复引入/移除确认门。
- 证据：`installation::install_gate` 遍历 `hooks` 数组命令并返回 `RequireConfirmation`，但没有生产调用；`ensure_claude_hooks_in` 只检查 `statusLine.command`；模块 CLAUDE 明确把审查门缩到 statusline。
- 优化方向：由合同所有者裁决审查门覆盖范围，统一 design 与模块 CLAUDE，再决定 `install_gate` 是接线还是删除。

### 提示｜管道关闭取消用例可能在没有进入 `ReadFile` 时也通过

- 置信度：疑似
- 位置：`slterm_app/src/ai_hook/e2e_tests.rs` 的 `pipe_server_shutdown_cancels_a_client_blocking_readfile`。
- 现象：用例连接后固定 sleep 一小段来等待 serve 线程从 `ConnectNamedPipe` 进入 `ReadFile`，随后只断言 shutdown 在预算内返回。若调度让线程尚未进入 `ReadFile`，shutdown 可由自连唤醒成功，用例仍通过，未真正覆盖 `CancelSynchronousIo` 路径。
- 根因：缺少「服务线程已在读」的可观察同步点，测试用时间等待代替状态握手。
- 影响：取消路径回归可能不被发现，而它正是防关窗挂死的关键。
- 证据：注释自认「负载裕度（非断言放宽）」；断言只检查 `done_rx.recv_timeout` 成功。
- 优化方向：通过可观测握手（例如服务线程状态位或客户端写一半阻塞的可观察连接）确认进入阻塞读后再触发 shutdown。

### 提示｜helper 把原始 pane 环境值直接写进协议头，宿主对 pane 解析失败会回退焦点

- 置信度：疑似
- 位置：`slterm_hook/src/lib.rs` 的 `envelope`、`run`；`slterm_app/src/ai_hook/protocol.rs` 的 `parse_envelope`；`slterm_app/src/agent_host.rs` 的 `dispatch_lifecycle`。
- 现象：`SLTERM_PANE_ID` 的值未经校验/转义即拼入 `pane=` 头字段。环境值可被同终端内的进程改写；若包含空白或换行，可改变头字段切分；宿主 `pane.parse().ok()` 失败后得到 `None`，事件回退到焦点 pane。
- 根因：协议头只信任环境，没有与 `SLTERM_PROCESS_ID` 等实例身份绑定或做格式校验。
- 影响：同用户内可把事件重定向到焦点 pane，造成错误状态；风险受同用户管道访问限制，故不升级为高危，但违反了「pane 来自可信 PTY 环境」的隐含假设。
- 证据：`envelope` 直接 `format!("...pane={pane}{contract}\n")`；`run` 只 `into_string().ok()`；`parse_envelope` 用 `pane.parse().ok()`，失败为 `None`；`dispatch_lifecycle` 在 `None` 时使用焦点兜底。
- 优化方向：helper 对 pane 做数字校验并在非法时不写头；宿主在信封声明了 pane 但解析失败时丢弃而非回退焦点。

## 跨域观察

- 设置中心的整体页面模型与 featureSpec「按 CLI 分节、每个 CLI 含基础配置与 Hooks」不一致：当前只有全局 `AgentHooks` 页与单一 Hooks 页，没有 Agent 基础配置页，也没有按 CLI 的 Hooks 空态。本报告只记录 Hooks 影响面，基础配置页缺失属设置中心域。
- 工作树存在 `.claude/worktrees/agent-a8f5d5aaa635aa749` 的旧 Tauri hook 源码（含旧 statusline 桥接实现）。它不属于当前源码树，但包含与当前 Hooks 域同名的概念，若被搜索或门禁误扫会形成第二事实源。

## 存疑未证

- 各 provider 的配置格式与调用约定（Cursor `hooks.json`、Copilot/Grok managed JSON、Kimi `[[hooks]]`、Codex native hooks 与 notify、Claude exec-form `args`）只经过仓内生成器的形状测试，未与真实 CLI 版本核验；需要真实安装与回合事件实测。
- 若未来接线，`SLTERM_NOTIFY_PIPE`／`SLTERM_HOOK_EXE` 经 ConPTY 环境传递到 agent 及其 hook 子进程的真实行为未验证；部分 CLI 可能清洗环境。
- `spawn_pipe_server` 中 `unsafe { std::env::set_var }` 的单线程启动前提没有生产调用点可验证；若在 GPUI 线程启动后调用，存在环境读写竞态风险。
- `CreateNamedPipeW` 的空安全属性是否在所有 Windows 版本上都等价于「仅同用户」DACL，需要真实 ACL 检查；当前只有注释声明。
- 真实 helper 进程与宿主 `PipeServer` 的跨进程链路（含 1 MiB 边界、畸形头、超时）没有端到端用例；现有两半测试无法证明组合行为。
- `codex --version` 与 `codex features list` 的输出解析依赖当前上游格式；真实安装路径下的 `.cmd` shim、超时与权限行为未实测。
