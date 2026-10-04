# pebrel 重构优化 spec · 分片 02:终端核心

baseline commit:`e537d528c508e8607d0f5f9fd25e5902f40d661e`(pebrel,Rust + GPUI 模块化单体)

本分片对象:`nebula_terminal` → `slterm_terminal`(已定全局决策:改名保留、纯领域 core、不依赖 GPUI/视图)。pebrel 侧位置一律以 `nebula_terminal/` 为根省略 crate 名。

## 优化面

本分片覆盖十件事:

1. **VT 处理**——`Term` 模型与 `vte::ansi::Handler` 全实现、TermMode 模式集合(含 kitty keyboard 五位)、VT100 字符集、窗口标题栈;解析器「crates.io vte 0.15 不 fork」裁定落地。
2. **OSC tee 嗅探**——vte 不解码的 OSC 7/9;9(cwd)、133;A/B/C/D(语义 prompt)、1337(内联图片/UserVar)、777(远端 hook)经裸字节流 tee 状态机提取;与 slTerminal 现有 OSC 处理(后端透传 + 前端 parser hook)对照后的承接裁定。
3. **Grid/回滚**——Alacritty 式环形缓冲 Storage、Row 占用计数、resize/reflow(含 ConPTY 行锚定)、行级 damage、同步重绘 redraw_anchor。
4. **渲染合同**——Term 模型 → viewport 协议 + 单调 revision 快照 → 前端三层合同;cell grid 唯一表示;像素尺寸恒等于 columns×cell_width。这是替换 xterm.js 的核心接口。
5. **boxdraw 内建几何**——框线/块元素/Powerline 纯几何指令与 CJK 字体坑的根治理由。
6. **ConPTY 层**——conpty.dll+OpenConsole.exe 侧载优先与系统 API 回退、可中断读写、子进程退出监视、注册表环境重建、cmd PROMPT 注入;与 slTerminal `src-tauri/src/pty` 现状(自定义 flags 矩阵、Win10 捆绑、DA1/DSR 接管)逐点对照合并。
7. **输入链路**——event_loop(polling、1MB 读缓冲、resize 合并与流边界排空、ConPTY resize 后 120ms 光标对账)、keyboard(kitty 协议 ↔ TermMode)、win32_input_matrix.ps1 基线回归。
8. **选择/复制/搜索**——四态选择、宽字符整字语义、正则搜索、vi mode;与 slTerminal「Ctrl+Shift+C 复制 / Ctrl+C 中断」定位的承接。
9. **其他**——OSC 8 hyperlink、FairMutex、语义 prompt、pty_trace 启动 profiling、DEC 2026 同步更新、Event 枚举。
10. **slTerminal 侧现状对照**——`src-tauri/src/pty/` 与 `src/panels/terminal/` 在新世界的归宿:哪些资产迁入 `slterm_terminal`、哪些上移 GPUI 壳、哪些随 Tauri/xterm.js 消亡。

## 采纳点

### VT 处理与 Term 模型

