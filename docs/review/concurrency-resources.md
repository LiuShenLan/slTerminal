# 跨模块并发、任务与资源生命周期审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），只审当前工作树，范围为跨模块并发、任务与资源生命周期。

## 覆盖与局限

已覆盖：

- 纪律与设计文档：根 `CLAUDE.md`、`slterm_app`／`app_root`／`gpui_terminal`／`persistence`／`persistence/document_snapshot`／`local_control`／`ai_hook`／`slterm_terminal`／`slterm_hook` 的目录 `CLAUDE.md`，`architecture/overview.md`、`testing/strategy.md`、`testing/exemptions.md`，`devPlan` 的 `plan.md`／`progress.md`／`traceability.md` 与 `design/lifecycle.md`／`terminal.md`／`persistence.md`／`workspace.md`／`runtime.md`／`hooks.md`／`agents.md`。
- 代码：`app_shell/app_root` 的装配、事件泵、任务服务、应用／项目／终端／批量关闭、Document 打开与外部变化、Diff、Runtime 快照与写意图、命令接线；`app_shell/global_close.rs`、`app_shell/mod.rs`、`shell_events.rs`、`fs_watch.rs`、`watcher_service.rs`、`windowing.rs`；`persistence/workspace_writer.rs`、`learning.rs`、`learning_writer.rs`、`document_snapshot/{writer,store,recovery,envelope}.rs`、`startup/recovery.rs`；`local_control/{service,server,events,operations}.rs`；`gpui_terminal/{session,registry,event_mailbox,session_pump,view,view/startup}.rs`；`slterm_terminal` 的 `event_loop`、`tty/windows/{mod,spawn,conpty,blocking}.rs`；`ai_hook/windows.rs` 与 `agent_host`；`background_tasks.rs`、`update_check.rs`、`logging.rs`、`notify.rs`、`tray.rs`。
- 相关测试文件与例名：`app_root/tests/{batch_close,application_close_receipts,runtime_events,view_tasks,diff/*,documents}.rs`、`persistence/document_snapshot/tests.rs`、`persistence/workspace_writer_*_tests.rs`、`local_control/event_ring_tests.rs`、`local_control/server_tests/runtime.rs`、`local_control/operation_dispatch_tests.rs`、`ai_hook/e2e_tests.rs`、`learning` 与 `background_tasks` 内嵌用例。

未覆盖／未读完：`local_control/server.rs` 各资源方法的逐臂细节（只读快照、事件订阅、读／等待、关闭路径）、`commit.rs` 与 `explorer.rs` 的全部业务分支、`update_download`／`handoff` 的内部实现、`ai_sessions`／`history_quarantine`／`encrypted_backup` 维护链、GPUI 依赖内部实现（`application.run` 的实体析构时序、background executor 线程模型）。这些限制影响“存疑未证”的判断，不影响下列已确认问题的代码路径。

## 问题

### 阻断｜崩溃快照写队列没有生产 owner，异常退出无法恢复未保存正文

