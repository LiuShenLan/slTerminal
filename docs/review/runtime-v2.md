# Runtime v2 独立审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），只审当前工作树；范围覆盖 `slterm_app/src/local_control`、`slterm_runtime_protocol`、随包 schema／参考文档、Runtime 设计／规格／追溯，以及 AppRoot 中承接 Runtime 事件与调用的接线。

## 覆盖与局限

- 已读文档：根 `CLAUDE.md`、`slterm_app/CLAUDE.md`、`slterm_app/src/local_control/CLAUDE.md`、`slterm_runtime_protocol/CLAUDE.md`、`slterm_app/src/app_shell/CLAUDE.md`、`slterm_app/src/app_shell/app_root/CLAUDE.md`、`slterm_app/src/persistence/CLAUDE.md`；`docs/devPlan/design/runtime.md`、`lifecycle.md`、`workspace.md`、`testing.md`；`docs/featureSpec/runtime.md`；`docs/devPlan/plan.md`、`progress.md`、`traceability.md` 的 Runtime v2 行；`.claude/docs/architecture/overview.md`、`.claude/docs/testing/exemptions.md`；`assets/runtime-control-api.md` 与随包 schema。
- 已读代码：`local_control` 的 server／service／mutations／operations／events／reads／git／credentials／discovery／client／startup／mod 及对应测试；`slterm_runtime_protocol` 的 protocol／method／operation／schema／生成 binary／协议测试；`cli.rs`；AppRoot 的 runtime_snapshot／events／commands／layout／batch_close／project_close／terminal_close／documents／hooks_page；`app_shell/global_close.rs` 与启动接线；`persistence/operation.rs`、`workspace_writer.rs`；`git/mod.rs` 与 `git_changes.rs`；`slterm_agents` 的会话与命令构造；终端 `TextSnapshot` 路径。
- 未执行：按合同不运行 cargo、构建、测试或网络，未启动真实 GUI、真实 CLI、真实 PTY、真实 Windows 凭据或真实桌面会话；无法运行 schema 生成器，schema 与资产的一致性只能做静态阅读；无法对驱动盘绝对路径、慢客户端、写入失败和崩溃窗口做运行时复现。
- 局限：本报告的“确认”表示代码路径已闭合阅读，不等于已通过运行时复现；`docs/review/` 下已有其他审查对部分同根问题有另一视角记录，本报告仍从 Runtime v2 合同角度保留证据，未以其他报告替代本范围判断。

## 问题

