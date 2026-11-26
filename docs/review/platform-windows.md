# Windows 平台集成、更新、套餐、通知、诊断、清理、语言与窗口几何审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`；只审当前工作树的 Windows 系统集成面，未运行 cargo、测试或网络。

## 覆盖与局限

- 已读：`slterm_app/src/platform/`（`windows_integration`、`startup`、`material`、`notifications`、`instance`、`close_dialog`、`distribution`、`elevation`、`update_installation`、`machine`、`locale`、模块 `CLAUDE.md`）；`tray.rs`、`taskbar.rs`、`notify.rs`；`windowing/` 与 `app_shell/windowing.rs`；`update_check`、`update_download`、`update_proxy`、`plan_balance`；`diagnostics.rs`、`cleanup.rs`；`i18n/` 与 `build/i18n.rs`；设置浮层、设置控件、设置页枚举、启动诊断；`app_shell/mod.rs`、`app_root` 的壳事件、关闭、应用关闭、更新守卫与几何观察；`background_tasks.rs`、`session.rs`；相关测试与 `docs/devPlan`、`docs/featureSpec`、`.claude/docs/testing/exemptions.md`。
- 未读或只按调用关系核对：`platform/file_operations`、`process`、`process_output`、`process_snapshot`、`dpapi`、`credentials` 的内部实现；托盘图标真实 DPI 呈现与 WinRT 桌面会话；真实 setup.exe、真实凭据库、真实任务栏/托盘视觉。
- 局限：合同禁止构建、运行与网络，因此涉及桌面会话、安装器、真实凭据、真实代理与真实 HWND 的行为只能到代码证据或列存疑，不把单元测试通过当作运行态成立。

## 问题

### 阻断｜更新链在运行态没有入口

- 置信度：确认
- 位置：`slterm_app/src/update_check.rs` 的 `start_gpui`、`check_now`、`deliver_cached`、`deliver_online`；`slterm_app/src/app_shell/shell_events.rs` 的 `SltermShellEvent`；`slterm_app/src/app_shell/app_root/update_install.rs` 的 `begin_update_install`；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`；`slterm_app/src/lib.rs` 的模块 dead-code 声明。
- 现象：应用启动链没有调用 `update_check::start_gpui`；壳事件枚举没有 `UpdateAvailable`；更新安装入口 `begin_update_install` 和 `cancel_update_install` 标注为等待 UI 接线；`auto_download_updates` 没有任何消费点；`update_download::hydrate` 仅被未接线的检查入口与 `apply_scheduled` 调用。
- 根因：更新模块按切片实现了检查、下载、handoff 与设置键，但没有任何壳层协调器把它们串成运行态流程；模块级 `#[allow(dead_code)]` 掩盖了缺失调用方。
- 影响：自动检查不执行，手动检查没有入口，下载与安装提示不存在，安装前关闭确认与启动对账无法从产品路径触发；`system-integration` 的更新验收不成立，追溯矩阵与进度的“更新已自动化/已验收”没有对应运行态证据。
- 证据：全仓检索 `start_gpui` 只有定义；`begin_update_install` 在 `update_install.rs` 内只有定义，调用点标注为后续 UI；`settings_controls.rs` 只暴露 `auto_check_updates`/`auto_download_updates` 开关，未读取运行值执行业务；`lib.rs` 对 `update_check` 与 `update_download` 的注释写明生产消费仍待接入。
- 优化方向：把更新检查、下载、提示与安装预约收编到启动链唯一协调器，经壳事件回前台；不得以模块内测试替代运行态接线。

### 阻断｜套餐余量功能在运行态不存在

- 置信度：确认
- 位置：`slterm_app/src/plan_balance/service.rs` 的 `PlanBalanceService`；`slterm_app/src/plan_balance/client.rs` 的 `PlanBalanceClient`；`slterm_app/src/plan_balance/mod.rs` 的 `PlanBalanceCredentials`；`slterm_app/src/background_tasks.rs` 的 `PLAN_BALANCE`；`slterm_app/src/settings_state.rs` 的 `SettingsPageId`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的字段表。
- 现象：`PlanBalanceService`、`PlanBalanceClient`、`PlanBalanceCredentials` 没有生产构造点；`PLAN_BALANCE` 任务只有注册表条目，`executor` 为空，除测试外没有 `subscribe` 或 `run_once`；设置页没有套餐来源、专用 token 输入、已配置状态或页脚展示；`invalidate`、`begin_auto`、`begin_manual_refresh`、`due` 仅被单元测试调用。
- 根因：套餐解析、客户端与状态机已实现并被 dead-code 豁免，但没有调度订阅、凭据 UI 与展示面把它们接入应用。
- 影响：用户无法配置套餐 token，无法看到 DeepSeek/Kimi 结果，查询永不调度；`system-integration` 的套餐来源与后台周期验收不成立；凭据代次、冷却、过期与隐藏调度只在测试时钟中成立。
- 证据：全仓检索 `PlanBalanceService`、`PlanBalanceClient`、`PlanBalanceCredentials` 在 `slterm_app/src/plan_balance` 之外无引用；`PLAN_BALANCE` 的 `subscribe` 只出现在 `background_tasks.rs` 测试；`settings_controls.rs` 没有套餐字段，`settings_state.rs` 没有应用页/后台任务页。
- 优化方向：建立唯一的套餐运行协调器，由同一调度订阅驱动自动查询，由设置页驱动凭据与手动刷新，结果只经该协调器回前台；不能把服务测试当作功能接线。

