# 终端核心（slterm_terminal）审查报告

> 审查 slterm_terminal 及 GPUI 终端接线与真实 PTY 测试，基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支），只读代码与文档，未运行 cargo、构建、测试或网络。

## 覆盖与局限

- 已读：`slterm_terminal/CLAUDE.md`；`slterm_terminal/src` 的 event_loop、tty/windows、term、grid、render、osc_cwd、selection、search 等模块；`slterm_terminal/tests/pty_integration_tests.rs`；`vendor/conpty/README.md`；`docs/devPlan/design/terminal.md`；`docs/featureSpec/panels/terminal.md`；`.claude/docs/testing/strategy.md` 与 `exemptions.md`；并核对 app 侧 `gpui_terminal`、`app_root` 的终端启动、恢复、重连、Runtime 与设置接线。
- 未覆盖：未运行任何测试或真实 PTY；未在 Win10 实机、真实 GPU、桌面会话、IME、系统剪贴板环境验证；并发时序、真实 ConPTY 握手与退出事件顺序只能做代码推演。

## 问题

### 阻断｜存活上限预检发生在真实 PTY 创建之后，拒绝路径在前台 join worker

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/startup.rs` 的 `TerminalView::launch`、`prepare`、`bind_spawned`、`restore_launch`；`slterm_app/src/gpui_terminal/registry.rs` 的 `TerminalRegistry::launch`、`ensure_live_capacity`；`slterm_terminal/src/event_loop/mod.rs` 的 `EventLoopHandle::drop`；测试 `terminal_live_pty_limit`。
- 现象：`launch`/`restore_launch` 先在 `prepare` 内调用 `session::local_options` 与 `session::spawn_local`，真实 `tty::new` 已创建 ConPTY 和子进程；随后 `bind_spawned` 才进入 `TerminalRegistry::launch`，由 `ensure_live_capacity` 检查上限。达到上限或目标重复时，Registry 在调用闭包前返回，持有 `TerminalSession` 的闭包被丢弃，`EventLoopHandle::drop` 在 GPUI 前台发送 `Shutdown` 并 `join`。
- 根因：容量闸门放在 Registry 的“工厂”内，但真正执行进程创建的工厂在 Registry 之外；`restore_launch` 也不像 `launch` 那样预检已有目标。前台启动路径没有把容量判定前置到 spawn 之前。
- 影响：已达到 64 个存活 PTY 时仍会创建并立即回收一个真实进程；回收的 `join` 阻塞 GPUI 线程，窗口冻结时间取决于 ConPTY 收尾和 shell 启动。违反项目 CLAUDE 中“额度不足不得执行工厂”的原子预检约束，以及“GPUI 前台不得 join”的关闭纪律。`terminal_live_pty_limit` 使用内存会话工厂并只断言闭包未执行，覆盖不到真实启动顺序。
- 证据：`TerminalView::launch` 的调用顺序是 `prepare`（含 `session::spawn_local`）→ `bind_spawned` → `registry.launch`；`TerminalRegistry::launch` 在 `ensure_live_capacity` 失败时先于 `factory()` 返回；`TerminalSession` 的 `_event_loop` 字段类型为 `EventLoopHandle`，其 `Drop` 实现执行 `tx.send(Msg::Shutdown)` 后 `join`。`restore_launch` 直接 `prepare` 后 `bind_spawned`，没有先检查 `registry.target`。
- 优化方向：把容量与目标预检移到任何 PTY 创建之前，或让 Registry 的工厂成为唯一的进程创建入口；失败清理必须移交后台关闭，不得在前台 join。真实启动路径需要覆盖上限拒绝与重复目标，而不只是内存工厂。

### 阻断｜空 `Msg::Input` 永久卡住写队列

- 置信度：确认
- 位置：`slterm_terminal/src/event_loop/mod.rs` 的 `drain_recv_channel`、`pty_write`、`State::ensure_next`/`take_current`/`set_current`/`needs_write`、`Writing::finished`/`remaining_bytes`；`slterm_app/src/gpui_terminal/view.rs` 的 `runtime_input`；`slterm_app/src/app_shell/app_root/runtime_snapshot.rs` 的 `TerminalInput` 与 `AgentInput`；`slterm_app/src/gpui_terminal/registry.rs` 的 `TerminalRegistry::send`。
- 现象：空字节输入被 `drain_recv_channel` 放入 `write_list`，生成一个长度为空的 `Writing`。`remaining_bytes()` 为空，`pty_write` 调用底层 `write` 得到 `Ok(0)`（或可重试错误），走 `set_current(Some(current))` 后 `break`。下一次 `ensure_next` 看到 `writing` 仍为 `Some`，不会前进；`take_current` 取出的仍是这个已“完成”的空项，再次写入空切片并保留，形成永久停在同一项的状态。后续所有输入只排队不写出，`needs_write` 持续为真，事件循环反复尝试空写。
- 根因：`Writing::finished` 只在 `advance(n)` 之后检查；零长度项没有在入队或取出时被识别为已完成；`EventLoopSender::try_send` 与 Registry 的 `send` 不拒绝空 `Msg::Input`。`Notifier::notify` 虽有空的提前返回，但 Runtime 路径直接调用 `runtime_input`，绕过了这一守卫。
- 影响：一次空的 Runtime `terminal.input`/`agent.input`（JSON 数据为空串即可）会使该终端永久无法再接收键盘输入，并造成事件循环忙转；用户只会看到终端不再回显。进程仍存活，PTY 代次与容量继续占用。
- 证据：`runtime_snapshot.rs` 对 `TerminalInput` 和 `AgentInput` 都把 `data.into_bytes()` 原样交给 `TerminalView::runtime_input`；`runtime_input` 直接 `send_worker_command(Msg::Input(...))`；Registry 的 `send` 只校验目标与 `process_live`，不校验空载荷。`TerminalView::write_input` 对空字节有保护，但 Runtime 不经过它。
- 优化方向：在命令端点统一拒绝空输入，或在写状态机中把零长度项直接视为完成并跳过，不保留在队首。需要覆盖“空输入后紧接普通输入仍能到达 PTY”的用例。

### 重要｜重连期间旧 worker 仍可向共享 Term 注入退出事件或显示命令

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/registry.rs` 的 `TerminalRegistry::reconnect`；`slterm_app/src/gpui_terminal/session.rs` 的 `spawn_on_term`；`slterm_terminal/src/event_loop/mod.rs` 的子进程退出分支；`slterm_terminal/src/term/reconnect.rs` 的 `Term::begin_reconnect`/`replace_event_proxy`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `reconnect_terminal`。
- 现象：重连允许的条件是 Registry 已把旧进程标记为不存活，而该标记由 View 处理旧 worker 发出的 `ChildExit` 触发。旧 worker 在发出 `ChildExit` 之后才调用 `self.terminal.lock().exit()`；如果用户或 Runtime 在两步之间发起重连，新 worker 会先对同一个共享 `Term` 调用 `replace_event_proxy`，随后旧 worker 的 `Term::exit()` 通过已被替换的代理把 `Event::Exit` 送进新会话的事件通道，新终端会被立即标记为已退出。旧 worker 队列里尚未处理的 `Resize`、`Scroll`、`Select` 等显示命令也可能在 `begin_reconnect` 之后修改新会话共享的网格。
- 根因：新旧代次共享同一个 `Term`，但旧 worker 没有在新 worker 接管前完成回收；`Term` 的事件代理替换早于旧 worker 线程终止，且显示命令与 `exit` 没有代次门控。Registry 的“先证明代次可推进，再启动新 worker”没有把旧 worker 停止纳入同一原子段。
- 影响：重连后新 shell 可能立即显示退出状态，或网格被旧尺寸/旧选区命令污染；与“旧代次事件与命令目标全部失效”的约束冲突。现有 `reconnect_invalidates_the_previous_target_and_worker` 使用内存会话并在新 worker 建立后才关闭旧 worker，不覆盖真实事件循环的这一步时序。
- 证据：`TerminalRegistry::reconnect` 在检查 `process_live` 后调用工厂，工厂内部用 `entry.session.shared_term()` 创建新会话；`spawn_on_term` 在重连时执行 `terminal.replace_event_proxy(proxy)` 与 `terminal.begin_reconnect()`，之后才 spawn EventLoop；旧 `TerminalSession` 由调用方在 `install_reconnect` 之后才交给 `reclaim_terminal_worker` 后台回收。旧 worker 的退出分支先发送 `ChildExit`，再执行 `terminal.lock().exit()`。
- 优化方向：在新 worker 接管共享网格前完成旧 worker 的停止或隔离；至少让 `Term::exit` 与显示命令绑定代次，旧代次的代理不得写入新通道。需要可确定性复现“旧 worker 收尾与新 worker 建立交错”的真实 EventLoop 用例。

