# 终端补全、建议与本地学习

> 审查基线 `53c2a150`（当前分支）；范围为 `slterm_completions`、应用侧补全/建议/学习/Git 补全接线，以及相关文档与测试。

## 覆盖与局限

- 已读：`slterm_completions` 全部源码（匹配、方言、编辑、文件/目录、语义/Git/脚本上下文）；`slterm_app` 的补全会话与请求、建议计算、可信行镜像、呈现接线、目录历史、学习服务与写线程、学习库 schema、项目脚本与 workspace 发现、Git 只读补全面；相关模块 `CLAUDE.md`、`featureSpec/panels/terminal.md`、`featureSpec/panels/settings.md`、`featureSpec/acceptance.md`、`docs/devPlan/design/terminal.md`、`docs/devPlan/traceability.md`、`.claude/docs/testing/strategy.md`。
- 已核对：追溯矩阵引用的补全/可信行/学习库/清空用例均存在且名称一致；Git 补全没有对应追溯条目。
- 未覆盖：GPUI 弹窗绘制与布局像素结果、真实 ConPTY 提示符序列、真实桌面环境下的 PATH 差异、实际运行时长与抖动。按合同未运行 cargo、测试、网络与任何构建。
- 推断边界：静态阅读能确认的代码路径标为确认；依赖运行时环境或实际事件序列的标为疑似，并在条内写明缺口。

## 问题

### 重要｜设置页缺少命令与目录采集开关、披露与清空

- 置信度：确认
- 位置：`slterm_settings/src/keys.rs`、`slterm_app/src/app_shell/app_root/settings_controls.rs`、`slterm_app/src/app_shell/config.rs`、`slterm_app/src/learning.rs`；`featureSpec/panels/settings.md`「补全与本地历史」、`featureSpec/panels/terminal.md`「本地学习数据」。
- 现象：设置页只有补全显示（`ghost`）、形态（`completion_style`）与接受键（`accept`）；没有命令历史/目录历史各自的采集开关，没有“只来自可信 Shell 输入行”的披露，没有带确认的清空入口。清空只有独立 CLI `clear-learning`。
- 根因：学习控制从未登记进设置键与设置中心字段表；采集在视图无条件发生；运行 `Settings` 结构也不携带学习控制。
- 影响：用户无法停用本机学习采集或从设置清空；`featureSpec/acceptance.md`「本地学习数据：命令与目录历史可独立停用、清空」不成立；未来补披露时还会与当前目录采集边界冲突。
- 证据：`keys.rs` 无历史/学习键，`settings_controls` 字段表只有 `ghost`、`completion_style`、`accept`；`config.rs` 的 `Settings` 无学习字段；`TerminalView` 在 `CommandDone`、`CwdReport`、`Title` 路径直接采集；`cli.rs::run_clear_learning_at` 是唯一清空入口；`traceability.md`「本地学习数据」只锚 `integration_local_learning_data`，该用例不覆盖停用。
- 优化方向：把命令采集与目录采集登记为独立设置键并生成页面控件，清空经确认后走 `LearningService::clear`，显示开关与采集开关互不隐含；披露文案以实际可信边界为准。

### 重要｜目录学习采集未受可信行/备用屏门控

- 置信度：确认
- 位置：`TerminalView::process_event` 的 `CwdReport`/`Title` 分支、`cwd_report::apply`、`Session::record_directory`、`DirectoryHistory::record_memory_at`；`featureSpec/panels/terminal.md`「本地学习数据」、`featureSpec/acceptance.md`「补全可信边界」、`docs/devPlan/design/terminal.md`「learning.db 与历史采集」。
- 现象：任意程序输出 OSC 7/9;9，或设置以 `SLTERM|` 开头、首段为绝对路径的标题，都会立即进入目录学习；备用屏、TUI、未知来源的 CLI 与失配镜像均未被检查。命令采集有 `ShellLineTracker` 待决执行与回显对账，目录采集没有对应 gate。
- 根因：目录事实走 cwd 事件旁路，只有“受管 cwd 边界”的来源假设，没有在消费点验证当前提示符代次、备用屏状态或行镜像可信性。
- 影响：TUI/CLI 可注入任意绝对路径并污染目录 frecency（进而影响 `cd` 候选与路径排序），与「备用屏、CLI 交互提示和不可信镜像不采集」的验收口径冲突。
- 证据：core `CwdSniffer` 对全部 PTY 字节发送 `CwdReport`，不区分主屏/备用屏；`cwd_report::apply` 接受任意绝对 `Title` 路径；`process_event` 在两处直接调用 `record_directory`，未调用 `completion_line()` 或检查 `TermMode::ALT_SCREEN`；命令记录路径才要求 `PendingExecution.started` 且镜像一致。
- 优化方向：目录采集绑定与命令相同的可信提示符边界（或明确受管 cwd 的认证来源），并补备用屏/TUI 注入用例。