### 阻断｜诊断与清理没有产品面

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `DiagnosticReport`、`DiagnosticSnapshot`、`DiagnosticSection`；`slterm_app/src/cleanup.rs` 的 `CleanupService::plan`、`CleanupService::execute`；`slterm_app/src/settings_state.rs` 的 `SettingsPageId`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的字段表；`slterm_app/src/lib.rs` 的 dead-code 声明。
- 现象：`DiagnosticReport` 与 `CleanupCandidate` 没有生产构造点；设置页没有诊断预览、复制、导出、自检或分类清理入口；`DiagnosticSection` 没有“完整路径”类别，也没有显式选择完整路径的端口；清理没有所有者提供候选与活跃代次的接线，也没有二次确认 UI。
- 根因：两个服务只有单元层 API，缺少应用级诊断协调器、设置页入口与各恢复材料所有者的候选登记。
- 影响：诊断包无法生成、预览、复制或导出；环境自检不存在；清理无法列出候选、二次确认或逐项展示部分成功；`system-integration` 的诊断与清理验收不成立。
- 证据：全仓检索 `DiagnosticReport`、`CleanupCandidate`、`CleanupService` 只在 `diagnostics.rs`、`cleanup.rs` 与测试中出现；`lib.rs` 注释写明“消费点 = 诊断/清理页的计划与执行接线”；设置页枚举没有诊断页。
- 优化方向：由应用级诊断协调器统一生成白名单 DTO、脱敏预览、复制与原子导出，并由各所有者登记清理候选与活跃代次；设置页只驱动该协调器。

### 阻断｜通知事件源与托盘快照没有接入

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `spawn_gpui_server`；`slterm_app/src/notify.rs` 的 `deliver_gpui`、`deliver_gpui_with_choices`、`from_accepted_hook`；`slterm_app/src/agent_host.rs` 的 `AgentHub::dispatch_lifecycle`；`slterm_app/src/tray.rs` 的 `update`、`Shared::apply`；`slterm_app/src/gpui_terminal/view.rs` 的 `TermEvent::Bell` 分支。
- 现象：hook 命名管道服务只在测试中启动；`AgentHub::dispatch_lifecycle` 没有生产调用；通知漏斗的 AI 投递入口没有生产调用；`tray::update` 没有调用方，托盘共享清单始终为空；终端 BEL 分支被显式忽略。
- 根因：hook 事件泵、AgentHub、通知漏斗与托盘快照各自实现，但没有一个启动链持有者把它们串起来。
- 影响：任务完成、等待输入、权限请求与错误四类通知不会产生；托盘 agent 列表、attention 橙点、提示文案与菜单语言始终停在初始空态；通知类别的调用链无法从产品路径触达。
- 证据：`spawn_gpui_server` 的调用只出现在 `ai_hook/e2e_tests.rs`；`deliver_gpui`/`from_accepted_hook` 在 `notify.rs` 外无调用；`AgentHub::dispatch_lifecycle` 在 `agent_host.rs` 外无调用；`tray::update` 在 `tray.rs` 外无调用；`TermEvent::Bell => {}`。
- 优化方向：启动链唯一持有 hook 管道服务，事件排空后交给 AgentHub 与通知漏斗，托盘快照由同一 AgentHub 事实源按节拍发布；BEL、OSC 与 hook 源必须汇入同一漏斗。

### 阻断｜系统关机与注销不走统一关闭链

- 置信度：确认
- 位置：`slterm_app/src/app_shell/windowing.rs` 的 `open_initial_window`（仅注册 `on_window_should_close`）；`slterm_app/src/app_shell/app_root/close_coordinator.rs` 的 `request_close`；`slterm_app/src/app_shell/app_root/application_close.rs` 的应用退出段。
- 现象：应用只处理窗口关闭与托盘退出；没有 `WM_QUERYENDSESSION`/`WM_ENDSESSION` 处理，也没有系统关机前冲刷 Document、Hooks、PTY 与最终状态的入口。钉版 GPUI 的 Windows 事件泵只分派 `WM_CLOSE`/`WM_DESTROY`。
- 根因：统一关闭链只在用户关窗路径注册，OS 会话结束路径没有平台钩子。
- 影响：注销、重启或关机时进程被系统终止，脏 Document 与未提交快照按崩溃处理；与项目“关窗/托盘退出/系统关机同一关闭链”的硬约束冲突，存在数据丢失风险。
- 证据：`open_initial_window` 只设置 `window.on_window_should_close`；全仓无 `WM_QUERYENDSESSION`、`WM_ENDSESSION` 或等价会话结束处理；钉版 `gpui_windows` 事件泵仅处理 `WM_CLOSE` 与 `WM_DESTROY`。
- 优化方向：在平台封装层接入会话结束消息并把请求转成同一 `CloseCoordinator` 应用退出作用域；系统不给完成窗口时按既有“知情放弃/对账”语义记录，不得伪装正常退出。

### 重要｜通知前台状态从未写入，失焦门控与闪烁规则失效