- 置信度：确认
- 位置：`slterm_app/src/persistence/document_snapshot/writer.rs` 的 `DocumentSnapshotWriter`／`DocumentSnapshotClient`；`slterm_app/src/persistence/document_snapshot/envelope.rs` 的 `DocumentSnapshot::capture`；`slterm_app/src/persistence/document_snapshot/store.rs` 的 `DocumentSnapshotStore`；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`；`slterm_app/src/app_shell/app_root/mod.rs` 的 `AppRootServices`。
- 现象：生产路径没有任何地方构造 `DocumentSnapshotWriter` 或 `DocumentSnapshotClient`，也没有任何编辑、保存、关闭或退出路径调用 `DocumentSnapshot::capture` 与快照 `write`／`reconcile`／`delete_closed`／`flush`／`shutdown`。全仓调用点只有 `persistence/document_snapshot/tests.rs` 与 `app_root/tests/documents.rs` 的测试夹具。启动链路只在 `AppRootPersistence::load` 构造 `DocumentSnapshotRecoverer` 读取快照目录，由于从不写入，异常启动永远看不到有主快照；`workspace.db` 的 `snapshot_index` 表同样没有任何生产写入点，启动维护的“快照索引存在性”闸门恒为空。
- 根因：快照写入所有权没有接入运行根。`AppRoot` 不持有当前 `run_id`（`AppRootServices` 只传 `persistence`／`global_close`／`learning`／`runtime_events`／`windows`），也没有“编辑事务后入队、保存或撤销后 reconcile、关闭收集与退出前 flush”的调用点；设计中的独立写线程与配额逻辑因此只存在于模块与测试中。
- 影响：进程崩溃或强杀时，所有未保存的 Document 正文丢失；`featureSpec` 的“异常启动的有主快照仍恢复未保存文本”“崩溃恢复先布局后快照”与 `persistence.md` 的快照去抖、配额、关闭前冲刷合同均不成立。该写队列的双上限、去抖、配额与失败保留语义从未在真实组合中运行，后续接入时也无法从现有测试获得回归覆盖。
- 证据：全仓检索 `DocumentSnapshotWriter`／`DocumentSnapshotStore`／`DocumentSnapshot::capture` 只命中 `persistence/document_snapshot` 自身与测试夹具（含 `app_root/tests/documents.rs`）；`AppRootPersistence::load` 只实例化恢复器；`AppRootServices` 字段不含快照 client；`progress.md` 的 Document 与文本视图阶段与 `traceability.md` 的“快照配额”锚指向 `integration_snapshot_quota`，该用例只在临时目录上直接操作 `DocumentSnapshotStore`，不经过 AppRoot 或任何生产 owner。
- 优化方向：把快照写线程与 client 作为显式所有者接入启动链（GPUI 外启动与回收，或由 AppRoot 强持并随运行根关闭），将当前 `run_id` 沿同一启动元数据传入；在编辑事务、显式保存、撤销清脏、Document 外部变化、关闭收集与应用退出等边界建立唯一入队与冲刷点，并补经 AppRoot 的集成用例，验证崩溃后重开能恢复正文。

### 重要｜hook 命名管道桥在宿主侧没有生产所有者，hook 事件无法进入运行壳

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `spawn_gpui_server`／`PipeServer`；`slterm_app/src/agent_host.rs` 的 `AgentHub::handle_hook`；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`；`slterm_app/src/ai_hook/e2e_tests.rs`。
- 现象：`SLTERM_NOTIFY_PIPE` 只在 `spawn_pipe_server` 内设置，而 `spawn_gpui_server` 在生产代码中无调用方；`AgentHub::handle_hook` 也没有生产调用方。`slterm-hook` 子进程因此永远读不到管道名，按 `Outcome::NotHosted` 静默退出；`PipeServer` 的 accept 线程、`CancelSynchronousIo` 停机与 join 路径只在 e2e 测试里运行。
- 根因：hook 事件桥的宿主侧所有者（管道服务线程 + 前台泵 + `AgentHub` 路由）没有接入启动链；`lib.rs` 的模块注释称“事件通道已接线”，但实际只有模块内部的发送端与测试。
- 影响：AI CLI 的 hook 生命周期、标题、注意状态、通知与隐藏窗口事件全部缺失，只能退化为 OSC 133 命令边界；`featureSpec/cross-cutting.md` 要求“CLI hooks 信号不因不可见而停止”无法满足；线程显式 shutdown／join 的纪律没有生产证据。
- 证据：全仓检索 `spawn_gpui_server` 仅命中 `ai_hook/e2e_tests.rs` 与定义；检索 `handle_hook`／`AgentHub::observe_screen` 仅命中 `agent_host.rs` 自身与测试；`run_shell_once` 全文无 hook 相关调用；`slterm_hook/src/lib.rs` 的 `run` 在缺少 `SLTERM_NOTIFY_PIPE` 时直接返回 `NotHosted`。
- 优化方向：在启动链中按单实例唯一起动管道服务，把接收端交给 GPUI 前台的单一 hook 泵并路由到 `AgentHub`；在窗口销毁与全局关闭顺序中显式停机并 join，补经真实启动组合的用例验证管道名发布、事件投递、隐藏时继续接收与退出后线程回收。

### 重要｜hook 事件通道使用无界 std 通道，违反有界通道纪律

- 置信度：确认
- 位置：`slterm_app/src/ai_hook/windows.rs` 的 `spawn_gpui_server`（`std::sync::mpsc::channel()`）。
- 现象：宿主侧 hook 事件接收端是无界 `std::sync::mpsc::channel`，而 `ai_hook/CLAUDE.md` 明确要求“hook 通道与 statusline 通道各自有界”，根 `CLAUDE.md` 也要求跨线程只经有界通道。
- 根因：从模块自带测试形态迁移时保留了标准库的无界通道，没有换成带容量与丢弃／背压语义的端点。
- 影响：一旦把管道服务接入生产（见“hook 命名管道桥在宿主侧没有生产所有者”），AI CLI 的 hook 突发在 GPUI 前台阻塞或事件泵停顿时会无界占用内存；当前因为服务未启动而不可达，但接入时这是直接的后压缺口。
- 证据：`spawn_gpui_server` 的通道构造；`ai_hook/CLAUDE.md` 的“隔离面：hook 通道与 statusline 通道各自有界”；生产无调用方（见“hook 命名管道桥在宿主侧没有生产所有者”）。
- 优化方向：改为有界通道并明确满载策略（丢弃并计数、或对 helper 施加背压），前台泵按批排空；在接入用例中覆盖突发与满载两种路径。

### 重要｜后台周期任务、屏幕看门狗、套餐与更新检查没有生产订阅者与驱动泵

