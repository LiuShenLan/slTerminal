# GPUI 终端视图与终端会话 UI 审查报告

> 审查 `slterm_app/src/gpui_terminal` 的视图、按键、粘贴、OSC、搜索、恢复与会话端点，以及 `slterm_app/src/session.rs` 的终端相关持久面，基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支）；只读代码与文档，未运行 cargo、构建、测试或网络。

## 覆盖与局限

- 已读：`slterm_app/src/gpui_terminal/CLAUDE.md`；`gpui_terminal` 下的 `mod.rs`、`view.rs`、`session.rs`、`session_pump.rs`、`registry.rs`、`event_mailbox.rs`、`element.rs`、`keymap.rs` 及 `keymap/`、`view/`（layout、startup、restore、search、pointer、completion、cwd_report、broadcast、typography 与各测试文件）、`colors.rs`、`cursor_painter.rs`、`mouse_protocol.rs`、`copy_feedback.rs`、`shell_line.rs`；`slterm_app/src/session.rs`；`docs/devPlan/design/terminal.md`；`docs/featureSpec/panels/terminal.md`、`settings.md` 的相关章节与 `sidebar/framework.md` 的快捷键条目；`docs/devPlan/traceability.md` 的 Terminal 段；`slterm_app/CLAUDE.md`、`app_shell/CLAUDE.md`、`app_root/CLAUDE.md`、根 `CLAUDE.md`；`.claude/docs/testing/exemptions.md`、`doc-style.md`。
- 交叉核对：`slterm_terminal/src/event_loop/mod.rs` 的 `Msg` 与 `apply_display_command`/`publish_frame`、`render.rs` 的 `TerminalFrame`/`ViewportTracker`、`term/mod.rs` 的 `cursor_style`/`scroll_display`，以及 GPUI 的按键派发与 `ShapedLine` 实现，用于确认调用顺序而非凭命名推断。
- 未覆盖：未运行任何测试、虚拟窗口或真实 ConPTY；未在 Win10/Win11 实机、真实 GPU、真实 IME、系统剪贴板、真实鼠标协议（全屏 TUI）下验证；事件时序、帧合并竞态与容量竞态只能做代码推演。GPU/真实鼠标跟踪的自动化豁免见豁免表条目，未重复报告。
- `slterm_app/src/session.rs` 是旧会话 schema 与身份助手，模块自述生产 `AppRoot` 不读取；本次未发现它被终端视图作为第二恢复通道使用，未单列问题。

## 问题

### 阻断｜存活上限预检发生在真实 PTY 创建之后，拒绝路径在前台 join worker

- 置信度：确认（代码路径完整；触发条件见影响）
- 位置：`slterm_app/src/gpui_terminal/view/startup.rs` 的 `TerminalView::launch`、`prepare`、`bind_spawned`、`restore_launch`；`slterm_app/src/gpui_terminal/registry.rs` 的 `TerminalRegistry::launch`、`ensure_live_capacity`；`slterm_app/src/gpui_terminal/session.rs` 的 `TerminalSession`；`slterm_terminal/src/event_loop/mod.rs` 的 `EventLoopHandle::drop`；测试 `terminal_live_pty_limit`。
- 现象：`launch`/`restore_launch` 先在 `prepare` 内执行 `session::local_options` 与 `session::spawn_local`，真实 ConPTY 与子进程已经存在；随后 `bind_spawned` 才调用 `TerminalRegistry::launch`，由 `ensure_live_capacity` 检查存活额度。Registry 在额度不足或目标重复时不会执行传入闭包，闭包持有的 `TerminalSession` 被直接丢弃；`TerminalSession` 的 `_event_loop` 是 `EventLoopHandle`，其 `Drop` 会发送 `Shutdown` 后同步 `join`。
- 根因：容量闸门放在 Registry 的工厂之前，但真正的进程创建工厂在 Registry 之外；`restore_launch` 也不像 `launch` 那样在创建 PTY 前检查既有目标。启动副作用与注册副作用之间没有单一事务边界。
- 影响：违反 `gpui_terminal/CLAUDE.md` 的“额度不足不得执行工厂、消耗新代次或改变既有目标”，以及根 `CLAUDE.md` 的“GPUI 前台不得 join”。一旦 Registry 的 `process_live` 计数暂时高于核心存活计数（退出事件尚未被前台处理），或已有旧 worker 而无面板时重走 `restore_launch`，就会出现“真实进程被创建后立即杀掉、GPUI 线程在回收时阻塞”的组合；阻塞时长取决于 ConPTY 收尾与 shell 启动。`terminal_live_pty_limit` 使用内存会话工厂，只证明 Registry 闭包未执行，覆盖不到真实启动顺序。
- 证据：`TerminalView::launch` 的调用顺序是 `prepare`（含 `spawn_local`）→ `bind_spawned` → `registry.launch`；`TerminalRegistry::launch` 的 `ensure_live_capacity` 在 `factory()` 之前返回；`bind_spawned` 把 `session` 移入闭包；`EventLoopHandle::drop` 执行阻塞发送与 `join`。`restore_launch` 没有 `registry.target` 预检。
- 优化方向：把容量与目标预检前移到任何 PTY 创建之前，或让 Registry 成为唯一的进程创建入口；失败清理必须移交后台关闭；真实启动路径需要额度拒绝与重复目标用例，而不是内存工厂替身。