- 置信度：确认
- 位置：`slterm_app/src/notify.rs` 的 `set_foreground`、`FOREGROUND`、`deliver_with`、`Funnel::plan_with`；`slterm_app/src/app_shell/app_root/mod.rs` 的 `observe_focus`；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`。
- 现象：`set_foreground` 只被 `notify.rs` 自身测试调用；启动链没有订阅窗口激活/失焦把状态写入 `FOREGROUND`；`deliver_with` 的 `foreground()` 恒为 false。
- 根因：设计声称“workspace 的 activation observer 调用”，但该 observer 没有实现或没有接到通知中心。
- 影响：窗口聚焦时通知仍可按失焦路径闪烁或弹 toast；重新聚焦时清等待输入提示的路径永不触发；`system-integration` 的“任务栏闪烁只在窗口失焦时触发”与通知验收不成立。
- 证据：`set_foreground` 在 `notify.rs` 之外无引用；`observe_focus` 只建立 FocusManager 订阅，没有调用通知中心；`integration_notification_taskbar` 直接调用 `set_foreground` 造状态，未验证生产接线。
- 优化方向：把窗口激活/失焦观察接到通知中心唯一写入口，并让漏斗门控使用该真实状态。

### 重要｜应用内错误不可见，错误类别不进入任务栏错误态

- 置信度：确认
- 位置：`slterm_app/src/notify.rs` 的 `deliver_with`、`Notification::ProcessAlert`、`Notification::is_failure`、`taskbar_signals`；`slterm_app/src/app_shell/app_root/events.rs` 的 `notify_workspace_write_failed`、`notify_document_save_error`、`notify_document_save_state`、`notify_application_close_decision_required`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `delivery_switches` 消费链。
- 现象：运行期错误与保存失败统一走 `deliver_startup_summary(ProcessAlert)`；`plan_with` 在桌面开关关闭时直接返回、不产生任何界面内提示；`ProcessAlert` 不被 `is_failure` 认同，任务栏只收到 `AttentionCleared`/`ErrorCleared`；启动汇总豁免前台门控。
- 根因：没有应用内通知存储或展示面，错误被当作启动汇总 toast 发送；错误类别没有映射到任务栏错误态。
- 影响：关闭桌面通知后应用内错误完全不可见，只剩日志；窗口聚焦时运行期错误仍弹系统 toast；错误不会把任务栏置为错误态，反而清除已有错误；`system-integration` 的“桌面总开关只控制系统通知、应用内错误保留”与“错误优先”不成立。
- 证据：`delivery_switches` 只由 `plan_with` 消费，进程错误调用点全部使用 `deliver_startup_summary`；`ProcessAlert` 不属于 `is_failure`；`taskbar_signals` 对非失败非等待输入的通知返回清除信号；全仓没有应用内通知视图。
- 优化方向：建立独立的应用内通知/错误保留面，桌面开关只决定系统 toast；错误类别必须映射到错误优先级，且清除只能由对应资源的解决/关闭触发。

### 重要｜任务栏聚合没有资源身份，低优先级完成会清除其他 pane 的错误与等待

- 置信度：确认
- 位置：`slterm_app/src/taskbar.rs` 的 `TaskbarState`、`apply_signal`、`progress`；`slterm_app/src/notify.rs` 的 `taskbar_signals`、`deliver_with`。
- 现象：任务栏状态只有进程级 `busy`、`attention`、`error` 布尔值与单个 OSC 来源；任何“回合完成”类通知都会发送 `AttentionCleared` 与 `ErrorCleared`，没有 pane/resource 身份与代次。
- 根因：聚合器只存最终布尔态，不记录每个错误/等待态来自哪个 pane、哪个资源代次。
- 影响：pane B 的完成会清除 pane A 的错误或等待提示，错误优先级“不得被低优先级覆盖”在多 pane 场景不成立；进程级错误与单 pane 状态互相污染。
- 证据：`TaskbarState` 字段只有 `current`、`busy`、`attention`、`error`、`applied`；`taskbar_signals` 对普通完成固定返回两条清除信号；`deliver_with` 对每条通知都调用该函数。
- 优化方向：按资源身份与代次维护错误/等待态集合，派生优先级；清除只移除对应来源，不能由其他 pane 的完成顺带清除。

### 重要｜更新预约在崩溃或知情放弃关闭时仍保留

- 置信度：确认
- 位置：`slterm_app/src/update_download/handoff.rs` 的 `UpdateInstallGuard::arm`、`UpdateInstallGuard::install_scheduled`、`UpdateInstallGuard::drop`；`slterm_app/src/app_shell/app_root/application_close.rs` 的 `finish_application_global`、`finish_recovery_global`、`finish_update_install`。
- 现象：`arm` 立即把资产写入 `updates/install-next.json`；`install_scheduled` 只清除内存 `armed` 标志，不落任何“最终提交已完成”的磁盘证据；进程在关闭段崩溃时 `Drop` 不执行，预约保持；`finish_application_global` 在 `FinalShutdownOutcome::Abandoned` 与 `Completed` 分支都调用 `finish_update_install`，恢复关闭路径也不区分结果。
- 根因：安装预约的“待定/已确认”状态只存在于内存守卫，持久文件在确认前就已是可执行预约；放弃关闭被当作正常完成处理。
- 影响：最终状态提交失败、用户知情放弃或关闭段崩溃后，下次启动仍会安装更新，绕过“只有统一关闭最终提交完成才保留预约”的守卫；`system-integration` 的更新关闭守卫验收不成立。
- 证据：`arm` 与 `install_scheduled` 的磁盘行为不对称；`finish_application_global` 的 `Abandoned` 分支与 `Completed` 分支最终都走到 `finish_update_install`；`recover_stale_handoffs` 只处理事务目录，不处理 `install-next.json`。
- 优化方向：预约文件应区分“待定”与“已确认”两种持久状态，确认只在最终提交完成后原子提升；放弃、失败与异常退出都收敛为未确认并撤销安装意图。

### 重要｜启动安装依赖已退役的 session.json，恢复票没有消费方

- 置信度：确认
- 位置：`slterm_app/src/update_download/handoff.rs` 的 `apply_scheduled_at`、`restore_ticket_at`、`acknowledge_restore`、`PreparedUpdate::commit`；`slterm_app/src/session.rs` 的模块说明与保存函数；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`。
- 现象：`apply_scheduled_at` 从 `<local>/session.json` 读取旧 `Session`，没有快照即报“缺少可恢复的工作区快照”；`PreparedUpdate::commit` 把旧 `Session` 写入 `workspace.json`；生产启动链没有调用 `restore_ticket_at` 或 `acknowledge_restore`。`session.rs` 明确说明生产 AppRoot 不读取该格式，且全仓没有生产写入 `session.json` 的调用方。
- 根因：更新 handoff 仍使用旧会话模型，而工作区恢复已迁移到 SQLite 项目快照；启动恢复票没有被新产品启动链接入。
- 影响：即使更新链完成 UI 接线，退出后安装也会因缺快照失败；成功或回滚后的新版本不会恢复工作区、不会留档防重放；启动对账验收不成立。
- 证据：`apply_scheduled_at` 的成功前置是 `load_from(...session.json)` 非空；`session.rs` 注释写明生产不读取旧格式；`restore_ticket` 与 `acknowledge_restore` 在 `handoff.rs` 外无调用；`lib.rs` 对 `session` 标注旧 schema 等待重写。
- 优化方向：安装预约与恢复票必须改用当前 `ProjectSnapshot`/工作区库的持久事实；启动链在维护期对账后显式消费恢复票并留档，不能保留并行旧会话通道。