### 阻断｜`git.diff` 的路径校验放过盘符绝对路径，工作区读取可越过仓库根

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol.rs` 的 `valid_repo_relative_path`、`runtime_request_is_valid`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `runtime_git_diff_target`；`slterm_app/src/local_control/git.rs` 的 `git_diff_snapshot`；测试例 `runtime_git_diff_target_validates_repository_relative_path`、`git_diff_snapshot_reads_head_against_worktree`。
- 现象：写请求路径校验只拒绝以 `/` 开头、含反斜杠、含 NUL 或含 `.`／`..` 组件的文本。`C:/...`、`D:/...` 这类盘符绝对路径的两个组件都不是 `.`／`..`，因此被视为合法“仓库相对路径”。`git_diff_snapshot` 随后执行 `worktree_root.as_path().join(&target.path)`；Windows 的 `Path` 拼接遇到绝对路径会整体替换，之后 `std::fs::File::open` 直接打开工作区目标。`head_file` 也会按该绝对路径找到其所在仓库并计算差异。结果是 `git.diff` 可以读取任意位于某个 Git 仓库内的文件，而不仅是当前项目或当前仓库。
- 根因：把“不以分隔符开头”等同于“仓库相对”，没有调用 `Path::is_absolute`、盘符前缀或根组件检查，也没有在拼接后复核解析后的目标仍位于捕获的 worktree 根内；工作区读取没有复用 `DocumentPathResolver` 或 `PathPolicy` 的读取授权与文件身份链。协议 crate 与 AppRoot 各自维护一份同样的词法校验，两份都遗漏了盘符前缀。
- 影响：持有 Runtime token 的本机进程可借 `git.diff` 读取项目根外、其他仓库内的源代码或配置文件正文，违反 featureSpec 的“读取不得泄露未获授权的文件内容”与设计中的仓库/项目授权边界；若目标是凭据、CLI 配置或会话文本，构成敏感材料外流。既有符号链接旁路是同根问题，但盘符路径绕过发生在词法校验入口，独立成立。
- 证据：`valid_repo_relative_path` 只检查空串、首字符 `/`、`\0`／`\`、空组件与点组件；`runtime_git_diff_target` 复述同一条件；`git_diff_snapshot` 的 `worktree_root.as_path().join(&target.path)` 没有 `canonicalize`、`starts_with(worktree_root)`、reparse 拒绝或 `PathPolicy::authorize`；`git_diff_snapshot_reads_head_against_worktree` 直接构造 `RuntimeGitDiffTarget`，未经过路径校验；`runtime_git_diff_target_validates_repository_relative_path` 的非法样例不含盘符绝对路径。写入与打开方法另有 `starts_with(write_root)` 或状态条目匹配，因此这个缺口先落在 `git.diff`。
- 优化方向：仓库相对路径必须是根组件为空、无盘符前缀的路径；拼接后按解析后的规范路径、文件身份与 `PathOperation::Read` 授权复核目标仍在捕获的 worktree 根内，对 reparse point 明确拒绝。若 Runtime 只允许读取已授权 Document，则改为 DocumentId 目标并复用既有打开授权。补盘符、UNC、设备名、根组件、链接与真实仓库的负例。

### 阻断｜快照 revision 固定为握手起点，事件续传会进入不可恢复的 resync 循环

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `RuntimeMethod::Snapshot` 分支；`slterm_app/src/local_control/service.rs` 的 `on_snapshot`、`RuntimeCallbacks`、`HelloResponse::new`；`slterm_app/src/local_control/events.rs` 的 `EventRing::resume`、`subscribe_ring`、`EventHub::subscribe`；测试例 `runtime_snapshot_returns_injected_resources_with_hello_generation`、`event_hub_backlog_over_client_capacity_requires_resync`；`slterm_app/src/local_control/CLAUDE.md` 与 `slterm_app/src/app_shell/app_root/CLAUDE.md` 的相关说明。
- 现象：`runtime.snapshot` 的 `revision` 始终取 `hello.revision`，而 `hello.revision` 在服务启动时固定为零；事件环自己从零开始单调分配 revision，却没有任何端口把当前末尾 revision 提供给快照。客户端按快照 revision 订阅；一旦事件环中自零以来的积压超过客户端队列容量，`subscribe_ring` 就返回 `resync_required`。客户端重新快照仍得到零，再次订阅仍失败，形成不可恢复循环。积压未超过队列时，客户端会重放快照之前已经发生的事件，快照 revision 与其内容不一致。
- 根因：`RuntimeCallbacks` 没有“事件环最新 revision”端口，`on_snapshot` 只生成资源；`server.rs` 的注释仍以“事件环接入前”的占位语义实现，Hub 已接入但快照代次未接管。CLAUDE 决策记录把“revision 固定取握手起点”写成当前事实，掩盖了与设计“快照与事件同一代次”的冲突。
- 影响：Runtime 事件订阅在正常使用、事件数超过客户端队列后对任何新连接或重订阅失效；`resync_required` 的文档恢复路径（重新快照）永远无法成功。快照内容可能晚于其 revision，客户端依 revision 拼接状态会得到错误增量。这是 Runtime 生命周期与事件验收的核心行为，不成立。
- 证据：`server.rs` 的快照响应使用 `hello.revision`；`service.rs` 构造 `HelloResponse::new(..., 0, ...)`；`RuntimeCallbacks` 字段中没有 revision；`EventRing::resume` 在积压大于客户端队列时返回 `ResyncRequired`；`subscribe_ring` 在 `backlog.len() > CLIENT_QUEUE_CAPACITY` 时拒绝；`runtime_snapshot_returns_injected_resources_with_hello_generation` 把注入的 hello revision 当成快照 revision 断言，固化了占位行为；无测试覆盖“快照→订阅→积压→重新快照”的真实链路。
- 优化方向：快照必须在同一时刻取得事件环当前末尾 revision，并与 `store_epoch`、`stream_epoch` 一起返回；hello、snapshot、subscribe 的 revision 语义应统一。补围绕客户端队列容量、事件环淘汰和慢客户端的端到端用例，覆盖重新快照后的成功订阅。

### 阻断｜非交互关闭在保存、停止或清理落定前返回 Succeeded

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `start_runtime_panel_close`、`start_runtime_project_remove`、`execute_runtime_mutation` 的 `TerminalClose`／`AgentClose` 分支；`slterm_app/src/app_shell/app_root/batch_close.rs` 的 `begin_views_close`、`close_view_from_request`；`slterm_app/src/app_shell/app_root/project_close.rs` 的 `remove_project_with_decisions`、`start_project_save`；`slterm_app/src/app_shell/app_root/terminal_close.rs` 的 `close_terminal_persisted`、`start_terminal_commit`；`slterm_app/src/local_control/service.rs` 的 `on_mutation`；测试例 `runtime_panel_close_save_policy_persists_before_destroying`、`runtime_project_remove_honors_discard_and_save_policies`。
- 现象：`panel.close`、`project.remove`、`terminal.close` 与 `agent.close` 的 AppRoot 执行体在登记关闭协调器、启动保存或提交任务后立即返回 `RuntimeMutationReply::Done(Ok(()))`。派发器据此把 Operation 写成 `Succeeded`。真实保存、删除意图提交、PTY 停止、Panel 销毁和清理都在后续任务或事件泵中完成；任何后续失败只进入 UI 通知与本地关闭失败态，Runtime 端已持有成功结果。
- 根因：关闭方法没有像 `panel.open_file`／`panel.save` 那样延迟完成回执，也没有把关闭协调器的 `Completed`／`Failed` 结算映射成 Operation 终态；`RuntimeMutationCompletion` 只用于打开与保存路径。关闭协调器的异步阶段与 Operation 的同步回执之间没有桥。
- 影响：featureSpec 明确要求“保存失败返回失败并保持现场，不得自动转为丢弃”。当前客户端选择 `documents=save` 后可能在保存尚未开始时即收到成功；保存失败后面板与正文仍在，但 Operation 已不可重试（同 mutation_id 返回原成功记录）。`terminal.close` 在 PTY join 前就把逻辑终端移除并报告成功，join 失败后 Runtime 无入口重试，可能留下孤儿 PTY。自动化调用方会把未落定的关闭当成事实，造成状态与数据承诺不一致。
- 证据：`close_view_from_request` 仅在 `begin_views_close` 完成收集、登记和异步任务启动后返回 `Ok`；`remove_project_with_decisions` 在 `start_project_save` 或 `start_project_commit` 后返回 `Ok`；`close_terminal_persisted` 在 `start_terminal_commit` 后返回 `Ok`；`service.rs` 的 `Done(Ok(()))` 直接映射 `DispatchOutcome::Succeeded`。测试先断言 `Done(Ok(()))`，随后轮询视图消失或文件内容，等于接受“回执早于副作用”。
- 优化方向：关闭类写方法的 Operation 终态必须等到协调器真实完成；若产品坚持“accepted 即成功”，则必须在线缆与分析中引入明确的“已接受、仍在推进”状态，不能复用 `Succeeded`。保存失败、PTY 停止失败和提交失败必须成为可查询的失败或结果未知，并允许同一意图在安全条件下重试。

### 重要｜事件环移除滞后订阅后，订阅连接保持静默

- 置信度：确认
- 位置：`slterm_app/src/local_control/events.rs` 的 `deliver_event`、`subscribe_ring`；`slterm_app/src/local_control/server.rs` 的 `stream_subscription`；测试例 `lagging_subscription_is_removed_without_blocking_publishers`。
- 现象：客户端队列满时 `deliver_event` 把该订阅的发送端从订阅表移除；事件写线程随后在队列读空后看到 `Disconnected` 并退出，或在写失败时退出。主连接线程仍阻塞在 `read_frame` 等待取消订阅或客户端断开，没有观察事件线程已经结束。客户端因此收不到后续事件、收不到 resync 错误、也收不到 EOF，直到它自己发送数据或断开；连接继续占用 worker 与连接限额。
- 根因：事件线程与订阅 worker 之间没有终止信号；`stream_subscription` 只把事件线程当作并行写者，没有把“事件源已失效”变成连接级结论。设计要求的“滞后客户端只终止其订阅并要求重新快照”只实现了移除订阅，没有实现“要求重新快照”。
- 影响：客户端无法区分“暂时无事件”和“订阅已被服务端丢弃”，会永久停在静默状态；一个慢消费者可以占住连接槽位。`runtime_events_visibility_and_generation` 与事件环单测看不到这一层，因为测试直接操作发送端或注入事件回调，没有经过完整连接。
- 证据：`deliver_event` 的 `retain` 只丢弃发送端；事件线程在 `TrySendError::Full`／`Disconnected` 或 `write_json` 失败时 `break`；`stream_subscription` 主循环只解析 unsubscribe，没有事件线程句柄、完成标志或取消通道；`EventHub::subscribe` 返回的 `Subscription` 没有“源已结束”回调。
- 优化方向：订阅 worker 必须观察事件源终止并主动结束连接或发送结构化 `resync_required`；滞后与写失败应作为可观察终态。补慢消费者、队列满、写失败和客户端空闲的端到端用例。

### 重要｜`expected_revision` 在线上端到端未生效，资源代次与授权代次没有进入 Operation

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `respond_mutation`；`slterm_app/src/local_control/service.rs` 的 `on_mutation`；`slterm_app/src/local_control/operations.rs` 的 `DispatchRequest`、`OperationDispatcher::dispatch`；`slterm_app/src/local_control/mutations.rs` 的 `RuntimeMutationRequest`；测试例 `stale_expected_revision_is_rejected_before_registration`。
- 现象：线缆上的 `MutationParams.expected_revision` 被 server 解析并继续传到 `RuntimeMutationRequest`，但 `service.rs` 在构造 `DispatchRequest` 时无条件写入 `expected_revision: None`、`current_revision: 0`、`resource_generations: BTreeMap::new()`。派发器支持的比较分支因此在生产路径永不触发；`resource_generations` 也恒为空，Operation 的持久记录没有资源代次事实。唯一覆盖 `expected_revision` 的用例直接调用派发器并手工设置字段，不经过 Wire、service 与 AppRoot。
- 根因：服务层没有“当前资源代次／授权代次”的端口；派发器的能力切片完成后没有接线，反而以服务层固定空值绕过了合同。`RuntimeMutation` 也不携带可达的授权快照，无法在执行前复核。
- 影响：调用方提供的过期快照不会被拒绝，破坏操作会在它已经观察到的状态之外执行；featureSpec 的“不得把过期快照当作执行授权”与设计中的“资源代次、授权代次、store_epoch 在执行前复核”均不成立。Operating 的幂等记录缺少资源代次，也无法用于审计或恢复判断。
- 证据：`service.rs` 的 `DispatchRequest` 字面量；`operations.rs` 的比较只在 `expected_revision` 为 `Some` 时运行；`operation_dispatch_tests.rs::stale_expected_revision_is_rejected_before_registration` 是该逻辑的唯一测试；server 测试与 AppRoot 测试发送的 `expected_revision` 全为 `None`。
- 优化方向：由前台在请求时刻给出可比较的资源代次和授权代次，随写意图与 Operation 持久化；登记前与执行前都要复核，失配返回 `revision_conflict` 且不消耗幂等键。补 Wire 到 service 到 AppRoot 的跨层用例，以及恢复后旧代次失效的用例。

### 重要｜Operation 停在 Queued／Running 后没有恢复路径，OutcomeUnknown 未覆盖持久化失败

- 置信度：确认
- 位置：`slterm_app/src/local_control/operations.rs` 的 `OperationDispatcher::dispatch`、`await_writer`；`slterm_app/src/persistence/operation.rs` 的 `transition_operation`、`OperationStatus::transition`；`slterm_app/src/persistence/workspace_writer.rs` 的 `register_operation`／`transition_operation`；`.claude/docs/testing/exemptions.md`；测试例 `runtime_outcome_queryable`。
- 现象：登记成功但推进到 `Running` 的回执超时、队列满或写线程停止时，派发器直接返回错误，数据库中的 Operation 仍为 `Queued`；最终终态推进失败时，Operation 停在 `Running`。进程在登记后、推进前或副作用后崩溃，同样留下 `Queued`／`Running`。同一 `mutation_id` 重试只会返回原记录且执行体不重跑；维护期没有任何 Runtime Operation 对账器把这些非终态收敛为 `Failed`、`Cancelled` 或 `OutcomeUnknown`。状态机本身也不允许 `Queued` 直接进入失败或未知终态。
- 根因：Runtime Operation 只经写线程登记与推进，但没有登记对应的维护期恢复器；写入回复的超时／断开被视为调用错误而不是操作终态事实。`OutcomeUnknown` 只在壳回执超时或断开时产生，没有覆盖“副作用可能已发生但持久终态无法证明”的其他断点。
- 影响：幂等命名空间可被永久占用，客户端 `operation.wait` 最终只得到超时，无法取得设计承诺的“进行中、成功或失败”之一的可靠终态；如果副作用实际已发生，状态仍是 `Running`，既不是成功也不是结果不明。恢复后旧 Operation 仍可查询但永不结束。
- 证据：`dispatch` 在 `register_operation` 成功后才推进 Running，两步各自 `await_writer` 有界超时；任一步失败直接 `Err` 返回；`transition_operation` 只有 `Queued → Running` 与 `Running → 终态`，没有失败出口；`maintenance`／`startup` 代码中没有 Runtime Operation 枚举或对账。`runtime_outcome_queryable` 只测试正常执行返回 `OutcomeUnknown` 与重复键不重跑。
- 优化方向：为 Runtime Operation 登记维护期与关闭期对账，把无法证明的中间态收敛为可查询的未知／失败终态；登记回复超时也不得把键永久留在无执行状态。补超时、写线程停止、崩溃重启和终态推进失败的用例。

### 重要｜项目、布局、面板与终端写 Operation 在持久快照提交前报告成功

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/mod.rs` 的 `commit_project_snapshot`、`publish_project_snapshot`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `execute_runtime_mutation`；`slterm_app/src/persistence/workspace_writer.rs` 的项目快照去抖；`docs/devPlan/design/runtime.md` 的“写操作与幂等”。
- 现象：`project.activate`、`project.rename`、`project.reorder`、布局操作、`terminal.create`、`terminal.rename`、`terminal.reconnect`、`panel.open_file`、`agent.start` 等先把候选提交到内存快照，再经去抖队列写入工作区库。AppRoot 返回 `Ok` 后，派发器立刻把 Operation 写成 `Succeeded`；此时快照可能尚未提交，写线程可能队列满、失败或进程崩溃。设计要求“纯数据库操作按事务结果与提交指纹判定”。
- 根因：Operation 的完成语义绑定的是前台状态提交，不是持久事务结果；没有从 writer 回传快照提交指纹并把 Operation 终态推迟到该回执。
- 影响：客户端收到成功后重启，可能看到项目、布局、终端记录或 Document 位置没有变化；Operation 记录与工作区事实分叉。对依赖幂等回执编排的自动化，这是错误成功。若写线程持续失败，Operation 仍显示成功，恢复后也无从判断。
- 证据：`commit_project_snapshot` 递增内存代次、替换 `self.snapshot` 后调用 `publish_project_snapshot`；后者向 writer 队列非阻塞提交，失败只保留待写副本。`execute_runtime_mutation` 的各命令返回 `Ok` 后即结束；派发器没有等待项目快照提交回执；设计中的“事务结果与提交指纹”没有对应端口。
- 优化方向：要么让这些 Operation 等到 writer 的快照提交回执与指纹，要么把 Operation 语义改为“已应用到运行态、持久化待完成”的可查询状态并让客户端可见。补写队列满、写线程失败、提交前崩溃与重启后查询的用例。