### 重要｜帧唤醒标记与最新帧存储存在丢失竞态

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/event_mailbox.rs` 的 `EventSender::send`、`EventReceiver::latest_event`、`EventReceiver::acknowledge`；`slterm_app/src/gpui_terminal/session_pump.rs` 的批量消费；测试 `frame_events_coalesce_to_latest_without_losing_semantic_events`。
- 现象：接收端 `latest_event` 先从 `FrameState` 取出最新帧，随后 `acknowledge` 才把 `wake_pending` 清零。若发送线程恰好在这两步之间写入新帧：`send` 先存入新帧，再对 `wake_pending` 做 `swap(true)`，此时旧值仍为真，发送线程直接返回，不向通道投递新的 `Wakeup`；接收端随后清零标记。新帧留在 `FrameState`，通道中没有唤醒事件，也没有后续发送者，最终画面停留在旧帧。
- 根因：帧存储与唤醒标记不是同一个原子状态；“先取帧、后清标记”的窗口允许新帧被“已存在的唤醒”吞掉。唤醒标记只保证通道中至多一个 `Wakeup`，没有保证 `FrameState` 更新与标记清理的配对。
- 影响：静默输出结束时，最后一帧可能永远不显示，直到下一次 resize、滚动或输出才刷新；隐藏恢复后的“最新画面”也可能过期。现有测试是单线程顺序发送，不覆盖该交错。
- 证据：`send` 对 `Event::Frame` 先写入 `self.frame.0` 再构造 `Wakeup`，在 `swap(true)` 返回真时直接返回；`try_recv`/`poll_next` 在返回事件前调用 `latest_event` 取帧，之后才调用 `acknowledge` 清零。没有在清零后重新检查 `FrameState`。
- 优化方向：让唤醒标记、帧替换与消费确认使用同一个锁或同一状态机；接收端在清零后检查是否出现新帧并补发唤醒。需要并发时序测试。

### 重要｜子进程退出时未排空已读输出，尾部画面可能丢失

- 置信度：确认
- 位置：`slterm_terminal/src/event_loop/mod.rs` 的 `PTY_CHILD_EVENT_TOKEN` 分支；`slterm_app/src/gpui_terminal/session.rs` 的 `local_options`/`spawn_on_term`；`slterm_terminal/src/tty/mod.rs` 的 `Options::default`；`slterm_terminal/src/tty/windows/blocking.rs` 的读取线程与 `OutputQueue`。
- 现象：子进程退出事件到达时，核心只在 `drain_on_exit` 为真时执行一次 `pty_read`，随后立即发送 `ChildExit`、调用 `Term::exit()`、发布最后一帧，并用 `continue 'event_loop` 跳过同批剩余事件并停止 PTY 读取。生产路径的 `Options` 使用默认值，`drain_on_exit` 为假；读取线程已经读入 `OutputQueue` 但解析 worker 尚未消费的块，以及管道中尚未读出的尾部字节，都不会再进入网格。
- 根因：退出处理把“停止 live 读取”提前到“排空已读输出”之前；默认配置关闭了 drain，且同一事件批次中子进程事件先于读写事件时没有补读队列。
- 影响：进程退出前刚打印的最后输出（包括错误、退出提示）可能不出现在保留画面中，与“进程退出后保留当前内存画面”的验收语义冲突。设计要求的真实 ConPTY 退出保留用例也没有覆盖该时序。
- 证据：退出分支只有在 `self.drain_on_exit` 为真时才调用 `self.pty_read`；`local_options` 构造 `tty::Options` 时使用 `..Default::default()`，而 `Options::default` 的 `drain_on_exit` 为 `false`；事件循环随后 `pty_active = false`，不再处理 `PTY_READ_WRITE_TOKEN`。读取线程的 `OutputQueue` 中可能仍有已读块。
- 优化方向：退出边界应先把队列中已公布的水位内容和已读块处理完，再发布最终帧；或者生产路径显式启用 drain 并证明事件顺序。需要真实 PTY 用例在退出前输出唯一标记，验证最终帧包含它。