### 重要｜搜索的帧驱动重查会重置导航位置并形成搜索风暴

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::process_event` 的 `TermEvent::Frame` 分支；`slterm_app/src/gpui_terminal/view/search.rs` 的 `refresh_terminal_search`、`move_terminal_search`、`scroll_to_terminal_match`；`slterm_terminal/src/event_loop/mod.rs` 的 `apply_display_command` 与 `publish_frame`；测试 `terminal_search_is_read_only`、`integration_terminal_search`。
- 现象：只要搜索打开，每收到一帧就 `refresh_terminal_search`：先 `invalidate` 清空匹配与选中，再发送新的 `SearchLiteral`，回包后无条件 `selected = Some(0)` 并滚动到首个匹配。用户按 Enter 切到下一处时，`scroll_to_terminal_match` 发送的 `Msg::Scroll` 本身会让 worker 发布新帧（`Scroll` 返回真即置 `display_dirty`），新帧又触发重查，选中被拉回 0，于是下一处“跳过去又跳回来”。持续输出时，每一帧都取消在途搜索并重发，搜索结果在输出停止前无法稳定。
- 根因：把“新输出使旧位置失效”实现成“每帧立即重查并重新选中”，没有区分失效与重查，也没有去抖、代次合并或保留用户导航位置；搜索自身的滚动帧未从重查触发中排除。
- 影响：回滚中搜索的“上一处/下一处”对屏幕外匹配基本不可用；匹配数、高亮和当前位置随帧抖动；搜索命令与用户输入争抢有界 worker 命令队列；隐藏项目中的终端若残留搜索，也会在后台持续做全回滚搜索，拖慢解析。
- 证据：`process_event` 的 `Frame` 分支在 `adopt_frame` 后无条件调用 `refresh_terminal_search`；`refresh_terminal_search` 每次清空 `matches`/`selected`；`scroll_to_terminal_match` 通过 `Msg::Scroll` 改变显示偏移；核心 `apply_display_command` 对可改变显示的命令返回真，事件循环随后 `publish_frame`。现有搜索用例在测试里手动调用 `refresh_terminal_search`，`pump_test_worker` 走 `refresh_test_frame`/`adopt_frame`，不经过生产的 `process_event(Frame)` 路径。
- 优化方向：新帧只作废旧位置，重查由用户动作或去抖后的显式请求触发；重查与滚动不得重置用户已导航到的匹配；排除搜索自身滚动产生的帧；用真实帧事件驱动搜索用例。

### 重要｜退出后键盘复制、搜索与回滚全部失效

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::on_key_down`；`slterm_app/src/gpui_terminal/CLAUDE.md` 的“退出不等于关闭”；`docs/featureSpec/panels/terminal.md` 的退出语义；`slterm_app/src/app_shell/app_root/CLAUDE.md` 的“退出与关闭分离”。
- 现象：`on_key_down` 在 `exited.is_some()` 时直接返回，早于 `Ctrl+Shift+C` 复制、`Ctrl+Shift+F` 搜索、`Shift+PageUp/PageDown/Home/End` 本地回滚等只读分支。退出后鼠标选择、滚轮和右键复制仍可用，但默认的键盘复制与键盘回滚不可用；若搜索在退出前已打开，导航键同样被整体拦截。
- 根因：把所有按键统一按“进程是否存活”拦截，而退出只应拒绝写入 PTY 的输入，不应拒绝显示与只读命令。
- 影响：用户退出 shell 后无法用文档默认的复制快捷键复制最后的输出，也无法用键盘搜索或翻看保留的回滚，与“显示 worker、事件订阅与最后画面继续服务滚动和选区”的约束不一致。
- 证据：退出判断位于 `on_key_down` 开头；复制、搜索、回滚分支都在其后；`mark_exited` 保留 `frame`、`binding` 与显示命令能力；`slterm_terminal` 的 Registry 仍允许非 `Input` 的显示命令。现有退出用例只验证运行状态与输入拒绝，未验证退出后的复制/搜索/滚动。
- 优化方向：退出只拦截会编码写 PTY 的分支，保留复制、搜索、滚动与选区快捷键；补充退出后的键盘只读用例。