### 重要｜补全按键绕过统一命令目录且忽略修饰键

- 置信度：确认
- 位置：`TerminalView::handle_completion_key`、`TerminalView::on_key_down` 的调用顺序、`CommandId`（`command_registry`）；`featureSpec/panels/terminal.md`「可信输入与呈现」、`featureSpec/panels/settings.md`「补全与本地历史」、`featureSpec/cross-cutting.md` 的唯一命令目录。
- 现象：Right、Tab、Up/Down、Enter、Esc 的补全语义在视图中按 `ks.key` 直接匹配；`mods` 未参与判定，末尾只用 `let _ = mods` 消音。存在弹窗选中项时，Ctrl+Enter 被当作“接受候选”而不再进入编码器；Ctrl+Right 可接受 ghost；Shift+Tab 被当作候选导航。补全接受/导航没有对应的 `CommandId`，无法进入目录、重绑或冲突检测。
- 根因：补全动作绕开 `CommandRegistry`，退化为终端视图内的键名字符串分支。
- 影响：终端合同中的 Ctrl+Enter 发送 LF 在有候选时失效；修饰键被静默吞掉；设置页的补全快捷键无法参与统一重绑与冲突检查。
- 证据：`handle_completion_key` 所有分支只匹配 key；弹窗分支对 enter/right 不检查修饰键；ghost 接受分支对 right 不检查；`CommandId` 枚举无补全项；现有 UI 用例只覆盖 Ctrl+Tab 冒泡，不覆盖 Ctrl+Enter/Right。
- 优化方向：把补全接受、导航、关闭登记为上下文命令并由按键层按修饰键派发；补全只消费未修饰键或显式配置键，其余交还编码器。

### 重要｜补全显示设置不热应用，accept 设置无消费端

- 置信度：确认
- 位置：`TerminalView::apply_settings`、`TerminalView` 的 `completion_style`/`ghost_enabled` 字段、`TerminalView::prepare`（`view/startup.rs`）、`Settings`（`app_shell/config.rs`）、`settings_controls` 的 `accept` 字段；`featureSpec/panels/settings.md`「补全与本地历史」。
- 现象：补全形态与显示开关只在新终端构造时读取；`apply_settings` 无调用者且带 dead-code 豁免。`accept` 既未进入运行 `Settings` 结构，也没有任何视图读取它，改设置对任何终端都无效果。
- 根因：设置协调器只重建全局快照、主题与后台任务，没有把补全配置分发到现有 `TerminalView`；accept 字段从未接线。
- 影响：设置页修改形态或关闭补全不会影响当前终端的后续候选，与「修改立即作用于后续候选」不符；接受键选项是死控件。
- 证据：`reload_settings_snapshot` 只 `set_global`、刷新主题与后台任务；`TerminalView` 只在构造时从全局快照取 `completion_style`/`ghost`；`Settings` 无 `accept` 字段；`apply_settings` 仅定义无调用。
- 优化方向：让终端视图订阅设置或按候选计算读取当前快照，并把 accept 接入按键判定；删除 dead-code 豁免并补热应用与接受键用例。

### 重要｜Popup/Hybrid 的选中与 Tab 语义偏离合同