### 重要｜读取线程 EOF 或读取错误不会转成退出/故障事件

- 置信度：疑似
- 位置：`slterm_terminal/src/tty/windows/blocking.rs` 的 `UnblockedReader::new` 读取线程与 `UnblockedReader::read`；`slterm_terminal/src/event_loop/mod.rs` 的 `pty_read_until`；`slterm_terminal/src/tty/windows/mod.rs` 的 `Pty::next_child_event`。
- 现象：读取线程在 `Ok(0)`、`BrokenPipe` 或其他 I/O 错误时只记录日志并 `queue.close()`，没有把 EOF/错误状态传给事件循环。`UnblockedReader::read` 对队列为空始终返回 `Ok(0)`，注释明确“只表示队列暂空，不代表 EOF”；事件循环的 `pty_read_until` 把 `Ok(0)` 当作本轮无数据并跳出，不发送 `PtyFailure`、不调用 `exit`，也不标记 Registry 的进程状态。只有显式 teardown 的 `drain_detached` 会观察 `is_closed`。
- 根因：`EventedPty` 读接口没有关闭/错误状态，EOF 被折叠为空读；错误只记录在读取线程。
- 影响：如果读取端先于子进程退出事件失效，终端可能长期保持“运行中”，事件泵继续占用容量，用户看不到失败提示，也不会自动进入退出重试状态。该路径与子进程正常退出不同，现有集成测试没有构造“管道关闭但子进程事件未到”的场景。
- 证据：读取线程三条结束路径都只 `break` 后 `producer_queue.close()`；`Read` 实现调用 `try_read`，队列空即返回 0；事件循环没有读取 `is_closed` 或 `PtyFailure` 的映射。需要真实或可注入的传输失败才能证明触发频率。
- 优化方向：让读接口显式返回 EOF/错误或提供关闭查询，由事件循环转换为既有的故障/退出事件，并让 Registry 释放存活额度。需要读取管道关闭而子进程事件缺失的测试。

### 重要｜OSC 原始缓冲无界，嗅探上限在分发后才生效

- 置信度：确认
- 位置：`slterm_terminal/Cargo.toml` 的 `vte` 依赖与 `std` feature；`slterm_terminal/src/event_loop/mod.rs` 的 `StreamProcessor::feed`；`slterm_terminal/src/osc_cwd.rs` 的 `CwdSniffer` 上限；`slterm_terminal/src/term/mod.rs` 的 `clipboard_store`；`slterm_app/src/gpui_terminal/event_mailbox.rs` 的事件通道容量。
- 现象：启用的 vte 0.15 在 `std` feature 下把 OSC 原始数据存入无界 `Vec<u8>`，直到遇到 BEL/ST 或取消序列才分发。`CwdSniffer` 的 4 KiB 载荷、12 MiB 图像等上限只在分发后检查；OSC 52 的 1 MiB 上限也在 Base64 解码为 `String` 之后才检查。子进程只要持续发送没有终止符的 OSC，内存就会无界增长；即使带终止符，超大 OSC 52 也会先整段解码再被拒绝。
- 根因：解析器层的原始缓冲没有字节上限，业务层的上限无法阻止解析器先缓存全部内容；事件通道可容纳多个大载荷事件。
- 影响：不可信终端输出可以触发内存耗尽或长时间分配，拖慢或杀死整个应用；与“输出不无界占用内存”的背压约束不一致。该问题不依赖 Win10 或真实 ConPTY，所有平台可达。
- 证据：vte 0.15 的 `Parser` 在 `feature = "std"` 下定义 `osc_raw: Vec<u8>`，`action_osc_put` 在 std 下没有容量检查；`Term::clipboard_store` 先 `Base64.decode` 再判断文本长度；`EventSender` 的通道容量为 256，单个 `ClipboardStore` 可携带大字符串。
- 优化方向：在解析器之前或包装层对 OSC 长度设置硬上限并丢弃超限序列，或改用有界 OSC 缓冲；对 OSC 52 在解码前按 Base64 长度拒绝。需要无终止符和超大单条 OSC 的自动化用例。

### 重要｜运行时设置未作用到终端核心

- 置信度：确认
- 位置：`slterm_app/src/app_shell/config.rs` 的 `Settings::term_config` 与字号默认；`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::apply_settings`；`slterm_app/src/gpui_terminal/session.rs` 的 `local_options`；`slterm_app/src/app_shell/mod.rs` 的 `frozen_startup_paths`；`slterm_settings/src/lib.rs` 的 `scrollback_lines`、`shell`、`conpty_input_modes`、`refresh_environment`、`terminal_proxy`、`startup_directory`。
- 现象：`Settings::term_config` 固定 `scrolling_history` 为 10000、`conpty_resize`/`kitty_keyboard` 为真，未读取 `RuntimeSettings.scrollback_lines`；字号默认写死为 15px，而 featureSpec 规定默认 14、范围 8–32；`TerminalView::apply_settings` 在生产路径没有调用者，字体、字号、连字、选中复制、调色板不会热应用到既有终端；`local_options` 总是执行环境刷新、`conpty_sideload` 恒 true、不注入 `conpty_input_modes`；`frozen_startup_paths` 不读取 `shell` 或 `startup_directory`，设置中的 shell 选择不会影响新建或恢复的终端。
- 根因：设置键与终端核心的 `Config`/`Options` 之间缺少单一映射与热应用接线；部分设置只进入 `RuntimeSettings`/设置页，未进入启动与视图更新路径。
- 影响：用户可配置的回滚行数、字号、字体、主题、选中复制、shell 选择、ConPTY 输入模式、环境刷新与代理等设置没有实际效果；违反 featureSpec 中“回滚可配置 1,000–100,000”“字号默认 14、变化只调整网格和 PTY 尺寸，不重启进程”的行为真值，也让设置页呈现与运行行为不一致。
- 证据：`term_config` 直接字面量；`apply_settings` 带“设置协调器接入后消费”的未接线注释且无生产调用；`local_options` 使用 `..Default::default()` 后只改 `conpty_sideload`；`frozen_startup_paths` 调用 `resolve_shell(None)`；`RuntimeSettings.shell`/`scrollback_lines` 仅在设置存储与控件层出现。现有测试只钉死默认值。
- 优化方向：建立设置到 `term::Config`/`tty::Options` 的唯一转换点，并热应用到所有既有视图；shell、输入模式、环境刷新与代理需要进入启动链。需要从设置值到真实终端行为的端到端用例。