### 重要｜托盘可用性判定早于建窗，失败后没有恢复节拍

- 置信度：确认
- 位置：`slterm_app/src/tray.rs` 的 `ensure_thread`、`available`、`thread_started`、`set_enabled`、`update`；`slterm_app/src/platform/windows_integration.rs` 的 `start_hidden_with`；`slterm_app/src/app_shell/mod.rs` 的 `starts_hidden`。
- 现象：`ensure_thread` 在 `spawn` 之前把 `THREAD_STARTED` 置真；`available()` 只读该原子量，因此启动期可能在建窗之前判定托盘可用；`CreateWindowExW` 失败会把标志复位，但 `set_enabled` 只在启动与设置变更时调用，`tray::update` 没有生产调用方，1 Hz 重试节拍不存在。
- 根因：可用性信号表达“启动请求已发出”，而不是“托盘窗口已创建且图标已挂载”；失败路径没有持续重试者。
- 影响：静默启动可能在托盘实际不可用时隐藏窗口；失败后没有图标入口，用户只能通过第二次启动移交恢复；`.claude/docs/testing/exemptions.md` 对该失败路径声称的 1 Hz 快照重试没有对应调用链。
- 证据：`THREAD_STARTED.swap(true)` 发生在 `Builder::spawn` 之前；`available()` 只读原子量；`tray::update` 在 `tray.rs` 外无调用；豁免表写明“重试节拍由 1 Hz 快照投递覆盖”。
- 优化方向：可用性必须以窗口创建成功与图标挂载成功为准；失败后由同一看门狗节拍持续重试，且静默启动只在该真实状态成立时隐藏窗口。

### 重要｜合同冲突：静默启动的触发范围

- 置信度：确认
- 位置：`docs/featureSpec/system-integration.md` 的“托盘与启动”；`docs/devPlan/design/windows.md` 的“WindowsIntegration 与窗口”；`slterm_app/src/platform/CLAUDE.md` 的“静默驻留依赖托盘可用”；`slterm_app/src/app_shell/mod.rs` 的 `starts_hidden`、`run_shell_once`；`slterm_app/src/platform/startup.rs` 的 `start_hidden_for`、`current_shortcut`。
- 现象：featureSpec 与设计都写“静默启动只用于登录自启”，平台 CLAUDE 与实现把触发条件定义为常规启动、设置两条件与端口托盘可用的合取；没有任何登录自启来源判定，启动项快捷方式也不携带区分参数。
- 根因：合同层对“谁可以触发静默启动”没有统一条件；实现采用了较宽的公式，且未把来源信息带入启动链。
- 影响：用户手动首次启动也可能被隐藏；登录自启与普通启动没有区分，合同双方都可被引用为正确，验收无法稳定判定。
- 证据：`starts_hidden` 的 `restore_tabs` 输入只表示 `regular_start`（无显式 CLI 目录），不是登录标记；`current_shortcut` 生成的 `arguments` 为空，快捷方式与手动启动不可区分；平台 CLAUDE 公式只有设置与托盘条件。
- 优化方向：先统一合同：若采用“仅登录自启”，启动项必须携带可验证来源且普通启动不入静默分支；若保留宽公式，featureSpec 与设计需同步改写并新增对应验收。

### 重要｜诊断脱敏不覆盖真实 Runtime token 形态，验收锚只证明了假值前缀

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `redact`、`consume_secret`、`runtime_token_absent_from_materials`；`slterm_app/src/local_control/credentials.rs` 的 `generate_credential_with`、`validate_credential`。
- 现象：`redact` 只识别 `sk-`、`Bearer` 与 `token=/token:/secret=/password=` 等带键形态；真实的 Runtime 凭据是 64 位小写 hex，不带 `sk-` 前缀。验收用例把 token 写成 `sk-test-runtime-token-value`，三条日志形态都由 `sk-` 规则命中，未证明裸 hex token 会被替换。
- 根因：脱敏规则按常见 API key 形态设计，与 Runtime 凭据的实际格式不一致；测试输入选择了会被前缀规则命中的假值。
- 影响：一旦其他模块把 Runtime token 以裸值或非敏感键写入日志，诊断导出可能携带真实凭据；当前“Runtime token 不在材料中”的自动化锚不能证明该边界，与凭据红线要求不符。
- 证据：`generate_credential_with` 生成 64 位 ASCII hex；`validate_credential` 要求恰好 64 位 hex；`redact` 的分支只处理 `sk-`、`Bearer` 与固定键名；用例 token 以 `sk-` 开头并通过三条形态。
- 优化方向：脱敏规则应覆盖实际凭据格式（含裸 64 位 hex 与 JSON 非键值形态），验收用例应使用真实格式的假值而非会被前缀规则命中的 token；日志采集仍应维持白名单来源。