- 置信度：确认
- 位置：`TerminalView::handle_completion_key` 的弹窗分支、`TerminalView::refresh_suggestion` 的结果回填、`SuggestState` 的字段文档与 `popup_does_not_accept_before_explicit_navigation`；`featureSpec/panels/terminal.md`「可信输入与呈现」、`docs/devPlan/design/terminal.md`「CompletionService」。
- 现象：Popup 形态下首个 Tab 先选中首项再立即接受，而不是只做候选导航；Hybrid 请求弹窗后，结果回填会把 `completion_selected` 自动设为 0，用户未主动导航时 Enter 就会接受首项；Hybrid 在 `request_completion_popup` 返回 false（无候选）时仍吞掉 Tab。
- 根因：接受与导航在非 Hybrid 弹窗分支合流；结果回填把“有结果”当成“已选”；Tab 请求忽略返回值。
- 影响：与「有弹窗时 Tab 用于候选导航」「无选中项时 Enter 原样交 PTY」以及 `SuggestState` 的“未导航不高亮”矛盾；弹窗浏览只能用 Up/Down；无候选时 Shell 自己的 Tab 补全被阻断。
- 证据：`"tab" if !hybrid` 分支先 `completion_popup_move` 再 `accept_completion_popup`；回填条件 `popup_requested && !completion_items.is_empty() && completion_selected.is_none()` 会写入 `Some(0)`；Hybrid 分支无条件 `return true`；`hybrid_tab_requests_popup_and_escape_suppresses_the_same_line` 只验证请求与关闭，不验证选中与 Tab 导航。
- 优化方向：弹窗可见时 Tab 只导航，Enter 或配置的接受键才接受；结果回填不自动选中；无候选时交还终端；同步修正文档与用例。

### 重要｜清空竞争导致成功误报或未清空

- 置信度：确认
- 位置：`LearningWriterClient::clear`、`run_writer` 的 `WriterCommand::Clear`、`LearningService::clear`、`learning_clear_cancels_pending_writes`、`learning_queue_full_is_nonblocking_and_invalidates_old_writes`；`slterm_app/src/persistence/CLAUDE.md`「学习写线程与代次」、`featureSpec/panels/settings.md`「补全与本地历史」。
- 现象：清空先递增共享代次，再 `try_send` 清空命令。若队列满，代次已推进但清空未入队；先前已入队的旧代次清空在写线程里因代次不符直接返回 `Ok(())`，数据库未删。两次清空时先调用者可能收到成功，而真正执行删除的是后一次；后一次失败时历史在重启后复活。
- 根因：被更新代次取代的清空与实际完成的清空共用 `Ok(())`；调用方无法判断自己的代次是否落库；队列满路径也推进代次。
- 影响：出现“已清空”的假成功或完全未清空；违反「失败保留可见错误，不报告已清空」。当前生产清空入口只有持实例锁的 CLI 单次调用，接入设置页清空后即可达。
- 证据：`fetch_update` 后 `try_send`；worker 对 `clear_generation != generation.load()` 返回 `Ok(())`；现有用例只覆盖单次清空与记录队列满，不覆盖两次清空或第二次清空入队失败。
- 优化方向：清空回执必须能区分“本代次已清”“被更新代次取代”“失败”；或串行化/合并清空并返回确定终态；补两次清空与第二次入队失败用例。

### 重要｜项目脚本与 workspace 有界预算缺少真实测试

- 置信度：确认
- 位置：`project_scripts_tests::scripts_follow_project_scope_and_refresh_without_running_project_code`、`oversized_manifest_and_relative_cwd_fail_closed`、`workspace::Discovery`/`collect`、`workspace::finish_catalog`；`slterm_app/src/completion/project_scripts/CLAUDE.md`「有界读取」「验收与测试」。
- 现象：单 manifest 的“超限”用例写入的是空格填充，本身不是合法 JSON；即使删除 1 MiB 检查，解析也会失败并返回空，因此用例不能证明预算。workspace 的 500 ms、4 MiB、4096 目录、512 包上限没有任何测试。`finish_catalog` 无生产调用者，也无用例。
- 根因：测试只覆盖“不可解析”，没有用“可解析但超限”的输入隔离预算；发现流程没有可注入时钟/预算。
- 影响：有界读取回归不会被门禁发现；不可信项目目录可能重新变为无界扫描；模块声明的必测面不成立。
- 证据：`oversized_manifest_and_relative_cwd_fail_closed` 使用 `vec![b' '; 1024 * 1024 + 1]`；`Discovery::read`、`Discovery::ready`、`collect` 的常量限制；workspace 测试模块只覆盖 YAML 窄解析与选择语义；`finish_catalog` 仅有定义。
- 优化方向：用合法 JSON 超限、深目录与多包夹具、可注入时钟分别验证每个上限降级为空；删除或接线 `finish_catalog`。