### 重要｜一次瞬态操作错误会替换并永久遮住终端画面

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::paste`、`TerminalView::render` 的 `error` 分支；`slterm_app/src/gpui_terminal/view/restore.rs` 的 `inject_agent_resume`；`slterm_app/src/gpui_terminal/view/startup.rs` 的 `install_reconnect`。
- 现象：粘贴内容超过上限时写入 `self.error`；`render` 只要 `error` 有值就只渲染错误文本，不再渲染 `TerminalElement`、搜索浮层和底部状态。Agent 恢复注入发送失败时也写同一个 `error`。生产路径没有清除该字段的入口，只有测试安装 worker 或重连成功时才置空，因此一次超大粘贴或一次瞬时发送失败会让终端内容从界面消失直到重连。
- 根因：启动/重连的致命错误与单次操作被拒绝的瞬态提示共用同一个状态，错误渲染分支与终端渲染互斥。
- 影响：用户无法继续查看或复制已有输出，也无法关闭错误提示；退出码、搜索和补全弹层一并被遮住，只能关闭终端。该行为与 `docs/featureSpec/panels/terminal.md` 的“错误可见”与“保留当前内存画面”方向冲突。
- 证据：`paste` 的超限分支直接 `self.error = Some(...)`；`render` 的 `if let Some(error) = &self.error` 分支用纯文本覆盖内容区；生产代码中 `self.error = None` 只出现在重连成功与测试安装 worker；`inject_agent_resume` 的发送失败分支写同一字段。
- 优化方向：区分持久启动错误与瞬态操作提示；瞬态提示不得替换终端元素，且必须可过期或可关闭；错误覆盖层需要显式的重试/关闭入口；补充超限粘贴后终端仍可见、仍可复制的用例。

### 重要｜`Ctrl+Enter` 未发送 LF，行镜像把修饰 Enter 当作提交

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/keymap.rs` 的 `encode`、`encode_for_program`、`is_shift_enter`、`preserves_enter_modifiers`；`slterm_app/src/gpui_terminal/view/completion.rs` 的 `note_text_key`、`commit_line`；`docs/featureSpec/panels/terminal.md` 的快捷键条目。
- 现象：行为真值要求 `Ctrl+Enter` 发送 LF。传统 VT 路径的 `encode` 只对裸 `Shift+Enter` 返回 `\n`，`Ctrl+Enter` 落到 `\r`；增强键盘模式下走 Win32 记录或 CSI-u，也不是 LF。与此同时 `note_text_key` 对所有 `enter`（不区分修饰键）调用 `commit_line`，把 `Shift+Enter`/`Ctrl+Enter` 当作命令行提交；实际写入 PTY 的字节却可能不是提交语义，行镜像与 shell 事实分叉。
- 根因：LF 条件以 Shift 而非 Ctrl 判定；编码器与行镜像各自解释 Enter，且行镜像不检查修饰键。
- 影响：依赖 `Ctrl+Enter` 在 AI CLI 中插入换行的用户得到错误的 CR 或协议记录；修饰 Enter 之后 ShellLineTracker 进入待决执行，可能阻塞补全或与真实命令边界不一致。
- 证据：`encode` 的 `"enter"` 分支对非 Shift、非 Alt 返回 `\r`；`is_shift_enter` 显式排除 Control；`note_text_key` 的 `"enter"` 分支无条件调用 `commit_line`；现有测试覆盖 Shift+Enter 与 `preserves_enter_modifiers`，没有钉住 Ctrl+Enter 的传统 VT 字节。
- 优化方向：按行为真值统一 Enter 修饰键编码；行镜像只在真实提交语义上建立待决执行；补修饰 Enter 在不同键盘模式下的字节级用例。