1. **照抄:crates.io vte 0.15,不 fork**(`Cargo.toml` 的 `vte` 依赖;`src/osc_cwd.rs` 文件头注释)。vte 不解码的 OSC 全部由 tee 嗅探补齐,解析器零改动、零维护 fork;DEC 2026 同步更新由 vte 原生 `sync_timeout` 承担。slTerminal 同样零解析器自研(xterm.js 内置),此裁定直接延续。
2. **照抄:`Term` 与 `Handler` 全实现**(`src/term/mod.rs` 的 `Term`、`impl Handler for Term`)。方法面:光标 `goto`/`goto_line`/`goto_col`/`move_up/down/forward/backward`、`insert_blank`、`erase_chars`/`delete_chars`/`delete_lines`/`insert_blank_lines`、`scroll_up/down`、`clear_line`/`clear_screen`/`clear_tabs`、`set_scrolling_region`、`save/restore_cursor_position`、`device_status`、`identify_terminal`、`terminal_attribute`(SGR 全语义含 INVERSE 交换)、`set_mode`/`unset_mode`/`report_mode`、`set_private_mode`/`unset_private_mode`/`report_private_mode`、`configure_charset`/`set_active_charset`、`set_cursor_style`/`set_cursor_shape`、`set_title`/`push_title`/`pop_title`、`set_hyperlink`、`clipboard_store`/`clipboard_load`、`set_color`/`dynamic_color_sequence`/`reset_color`(OSC 4/10/11/12…)、`text_area_size_pixels`/`text_area_size_chars`(CSI 14t/18t)、kitty `push_keyboard_mode`/`pop_keyboard_modes`/`report_keyboard_mode`。经长期 claude/codex 全屏 TUI 实战检验的完备 VT 语义,逐字照抄。
3. **照抄:`TermMode` bitflags 全集**(`src/term/mod.rs` 的 `TermMode`,u32)。位含 `SHOW_CURSOR`(DECTCEM)/`APP_CURSOR`(DECCKM)/`APP_KEYPAD`(DECKPAM)/`MOUSE_REPORT_CLICK`(1000)/`MOUSE_DRAG`(1002)/`MOUSE_MOTION`(1003,`MOUSE_MODE` 聚合)/`SGR_MOUSE`(1006)/`UTF8_MOUSE`(1005)/`BRACKETED_PASTE`(2004)/`LINE_WRAP`(DECAWM)/`LINE_FEED_NEW_LINE`(LNM)/`ORIGIN`(DECOM)/`INSERT`(IRM)/`FOCUS_IN_OUT`(1004)/`ALT_SCREEN`(1049)/`ALTERNATE_SCROLL`(1007)/`VI`(reset 时唯一保留位)/`URGENCY_HINTS`(1042)/kitty 五位 `DISAMBIGUATE_ESC_CODES`/`REPORT_EVENT_TYPES`/`REPORT_ALTERNATE_KEYS`/`REPORT_ALL_KEYS_AS_ESC`/`REPORT_ASSOCIATED_TEXT`(`KITTY_KEYBOARD_PROTOCOL` 聚合)/`WIN32_INPUT_MODE`(1<<23,DECSET 9001)/`COLOR_SCHEME_UPDATES`(1<<24,DECSET 2031);`Default = SHOW_CURSOR | LINE_WRAP | ALTERNATE_SCROLL | URGENCY_HINTS`。DECSET 9001/2031 在 vte 里是 `PrivateMode::Unknown` 编号,Term 侧按编号特判(只订阅不主动上报,变更由 `set_color_scheme` 驱动发 `CSI ? 997;N n`)。
4. **照抄:kitty 协议 ↔ TermMode 双向映射与模式栈**(`src/term/keyboard.rs` 的 `KEYBOARD_FLAGS`、`From<KeyboardModes> for TermMode` 及反向、`set_keyboard_mode` 的 Replace/Union/Difference 三态应用)。主/备屏各持一份 `keyboard_mode_stack`(`KEYBOARD_MODE_STACK_MAX_DEPTH`),`Term::swap_alt` 换栈并按栈顶重放,`reset_state`/配置翻转清双栈;`CSI =` 修改当前帧(含隐式基帧)而非裸栈顶,是 Nebula 对上游的关键修正。契约由 `src/term/keyboard_contract_tests.rs` 五例锁死(嵌套 push/pop、alt screen 各自保存修改帧、溢出界与 reset 清栈)。slTerminal 现状仅 xterm `vtExtensions.kittyKeyboard` 被动启用,Term 侧协议状态机缺失,必须补。
5. **照抄:VT100 字符集 G0–G3**(`src/grid/mod.rs` 的 `Charsets`、`Cursor.charsets`;`src/term/mod.rs` 的 `active_charset`、`configure_charset`、`set_active_charset`;vte `StandardCharset`)。DEC 图形字符集的指定/调用/映射全链路。
6. **照抄:窗口标题栈**(`src/term/mod.rs` 的 `TITLE_STACK_MAX_DEPTH`、`title_stack`、`push_title`/`pop_title`/`set_title`;OSC 0/1/2/22)。溢出丢栈底;`None` 触发 `Event::ResetTitle`。
7. **照抄:OSC 8 hyperlink**(`src/term/cell.rs` 的 `Hyperlink`/`HyperlinkInner`/`HYPERLINK_ID_SUFFIX`、`CellExtra.hyperlink`、`Cell::set_hyperlink`/`hyperlink`;`src/term/mod.rs` 的 `set_hyperlink`)。无 id 时进程内单调发号;`Arc` 共享、随 `extra` 惰性分配/回收。slTerminal 现状靠 xterm 内建解析 + `linkHandler` 开浏览器,等价能力须落 core。
8. **照抄:OSC 52 走 vte Handler 与 `ClipboardType`**(`src/term/mod.rs` 的 `clipboard_store`/`clipboard_load`、`ClipboardType`;`src/event.rs` 的 `Event::ClipboardStore`/`ClipboardLoad`;`Config.osc52`)。读写都经事件上抛,壳决定写系统剪贴板与读请求应答。
9. **照抄:`Term` 双网格主备与 alt screen 切换语义**(`src/term/mod.rs` 的 `Term` 主/备 `Grid` 字段对)。切屏不清 scrollback、光标样式各自恢复; Vi/alt 模式互斥假设贯穿 `src/term/redraw_anchor.rs`、`src/term/prompt.rs`。
10. **照抄:`Config` 配置面**(`src/term/mod.rs` 的 `Config`)。字段:`scrolling_history`(默认 10000)、`default_cursor_style`、`vi_mode_cursor_style`、`semantic_escape_chars`、`kitty_keyboard`、`osc52`、`suppress_bringup_da1`、`conpty_resize`。语义注释一并迁移(尤其 `suppress_bringup_da1` 与 `conpty_resize` 的 ConPTY 专项理由)。
11. **照抄:`Colors` OSC 颜色覆盖表**(`src/term/color.rs` 的 `Colors`、`COUNT`)。269 槽位(16 ANSI + cube + 灰阶 + fg/bg/cursor/dim/bright),快照随 `RenderSnapshot::color_overrides` 输出给前端解析。

### OSC tee 嗅探

12. **照抄:`CwdSniffer` tee 状态机**(`src/osc_cwd.rs` 的 `CwdSniffer`、`Phase`、`feed`/`step_osc`/`push`/`parse`)。四态(Ground/Esc/Osc/OscEsc) survive 跨块拆分;`prefix_could_match` 前缀早弃——无关大 OSC(如 52 剪贴板块)永不入缓冲。有界常量:`MAX_PAYLOAD`(4KB)、`MAX_IMAGE_PAYLOAD`(12MB)、`MAX_IMAGE_PIXELS`(16M 像素,防 image bomb)、`MAX_HOOK_PAYLOAD`(96KB)。
13. **照抄:`OscEvent` 枚举与解析族**(`src/osc_cwd.rs`)。`Cwd`(OSC 7 `file://` URI 百分号解码 + Windows 盘符剥前导斜杠 `parse_osc7_uri`/`is_windows_drive_path`;OSC 9;9 原生路径去尾分隔符)、`PromptMark`/`PromptInput`/`CommandStart`/`CommandDone{exit_code}`(OSC 133 A/B/C/D,D 的首参退出码含负数 round-trip)、`UserVar`(1337 `SetUserVar` 名值 b64,8KB 值上限)、`InlineImage`(1337 `File=…inline=1`,PNG/JPEG/GIF 头嗅探取尺寸 `image_dimensions`,尺寸以编码文件为准不信元数据)、`Notify`、`Progress`(9;4 ConEmu 进度,原始 state 不上收窄)。
14. **照抄:按字节偏移拆分 `parser.advance`**(`src/event_loop.rs` 的 `StreamProcessor::feed`/`advance`)。每个 OSC 事件携带「刚过终结符」的偏移,reader 在偏移处切开 VT 解析并即时应用事件(prompt mark 落在光标恰在新鲜提示符行的时刻),事件间字节保线序——远程与本地会话不可能产生不同 cwd/命令状态。大输入 4096 分块防同步缓冲绕过(约 2MiB 强制提交前撤兼容锚点)。
15. **参考:OSC 1337 内联图片事件与行推进**(`src/event_loop.rs` 的 `StreamProcessor::feed` InlineImage 分支;`src/event.rs` 的 `Event::InlineImage`)。按视口宽等比缩放、光标绝对行锚定、推进 `\r\n` 占位。嗅探层照抄,GPUI 壳是否呈现内联图片归壳分片(需有界解码资源);slTerminal 现状无此功能。
16. **参考:OSC 9;4 进度与 OSC 9 文本通知上抛**(`src/event.rs` 的 `Event::Progress`/`Event::Notify`)。壳可映射 Windows 任务栏进度/角标;不做静默丢弃。