### 重要｜搜索在每帧自动重查并重置选中位置

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `process_event` 的 `TermEvent::Frame` 分支；`slterm_app/src/gpui_terminal/view/search.rs` 的 `refresh_terminal_search`、`move_terminal_search`；测试 `terminal_search_is_read_only`、`integration_terminal_search`。
- 现象：只要终端搜索处于打开状态，每收到一帧就调用 `refresh_terminal_search`：先作废旧结果并发送新的 `SearchLiteral`，worker 回包后把 `selected` 设为 0 并滚动到首个匹配。搜索打开期间持续输出会形成“每帧一次全回滚搜索”的循环，worker 命令队列被搜索命令占用，用户正在查看的当前位置被重置。每次新查询还会通过有界的 worker 命令通道，搜索风暴可能与用户输入争抢队列容量。
- 根因：把“新输出使旧位置失效”实现成了“每帧立即重查并重新选中”，没有去抖、查询代次合并或保留用户导航位置；失效与重查没有分离。
- 影响：高输出终端打开搜索会明显拖慢解析与输入，搜索结果计数和当前位置不断跳变；关闭搜索前用户无法稳定停留在某个匹配。现有搜索用例在测试中手动调用 `refresh_terminal_search`，`feed_test_bytes` 又直接作废搜索，未走生产的 `TermEvent::Frame` 自动重查路径。
- 证据：`process_event` 的 `Frame` 分支在 `adopt_frame` 后无条件 `if self.terminal_search.is_some() { self.refresh_terminal_search(cx); }`；`refresh_terminal_search` 每次 `search.invalidate()`、清空任务、发送新的 `SearchLiteral`，回包设置 `selected = Some(0)` 并滚动；`terminal_search_is_read_only` 在喂入新输出后调用 `view.refresh_terminal_search(cx)` 并手动 pump，而非等待生产路径。
- 优化方向：新帧只作废旧结果并保留查询，由用户导航或显式触发再重查，或对重查做去抖与代次合并；重查回包不得无条件重置当前位置。需要真实帧事件驱动的搜索测试。

### 重要｜退出后键盘复制、搜索与滚动被整体阻断

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::on_key_down`；`slterm_app/src/gpui_terminal/CLAUDE.md` 的“退出不等于关闭”；`docs/featureSpec/panels/terminal.md` 的退出语义。
- 现象：`on_key_down` 在 `self.exited.is_some()` 时直接返回，早于 `Ctrl+Shift+C` 复制、`Ctrl+Shift+F` 搜索、`Shift+PageUp/PageDown/Home/End` 回滚等显示与只读快捷键的判断。退出后鼠标仍可选择，但键盘复制选区不可用，搜索不能打开，键盘回滚失效；只有会冒泡到工作区的关闭/重连快捷键仍可工作。
- 根因：把所有按键统一按“进程是否退出”拦截，而退出语义只应拒绝写入 PTY 的输入，不应拒绝读取保留画面的显示命令。
- 影响：用户退出 shell 后无法用键盘复制最后的错误输出，也无法搜索或翻看保留的回滚，违背“显示 worker、事件订阅与最后画面继续服务滚动和选区”的约束。
- 证据：退出判断位于 `on_key_down` 开头；复制、搜索、回滚分支都在其后；`mark_exited` 不清除 frame、binding 与显示命令能力，Registry 也仍允许非 `Input` 的显示命令。现有退出测试只验证状态与拒绝输入，未验证复制/搜索/滚动。
- 优化方向：退出只拦截编码写 PTY 的输入分支，保留复制、搜索、滚动与选区快捷键；需要退出后复制与搜索的虚拟窗口用例。

### 重要｜粘贴超限错误复用致命 error 状态，永久隐藏终端

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::paste`、`TerminalView::render` 的 `error` 分支、`install_test_worker`/`install_reconnect`；`slterm_app/src/gpui_terminal/view/restore.rs` 的 `inject_agent_resume`。
- 现象：粘贴内容超过 10 MiB 时设置 `self.error = Some("粘贴内容超过 10 MiB，已拒绝")`；`render` 只要 `error` 有值就只渲染错误文本，不再渲染 `TerminalElement`、搜索框和底部浮层。生产路径没有清除该 `error` 的入口，只有测试安装新 worker 或重连成功时才置空。Agent 恢复注入因命令通道满而失败时同样设置 `error`，也会永久遮住终端。
- 根因：把“启动/重连失败的持久错误”和“单次操作被拒绝的瞬态提示”共用同一个 `error` 状态，且错误渲染分支与终端渲染互斥。
- 影响：一次过大的粘贴或一次瞬时发送失败会让终端内容从界面消失，用户无法继续查看或复制已有输出，只能关闭终端；错误文本也没有关闭/重试入口。
- 证据：`paste` 的超限分支直接写 `self.error` 并 `cx.notify()`；`render` 的 `if let Some(error) = &self.error` 分支用纯文本覆盖整个内容区；`self.error = None` 只出现在测试安装 worker 与 `install_reconnect`，没有用户可触发的清除路径。`inject_agent_resume` 的发送失败分支也写 `self.error`。
- 优化方向：区分持久启动错误与瞬态操作提示，瞬态提示不得替换终端视图；错误覆盖层需要可关闭或重试，并保留底层画面。需要粘贴超限后终端仍可见且可继续复制的用例。