### 重要｜追溯矩阵把服务层或注册表层用例标成运行态集成

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的“系统集成”“跨域集成场景”“导航树”“Settings 面板”表；`slterm_app/src/background_tasks.rs` 的 `integration_plan_schedule`、`settings_plan_schedule_commit_and_visibility`、`plan_balance_runs_while_navigation_hidden`；`slterm_app/src/update_download/handoff/tests.rs` 的 `update_install_uses_close_guard`、`integration_update`；`slterm_app/src/notify.rs` 的 `integration_notification_taskbar`；`slterm_app/src/cleanup.rs` 的 `integration_diagnostics_cleanup`；`slterm_app/src/app_shell/app_root/tests/language.rs` 的 `i18n_switch_preserves_workspace`、`integration_language`。
- 现象：多个矩阵行引用真实存在的例名，但用例只操作通用任务计数器、直接调用任务栏信号、直接构造清理候选、或只比较项目/终端标识；没有对应导航隐藏、设置页提交、产品更新链、通知投递、诊断复制导出、未保存文本语言切换的前置条件与可观察结果。
- 根因：集成用例以“模块内服务能跑”替代“产品运行态可触达”，例名沿用集成语义但断言停在单元层。
- 影响：全量测试全绿不能证明系统集成验收；矩阵“已自动化”与进度“Windows 与发布已验收”对运行态给出错误结论；缺失入口被错误的核销状态掩盖。
- 证据：`plan_balance_runs_while_navigation_hidden` 不构造任何导航状态；`settings_plan_schedule_commit_and_visibility` 直接调用 `apply_config`；`update_install_uses_close_guard` 只断言预约文件与守卫文件；`integration_notification_taskbar` 直接调用 `apply_taskbar_signal` 与 `set_foreground`；`integration_diagnostics_cleanup` 在清理后另建诊断报告，不验证诊断服务集成；`i18n_switch_preserves_workspace` 只加入终端，没有未保存 Document。
- 优化方向：矩阵状态只按产品入口可触达的用例核销；服务层用例保留为单元锚，但不得占用集成场景名；缺失入口先修接线再核销。

### 重要｜设置中心缺少应用页与后台任务页，多个系统集成入口无处可达

- 置信度：确认
- 位置：`slterm_app/src/settings_state.rs` 的 `SettingsPageId`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `FIELDS`、`fields_for_page`；`docs/featureSpec/panels/settings.md` 的“应用页”“后台定时任务页”“套餐来源”。
- 现象：设置页枚举与字段表只有外观、滚动、交互、会话、系统、AgentHooks、Hooks；没有应用页，也没有后台定时任务页。语言、更新开关、托盘与通知散落在会话页/系统页；套餐来源、诊断、清理、更新状态、Runtime API、备份与恢复没有对应界面。
- 根因：设置中心停留在较早的页面结构，系统集成功能按字段零散接入，没有按所有者页面落位。
- 影响：更新状态/手动检查、套餐 token 与刷新、诊断复制与清理、Runtime API 只读状态、备份恢复均无用户入口；即使后端服务接线，也无法完成产品验收；设置页导航契约与 featureSpec 不一致。
- 证据：`SettingsPageId` 的页面集合无 App/BackgroundTasks；`FIELDS` 无套餐、诊断、清理、备份字段；`featureSpec/panels/settings.md` 要求应用页单页分节并提供上述入口。
- 优化方向：恢复应用页与后台定时任务页作为系统集成所有者的统一入口；字段与命令只驱动对应后端协调器，不新增第二套状态。

### 重要｜套餐周期契约与后台任务注册表不一致

- 置信度：确认
- 位置：`slterm_app/src/background_tasks.rs` 的 `PLAN_BALANCE` 条目；`slterm_app/src/plan_balance/service.rs` 的 `DEFAULT_PERIOD`、`MIN_PERIOD`、`MAX_PERIOD`；`docs/featureSpec/panels/settings.md` 的“后台定时任务页”；`docs/devPlan/design/windows.md` 的“套餐余量”。
- 现象：后台任务注册表把套餐周期定义为最小 60 秒、默认 300 秒、最大 3600 秒；套餐服务与规格要求默认 60 秒、范围 30–3600 秒。
- 根因：调度注册表与服务常量各自维护契约数值，没有单一来源。
- 影响：一旦按注册表接线，用户看到的默认周期与可配置下限违反 featureSpec；服务测试会通过但产品行为不同；周期修改与过期判定的实际周期不一致。
- 证据：`PLAN_BALANCE` 的 `interval_min`、`interval_default` 与 `PlanBalanceService` 的常量分组不同；设置页规格与设计文档写明默认 60、下限 30。
- 优化方向：套餐周期只在服务或注册表之一拥有契约值，另一侧引用同一值；接线时用跨层测试锁定默认值与下限。