### 重要｜Hooks 草稿在决定齐备前被丢弃，失败请求仍摧毁未保存内容

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `start_runtime_application_close`、`start_runtime_project_remove`；`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `discard_all_hooks_drafts_for_close`、`discard_project_hooks_drafts_for_close`；测试例 `runtime_close_requires_decision`、`runtime_project_remove_requires_document_decision`、`integration_runtime_noninteractive_close`。
- 现象：`app.close` 与 `project.remove` 在存在 Hooks 草稿且调用方给出 `hooks=discard` 时，先清空草稿与 Hooks 编辑器文档，再检查 Document 决定是否齐备。若 Document 策略缺失，函数随后返回 `decision_required`；若保存路径需要 writer 而 writer 不存在，则返回 `NotReady`。请求整体失败，但 Hooks 草稿已经被丢弃，Operation 记录为失败，调用方无法知道失败前已发生不可逆内存副作用。
- 根因：把“显式策略的应用”放在了“全量决定收集与可行性预检”之前；关闭协调器的“收集决定后再进入保存／清理”顺序没有覆盖 Hooks 草稿。
- 影响：非交互关闭的失败语义被破坏。调用方通常在 `decision_required` 后修正策略并以新 `mutation_id` 重试；此时 Hooks 草稿已消失，无法恢复。对用户而言，一次被拒绝的关闭仍丢失未保存的 Hooks 编辑，违反“失败保持现场”与“不自动丢弃”的红线。
- 证据：`start_runtime_application_close` 在 `hook_drafts` 非空且 `hooks=discard` 时立即 `discard_all_hooks_drafts_for_close`，之后 `start_application_close_with_decisions` 才可能返回 MissingDecision／Rejected；`start_runtime_project_remove` 的 `discard_project_hooks_drafts_for_close` 同样早于 Document 收集。现有测试全部传 `hooks: None` 且没有草稿，因此未覆盖该顺序。
- 优化方向：先完成所有作用域的收集、决定与可行性预检，再按统一提交顺序应用 Hooks 丢弃、Document 保存与关闭；失败时不触碰任何草稿。补“有 Hooks 草稿 + 缺 Document 策略”“有草稿 + 无 writer 保存”的失败用例。

### 重要｜合同冲突：`app.close` 回执语义与“保存失败返回失败”相互冲突

- 置信度：确认
- 位置：`docs/featureSpec/runtime.md` 的「调用语义」与「验收判据」；`slterm_app/src/local_control/CLAUDE.md` 的「能力边界」；`slterm_runtime_protocol/CLAUDE.md` 的「应用聚焦与关闭」；`slterm_app/src/local_control/server.rs` 与 AppRoot 的 `start_runtime_application_close`；测试例 `runtime_close_requires_decision`、`integration_runtime_noninteractive_close`。
- 现象：featureSpec 要求脏关闭保存失败时返回失败并保持现场，不得自动转丢弃；实现文档则明确把 `app.close` 的成功定义为“关闭已开始推进”，进程退出与连接断开仍可能发生。代码在保存任务启动前就返回成功，保存失败只进入 UI 通知。两侧都各自有文本依据，无法由本报告判定哪一方优先。
- 根因：关闭主干的异步阶段与 Operation 同步回执之间没有统一的“接受／落定”语义，featureSpec 与 Runtime 线缆文档对 `app.close` 的承诺不一致。
- 影响：客户端无法从回执判断保存结果；若按 featureSpec 期待失败，会高估关闭失败可见性；若按实现文档期待“已接受”，会忽略保存失败。验收与实现各自都能声称成立。
- 证据：featureSpec 原文要求保存失败返回失败并保持现场；`local_control/CLAUDE.md` 与协议 crate 文档写“接受后回执只表示关闭已开始推进”；`start_runtime_application_close` 对 `Started`／`Deferred` 均返回 `Ok`；测试在保存完成前接受回执。
- 优化方向：先裁决 `app.close` 究竟是“接受”还是“完成”语义，再让线缆文档、Operation 状态、客户端重试规则和测试一致；如果保留接受语义，应在随包合同和错误码中显式表达，而不是复用 `Succeeded`。

### 重要｜合同冲突：非交互关闭缺少运行终端处置策略

- 置信度：确认
- 位置：`docs/devPlan/design/runtime.md` 的「关闭与读取安全」；`docs/featureSpec/runtime.md` 的「调用语义」；`slterm_runtime_protocol/src/protocol.rs` 的 `AppCloseRequest`、`ProjectRemoveRequest`、`PanelCloseRequest`；`assets/runtime-control-api.md` 的「非交互关闭」；`slterm_app/src/app_shell/app_root/project_close.rs`、`terminal_close.rs`、`batch_close.rs` 的 `CloseObject` 构造。
- 现象：设计要求 `app.close`、`panel.close`、`project.remove` 的调用方必须显式提供 Document、Hooks 和运行终端处置策略，缺少决定返回 `decision_required`。线缆 DTO 只有 `documents` 与 `hooks` 字段；实现把终端对象的 `decision` 固定为 `NotRequired`，直接停止运行中的 PTY，非交互调用方没有选择保留或确认运行的入口。
- 根因：协议 DTO 与关闭协调器的对象决定集合没有扩到终端；设计、随包参考与实现各自停留在不同版本。
- 影响：运行中的 AI CLI 或长任务会在非交互关闭中被直接停止，调用方无法表达“保留终端／拒绝关闭／等待”等决定；`decision_required` 的语义只覆盖文档与 Hooks，不覆盖设计声明的完整关闭集合。featureSpec 的非交互决定要求不完整。
- 证据：各关闭请求 DTO 的字段集；`runtime_snapshot.rs` 把 `documents`／`hooks` 翻译成 Document 决定；`project_close.rs` 为项目终端构造 `CloseDecision::NotRequired`；`terminal_close.rs` 与 `batch_close.rs` 同样不做终端决定收集；随包 API 文档只列 Document／Hooks 策略。
- 优化方向：要么增加封闭的终端处置策略并进入关闭决定收集，要么在 featureSpec 与设计中把终端明确排除并说明运行 PTY 会被自动停止。无论哪种，线缆、随包文档、协调器和测试必须同源。

### 重要｜控制面一次 join 失败被永久重放，正常退出可能无法完成

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `stop`、`termination_error`；`slterm_app/src/local_control/service.rs` 的 `shutdown`、`shutdown_components`；`slterm_app/src/app_shell/global_close.rs` 的 `run_worker` Close 分支；测试例 `local_control_shutdown_replays_worker_join_failure`、`local_control_shutdown_continues_after_worker_join_failure`、`service_shutdown_attempts_both_failures_and_returns_first_error`。
- 现象：`LocalControlServer::stop` 第一次关闭时保存 join 错误；以后每次 `shutdown` 都重放同一错误。`LocalControlService::shutdown` 不区分“已经关闭但曾有一次 join 错误”与“本次关闭新失败”，继续把该错误返回给 `GlobalCloseRuntime`。后者把任何 `ControlPlaneCleanup::Failed` 当成不可继续，直接向窗口返回 `ControlPlaneCleanupFailed`，不消费 writer、不提交正常退出；用户再次关窗只会得到同一错误。发布发现失败且 server 回收失败时也会进入该状态，因为服务已标记关闭但后续关闭仍重放终止错误。
- 根因：终止错误的重放语义只为可观察性设计，没有“已终态且资源已回收”的成功判定；全局关闭把控制面错误当成必须阻止退出的唯一事实，没有知情放弃或只读诊断出口。
- 影响：一次 worker panic／join 失败或发布失败后的回收错误会使正常退出永久不可达；用户只能强杀，`clean_shutdown` 无法写入，更新安装、托盘退出与系统关机路径都会受阻。该行为与“重复调用保持安全”和“显式 shutdown 汇总错误”的目标不一致。
- 证据：`stop` 的 `shutdown.swap` 后重放 `termination_error`；`service.shutdown` 无条件调用 `shutdown_components`；`GlobalCloseRuntime::run_worker` 在 `control.shutdown() == Failed` 时发送错误并 `continue`，不进入 writer 关闭；`local_control_shutdown_replays_worker_join_failure` 明确断言重复调用仍返回同一错误；没有“已关闭但清理曾失败时允许退出”的用例。
- 优化方向：区分“资源仍存活且本次关闭失败”与“资源已回收但存在历史终止错误”，后者不应阻止正常退出；若产品要求 fail-closed，必须提供可见的知情放弃／诊断出口，而不是无限重试同一错误。补 join 失败后重试、发布失败后关闭、更新移交与托盘退出的用例。

### 重要｜随包 schema 未覆盖多数方法的请求结构，且 method 与 params／result 没有关联校验

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol.rs` 的 `RuntimeParams`、`RuntimeRequest`、`RuntimeResult`；`slterm_runtime_protocol/src/method.rs` 的 `MethodSpec.params_schema`／`result_schema`；`slterm_runtime_protocol/src/schema.rs` 与 `src/bin/runtime_schema.rs`；`assets/runtime-api-v2.schema.json` 的 `$defs` 与 `RuntimeRequest`；测试例 `schema_functions_cover_every_registered_method`、`schema_accepts_real_requests_and_rejects_unknown_methods`。
- 现象：生成根是 `ControlMessage`，其中 `RuntimeRequest.params` 只引用手工维护的 `RuntimeParams` 联合；该联合只包含元方法、项目方法与部分面板／终端方法。Agent、Git、终端读取／等待／关闭／重连／重命名、面板打开文件／保存／关闭、应用关闭等请求 DTO 没有进入随包 schema。即使某个方法的结构已在联合中，`method` 与 `params` 也彼此独立，schema 不能表达“该方法的参数必须是该 DTO”；结果侧同样是 `RuntimeResult` 的联合，不与方法关联。`MethodSpec` 中逐方法注册的 schema 函数只在测试中被调用，生成器没有遍历注册表。
- 根因：schema 生成没有按“方法注册表驱动根定义”实现，而是维护了第二份手工联合；注册表里的 schema 入口只做对象性检查，没有参与资产生成。
- 影响：featureSpec 把随包文档与 schema 定为“方法、参数、错误码、事件和发现格式的唯一真值”，但 schema 无法校验 `terminal.read`、`git.diff`、`agent.input`、`panel.open_file` 等合法请求；契约客户端或代码生成会得到不完整、且不能按方法区分的结构。schema 与代码的漂移不仅是字节是否一致，还包括“方法面覆盖”这一层。
- 证据：资产 `$defs` 中不存在 `TerminalReadRequest`、`TerminalWaitRequest`、`TerminalCloseRequest`、`TerminalReconnectRequest`、`TerminalRenameRequest`、`AgentInputRequest`、`AgentCloseRequest`、`AgentReadRequest`、`AgentWaitRequest`、`AgentStartRequest`、`AgentResumeRequest`、`AgentForkRequest`、`GitStatusRequest`、`GitDiffRequest`、`GitRollbackRequest`、`GitDeleteRequest`、`GitOpenDiffRequest`、`GitOpenDeletedRequest`、`PanelOpenFileRequest`、`PanelSaveRequest`、`PanelCloseRequest`、`AppCloseRequest`；`RuntimeParams` 的 `anyOf` 只引用已有子集；`RuntimeRequest` 的 `method` 与 `params` 没有条件绑定；`schema_accepts_real_requests_and_rejects_unknown_methods` 只验证 hello、handover、describe、snapshot、operation 与少量错误形状。
- 优化方向：从方法注册表生成带方法判别的方法级请求／结果定义，至少让每个已登记方法的 request 与 result 都进入资产并可被真实请求验证；补逐方法 schema 用例，而不是只检查 schema 函数返回对象。

### 重要｜Runtime 合同测试不在任何固定门禁内，资产漂移不会被发现