### 重要｜`missing_git_cache_for_semantic_source_never_falls_back_to_history` 前置不成立

- 置信度：确认
- 位置：`completion_tests::missing_git_cache_for_semantic_source_never_falls_back_to_history`、`Session::request_at_revision`、`Request::calculate`；`featureSpec/acceptance.md`「补全可信边界」。
- 现象：用例注释声明“Git 缓存缺失”，但 `request_at_revision` 对 Git 语义来源始终构造 facade 与默认执行环境，不存在“来源缺失”的注入点；用例使用 `cwd` `.`，实际会对运行目录所在仓库执行真实 refs 读取，只因该仓库没有以 `s` 开头的分支才返回空。
- 根因：测试把“空缓存”当作“来源不可用”；真实失败路径是 `complete_available` 返回 `None` 后被 `or_else(|| Some(Vec::new()))` 吞成空。
- 影响：该防复发锚没有验证“Git 来源失败不回收历史”；在分支名以 `s` 开头的 checkout 中结果改变，在非仓库源码树中又因另一原因通过，环境相关。
- 证据：`request_at_revision` 的 `execution.cloned().unwrap_or_else(|| PaneExecContext::from_pty_options(&Options::default()))`；测试 `cwd` 为 `.`；`Request::calculate` 的 `or_else`；ambient 仓库来自 crate 父目录。
- 优化方向：注入一个确定失败的 Git 来源或使用非仓库临时 cwd，并在测试历史中放入能匹配前缀的命令，断言候选仍为空；这也是唯一能证明“失败不伪装成历史”的形态。

### 重要｜可信边界与旧代丢弃的验收测试不足

- 置信度：确认
- 位置：`feedback_tests::terminal_trusted_line_completion_gate`、`completion_generation_and_source_failure`、`suggest_tests::query_keys_gate_stale_background_results`、`TerminalView::refresh_suggestion`、`TerminalView::commit_line`、`Request`；`featureSpec/acceptance.md`「补全可信边界」、`docs/devPlan/design/terminal.md`「CompletionService」。
- 现象：可信门控用例用 Left 使镜像失效，未驱动备用屏/TUI；cwd 用例只比较新请求完成后的终态，不注入旧 key、旧 revision 或旧 cwd 的迟到结果；没有用例证明提交命令或提示符代次变化会取消在途请求；`Request` 也不携带设计列出的 TerminalId、PTY 代次、Prompt 代次与项目代次。
- 根因：取消依赖视图缓存键和任务替换，测试只观察最终状态；请求身份比设计窄，提交路径只清候选与缓存而不取消任务。
- 影响：删除旧结果守卫或取消逻辑的回归不会被现有锚发现；“旧代结果不回填”的验收证据实际来自状态单元测试，不来自真实异步路径。
- 证据：回包守卫在 `refresh_suggestion` 的更新闭包中比较 key、行、revision、形态与 exited；用例均在 `run_until_parked` 后断言；`commit_line` 调用 `invalidate` 与 `clear_completion_hints`，不清 `suggestion_task`；`design/terminal.md`「CompletionService」要求请求携带多代次。
- 优化方向：用可延迟返回的来源或注入时钟制造迟到结果，断言 UI 与 PTY 零变化；补备用屏帧用例；提交与提示符变化时显式取消并测试；对齐请求身份字段或修订设计。

### 一般｜学习持久化失败恢复路径缺失