### 重要｜键盘选区扩展与全选未实现，`Shift+PageUp/PageDown/Home/End` 语义与行为真值相反

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::on_key_down`；`slterm_app/src/gpui_terminal/keymap.rs` 的 `encode`；`docs/featureSpec/panels/terminal.md` 的输入与选区条目；测试 `shift_pageup_scrolls_main_screen_but_reaches_alt_screen_encoded`。
- 现象：行为真值要求 `Shift+方向键`、`Shift+PageUp/PageDown`、`Shift+Home/End` 扩展选区，并要求备用屏全选与扩选仅限当前画面。实现没有 `SelectAll` 或键盘扩选路径；`Shift+方向键` 落入编码器发送带修饰的方向键序列；`Shift+PageUp/PageDown/Home/End` 被解释为本地滚动（到顶/到底/翻页），备用屏则不拦截。现有用例把 `Shift+PageUp` 的滚动语义当作正确行为钉住。
- 根因：键盘层只接入鼠标选区，没有把行为真值的选区命令接入 `SelectionCommand`；同一组合在键位上被滚动语义占用。
- 影响：用户无法用键盘全选或扩展选区，复制长输出只能依赖鼠标；行为真值与实现及测试三方不一致。
- 证据：`on_key_down` 的 Shift 分支只发送 `Msg::Scroll`；没有 `SelectionCommand::Set/Update` 的键盘调用；`keymap::encode_for_program` 会为 Shift+方向键生成 CSI 序列；`docs/featureSpec/panels/terminal.md` 明确写的是扩展选区。
- 优化方向：实现键盘全选与扩选命令并决定与滚动的优先级；备用屏按当前画面限制；用真实按键路径断言选区与零 PTY 写入。

### 重要｜合同冲突：`Ctrl+Shift+A` 同时被终端全选与侧栏 Agent 文件占用

- 置信度：确认
- 位置：`docs/featureSpec/panels/terminal.md` 的快捷键条目；`docs/featureSpec/sidebar/framework.md` 的 SideView 默认快捷键；`slterm_app/src/app_shell/app_root/command_registry.rs` 的 `SIDEBAR_DEFINITIONS` 与 `ActivateAgentFiles`；`slterm_app/src/gpui_terminal/view.rs` 的 `on_key_down`。
- 现象：终端行为真值要求 `Ctrl+Shift+A` 全选；侧栏行为真值要求同一组合打开 Agent 全局文件；实现把该组合登记为无上下文的全局命令并绑定 `ActivateAgentFiles`。按 GPUI 的派发顺序，keymap 动作先于按键监听执行，动作在冒泡阶段默认停止传播，因此终端聚焦时实际发生的是侧栏切换，终端既没有执行全选，也不会收到该键的编码。终端的放行名单对已绑定组合不可达，不能改变这个结果。
- 根因：两份行为真值对同一组合给出不同归属，命令目录与终端键位没有裁决点。
- 影响：合同冲突本身使“全选”或“侧栏快捷键”至少一项无法按各自文档成立；终端全选能力完全缺失。若按终端真值实现，必须同时修改侧栏真值与命令目录；若保留侧栏，则终端真值需更新。
- 证据：两份 featureSpec 原文指向同一组合；`CommandRegistry::register_bindings` 为 `ActivateAgentFiles` 注册无上下文键位；`TerminalView` 没有全选实现；GPUI 的 `dispatch_key_event` 先派发绑定动作，`dispatch_action_on_node_inner` 的冒泡阶段默认把 `propagate_event` 置假，因此按键监听不会运行。
- 优化方向：由产品真值裁决键位归属并同步两份文档与命令目录；无论归属如何，终端侧需要真实的选区全选能力。

### 重要｜光标形状、闪烁与相关设置不符合行为真值

- 置信度：确认（设置未接线）；有效闪烁状态受子进程模式覆盖，需实机确认
- 位置：`slterm_app/src/app_shell/config.rs` 的 `Settings::term_config`；`slterm_terminal/src/term/mod.rs` 的 `Config::default` 与 `Term::cursor_style`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `cursor_shape`、`cursor_blink`、`cursor_motion` 控件；`slterm_app/src/gpui_terminal/view.rs` 的 `cursor_should_blink`；`docs/featureSpec/panels/terminal.md` 与 `settings.md` 的光标条目。
- 现象：行为真值要求光标形状固定竖线、闪烁默认开启（系统减少动效时默认关闭，用户可覆盖）。`term_config` 不读取 `cursor_shape`/`cursor_blink`/`cursor_motion`，`Config::default` 的形状来自底层默认的块状光标，闪烁来自引擎默认或子进程的 DECSET 覆盖；视图只读 `frame.cursor_style()`，没有任何设置映射；代码中没有减少动效检测。设置页却提供了形状选择控件，与“固定竖线”的合同相反。
- 根因：光标设置键没有进入 `term::Config` 的映射，也没有热应用入口；默认值沿用引擎默认而非行为真值。
- 影响：用户配置的光标形状、闪烁与平滑移动不生效；默认形状与文档不一致；闪烁行为实际取决于子进程是否发送模式序列，跨机器不一致。
- 证据：`Settings::term_config` 只写回滚、ConPTY 与 kitty 位；`cursor_shape`/`cursor_blink`/`cursor_motion` 只出现在设置页与设置库；视图的闪烁判定读帧内 `CursorStyle`；`Term::cursor_style` 在无覆盖时返回 `config.default_cursor_style`。
- 优化方向：把光标设置映射到引擎配置并设置竖线与闪烁策略；接入系统减少动效；删除或裁决形状选择控件；用不同模式下的光标用例验证。

### 重要｜主题、字体、字号、连字与复制/补全外观设置不热应用

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::apply_settings` 与 `palette`/`font`/`font_size`/`copy_on_select`/`completion_style`/`ghost` 字段；`slterm_app/src/gpui_terminal/view/startup.rs` 的 `from_prepared` 与 `SetColorScheme`；`slterm_app/src/gpui_terminal/element.rs` 的绘制取色；`slterm_app/src/gpui_terminal/view.rs` 的 `ColorRequest` 应答；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `reload_settings_snapshot`。
- 现象：设置页修改主题、字体族、字号、连字、选中复制、补全形态或 ghost 后，`reload_settings_snapshot` 只替换全局 `Settings`、主题与语言并刷新窗口；既有 `TerminalView` 的字体、字号、调色板、复制语义与补全状态是构造期快照，`apply_settings` 没有生产调用者。终端绘制、OSC 4/10/11 颜色应答与 DECSET 2031 亮暗通知继续使用旧调色板。新建终端使用新设置，已有终端保持旧外观，同一窗口出现两套外观。
- 根因：缺少 AppRoot 到全部终端视图的设置扇出；视图把可热变的外观保存在本地字段；`apply_settings` 作为唯一的更新入口被留在未接线状态。
- 影响：违反 `docs/featureSpec/panels/settings.md` 的“主题切换立即重绘全部视图”和 `docs/featureSpec/panels/terminal.md` 的“字号变化只调整网格和 PTY 尺寸，不重启进程”；用户需要重开终端才能看到主题或字号变化，OSC 颜色查询回答也与当前主题不一致。
- 证据：`apply_settings` 带未接线标记且全仓无调用者；`render` 与元素绘制读取 `self.palette`/`self.font`/`self.font_size`；`ColorRequest` 用 `self.palette`；`SetColorScheme` 只在构造时发送一次；`reload_settings_snapshot` 不遍历终端视图。
- 优化方向：由 AppRoot 在设置快照提交后向全部终端视图扇出一次更新，覆盖调色板、字体度量、复制语义与补全形态，并同步发送配色/尺寸命令；用主题与字号热应用用例验证。