### 一般｜任务栏进度在命令结束与终端退出时不清理

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/terminal_events.rs` 的 `on_terminal_event`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `activate_view`；`slterm_app/src/app_shell/app_root/terminal_close.rs` 的 `destroy_closed_terminal`；`slterm_app/src/taskbar.rs` 的 `TaskbarState::clear`。
- 现象：任务栏进度只在面板关闭与显式切换焦点时调用 `clear_taskbar_for`；`Exited` 只提交运行意图，`CommandFinished` 只更新 Agent 状态；`TaskbarState::clear` 没有生产调用方。
- 根因：OSC 9;4 清除边只信任生产端发 `0`，没有把终端退出/命令结束纳入清除链。
- 影响：构建工具或 agent 异常退出、取消或未发清除序列时，任务栏残留进度条，违反“任务结束、取消或应用退出后清理对应进度”。
- 证据：`on_terminal_event` 的 `Exited` 分支不调用 `clear_taskbar_for`；`CommandFinished` 分支只调用 `sync_agent_command`；`clear_taskbar_for` 的调用点只在视图激活与面板销毁。
- 优化方向：把终端退出、命令结束与取消并回同一任务栏清除边；清除仍只作用于对应来源 pane。

### 一般｜通知动作按钮丢弃选择身份

- 置信度：确认
- 位置：`slterm_app/src/notify.rs` 的 `deliver_with`；`slterm_app/src/app_shell/shell_events.rs` 的 `NotificationChoice`；`slterm_app/src/platform/notifications/windows.rs` 的 `show_request` 激活回调。
- 现象：每个 toast 动作按钮的 `activate` 闭包都发送 `SltermShellEvent::NotificationChoice { pane_id }`，事件没有动作下标或选项文本；壳事件处理只聚焦 pane 并写审计。`show_request` 解析出 `choice-<index>` 后只调用对应闭包，闭包不读该下标。
- 根因：动作按钮被建模为“回窗”而不是“回传权限决定”，事件载荷缺少选择身份。
- 影响：权限请求或确认类通知的动作按钮无法把用户选择交给 confirmation 链；即使用户点了“允许/拒绝”，应用只能回到对应终端，无法执行选择。
- 证据：`labels.into_iter().map(...)` 的闭包忽略 `label`；`SltermShellEvent::NotificationChoice` 只有 `pane_id`；`handle_shell_event` 对 `NotificationChoice` 只调用 `focus_notification_pane` 与审计。
- 优化方向：动作事件必须携带稳定的请求身份与选项身份，回传后由 confirmation 所有者校验请求代次再执行；回窗与选择执行保持分离。

### 一般｜失败冷却只覆盖 AI issue，不覆盖命令失败

- 置信度：确认
- 位置：`slterm_app/src/notify.rs` 的 `PaneFailureThrottle::accepts`、`Notification::is_failure`、`Notification::CommandFailed`。
- 现象：`is_failure` 包含 `CommandFailed`，但 `accepts` 仅对 `AiTurnIssue` 记账；命令失败直接返回 true，只受全局 3 秒与 pane 同键节流约束。
- 根因：失败 episode 只按 AI provider 报错设计了键，没有为命令失败建立同样的来源/错误冷却。
- 影响：持续失败的命令或 shell 集成长时间每 3 秒弹一次通知/闪烁，违反“系统通知不重复骚扰”和失败冷却约定。
- 证据：`accepts` 中非 `AiTurnIssue` 的失败直接返回 true；`RecentFailure` 只从 `AiTurnIssue` 构造；命令失败没有 30 秒 episode 记录。
- 优化方向：把失败 episode 提升为类别无关的来源身份冷却，命令失败与 provider 失败共用同一策略，恢复事件仍结束 episode。

### 一般｜诊断健康状态是硬编码常量

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `DiagnosticSnapshot::current`、`DiagnosticSnapshot`。
- 现象：`healthy` 恒为 true，`last_error_code` 恒为 None；报告生成不读取任何真实健康状态或最近错误码。
- 根因：健康快照尚无真实来源，字段以占位值满足结构。
- 影响：诊断包会向用户与支持方宣称健康，即使工作区维护失败、服务未就绪或出现错误；`system-integration` 的“健康状态与错误码”白名单材料名不副实。
- 证据：`current` 直接写入 `last_error_code: None` 与 `healthy: true`；没有健康端口或错误码输入参数。
- 优化方向：健康与错误码必须来自真实运行快照（启动阶段、服务就绪、最近封闭错误码），缺失时明确为未知而不是健康。

### 一般｜语言选项显示存储代码而非语言专名

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `LANGUAGES`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的选项渲染；`slterm_settings` 的 `LanguagePref::native_name`。
- 现象：语言选项除 System 使用翻译文案外，`zh-CN` 与 `en-US` 使用 `literal`，直接显示存储代码；`LanguagePref` 已有语言专名接口但设置页未使用。
- 根因：设置控件选项表把语言身份值当成显示值。
- 影响：用户看到 `zh-CN`/`en-US` 而不是 `简体中文`/`English`，与 featureSpec 的语言选项展示不符；语言专名与存储值的单点映射被绕过。
- 证据：`LANGUAGES` 的两个语言项为 `literal("zh-CN")`、`literal("en-US")`；`choice_is_selectable` 与渲染只按 `label` 是否存在决定文案；`native_name` 没有设置页消费点。
- 优化方向：语言选项的显示名从语言注册表的 native name 取，存储值只作提交值，不复制第二份显示文案。

### 一般｜运行期语言切换的验收用例没有未保存文本

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/tests/language.rs` 的 `i18n_switch_preserves_workspace`、`integration_language`；`docs/featureSpec/system-integration.md` 的“界面语言”验收判据。
- 现象：用例只创建项目与终端，比较项目实体、快照代次、布局与终端数量；没有打开 Editor/Markdown、写入未保存正文或断言重绘后草稿仍在。
- 根因：测试把“不重建工作区”收窄成“不重建终端/项目”，没有覆盖用户最关心的未保存内容。
- 影响：即使语言切换路径误重建编辑器或丢失草稿，当前用例仍会通过；featureSpec 要求的“存在未保存文本且现场不重建”没有被验证。
- 证据：用例构造只有 `add_project`、`new_terminal`；断言项不含 DocumentRegistry、编辑器实体或脏态；`integration_language` 只检查通知文案与全局快照。
- 优化方向：语言切换用例必须包含带未保存正文的 Document 与打开的面板，验证实体、代次、草稿文本与焦点均保持不变。