- 置信度：确认
- 位置：`slterm_app/src/background_tasks.rs` 的 `subscribe`／`run_once`／`TaskSchedule`／`TaskSubscription`；`slterm_app/src/update_check.rs` 的 `start_gpui`；`slterm_app/src/agent_host.rs` 的 `AgentHub::observe_screen`；`slterm_app/src/app_shell/mod.rs` 的 `run_shell_once`。
- 现象：生产代码中唯一的 `background_tasks::subscribe` 调用在 `update_check::start_gpui` 内，而 `start_gpui` 全仓无调用方；`run_once` 在生产中无调用方（壳事件泵只做 120 毫秒唤醒与壳事件派发，不按注册表节拍驱动任何任务）。`AgentHub::observe_screen` 同样没有生产调用方。因此注册表中的屏幕看门狗、套餐余额、启动更新检查与按需下载任务都不会运行。
- 根因：`background_tasks` 文档所称的“订阅方 = workspace 的驱动泵”没有实现；任务框架只完成了注册表、订阅守卫与单元级执行语义，没有组合根接线。
- 影响：套餐余量从不查询；Agent 屏幕兜底识别与托盘 attention 快照从不采样；启动更新检查与后台预取从不执行。`plan_balance_runs_while_navigation_hidden`、`settings_plan_schedule_commit_and_visibility`、`integration_plan_schedule` 只验证 `run_once` 与原子量，不验证任何生产驱动；`testing/exemptions.md` 把该驱动循环登记为不可自动化并给出“人工观察（运行中改配置）”兜底，但生产中没有可观察的循环。
- 证据：全仓检索 `background_tasks::subscribe`／`begin`／`run_once` 仅命中 `update_check.rs` 与测试；`run_shell_once` 无任务驱动调用；壳事件泵只有 `executor.timer(Duration::from_millis(120))`；`AgentHub::observe_screen` 仅命中测试。
- 优化方向：指定唯一 owner（启动链上的 workspace 泵任务或 AppRoot 前台泵）在启动时订阅已登记任务并按注册表节拍驱动，窗口关闭前退订；为组合根补可断言订阅数、节拍推进与退订归零的用例，并把该豁免项改为可自动化覆盖或删除。

### 重要｜全局关闭工作线程在 writer 失败后直接退出，窗口保留但所有重试入口已断连

- 置信度：确认
- 位置：`slterm_app/src/app_shell/global_close.rs` 的 `run_worker`／`GlobalCloseRuntime::join`；`slterm_app/src/app_shell/app_root/application_close.rs` 的 `finish_application_global`／`finish_recovery_global`／`fail_application_close_unknown`；`slterm_app/src/app_shell/mod.rs` 的 `WorkspaceWriterRuntime::shutdown_internal`。
- 现象：`run_worker` 只在控制面清理失败时 `continue` 并允许重试；任何 writer 侧错误（最终冲刷失败、元数据提交失败、最终确认失败）都会发送错误回执并 `return`，工作线程结束。AppRoot 收到 `GlobalCloseError::Writer` 后调用 `fail_application_close_unknown`：标记 outcome unknown、保留窗口、`close_in_progress=false`。此时 `project_writer` 的写线程已被消费并析构，`global_close` 请求通道的接收端也已丢弃，后续任何关窗、项目关闭、设置写入或再次关闭请求都得到 `Stopped`／`Disconnected`。
- 根因：writer 运行时的所有权被 `GlobalCloseRuntime` 一次性消费，而工作线程的失败退出没有重建路径；原生“重试／知情放弃”对话框只覆盖 `mark_clean_shutdown` 失败，不覆盖其之前的 `FlushFailed` 或控制面之外的其他 writer 失败。
- 影响：一次冲刷失败后进程无法正常退出，用户只能强杀（未提交状态丢失且无知情选择）；与 `design/lifecycle.md` 的“最终状态提交失败提供重试，或允许用户显式选择退出且不保存”“已停止 PTY 不自动重开”以及 `app_shell/CLAUDE.md` 的“flush 失败保留窗口并允许重试”矛盾。现有 `writer_flush_failure_does_not_commit_clean_shutdown` 只断言失败与未写正常退出，不验证可重试性。
- 证据：`run_worker` 的 writer 错误分支 `return summary`；`finish_application_global` 在 `Ok(Err(GlobalCloseError::Writer(_)))` 分支调用 `fail_application_close_unknown`；`retry_application_close` 在 `outcome_unknown` 时直接返回；`WorkspaceWriterRuntime::shutdown_internal` 在 `report.failed > 0` 时提前返回 `FlushFailed`，此时 `writer.shutdown()` 已完成且 writer 被消费。
- 优化方向：把 writer 与关闭工作线程的失败分成可重试与终态两类：可重试失败不得消费 writer 的所有权，或由上层以新连接重建关闭链；所有最终提交失败都走同一显式重试／知情放弃决策；用真实 writer 注入一次冲刷失败并验证重试成功与放弃路径均可退出。