### 重要｜`Ctrl+Shift+A` 全选契约缺失，并与侧栏快捷键冲突；`Shift+方向键` 选区扩展未实现

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::on_key_down`；`slterm_app/src/app_shell/app_root/command_registry.rs` 的 `ActivateSideView` 定义；`docs/featureSpec/panels/terminal.md` 的快捷键与选区条目。
- 现象：featureSpec 要求 `Ctrl+Shift+A` 全选、`Shift+方向键`/`Shift+PageUp/PageDown`/`Shift+Home/End` 扩展选区。终端没有 `SelectAll` 实现；`Shift+方向键` 不进入选区逻辑，最终由编码器写入 PTY；`Shift+PageUp/PageDown/Home/End` 只做滚动，不做选区扩展。`Ctrl+Shift+A` 不在终端快捷键放行列表内，会被编码器消费并向 PTY 发送控制字节，而统一命令目录把同一组合绑定到 Agent 文件侧栏，终端聚焦时该侧栏快捷键实际不可达。
- 根因：键盘层只实现了鼠标选区，没有把 featureSpec 的选区命令接入 `SelectionCommand`；快捷键放行表与全局命令目录没有针对同一组合做归属裁决。
- 影响：用户无法用键盘全选或扩展选区，复制长输出只能依赖鼠标；Agent 文件侧栏的既有快捷键在终端焦点下失效并被当作终端输入发送。
- 证据：`on_key_down` 的 `ctrl`+`shift` 分支只放行 `t/w/b/p/d/s/g/o/enter/pageup/pagedown` 等，未包含 `a`；命令目录中 `ActivateSideView` 的 `target` 为 `SideViewId::AgentFiles`，`keystroke` 为 `ctrl-shift-a`；`TerminalView` 没有 `SelectAll` 或方向键扩选调用，`Shift+方向键` 落到 `keymap::encode_for_program`。
- 优化方向：在终端焦点下按 featureSpec 实现全选与 Shift 扩选，并把冲突快捷键的优先级明确到统一命令目录；需要真实键盘路径的选区用例。

### 重要｜`Ctrl+Enter` 未发送 LF，`Shift+Enter` 反被当作 LF，行镜像把修饰 Enter 当作提交

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/keymap.rs` 的 `encode_for_program`、`is_shift_enter`、`encode` 的 `"enter"` 分支；`slterm_app/src/gpui_terminal/view/completion.rs` 的 `note_text_key`、`commit_line`；`docs/featureSpec/panels/terminal.md` 的快捷键条目。
- 现象：featureSpec 要求 `Ctrl+Enter` 发送 LF。传统 VT 路径中 `encode` 只对裸 `Shift+Enter` 返回 `\n`，`Ctrl+Enter` 落到 `\r`；增强键盘模式下则分别走 Win32 `KEY_EVENT_RECORD` 或 CSI-u 序列，也不是 LF。`note_text_key` 对所有 key 为 `enter` 的按键（不区分修饰键）调用 `commit_line`，会把 `Shift+Enter`/`Ctrl+Enter` 当成命令行提交，而实际写入 PTY 的字节可能不是提交语义。
- 根因：编码器与行镜像各自判断 Enter 语义，且编码器以 Shift 而非 Ctrl 作为 LF 条件；没有按 featureSpec 绑定。
- 影响：依赖 Ctrl+Enter 插入换行的用户（包括 AI CLI 多行输入）得到错误字节；ShellLineTracker 在修饰 Enter 后进入待决执行，可能阻塞补全或与实际 shell 边界不一致。
- 证据：`encode_for_program` 仅在 `is_shift_enter(ks) && shift_enter_as_lf(...)` 时返回 `\n`；`is_shift_enter` 明确排除 control；`encode` 的 `"enter"` 分支对非 Shift、非 Alt 返回 `\r`；`note_text_key` 的 `"enter"` 分支无条件调用 `commit_line`。现有测试覆盖 `preserves_enter_modifiers` 与 Shift+Enter，没有钉住 Ctrl+Enter 的 LF 行为。
- 优化方向：按 featureSpec 统一 Enter 修饰键编码，并让行镜像只在真实提交语义上建立待决执行；需要 Ctrl+Enter/Shift+Enter 在不同键盘模式下的字节级用例。

### 重要｜真实 ConPTY 验收场景覆盖不足

- 置信度：确认
- 位置：`slterm_terminal/tests/pty_integration_tests.rs` 的 `conpty_spawn_write_read_resize_close_full_chain`、`conpty_output_burst_backpressure_preserves_tail`、`default_shell_resolve_chain_osc_marks`、`da1_unified_identity_bundled_host`、`integration_pty_limit`；`slterm_app/src/gpui_terminal/view/visibility_tests.rs` 的 `terminal_hidden_output_backpressure`；`slterm_app/src/gpui_terminal/view/key_tests.rs` 的 `integration_terminal_search`；`docs/devPlan/design/terminal.md` 的验收要求。
- 现象：设计明确要求真实 ConPTY 覆盖 UTF-8 跨块、洪峰背压、隐藏输出、主/备用屏、resize 水位、进程退出、重连和 64 个存活上限。现有真实 PTY 用例覆盖基础 spawn/写读/resize/关闭、洪峰尾部、shell 探测与 OSC、DA1、句柄回收和 64 上限；UTF-8 跨块、备用屏、隐藏输出、resize 水位、退出前尾部保留和真实重连没有真实 PTY 用例。隐藏与搜索用例使用内存 session 或测试注入帧，不能证明真实 ConPTY 事件顺序。
- 根因：集成层用例覆盖了主链路，但验收矩阵中的跨块、屏幕模式、退出和重连时序仍由单元或测试替身代替。
- 影响：本报告中退出尾帧丢失、读取 EOF、重连旧 worker 竞态、UTF-8 跨块坏码等缺陷无法被现有测试发现；“真实 ConPTY 验收完成”的结论缺少对应证据。
- 证据：上述测试用例的驱动方式均为 `DrivenTerm`/内存 `test_session`/手动帧注入；`terminal_hidden_output_backpressure` 与 `integration_terminal_search` 明确使用测试会话而非真实 PTY；没有用例在退出前写入唯一尾标记并断言最终帧包含它，也没有真实重连用例。
- 优化方向：按设计验收项补齐真实进程用例，尤其是退出前尾部、跨块 UTF-8、备用屏与重连时序；测试替身用例保留，但不得替代真实 PTY 证据。