- 置信度：确认
- 位置：`run_writer` 的 `failed` 标志、`LearningWriterClient` 状态、`LearningService::start`、`app_shell::run_shell_once` 的启动回退、`cli::run_clear_learning_at`；`docs/devPlan/design/terminal.md`「learning.db 与历史采集」、`docs/devPlan/design/persistence.md` 的学习库段落。
- 现象：任何一次数据库写错误都会把 `failed` 置位并跳过后续命令，只有一次成功的清空能复位；损坏或高版本时服务静默降级为内存学习，没有设计要求中的显式重置；`Failed`/`MemoryOnly` 状态只有 CLI 读取；数据库损坏时 CLI 清空在 `LearningService::start` 就失败。
- 根因：可重试 I/O、损坏与版本不兼容被归为同一永久失败；缺少 reset 端口与用户可见状态。
- 影响：一次瞬时锁/IO 失败即可让本次会话后续学习静默丢失；损坏库只能由用户手动找到并删除文件才能恢复。
- 证据：`run_writer` 在 `record_command`/`record_directory` 任意 `Err` 置 `failed`；仅 `Clear` 成功分支复位；`LearningService::start` 失败时调用方使用 `memory_only` 并只写日志；仓库内无 reset API 或设置项。
- 优化方向：区分可重试错误与损坏/版本不兼容，对前者退避重试；为后者提供保留原库的显式重置并暴露状态；补服务层降级与恢复用例。

### 一般｜Inline ghost 截断被接受，命令 ghost 无编辑合同

- 置信度：确认
- 位置：`suggest::calculate`、`clamp_ghost`、`SLTERM_GHOST_MAX`、`TerminalView::handle_completion_key` 的 ghost 接受分支、`tail_edit`；`slterm_app/src/gpui_terminal/CLAUDE.md`「补全编辑复核」。
- 现象：历史余量在构造 `CompletionEdit` 之前被截到固定上限，Tab/Right 接受后只插入截断部分；PATH 命令余量没有 `CompletionEdit`，接受路径直接把字符串追加到行镜像并写 PTY，未执行原文指纹复核。
- 根因：显示预算与接受值共用同一字符串；命令 ghost 走独立写路径。
- 影响：长历史命令被静默截断成另一条命令；命令 ghost 的接受绕过编辑合同；两条路径都无长命令/长历史测试。
- 证据：`hinted.map(clamp_ghost)` 之后才 `tail_edit`；`command_hint` 只写 `result.suggestion`；ghost 接受分支执行 `line.push_str(&ghost)` 后 `write_user_text`，未调用 `CompletionEdit::apply`。
- 优化方向：显示预算只作用于展示；接受使用完整候选或明确拒绝超限；所有接受路径统一经 `CompletionEdit` 复核并补用例。

### 一般｜匹配高亮索引单位不一致且无矩阵测试

- 置信度：确认
- 位置：`CandidateMatcher`、`IgnoreCaseExt`、`Suggestion::match_indices`、`matcher_tests`；`slterm_completions/CLAUDE.md`「匹配算法」「验收与测试」。
- 现象：prefix/substring 路径用 grapheme 数计算 `match_indices`，fuzzy 路径直接使用 nucleo `Atom::indices` 的 UTF-32 码点索引，并统一加上前导引号字节偏移；同一字段在不同算法下单位不同。应用当前不消费 `match_indices`，测试也没有任何索引数值/emoji/组合字符断言。
- 根因：移植时保留了两套索引语义；高亮索引没有产品消费端，测试只断言成员与排序。
- 影响：声明的“grapheme 对齐”合同不成立；一旦接弹窗高亮，fuzzy 结果在 emoji/组合字符/引号候选上会错位；Windows 的大小写不敏感分支也缺少直接矩阵。
- 证据：unscored 路径 `haystack_folded[0..byte_start].graphemes(true).count()`；fuzzy 路径 `atom.indices(...)` 返回码点索引（nucleo 文档明确为 char indices）；`matcher_tests` 无 `match_indices` 断言；`match_indices` 在 `slterm_app` 无消费者。
- 优化方向：统一索引单位为 grapheme 或明确改为码点，并在类型文档中锁定；补 prefix/substring/fuzzy × 大小写 × Unicode/emoji × 引号的精确矩阵；若暂无消费者则裁剪或标为内部。