- 置信度：确认
- 位置：`slterm_runtime_protocol/Cargo.toml` 的 `schema` feature 与 `runtime-schema` 的 `required-features`；`slterm_runtime_protocol/src/protocol_tests.rs` 的 `generated_schema_matches_the_packaged_asset`、`schema_generator_*`、`schema_accepts_*`；`.github/workflows/ci.yml`；`.claude/docs/operations/commands.md`；`slterm_app/src/local_control/service.rs` 的 `include_bytes!`；`docs/devPlan/design/runtime.md` 与 `docs/devPlan/plan.md` 的 schema 验收。
- 现象：schema 生成、逐字节资产比较与真实请求校验全部被 `#[cfg(feature = "schema")]` 门控，`runtime-schema` 也带 `required-features`。全量 workspace 测试、core release、GPUI 特征测试、clippy 与发布构建都没有启用该 feature；命令矩阵同样没有 schema 定向命令。随包资产以静态文件被 `include_bytes!` 进二进制，漂移不会被编译发现。
- 根因：把可选 feature 的生成与比对当成人工操作，没有把 Runtime 合同纳入“门禁常挂”；测试里的字节比较只保证“在有人运行时一致”。
- 影响：DTO／方法注册表变化后，资产可以继续发布旧结构；hello／describe 的 schema 指纹取自旧资产，客户端按错误合同校验。方法注册表、生成器、资产与测试四者之间的“同源”没有机械保证，设计验收中的 schema 漂移测试实际不产出证据。
- 证据：`protocol_tests.rs` 中所有 schema 用例均带 feature 门控；CI 命令无 `--features schema`；命令矩阵无协议 crate 的 schema 命令；`build-windows-product.ps1` 不以 schema feature 构建；`runtime-control-api.md` 写了定向命令但没有进入门禁。
- 优化方向：把协议 schema 生成与逐字节比较纳入 CI／发布门禁，并对生成 binary 执行编译与运行；若继续人工执行，必须在豁免表登记根因与兜底层级，不能继续宣称已自动化。

### 重要｜Runtime 双进程 conformance 与事件断线矩阵缺失，追溯仍标自动化

- 置信度：确认
- 位置：`docs/devPlan/design/testing.md` 的「测试层次与隔离」与 Runtime 行；`docs/devPlan/design/runtime.md` 的「验收与测试」；`docs/devPlan/plan.md` 的 Runtime v2 交付、退出证据与集成检查点；`docs/devPlan/traceability.md` 的 Runtime v2 行；`docs/devPlan/progress.md` 的 Runtime v2 行；`.claude/docs/testing/exemptions.md`；`scripts/tests/test_runtime_retirement.py`；`slterm_app/tests/` 与 `slterm_app/src/local_control/server_tests/`。
- 现象：测试设计把“双进程 conformance”列为 Runtime 的主要证据，要求启动真实 GUI 与 CLI、使用临时 AppPaths、隔离数据根与假凭据；计划把真实 GUI／CLI 的握手、断线重查、重复 mutation、事件缺口、非交互关闭、关闭中拒绝与 schema 漂移列为退出证据，并把 Runtime 双进程断线列为集成检查点。实际测试都是进程内的真实 loopback 服务与虚拟窗口 AppRoot，没有真实 GUI 进程加真实 CLI 客户端的矩阵，也没有“全方法 × 事件断线”的逐方法覆盖。`test_runtime_retirement.py` 反而断言旧 conformance target 不存在；豁免表没有登记该缺口；追溯与进度仍写已自动化／已验收。
- 根因：旧 Runtime conformance target 随旧栈删除后没有以新形态重建；进程内用例被用来替代真实进程边界，计划退出证据没有对应产物。
- 影响：发现、认证、跨进程凭据轮换、首帧后开放、事件续传与断线、非交互关闭、关闭中拒绝和 v1 拒绝都没有在真实进程边界上验证；进程内替身看不到连接生命周期、真实 CLI 解析、真实窗口与真实凭据域行为。Runtime v2 的“完成”叙述高于可复现证据。
- 证据：`slterm_app/tests/` 只有 i18n 合同与 console 启动目标；`local_control/server_tests` 与 `app_root/tests/runtime_*` 都是进程内；`test_runtime_retirement.py` 的 `test_legacy_runtime_implementation_is_removed` 明确排除 `slterm_app/tests/runtime_conformance.rs`；`exemptions.md` 无 Runtime conformance 行；`progress.md` 的 Runtime v2 行写已验收。
- 优化方向：以真实 GUI 进程与真实 CLI 客户端重建 conformance，覆盖已登记方法与事件断线矩阵；若暂不实现，必须在豁免表登记根因、兜底层级与人工核验方式，并把追溯／进度降级为未验收。不能把进程内替身测试写成双进程验收。

### 重要｜事件面只发布项目激活，没有终端／面板／资源生命周期事件，事件负载也没有资源输出 revision

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `publish_runtime_event`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `activate_project`；`slterm_app/src/app_shell/app_root/CLAUDE.md` 的「Runtime 事件发布」；`slterm_runtime_protocol/src/protocol.rs` 的 `RuntimeEvent`；测试例 `runtime_events_visibility_and_generation`、`runtime_events_are_suppressed_after_close_starts`、`runtime_events_are_suppressed_while_close_is_in_progress`。
- 现象：生产发布点只有一个：项目激活时发布 `project.activated`。终端创建／输入／退出／关闭／重命名、面板打开／关闭／激活／移动、项目重命名／移除、布局变化与 Git 写都没有事件。`RuntimeEvent` 只有全局事件 revision、kind、resource 与 generation，没有资源自己的输出 revision 或资源 revision；设计所说“事件只携带输出 revision”无法表达。测试中的 `panel.closed`、`terminal.output` 都是合成 kind，不来自生产发布点。
- 根因：事件接入按方法族逐个排期，但 Runtime 能力与文档已经把这些方法登记为可用；事件的单一发布 helper 没有资源类型矩阵，也没有和资源代次／输出 revision 的绑定。
- 影响：订阅者无法从事件得知资源变化，只能轮询快照或读取；featureSpec 的“事件按资源身份和单调代次投递”只对项目激活成立，设计中的终端输出事件与面板事件没有落地。事件负载没有资源 revision，客户端无法把事件与 `terminal.read`／`terminal.wait` 的 revision 对齐。
- 证据：`publish_runtime_event` 的生产调用点只有 `activate_project`；AppRoot 文档明确写“当前含项目激活；面板与终端事件随各方法族接入”；`RuntimeEvent` 字段；`runtime_events_visibility_and_generation` 只验证项目激活与关闭抑制，`runtime_events_are_suppressed_*` 用任意 kind 直接调用 helper。
- 优化方向：为每个已承诺的资源方法定义事件 kind、资源身份、资源代次与输出 revision，并在前台状态提交成功后发布；补“终端输出→事件 revision→分页读取／等待”的链路测试。若事件面暂不完整，应把 `events.subscribe` 的能力描述和 featureSpec 同步降级。

### 重要｜路由与能力摘要不按方法注册表作用域执行

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `handle_connection` 运行时／handover 分支；`slterm_runtime_protocol/src/method.rs` 的 `allows`、`methods_for`、`method_spec`；`slterm_runtime_protocol/src/protocol.rs` 的 `Capabilities::current`、`DescribeResponse::current`；测试例 `runtime_describe_returns_registry_and_contract_hashes`。
- 现象：注册表声明每个方法允许的作用域，但 server 的路由与作用域判断只看请求枚举变体（Runtime 请求 / Handover 请求），从不调用 `allows` 或 `methods_for`；`Capabilities` 与 `DescribeResponse` 返回全部注册方法，不按当前连接作用域过滤，因此 Runtime 连接的 describe 与 hello 能力里会出现 `instance.handover`，handover 连接也会看到全部 Runtime 方法。测试甚至把 `instance.handover` 作为 Runtime describe 的期望值。
- 根因：注册表只被当作 describe／capabilities 的展示数据，没有成为路由与作用域授权的唯一事实；`ClientRequest` 的封闭枚举与注册表 scopes 构成双重作用域真值。
- 影响：以后给某个方法改 scopes 或新增登记时，server 仍按请求类型接受，客户端观察到的能力与实际可调用面可能不一致。注册表的“单源”承诺只覆盖展示，不覆盖认证后的路由。
- 证据：`server.rs` 的作用域判断分支只比较 `ControlScope`；`allows`／`methods_for`／`method_spec` 在生产代码中无调用；`Capabilities::current` 直接遍历 `method_names()`；`DescribeResponse::current` 直接遍历 `METHODS`；server 测试的 describe 期望包含 handover。
- 优化方向：路由与作用域授权以注册表 scopes 为唯一判据；hello 能力与 describe 按连接作用域过滤，或明确提供“跨作用域全量注册表”的独立字段。补注册表 scopes 与 server 接受面一致的测试。

### 重要｜`GitStatusResponse.root` 返回项目根而不是仓库根

- 置信度：确认
- 位置：`slterm_app/src/local_control/git.rs` 的 `git_status_response`；`slterm_app/src/local_control/service.rs` 的 `on_git_status`；`slterm_app/src/git/mod.rs` 的 `StatusSnapshot.worktree_root`；`slterm_runtime_protocol/CLAUDE.md` 的「Git 状态」；测试例 `runtime_git_status_routes_project_selection`。
- 现象：`git_status_response` 把调用方传入的 `target.root` 写进响应的 `root` 字段；`on_git_status` 传入的是项目根。`StatusSnapshot` 已经携带真实 `worktree_root`，而条目路径明确是相对仓库根。项目根是仓库子目录时，响应的 `root` 与 `entries` 的参照系不一致。
- 根因：响应映射选了项目根作为展示根，没有使用 Git 读取已经解析出的仓库工作树根；协议文档把该字段定义为“仓库根”，实现没有对齐。
- 影响：客户端用 `root + entry.path` 解析目标会落到错误位置；项目根为子目录时，仓库兄弟路径无法被正确表示。对依赖 Runtime 做自动审阅或文件操作的客户端，这会把仓库相对路径错误地绑定到项目根。
- 证据：`git_status_response(project_id, &target.root, &snapshot)`；`StatusSnapshot.worktree_root: Option<ResolvedPath>` 的注释明确为应用层真实读取的 RepoRoot；`runtime_git_status_routes_project_selection` 使用注入的 `GitStatusResponse`，没有验证真实映射；仓库子目录场景没有用例。
- 优化方向：响应的 `root` 应取本次 Git 读取捕获的 worktree root；无法取得或非 Unicode 时明确失败，不退回项目根。补项目根为仓库子目录、仓库根与项目根不同、非 Unicode 仓库根的用例。