### 重要｜runtime.snapshot 的 revision 固定为握手值，事件环积压超过客户端队列后新连接永远无法订阅

- 置信度：确认
- 位置：`slterm_app/src/local_control/service.rs` 的 `LocalControlService::start_with`（`HelloResponse::new(..., 0, ...)`）；`slterm_app/src/local_control/server.rs` 的 `handle_connection` 中 `RuntimeMethod::Snapshot` 分支；`slterm_app/src/local_control/events.rs` 的 `EventRing::resume`／`subscribe_ring`／`EventHub`。
- 现象：握手 hello 的 revision 恒为 0，`runtime.snapshot` 返回的 revision 直接复制 `hello.revision`，不查询事件环当前末尾；而 `EventsSubscribedResponse.revision` 使用 `ring.latest_revision()`。事件环每次发布单调递增 revision，环中最多保留 1,024 条，单客户端订阅队列容量 64；`subscribe_ring` 在积压超过 64 时返回 `ResyncRequired`。因此进程内发布超过 64 个事件后，新连接按“snapshot 的 revision → events.subscribe since_revision”的文档流程订阅，永远得到 resync；重新快照后 revision 仍是 0，无法收敛。
- 根因：事件环接线后，快照与握手的 revision 仍停在事件环接入前的占位实现（`local_control/CLAUDE.md` 该条的“事件环接入前 revision 由握手起点填写”已过期）；`EventHub` 也没有向 server 暴露 latest revision 的查询。
- 影响：Runtime 事件订阅在长会话中不可用，且客户端没有可用的“从当前时刻订阅”入口；快照也不能作为同一资源模型的续传基线，违反 `design/runtime.md` 的“事件续传必须同时匹配 store_epoch、stream_epoch 和 revision”与“快照与事件使用同一资源模型”。当前 `runtime_snapshot_*` 测试使用测试握手 revision（非真实环末尾）且不发布事件，`runtime_events_visibility_and_generation` 使用注入的发布回调并断言 `revision == 0`，都不覆盖该路径。
- 证据：`LocalControlService::start_with` 的 hello 构造；server 的 Snapshot 分支 `revision: hello.revision`；`EventRing::resume` 的 `since_revision > next_revision` 与 `oldest` 判断；`subscribe_ring` 的 `backlog.len() > CLIENT_QUEUE_CAPACITY`；`EventHub` 只在 `subscribe_ring` 内使用 `latest_revision`。
- 优化方向：握手与快照的 revision 都取事件环当前末尾（访问同一所有者线程，避免重复的 revision 事实），并保证快照生成与 revision 捕获在同一前台时刻；新增“先发布超过客户端队列容量的事件，再 snapshot → subscribe”的端到端用例。