### Grid/回滚/损伤

17. **照抄:`Grid` 全模型**(`src/grid/mod.rs` 的 `Grid`、`Scroll`、`Dimensions`、`GridIterator`/`BidirectionalIterator`、`display_iter`/`display_iter_from`/`iter_from`、`Indexed`)。`display_iter_from` 的视口裁剪迭代支撑「提交前拖拽期」渲染,无需投影网格。
18. **照抄:`Storage` 环形缓冲**(`src/grid/storage.rs` 的 `Storage`、`zero` 偏移、`rotate`/`rotate_down` 模加、`MIN_CACHE_SIZE`/`MAX_CACHE_SIZE` 行缓存、`shrink_lines` 缓存回收)。零行搬移滚动是热路径根基。
19. **照抄:`Row` 占用计数**(`src/grid/row.rs` 的 `Row`、`occ`、`shrink`/`shrink_with_minimum` 溢出回收、`MIN_RECLAIMABLE_CELLS`、`from_vec`/`append`/`append_front`/`front_split_off`)。occ 之后单元格恒等于模板,reset/裁剪都按占用段操作。
20. **照抄:resize/reflow 与 ConPTY 行锚定**(`src/grid/resize.rs` 的 `resize`/`resize_conpty`、`grow_lines`/`shrink_lines`/`shrink_scroll_amount`、列向 `grow_columns`/`shrink_columns`、`cursor_anchor`/`restore_cursor_anchor`;`src/grid/mod.rs` 的 `set_reflow_on_grow`)。shrink 无条件折行、grow 按 `WRAPLINE` 标记重并(shrink→grow 往返无损);`conpty` 变体复刻 host 的顶端对齐/末写行锚定;光标锚定「骑在内容上」重推导,拒绝增量簿记的误差累积。`scrolled_out` 绝对行号跨 scrollback 增长稳定,prompt mark/内联图片锚定靠它。
21. **照抄:行级 damage**(`src/term/damage.rs` 的 `LineDamageBounds`、`TermDamage::{Full,Partial}`、`TermDamageIterator`、`TermDamageState`)。`undamaged` 逆序表示(left>right)、`expand` 合区间、display_offset 之上自动不可见;resize/full reset 置 Full。
22. **照抄:`RedrawAnchor` 同步重绘阅读定位**(`src/term/redraw_anchor.rs` 的 `RedrawAnchor`、`begin_redraw_anchor`/`observe_redraw_clear`/`finish_redraw_anchor`、`row_key`)。DEC 2026 同步窗口内清屏+清史后按 3 行锚在新网格找唯一匹配;用户操作/迟到输出即取消;WRAPLINE 行与纯边框行(重复 `─` 归一)有特殊键语义;扫描上限 `MAX_SCAN_CELLS`。

### 渲染合同(替换 xterm.js 的关键接口)

23. **照抄:三层渲染合同全套**(`src/render.rs`)。层间:`Term`/grid(模型)→ 本模块(viewport 协议 + 纯数据快照)→ 前端渲染器(GPUI element);前端只依赖这些类型,本模块不依赖任何 UI 框架。硬规则编码:终端内容只有 cell grid 一种表示(无「字符串流」,排版引擎不得移动字形);快照一经发出不可变且携单调递增 `revision`(过期 resize 永不覆盖新 resize);上报 PTY 的像素恒为 `cols × cell_width` 精确积(零残余像素);行列不变但像素变(字号变)也必报。符号:`CellMetrics`(`device_cell_width/height`)、`MIN_COLS`/`MIN_ROWS`、`TerminalViewport`(`from_content_size` 向下取整钳最小网格、`text_area_width_px`/`text_area_height_px`、`window_size`、`grid_eq`/`pixel_eq`)、`ViewportTracker::observe`(亚细胞抖动不产事件、合并布局观察为 revision)、`ViewportChange`、`SnapshotConfig`、`RenderSnapshot::capture`。
24. **照抄:快照成员类型**(`src/render.rs` 的 `SnapCell`、`TextSegment`(`wide` 段双列步进)、`CellRun`(选区几何)、`BgRun`(带未解析 `Color` 的背景游程,默认背景在源头抑制)、`CursorSnapshot`(shape/wide/格下字符与 flags/bg——Claude Code 藏 DECSCUSR 画反色假光标,前端需格数据重着色)、`BoxGlyph`)。INVERSE 在源头已交换进 bg;分段只由内容与宽度类决定,光标/焦点/闪烁绝不掺入(否则行缓存键随闪烁相位抖动、可见跳字)——此规则连同测试语义一并照抄。
25. **照抄:宽字符选区整字语义**(`src/render.rs` capture 的 `WIDE_CHAR_SPACER` 处理与 `src/selection.rs` 的 `SelectionRange::contains_cell`)。命中首格或占位格都按整字高亮,仅首格写两列 run;复制出的文本保持完整字符(测试 `capture_selects_complete_wide_cells_and_copy_keeps_complete_text` 为基准)。

### 内建几何字形