### 一般｜托盘不可用时静默启动选项仍可提交

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `apply_settings_field`、`integration_note_message`；`slterm_app/src/platform/windows_integration.rs` 的 `settings_note`、`choice_is_selectable`。
- 现象：托盘不可用时只在字段下显示 `TrayUnavailable` 说明；`silent_start` 仍可点击并写入设置；`choice_is_selectable` 只处理材质档位。产品启动会回退为可见窗口，但设置值保留为已启用。
- 根因：把“说明回退”实现成提示，而没有实现 featureSpec 要求的“相关选项禁用”。
- 影响：用户可在托盘不可用时开启静默启动，界面却无法兑现；设置值与实际行为不一致，下一次启动仍按同一回退链路。
- 证据：`apply_settings_field` 对 `silent_start` 没有可用性校验；`settings_note` 只生成说明；`choice_is_selectable` 对非 `blur` 键直接返回 true；对应 UI 用例只断言说明存在。
- 优化方向：托盘不可用时静默启动控件必须按端口事实禁用并给出原因，运行值保留用户显式选择但不产生静默承诺。

### 一般｜更新提醒状态不按版本隔离

- 置信度：确认
- 位置：`slterm_app/src/update_check.rs` 的 `UpdatePromptState`、`should_prompt`、`mark_prompted`、`remind_later`、`skip`。
- 现象：`remind_after` 是单一全局时间戳，不属于具体版本；任一已提示版本在时间到后都会因该时间戳重新可提示；`mark_prompted` 会清掉其他版本的稍后提醒。
- 根因：按版本的状态只做了 `prompted`/`skipped` 集合，稍后提醒没有并入版本键。
- 影响：旧版本的稍后提醒会在新版本提示后重新出现，或新版本的提示状态意外覆盖旧版本的抑制；与“提示状态按版本生效”不符。
- 证据：`UpdatePromptState` 只有单个 `remind_after`；`should_prompt(version)` 使用该全局值；`mark_prompted(version)` 置空全局值；持久化结构没有版本到时间的映射。
- 优化方向：稍后提醒按版本存截止时间，所有提示判定只读该版本自己的记录。

### 一般｜运行期显示器变化不重新夹回窗口

- 置信度：疑似
- 位置：`slterm_app/src/app_shell/app_root/window_geometry.rs` 的 `observe_window_geometry`、`record_window_geometry`；`slterm_app/src/windowing/geometry.rs` 的 `restored_bounds`；`slterm_app/src/app_shell/windowing.rs` 的 `startup_window_bounds`。
- 现象：几何夹回只在开窗前调用；运行期窗口边界变化只记录并去抖保存，没有显示器移除、工作区缩小或 DPI 变化的监听与重新定位。
- 根因：设计把夹回定义为启动恢复行为，运行期完全委托 GPUI 与 Windows 默认重定位。
- 影响：若系统未自动把窗口移回可见区，显示器移除或工作区缩小后标题栏与主要操作可能不可达；与 featureSpec 的“显示器移除、DPI 或工作区变化后仍保证可达”存在差距。
- 证据：`restored_bounds` 只被 `startup_window_bounds` 消费；`observe_window_geometry` 只保存 `from_window` 快照；全仓没有显示器变化监听或运行期 clamp。还缺真实显示器拔插、DPI 切换与工作区缩小的实测证据，无法确认 Windows 是否总会自动重定位。
- 优化方向：把显示器/工作区变化纳入窗口几何协调器，运行期也只经同一夹回规则调整；仍需真实桌面验收。

### 一般｜通知漏斗的去重键不含来源程序与资源代次

- 置信度：确认
- 位置：`slterm_app/src/notify.rs` 的 `PaneNotificationThrottle`、`PaneFailureThrottle`、`Funnel::plan_with`；`docs/devPlan/design/windows.md` 的“通知、任务栏与语言”。
- 现象：pane 级节流键只有 `(pane, attention, request)`；失败 episode 键是 `(pane, program, message, outcome)`，没有资源代次；设计要求的“按资源身份和代次去重”没有对应状态。
- 根因：通知身份直接取 pane 与短窗口键，没有把终端/资源代次纳入。
- 影响：同一 pane 重连、换程序或资源代次变化后，旧节流/旧失败 episode 仍可能压住新通知；不同来源在同一 pane 的同键通知会被误合并。
- 证据：节流 map 键类型不含 generation；失败记录只存四元组；`from_accepted_hook` 不携带资源代次；通知投递上下文没有 generation 字段。
- 优化方向：通知身份应包含稳定资源身份与代次；重连或代次变化使旧节流与旧 episode 失效。

### 一般｜合同冲突：缺译回退的验收口径

- 置信度：确认
- 位置：`docs/featureSpec/system-integration.md` 的“界面语言”与“验收判据”；`slterm_app/build/i18n.rs` 的 `generate`、`translated`；`slterm_app/src/i18n/mod.rs`。
- 现象：featureSpec 要求缺译显示 English；生成器要求各语言键集与英文全等，缺键直接构建失败；运行时没有缺译回退分支。
- 根因：featureSpec 的运行时回退承诺与构建期硬键集合同两种策略并存，没有声明哪一个是最终口径。
- 影响：该验收判据无法在产品中发生，也无法被自动化用例证明；维护者可能误以为存在运行时兜底，而实际行为是构建失败。
- 证据：`generate` 对键集不等返回错误；`translated` 的回退只在生成器内部且被键集校验短路；`i18n` 查询直接索引生成表，没有缺键分支。
- 优化方向：明确采用构建期强合同还是运行时回退，并同步所有者文档与验收；两者不能同时作为有效承诺。

### 一般｜合同冲突：WindowsIntegration 端口边界