### 一般｜测试读取真实环境，隔离不足

- 置信度：疑似
- 位置：`completion::cache_key`、`suggest::commands_handle`、`completion_tests::cache_key_changes_with_the_input_snapshot_and_command_generation`、`git::command`、Git 相关真实仓库用例、GPUI `feedback_tests` 的视图打开/刷新；`.claude/docs/testing/strategy.md`「隔离优先」。
- 现象：`cache_key` 首次调用会初始化进程级 `commands_handle`，在后台扫描真实 PATH 并运行 `pwsh`/`powershell`；所有会重算键的 GPUI 用例都会触发。Git 用例继承用户与系统 git 配置，未设置 `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_NOSYSTEM`。`cache_key` 用例还断言相邻两次调用返回同一键，而后台命令表填充会改变键中的命令代次。
- 根因：命令发现是进程级 `OnceLock`，没有测试注入点；Git 子进程只覆盖部分 `GIT_*` 护栏，没有隔离配置文件。
- 影响：测试结果依赖机器 PATH、外部 shell 与 git 全局配置；可能变慢或间歇红；无法证明测试只依赖夹具。抖动是否实际发生尚缺计时证据。
- 证据：`commands_handle` 的 `OnceLock` 加 `std::thread::spawn`，后台完成后才写入 Vec；`cache_key` 读取 `commands.lock().len()`；`powershell_commands` 用 `Command::new(program)` 探测；`git::command` 只追加固定环境变量；`cache_key` 用例先取基准再取第二次。
- 优化方向：为命令与 Git 来源提供可注入端口或隔离环境（受控 PATH、隔离 git 配置），等待后台稳定后再断言；把外部进程探测移出键计算路径。

### 一般｜合同冲突：Posix 与未知 Shell 方言在生产不可达

- 置信度：确认
- 位置：`ShellSyntax::for_program`、`ShellLineTracker::new` 的 `supported` 判定、`slterm_terminal/src/tty/windows/shell.rs` 的 `ALLOWED_SHELLS`/`validate_shell_allowlist`、`slterm_completions/CLAUDE.md`「裁剪已成定局」、`featureSpec/panels/terminal.md`「来源与方言」。
- 现象：补全引擎保留 Posix 与 Literal 方言，但终端只能创建并校验 `pwsh.exe`、`powershell.exe`、`cmd.exe`；`ShellLineTracker` 也只信任这三类。生产补全请求只来自可信行，因此 Posix 与 Literal 没有生产入口，只在测试里通过语法覆盖或 `syntax_override` 出现。
- 根因：方言保留决策与可执行/可信 Shell 白名单不一致，三处文档各自表述。
- 影响：featureSpec「原生 Posix Shell 分别解析」与 completions 模块文档的 Git Bash/MSYS 定位无法兑现；相关测试只证明核心分支，不证明产品能力。
- 证据：`ALLOWED_SHELLS` 仅三 shell；`Pty::new` 对非显式 Shell 调用 `validate_shell_allowlist`；`ShellLineTracker::new` 的 `supported` 同集；`request_at_revision` 的唯一生产调用点是 `refresh_suggestion`。
- 优化方向：裁决产品范围。若保留 Posix，需接入可信任的提示符/执行边界并补真实路径测试；否则删除方言与文档承诺，未知 Shell 同理。

### 提示｜核心保留第二套路径转义与未用的来源抽象