26. **照抄:`boxdraw` 纯几何指令**(`src/render/boxdraw.rs` 的 `is_builtin`、`Rect`、`Primitive::{Rect,Poly}`、`primitives`、`Geom`)。覆盖 U+2500–259F(框线/块元素)、U+1FB00–1FB3B 与 U+1FB82–1FB8B(Legacy Computing)、U+E0B0–E0B4 与 U+E0B6(Powerline,`e0b5` 刻意不内建)。单元格局部逻辑像素坐标、内部换算设备像素做整数吸附、Sutherland–Hodgman 四边裁剪、出口除回 scale;`░▒▓` 以 alpha 64/128/192 表浓度。根治理由:框线/块/Powerline 必须精确盖满单元格并无缝拼接,任何字体(尤其 CJK 字体)不保证——几何是唯一权威,任何渲染后端免费继承。xterm.js 无此能力(靠字体碰运气),照抄是净增益。

### ConPTY 层

27. **照抄:`ConptyApi` 侧载优先 + 系统回退**(`src/tty/windows/conpty.rs` 的 `ConptyApi::new`/`load_conpty`、`bundled_conpty_dir`、`ConptyLibrary`)。只接受同一候选目录(exe 旁 `runtime/` 或 exe 目录)的 `conpty.dll`+`OpenConsole.exe` 完整文件对;DLL 一律绝对路径 `LoadLibraryW`(PATH 同名文件不进认证边界);`GetProcAddress` 三符号;每 PTY 持一份 `FreeLibrary` 引用。回退系统 `CreatePseudoConsole`/`ResizePseudoConsole`/`ClosePseudoConsole` 函数指针,双路同型消歧。
28. **照抄:DA1 应答预热 priming**(`src/tty/windows/conpty.rs` 的 `new`;`src/term/mod.rs` 的 `Config::suppress_bringup_da1` 与 `bringup_da1_pending`)。sideloaded OpenConsole 启动握手发 DA1 并等待应答:spawn 前把 `\x1b[?61c` 预写进 conin(它成为 host 起来读到的第一字节),握手从每 pane 等待变瞬时;仅 sideloaded 做(in-box 会把无请求应答漏成输入)。host 起后再发的首个 DA1 由 Term 侧一次性标记 `bringup_da1_pending` 吞掉(`identify_terminal` 消费并跳过),防双答。
29. **照抄:Win32 input mode 自门控降级**(`src/tty/windows/conpty.rs` 的 `new`,`PSEUDOCONSOLE_WIN32_INPUT_MODE`)。先以 0x4 建 host,旧 build 拒绝(E_INVALIDARG)则退 flags=0 重试;flagless host 永不发 DECSET 9001、编码器恒走传统 VT 路径——降级端到端自门控,不硬编码 build 判断。全仓不做任何 `SetConsoleMode` 控制台模式位操作(快速编辑/回显等全交 ConPTY 默认值),0x4 是输入行为的唯一开关。
30. **照抄:`UnblockedReader`/`UnblockedWriter` 可中断读写**(`src/tty/windows/blocking.rs`)。阻塞匿名管道经 piper 中转 + 独立读写线程 + `polling` IOCP 完成包;`try_read` 的 waker 补投(读到即再投递,堵住「收尾字节打在空 waker 上」的残帧丢失——AI CLI 输入框字节丢失即此因);`drain_detached` 拆收前把管道交给分离 drain 线程(`ClosePseudoConsole` 阻塞直至 host flush,无消费端即死锁,「窗口已关进程挂任务管理器」根因)。
31. **照抄:`Pty` 字段顺序与 Drop 纪律**(`src/tty/windows/mod.rs` 的 `Pty`、`backend` 必须首字段保证 drop 序)。Drop:先 `TerminateProcess` 子树(忙进程树不能在拆解中继续产输出)→ `drain_detached` → `backend` drop 内 `ClosePseudoConsole`。
32. **照抄:`ChildExitWatcher`**(`src/tty/windows/child.rs`)。`RegisterWaitForSingleObject`(WT_EXECUTEINWAITTHREAD|WT_EXECUTEONLYONCE)线程池等待,回调取 `GetExitCodeProcess` 后经 mpsc 发 `ChildEvent::Exited` 并补投 poller 事件;pid 构造时缓存;Drop 用 `UnregisterWaitEx(INVALID_HANDLE_VALUE)` 阻塞等回调落地(反注册失败故意泄漏 context,native 侧可能仍持有指针);`register`/`deregister` 填清 Interest;`EventedPty::next_child_event`/`child_pid` 接 event_loop。
33. **照抄:`EventLoop` 全套**(`src/event_loop.rs`)。`READ_BUFFER_SIZE`(1MB)/`MAX_LOCKED_READ`(64KB 持锁上限);`FairMutex::lease` 预约锁防 UI 饥饿;`Msg::{Input,Shutdown,Resize,ResizeGrid}` 同通道有序(`ResizeGrid` 只 reflow 本地网格不通知子进程,UI 拖拽期逐帧 reflow、子进程防抖);`drain_recv_channel` resize 合并(一次 drain 只最新尺寸,grid-only 不顶掉 full);**resize 是流边界**:先按旧几何排空可读字节(旧宽绝对 CUP 不进新网格),再本地 `Term::resize`,再 `ResizePseudoConsole`(同步),随后同步对账一次 + 120ms `ALIGN_DELAY` 死线兜底(`conpty_cursor_probe`:`FreeConsole`+`AttachConsole(pid)`+`GetConsoleScreenBufferInfo` 取 conhost 光标真值,全局锁串行化,失败静默放弃);同步更新超时与对账死线合并进 poll deadline;传输死因报 `Event::PtyFailure`(会话变僵尸的根因治理)。
34. **照抄:`conpty_realign`**(`src/term/mod.rs`)。conhost 内部塌缩(顶端对齐)与变宽变窄折行差两个方向的双向滚动对账,选区随内容 `rotate`,差额 ≥ 视口高放弃;注释中的字节取证法(现场 prompts=[15] cursor=19)作为机制文档迁移。
35. **参考:注册表环境重建**(`src/tty/windows/environment.rs` 的 `refresh_environment`、`RegistrySnapshot`、`CaseInsensitiveEnv`、`expand_environment_variables`)。HKLM/HKCU/Volatile Environment(含按 session id 的子键)快照 + 进程继承环境合并:注册表键删除追踪(运行期删掉的变量在新会话真正消失,父进程私有变量保住)、Path/LibPath/Os2LibPath 分号追加、REG_EXPAND_SZ 两遍展开、`ERROR_MORE_DATA` 缓冲翻倍重试;GUI 长驻进程装完软件不重启即拿到新 PATH。环境块构造另照抄 `src/tty/windows/conpty.rs` 的 `convert_custom_env`(大小写不敏感 Unicode 序排序去重、双 NUL 结尾,CreateProcess 硬性要求)。与 slTerminal 现状(直接继承父进程块 + 固定注入)相比是净改进;覆盖项最后叠加语义保留 slTerminal 的 `TERM`/`COLORTERM`/`TERM_PROGRAM`/`SLTERM_PANEL_ID` 注入。
36. **照抄:`cmd_prompt::prepare`**(`src/tty/windows/cmd_prompt.rs`)。cmd.exe 的 PROMPT 环境注入 OSC 133;A + 原 prompt + 133;B + `SetUserVar=pebrel_cmd_prompt`(幂等、不突变调用方、不碰其他 shell)。slTerminal 现状 cmd 零集成、命令检测靠固定延迟兜底,采纳后 cmd 也有语义 prompt;变量名迁移时改 `slterm_cmd_prompt`。
37. **参考:shell 集成装配**(`src/tty/windows/mod.rs` 的 `powershell_with_nebula_integration`、`resolved_default_shell`/`nebula_default_shell`、`powershell_integration_args`、`push_escaped_arg`/`cmdline`)。原则照抄:集成脚本跟在 profile 之后加载、显式不 `-NoProfile`(与 slTerminal B17 红线一致)、`-NoLogo -NoExit`;参数转义改写自 Rust stdlib C 运行时规则,与 slTerminal `build_cmdline` 同宗,落位时择优留一。具体载体保留 slTerminal 方案(`-EncodedCommand` 内联,避免 AMSI/ASR 文件写入),pebrel 的脚本落盘路径法不采纳。默认 shell 裁定保留 slTerminal 顺序 pwsh→powershell→cmd(pebrel 直接用字面量 `powershell` 不做存在性探测),pebrel 的 powershell 默认与 bash/WSL 探测不承接。