- 置信度：确认
- 位置：`docs/devPlan/design/windows.md` 的“WindowsIntegration 与窗口”；`slterm_app/src/platform/CLAUDE.md` 的“WindowsIntegration 端口”；`slterm_app/src/platform/windows_integration.rs`；`slterm_app/src/notify.rs` 的 `deliver_with`；`slterm_app/src/app_shell/windowing.rs` 的 `startup_window_bounds`。
- 现象：设计把窗口几何、系统通知与任务栏都列为 WindowsIntegration 端口覆盖面；模块 CLAUDE 与实现只把托盘、登录启动、材质、任务栏进度放进端口。窗口几何直接走 `windowing` 与持久化，toast 闪烁由 `notify` 直接调平台通知模块，AUMID 预热也直接调用平台模块。
- 根因：端口范围在设计与实现约束之间没有收敛，通知通道只接入了任务栏进度的一半。
- 影响：测试替身无法覆盖闪烁与几何平台面，业务模块仍可直接触达平台封装；设计声称的可注入边界不成立，违反“平台能力必须可注入替身”的意图。
- 证据：`WindowsIntegration` 没有几何或闪烁方法；`deliver_with` 直接调用 `crate::platform::notifications::flash_taskbar`；`startup_window_bounds` 直接使用几何与 `App` 显示器信息；端口注释只承诺托盘与开机启动。
- 优化方向：先裁决端口边界；若设计为准，补齐几何与通知端口并收回直接调用；若实现为准，改写设计与模块文档并登记边界。

### 提示｜脱敏规则会误伤普通单词

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的 `redact`。
- 现象：`redact` 在整行任意位置查找 `sk-`，普通单词如 `task-`、`disk-` 中的子串也会命中，随后把到分隔符为止的内容替换为 `[redacted]`。
- 根因：`sk-` 检测没有词边界或上下文约束。
- 影响：诊断日志被错误截断，支持方可能失去关键线索；误伤本身不泄漏凭据，但降低诊断可用性。
- 证据：`lower.find("sk-")` 与 `lower[index..].starts_with("sk-")` 不看前一个字符；分支在任意位置生效。
- 优化方向：只在前缀边界或已知键值上下文命中，保留普通复合词；覆盖率与误伤率都需用例锁定。

### 提示｜更新下载已处于 Ready 时无法重新下载

- 置信度：确认
- 位置：`slterm_app/src/update_download.rs` 的 `begin_download_session`、`status`、`ready_path_at`、`cancel`。
- 现象：同一资产已有 `Downloading` 或 `Ready` 会话时 `begin` 返回 None；若 Ready 文件后来被删除或损坏，`ready_path_at` 重新校验失败，但没有清空状态的入口；`cancel` 标注为等待 UI 且无生产调用。
- 根因：会话状态机把 Ready 当成终态，没有把“文件重新校验失败”转回可下载态。
- 影响：用户可能卡在“已下载但安装前校验失败”的状态，只能重启应用或等 UI 显式取消；更新重试路径不完整。
- 证据：`begin_download_session` 对 `Ready` 直接返回 None；`ready_path_at` 校验失败只返回错误；`cancel` 无生产调用点。
- 优化方向：安装前校验失败应使会话回到可重新下载态；取消与重试必须由同一更新协调器驱动。

### 提示｜机器几何身份只取主机名

- 置信度：确认
- 位置：`slterm_app/src/platform/machine.rs` 的 `machine_key`、`machine_key_from`。
- 现象：窗口几何按 DNS 主机名分行；主机名为空回退固定占位，重命名主机或两台同主机名机器会共享同一行。
- 根因：机器身份没有使用稳定唯一标识或安装实例标识。
- 影响：跨机恢复几何的隔离依赖主机名碰撞概率；主机改名会丢失本机几何，同名机器可能继承对方的几何。
- 证据：`machine_key` 只读 `GetComputerNameExW(ComputerNameDnsHostname)`；无其他标识输入。
- 优化方向：使用稳定且不跨机复用的本机标识；主机名只作显示或兜底，不承担隔离键。

## 跨域观察

- 设置控件仍保留 `ai_toasts`、`bell` 等键，但应用设置快照没有对应字段，生产也没有消费点；用户改这些开关只落盘，不改变终端或通知行为。
- 后台任务框架在当前工作树没有生产订阅者：看门狗、更新检查、更新下载与套餐任务都停在注册表与测试层；这同时影响托盘快照、更新与套餐。
- 更新检查的 GPUI 后台任务使用 `detach`，没有登记到 `TaskService`；与设计要求的“新增后台任务同时登记 TaskService 作用域与关闭路径”不一致。
- 设置页缺应用页/后台任务页还导致 Runtime API 只读状态、备份与恢复、主题导入导出等所有者入口无法按 featureSpec 的应用页落地；这不属于本范围的服务实现，但阻塞跨域验收。
- 更新 handoff 仍引用旧 `Session`/`session.json`，而工作区持久化已迁移到 SQLite 项目快照；这是更新与持久化之间的跨域模型冲突。

## 存疑未证

- 静默启动窗口经托盘“显示”的真实曝光：应用层只调用 GPUI `activate_window`，其平台实现消费首次 placement；代码路径看起来可行，但没有用例覆盖隐藏窗口转可见，需要真实 `slterm.exe` 与托盘点击验收。
- 系统会话结束的真实消息序列：应用代码没有会话结束处理，但 Windows/GPUI 在特定退出路径上是否仍先发 `WM_CLOSE` 未实机确认；需要注销/关机 smoke。
- 真实 setup.exe helper 的覆盖安装、版本回读、回滚重启与 `apply_scheduled` 的端到端路径：受安装器与提权边界限制，当前只有文件态机单测与豁免声明。
- 真实 Runtime token 是否可能进入日志：脱敏规则不覆盖裸 64 位 hex，但当前日志调用点是否会出现该形态未逐点证明；需要日志审计或注入用例。
- 真实显示器拔插与 DPI 变化时 Windows 是否自动重定位窗口：代码没有运行期 clamp，实际是否可达标题栏取决于系统行为。
- 真实代理注册表与环境变量组合（含 `NO_PROXY` 与 SOCKS）的端到端行为：只有本地回环与纯函数用例，未触真实网络。