### 一般｜字号默认值、允许范围与校验不符合行为真值

- 置信度：确认
- 位置：`slterm_app/src/app_shell/config.rs` 的 `DEFAULT_FONT_SIZE_PX`；`slterm_settings/src/lib.rs` 的 `font_size_px` 解析与测试；`slterm_settings/src/keys.rs` 的 `font_size` 键校验；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `font_size` 步进控件；`slterm_app/src/gpui_terminal/view/typography.rs` 的默认回退；`docs/featureSpec/panels/terminal.md` 与 `settings.md`。
- 现象：行为真值规定终端字号默认 14、范围 8–32 的整数；实现默认 15，设置库把值夹到 4–96 且接受小数（测试明确接受十六点三），设置页步进控件范围也是 4–96。字体不可用时只追加内嵌等宽字体兜底，没有向用户提示。
- 根因：设置键定义、应用默认值与设置页控件各自实现，没有统一到行为真值的数值合同。
- 影响：默认外观与合同不符；越界或非整数字号可以被持久化并生效，之后重启或换版本可能被再次夹紧；字体缺失时用户不知道发生了回退。
- 证据：默认常量为 15；`font_size_px` 使用 `clamp(4.0, 96.0)`；键校验为通用有限数字；设置页步进为 4–96；`mono_font` 只追加兜底字族，没有可用性检查或提示。
- 优化方向：在设置注册表与设置页统一执行 8–32 整数校验，默认 14；字体不可用时给出明确回退提示；补非法值零提交的用例。

### 一般｜恢复的自定义标题永久锁定视图标题，清除后无法回到 CLI 标题

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/startup.rs` 的 `prepare` 与 `title_locked` 初始化；`slterm_app/src/gpui_terminal/view.rs` 的 `TermEvent::Title`/`ResetTitle` 分支；`slterm_app/src/app_shell/app_root/tabs.rs` 的 `tab_title`；`slterm_app/src/app_shell/app_root/commands.rs` 的 `rename_terminal`。
- 现象：恢复或懒启动带自定义标题的终端时，`shell_name` 取自定义标题，`title_locked` 置真，视图标题被冻结在构造值，CLI/OSC 标题更新被忽略。之后用户清除自定义标题，标签按 `tab_title` 回退到 `view.title`，看到的是旧的自定义标题而不是当前 CLI 标题；“CLI 运行中不可重命名”的守卫也不在重命名路径。
- 根因：锁定状态只在构造时由“是否有自定义标题”一次性决定，没有随快照中的自定义标题变化解锁；运行期重命名不通知视图。
- 影响：清除自定义标题后标题状态错误；同一逻辑终端在重启前后可能显示不同来源的标题。现有用例只覆盖 Runtime 重命名的快照值，没有覆盖清除后的标签回退。
- 证据：`prepare` 中 `title_locked = shell_name.is_some()`；`Title` 与 `ResetTitle` 都以 `!title_locked` 为前提；`tab_title` 在无自定义标题时读视图标题；`install_reconnect` 不重置标题锁定；没有解锁入口。
- 优化方向：以快照中的当前自定义标题决定是否锁定，重命名时同步视图；补充恢复带标题再清除、以及 CLI 运行中重命名的用例。

### 一般｜退出与错误状态缺少“重新连接／关闭面板”的可见入口

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::render` 的 `bottom_stack`、`TerminalView::mark_exited`；`docs/featureSpec/panels/terminal.md` 的退出条目；`slterm_app/src/app_shell/app_root/command_registry.rs` 的 `ReconnectTerminal`/`CloseActiveTerminal`。
- 现象：进程退出后只在底部浮层显示一条文本，没有“重新连接”或“关闭面板”按钮；启动/重连失败的错误面板同样没有重试或关闭入口。命令目录里虽有对应动作与快捷键，但界面状态本身不呈现操作。
- 根因：退出与错误状态只驱动文本浮层，没有把命令目录中的动作接入该浮层。
- 影响：行为真值要求退出后显示退出码和“重新连接／关闭面板”，用户只能记忆快捷键或寻找其他入口；错误恢复路径对普通用户不可发现。
- 证据：`render` 只追加 `exited` 文本；`mark_exited` 只写消息；命令动作存在但视图没有为浮层绑定动作；无对应虚拟窗口用例。
- 优化方向：退出/错误浮层提供与命令目录同源的重连、重试与关闭动作；补点击路径的用例。