### 输入链路与事件循环的键盘侧

38. **照抄:`win32_input_matrix.ps1` 基线回归**(`scripts/win32_input_matrix.ps1` + `win32_input_matrix.baseline.txt` + `win32_input_matrix_probe.cjs`)。PostMessage 驱动最小化实例(不抢焦点)、node reader 收字节与检入基线比对;矩阵含裸修饰键静默哨兵(PostMessage 不改真实键盘状态,chord 覆盖留人工步骤,LIMITS 注释照抄)。GPUI 壳移植后作为端到端键矩阵基线守门(kt/Win32 input/legacy 三编码路径)。
39. **照抄:`Event`/`EventListener`/`WindowSize`/`Notify`/`OnResize` trait 面**(`src/event.rs`)。事件枚举即 core→壳的边界契约;`PtyWrite`/`TextAreaSizeRequest`/`ColorRequest` 带格式化闭包的应答形态一并照抄。

### 选择/复制/搜索

40. **照抄:`selection` 四态模型**(`src/selection.rs` 的 `SelectionType::{Simple,Block,Semantic,Lines}`、`Selection::new`/`update`/`rotate`/`to_range`/`is_empty`、`Anchor`、`SelectionRange::contains`)。语义选区按 `semantic_escape_chars` 向两侧扩展,行选区扩展整行,选区随滚动 `rotate` 并保持块选区列不变。
41. **照抄:文本导出**(`src/term/mod.rs` 的 `selection_to_string`/`line_to_string`)。行选区带尾换行、块选区逐行、宽字符整字不截断——GPUI 壳 Ctrl+Shift+C 的唯一取词来源。
42. **照抄:正则搜索**(`src/term/search.rs` 的 `RegexSearch`/`LazyDfa`、`search_next`、`regex_search_left`/`regex_search_right`、`RegexIter`、`bracket_search`、`semantic_search_left/right`、`inline_search_left/right`、`line_search_left/right`)。regex-automata DFA 双向扫描,壳侧搜索框直接消费。
43. **照抄:`vi_mode`**(`src/vi_mode.rs` 的 `ViMotion` 全表、`ViModeCursor::motion`/`scroll`)。纯 core 无成本;壳是否暴露 vi 键位归壳分片,core 能力先备。

### 杂项

44. **照抄:`FairMutex`**(`src/sync.rs`)。`lease`/`lock`/`lock_unfair`/`try_lock_unfair` 四件套:读线程预约锁防 UI 线程饿死,渲染快照走 unfair 快速通道。event_loop 与渲染并发正确性的根基。
45. **照抄:语义 prompt**(`src/term/prompt.rs` 的 `nebula_add_prompt_mark`/`nebula_end_prompt`/`nebula_mark_prompt_input`/`nebula_prompt_input_point`/`nebula_prompt_jump`/`nebula_cursor_abs_line`)。prompt mark 只记主屏、行号严格递增去重、滚动出史即剪枝;input 边界随滚动增长解析、reflow/reset 丢弃(全宽 prompt 的 `input_needs_wrap` 特例在案)。OSC 133 的 A/B/C/D 事件序由 `StreamProcessor` 保序——slTerminal 现状的 133;A 恢复注入闸门(`TerminalRegistry.markPromptReady`)语义由本模型替代。
46. **照抄:`pty_trace` 启动 profiling**(`src/lib.rs` 的 `pty_trace`、`NEBULA_BOOT_TRACE`)。OnceLock T0 + 环境门控;埋点覆盖 CreatePseudoConsole 各阶段、shell attach、DA1 priming、首批 conout 字节(转义 dump 前 12 块)、conin 写前 12 块、空读唤醒。改名 `SLTERM_BOOT_TRACE`;`NEBULA_RESIZE_TRACE`(`src/event_loop.rs` 的 `resize_trace`/`trace_terminal_state`,按 `❯` 提示符行取证)一并改名保留。
47. **照抄:终端核心纪律**(`nebula_terminal/AGENTS.md`)。不依赖产品视图/窗口状态/GPUI;输入修复须分别验证原生控制台、翻译字节流、协商协议三方读取;字节流/解析/回放测试覆盖分块边界、静默源、EOF、平台差异;热路径变更须代表性成本证据;重大协议/缓冲/线程/所有权决策登记 `architecture/notes/` 惯例。随 crate 整体迁入 `slterm_terminal/AGENTS.md`。