### 重要｜测试语义与生产路径断裂，容量与搜索缺陷未被捕获

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/registry.rs` 的 `terminal_live_pty_limit`；`slterm_app/src/gpui_terminal/session.rs` 的 `feed_test_bytes`；`slterm_app/src/gpui_terminal/view.rs` 的 `terminal_search_is_read_only` 所在测试及 `feed_test_bytes`；`slterm_app/src/gpui_terminal/view.rs` 的 `terminal_view_never_locks_or_directly_accesses_the_live_grid`。
- 现象：`terminal_live_pty_limit` 用内存会话填满 Registry，只验证 Registry 内的工厂未执行，无法发现真实启动路径在 Registry 之前已经 spawn。`feed_test_bytes` 每次调用都新建 `StreamProcessor`，跨调用丢失 OSC/DA1/DSR 等解析状态，测试无法代表真实跨块输入。搜索用例在喂入新输出后手动调用 `refresh_terminal_search`，而不是走 `process_event` 的自动重查路径。所有权源码扫描只检查 `view.rs`、`pointer.rs`、`layout.rs`、`startup.rs`，不覆盖搜索、补全、恢复等同样可能直接访问网格的文件。
- 根因：测试夹具绕过了生产入口；部分用例验证的是夹具行为而非生产行为；源码扫描范围与约束声明不一致。
- 影响：本报告中的容量预检、搜索风暴、跨块解析与潜在越界访问都没有测试兜底，测试通过不能证明相关合同成立。
- 证据：`terminal_live_pty_limit` 的工厂闭包只构造 `session::test_session()`；`feed_test_bytes` 内 `StreamProcessor::default()` 每次新建；搜索用例在 `feed` 后显式调用 `refresh_terminal_search`；所有权测试的 `include_str!` 列表不包含 `search.rs`、`completion.rs`、`restore.rs` 等。
- 优化方向：测试应通过真实 `TerminalView` 启动路径与真实事件流；测试辅助层保留跨调用解析器状态；源码扫描覆盖全部终端视图文件或改为类型级约束。需要容量拒绝与搜索帧驱动的生产路径用例。

### 一般｜ConPTY 捆绑文件只按大小复用，未校验哈希，提取写盘非原子

- 置信度：确认
- 位置：`slterm_terminal/src/tty/windows/conpty.rs` 的 `ensure_extracted`、`write_if_size_differs`、`load_conpty`；`slterm_terminal/vendor/conpty/README.md` 的来源与 MD5 记录；测试 `write_if_size_differs_overwrites_only_on_size_mismatch`。
- 现象：提取目录已存在且文件大小与嵌入字节一致时直接复用，不校验 MD5/SHA；大小不同才用 `std::fs::write` 覆盖，不是临时文件加 rename。测试明确钉住“同大小不同内容跳过写入”。加载时只检查文件对存在并按绝对路径 `LoadLibraryW`，没有内容哈希校验。
- 根因：把“嵌入内容编译期固定”当作完整性保证，忽略了提取目录可能被同大小替换、损坏或来自旧版本；写盘没有遵循原子写纪律。
- 影响：被篡改或损坏但大小相同的 `conpty.dll`/`OpenConsole.exe` 会被加载；提取中断留下部分文件时，下一次按大小判断可能误复用或直接失败回退。README 记录了 MD5，但运行时不使用。
- 证据：`write_if_size_differs` 只比较 `metadata.len()`；`ensure_extracted` 逐文件调用该函数；测试 `write_if_size_differs_overwrites_only_on_size_mismatch` 断言相同长度不同内容保持旧内容；`vendor/conpty/README.md` 记录 MD5 但代码没有哈希比较。
- 优化方向：复用与加载前校验嵌入哈希，提取使用原子写并处理并发/中断；哈希校验失败应回退并记录原因。需要同大小篡改与中断写入用例。

### 一般｜Win10 真实 PTY 测试会写入真实 `%LOCALAPPDATA%`，条件跳过未登记

- 置信度：确认
- 位置：`slterm_terminal/tests/pty_integration_tests.rs` 的 `options`、`conpty_spawn_write_read_resize_close_full_chain`、`default_shell_resolve_chain_osc_marks`、`pwsh_integration_script_osc_semantics`、`bundled_psreadline_first_input_is_not_polluted`；`slterm_app/src/app_shell/app_root/tests/terminal_restore.rs` 的 `terminal_restore_real_pty_auto_and_lazy_launch`/`terminal_restore_real_pty_falls_back_to_project_root`；`.claude/docs/testing/exemptions.md` 的条件跳过清单。
- 现象：集成测试的 `options` 使用默认 `conpty_sideload = true`。在 build 小于 21376 的 Win10 上，测试 exe 旁通常没有完整文件对，`ConptyApi::new` 会走嵌入字节提取到 `%LOCALAPPDATA%\slterm\conpty` 并加载；app 侧真实 PTY 恢复测试通过 `LocalTerminalLauncher` 走 `local_options`，同样硬编码 sideload。另有一些测试在缺少 pwsh/powershell 时只打印“跳过”并返回，未登记到条件跳过表。
- 根因：测试启动参数未统一关闭侧载或注入 `LOCALAPPDATA`；条件跳过依赖打印而不进入豁免登记。
- 影响：在目标平台 Win10 上运行测试会读写真实用户目录，违反测试隔离纪律；无 pwsh 的机器上相关用例静默计为通过，覆盖结论不可复现。
- 证据：`options` 只覆盖 `shell` 字段；`default_shell_resolve_chain_osc_marks` 等在 `ShellKind::Cmd` 或找不到 pwsh 时 `eprintln!` 后 `return`；豁免表的条件跳过段只登记了别名/ACL 与凭据管理器用例。当前开发机为 Win11，上述 Win10 路径未实际触发。
- 优化方向：测试统一使用注入的临时 `LOCALAPPDATA` 或关闭 sideload；所有条件跳过登记根因与兜底。需要在 Win10 构建上验证不写真实目录。

### 一般｜豁免登记册引用失效路径与测试名

- 置信度：确认
- 位置：`.claude/docs/testing/exemptions.md` 的 `EventLoop` 读/写 I/O 行、容量与 bundled OpenConsole 行。
- 现象：豁免行引用 `slterm_terminal/src/event_loop.rs`，实际文件为 `event_loop/mod.rs`；引用测试 `spawn_lock_serializes_and_capacity_32_enforced`，实际为 `spawn_lock_serializes_capacity_check_and_reservation`；引用 `mirror_da1_query`，仓库中只有 `mirror_dsr_query`。文档宣称“复审已核验所有被引用的测试函数/模块/路径存在”，与实际不符。
- 根因：实现改名/移动后未同步更新豁免表；复审只改结论未核验锚点。
- 影响：豁免兜底证据链断裂，后续无法据此判断真实覆盖；门禁与文档策略检查没有发现失效锚点。
- 证据：上述路径与测试名在仓库中不存在；容量真实测试名为 `integration_pty_limit`，单元测试名为 `spawn_lock_serializes_capacity_check_and_reservation`；`mirror_da1_query` 无函数定义。
- 优化方向：按当前符号与路径修正豁免锚点，并把锚点存在性纳入文档策略或脚本门禁。

### 一般｜重连未重置动态颜色与光标模板，新进程继承旧 SGR 状态

- 置信度：确认
- 位置：`slterm_terminal/src/term/reconnect.rs` 的 `Term::begin_reconnect`；`slterm_terminal/src/term/mod.rs` 的 `Term::reset_dynamic_colors`、`Colors`、`grid.cursor.template`。
- 现象：`begin_reconnect` 清理了备用屏、模式、提示符标记、标题、选区、滚动区、Tab 位、键盘模式并插入分隔线，但没有重置 `self.colors`（OSC 4/10/11/12 等动态颜色）和 `grid.cursor.template`（前景、背景、粗体、斜体、下划线、反显等 SGR 状态）。旧进程退出时若仍处于反显、下划线或自定义调色板，新 shell 的首个提示符和分隔线会继承这些状态。
- 根因：重连清理清单没有覆盖所有跨进程显示状态；`reset_dynamic_colors` 已存在但未在重连路径调用。
- 影响：新会话首屏颜色/字形可能与默认主题不一致，用户自定义动态颜色在重连后残留；与“清理旧进程交互态”的意图不符。
- 证据：`begin_reconnect` 中没有对 `colors` 或 `cursor.template` 的赋值；`reset_dynamic_colors` 只在测试或其他路径定义，未被重连调用；分隔线使用当前光标模板绘制。
- 优化方向：重连边界恢复到默认显示状态，再插入分隔；需要设置动态颜色和 SGR 后重连的断言用例。

### 一般｜`conpty_realign` 下滚后未调整提示符绝对标记与输入边界

- 置信度：确认
- 位置：`slterm_terminal/src/term/mod.rs` 的 `Term::conpty_realign`；`slterm_terminal/src/term/prompt.rs` 的 `slterm_add_prompt_mark`、`slterm_mark_prompt_input`、`slterm_prompt_input_point`；`slterm_terminal/src/grid/mod.rs` 的 `scroll_down`。
- 现象：当 conhost 的视口行号大于本地光标行时，`conpty_realign` 调用 `grid.scroll_down` 并增加光标行；`scroll_down` 在存在回滚时不会改变 `history_size`/`scrolled_out`，但 `slterm_prompt_marks` 与 `slterm_prompt_input` 保存的是绝对行号，没有随之下移。之后提示符跳转、回显快照和 `ShellLineTracker` 的输入边界会指向旧位置；如果提示符仍在活动状态，快照可能计算错误或返回 None。
- 根因：提示符标记使用绝对坐标，只有正常输出滚动/重置维护它们；`conpty_realign` 直接操作网格，没有调用提示符失效或坐标平移。
- 影响：resize 对账发生在提示符期间时，补全/历史采集可能失效到下一个提示符，提示符跳转落在错误行；上滚方向的绝对坐标恰好保持不变，下滚方向不稳定。
- 证据：`slterm_cursor_abs_line` 为 `scrolled_out + history_size + cursor.line`；`scroll_down` 不改变前两项而光标增加；`conpty_realign` 只调整 selection 并 `mark_fully_damaged`，未修改 `slterm_prompt_marks`/`slterm_prompt_input` 或调用 `slterm_end_prompt`。
- 优化方向：重排改变坐标系时同步平移或作废提示符标记与输入边界；需要 resize 期间存在活动提示符的用例。

### 一般｜初始网格固定 116×30，与 featureSpec 的 80×24 回退不符

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `DEFAULT_GRID_COLUMNS`、`DEFAULT_GRID_LINES`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `grid` 构造。
- 现象：新建与恢复启动都用 116×30 作为 spawn 网格；featureSpec 规定“尺寸变化先按旧网格处理已接收输出，再应用新尺寸并通知 PTY；初始尺寸暂不可用时使用 80×24，容器就绪后立即适配”。实际没有 80×24 的回退分支，而是沿用旧壳的 116×30。
- 根因：默认画布常量来自旧壳，未按当前 featureSpec 调整。
- 影响：无法在启动时取得可靠容器尺寸的场景会以非约定初始尺寸启动 shell，随后 reflow 一次；与行为真值不一致，也可能放大启动期的 ConPTY rewrap。
- 证据：两个常量分别为 116 与 30；`launch`/`restore` 都直接传这对常量作为 `spawn_grid`，`prepare` 只用 `.max(2)` 下限；没有 80×24 分支。
- 优化方向：按 featureSpec 提供 80×24 的不可用尺寸回退，并在容器就绪后立即适配；需要无容器尺寸时的启动用例。

### 一般｜退出显示缺少“重新连接／关闭面板”的可操作入口

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::render` 的 `exited` 浮层、`TerminalView::mark_exited`；`docs/featureSpec/panels/terminal.md` 的退出条目。
- 现象：进程退出后只在底部浮层显示退出消息文本，没有“重新连接”或“关闭面板”的按钮/可点击入口。重连与关闭虽然有 `Ctrl+Shift+R`/`Ctrl+Shift+W` 命令绑定，但退出显示本身不呈现操作。
- 根因：退出状态只驱动文本浮层，没有把命令目录中的重连/关闭动作接入该浮层。
- 影响：featureSpec 要求退出后显示退出码和“重新连接／关闭面板”，用户无法从退出提示直接恢复或关闭，只能记忆快捷键或寻找其他入口。
- 证据：`render` 的 `bottom_stack` 只追加 `exited` 文本；`mark_exited` 只设置消息并通知；命令目录存在 `ReconnectTerminal`/`CloseActiveTerminal`，但视图没有为退出浮层绑定动作。
- 优化方向：退出浮层提供与命令目录同源的重连与关闭动作；需要退出后点击重连/关闭的虚拟窗口用例。