### 重要｜PTY 启动与重连在 GPUI 前台执行阻塞创建，并受进程级 SPAWN_LOCK 约束

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commands.rs` 的 `LocalTerminalLauncher::launch`／`reconnect`、`restore_terminal_panel`、`new_terminal_from_request`／`new_hidden_terminal_from_request`、`launch_candidate`；`slterm_app/src/gpui_terminal/view/startup.rs` 的 `TerminalView::launch`／`restore_launch`／`from_prepared`；`slterm_app/src/gpui_terminal/session.rs` 的 `spawn_local`／`spawn_on_term`／`local_options`；`slterm_terminal/src/tty/windows/spawn.rs` 的 `spawn_locked`；`slterm_terminal/src/tty/windows/conpty.rs` 的 `new`；`slterm_terminal/src/tty/windows/environment.rs` 的 `refresh_environment`。
- 现象：新建终端、重连终端、启动恢复与项目激活懒启动都在 GPUI 前台同步调用 `tty::new`：持进程级 `SPAWN_LOCK`、执行 `CreateProcessW`、创建 ConPTY、指派 Job Object；首次侧载还可能提取并写入 `conpty.dll`／`OpenConsole.exe`，`local_options` 还会在前台刷新环境快照（注册表读取）。前台在上述路径中直接等待 OS 创建与文件 I/O，而不是走 background executor 后再回前台提交。
- 根因：`TerminalPanelLauncher` 是同步返回 `io::Result<Entity<TerminalView>>` 的接口，`TerminalView::launch` 把“可失败启动”与“实体构造”绑在同一前台调用内；`AppRoot` 的候选提交虽然原子，但没有把真实 spawn 移出前台。
- 影响：违反根 `CLAUDE.md` 的“GPUI 前台线程禁止阻塞 I/O、网络、子进程等待”和 `slterm_app/CLAUDE.md` 的前台纪律；首次 spawn、慢 CreateProcess 或并发 spawn 争锁时窗口冻结；`SPAWN_LOCK` 让前台直接依赖其他 spawn 路径的完成时间（当前所有 spawn 都在前台，未来任何后台 spawn 或探测都会扩大该耦合）。
- 证据：`open_initial_window` 的窗口构造闭包调用 `restore_active_project`；`restore_project_panels` → `restore_terminal_panel` → `launcher.restore`／`TerminalView::restore_launch`；新建命令 `new_terminal_with_identity` → `launch_candidate` → `launcher.launch`；`spawn_locked` 在 `SPAWN_LOCK` 内执行容量判定与 `conpty::new`；`conpty::new` 内 `ConptyApi::new` 与 `CreateProcessW` 均为同步调用。
- 优化方向：把“解析目标 → 真实 spawn → 结果回前台提交候选与实体”拆成后台任务，前台只保留纯预检、候选快照与实例登记；`TerminalPanelLauncher` 改为可失败的异步接缝，所有创建错误仍在同一回执内呈现；补真实启动经后台执行体的用例或在豁免中明确登记。

### 重要｜集成与验收锚只覆盖替身或存储单元，未覆盖生产组合根

- 置信度：确认
- 位置与例名：`slterm_app/src/persistence/document_snapshot/tests.rs` 的 `integration_snapshot_quota`、`snapshot_quota_does_not_evict_dirty`；`slterm_app/src/app_shell/app_root/tests/runtime_events.rs` 的 `runtime_events_visibility_and_generation`；`slterm_app/src/background_tasks.rs` 的 `plan_balance_runs_while_navigation_hidden`、`settings_plan_schedule_commit_and_visibility`、`integration_plan_schedule`；`slterm_app/src/ai_hook/e2e_tests.rs`；`.claude/docs/testing/exemptions.md` 的后台任务周期驱动条目。
- 现象：这些例名或追溯锚声称验证集成、配额、事件代次、套餐调度与 hook 桥，但实际前置条件是模块内直接构造的存储、注入的发布回调或测试自建的管道服务，动作不经过 `run_shell_once`／`AppRoot`／`LocalControlService` 的真实装配，可观察结果也不是生产 owner 的线程、订阅或队列状态。`runtime_events_visibility_and_generation` 断言 `last.revision == 0` 并称“revision 由事件环分配”，而其发布端是注入的 `RuntimeEventPublisher::from_callback`，从未经过 `EventHub`。
- 根因：验收锚以“模块能力存在”代替“生产接线成立”，且测试替身被当作生产路径的证据；`testing/exemptions.md` 又把未实现的驱动循环以“人工观察”兜底，形成自证闭环。
- 影响：`progress.md` 中 Document、Agent 与 Hooks、Runtime、Windows 与发布阶段的“已验收”无法由这些锚复现；上述无 owner 的写队列、hook 桥、任务驱动与快照 revision 偏差因此长期未被发现；后续真实接线缺少回归基线。
- 证据：上述例名的构造与断言路径；`runtime_events_visibility_and_generation` 的 `install_publisher` 只替换 `AppRoot::runtime_events`；`integration_snapshot_quota` 直接调用 `DocumentSnapshotStore::write`；`ai_hook/e2e_tests.rs` 自行 `spawn_gpui_server`。
- 优化方向：把“生产组合根启动／订阅／回收”作为独立可断言前置，补经真实启动链的组合测试（可在虚拟窗口与临时双根下断言线程、订阅数、回执与磁盘事实）；纯模块测试保留但不得继续作为集成验收锚；豁免表只登记真正的平台边界。

### 一般｜Runtime Operation 在写线程回执超时后永久停在 Queued／Running

- 置信度：确认
- 位置：`slterm_app/src/local_control/operations.rs` 的 `OperationDispatcher::dispatch`／`await_writer`／`query`；`slterm_app/src/persistence/workspace_writer.rs` 的 `RegisterOperation`／`TransitionOperation` 与 `run` 循环。
- 现象：`dispatch` 先登记 Operation，再推进 Running，最后推进终态；每一步都以 4 秒 `recv_timeout` 等待写线程回执。若登记回执超时，调用方得到 `NotReady`，但登记命令可能已经或稍后在写线程执行，产生永久 Queued 的记录；若执行后终态推进回执超时，记录永久停在 Running。重试相同幂等键时 `RegistrationOutcome::Existing` 直接返回原记录（不检查是否终态），调用方会收到一个状态为 Queued／Running 的“成功结果”。启动维护期不枚举 `runtime_operations`，也没有恢复器把这些记录转为 `OutcomeUnknown`。
- 根因：Operation 状态机的推进依赖同步回执，但超时／未知结果没有补偿推进，也没有启动对账；“登记成功即派发”与“派发结果可查询”之间缺少可恢复的中间态。
- 影响：数据库或写线程短暂抖动会让一次写请求永久不可完成；客户端按协议只能等待到 600 秒上限后得到 NotReady，且以同键重试永远返回非终态原记录，既不执行也不报 OutcomeUnknown。与 `design/runtime.md` 的“无法证明副作用的中断标为 OutcomeUnknown，绝不自动重发”不一致（既未标 OutcomeUnknown，也不可完成）。
- 证据：`dispatch` 的两次 `await_writer`；`await_writer` 的超时分支只返回 `NotReady`；`RegistrationOutcome::Existing` 无条件返回 `operation_resource`；`persistence/startup` 无 `runtime_operations` 枚举；`workspace_writer` 的 `run` 对登记与推进命令不检查终态前置。
- 优化方向：在回执未知时以独立通道或启动恢复器把非终态 Operation 收敛为 `OutcomeUnknown`（或明确保留 Running 并提供显式重查推进）；重试命中非终态原记录时不得作为成功的写回执，至少返回可查询的结构化未知态；补超时注入用例。

### 一般｜Runtime 事件环的已断开订阅只在下次发布时清理

- 置信度：确认
- 位置：`slterm_app/src/local_control/events.rs` 的 `event_hub_loop`／`deliver_event`／`subscribe_ring`；`slterm_app/src/local_control/server.rs` 的 `stream_subscription`。
- 现象：订阅表是 `Vec<SyncSender<RuntimeEvent>>`，只在 `deliver_event` 的 `try_send` 失败时 `retain` 移除；`events.unsubscribe` 与客户端断连只关闭连接与事件接收端，不向 hub 发送取消命令。若此后没有新事件发布，已断开的订阅条目与其容量 64 的通道分配会一直保留到下一次发布或 hub shutdown。
- 根因：订阅注册是单向命令（`HubCommand::Subscribe`），没有对应的取消／回收命令；hub 也没有周期性探测 `sender.is_closed()`。
- 影响：长时间运行且事件稀疏（当前只有项目激活发布事件）时，反复订阅／断连会累积无人消费的订阅槽与内存；也会让关闭前的订阅表规模与真实客户端数脱节。`event_ring_tests` 覆盖了积压、续传与滞后订阅移除，但未覆盖“断连后无发布”的清理。
- 证据：`HubCommand` 只有 `Publish` 与 `Subscribe`；`stream_subscription` 的取消确认只写回连接；`deliver_event` 是唯一移除点。
- 优化方向：增加显式的取消／回收命令并在断连与取消订阅时提交；或在订阅时对表执行 `is_closed` 修剪并为 hub 设置低频清理节拍；补“订阅 → 断开 → 不再发布 → 表规模归零”的用例。

### 一般｜学习清空的代次推进先于入队，被取代的清空可能返回成功但从未删除数据库

- 置信度：确认
- 位置：`slterm_app/src/persistence/learning_writer.rs` 的 `LearningWriterClient::clear`、`run_writer` 的 `WriterCommand::Clear` 分支；`slterm_app/src/learning.rs` 的 `LearningService::clear`。
- 现象：`clear` 先以 `fetch_update` 推进共享代次，再 `try_send(Clear)`；若队列已满，代次已经推进但清空命令没有入队，调用方得到 `QueueFull`。若此时存在更早一次已入队的清空，写线程处理它时发现 `clear_generation != generation.load()`，直接返回 `Ok(())` 而不执行事务删除；更早的调用方（以及已清空内存的 `LearningService`）会认为持久清空成功，实际数据库仍保留全部历史，下次启动重新加载。
- 根因：代次递增与入队不是同一临界区；“被更新代次取代”被当作成功，但没有证明更新的清空一定已入队并会执行。内存清空与持久清空的结果也没有分离表达。
- 影响：用户显式清空历史后，历史仍可能在下次启动复活；违反 `persistence.md` 的“清空先递增学习代次、取消待写、清内存并事务删除；失败保留可见错误”和 `persistence/CLAUDE.md` 的“队列拒收或事务失败必须返回错误，不得报告已清空”。当前生产清空入口只有单进程 `clear-learning`（持实例锁、单次调用），可达性依赖未来设置页清空按钮；一旦接入即为用户可见错误。
- 证据：`clear` 的 `fetch_update` 后 `try_send`；`run_writer` 对被取代清空返回 `Ok(())`；`LearningService::clear` 在返回 writer 结果前已重置内存；`learning_clear_cancels_pending_writes` 与 `learning_clear_serializes_with_an_inflight_recording` 均未覆盖两次清空竞争。
- 优化方向：只在入队成功后推进代次（或由写线程按命令序号判定取代关系），被取代请求不得直接返回成功；若更新代次未能入队，应让更早请求得到明确的未完成结果；补双清空与满队注入用例。

### 一般｜Diff HEAD 过期回执提前返回，不收割任务身份也不结算新票据

- 置信度：疑似
- 位置：`slterm_app/src/app_shell/app_root/diff.rs` 的 `finish_diff_head`／`schedule_diff_head`；`slterm_app/src/app_shell/app_root/task_service.rs` 的 `finish_view_refresh`。
- 现象：`finish_diff_head` 先 `finish_view_refresh(ticket)`，当结果为 `Stale` 时直接返回，不调用 `tasks.complete(TaskId::DiffHead(panel))`，也不清除视图的 `head_loading`；而 `finish_view_refresh` 只对当前代次结算，Stale 票据不会触发 `next`。若过期回执真的出现，任务身份会永久留在 `TaskService`，后续 `schedule_diff_head` 的登记失败只记录 debug，视图停留在加载态。`schedule_diff_head` 在登记失败时同样不清理 `head_loading`，也不结算刚拿到的票据。
- 根因：代次协议把“旧结果不得覆盖新任务”实现为直接返回，但没有区分“旧回执必须被收割”与“不得触碰新任务”；票据与任务句柄的生命周期没有在同一处闭合。
- 影响：Diff HEAD 读取永久停止、面板错误地显示加载中、TaskId 泄漏；只有在 `Stale` 可达时发生。当前隐藏路径会先 `cancel(TaskId::DiffHead)` 再重新可见，因此常规切项目可能避开；需要运行时证据确认是否存在“票据被取代但旧任务未被取消”的窗口。
- 证据：`finish_diff_head` 的 Stale 早退；`schedule_diff_head` 对 `registration.is_err()` 只 `log::debug`；`finish_view_refresh` 的 Stale 分支返回 `next: None`。
- 优化方向：Stale 回执仍收割对应任务身份并恢复加载态，只拒绝写入视图；登记失败时立即结算票据并恢复 `head_loading`；补注入过期回执的定向用例。

### 一般｜知情放弃仍保留更新安装预约

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/application_close.rs` 的 `finish_application_global`／`finish_recovery_global`；`slterm_app/src/app_shell/app_root/update_install.rs` 的 `finish_update_install`。
- 现象：`FinalShutdownOutcome::Completed` 与 `Abandoned` 分支在设置 `close_completed` 后都调用 `finish_update_install()`；`Abandoned` 表示最终状态提交失败且用户选择退出且不保存，但安装守卫仍被 `install_scheduled()` 保留，下次启动执行安装。恢复失败路径的全局回执同样不区分结果。
- 根因：`finish_update_install` 的语义只表达“窗口结束”，没有接收正常的最终提交结果；调用点把“进程结束”误当作“正常关闭完成”。
- 影响：一旦更新提示接线（`begin_update_install` 目前无生产调用方），失败关闭后的知情放弃会继续安装更新，违反 `design/lifecycle.md` 的“最终状态提交失败取消 handoff”。当前为潜伏缺陷，因为更新安装入口尚未接线。
- 证据：`finish_application_global` 的 `Completed`／`Abandoned` 分支汇合到同一 `finish_update_install` 调用；`update_install.rs` 的文档“统一关闭 Completed：保留预约”与调用点不一致。
- 优化方向：`finish_update_install` 按最终结果区分保留与撤销，`Abandoned` 与任何失败回执都必须撤销预约；补覆盖 Completed 与 Abandoned 的守卫状态断言。