### 一般｜复制成功反馈没有绘制消费端，测试只断言内部状态

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/copy_feedback.rs` 的 `CopyFeedback`；`slterm_app/src/gpui_terminal/view.rs` 的 `copy_selection` 与 `render`；测试 `ctrl_shift_c_copies_selection_and_feedback_expires_on_controlled_clock`。
- 现象：复制后 `mark_copied` 维护“已复制”状态与过期任务，但视图的 `render` 从未读取该状态，也没有任何对勾或提示元素；`is_copied` 是仅测试消费的接口。用例只断言内部布尔值与时钟推进，不验证用户可见反馈。
- 根因：复制反馈的展示接线未完成，状态机却已按合同存在。
- 影响：复制成功没有可感知反馈；用例制造了“反馈已覆盖”的错觉，后续若删除或改写展示路径不会被测试发现。
- 证据：全仓 `copy_feedback` 的消费者只有字段定义、`copy_selection` 与测试；`render` 不引用它；`is_copied` 带未使用标记。
- 优化方向：接入基于语义 token 的就地反馈；若产品决定不需要成功反馈，删除状态机与用例，避免伪覆盖。

### 一般｜设置页暴露多个终端键但无任何消费者

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `scrollback_lines`、`font_family_cjk`、`ctrl_wheel_font_zoom`、`dim_inactive_panes`；`slterm_app/src/app_shell/config.rs` 的 `Settings` 与 `term_config`；`slterm_app/src/gpui_terminal/view/pointer.rs` 的 `on_scroll`；`slterm_app/src/gpui_terminal/view/typography.rs` 的 `mono_font`；`slterm_settings/src/lib.rs` 的 `RuntimeSettings`。
- 现象：`scrollback_lines` 有校验与设置控件，但 `term_config` 固定 10,000，新建终端也不读取该值；`font_family_cjk` 只出现在设置页与设置库，等宽字体构造不消费；`ctrl_wheel_font_zoom` 有开关，但终端对 Ctrl+滚轮一律吞掉且不缩放；`dim_inactive_panes` 有开关，应用设置却只从自定义主题读 `veil` 并标记为未消费，非聚焦面板不压暗。
- 根因：设置键先于终端消费者落地，缺少“键到行为”的映射与门禁；`Settings` 快照没有包含这些字段。
- 影响：用户修改这些设置没有任何效果，设置页呈现与运行行为不一致；回滚行数的可配置合同不成立，非聚焦面板的视觉区分缺失。
- 证据：上述键在应用层只出现于设置控件或设置库；`term_config` 使用常量；`on_scroll` 的 Ctrl 分支无条件 `stop_propagation` 后返回；`mono_font` 只使用主字族与内嵌兜底；`Settings.veil` 带未接线注释且不映射 `dim_inactive_panes`。
- 优化方向：逐键接入真实消费者或在设置页移除；`scrollback_lines` 至少作用到新建终端；补设置值到行为的端到端用例。

### 一般｜结构性 resize 的置位点不存在，分屏/缩放全部落入尾沿去抖

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/layout.rs` 的 `mark_structural_resize`；`slterm_app/src/gpui_terminal/view.rs` 的 `structural_resize` 字段注释与 `set_layout`；`slterm_app/src/gpui_terminal/view/startup.rs` 的 `install_reconnect`。
- 现象：`mark_structural_resize` 带“无生产者”注释，全仓没有调用者；分屏创建/关闭、tab 新建、zoom 等一次性结构变化没有置位，`set_layout` 对它们同样走 150 毫秒尾沿去抖。字段注释明确说明这类变化必须同步下发，否则子进程在旧宽度下的输出会被解析进新宽度网格，造成提示符与回显错行；该保护实际未生效。
- 根因：结构性布局变更的宿主入口未接入视图；策略只存在于字段注释与未调用的方法。
- 影响：持续输出时改变分屏或缩放，对子进程的 resize 延迟一个去抖窗口，期间新旧几何混用；代码自述的累积漂移风险没有被规避。
- 证据：`mark_structural_resize` 只有定义；`set_layout` 在非结构性路径统一写 `pending_resize` 并调度 settle；布局命令、分屏与 zoom 的宿主路径没有调用。
- 优化方向：在分屏/合并/关闭/tab 结构变化与字体度量变化点置位；或用结构事件直接提交视口；补结构变化时的 resize 顺序用例。

### 一般｜退出横幅硬编码灰色，违反配色单点

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `TerminalView::render` 退出消息浮层的背景。
- 现象：退出横幅背景使用 GPUI 的固定灰色辅助函数，不是主题/调色板语义 token；同文件其他浮层（搜索、补全）都使用 `palette`。根 `CLAUDE.md` 规定颜色定义于主题/设置体系、组件经语义 token 引用、禁硬编码颜色。
- 根因：浮层背景绕过了调色板。
- 影响：主题或语义调整不会作用到退出横幅；同一终端内出现两套颜色来源。该处是当前终端视图唯一的颜色字面量来源。
- 证据：全仓该辅助函数只在此处使用；其余浮层使用 `palette.background`/`foreground`/`dim_foreground`/`selection`。
- 优化方向：改为调色板中的语义 token（如叠加表面色），并在主题体系里补齐该语义。