- 置信度：确认
- 位置：`slterm_completions/src/file.rs` 的 `escape_path`、`surround_remove`、`complete_item`，`directory.rs::DirectoryCompletion`，`static_completion.rs::StaticCompletion`，`completer.rs`；`slterm_app/src/completion/paths.rs`、`slterm_completions/CLAUDE.md`「来源合同」、`slterm_app/src/completion/CLAUDE.md`「路径转义归属」。
- 现象：`escape_path` 用反引号与单引号实现一套与方言无关的引用，与应用使用的 `command_context::CommandContext::candidate` 形成第二套转义；`DirectoryCompletion`、`StaticCompletion` 与 `Completer` 没有生产实现或注册；`complete_item` 的非 literal 分支应用未调用，测试只覆盖其取消。
- 根因：上游 nu-cli 资产保留，应用层另起语义编排。
- 影响：后续消费者可能绕过方言安全的引用与 home/n-dot 规则；死 API 与“来源合同/转义单点”文档漂移。
- 证据：`lib.rs` 再导出这些类型；应用无相关调用；`escape_path` 的 `` `{path}` `` 与单引号分支；`file_tests` 只覆盖 `complete_item` 取消。
- 优化方向：裁剪或私有化未用资产，保留唯一转义与来源通道；若保留 trait，让真实来源经它注册并补测试。

### 提示｜可执行命令与路径候选的拼写边界

- 置信度：确认
- 位置：`CommandContext::candidate` 的 safe 集合、`completion/paths.rs` 的 `make`、`suggest::path_commands`/`command_hints`、`file::complete_literal_with_cancel`。
- 现象：以 `-` 开头的路径候选会裸写进 PowerShell/git，被当作选项或参数而非路径；PATH 中带空格的可执行文件名会裸插入命令位置，未引用。
- 根因：`candidate` 不知道候选类型，safe 集合对 `-` 放行；命令来源没有 token 拼写层。
- 影响：补全结果与用户意图不同，甚至形成非法命令；仅影响罕见文件名，且无测试。
- 证据：`candidate` 的 safe 条件包含 `-`；路径来源未加 `./` 前缀；`command_name` 保留文件名中的空格；应用无对应用例。
- 优化方向：按候选类型（路径/命令）决定引用与 `./` 前缀；补以 `-` 开头和含空格名称的方言用例。

### 提示｜Git 补全在追溯矩阵中没有独立条目

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的补全相关行、`docs/devPlan/plan.md`「GitFacade 只读收编」、`slterm_app/src/git/completion.rs` 及其测试。
- 现象：矩阵只有补全可信边界、不可信镜像、cwd 变化等行为行；没有以 Git refs/remotes、只读 argv、双预算、禁网或单 argv 通道为对象的条目。对应验收只出现在计划行与测试内部。
- 根因：矩阵按行为切片登记，Git 来源的验收证据未落表。
- 影响：Git 补全的验收与回归证据不可从矩阵追踪，后续维护无法按矩阵核销。
- 证据：`traceability.md` 无 refs/remotes/GitFacade/argv 相关行；`plan.md` 的“覆盖取消、超时、输出上限、禁止网络与写命令、禁止另建 argv 通道”未映射到矩阵。
- 优化方向：补一条 Git 补全验收行，锚定固定 argv/禁网用例与真实临时仓库用例。

## 跨域观察

- 加密备份的 i18n 仍保留 `settings.backup.category_command_history` 与 `category_directory_history`（中英双语），但备份白名单不收集学习库且 featureSpec 永久排除命令/目录历史；这些键无生产消费者，未来若生成页面会给出无法实现的类别。
- `suggest::commands_handle` 的进程级命令发现线程没有所有者、join 或超时，兜底依赖进程退出；这与本仓“线程与生命周期必须有显式所有权与 shutdown 路径”的纪律存在张力，也会被补全测试触发（见上条测试隔离问题）。

## 存疑未证

- 命令来源使用 `std::env::var_os("PATH")` 的进程环境，而 `PaneExecContext` 冻结的是 pane 自己的合并环境；`suggest::calculate` 的注释称“复用 shell 继承的进程 PATH”，但 `SuggestSources` 没有环境入口。需要实测一个 pane 的 PATH 与应用进程 PATH 不同的场景，才能确认候选是否偏离该 Shell 的实际可执行集。
- `ShellLineTracker::observe_prompt` 在提示符代次变化时直接把帧回显设为 `Trusted` 行。若主屏中的第三方程序自行输出 OSC 133;A/B 与回显文本，可能种下一条未经本地按键的“可信”行；需要真实 emitter 或注入帧用例证明该路径是否可达。备用屏路径已失效，不在此怀疑内。