### slTerminal 现状资产承接(本仓资产,迁入新 core/壳)

48. **照抄(迁入 `slterm_terminal`):DA1/DSR 接管纯函数族**(`src-tauri/src/pty/reader.rs` 的 `mirror_da1_query`/`inject_da1_response`/`mirror_dsr_query`/`should_answer_dsr`/`inject_cpr_response`/`strip_da1_queries`/`strip_dsr_queries`/`split_trailing_query_prefix`)。「每次查询必答、谁问谁答、禁盲注」与 Win10 键事件模式 CPR→F3 吞键毒键链(ADR-0022 教训)是比 pebrel priming 更全的会话内处置,与 priming 互补共存(机制见「优化方向」)。`strip_conpty_startup` 启动序列剥离首轮窗口语义保留,与 pebrel 首块字节 trace 合并。
49. **照抄(迁入):ConPTY flags 能力矩阵**(`src-tauri/src/pty/spawn.rs` 的 `compute_conpty_flags`、`ConptyInputModes`、守卫 `conpty_flags_default_matrix_matches_legacy_tristate`)。0x1/0x2/0x4/0x8 逐位矩阵 + 默认三态零漂移守卫 + 0x8 passthrough 默认关(真实 claude 全屏滚轮失效实测,翻转须人工实测门禁)——pebrel 只有 0x4 单 flag,slTerminal 控制面更细,保留。
50. **照抄(迁入):Win10 NuGet 捆绑发行形态**(`src-tauri/src/pty/conpty_api.rs` 的 `resolve_conpty_api`/`should_bundle`/`ensure_extracted`/`load_bundled`/`conhost_input_corrupts_cpr`、vendor/conpty)。`include_bytes!` 嵌入 + `%LOCALAPPDATA%` 幂等提取适配 slTerminal 单文件 exe 发布;pebrel 的「完整文件对 + 绝对路径加载」红线并入(采纳点 27)。回退状态可观测(`ConptyStatus` + 启动 toast 语义,壳侧重建)。
51. **照抄(迁入):Job Object 孤儿防护与销毁纪律**(`src-tauri/src/pty/spawn.rs` 的 `add_to_job_object`/`create_and_assign_job`/`JobHandle`、纯函数 `job_name`/`job_limits` 锁死 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`;`pty_kill` 的提取-释放写锁-`spawn_blocking` 销毁序;reader.rs 的 `CleanupPlan`/`plan_cleanup_after_join_timeout` 监督线程带超时销毁)。pebrel 无 Job Object(进程 linger 靠 drain 线程),slTerminal 方案在父进程崩溃路径更硬,保留;`ClosePseudoConsole` 永久阻塞(上游 Discussion #17716)的双保险(pebrel 的 `drain_detached` 前置 + slTerminal 的监督超时后置)并存。
52. **照抄(迁入):spawn 串行化与容量纪律**(`SPAWN_LOCK`、`MAX_PTY_SESSIONS`、锁内判定+写锁原子复查、命中上限显式 kill 已 spawn 子进程)。pebrel 无对应物(其 spawn 走 alacritty 路径无并发 spawn 死锁记录),slTerminal 的 ConPTY 并发 spawn 卡死实测红线保留。
53. **照抄(迁入):shell 白名单深检**(`src-tauri/src/pty/shell.rs` 的 `validate_shell_allowlist`/`which_full_path`/`paths_match`/`file_identity`/`reparse_entry_identity`/`fallback_identity_match`、白名单三 exe)。含路径输入只信 PATH 解析 + canonicalize 双失败回退句柄级身份比对(AEL 应用执行别名经 reparse 条目身份,任一侧证据缺失即拒绝,不降级字符串)——pebrel 无白名单概念,SEC-01/SEC-15 保留。
54. **照抄(迁入):pwsh 启动形态与 B17 守卫**(`build_pwsh_command`/`build_pwsh_info`/`encode_utf16le_base64`、`shell-integration.ps1`、守卫用例 `pwsh_args_no_noprofile_b17`)。`-NoLogo -NoExit -EncodedCommand`,禁 `-NoProfile`,与 pebrel profile 优先原则同源,载体不同各留其长。
55. **照抄(迁入):会话归属与元数据单点**(`state.rs` 的 `PtySession`/`PtyState`/`validate_session_ownership`、前端 `src/panels/terminal/TerminalRegistry.ts` 的 panelId→sessionId 注册表 + 订阅模式 + `markPromptReady`/`setAgentSession` 语义)。单进程世界无 session_id/panel_id 跨进程边界,映射为壳内「终端面板注册表」(对象标识 + 订阅),「会话元数据唯一单点」约束以 Rust 形态重建(硬约束 #8 的问题域延续,约束文本不迁移)。
56. **照抄(迁入):OSC 52 壳侧策略**(`src/panels/terminal/oscHandlers.ts` 的 `registerOsc52` 语义)。仅放行系统剪贴板选择器 `c`、禁读请求、1MB payload 上限、焦点门控(后台面板不得静默改剪贴板)。迁入后挂 `Event::ClipboardStore` 消费端;core 嗅探的有界常量(4KB)与壳策略上限(1MB)分层各守。
57. **照抄(迁入):OSC 133 命令 profile 消费**(`src/panels/terminal/oscHandlers.ts` 的 `registerOsc133` + `src/panels/terminal/useCommandDetection.ts`)。133;C 首 token 精确匹配 CLI profile(覆盖 `claude --resume` 变体)、B12 顺序约束(setAgentSession 先于 onTabStateChange)、133;D/PTY exit 双真退出信号——迁入后挂 `Event::CommandStart`/`CommandDone`,语义 prompt 边界取自采纳点 45。
58. **照抄(迁入):复制/中断语义**(`src/panels/terminal/keyboard.ts` 的 `terminal.copy`「选中才复制、空选区不动剪贴板」+ `terminal.interrupt` 本地提示后透传 `\x03` 的 CP-020 双路径幂等语义;复制合同反馈按 `docs/project-constraints.md` §5.2)。GPUI 壳中 Ctrl+Shift+C → `selection_to_string` → 剪贴板(选中才写),Ctrl+C 恒透传中断;复制成功反馈(勾+说明→恢复,约 1.5s 共享时限)按 §5.2 单点实现。pebrel 的复制合同与本仓定位约束在此合流。
59. **照抄(迁入):微批聚合与 pending 判定**(`src-tauri/src/pty/reader.rs` 的 `micro_batch_tail`、`READER_BUF_SIZE`/`MICRO_BATCH_MAX`、PeekNamedPipe 式 pending 查询替代 WaitForSingleObject 竞态修复)。迁入 event_loop 读侧后,1MB 大缓冲为主、微批为 IPC 消除手段——GPUI 壳内无 IPC,微批自然退化为单事件循环内合帧,`pending` 判定机制保留防半帧唤醒丢失。
60. **消亡(不迁):xterm.js 运行时全家**(`@xterm/xterm` 6.1、addon-fit/addon-webgl、useXterm/usePtyOutput/usePtyResize/useTerminalInstance/webgl.ts)。其职责被显式替代:VT 解析→core、渲染→`RenderSnapshot`+GPUI、fit→`ViewportTracker`、WebGL 装配/SwiftShader 检测/退避→无对应物(GPUI 渲染栈自管,能力不迁移)、DEC 2026 前后包裹合帧→vte 原生 sync + 壳帧渲染自然消除、`windowsPty.buildNumber` 钳制(ADR-0004)→Win32 input flag 自门控降级替代、`scrollback: 5000`→`Config.scrolling_history`(按 slTerminal 现值定)、`drawBoldTextInBrightColors`→壳调色板解析策略。`vtExtensions.kittyKeyboard` 被动启用语义由采纳点 4 的 TermMode 状态机 + 壳编码器承接。
61. **消亡(不迁):Tauri 传输层**(`Channel<PtyEvent>`、`src/ipc/pty.ts`、bytes→number[] 序列化、pty 命令五件套)。单进程后 event_loop 直送壳事件队列;「前端绝不直接碰 OS」约束(硬约束 #1)随边界消失,问题域由「core 不依赖视图」承接。

## 不采纳点

1. **`src/tty/unix.rs` 与 Unix 依赖全家**——多平台为已定不采纳项;`Cargo.toml` 的 `rustix-openpty`/`rustix`/`signal-hook` 依赖随之删除。平台分支收敛约定(`src-tauri/src/pty/CLAUDE.md`)落地形态变为:tty 模块只有 windows 子模块,业务 `#[cfg]` 无生存土壤。
2. **bash/WSL shell 探测与集成**(`src/tty/windows/mod.rs` 的 `nebula_bash_shell`、`nebula_wsl_shell`、`bash_with_nebula_integration`、`explicit_bash_integration_args`、`nebula_find_bash`;`environment.rs` 相应分支)——WSL 为已定不采纳项,bash 在 Windows 原生定位无位;`Shell` 枚举收敛为三 exe(pwsh/powershell/cmd)。
3. **Lua 运行时设置读取**(`src/tty/windows/mod.rs` 的 `nebula_runtime_settings`、`nebula_default_shell` 对 nebula_settings.txt 的解析;sideload 开关 `openconsole=off` 的设置源)——Lua 为已定不采纳项;侧载/flags 开关改由 settings JSON 键承载(采纳点 49 矩阵已备)。
4. **OSC 777 远端 hook 通道**(`src/osc_cwd.rs` 的 `parse_remote_hook`/`OscEvent::RemoteHook`;`src/event_loop.rs` 的 `StreamProcessor::set_remote_hook_token`、`EventLoopSender::standalone`;`src/event.rs` 的 `Event::AiHookEnvelope`)——远端/SSH 为已定不采纳项;slTerminal 的 CLI 生命周期 hook 自有通道(hooks 模块信号文件),不经 OSC。`SetUserVar` 机制保留(采纳点 13),但 pebrel 私有变量名(`pebrel_shell`/`nebula_ai_query`)不迁移,slTerminal 的 shell-integration 变量名另立。
5. **DEC 2031 亮暗订阅与 `URGENCY_HINTS` 的壳侧消费**(`src/term/mod.rs` 的 `report_color_scheme`/`set_color_scheme` 调用侧)——仅暗色定位下壳无主题切换消费端;Term 侧协议解析照抄保留(壳可备用),不接线不订阅。
6. **`EventLoopSender::sink` 非 PTY 传输通道**(`src/event_loop.rs`)——为无 PTY 面板(文档查看器)吞输入而设;GPUI 壳的输入路由由壳自管,core 不提供 sink。
7. **ref_test 录制回放**(`src/event_loop.rs` 的 `ref_test`、`nebula.recording` 写盘)——alacritty 血统的回归设施,被 `NEBULA_RESIZE_TRACE`/win32_input_matrix/四级测试金字塔替代;不迁移(留 git 历史,需要时找回)。
8. **内嵌集成脚本的 `%LOCALAPPDATA%` 落盘路径法**(`src/tty/windows/mod.rs` 的 `nebula_prompt_script_path`/`write_if_changed`)——slTerminal `-EncodedCommand` 内联方案已规避 AMSI/ASR 文件写入风险,pebrel 落盘法不承接(采纳点 37 已取原则)。
9. **inline image 的 `Event::InlineImage` 壳呈现承诺**——采纳点 15 已定:嗅探层照抄,壳是否呈现归壳分片;本分片不承诺。
10. **HTTP 代理注入与内嵌连接/补全脚本**(`src/tty/windows/mod.rs` 的 `NEBULA_PROMPT_PS1` 中 `proxy.ps1` 段与 `PEBREL_HTTP_PROXY` 标记、`src/tty/windows/proxy.ps1`、`src/tty/connection.ps1`、`src/tty/completion.ps1`、`src/tty/mod.rs` 的 `connection_shell()`、`proxy_tests.rs` 回环代理测试)——slTerminal 无代理注入产品需求;代理设置经系统环境自然继承,不在终端核心造私有通道。
11. **`tty::setup_env()` 的 terminfo 存在性探测**(`terminfo_exists` 的多目录探测链)——Unix 面向机制;Windows-only 下沿用 slTerminal 现状定死注入(`TERM=xterm-256color`/`COLORTERM=truecolor`/`TERM_PROGRAM=slterm`),不探测不回退。

## 优化方向

1. **`slterm_terminal` 目标形态**:目录映射 `src/term/`(mod/cell/clear/color/damage/keyboard/prompt/redraw_anchor/renderable/search)、`src/grid/`(mod/resize/row/storage)、`src/render.rs`+`src/render/boxdraw.rs`、`src/osc_cwd.rs`、`src/event.rs`、`src/event_loop.rs`、`src/selection.rs`、`src/vi_mode.rs`、`src/sync.rs`、`src/index.rs`、`src/thread.rs`、`src/tty/mod.rs`+`src/tty/windows/`(conpty/blocking/child/cmd_prompt/environment/mod)、`AGENTS.md` 纪律;`tty/unix.rs` 不拷。对外边界即 `lib.rs` 模块面 + `Event`/`EventListener`/`RenderSnapshot`/`TerminalViewport`/`ViewportTracker`/`Term`/`Grid`/`CwdSniffer`/`EventLoop`/`Msg` 十个出口。
2. **解析器与同步更新**:vte 0.15 走 crates.io 不 fork,OSC 缺口由 `CwdSniffer` tee 永久补齐;DEC 2026 由 vte `sync_timeout`/`stop_sync` 原生承担,壳帧渲染只在 sync 窗口外提交,`redraw_anchor` 只在同步更新完整收尾后恢复阅读位置(动画帧不得观察部分同步更新,测试语义照抄)。
3. **nebula→slterm 改名面**:方法族 `nebula_*`(prompt/光标绝对行)改 `slterm_*`;`NEBULA_BOOT_TRACE`/`NEBULA_RESIZE_TRACE` 改 `SLTERM_*`;`SetUserVar` 键(`pebrel_cmd_prompt` 等)改 `slterm_*`;事件与文档中 pebrel 私有称谓统一替换;协议号(DEC 2026/9001/2031、OSC 7/9;9/52/133/1337/777)、机制与注释语义不动。
4. **DA1/DSR 双方案合流**:sideloaded host 启动握手走 priming(预写 `\x1b[?61c` + `bringup_da1_pending` 吞重答);会话内 DA1 查询走 reader 每查询代答(双后端统一);DSR 维持门控(Win10 家族剥离不答、Win11 按需代答、禁盲注红线)。现有三处应答身份各异——priming `\x1b[?61c`、Term 自答 `\x1b[?6c`(`identify_terminal`)、slTerminal reader 代答 `\x1b[?64;22c`——合并时统一为单一身份并经真实 claude 全屏验证后钉死,消除双重身份。
5. **resize 链路归位**:壳布局 → `CellMetrics`+`ViewportTracker`(亚细胞抖动不产事件)→ `Msg::ResizeGrid` 拖拽期逐帧 reflow(视口所见即真实几何的 reflow)→ 落定 `Msg::Resize` 通知子进程;事件循环内「先排空旧几何、再本地 reflow、再 `ResizePseudoConsole`、同步对账 + 120ms 死线兜底」全链照抄;slTerminal 前端 X/Y 分离 debounce 的意图(列变 re-wrap 昂贵)由该链天然承担,debounce 本身不迁移。
6. **复制与中断的合同位置**:Ctrl+Shift+C=`selection_to_string`→壳剪贴板(选中才写、写后按 §5.2 给可感知反馈、单一权威时限实现);Ctrl+C 不经 core 直接编码 `\x03`;OSC 52 写走 `Event::ClipboardStore` + 焦点门控与选择器白名单;OSC 52 读请求恒拒绝。中断的本地提示(页签 attention)归壳,core 只保字节语义。
7. **测试迁移与重建**:L1 全量照抄(term/grid/osc_cwd/render/selection/search/keyboard_contract/damage/redraw_anchor/event_loop 纯函数用例);slTerminal reader/spawn/shell/conpty_api 纯函数族并入同层;pty 集成测试(`--test-threads=1` 纪律)重建于新 tty 层;pebrel 真 shell 集成测试类别一并移植(`src/tty/windows/mod.rs` 的 `run_powershell_integration_case` 范式:集成脚本 BOM、用户 profile 优先、OSC 133 A/C/D 生命周期、SetUserVar 协议、用户自定义 prompt 不被覆盖;`src/tty/windows/test/bash_input.rs` 的「真 PTY 端到端编辑回归」范式迁到 pwsh,作为键事件/kitty/编辑擦除的实机守门);`win32_input_matrix` 三件套移植为 GPUI 壳键矩阵基线(chord 人工步骤保留);Win10 捆绑与 flags 矩阵的实机验证点(ADR-0005/CP-009 门禁)原样继承。
8. **slTerminal 终端相关资产归宿汇总**:后端五件套(spawn/reader/shell/conpty_api/win_build)的领域规则入 `slterm_terminal` 对应位置,Win32 胶水随 tty/windows 重组;`PtySession`/`PtyState` 退化为壳内终端面板注册表(硬约束 #8 问题域);前端十文件全消亡(采纳点 60/61);TerminalRegistry/OSC 52/133 消费/复制中断语义上移壳(采纳点 55–58);theme 的 terminal 25 键调色板段转壳调色板 token,配色单点约束(硬约束 #6)问题域由壳重建。
9. **热路径治理继承**:1MB 读缓冲/64KB 持锁上限/Storage 环形零搬移/Row occ/快照单趟无引用——pebrel 现状即基准,任何改动须按 `AGENTS.md` 纪律附代表性成本证据;slTerminal 侧亦照此口径登记豁免,禁止为「整洁」牺牲已验证的热路径形态。