### 一般｜终端会话 UI 文案硬编码中文，退出码以 Debug 形态展示

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view.rs` 的 `paste` 确认对话框、超限错误、退出消息、搜索浮层、`completion_kind_tag`；`slterm_app/src/gpui_terminal/registry.rs` 的 `RegistryError::Display`（其文本经 `reconnect_failed`/`error` 进入界面）；`slterm_app/i18n/` 目录。
- 现象：搜索占位与计数、补全来源标签、粘贴确认按钮与说明、退出/故障文案、错误文案均直接写中文字符串，没有使用构建期 `Message` 键；i18n 目录中没有这些字符串。退出消息用 `format!("...({code:?})")` 插入 `std::process::ExitStatus` 的调试形态，用户看到的是带包装的调试串而不是退出码。
- 根因：终端视图绕过了应用的 i18n 通道；退出状态直接复用调试格式化。
- 影响：界面语言切到英文后这些终端文案仍为中文；退出码展示不符合“显示退出码”的行为真值，也不可翻译。
- 证据：`slterm_app/i18n` 两个目录均无“确认粘贴”“进程已退出”等键；组件全仓只在视图里出现这些字面量；`Event::ChildExit` 携带 `std::process::ExitStatus`，视图用 Debug 格式化。
- 优化方向：把这些文案纳入 typed Message 键；退出码按数值格式化，未知码单独给封闭文案；补语言切换后的终端文案用例。

### 一般｜关键交互缺少生产路径测试，现有用例掩盖缺陷

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/view/key_tests.rs` 的 `terminal_search_is_read_only`、`integration_terminal_search`；`slterm_app/src/gpui_terminal/view/visibility_tests.rs` 的 `terminal_hidden_output_backpressure`；`slterm_app/src/gpui_terminal/registry.rs` 的 `terminal_live_pty_limit`；`slterm_app/src/gpui_terminal/view.rs` 的测试辅助 `feed_test_bytes`、`pump_test_worker`、`refresh_test_frame`。
- 现象：搜索用例手动调用 `refresh_terminal_search`，帧刷新走 `adopt_frame`，不经过生产的 `process_event(Frame)`，因此帧驱动重查与导航重置不会被发现；粘贴超限后终端仍可见、外部主题/字号热应用、退出后复制/搜索/滚动、恢复自定义标题清除后的标签回退、重连 cwd 回退后的视图目录、应用真实启动顺序下的额度拒绝都没有用例。`terminal_live_pty_limit` 只用内存工厂证明 Registry 闭包未执行，不能证明真实 PTY 未创建。
- 根因：测试夹具绕开了生产事件入口与真实启动链；部分用例钉住了与行为真值不一致的实现。
- 影响：本报告中的搜索、退出、错误状态、设置热应用与标题问题都没有测试兜底；测试全绿不能证明对应合同成立。
- 证据：上述测试的驱动方式均为手动 refresh/pump 或内存会话；`process_event` 未被搜索用例调用；`feed_test_bytes` 每次新建解析器状态；没有退出后只读路径与热应用用例。
- 优化方向：以真实 `process_event`/事件泵和真实启动入口驱动关键路径；测试辅助层保留跨调用解析状态；搜索、退出、错误与热应用各补一条可失败的端到端用例。

### 提示｜重连的 cwd 回退不更新视图与逻辑记录的 cwd

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commands.rs` 的 `ReconnectLaunch`、`reconnect_terminal`、`LocalTerminalLauncher::reconnect`；`slterm_app/src/gpui_terminal/view/startup.rs` 的 `install_reconnect`；`slterm_app/src/gpui_terminal/session.rs` 的 `LocalSpawn`。
- 现象：`spawn_local` 在真实目录错误后回退项目根并返回实际 `cwd` 与 `cwd_fallback`，但重连适配层只把 `cwd_fallback` 与 `exec_context` 带出，丢弃实际目录；`install_reconnect` 不更新 `self.cwd`，也不更新快照中的终端记录。视图在 shell 下一次上报目录前继续用失效路径做补全请求与目录记录；持久记录在每次重启/重连时都会再次触发回退提示。
- 根因：恢复启动路径消费了回退后的实际 cwd，重连路径没有对齐。
- 影响：重连后的补全与目录事实短暂使用失效路径；反复提示同一回退；与“回退目标项目根并提示”的意图只完成了一半。
- 证据：`ReconnectLaunch` 只有 `session`、`events`、`exec_context`、`cwd_fallback`；`LocalSpawn` 有实际 `cwd`；`install_reconnect` 只清/设固定字段，不触碰 `cwd`；`reconnect_terminal` 不提交记录 cwd 候选。
- 优化方向：重连同样使用实际回退目录更新视图，并明确是否把回退根写回记录；补 cwd 失效重连后的补全与提示用例。

### 提示｜若干状态只有生产写入或测试读取，没有真实消费者

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/event_mailbox.rs` 的 `NativePromptState`；`slterm_app/src/gpui_terminal/view/broadcast.rs` 的 `TerminalInput`；`slterm_app/src/app_shell/app_root/terminal_events.rs` 的 `on_terminal_event`。
- 现象：`NativePromptState` 在输入与提示符事件路径持续更新，并有完整的 `preserves_native_prompt` 解析测试，但 `snapshot()` 没有生产消费者；`TerminalViewEvent::UserInput` 携带的按键/文本载荷在宿主侧被直接忽略，广播输入没有接收端。
- 根因：相关能力在宿主侧尚未接线，保留了一半状态机。
- 影响：维护者会误判原生提示符对账与输入广播已生效；后续接线时真实语义可能已被测试固化。
- 证据：`snapshot()` 只被测试调用；宿主对 `UserInput` 与 `CommandSubmitted` 都是空分支；`broadcast` 模块自述接收方向未落地。
- 优化方向：接入真实消费者或删除未用状态与事件载荷，避免半成品协议成为事实。