### 提示｜壳事件泵以固定 120 毫秒定时器常驻轮询

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/events.rs` 的 `start_shell_event_pump`。
- 现象：泵在窗口生命周期内无条件每 120 毫秒唤醒一次，即使没有任何壳事件；文件监听结算、Document 自动保存结算与设置写盘结算都复用这一节拍。
- 根因：以轮询定时器承担事件驱动与去抖结算，没有在无待处理工作时休眠到下一次真实唤醒。
- 影响：常驻 CPU 唤醒与功耗开销；fs watch／autosave／设置写盘的响应存在 120 毫秒下限；与 `app_root/CLAUDE.md` 的“时间结算复用壳事件泵，不新增轮询”和 `fs_watch` 文档的“事件驱动”表述存在语义偏差。
- 证据：`start_shell_event_pump` 的 `executor.timer(Duration::from_millis(120)).await`；`poll_fs_watch`／`poll_document_autosave`／`poll_settings_save` 均只在该循环内调用。
- 优化方向：由 fs watch 唤醒、autosave 截止时间与设置去抖截止时间共同决定下次唤醒（最早截止时间），无待处理项时阻塞等待事件；若保留轮询，应在文档与豁免中明确其为已裁定的统一结算节拍与预算。

### 提示｜AppRoot 析构可能在前台 join PTY

- 置信度：疑似
- 位置：`slterm_app/src/app_shell/app_root/mod.rs` 的 `AppRoot` 字段 `terminals` 与 `tasks` 的声明顺序；`slterm_app/src/gpui_terminal/session.rs` 的 `TerminalSession::shutdown`；`slterm_terminal/src/event_loop/mod.rs` 的 `EventLoopHandle::drop`。
- 现象：`TerminalRegistry` 字段先于 `TaskService` 析构；`TerminalSession` 持有的 `EventLoopHandle` 在 Drop 时发送 `Shutdown` 并 `join` PTY 线程。若窗口在统一关闭链之外被销毁（异常退出、未来新增的销毁路径，或关闭失败后的强制销毁），注册表仍可能持有存活会话，join 发生在 GPUI 前台线程。
- 根因：统一关闭链是回收 PTY 的唯一正常路径，但没有为注册表提供“析构时移交后台回收”的兜底；字段顺序让前台承担了最后的阻塞回收。
- 影响：异常销毁路径下窗口线程被阻塞；与 `gpui_terminal/CLAUDE.md` 的“GPUI 前台不得 join”和“关闭单点”纪律存在结构性缺口。正常应用关闭会先清空注册表，故当前可达性取决于异常路径。
- 证据：`AppRoot` 字段顺序；`EventLoopHandle::drop` 的 `tx.send(Msg::Shutdown)` 与 `join.join()`；`TerminalSession::shutdown` 文档明确“会阻塞，只能由后台关闭协调器调用”。
- 优化方向：为 `TerminalRegistry`／`TerminalSession` 的析构提供“移交专用回收线程”的兜底（或将注册表放入可在 Drop 时移交给后台的所有者），确保任何前台析构都不会 join；补“未走关闭链的窗口销毁”用例验证前台不被阻塞。

## 跨域观察

- `slterm_app/src/update_check.rs` 的 `start_gpui` 无生产调用方，`slterm_app/src/app_shell/app_root/update_install.rs` 的 `begin_update_install`／`cancel_update_install` 被标为 dead code；更新检查、下载预取与更新关闭移交的 UI 前缀都没有接线。这与 `progress.md` 的 Windows 与发布阶段“更新守卫与启动对账切片全绿”的措辞不一致。
- `slterm_app/src/ai_sessions.rs`（原生历史扫描）与 `slterm_app/src/plan_balance` 的服务与客户端同样无生产调用方；导航历史恢复、隔离批次撤销与套餐展示因此只有模块级证据。
- `slterm_app/src/persistence/document_snapshot/recovery.rs` 的恢复器已在启动链中运行，但由于没有快照写入方（见“崩溃快照写队列没有生产 owner”），其恢复、隔离、无主材料与授权占位分支在生产中没有输入来源，相关测试只验证恢复器自身。
- `slterm_app/src/persistence/learning_writer.rs` 的 `LearningWriterRuntime` 在显式 `shutdown()` 后因 `Drop` 再次调用 `shutdown_inner()` 会向已断开通道发送 `Shutdown`，正常退出也会记录“学习写线程已断开”的告警；不影响资源回收，但会污染诊断。

## 存疑未证

- Term 锁与事件信箱的锁序环：`slterm_terminal/src/event_loop/mod.rs` 的 `pty_read_until` 在持有 `FairMutex` 租约与数据锁期间调用 `StreamProcessor::feed`，后者经 `EventSender::send` 在信箱满 256 项时 `futures::executor::block_on` 阻塞；信箱唯一消费者是 GPUI 前台的 `session_pump`。而前台在 `TerminalView::launch`／重连安装前会 `capture_frame`，对同一 `FairMutex` 取租约。若新 worker 在泵挂接前积压超过 256 个语义事件，前台与 worker 会互相等待。需要运行时证据：真实 shell 在 spawn 到 install 的窗口内产生该量级语义事件，或构造可稳定复现的压力用例。
- GPUI background executor 的线程模型：多个 `TerminalSession::shutdown`（批量关闭把全部会话放在一个后台任务，单终端回收每终端一个任务）在共享 background executor 上执行阻塞 join，而壳事件泵的 120 毫秒定时器与写回执任务也在同一 executor 上。需要 GPUI executor 的池大小与扩容策略证据，判断大量并存 PTY 收尾是否会饿死定时器与回执。
- `application.run` 返回后 `GlobalCloseRuntime::join()` 依赖 AppRoot 的 `GlobalCloseClient` 克隆已随窗口/实体析构；若 GPUI 在 `run` 返回时仍保留实体，`join` 会等待全部 client 断连。需要 GPUI 生命周期证据或真实窗口销毁后的 join 探针。
- `terminal.read` 的游标绑定 `state.frame_revision`，但同步更新（DECSET 2026）期间网格可继续解析而帧不发布；同一 revision 下两次读取可能返回不同的正文，旧分页游标不会失效。需要真实同步更新输出的复现，确认是否会造成混合页。
- `application.run` panic 或异常展开会绕过 `run_shell_once` 末尾的 `global_close.join()` 与 `learning_runtime.shutdown()`；panic 钩子若直接结束进程，最终冲刷与正常退出提交处于竞态。需要确认 panic 钩子的实际退出行为。