### 重要｜快照不暴露布局树与分割几何，`panel_group.resize` 路径不可推导且视口由客户端自报

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol.rs` 的 `SnapshotResources`、`PanelGroupResource`、`PanelGroupResizeRequest`、`PanelGroupSplitRequest`；`slterm_app/src/local_control/server.rs` 的 `PanelGroupResize`／`PanelGroupSplit` 路由；`slterm_app/src/app_shell/app_root/layout.rs` 的 `split_view_from_request`、`resize_split_from_request`；`docs/devPlan/design/workspace.md` 的「布局树与面板」。
- 现象：快照只返回项目、组 id、组内 ViewKey 顺序和 active view；没有组在分割树中的父链、方向、比例或节点身份。`panel_group.resize` 要求从根到分割节点的布尔路径，客户端无法从快照推导该路径，只能猜测。`panel_group.split` 与 `resize` 的 `viewport` 完全由客户端提供，AppRoot 用它做最小空间校验；没有与真实窗口内容区比较。
- 根因：快照资源模型选择了“组列表”而没有选择“布局树”，但 resize 合同按树路径设计；几何约束依赖客户端自报，没有前台真实视口端口。
- 影响：Runtime 客户端无法可靠使用 resize，只能试探未知路径；客户端可用任意大的 viewport 绕过 120×80 的逻辑最小尺寸检查，使拆分在实际窗口中不可用。快照与事件宣称“同一资源模型”，但布局这一等公民没有对应资源表达。
- 证据：`PanelGroupResource` 只有 `project_id/group_id/views/active_view`；`PanelGroupResizeRequest.path` 是布尔路径；`split_view_from_request` 直接把 `options.viewport` 传入 core；`resize_split_from_request` 同样使用请求视口；`SnapshotResources` 没有 tree 或 geometry 字段；布局测试使用手工路径而不是快照推导。
- 优化方向：快照增加不可变的布局树／分割几何或稳定的树路径身份，使 resize 路径可推导；最小空间校验应使用前台真实内容视口，客户端的 viewport 只能作为提示或被拒绝。补空组、子组、嵌套分割、视口压缩与客户端伪造视口的用例。

### 重要｜等待操作不能因客户端断开而取消，worker 与连接可被长时间占用

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `wait_terminal`、`wait_operation`；`docs/devPlan/design/runtime.md` 的传输约束“所有读取、订阅和等待都可取消并受关闭状态约束”；`slterm_app/src/local_control/CLAUDE.md` 的等待语义；测试例 `runtime_terminal_wait_returns_newer_revision_and_times_out`、`runtime_operation_wait_polls_until_terminal_then_returns_result`。
- 现象：等待循环只检查全局 `shutdown` 标志与 revision／操作状态；客户端断开或不再读取，worker 不会从读线程得到信号，会继续轮询直到自身超时。单连接顺序处理意味着等待期间该连接不能发送取消请求；协议也没有独立取消方法。默认与最大等待可达 600 秒，连接上限固定。
- 根因：等待实现把“客户端连接仍然存活”当成不变量；没有在读循环与等待循环之间共享取消／断线状态，也没有连接级 keepalive 或取消帧。
- 影响：一个客户端可发起多个长等待后断开，持续占用 worker 与连接名额；服务关闭时虽然会设置全局标志，但连接关闭前的时间仍被浪费。设计要求的“可取消”没有实际入口，客户端只能靠超时。
- 证据：`wait_terminal` 与 `wait_operation` 的循环体；两者都不读取 `stream` 状态；`read_frame` 只在主循环执行，等待期间不可达；没有取消参数或取消消息；测试只覆盖正常返回与超时，不覆盖客户端断开。
- 优化方向：等待期间监测连接 EOF／可读取消帧，或把等待改为可被连接生命周期取消的后台任务；明确客户端断开即结束等待。补断开、半关闭、连接重置与关闭中的用例。

### 重要｜`runtime_token_absent_from_materials` 锚实际只覆盖诊断材料

- 置信度：确认
- 位置：`slterm_app/src/diagnostics.rs` 的测试例 `runtime_token_absent_from_materials`；`slterm_app/src/local_control/boundary_tests.rs` 的 `integration_runtime_credential_boundaries`；`docs/devPlan/traceability.md` 的 Runtime 行“端点、设置、备份和诊断不含 Runtime token”；`docs/featureSpec/runtime.md` 的「凭据与发现」验收。
- 现象：该锚构造一个诊断报告，插入含假 token 的事件行，断言导出 JSON 被脱敏且类别在白名单内；它没有读取端点文件、设置文件或备份包，也没有验证“设置与备份中不存在 token”。“端点”由另一个边界用例覆盖，“设置”与“备份”没有任何自动化材料检查，追溯却把该锚名写成端点、设置、备份与诊断的总证据。
- 根因：测试名与注释按最终验收场景命名，实现只落地了诊断脱敏一段；端点、设置、备份的验证分散或缺失，追溯没有拆分锚。
- 影响：凭据红线的一条验收由不覆盖其声明场景的测试充当证据；设置或备份若未来引入 token 字段，现有测试不会红。安全结论高于实际证据。
- 证据：测试只构造 `DiagnosticReport`；`integration_runtime_credential_boundaries` 只检查端点文件与 discover 输出，不检查设置／备份；`backup_workspace_category_exclusions` 与 `backup_restore_does_not_restore_credentials` 不扫描 Runtime token；追溯行把两类证据混为一条。
- 优化方向：按材料类别分别建立可复现检查：真实临时 AppPaths 下的端点文件、设置写盘结果、备份包与诊断导出都扫描 Runtime／handover token；追溯按类别拆分锚名，不允许单个诊断用例承担这些材料。

### 一般｜注册表 kind 与 idempotent 语义和真实副作用不符

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/method.rs` 的 `RuntimeEventsSubscribe`、`RuntimeEventsUnsubscribe`、`InstanceHandover` 登记；`slterm_app/src/local_control/server.rs` 的订阅连接处理；`slterm_app/src/app_shell/app_root/events.rs` 的 `HandoverParams::NewTerminal`；`slterm_runtime_protocol/src/protocol.rs` 的 `MethodKind`、`MethodDescriptor`。
- 现象：`events.subscribe` 被登记为 `MethodKind::Query` 且 `idempotent: true`，但它会注册连接级订阅、消费 backlog、改变后续事件写线程，且同一连接不能重复订阅；`events.unsubscribe` 也有连接副作用。`instance.handover` 被登记为 `Mutation` 且 `idempotent: true`，但不使用 `MutationParams`／`OperationResource`，重复 `NewTerminal` 会创建新终端，没有幂等键。`runtime.describe` 把这些语义原样暴露给客户端。
- 根因：注册表的 kind／idempotent 按“是否写持久状态”粗分，没有区分连接级副作用与可安全重试；handover 的类型与 Runtime 写方法共用注册表但语义不同。
- 影响：客户端按 describe 断言“查询无副作用、可安全重试”时，订阅与 handover 的重试行为不符合；幂等能力被错误宣传。对自动化调用方，重复订阅或重复 handover 的语义不可预测。
- 证据：`MethodSpec` 条目；`MethodKind` 的注释“查询无副作用，写操作必须携带幂等语义”；`server.rs` 的 `EventsSubscribe` 分支在成功后独占连接；AppRoot 的 `NewTerminal` handover 每次创建新终端；`DescribeResponse` 输出 registered kind／idempotent；没有测试断言这些语义与真实副作用一致。
- 优化方向：为连接级方法增加独立的 kind 或能力位，明确是否可重试；handover 若不可幂等，应取消 `idempotent: true` 或在文档中限定其幂等范围。补重复订阅、重复 handover、同连接多方法的行为用例。

### 一般｜Runtime 运行状态与凭据失败没有设置界面可见面

- 置信度：确认
- 位置：`docs/featureSpec/runtime.md` 的「凭据与发现」；`slterm_app/src/app_shell/mod.rs` 的 `start_local_control_service`；`slterm_app/src/app_shell/app_root/settings_controls.rs`、`settings_overlay.rs`；`slterm_app/i18n/en-US.json`、`slterm_app/i18n/zh-CN.json`；`slterm_app/src/secrets.rs` 的 `CredentialStore::exists` 注记；`slterm_app/src/local_control/discovery.rs` 的 `read_discovery_for`。
- 现象：featureSpec 要求设置界面显示 Runtime 运行状态与非敏感端点信息，并让凭据读取、替换或删除失败可见。设置控件表与设置浮层没有 Runtime 分区，i18n 没有对应文案；控制面启动失败只在 `start_local_control_service` 记录日志并降级，没有用户可见提示；`CredentialStore::exists` 的注记写“设置页凭据状态展示（后）”，说明状态展示仍是计划项。CLI `runtime discover` 与诊断报告不是设置界面。
- 根因：设置中心切片只完成通用设置，Runtime 凭据与端点的状态面没有接入；服务启动失败被当作降级而不是可见错误。
- 影响：用户无法从应用内知道 Runtime 是否可用、端点是否已发布、凭据读取／轮换／删除为何失败；featureSpec 的凭据可见性验收没有产品入口。控制面失败时用户只看到“功能不可用”，没有重试或诊断线索。
- 证据：设置控件表与 i18n 无 Runtime 状态键；`start_local_control_service` 的 `log::warn!` 是唯一失败出口；`CredentialStore::exists` 注记；诊断报告只覆盖诊断导出而非设置界面。
- 优化方向：在设置中心增加只读 Runtime 状态／端点与凭据错误展示，或明确把该能力从 featureSpec 延期并登记豁免。凭据失败必须以用户可见状态出现，不能在后台日志中静默降级。