### 提示｜视图注释仍以过程编号与内部条目号引用未实现能力

- 置信度：确认
- 位置：`slterm_app/src/gpui_terminal/element.rs` 的模块注释与 `TermLayout` 字段注释；`slterm_app/src/gpui_terminal/view.rs` 的 `process_event`、`on_key_down`、`copy_selection`、`drop_path_text` 等注释；`slterm_app/src/gpui_terminal/view/pointer.rs` 的模块/`on_scroll` 注释；`copy_feedback.rs` 的模块注释；`keymap.rs` 与 `keymap/sequence.rs` 的注释；`view/key_tests.rs` 与 `view/pointer/tests.rs` 的注释。
- 现象：这些注释用两位阶段号、文档分节号、内部规则号与“改造节/定位约束/缝合点/交界/不可变更项”等条目名来标注“后续模块再实现”的能力（连字合并、CJK 字体族、内联图片、链接悬停、Ctrl+滚轮缩放、复制反馈合同等）。根 `CLAUDE.md` 与 `doc-style.md` 禁止阶段号、批次/条目号、规则号与文档编号进入注释；现有文档策略脚本不识别这些中文形态，门禁不会报错。
- 根因：迁移期注释保留了过程性编号，脚本正则只覆盖部分英文前缀。
- 影响：阶段结束后编号失去语义，注释与代码持续腐化；文档门禁给出“全绿”的错误安全感。注释本身也无法说明当前有效约定。
- 证据：上述文件的注释直接以编号引用后续工作；`check_doc_policy.py` 的规则集不包含这些中文形态或裸两位数编号。
- 优化方向：把注释改写为行为事实或指向当前模块文档的符号，不保留过程编号；扩展门禁或至少在审阅时人工检查。

## 跨域观察

- “CLI 运行中不可重命名”的守卫不在 `slterm_workspace` 的 `ProjectSnapshot::with_terminal_custom_title` 与 `AppRoot::rename_terminal` 中，Runtime 的 `terminal.rename` 在 CLI 运行期间仍可改标题；该规则跨工作区与 Runtime 域，需对应所有者补齐。
- `slterm_settings` 的 `font_size` 校验只做有限数字与夹紧，`ui_font_size` 有独立夹紧；与终端行为真值的整数/范围合同分属设置域，建议由设置所有者统一裁决。
- 终端 UI 的字符串没有进入 i18n 键集，属于 `slterm_app` 的 i18n 合同，不只是终端视图问题；同一批文案也出现在 `RegistryError::Display` 等共享错误面。
- `slterm_terminal` 的 `Config::default_cursor_style` 注释明确说它应由“运行时设置页”驱动，但应用层没有映射；该接口的所有权横跨 core 与 application，需要共同裁决默认值与设置入口。

## 存疑未证

- 帧唤醒与最新帧存储的丢失竞态：`EventReceiver::try_recv`/`poll_next` 先 `latest_event` 取走帧再 `acknowledge` 清唤醒位；如果 worker 恰好在这两步之间写入新帧，唤醒位仍为真而帧已被抑制，最新帧会留在 `FrameState` 中直到下一次输出或语义事件。代码顺序可证，但丢失最后一次重绘需要并发时序与真实输出复现；现有帧合并用例是单线程。缺少确定性复现或插桩证据。
- 存活上限预检的触发频率：需要运行时证明 Registry 的 `process_live` 计数会在核心释放名额后、前台处理退出事件前短暂偏高，或用真实多终端场景复现“上限拒绝后前台卡顿”。代码路径已确认，触发窗口依赖事件顺序。
- 光标闪烁的实际默认：ConPTY shell 在启动时可能发送 DECSET 12，使 `cursor_blinking_override` 为真；因此“默认不闪烁”的结论只在引擎默认层面成立，真实首屏是否闪烁需要实机观察。设置未接线与形状默认块状不受此影响。
- 真实 IME、系统剪贴板与全屏 TUI 鼠标：按豁免表依赖构建产物人工验收；本次未运行，无法确认 OSC 52 跨进程写入、Ctrl+单击打开链接与鼠标协议在真实桌面下的端到端表现。
- 已读路径之外是否还有第二条终端视图/终端 UI 实现形成双源：本次只按指定范围阅读 `gpui_terminal` 与 `app_root` 接线，未全域搜索所有终端渲染入口；如需证明单源，还需跨 crate 的类型级约束或门禁。