### 一般｜默认标签标题与 CLI 运行中重命名规则不符合 featureSpec

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/startup.rs` 的初始 `title` 与 `title_locked`；`slterm_app/src/gpui_terminal/view.rs` 的 `ResetTitle` 分支；`slterm_app/src/app_shell/app_root/tabs.rs` 的 `tab_title`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `rename_terminal`。
- 现象：新建终端的初始 `title` 为 `"shell"`，`tab_title` 只过滤空标题后直接使用视图标题，因此默认标签显示 `shell` 而不是 `terminal-N`；`ResetTitle` 也恢复为 `"shell"`。featureSpec 规定自定义标题优先于 CLI 标题和默认编号，默认编号才是 `terminal-N`。此外 `rename_terminal` 只做快照标题校验，不检查 `running_program`/CLI 是否正在运行，而 featureSpec 规定 CLI 运行中不可重命名。
- 根因：把占位字符串 `"shell"` 当成默认显示标题，没有“是否已有 CLI 标题”的状态；重命名路径没有读取终端运行状态。
- 影响：新建终端不显示约定的编号；重置标题不回到默认编号；CLI 运行期间仍可经 Runtime 重命名，违反元数据规则。
- 证据：`prepare` 中 `let title = shell_name.unwrap_or_else(|| String::from("shell"))`；`ResetTitle` 同样写 `"shell"`；`tab_title` 使用 `view.title` 并仅在空串时回退 `format!("terminal-{}", record.number())`；`rename_terminal` 没有 `running_program` 检查。
- 优化方向：区分“无标题”和默认编号，CLI 标题到达前显示 `terminal-N`；重命名前查询终端运行状态。需要默认标题、重置标题与 CLI 运行中重命名的用例。

### 提示｜存在未接线或只有测试消费者的状态

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/event_mailbox.rs` 的 `NativePromptState`；`slterm_app/src/gpui_terminal/view/layout.rs` 的 `mark_structural_resize`/`structural_resize`；`slterm_app/src/gpui_terminal/view.rs` 的 `default_cursor_style`、`apply_settings`。
- 现象：`NativePromptState` 的 `observe_prompt`/`observe_input` 会在生产事件与输入路径更新状态，但 `snapshot()` 只被测试读取，没有生产消费者；`structural_resize` 的置位函数带“无生产者”注释，分屏/zoom 等结构性布局不会走立即提交路径；`default_cursor_style` 字段注释说明消费点尚未接线；`apply_settings` 同样无生产调用者。
- 根因：设置与提示符能力接入未完成，留下可编译但无效的状态。
- 影响：相关行为看起来已实现但实际不生效，维护者容易误判覆盖；结构性 resize 全部落入尾沿去抖，与注释声明的策略不一致。
- 证据：`NativePromptState::snapshot` 只出现在测试与 Registry 的输入观察路径；`mark_structural_resize` 标记为未接线；`apply_settings` 标记“设置协调器接入后消费”；`default_cursor_style` 标记“恒默认”。
- 优化方向：接入真实消费者或删除未用状态；结构性布局变更应在真实入口置位。需要对应行为测试，避免死代码掩盖缺口。