### 一般｜合同冲突：写请求参数散列没有客户端值，服务端规范投影是应用私有事实

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol.rs` 的 `MutationParams`、`params_sha256`；`slterm_app/src/local_control/mutations.rs` 的 `RuntimeMutation::params_value`；`slterm_app/src/local_control/service.rs` 的 `on_mutation`；`docs/devPlan/design/runtime.md` 的写操作契约；`slterm_runtime_protocol/CLAUDE.md` 的「两阶段参数解析与写信封」。
- 现象：设计写“每个写请求携带 client_id、mutation_id、规范参数散列和可选 expected_revision；服务器重新计算参数散列，失配拒绝”。线缆 `MutationParams` 只有 client_id、mutation_id、expected_revision 与 request，没有参数散列字段；服务端使用应用层的 `params_value` 重新构造规范参数对象，可选字段会被补成 `null`，视图身份会被重新编码。协议 crate 只导出 `params_sha256`，没有导出“从线缆请求生成服务端规范投影”的函数。
- 根因：幂等散列在应用层实现，协议层只保留通用 JSON 散列；设计中的“客户端与服务端共用同一推导”没有共享投影函数或线缆字段。
- 影响：同体重的重试仍可幂等，因为服务端对同一解析结果重新计算；但第三方客户端无法在发送前计算服务端散列，也无法用散列检测自己是否与服务器对同一参数理解一致。不同 Wire 编码（省略可选字段、显式 null、视图 JSON 的不同无损写法）会被服务端投影成同一或不同值，客户端无法预知。设计的“失配拒绝”没有可失配的客户端值。
- 证据：`MutationParams` 字段集；`params_sha256` 只接受 `serde_json::Value`；`service.rs` 调用 `request.mutation.params_value()`；`params_value` 对 `ProjectRemove`、`PanelClose`、`TerminalCreate` 等补全 `null`；协议测试的 `params_hash_is_stable_and_json_serialization_dependent` 只测试通用 JSON 键序，不测试方法投影。
- 优化方向：由协议 crate 提供方法级规范投影与散列，或把客户端计算的散列放进写信封，让服务端比较；随包文档应说明哪些字段参与散列。补同一意图不同 Wire 编码、可选字段省略／显式 null、跨客户端重试用例。

### 一般｜握手后的解析失败直接断开连接，不返回结构化 InvalidRequest

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `handle_connection` 主循环 `parse_client_request(&frame)` 失败分支；`slterm_runtime_protocol/src/protocol.rs` 的 `ProtocolError`、`DomainErrorCode::InvalidRequest`；测试例 `local_control_invalid_credential_closes_without_response`、`local_control_rejects_handover_before_hello`、`runtime_v1_rejected`、`runtime_panel_mutations_route_through_dispatcher_and_reject_bad_views`。
- 现象：认证完成后，任何信封错误、未知方法名、未知字段、非法参数形状、重复字段、batch 或超限帧都会让 `parse_client_request` 返回 `Err`，`handle_connection` 直接 `return`，连接静默关闭。作用域不匹配的断开是有意设计，但参数解析失败没有返回带 `invalid_request` 的错误响应。客户端只能看到 EOF。
- 根因：把“首包认证失败不返回协议预言机”的纪律扩展到了认证后的普通请求解析；协议已定义封闭 `InvalidRequest` 与 JSON-RPC 错误响应，却没有使用。
- 影响：客户端参数错误与传输中断无法区分，无法在同一连接上纠正并重试；长连接（例如持有快照或等待结果）会因一次客户端拼写错误丢失上下文。未知 v1 方法名也以静默断连表现，featureSpec 的“明确拒绝”只剩无响应。
- 证据：首包失败分支与主循环失败分支都直接返回；协议 crate 有 `ProtocolError` 与 `DomainErrorCode::InvalidRequest`，但 server 只对回调错误和身份解析错误构造错误响应；测试对首包只断言空响应，没有认证后非法参数的负例。
- 优化方向：首包认证失败维持不返回预言机；认证后若能解析 `id`，返回带封闭 `invalid_request` 的 JSON-RPC 错误并保留连接，不能解析 `id` 时才断开。补非法字段、非法方法、重复字段、batch 与超限帧的认证后用例。

### 一般｜分页与响应预算按原始字节估算，JSON 转义或无界列表可突破响应上限并静默断连

- 置信度：确认
- 位置：`slterm_app/src/local_control/reads.rs` 的 `PAGE_BUDGET_BYTES` 与注释；`slterm_app/src/local_control/server.rs` 的 `write_json`、`RESPONSE_BUDGET`、`respond_read`、`respond_git_diff`；`slterm_app/src/local_control/git.rs` 的 `git_status_response`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `runtime_snapshot_resources`；测试例 `local_control_response_budget_is_enforced`、`runtime_terminal_read_pages_snapshot_and_rejects_stale_cursor`、`runtime_git_diff_paginates_and_rejects_stale_cursor`。
- 现象：分页预算按正文原始字节计算，注释假设最坏 JSON 转义为两倍；`serde_json` 对控制字符使用 `\u00XX` 形式，最坏可达六倍。终端画面通常不含控制字符，但 Git diff 可以包含；终端读取和 Git diff 仍可能因单页编码后超过响应上限。`git.status` 返回完整条目集，`runtime.snapshot` 返回全部项目、组、终端与 Document，二者没有分页；`write_json` 一旦发现序列化后超过上限就返回错误，连接静默关闭，没有结构化错误。
- 根因：预算按“原始页字节”设定，没有在编码后或流式写入时校验；无界读取方法没有分页或超限错误映射；响应超限被当作传输错误。
- 影响：含控制字符的 diff 页或大仓库状态会被静默断连，客户端无法区分“内容太大”与“服务崩溃”；设计承诺的 1 MiB 响应上限被当作连接终止条件而不是可判定结果。大工作区的 snapshot／git.status 也可能触发。
- 证据：`PAGE_BUDGET_BYTES` 的注释声称两倍转义仍低于上限；`write_json` 在 `encoded.len() > RESPONSE_BUDGET` 时返回错误；`respond_read`／`respond_git_diff` 只在切页后调用 `write_json`；`local_control_response_budget_is_enforced` 只验证超大 hello 被断开；分页测试的正文由普通字符构成。
- 优化方向：按编码后大小或流式预算切页，超限返回封闭错误而不是断连；对无界列表提供分页或明确的容量上限。补控制字符、六倍转义、超大 git.status 与 snapshot 的用例。

### 一般｜分页游标不绑定资源身份，也接受非 UTF-8 字符边界的偏移

- 置信度：确认
- 位置：`slterm_app/src/local_control/reads.rs` 的 `ReadCursor`、`read_page`；`slterm_app/src/local_control/server.rs` 的 `respond_read`、`respond_git_diff`；`slterm_app/src/local_control/read_page_tests.rs` 的 `stale_generation_or_revision_invalidates_the_cursor`；测试例 `runtime_terminal_read_pages_snapshot_and_rejects_stale_cursor`。
- 现象：游标只编码 generation、revision 与 offset，没有 terminal_id、project_id 或 path。两个不同终端可以有相同的 generation／revision 数值，客户端把 A 的游标用于 B 时服务端不会拒绝；Git diff 的 revision 是内容散列，相同内容的不同路径也会接受同一游标。客户端构造一个落在多字节字符中间的 offset 时，`ReadCursor::parse` 只检查上界，`read_page` 返回以续字节开头的片段，随后 `String::from_utf8` 失败，终端／Git 读取映射为 `NotReady`，而不是契约要求的 `stale_cursor`。
- 根因：游标把“资源代次与输出 revision”当成全局唯一，没有把资源身份纳入不透明文本；偏移只做长度检查，没有做字符边界检查。
- 影响：客户端错误或跨资源复用游标不会被识别，可能得到另一资源的部分正文或错误码漂移；`NotReady` 表示服务暂时不可用，而实际是游标无效，客户端会错误重试。虽然不构成越权读取，但违反“客户端不得构造游标、非法游标一律 stale_cursor”的封闭语义。
- 证据：`ReadCursor::encode` 的 `generation:revision:offset` 格式；`read_page` 只在 `cursor.offset > bytes.len()` 时拒绝；没有 `bytes.is_char_boundary(offset)` 或资源身份字段；测试的非法样例只覆盖格式与越界（`v1:3:7:999`），不覆盖字符中间偏移或跨终端复用；server 对 `String::from_utf8` 失败使用 `read_encoding_error`（NotReady）。
- 优化方向：游标中包含资源身份或对资源身份签名，并把字符边界校验纳入 `ReadCursor`／`read_page`；非法游标统一映射 `stale_cursor`。补跨终端、跨路径、续字节偏移和多字节字符边界用例。

### 一般｜Handover 客户端不核对 hello 的协议版本与合同散列

- 置信度：确认
- 位置：`slterm_app/src/local_control/client.rs` 的 `handover`；`slterm_runtime_protocol/src/protocol.rs` 的 `HelloResponse`；`slterm_app/src/local_control/CLAUDE.md` 的「真实运行合同」；`docs/devPlan/design/runtime.md` 的握手与发现。
- 现象：单实例 handover 客户端在 hello 后只检查能力集合是否包含 `instance.handover`，不比较 `protocol_sha256`、`schema_sha256`、`protocol_version` 或 `store_epoch`／`stream_epoch`。本仓唯一的 Runtime 连接客户端因此没有执行“客户端必须核对真实运行与合同版本”的决策。
- 根因：能力检查被当作版本兼容检查；合同散列只被 server 计算和输出，没有消费点。
- 影响：第一实例与第二实例版本不一致时，客户端会把旧／新 handover 参数发给可能不兼容的服务；服务端可能因未知字段或语义变化拒绝，或接受后按另一版语义执行。客户端无法在发送移交意图前识别合同不同。若端点是同用户伪装的本地服务，客户端也不会用散列发现。
- 证据：`client.rs` 的 `capabilities.methods.contains(...)` 是唯一版本相关检查；`HelloResponse` 的兼容字段都未读取；`local_control/CLAUDE.md` 写“客户端必须核对真实运行与合同版本”；`client_tests` 的 capability 测试删除 `instance.handover`，没有散列不匹配用例。
- 优化方向：握手后比较协议版本与 `protocol_sha256`／`schema_sha256`，失败明确退出；补错误 schema／protocol 散列、错误能力集和版本偏差的客户端用例。若 handover 明确不受 Runtime 合同版本约束，应在文档中写清并移除“必须核对”的承诺。

### 一般｜`git.open_diff`／`git.open_deleted` 超时按失败处理，但打开可能仍会完成

- 置信度：确认
- 位置：`slterm_app/src/local_control/service.rs` 的 `on_mutation` `GitOpen` 分支；`slterm_app/src/local_control/git.rs` 的 `execute_git_open`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `handle_runtime_git_open`；`slterm_app/src/local_control/CLAUDE.md` 的打开回执映射；`docs/devPlan/design/runtime.md` 的 OutcomeUnknown 规则。
- 现象：`execute_git_open` 先向 AppRoot 投递 `RuntimeGitOpen` 事件，再在连接线程等待回执。如果回执超时，它返回 `NotReady`，service 把 Operation 记为 `Failed`。该事件已经进入有界壳事件通道，AppRoot 之后仍可能处理并真正打开 Diff／GitShow 面板；失败的 Operation 与已发生的副作用分叉。
- 根因：打开动作的完成回执超时被当成“未执行”，没有取消或证明“不会执行”；这与设计“无法证明副作用的执行体必须返回 OutcomeUnknown”不一致。
- 影响：客户端收到失败后可能用新 mutation_id 重试，造成重复打开、焦点跳转或同一 Document 的重复安装；也可能放弃，留下一个实际已打开的面板。`NotReady` 的语义被用来表示“结果不明”。
- 证据：`execute_git_open` 先 `try_send_runtime_git_open` 再 `recv_timeout`；超时分支返回 `NotReady`；service 对 `Ok(Err(error))` 一律 `DispatchOutcome::Failed`；AppRoot 的事件泵没有被取消，事件仍会被处理；没有“打开已投递、回执超时”的用例。
- 优化方向：回执超时或断连应映射为可查询的 `OutcomeUnknown`，或让 AppRoot 在回执失效后撤销未完成的打开；打开动作需要可取消身份。补事件已投递但回执超时、AppRoot 稍后处理、重试不重复打开的用例。

### 一般｜`agent.start`／`agent.resume`／`agent.fork` 在提示符注入前报告成功

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `execute_runtime_mutation` AgentStart／AgentResume／AgentFork 与 `start_agent_terminal`；`slterm_app/src/gpui_terminal/view/restore.rs` 的 `schedule_agent_command`、`inject_agent_resume`；`slterm_app/src/app_shell/app_root/CLAUDE.md` 的「Runtime Agent 资源」。
- 现象：Agent 方法先创建终端，再调用 `schedule_agent_command` 把 CLI 命令排入首个提示符或超时注入，然后立即返回 `Ok`。派发器把 Operation 写成 `Succeeded`。注入失败只写视图错误或静默消费，不会回投 Runtime；注入超时也由视图自行处理。客户端无法区分“终端已创建但 Agent 未启动”和“Agent 已启动”。
- 根因：Operation 完成语义绑定到终端创建和命令排队，而不是提示符注入的副作用；视图错误通道没有接到 Runtime Operation。
- 影响：自动化调用 `agent.start` 后可能立即发送 `agent.input`／`agent.read`，但目标 CLI 尚未运行，输入被 shell 消费或读取到错误画面；Operation 成功被误当成 Agent 会话可用。
- 证据：`start_agent_terminal` 在 `schedule_agent_command` 后返回 `Ok`；`inject_agent_resume` 失败只设置 `view.error`；`start_runtime_panel_open`／`panel.save` 会延迟完成，而 Agent 方法没有对应延迟；没有注入失败或超时的 Runtime 级测试。
- 优化方向：让 Agent 方法的 Operation 等到注入落定，或明确返回“终端已创建、Agent 注入进行中／未知”的可查询状态；注入失败必须可见。补提示符超时、终端提前退出、输入通道断开与注入失败的用例。

### 一般｜封闭文本字段在 DTO 与 schema 中仍是自由字符串

- 置信度：确认
- 位置：`slterm_runtime_protocol/src/protocol.rs` 的 `RuntimeEvent.kind`、`TerminalResource.run_intent`、`AgentResource.status`、`AgentResource.kind`、`ViewResource.kind`、`GitStatusEntry.index`／`worktree`；`assets/runtime-api-v2.schema.json` 对应定义。
- 现象：协议注释与决策记录把这些字段描述为封闭类型名或封闭状态（事件类型、`launch`／`exited`、`idle`／`working`／`blocked`／`done`／`unknown`、视图类型、porcelain 原始状态），DTO 全部用 `String`，schema 只有 `type: string`。`DomainErrorCode`、`ControlScope`、`MethodKind` 等真正的封闭域使用枚举，这几个高频资源字段却没有。
- 根因：为了保持轻量与向前兼容，资源字段使用了文本；但“封闭”只存在于注释，没有编码为枚举或 schema 约束。
- 影响：客户端无法机器校验事件类型与资源状态，拼写错误或未来新增值不会在 schema 层被发现；服务端也可能发布任意 kind。事件与快照的“封闭资源模型”缺少可验证边界。
- 证据：DTO 字段类型；schema 中这些定义没有 `enum`；事件测试使用 `panel.closed`／`terminal.output` 等合成 kind，说明 kind 没有注册表；`TerminalResource.run_intent` 只在 AppRoot 映射为 `launch`／`exited` 文本值，没有协议枚举。
- 优化方向：把已封闭的资源状态与事件类型提升为协议枚举，或在 schema 中至少给出封闭 `enum`；新增值必须先登记协议与测试。补未知值被 DTO／schema 拒绝的用例。

### 一般｜合同冲突：`runtime discover --json` 的输出是手工 JSON，不在随包 schema／DTO 中

- 置信度：确认
- 位置：`slterm_app/src/local_control/discovery.rs` 的 `discover_json_at`；`slterm_runtime_protocol/src/protocol.rs` 的 `DiscoveryRecord`；`slterm_app/src/cli.rs` 的 `run_runtime_discover`；`docs/featureSpec/runtime.md` 的「凭据与发现」；`docs/devPlan/design/runtime.md` 的「协议 crate 与随包契约」。
- 现象：`discover_json_at` 用 `serde_json::json!` 现场拼出 `protocol_version/endpoint/run_id/process_id/process_created_at_ms/runtime_token`，该形状既不是 `DiscoveryRecord`（多了 token，且端点由记录结构序列化），也没有独立 DTO 或 schema 定义。featureSpec 说“随包文档与 schema 是……发现格式的唯一真值”；设计说“CLI 使用相同 DTO，不自行拼协议信封”。
- 根因：token 输出被当作 CLI 层字符串合成，没有在协议 crate 定义“发现输出”合同；schema 只覆盖端点记录。
- 影响：客户端消费 `--json` 时只能按非合同形状解析，无法用 schema 验证；未来字段改名或 token 字段名变化没有机械约束，发现输出与端点记录可能漂移。token 输出本身仍是唯一入口，但没有字段级合同。
- 证据：`discover_json_at` 的 `json!` 字面量；`DiscoveryRecord` 没有 token 字段；`runtime-api-v2.schema.json` 没有对应 discover 输出定义；CLI 测试只解析固定字段，没有 schema 校验；协议 crate 的 discovery 测试只覆盖端点记录。
- 优化方向：在协议 crate 定义只用于 CLI 的 DiscoverOutput DTO，并纳入 schema 或明确登记为“非协议发现机制”的例外；补字段级与 token 字段存在性的测试。

### 一般｜合同冲突：`local_control` 与设计文档存在多处与当前代码冲突的决策记录

- 置信度：确认
- 位置：`slterm_app/src/local_control/CLAUDE.md` 的「首包认证与双作用域」「Operation 派发与查询」「能力边界」；`docs/devPlan/design/runtime.md` 的方法表；`slterm_app/src/app_shell/app_root/CLAUDE.md` 的「Runtime 快照端口」「Runtime 事件发布」；`slterm_runtime_protocol/CLAUDE.md` 的写方法与事件段。
- 现象：local_control 决策记录仍写“Runtime 作用域当前只开放握手”，但同文件能力边界与代码已实现全部已登记方法；写“不存在映射 `unknown_operation`”，但 server 在 `operation_get`／`operation_wait` 未命中时返回 `UnknownOperation`；写“OpenFile 仍返回 InvalidRequest”，但 `HandoverParams::OpenFile` 已实现并通过 `integration_single_instance_runtime_parity`、`runtime_parity` 覆盖；设计方法表写 `operation.get`／`operation.wait`，代码与随包文档使用 `runtime.operation_get`／`runtime.operation_wait`；AppRoot 文档写“事件环接入前 revision 由服务端按握手起点填写”，但事件环已经接入且该占位语义正是快照问题的根因。
- 根因：能力切片推进后没有同步决策记录，旧阶段的“当前只开放握手”“仍返回 InvalidRequest”“接入前”被保留为当前事实；方法改名没有回写设计表。
- 影响：后续维护者会按过时真值判断能力、错误码与数据结构；文档冲突会掩盖真实的快照 revision 缺陷与 handover OpenFile 行为，也让追溯讨论无法以一份事实为准。
- 证据：上述段落文本与代码符号／测试名；`HandoverParams::OpenFile` 分支与测试；`unknown_operation_error` 的 server 映射；`RuntimeMethod` 名称；`RuntimeMethod::Snapshot` 的 `hello.revision`。
- 优化方向：按代码与 featureSpec 重新裁决这些段落，把“当前只开放握手”“OpenFile 仍拒绝”“不存在 unknown_operation”“operation.get”等旧事实删除或标注为历史；快照 revision 的占位说明应与真实缺陷修复同步，而不是作为现状保留。

### 一般｜Runtime 追溯与验收锚在多个场景上只验证替身、弱断言或非声明对象

- 置信度：确认
- 位置：测试例 `runtime_v1_rejected`、`runtime_outcome_queryable`、`runtime_snapshot_returns_injected_resources_with_hello_generation`、`same_mutation_key_with_different_params_is_rejected_without_execution`、`runtime_events_visibility_and_generation`、`runtime_events_are_suppressed_*`、`runtime_git_diff_target_validates_repository_relative_path`、`runtime_git_status_routes_project_selection`；`docs/devPlan/traceability.md` 的 Runtime v2 行；`docs/devPlan/progress.md` 的 Runtime v2 行。
- 现象：v1 锚只发协议版本为 1 的 hello，不验证旧方法名、旧资源或旧发现格式；`runtime_outcome_queryable` 直接调用派发器，不经过 Wire、service、AppRoot，也没有终态推进失败或重启重查；快照测试把注入的 hello revision 断言为快照 revision，固化了错误；幂等测试中“键序不同但语义相同”的重试实际传入了不同内容，并用 `retry.is_err() || ...` 的恒真式弱断言；事件测试只验证项目激活与直接 helper 抑制，不经过真实 hub、订阅连接与滞后客户端；Git diff 路径校验不测试盘符绝对路径；Git status 路由测试注入完整响应，不测试真实 `git_status_response` 映射。追溯与进度仍把这些行记为已自动化／已验收。
- 根因：验收锚以“函数可调用”“替身返回成功”或“代码路径存在”为判据，没有按前置条件、动作、可观察结果三者一致构造；部分锚名比实现场景更宽。
- 影响：测试通过不能证明所声明的跨层行为；错误成功、revision 错误、真实 Git 映射错误和 v1 兼容拒绝都可能未被发现。计划退出证据中的“真实 GUI／CLI 全方法与事件断线矩阵”仍无可复现目标。
- 证据：上述用例体与 `traceability.md`／`progress.md` 的状态；`test-quality.md` 与 `concurrency-resources.md` 对 schema 门禁、双进程 conformance、快照 revision 与 Operation 停滞另有同根记录，本报告保留 Runtime 视角的对应证据。
- 优化方向：按验收场景重建锚：真实 Wire／service／AppRoot／双进程链路，或明确降级为单元层并拆分追溯锚；测试必须包含失败路径、边界、断线与重启，不以替身成功替代声明行为。

### 提示｜`service.publish_discovery` 双失败时丢弃事件环回收错误

- 置信度：确认
- 位置：`slterm_app/src/local_control/service.rs` 的 `publish_discovery`；`slterm_app/src/local_control/CLAUDE.md` 的「首实例服务组合」；测试例 `service_publish_failure_reports_real_server_cleanup_failure`。
- 现象：发布失败后分别尝试 `server.shutdown()` 与 `event_hub.shutdown()`。若两者都失败，返回的错误文本只包含 server 错误；只有 server 清理成功时才返回 hub 错误。决策记录却要求“server 与 hub 回收失败都必须一并可观察”。
- 根因：错误聚合分支只选择其中一个，没有拼接两处回收错误。
- 影响：关闭期的事件环回收失败被隐藏，诊断只能看到监听回收失败；运维无法同时处理相关资源。
- 证据：条件分支 `if let Err(hub_error) ... && server_cleanup.is_none()`；测试只注入 server 失败；`service_shutdown_attempts_both_failures_and_returns_first_error` 只证明首个错误策略，不覆盖发布失败路径。
- 优化方向：发布失败路径同时保留发布根因、server 回收错误与 hub 回收错误；或明确“只报首个”并修改决策记录。补双失败用例。

### 提示｜错误数值码重叠，Operation 错误形状不统一

- 置信度：确认
- 位置：`slterm_app/src/local_control/operations.rs` 的 `control_error`、`operation_error`、`error_value`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `decision_required_error`；`slterm_runtime_protocol/src/protocol.rs` 的 `DomainErrorCode`、`JsonRpcErrorData`；测试例 `runtime_operation_get_returns_persisted_result_or_structured_unknown`。
- 现象：`MutationConflict` 与 `DecisionRequired` 都使用数值码 `-32006`；`InvalidRequest`、`UnknownOperation`、`UnknownAgent`、`StaleCursor` 共用 `-32000`（靠 `data.code` 区分）。`OutcomeUnknown` 作为 Operation 终态时错误值是手工 `{"code":"outcome_unknown"}`，而 `Failed` 时是完整 `JsonRpcError` 序列化；同一 `OperationResource.error` 字段存在两种形状。
- 根因：数值码没有统一分配表，Operation 错误值由不同路径手写。
- 影响：只用数值码的客户端无法区分冲突与决定错误；解析 Operation 错误的客户端必须同时处理两种 JSON 形状。虽然封闭领域码可用，但增加了跨客户端实现分歧。
- 证据：`control_error(DomainErrorCode::MutationConflict, -32006, ...)` 与 `decision_required_error` 的 `-32006`；`unknown_operation_error`、`invalid_request_error`、`stale_cursor_error`、`unknown_agent_error` 的 `-32000`；`service.rs` 的 `OutcomeUnknown` 错误字面量；`operation_dispatch_tests` 与 `runtime_operation_get...` 使用不同形状。
- 优化方向：为领域码分配不重叠的数值码，或明确数值码只作传输分类并在文档中列出；`OperationResource.error` 统一为同一错误 DTO。补解析与兼容用例。

### 提示｜等待实现重复、参数版本常量和请求预算存在第二真值

- 置信度：确认
- 位置：`slterm_app/src/local_control/operations.rs` 的 `OperationDispatcher::wait`、`PARAMS_DTO_VERSION`；`slterm_app/src/local_control/server.rs` 的 `wait_operation`、`REQUEST_BUDGET`；`slterm_runtime_protocol/src/protocol.rs` 的 `REQUEST_BUDGET`；测试例 `runtime_operation_wait_polls_until_terminal_then_returns_result`。
- 现象：派发器实现了 `wait`，但生产 server 另写 `wait_operation`，两者都轮询 100 毫秒、都映射 NotReady／Closing，逻辑重复；`PARAMS_DTO_VERSION` 是手工常量，参数 DTO 变化是否递增没有机械检查；请求预算常量在协议 crate 与 server 各有一份，方法字节上限依赖两处同步。
- 根因：功能切片各自实现等待与预算，没有收敛到单一端口；DTO 版本没有与 schema／序列化结构绑定。
- 影响：两条等待路径的错误映射或轮询语义可能漂移；新增参数字段忘记递增版本会让旧幂等键与新 DTO 混淆；预算变化可能只改一处。
- 证据：`OperationDispatcher::wait` 在生产无调用；`wait_operation` 是另一套实现；`PARAMS_DTO_VERSION` 注释要求“字段变化才递增”；协议 crate 与 server 各有一份 `REQUEST_BUDGET`。
- 优化方向：等待只保留一个生产端口，测试经同一端口；参数版本由生成或 schema 指纹派生；请求预算从协议 crate 导出，server 不再复制。补版本变更与预算边界的用例。

### 提示｜取消订阅确认后仍可能收到事件

- 置信度：确认
- 位置：`slterm_app/src/local_control/server.rs` 的 `stream_subscription`；`slterm_app/src/local_control/events.rs` 的 `deliver_event`；测试例 `runtime_subscription_streams_events_and_accepts_unsubscribe`。
- 现象：主线程在写锁内写入 unsubscribe 成功响应后立即返回；事件线程仍可在返回后、`stream.shutdown` 前拿到写锁并写出一条 `events.event`。共享写锁只避免 JSON 行交错，没有保证“确认之后不再有事件”。
- 根因：取消确认没有先终止事件源或设置停止屏障；发送端未在确认前移除。
- 影响：客户端可能把取消确认当作事件流已经终结，却在同一连接上再收到一条事件；对严格的事件流状态机是竞态。测试只发送取消并读取确认，不覆盖并发事件。
- 证据：`write_json` 取消响应后返回，随后才 `stream.shutdown` 与 `event_thread.join`；`writer` Mutex 只保护单行写入；没有按订阅身份停写事件的标志。
- 优化方向：先停止该订阅的事件写者并排空，再写取消确认；或让确认与事件共享同一取消代次。补确认与事件并发的用例。

### 提示｜单字符超过预算时页面会略超预算

- 置信度：确认
- 位置：`slterm_app/src/local_control/reads.rs` 的 `read_page`；`slterm_app/src/local_control/read_page_tests.rs` 的 `reads_prefer_line_boundaries_without_starving`。
- 现象：预算小于单个 UTF-8 字符宽度时，代码把 `end` 推进到该字符末尾以保证前进；页面字节长度因此可超过调用方给定的 budget，最多超出三个字节。测试只断言页面非空且为有效 UTF-8，没有断言预算上界。
- 根因：预算被当作切分目标而不是硬上限；“保证前进”优先于“不超预算”。
- 影响：调用方以 `PAGE_BUDGET_BYTES` 作为响应编码前的硬预算时，单字符边界处会得到比预算多几个字节的页；在响应上限附近可能增加溢出概率。影响有限，但契约应明确。
- 证据：`if end == offset { end = limit; while ... }` 的前进分支；单字符样例测试只检查有效性与非空；`Page` 没有声明允许的溢出。
- 优化方向：明确页面预算是上限还是目标；若为上界，单字符超限时应返回明确错误或允许调用方选择；测试断言最坏长度。

### 提示｜Runtime token 没有删除或替换命令

- 置信度：确认
- 位置：`slterm_app/src/cli.rs` 的 `RuntimeCommand`；`slterm_app/src/local_control/credentials.rs` 的 `rotate_runtime_at`；`slterm_app/src/local_control/CLAUDE.md` 的「凭据写入所有权」；`docs/devPlan/design/runtime.md` 的「传输、认证与发现」。
- 现象：CLI 只提供 `runtime discover` 与 `runtime rotate`；`CredentialStore::delete` 没有 Runtime 用户入口。设计写“删除或轮换 Runtime token 只断开 Runtime 会话，不影响 handover”，featureSpec 也提到“凭据读取、替换或删除失败必须可见”；实现只有轮换，且轮换要求应用先退出。
- 根因：凭据维护只完成轮换切片，删除／替换入口与可见失败没有接线。
- 影响：用户无法从命令行显式移除 Runtime 凭据；若凭据疑泄露，只能轮换，不能删除或验证删除失败可见。设计承诺的删除语义没有可观察实现。
- 证据：`RuntimeCommand` 枚举；`run_runtime_rotate` 的实例锁与 `rotate_runtime_at`；`CredentialStore::delete` 未被 Runtime 命令使用；`CredentialStore::exists` 注记“设置页凭据状态展示（后）”说明状态展示仍未接线。
- 优化方向：补齐删除／替换的维护子命令或设置入口，并让失败可见；若产品不提供删除，应在设计／featureSpec 中明确撤销该承诺。补删除失败、轮换后旧 token 失效和 handover 不受影响的用例。

## 跨域观察

- `slterm_app/src/git/mod.rs` 的 `head_file`、`verify_head`、`file_hunks` 使用裸 `command`，没有统一设置 `GIT_TERMINAL_PROMPT=0`、禁用 lazy fetch、超时与输出上限；该路径直接服务 Runtime `git.diff`，属于 Git 执行域而非 `local_control`，但会使 Runtime 读取在 partial clone 或异常仓库上阻塞／联网／无界占用内存。
- `slterm_app/src/app_shell/app_root` 的关闭协调器本身还有多处异步完成与失败状态语义问题，本报告只记录 Runtime 回执与 Operation 的接缝；关闭协调器的内部阶段、代次收集和 UI 失败重试属于 AppRoot 生命周期域。
- `slterm_app/src/local_control` 的端点发现记录只校验进程身份字段非零，客户端没有用 `RunMarker` 核对存活进程；该风险在安全跨域报告中另有记录，本报告的 `git.diff` 与 handover 问题不依赖它。

## 存疑未证

- `git.diff` 盘符绝对路径的实际可利用性依赖目标位于某个 Git 仓库内，并且 `head_file` 的仓库解析在该环境下成功；静态代码路径已闭合，但未在真实 Windows 仓库上复现文件正文返回。需要运行时或可注入的文件系统证据。
- 已退出但无 worker 的终端能否按 `generation=0` 读取保留画面：协议 DTO 要求 `generation > 0`，而快照把无 worker 记录的代次写成零，`runtime_terminal_view` 又要求 Registry target；需要真实终端生命周期证据确认“只读允许已退出终端”是否只适用于仍有 target 的情况。
- 事件滞后连接在真实慢客户端下是否会因 TCP 层错误最终关闭：代码中主线程不观察事件线程，但客户端若继续发送或读端出现错误可能自行退出；需要真实 socket 半关闭／背压实验。
- `app.close` 的“接受”与 featureSpec 的“保存失败返回失败”到底哪一方是产品真值：当前只能确认文档冲突，需产品／计划所有者裁决。
- `Runtime` 设置状态是否被明确推迟到后续切片：`CredentialStore::exists` 的注释提示“后接设置页”，但计划与进度没有登记延期，豁免表也没有；需要确认是否为已知范围裁剪。
- `discover --json` 的输出形状是否被有意排除在 schema 之外：文档同时写“schema 是发现格式唯一真值”和“CLI 层合成输出”，需要合同所有者确认。
- `git.open_*` 回执超时后 AppRoot 是否一定仍会处理已投递事件：事件泵可能因关闭或队列丢弃；代码路径显示“可能”，需要在真实事件泵下注入超时。
- `Operation` 最终终态推进失败在正常写线程负载下是否可达：静态看 `transition_operation` 有超时／队列满／停止分支，但没有故障注入测试；需要 writer 故障注入证据。