### 提示｜DA1/DSR 代答绕过有界命令队列，写失败只记录日志

- 置信度：确认
- 位置：`slterm_terminal/src/event_loop/mod.rs` 的 `strip_and_answer`、`inject_da1_response`、`inject_cpr_response`；`slterm_terminal/src/event_loop/mod.rs` 的 `pty_write` 写队列。
- 现象：DA1/DSR 的应答在读取处理路径直接调用 `self.pty.writer().write_all`/`flush`，不经过 `State::write_list` 与 `pty_write` 的背压/错误处理；写入失败只记录错误，不产生 `PtyFailure` 或调用方可见结果。
- 根因：应答注入被视为启动握手旁路，未复用命令端点。
- 影响：`write_all` 在管道压力下可能阻塞事件循环或直接失败；失败被静默吞掉会让子进程等待握手，表现为启动期卡顿或输入异常。当前读写在同一事件循环线程，未发现跨线程撕裂，但错误不可观测。
- 证据：`inject_da1_response` 与 `inject_cpr_response` 直接写 writer 并 `log::error!`；`pty_write` 的 `Ok(0)` 会被当作背压保留，而直接 `write_all` 没有该状态机。
- 优化方向：将代答应答纳入同一有界写队列和错误出口，或在失败时上报故障；需要管道写压力下的握手用例。

## 跨域观察

- 追溯矩阵存在无对应测试函数的条目：`agent_files_default_capabilities`、`commit_rollback_preserves_staged_changes`、`path_file_identity_deduplicates_document` 只出现在 `docs/devPlan/plan.md`、`traceability.md` 或 `progress.md`，仓库中没有同名测试函数。相关模块不在本次终端核心范围，但矩阵“已自动化”的状态与代码事实不符，需由对应所有者复核。
- `terminal_hidden_output_backpressure` 的用例名包含 backpressure，但实际验证的是隐藏期间帧合并与恢复可见，不覆盖核心 `OutputQueue` 的块数/字节上限；核心洪峰用例另在 `conpty_output_burst_backpressure_preserves_tail`，两者语义不应混为同一证据。
- `terminal_search_is_read_only`/`integration_terminal_search` 使用内存会话与手动帧注入，未覆盖真实 ConPTY 的搜索时序；真实 PTY 验收结论不应引用这两个用例。
- `slterm_app/src/app_shell/app_root/commands.rs` 的 `rename_terminal` 与 `slterm_app/src/app_shell/app_root/tabs.rs` 的标题派生属于终端元数据接线，已在本报告标题问题中记录；其规则由工作区与终端共同拥有。

## 存疑未证

- 空 `Msg::Input` 的卡死机制依赖底层 `Write` 对空切片返回 `Ok(0)` 或可重试错误；未运行验证 `piper::Writer`/Windows 管道的具体返回值，但任何零长度前进都不改变保留已“完成”项的事实。需要运行用例确认。
- 帧唤醒竞态、重连旧 worker 注入 `Exit`、子进程退出尾帧丢失都依赖线程/事件交错；代码路径已闭合，但没有确定性运行时复现。需要真实事件循环与可控延迟的测试。
- 读取线程 EOF/错误未上报的影响需要“读取端失效但子进程事件未到”的真实场景；当前没有证据证明该场景在 ConPTY 生命周期中可稳定出现。
- Win10 捆绑 OpenConsole 的真实 DA1/DSR/CPR 行为、提取目录的哈希与加载边界未在 Win10 实机验证；本机 Win11 只覆盖 in-box 路径与纯函数分支。
- 真实 IME、鼠标跟踪、滚轮、kitty 协议与系统剪贴板未在桌面会话中验证；相关条件跳过用例的环境依赖未在当前审查中复现。
- `conpty_realign` 后提示符标记错位的视觉与补全影响未做实机目测；需要 resize 期间存在活动提示符的用例。
- 搜索匹配坐标在回滚与备用屏下的显示偏移未做视觉验证；现有用例只断言当前匹配落在可见区域，未覆盖多帧输出后的导航稳定性。
- `RuntimeSettings.shell` 等设置键是否确为产品要求的接线面，需以 featureSpec 与设置页契约为准；本报告按 featureSpec 的默认 shell/显式 shell 语义判定为未接线。
- 子进程退出事件与读写事件在同一批 poll 事件中的顺序依赖运行时；未在真实 PTY 上注入“退出前最后输出”以确认丢失范围。
