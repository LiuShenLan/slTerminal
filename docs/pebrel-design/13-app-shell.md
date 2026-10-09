# 13 应用壳与终端点亮详细设计(M3)

> pebrel-design 补篇(review C-1 承接:堵 M3 无设计篇承接的结构性空洞)。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`;终端 core 公共面:`docs/pebrel-design/02-terminal-core.md`;pane/tab 与 Workspace 契约:`docs/pebrel-design/05-workspace-layout.md`。
>
> 本篇是 **GPUI 壳层类型** 的锚点归属篇:`TerminalView`/`TerminalElement`/`TerminalSession`/`EventProxy`/keymap 编码器/`SltermWorkspace` M3 雏形在此签名级定义,他篇(03/04/05/06/09 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

M3 终态 = `slterm.exe` 首个可运行态:console 子系统进程 → GPUI 消息循环 → 单窗口(暗色、GPU 加速) → 单 `Workspace` 雏形 → 单终端 pane → ConPTY 直连默认 shell(pwsh → powershell → cmd 回退,02 篇 `resolve_shell` 已锚) → 键入即回显、复制/中断语义就位、IME 可输入中文。无分屏/无 AI/无设置页/布局不持久(00-roadmap 过渡期清单原样成立)。

壳即前端:**壳内状态权威雏形 = `SltermWorkspace` 直持 pane 实体与展示态**,无投影层;M7.2 由 04 篇 RuntimeHub 以 facade 缝接管投影(C-2 承接,见「改造节 2」)。

### 模块终态(`slterm_app/src/`,M3 落位面)

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `src/main.rs` | pebrel 裁剪重建 | 启动链:helper 拦截 → CLI 解析 → console 准备 → `run_shell`;关窗即退出 |
| `src/brand.rs` | 01 篇锚点 | `NAME`/`WINDOWS_APP_ID`/`DESCRIPTION` 落位 |
| `src/error.rs` | 01 篇锚点 | `AppError` 落位(M3 最小:io/pty/gpui 三包) |
| `src/logging.rs` | pebrel 照抄裁剪 | 日志初始化(写 stderr/文件,GUI 无控制台句柄纪律) |
| `src/panic.rs` | pebrel 照抄 | 启动期错误弹面 + panic 钩 |
| `src/platform/startup.rs` | pebrel 改造 | `prepare_gui_process`(console 子系统语义反转改造,本篇厚写)+ `report_error` + `first_frame` |
| `src/platform/dirs.rs` | 照 01 篇 | `data_dir`/`home_dir`(OnceLock + USERPROFILE) |
| `src/platform/keyboard.rs` | pebrel 照抄 | `native_character`(ToUnicode 恢复 GPUI 丢弃的控制字符) |
| `src/app_shell/mod.rs` | pebrel `gpui_shell/mod.rs` 裁剪重建 | `run_shell`/`init`/`open_main_window`/壳事件队列 |
| `src/app_shell/config.rs` | pebrel `gpui_shell/config.rs` 裁剪 | 壳侧 `Settings` 全局(字体/调色板/滚动/复制语义,M3 最小消费面) |
| `src/app_shell/theme.rs` | pebrel 裁剪 | 暗色单主题装载(pebrel 暗色子集,D06-3;主题系统归 06,M3 只保一份内置暗色) |
| `src/app_shell/assets.rs` | pebrel 照抄 | `gpui::AssetSource` 内嵌资产桥 |
| `src/gpui_terminal/mod.rs` | pebrel `gpui_shell/terminal/mod.rs` | 终端垂直切片根:key_context + Tab/Shift-Tab 绑定 |
| `src/gpui_terminal/session.rs` | pebrel `terminal/session.rs` 裁剪(SSH 臂不迁) | `TerminalSession`/`EventProxy`/`spawn`(PTY 接线) |
| `src/gpui_terminal/event_mailbox.rs` | pebrel 照抄裁剪 | Wakeup 合并通道(NativePrompt 族随 cmd prompt 归 M2/02 裁留判定,见改造节) |
| `src/gpui_terminal/session_pump.rs` | pebrel 照抄裁剪 | 事件批合泵 → `TerminalView::process_event` |
| `src/gpui_terminal/view.rs` + `view/`(layout/pointer/startup/typography) | pebrel 裁剪重建 | `TerminalView` 实体(M3 字段集,锚点) |
| `src/gpui_terminal/element.rs` | pebrel 裁剪重建 | `TerminalElement` 自绘元素(消费 02 篇 `RenderSnapshot`) |
| `src/gpui_terminal/colors.rs` | pebrel 照抄 | `Palette` → `Rgba` 解析(默认暗色;矫正器归 06) |
| `src/gpui_terminal/keymap.rs` + `keymap/win32.rs` | pebrel 照抄 | 三路径键编码器(kitty/Win32 input/legacy) |
| `src/gpui_terminal/mouse_protocol.rs` | pebrel 照抄 | X10/normal/SGR 鼠标上报纯函数 |
| `src/gpui_terminal/copy_feedback.rs` | pebrel 照抄 | 复制反馈时限状态机(§5.2 合同) |
| `src/app_shell/workspace.rs` | pebrel `gpui_shell/workspace.rs` 裁剪重建 | `SltermWorkspace` M3 雏形:单 tab 单 pane + 矩形回写机制(05 篇契约) |
| `src/app_shell/windowing.rs` + `windowing/startup_geometry.rs` | pebrel 裁剪重建 | 窗口创建/`WindowOptions`/`prepare_initial_grid`/首帧 present-then-show |
| `src/terminal_registry.rs` | 05 篇锚点(新建) | 模块级单例:PaneId → 会话元数据索引(M3 立本体,字段最简) |

裁剪总口径:SSH/WSL/Quick Terminal/mux 驻留/多窗/补全/数学/AI hook/inline image/答案阅读器/托盘/更新/设置页全部不迁(各归其篇或消亡);pebrel 侧 `TerminalView` 约 40 个领域字段在 M3 裁到本篇章节列出的最小集,其余按归篇在 M4–M10 各自回填——**字段只增不改,回填路径在各归篇**。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **core↔壳单向依赖**:`slterm_terminal` 不出现 GPUI 类型(02 篇纪律);壳消费 core 只经 02 篇公共面(`lib.rs` 出口集合),反向通知只经 `Term<T: EventListener>` 的泛型通道。壳层 `gpui_terminal` 模块之外不出现 `slterm_terminal` 类型(pebrel `gpui_shell/terminal/mod.rs` 模块头纪律随迁)。
3. **状态权威雏形直读 Workspace**(C-2 承接):M3 不建 RuntimeHub、不建投影层;`SltermWorkspace` 是唯一 pane 属主(05 篇不变式照抄),侧栏/标题等展示直接读实体。04 篇 M7.2 接入时以 facade 缝投影,本篇预留缝位置(改造节 2),不预埋 hub 本体。
4. **console 子系统(D04-1 落点)**:`#![windows_subsystem = "console"]` 落 `src/main.rs`;启动链 console 语义反转改造归本篇(改造节 1),「shell 同步等待」后果归开放问题登记用户裁决。
5. **关窗即退出**(跨领域不变量 5):`QuitMode::LastWindowClosed` + 无 mux 驻留臂;最后一窗关闭 = 进程退出。pebrel 的 `hide_native_window`/驻留托盘语义不迁。
6. **类型锚点纪律**:本篇唯一定义壳层类型(`TerminalView`/`TerminalViewEvent`/`TerminalSession`/`EventProxy`/`GridSize`/`TerminalElement`/`TermLayout`/`SltermWorkspace`/`SltermShellEvent`/壳 `Settings`);终端 `Grid`/`RenderSnapshot`/`ViewportTracker` 等归 02 篇只 `use`;`PaneId`/`TabId`/`SplitTree` 归 05 篇只 `use`(M3 雏形只用 `PaneId`,树归 M5);主题/`RuntimeSettings` 归 06 篇(M3 壳 `Settings` 只含终端渲染必需字段,与 06 的缝合见改造节 2)。壳层类型行补登 01 篇锚点表归开放问题。
7. **复制语义定位约束落地**:Ctrl+Shift+C = 复制(有选区才写剪贴板 + §5.2 反馈),Ctrl+C = 恒透传 `\x03` 中断;分发判定在 `on_key_down` 快捷键层(改造节 3),编码器层无特例。
8. **仅暗色**:M3 只装一份内置暗色 palette(pebrel deep-space 默认值,D06-3 的 linear 默认归 06);无主题切换、无亮暗跟随;`Term::set_color_scheme(true)` 出生即种暗(因果链随迁:palette 翻转去重吞首条通知)。
9. **测试覆盖门禁**:本篇改动全部可自动化;键位合同以真实分发形态锚定(11 篇「键绑定真实分发测试锚」纪律);win32 输入矩阵基线建基 + 常挂比对进 M3 出口(C-4 承接,00-roadmap M2 出口已收窄)。
10. **feature 门控**:UI 测试全部 `#[cfg(all(test, feature = "gpui-test-support"))]` 双门控(11 篇锚定);生产构建不编测试夹具。

## 关键类型与签名

> 均为草稿级签名;照抄部分的签名与 pebrel 一致,改名点已标。`u64` 裸 pane id 在壳层收编为 05 篇 `PaneId` newtype(本篇唯一的类型面偏差,防 id/比例混算;不改语义)。

### 启动链(`src/main.rs` + `src/app_shell/mod.rs`,本篇锚点)

```rust
// src/main.rs
#![windows_subsystem = "console"]          // D04-1:console 子系统;GUI 人格闪控制台窗已接受
fn main() -> Result<(), Box<dyn Error>> {
    // 序固定(因果链照抄 pebrel main.rs,逐环登记见改造节 1):
    // 1. ai_session_identity helper 拦截归 03,M3 无此臂
    // 2. import_environment_aliases 不迁(NEBULA_/PEBREL_ 双名层消亡,01 篇 F 表)
    // 3. boot_trace 锚点(SLTERM_BOOT_TRACE,02 篇已锚 core 侧;壳侧同宏随迁)
    // 4. CLI 解析(clap 面归 04 篇;M3 只保 --working-directory/-e 两个启动参数)
    // 5. panic::attach_handler
    // 6. platform::startup::prepare_gui_process()(console 改造,见下)
    // 7. logging::initialize
    // 8. 单实例移交归 04/M7;M3 无 probe——二次启动 = 拒绝(同一 runtime.port
    //    锁位不建,见开放问题)
    // 9. app_shell::run_shell(initial_cwd, initial_command)
    // 10. 退出序:QuitMode::LastWindowClosed 收束 → session 收尾归 05/M5.5;
    //     M3 无 1Hz 快照,关窗即进程退出
}

// src/app_shell/mod.rs
pub(crate) enum SltermShellEvent {          // pebrel GpuiShellEvent 裁剪:Tray*/Notification*/
    MuxAttach,                              //   SshPrompt/UpdateAvailable 臂不迁;M3 只留两变体,
    RuntimeControl(Arc<RuntimeDispatch>),   //   RuntimeControl 是 04 篇 M7 RuntimeCallback 的预埋缝
    OpenDirectories(Vec<String>),           //   (on_open_urls/右键「在此处打开」归 09,M3 登记不接线)
}                                           //   ——变体集按 M3 实际消费收窄,扩归各篇

pub fn run_shell(initial_cwd: Option<PathBuf>, initial_command: Option<Program>);
fn init(cx: &mut App);                      // 组件库 init + 内嵌字体注册 + 暗色主题装载 +
                                            //   gpui_terminal::init + workspace::init
fn open_main_window(cx: &mut App, shell_rx: mpsc::Receiver<SltermShellEvent>,
                    initial_cwd: Option<PathBuf>, initial_command: Option<Program>);
pub(crate) fn try_write_stderr(args: fmt::Arguments<'_>);   // GPUI 主窗标准错误句柄不可依赖
// gpui_shell 模块头 println!/eprintln! 编译期拦截宏照抄(GUI 无效标准句柄防 panic;
// console 子系统下句柄有效,但拦截宏保留——诊断统一走 try_write_stderr 单点)
```

### console 启动准备(`src/platform/startup.rs`,改造后签名)

```rust
pub(crate) fn prepare_gui_process() -> io::Result<()> {
    // D04-1 console 子系统下的语义反转改造(改造节 1 厚写):
    // - SetConsoleCtrlHandler(None, 0) 归一化保留(NULL-handler ignore 继承链治理不变)
    // - CLI_ENV 自举(set_var current_exe)保留,改名 SLTERM_CLI
    // - FreeConsole 臂删除:console 子系统下父 shell 同步等待是 D04-1 已接受后果,
    //   FreeConsole 会把继承的控制台扔掉、断开标准流(标准流正确性优先于闪窗)
}
pub(crate) fn report_error(error: &dyn Display, gui_launch: bool);   // 照抄
pub(crate) mod first_frame {        // 照抄 pebrel first_frame.rs
    pub(crate) fn defer_show(options: &mut WindowOptions) -> bool;
    pub(crate) fn present_then_show(handle: AnyWindowHandle, cx: &mut App);
}
pub(crate) fn primary_display_scale() -> Option<f32>;               // 照抄(开窗前 DPI 预取)
```

### Workspace M3 雏形(`src/app_shell/workspace.rs`,本篇锚点)

05 篇锚定的 `Workspace`/`WorkspaceTab`/`TerminalPane` 三件套是全量终态;M3 落**最小子集**,字段语义与 05 篇终态严格一致(05 篇「与 M3 的接口契约」:M5 扩不改):

```rust
pub struct SltermWorkspace {                    // pebrel NebulaWorkspace 裁剪
    tabs: Vec<TabEntry>,                        // M3 恒为单 Terminal tab;TabEntry 用 05 篇锚定形态
    active: usize,
    next_pane: u64, next_tab: u64,              // PaneId/TabId 单调分配权威(05 篇不变式 2)
    initial_grid: (u16, u16),
    pane_bounds: Rc<RefCell<HashMap<PaneId, Bounds<Pixels>>>>,   // PaneBoundsStore 回写机制
    split_bounds: Rc<RefCell<HashMap<Vec<bool>, Bounds<Pixels>>>>, // SplitBoundsStore(M3 恒空,
}                                               //   机制先立,M5 分屏递归直接消费——05 篇契约行)
impl SltermWorkspace {
    pub fn new(window, shell_rx, startup, cx) -> Self;   // 单窗单 tab 单 pane;无 WindowRegistry/多窗臂
    fn new_pane(&mut self, grid: (u16,u16), launch: TerminalLaunch, window, cx) -> TerminalPane;
    fn on_terminal_event(&mut self, ...);       // 宿主订阅:TitleChanged/Exited/UserInput 最小白集
    // 壳事件泵:start_shell_event_pump(120ms 轮询 mpsc → dispatch)照抄 pebrel residency.rs 形态;
    // M3 泵存在但事件集最小(SltermShellEvent 仅两变体),M4+/M7 事件臂在各归篇回填
}
pub fn init(cx: &mut App);                      // workspace 级键绑定:M3 只注册窗口/复制族(改造节 3)
```

窗口创建(`windowing.rs`,裁剪 pebrel `open_workspace_window`):

```rust
fn workspace_window_options(cx: &App, focus: bool) -> WindowOptions {
    // WindowRole::Regular 单形态;preferred_size = startup_geometry 反推(DEFAULT_GRID 116×30)
    // titlebar: TitleBar::title_bar_options() + app_id = brand::WINDOWS_APP_ID(P-3)
    // window_background: 暗色不透明(壁纸/acrylic 归 09;M3 = WindowBackgroundAppearance::Opaque)
    // window_min_size 照抄;window_chrome::configure_options 归 09,M3 无 macOS 臂整体不迁
}
pub(crate) fn open_initial_window(cx, shell_rx, initial_cwd, initial_command);
// WorkspaceStartup 裁为两变体:NewTerminal{cwd} / LaunchTerminal{cwd, command}
// (RestoreOrDefault 归 05/M5.5 接;RestoreUpdate 归 09/M10)
```

### TerminalView(`src/gpui_terminal/view.rs`,本篇锚点;M3 裁剪字段集)

```rust
pub struct TerminalView {
    // ── 会话与身份 ──
    pub pane_id: PaneId,                        // 05 篇 newtype(pebrel 为 u64)
    pub session: Option<TerminalSession>,
    pub focus_handle: FocusHandle,
    // ── 字体与度量 ──
    pub font: Font, pub font_bold: Font, pub font_italic: Font, pub font_bold_italic: Font,
    pub font_size: Pixels, pub ligatures: bool,
    font_offset_x: f32, font_offset_y: f32,
    line_height_multiplier: Option<f32>,
    // ── 配色 ──
    pub palette: Arc<Palette>,                  // 暗色单份;TerminalColorResolver 矫正归 06
    // ── IME ──
    pub marked_text: Option<String>, pub ime_bounds: Bounds<Pixels>,
    // ── 标题/cwd(展示态最小集;NEBULA| 标题协议字段族归 03/05 回填)──
    pub title: String, pub cwd: String,
    // ── 几何与尺寸链 ──
    origin: Point<Pixels>, cell_width: Pixels, line_height: Pixels,
    cols: usize, rows: usize,
    window_size: WindowSize,                    // 子进程已被告知的几何(与 cols/rows 分离的因果随迁)
    grid_synced: bool, spawn_at: Instant,       // 启动稳定闸
    viewports: ViewportTracker,                 // 02 篇锚点类型
    pending_resize: Option<TerminalViewport>, structural_resize: bool, resize_epoch: u64,
    // ── 滚动与选择 ──
    scroll_px: f32, selecting: bool,
    selection_scroll_epoch: u64, selection_scroll_active: bool,
    // ── 光标 ──
    cursor_visible: bool, cursor_blink_epoch: u64,
    cursor_window_active: bool, cursor_pane_focused: bool,
    _cursor_blink_subscriptions: [Subscription; 3],
    default_cursor_style: CursorStyle,
    // ── 生命周期 ──
    error: Option<String>, exited: Option<String>,
    // ── 复制语义 ──
    copy_on_select: bool,
    copy_feedback: CopyFeedback,                // §5.2 反馈状态机(时限实现单点)
    // ── 鼠标上报去重 ──
    last_report_point: Option<TermPoint>,
    // ── 输出可见性(隐藏 pane 不失效缓存,05 篇 zoom/tab 切换的前置)──
    output_visible: bool,
}
```

裁剪掉的 pebrel 字段及归篇:`answers`/`answer_reader`(08)/`math`(08)/`agent_activity`/`ai_session*`(03)/`suggest*`/`completion_*`/`editor_query_task`(08)/`inline_images`/`image_paste`/`path_drop`(02 开放问题 4/07)/`ssh_*`(消亡)/`exec_context`/`active_run`/`last_run`/`pending_runtime_submit`(04)/`progress`/`bell_flash`(09)/`confirmation`/`pending_shell_command`/`recovery`/`session_launch`(05 恢复链 M5.5)/`command_running*`/`awaiting_input`/`last_command_failed`/`completed_at`/`last_task_state`(03/09 徽章族)/`hint_config`/`link_hover`/`pending_link_open`(OSC8 链接,07/09 裁定)/`scrollbar_drag`(滚动条 overlay,M5 随分屏渲染扩)/`cursor_animation`(光标平滑动画,M6 主题联动裁定)/`cell_width_mode`(设置键域归 06,M3 恒 Compact)。

M3 关键方法面(签名照抄 pebrel,语义注释随迁):

```rust
impl TerminalView {
    pub const DEFAULT_GRID_COLUMNS: u16 = 116;  // 与 DEFAULT_GRID_LINES=30 一起是窗口定形基准
    pub const DEFAULT_GRID_LINES: u16 = 30;
    pub fn new(pane_id: PaneId, spawn_grid: (u16,u16), launch: TerminalLaunch,
               window: &mut Window, cx: &mut Context<Self>) -> Self;
    pub fn cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels);
    pub fn startup_cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels);
    pub fn shutdown(&self);                     // Msg::Shutdown;Drop 兜底(幂等)
    pub(super) fn process_event(&mut self, event: TermEvent, cx);   // 见数据流节白集
    pub fn mark_structural_resize(&mut self);
    pub fn set_layout(&mut self, origin, cell_width, line_height, content: Size<Pixels>, scale, cx);
    pub fn copy_selection(&mut self, notify: bool, window, cx) -> bool;  // 选中才写剪贴板
    fn paste(&mut self, window, cx);            // bracketed-paste 包装照抄(paste_now_impl)
    fn on_key_down(&mut self, event: &KeyDownEvent, window, cx);    // 改造节 3 合同
    fn on_scroll(&mut self, event: &ScrollWheelEvent, window, cx);  // 滚轮→Scroll/鼠标上报/alt 方向键
    // on_mouse_down/move/up/right/middle:选择四态 + 鼠标协议消费,裁剪补全/链接臂
}
impl EventEmitter<TerminalViewEvent> for TerminalView {}
impl Drop for TerminalView { /* shutdown 兜底 */ }
impl Focusable for TerminalView { /* focus_handle */ }
impl gpui::EntityInputHandler for TerminalView { /* IME 七件,见改造节 4 */ }
impl Render for TerminalView { /* div.key_context + TerminalElement + exited/error 角标 */ }

pub enum TerminalViewEvent {    // M3 白集;TitleChanged/Exited 宿主消费,
    TitleChanged, Exited,       //   UserInput/Notification/ProgressChanged 归各篇回填
}
pub enum TerminalLaunch {       // SSH 变体消亡
    Local { cwd: Option<PathBuf>, shell: Option<tty::Shell>, shell_name: Option<String> },
}
```

### 会话接线(`src/gpui_terminal/session.rs`,本篇锚点)

```rust
#[derive(Clone)]
pub struct EventProxy { events: event_mailbox::EventSender }   // stages/remote_reader 臂随 SSH 消亡
impl EventListener for EventProxy { fn send_event(&self, event: Event) { self.events.send(event) } }

#[derive(Clone, Copy)]
pub struct GridSize { pub columns: usize, pub screen_lines: usize }
impl Dimensions for GridSize { ... }

pub struct TerminalSession {
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    pub notifier: Notifier,
    pub shell_pid: u32,                         // 关闭确认判据归 05;M3 仅观测
}
pub type SpawnedSession = (TerminalSession, event_mailbox::EventReceiver);
pub fn spawn(window_size: WindowSize, term_config: Config, options: tty::Options)
    -> io::Result<SpawnedSession>;
pub(super) fn local_options(shell: Option<tty::Shell>, pane_id: PaneId, cwd: Option<PathBuf>)
    -> tty::Options;
// local_options 内:refresh_environment(02 篇)∪ 固定注入(TERM/COLORTERM/TERM_PROGRAM/
//   SLTERM_PANE_ID)∪ Options.conpty_sideload 注入(C-6 已定:壳读 settings 键注入,
//   core 零路径逻辑——M3 设置系统未立,键读取以 06 篇键域约定的默认 true 硬编码,
//   接线点注释登记,06 篇 M6.1 换成真读)
```

### 事件通道与泵(`event_mailbox.rs` + `session_pump.rs`,照抄裁剪)

```rust
// event_mailbox:Wakeup 合并(wake_pending AtomicBool)——重绘唤醒可合并,语义事件全保留
pub(super) fn channel() -> (EventSender, EventReceiver);
// NativePromptState/pebrel_cmd_prompt 族:cmd 语义 prompt 归 02 篇 cmd_prompt(UserVar 键已改
//   slterm_cmd_prompt);M3 视图侧消费面(native prompt 补全)归 08,通道件随 EventSender 整体照抄,
//   observe_input 调用点随 write_input 保留——裁剪只发生在消费端,通道语义零改动
// session_pump:
pub(super) fn attach(rx: EventReceiver, cx: &mut Context<TerminalView>);
//   cx.spawn 消费循环:批合 ≤128、full_batch 1ms 让渡、Exit/实体死亡即断链——全部照抄
```

### 渲染元素(`src/gpui_terminal/element.rs`,本篇锚点)

```rust
pub struct TerminalElement { view: Entity<TerminalView> }
impl TerminalElement { pub fn new(view: Entity<TerminalView>) -> Self }
pub struct TermLayout { cell_width: Pixels, line_height: Pixels, rows: usize, cols: usize,
                        hitbox: Hitbox }
impl Element for TerminalElement {
    type RequestLayoutState = (); type PrepaintState = TermLayout;
    // request_layout: relative(1.) 撑满父级
    // prepaint: shape "M" 采样 → typography 度量 → view.set_layout(尺寸链入口)
    //           → insert_hitbox → TermLayout
    // paint: window.handle_input(ElementInputHandler) 挂 IME → snapshot() 一次锁取
    //        RenderSnapshot → 调色板解析(02 篇缝合点 3)→ 绘制序:bg_runs →
    //        selection_runs → 光标底 → box_glyphs(boxdraw 几何,永不进字形 atlas)
    //        → segments(字形 atlas)→ 光标反色文字;IME marked_text 与选区同帧
}
// 裁剪:math 覆盖层(M9)/补全弹窗族(M9)/link 预览(07/09)/app_cursor 应用光标
//   特判(归 03 屏幕规则联动,M3 删臂保 host 光标)/inline image 层(02 开放问题 4)
```

### keymap(`src/gpui_terminal/keymap.rs` + `keymap/win32.rs`,照抄)

```rust
pub fn encode(ks: &Keystroke, mode: &TermMode) -> Option<Vec<u8>>;
pub(super) fn encode_for_program(ks, mode, program: Option<&str>) -> Option<Vec<u8>>;
//   —— shift_enter_as_lf 的 program 特判(claude 换行回退)随迁;M3 running_program 恒 None,
//   program 参数留位归 03(签名先带,行为退化为 encode)
pub(super) fn preserves_enter_modifiers(ks, mode) -> bool;
pub(super) fn is_native_window_shortcut(ks: &Keystroke) -> bool;   // alt+f4/alt+space 放行系统
pub(super) fn trace_enter(ks, mode, bytes);     // SLTERM_TRACE_ENTER 改名
// win32.rs:win32_encodes_keystroke / virtual_key_of / unicode_char_of /
//   win32_input_record / win32_press_and_release 全量照抄——
//   Ctrl+字母的 C0 文本经 platform::keyboard::native_character(ToUnicode) 恢复
```

### 度量(`view/typography.rs`,照抄)

```rust
pub(super) fn mono_font(family, weight, style, ligatures) -> Font;   // calt/liga/clig/kern 显式
pub(super) fn effective_cell_width(raw, mode, scale, offset_x) -> Pixels;   // 设备像素取整合同
pub(super) fn line_height_for_view(...) -> Pixels;
pub(super) fn cell_metrics(window, cx) -> (Pixels, Pixels);
pub(super) fn startup_cell_metrics_at_scale(scale, cx) -> (Pixels, Pixels);
// CellWidthModeName 归 06 键域;M3 恒取 Compact 默认值(函数签名保 mode 参,调用点传死)
```

### 壳 Settings(`src/app_shell/config.rs`,M3 最小消费面)

```rust
pub struct Settings {           // pebrel gpui_shell/config.rs 裁剪;06 篇 RuntimeSettings
    pub font_family: String, pub font_bold_family: String,      //   的 GUI 形态在 M6 重建,
    pub font_italic_family: String, pub font_bold_italic_family: String, // 本篇只锚 M3 消费字段集
    pub font_size_px: f32, pub ligatures: bool,
    pub palette: Palette,                   // 暗色单份
    pub copy_on_select: bool, pub scroll_speed: f32,
}
impl Settings {
    pub fn load() -> Self;                  // M3 = 常量默认(设置 JSON 读通道归 06);
    pub fn term_config(&self) -> slterm_terminal::term::Config;   // scrolling_history 10000(D02-3)
}
// Global: cx.set_global(settings) 一次性;M3 无热应用(apply_runtime_settings 归 06)
```

## 数据流与状态机

### 启动链(console 子系统,D04-1 落地形态)

```
slterm.exe(console 子系统)
  → helper 拦截(无)/CLI 解析/panic 钩
  → prepare_gui_process():CLI_ENV 自举 + SetConsoleCtrlHandler(None,0) 归一化
      (GUI 子系统前提下的 AttachConsole(ATTACH_PARENT_PROCESS) 臂消亡——
       console 子系统天然继承父控制台;FreeConsole 臂消亡,见改造节 1)
  → logging::initialize
  → run_shell:
      gpui_platform::application().with_assets(SltermAssets)
        .with_quit_mode(QuitMode::LastWindowClosed)   // 关窗即退出,不变量 5
      application.run(|cx| { init(cx); open_main_window(...) })
        → init:内嵌字体注册(text_system().add_fonts,Maple 内嵌随迁——
            跨机器字形同源是截图/UI 测试可比对的前提)→ gpui_component::init
            → Settings::load 一次性 set_global → theme 暗色装载 →
            gpui_terminal::init(Tab 绑定)→ workspace::init(键位)
        → open_main_window → windowing::open_initial_window:
            workspace_window_options(尺寸反推/titlebar/app_id/暗色底)
            → first_frame::defer_show(show=false 延迟出窗)
            → cx.open_window(|window, cx| {
                prepare_initial_grid(度量反推 116×30 目标网格)
                SltermWorkspace::new → new_pane → TerminalView::new
                  → session::spawn → EventLoop::spawn(02 篇读线程起跑)
                Root::new(workspace)                       // gpui-component 根
              })
            → first_frame::present_then_show(WM_PAINT 同步一帧后才 activate——
                首帧黑窗/半构窗口不曝光的因果链照抄)
```

### PTY 输出 → 重绘泵(core 主链的壳侧半段)

```
02 篇 EventLoop 读线程 → EventProxy.send_event → event_mailbox(Wakeup 合并)
  → session_pump::attach 的 cx.spawn 消费环:
      批合 ≤128 → view.process_event(逐事件) → Wakeup 且 output_visible → cx.notify()
  → GPUI 下一帧 Render → TerminalElement.paint → snapshot()(FairMutex lock_unfair 一次锁)
  → RenderSnapshot::capture(02 篇)→ 锁外调色/绘制
```

`process_event` M3 白集:`Wakeup`(通知重绘)/`MouseCursorDirty`/`CursorBlinkingChange`(光标相位)/`Title`(直存 `title`;NEBULA| 协议解析臂归 03/05,M3 不建)/`ResetTitle`/`PtyWrite`(回写 PTY)/`ClipboardStore`(OSC 52 → 系统剪贴板,选择器 `c` 放行 + 焦点门控照 02 篇改造节 2)/`ClipboardLoad`(恒读剪贴板回格式串)/`ColorRequest`(palette query_reply 应答)/`TextAreaSizeRequest`/`ChildExit`/`PtyFailure`/`Exit`(mark_exited 三态文案)/`CwdReport`(存 `cwd` + TitleChanged)/`CommandStart`/`CommandDone`(M3 只驱动光标/展示态最小消费,徽章族归 03/09)/`Notify`(M3 落 toast 角标最小实现,通知漏斗归 09)/`Bell`(视觉闪烁 + 事件,声音归 09)/`Progress`/`InlineImage`/`UserVar`(M3 不消费,静默——消费端归 09/02-开放4/03)。

### 键位分发状态机(改造节 3 的时序图)

```
GPUI KeyDownEvent → TerminalView::on_key_down(焦点 = pane focus_handle)
  0. exited → 全吞(只留系统键)
  1. marked_text.is_some() || is_native_window_shortcut(alt+f4/alt+space) → 放行
     (stop_propagation 后 GPUI Windows 跳过 TranslateMessage——IME 组合与系统
      快捷键的默认处理必须保住,因果链照抄 on_key_down 注释)
  2. ctrl+shift+c → copy_selection(true)(有选区才写剪贴板 + CopyFeedback)
     ctrl+shift+v → paste;其余 ctrl+shift 应用族 → 放行冒泡(workspace 动作,M5 起接)
  3. ctrl+alt+方向 / ctrl+tab 族 → 放行冒泡(M5/M3 后期接)
  4. shift+pageup/pagedown/home/end(主屏)→ scroll_display 本地翻页,stop_propagation
  5. keymap::encode_for_program → Some(bytes) → write_user_key → write_input:
     选区清空 + scroll_display(Bottom) + notifier.notify(bytes) → stop_propagation
     None(纯文本键)→ 落 IME 管道:replace_text_in_range → write_user_text
Ctrl+C 无选区语义:不在快捷键层出现——ctrl_char('c')=0x03 经编码器恒透传(02 篇边界 8)
```

### resize 链(壳侧半段,02 篇 viewport 合同的执行器)

```
TerminalElement::prepaint(bounds) → view.set_layout:
  启动稳定闸(!grid_synced):命中 spawn 网格 → 零下发收口;
    未命中且 < STARTUP_GRID_GRACE(400ms)→ pending_resize + 自有死线定时器;
    超时/命中 → grid_synced=true,一次性 commit
  稳态:ViewportTracker::observe → None 稳态帧零开销
    grid_changed → Msg::ResizeGrid(本地 reflow 逐帧跟手,不通知子进程)
    structural_resize(分屏/zoom,M5 起产)→ 立即 commit_viewport
    否则 → pending_resize + RESIZE_SETTLE_DELAY(150ms)尾沿去抖
           + drag_gesture_active(GetAsyncKeyState VK_LBUTTON)手势门控:
             左键按住不提交,settle 定时器自续——conhost rewrap 漂移的字节取证
             因果链(13 次中间提交漂 7 行)照 layout.rs 注释随迁
  commit_viewport = Msg::Resize(grid 先于 PTY 序,02 篇流边界)
```

### IME 与文本输入(GPUI `EntityInputHandler` 七件,改造节 4)

组合中:`replace_and_mark_text_in_range` 更新 `marked_text` + 光标相位重启;提交:`replace_text_in_range` → `write_user_text`(字节 = UTF-8 文本原样,不经编码器);`bounds_for_range` 回 `ime_bounds`(paint 帧按光标格算);`selected_text_range` 恒回折叠选区(终端无文本框选区语义)。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;改名一律以 01 篇单点表为准)

| pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- |
| `nebula_app/src/gpui_shell/terminal/session.rs` `EventProxy`/`GridSize`/`TerminalSession`/`spawn` | `gpui_terminal/session.rs`;SSH 臂消亡 | GPUI 前台 await 事件的通道形态,本地会话即完整闭环 |
| `nebula_app/src/gpui_shell/terminal/event_mailbox.rs` `channel`/`EventSender`/`EventReceiver` | 全量(NativePrompt 通道随迁,消费端归 08) | Wakeup 合并而语义事件全保留——重绘洪峰不丢 agent 边沿 |
| `nebula_app/src/gpui_shell/terminal/session_pump.rs` `attach` | 裁 SSH stage 泵 | 批合 128 + 1ms 让渡是重绘风暴下的前台公平性 |
| `nebula_app/src/gpui_shell/terminal/keymap.rs` 全族 | 全量改名(NEBULA_TRACE_ENTER→SLTERM_TRACE_ENTER) | 三路径编码器是 win32 输入矩阵基线的被测对象 |
| `nebula_app/src/gpui_shell/terminal/keymap/win32.rs` 全族 | 全量 | uChar=0 的 VK_ESCAPE 被 OpenConsole 翻译层丢弃——记录必须带真实字符 |
| `nebula_app/src/platform/keyboard.rs` `native_character` | `platform/keyboard.rs` 全量 | GPUI 丢掉 Ctrl+C 的 ETX,ToUnicode(0x5 flags)无损恢复 |
| `nebula_app/src/gpui_shell/terminal/view/typography.rs` 全族 | 全量(CellWidthMode 传死 Compact) | 设备像素取整合同:spawn 网格与首帧布局一致,启动零 resize |
| `nebula_app/src/gpui_shell/terminal/colors.rs` `Palette` | 全量;主题装载归 06 | deep-space 暗色默认是 M3 唯一 palette(定位约束:仅暗色) |
| `nebula_app/src/gpui_shell/terminal/mouse_protocol.rs` 全族 | 全量 | 旧壳 mouse.rs 逐字对照的纯函数,SGR/normal/UTF-8 三协议 |
| `nebula_app/src/gpui_shell/copy_feedback.rs` `CopyFeedback` | `gpui_terminal/copy_feedback.rs` 全量 | §5.2 复制反馈的受控时钟时限(TestAppContext 可 advance_clock) |
| `nebula_app/src/gpui_shell/terminal/view/layout.rs` `set_layout` 全链 | 全量(启动闸/尾沿去抖/手势门控) | conhost rewrap 与本地 reflow 路径依赖差异——中间提交即漂移 |
| `nebula_app/src/gpui_shell/terminal/view.rs` 光标闪烁族(`restart_cursor_blink`/`schedule_cursor_blink_tick`/`cursor_blink_allowed`) | 全量 | GPUI 无 BlinkCursor 事件,视图自维护相位 + 双焦点条件 |
| `nebula_app/src/gpui_shell/terminal/view.rs` IME 七件(`EntityInputHandler` impl) | 全量(answer_reader 臂消亡) | 折叠选区 + ime_bounds 是中文 IME 候选窗定位的合同 |
| `nebula_app/src/gpui_shell/terminal/mod.rs` `init`(Tab/Shift-Tab 绑定)+ 遮蔽测试 | 全量改名(KEY_CONTEXT→"SltermTerminal") | 终端必须先于组件库 Root 接住 Tab,否则补齐后光标失焦 |
| `nebula_app/src/gpui_shell/mod.rs` `println!`/`eprintln!` 拦截宏 + `try_write_stderr` | `app_shell/mod.rs` 全量 | GUI 无效标准句柄上的裸 print 会 panic——编译期拦截 |
| `nebula_app/src/gpui_shell/mod.rs` `run_shell`/`init`/`open_main_window` 骨架 | 裁剪重建(update/tray/ssh/mobile/ai_hook 臂全删) | GPUI 消息循环装配序:assets→QuitMode→run 闭包内 init→open_window |
| `nebula_app/src/platform/startup.rs` `report_error`/`primary_display_scale` | 全量 | 开窗前 DPI 预取 = 首窗尺寸不漂的前提 |
| `nebula_app/src/platform/startup/first_frame.rs` `defer_show`/`present_then_show` | 全量 | WM_PAINT 同步一帧后才 activate:首帧黑窗不曝光 |
| `nebula_app/src/gpui_shell/workspace/windowing/startup_geometry.rs` 度量反推族 | 裁 Quick Terminal/配置尺寸臂 | 窗口按 116×30 基准网格定形;亚像素往返不触发二次 SetWindowPos |
| `nebula_app/src/platform/dirs.rs` `data_dir`/`home_dir` | 照 01 篇锚定落位 | OnceLock 缓存 + 「空串视为未设置」过滤器 |
| `nebula_app/src/gpui_shell/assets.rs` `NebulaAssets` | `SltermAssets` 改名 | gpui_component 资产桥(AssetSource 实现) |
| `nebula_app/src/font_install.rs` `REQUIRED_FONT_BYTES`/`imported_font_files` | 裁剪(导入字体目录归 06;内嵌 Maple 保留) | 系统等宽字体无 NF 图标码点,内嵌字形同源是兜底 |
| `nebula_app/src/logging.rs`/`panic.rs` 骨架 | 裁剪(legacy 壳臂删) | 启动期错误必须上弹面;logger 先于一切子系统 |
| `nebula_app/src/gpui_shell/terminal/view/pointer.rs` 选择/滚动/鼠标上报族 | 裁剪(补全弹窗/链接臂删) | grid_point 钳制 + 拖选自动回滚 + MOUSE_MODE 消费序 |
| `nebula_app/src/gpui_shell/terminal/view.rs` `write_input`/`write_bytes`/`shutdown`/`term_mode` | 裁剪(补全/镜像记账删) | 输入即回底 + 清选区 + notify 的单点出口 |
| `nebula_app/src/gpui_shell/terminal/view.rs` `copy_selection`/`paste_now_impl` | 全量(copy_on_select 设置键归 06,M3 默认关) | 选中才写剪贴板;粘贴过滤 `\x1b[201~` 防注入截断 |
| `nebula_app/src/gpui_shell/workspace.rs` `new_pane`/`inherited_grid` 形态 | M3 雏形(单 pane;inherited_grid 退化为 initial_grid) | PTY 出生即目标几何 = 启动零 resize 合同的宿主半段 |
| `architecture/notes/nebula_app/gpui_shell/terminal/` 三条(2026-09-19 hidden-output-invalidation、2026-09-20 ligatures-on-a-fixed-grid、2026-09-25 cursor-motion) | 改名归 `architecture/notes/slterm_app/gpui_terminal/` | 隐藏输出不失效缓存/连字固定网格/光标动画的因果档 |
| `architecture/notes/nebula_app/gpui_shell/workspace/` 两条(2026-09-22 first-frame-startup、nonblocking-first-frame) | 改名归 `slterm_app/app_shell/` | 首帧 present-then-show 的取证档 |
| `architecture/notes/nebula_app/gpui_shell/2026-09-19-render-resource-lifetimes.md` | 改名归 `slterm_app/` | GPUI 实体/渲染资源生命周期纪律 |

### 改名映射引用

本篇一切改名以 01 篇「改名映射单点表」为唯一权威,不另立。本篇直接消费点:A 节 `nebula_app` → `slterm_app`;C 节 `PEBREL_CLI` → `SLTERM_CLI`、`PEBREL_PANE_ID` → `SLTERM_PANE_ID`;P-3 AUMID `com.slterminal.terminal`(开窗 `app_id`);QA 族按需新立:`NEBULA_BOOT_TRACE` → `SLTERM_BOOT_TRACE`、`NEBULA_RESIZE_TRACE` → `SLTERM_RESIZE_TRACE`、`NEBULA_TRACE_ENTER` → `SLTERM_TRACE_ENTER`、`NEBULA_PTY_RECORD` → `SLTERM_PTY_RECORD`(02 篇已锚 `SLTERM_BOOT_TRACE`,本篇登记其余三件的壳侧消费点)。模块名映射:`gpui_shell` → `app_shell`(壳总面)、`gpui_shell::terminal` → `gpui_terminal`(终端切片)——层级压平一格,因果:pebrel 的 gpui_shell 以「区别于 legacy 壳」得名,slTerminal 无旧壳,`gpui_shell` 名失去对照对象。

### 缝合点

1. **`Term<EventProxy>` 泛型通道**(02 篇缝合点 1 的壳侧实现):`EventProxy: EventListener` 送 `event_mailbox`;core 不感知 GPUI。core→壳唯一反向边。
2. **viewport 协议消费**(02 篇缝合点 2):`TerminalElement::prepaint` 每帧喂 `ViewportTracker`;过期 revision 壳丢弃(tracker 内部单调保证,壳侧不再判)。
3. **快照调色**(02 篇缝合点 3):`RenderSnapshot` 的 `Color` 经 `Palette::resolve` + `color_overrides` 解析;`box_glyphs` 走 boxdraw 几何直绘,永不进字形 atlas;应用写死颜色的主题矫正(`TerminalColorResolver`)归 06,M3 不做矫正(仅暗色单主题,矫正无对照面)。
4. **键盘三路径**(02 篇缝合点 4):core 出 `TermMode`,本篇 keymap 为唯一编码器;win32_input_matrix 基线守三路径端到端(M3 出口)。
5. **per-pane 环境**(02 篇缝合点 5):`local_options` 组装 `Options.env`;`SLTERM_PANE_ID` 注入时机 = `convert_custom_env` 末尾叠加,值 = `PaneId` 十进制串(03 篇环境契约的 M3 最小子集,其余变量族归 03)。
6. **复制合同**(02 篇缝合点 8):`selection_to_string` → `copy_selection` → `cx.write_to_clipboard`;CopyFeedback 为 §5.2 反馈时限的单一权威。
7. **pane 矩形回写**(05 篇契约):`pane_bounds`/`split_bounds` 机制 M3 建立(canvas 探针 prepaint 回写);M3 单 pane 满卡直渲,回写值即内容矩形,M5 分屏递归直接复用,不改机制。
8. **terminal_registry**(05 篇缝合点 2):M3 立模块级单例本体(`register`/`get`/`remove`/`subscribe` + `_reset`),`PaneSessionMeta` M3 字段最简(`shell_kind` 归 02;`prompt_ready`/`agent` 归 03 回填);写入点 = `new_pane`/关 pane 两处。
9. **壳事件队列 × RuntimeHub**(04 篇):`SltermShellEvent::RuntimeControl` 变体与 `start_shell_event_pump` 的 120ms drain 形态是 04 篇 M7 `RuntimeCallback` 的预埋缝——M3 泵存在、变体存在、无生产者;M7 只接 `RuntimeServer::spawn_callback` 的生产端。
10. **Settings 键域**(06 篇):壳 `Settings` M3 字段集(font/palette/copy_on_select/scroll_speed)是 06 篇 `RuntimeSettings` GUI 化的消费白名单;M3 `Settings::load()` 常量默认,M6 换 JSON 读通道时字段名/默认值零漂移(双侧字面量测试归 06)。
11. **窗口三钮与标题栏**:TitleBar 自绘归 05 篇(05 已裁归属);M3 用 `TitleBar::title_bar_options()` 系统底座 + `brand::NAME` 窗口标题,自绘层 M5 接入;窗口特效/壁纸归 09(M3 `window_background` = 不透明暗色)。

缝合面 11 条已闭合(02 篇 5 条、05 篇 2 条、04/06/09/11 各对应条),无遗漏缝合点。

## 改造 / 移植 / 新建设计

### 1. console 子系统启动链改造(C-5b 承接,D04-1 落点)

pebrel 原链以 GUI 子系统为前提:`main.rs` 显式 `AttachConsole(ATTACH_PARENT_PROCESS)`(windows 子系统不自动挂父控制台,无父控制台静默失败);`prepare_gui_process` 内 `NEBULA_DETACHED_LAUNCH` 臂 `FreeConsole()`(启动器退出不拖垮窗口);`console::prepare_console_for_gui` 归一化 `SetConsoleCtrlHandler(None, 0)`。console 子系统下语义反转,三刀:

- **AttachConsole 臂消亡**:console 子系统进程出生即继承父控制台(双击启动由系统分配新控制台),显式 Attach 失去对象。
- **FreeConsole 臂消亡**:console 子系统下从 pwsh/cmd 启动,父 shell 同步等待子进程退出;`FreeConsole` 会扔掉继承的控制台、断开标准流。D04-1 裁决理由即「标准流正确性优先」(AI CLI agent 是 stdio 消费者),Detach 语义整体不迁,`SLTERM_DETACHED_LAUNCH` 不立新名。
- **`SetConsoleCtrlHandler(None, 0)` 保留**:NULL-handler ignore 标志会被 ConPTY 子 shell 继承,必须在任何 worker/终端子进程存在前归一化一次,永不围绕 spawn 翻位(pebrel `console.rs` 注释因果链照抄)。

保留件:CLI_ENV 自举(`set_var(current_exe)`,便携版不在 PATH 时非 PTY 子进程的 exe 回退)改名 `SLTERM_CLI`;`console` 子模块(`console.rs` + `console_tests.rs`)整文件随迁。

退出侧:pebrel legacy 壳收尾的 `FreeConsole` 臂不迁(GPUI 路径本无此调用;console 子系统下进程退出由系统回收控制台)。函数名 `prepare_gui_process` 不变,内部只剩两动作。「从 pwsh/cmd 直接敲 `slterm` 会阻塞父 shell 直至关窗」属发布级行为意外,登记开放问题 1,本篇不裁。

### 2. 状态权威雏形:M3 直读 Workspace,M7.2 facade 缝接管(C-2 承接)

裁决照 00-roadmap M3 行校准形态:**M3 不建 RuntimeHub、不建投影层**;`SltermWorkspace` 直持 pane 实体与展示态(title/cwd/error/exited),是唯一 pane 属主(05 篇不变式)。M3 无侧栏/托盘等消费面;M7.2 由 04 篇 RuntimeHub 以 facade 缝接管投影。预留缝三处,只留缝、不埋 hub 本体:

1. **壳事件泵缝**:`SltermShellEvent::RuntimeControl` 变体 + 120ms drain 泵(缝合点 9);M7 只接 `RuntimeServer::spawn_callback` 生产端。
2. **投影读缝**:M7.2 投影函数(Workspace → RuntimeSnapshot,形状由 04 篇锚定、函数 05 篇共建)是读-only facade;M3 字段语义与 05 篇终态严格一致(「与 M3 的接口契约」扩不改),投影函数落地不改 M3 字段。
3. **元数据单点缝**:`terminal_registry` 写入点 M3 即收敛在 new_pane/关 pane 两处(缝合点 8);04 篇 `pane.read`/`pane.exec` 经它定位宿主,M7 不加新写入点。

迁移面枚举(M7.2 接管时实际改动):publish 调用点插入三处已有订阅出口(new_pane/close/on_terminal_event);hub 本体新建属 04 篇 M7.2;侧栏/托盘/外部客户端 M3 不存在,无旧读取端返工。

不选「M3 携最小 hub」的理由:hub 的 revision/盖戳/订阅有界(cap 16)是协议契约,无消费者先立 = 先背契约税再在 M4–M6 叠加中返工;facade 缝是函数调用点而非类型面,后续阶段守「字段语义与终态一致」即不在返工面内。

### 3. 键位合同落地:分发判定唯一点 = `on_key_down` 快捷键层

定位约束 7 的机械落点(pebrel `view.rs` `on_key_down` 同构照抄),编码器层零特例:

- **`ctrl+shift+c` → `copy_selection(true)`**:有选区 → `selection_to_string`(02 篇唯一取词)→ 写系统剪贴板 → CopyFeedback(§5.2 时限);无选区 → 不写剪贴板、不反馈,`stop_propagation` 照吞。Ctrl+Shift+C 任何情况下不下发 PTY(pebrel 同语义;无选区的静默吞是否给负反馈见待沉淀 D13-1)。
- **`ctrl+shift+v` → `paste`**:bracketed-paste 包装,过滤 `\x1b[201~` 防注入截断。
- **其余 ctrl+shift 应用族**(t/w/b/p/f/d/s/g/o/enter/pageup/pagedown)与 **ctrl+alt+方向 / ctrl(+shift)+tab** 一律放行冒泡:workspace 动作 M5 起接,M3 无绑定冒泡即终。漏放行 = CSI 序列误注 PTY(pebrel 注释因果链照抄)。
- **Ctrl+C 不在快捷键层出现**:有/无选区一律经 keymap 编码 `ctrl_char('c')`=0x03 恒透传(02 篇边界 8);GPUI 丢弃的 ETX 文本由 `native_character`(ToUnicode flags=0x5)恢复。

kitty 协议与 02 篇接缝:core 持 kitty 模式栈(`TermMode` 五标志位,02 篇键盘契约);壳侧 `encode`/`encode_for_program` 是模式位唯一消费者,依位分派三路径(kitty 序列 / Win32 input record / legacy)。壳不改栈、不缓存模式——`term_mode()` 每键现读(锁内快照),模式变化经 core Wakeup 事件自然驱动,键路径无第二同步面。

`shift_enter_as_lf` 的 program 特判(claude 换行回退)随迁;M3 `running_program` 恒 None(agent 检测属 03 篇),签名留位、行为退化为 `encode`。原生宿主键(alt+f4/alt+space)= `is_native_window_shortcut` 放行系统;IME 组合中(`marked_text.is_some()`)全键放行——`stop_propagation` 后 GPUI Windows 跳过 TranslateMessage,输入法组合与系统快捷键的默认处理必须保住。

### 4. IME 与 win32 输入矩阵基线 M3 承接(C-4)

IME 七件(`EntityInputHandler` impl)照抄:组合中 `replace_and_mark_text_in_range` 更新 `marked_text` + 光标相位重启;提交 `replace_text_in_range` → `write_user_text`(UTF-8 字节原样、不经编码器);`bounds_for_range` 回 `ime_bounds`(paint 帧按光标格算,候选窗跟光标);`selected_text_range` 恒回折叠选区(终端无文本框选区语义)。IME 合成的真桌面路径不可自动化,进 test-exemptions 实机验收点(11 篇登记口径);本篇用例只锚虚拟窗口可测面(marked_text 置清、ime_bounds 数值)。

win32 输入矩阵基线(C-4 承接):M2 纯库态无可驱动 exe,基线建基与常挂比对移至 M3 出口(00-roadmap M2 出口已收窄为「脚本 GPUI 化改造编译过」)。M3 承接三面:

- 被测对象 = 本篇 keymap 三路径编码器端到端(普通键 + 扩展键 + 裸修饰键零字节 + 哨兵键,11 篇矩阵节形态)。
- 驱动 = `Start-Process -WindowStyle Minimized` + PostMessage 逐键 + 探针收字节,不抢焦点;exe 默认 `target\debug\slterm.exe`,M3 起成为首个可驱动目标。
- 建基时机 = M3.4 渲染闭环后、M3 出口前 `-Record`;基线文件 git 追踪,语义变更时「代码 + 基线」同提交(11 篇基线治理状态机)。

### 5. 窗口回放消费登记(D05-1:M3 不写不回放,回放消费属 M5.5)

口径核对(05 篇「与 M3 的接口契约」):M3 期 session 不持久化——M3 既不写 `WindowState` 也不回放,关窗丢布局是过渡期预期态;D05-1 已裁「回放」,回放消费在 05 篇 M5.5 快照闭环。本篇登记 M3 侧既有数据源,M5.5 接入零预埋改动:

- **写入端采集源**:`SltermWorkspace` 持窗口句柄 + `pane_bounds` 回写机制,窗口尺寸/最大化态在 M3 开窗链已可读;M5.5 的 1Hz 快照与 `save_final` 直接采,M3 不加字段。
- **回放端单缝**:`workspace_window_options` 是窗口定形唯一函数(09 篇首窗几何域同缝);M5.5 回放 = 定形前读 `Session.window` 覆盖 `preferred_size`,不改 M3 调用序。

M3 恒走默认定形(116×30 基准反推 + 首窗 DPI),与 D05-1 不冲突:回放是 session 恢复链行为,session 本体 M5.2 才落 `slterm_app`。

## 测试点清单

> 引测试一律例名;测试基础设施形态(虚拟窗口 builder/键绑定双形态/矩阵夹具/基线治理)属 11 篇,本篇只列 M3 领域用例。pebrel 随迁用例例名照抄不改。

| 测试 | 层级 | 机制 |
| --- | --- | --- |
| 虚拟窗口设施冒烟:feature 链开窗 + Root 挂载 + draw 不 panic | #[gpui::test] | 新建;M3 开工第一步(C-9 闭环),补丁形态与设计假设不符则此例起不来 |
| 终端渲染关键路径:注入字节 → process_event → paint 帧可见;`debug_bounds` 取真实坐标 | #[gpui::test] | 新建;00-roadmap M3 出口主件 |
| keymap 解析矩阵:`encode` × TermMode 全键族(含裸修饰零字节、哨兵键、`native_window_shortcuts_follow_the_host_window_policy`) | L1 单测 | 迁移(pebrel keymap `mod tests`) |
| 键位真实分发:`ctrl+shift+c` 有选区写剪贴板 + CopyFeedback `advance_clock` 时限;无选区剪贴板不动且 PTY 零字节 | #[gpui::test] | 迁移扩(pebrel `native_window_shortcuts_propagate_through_root_and_terminal` 同族) |
| Ctrl+C 合同:有/无选区均透传 0x03,剪贴板不动 | #[gpui::test] + L1 | 新建;定位约束 7 端到端锚 |
| `ctrl+shift+v` bracketed paste 包装 + `\x1b[201~` 过滤;shift+pageup 主屏翻页 / 备用屏透传编码;alt+f4 放行;marked_text 中全键不编码 | #[gpui::test] | 迁移 |
| IME 七件可测面:marked_text 置/清、提交字节原样、ime_bounds 数值 | #[gpui::test] | 新建 |
| event_mailbox:Wakeup 合并(wake_pending)语义事件全保留;session_pump 批合 ≤128、Exit 断链 | L1 | 迁移 |
| resize 链:启动闸三态(命中零下发 / grace pending / 超时 commit)、150ms 尾沿去抖、手势门控 | L1 | 迁移(pebrel layout 族) |
| 启动零 resize:spawn 网格 == 首帧布局网格(typography 设备像素取整端到端) | #[gpui::test] | 迁移 |
| first_frame:defer_show 置 show=false、present_then_show 时序 | L1 | 迁移 |
| 复制宽字符:命中宽字符任一格按整字复制(02 篇 selection 合同的壳侧端) | #[gpui::test] | 迁移 |
| console 归一化:`prepare_console_for_gui` NULL-handler 用例随迁(承载目标形态见开放问题 2) | L1 | 迁移(pebrel `console_tests.rs`) |
| win32 矩阵:`-Record` 建基 + 常挂比对逐字节一致 | PS1 脚本层 | 11 篇夹具;M3 出口件 |
| feature on/off 双形态:`gpui-test-support` 开/关两形态编译 + 测试绿 | cargo | 11 篇门禁 |

## 阶段归属与出口标准

本篇全部属 **M3 app 骨架点亮**(00-roadmap M3 行),细分四步,出口全为可机验项:

**M3.1 启动链与 console 改造**:`main.rs` 骨架(CLI 两参/panic 钩)+ `prepare_gui_process` 语义反转(改造节 1)+ logging/dirs + `SltermAssets`。出口 = `cargo check` 过;禁名门禁过;**虚拟窗口设施冒烟例绿(M3 开工第一步,C-9 闭环)**。

**M3.2 渲染元素挂接**:windowing(startup_geometry/first_frame)+ `SltermWorkspace` 雏形 + `TerminalView` 骨架 + `TerminalElement`/typography/colors + session/event_mailbox/session_pump(ConPTY 直连默认 shell)。出口 = 虚拟窗口 UI 测试(终端渲染关键路径:注入字节回显)过;启动零 resize 锚绿。

**M3.3 输入与键位**:keymap 三路径 + `platform::keyboard` + `on_key_down` 分发(改造节 3)+ IME 七件 + mouse_protocol + copy_feedback + pointer 族。出口 = keymap 双形态(解析矩阵 + 真实分发)全绿;Ctrl+Shift+C / Ctrl+C 合同用例绿。

**M3.4 冒烟闭环与基线建基**:win32 矩阵 `-Record` 建基 + 常挂比对;console 归一化用例随迁(承载形态按开放问题 2 裁决);feature on/off 双形态。出口 = 00-roadmap M3 出口三项全绿:虚拟窗口 UI 测试(终端渲染关键路径)过 + win32 输入矩阵基线建基 + 常挂比对过 + `cargo test` 全绿。

M3 总出口对齐 00-roadmap M3 行;RuntimeHub 投影不在本阶段(属 M7.2)。首个可运行态的过渡期缺席面(无分屏/无 AI/无设置页/布局不持久)照 roadmap 不可用清单,本篇不重复登记。

## 待沉淀决策

> [待沉淀] **D13-1 · `ctrl+shift+c` 无选区语义(静默吞 vs 负反馈 vs 透传中断)**。真实权衡:pebrel 语义 = 无选区也 `stop_propagation` 吞掉(复制键永不入 PTY,按下无感知);负反馈(闪烁/提示)可感知但引入新 UI 面;透传 `\x03`(kitty copy_or_interrupt 同形)违背「ctrl+shift 族永不进 PTY」的冒泡纪律,且与 Ctrl+C 恒中断形成同果双键。难逆点:发布后变更 = 用户肌肉记忆层面的可感知行为变更。意外因素:AI CLI 场景 Ctrl+C 高频(取消生成),误把 Ctrl+Shift+C 当中断按的用户在「吞」语义下会误判应用卡死;CopyFeedback 只在有选区时反馈,无选区静默与 §5.2 反馈合同存在感知缺口。截止:M3.3 键位落地前定;默认照抄 pebrel 吞。

## 开放问题

1. **console 子系统阻塞父 shell 的后果告知方式**(C-5a,待用户裁决):从 pwsh/cmd 直接敲 `slterm` 启动 GUI 人格,父 shell 同步等待直至关窗——「从终端里开终端卡住原终端」,比 DECISIONS 已接受的闪控制台窗更影响自家目标用户(AI CLI 重度 shell 用户)。候选缓解:快捷方式/开始菜单为主入口、文档明示 `Start-Process slterm` 脱壳启动、或启动时检测父控制台打一行提示。默认:文档明示,不检测不拦截。
2. **`windows_console_startup` 对照目标存废与子步归属**(C-5c,待用户裁决):11 篇锚定该对照目标(显式 console 子系统测试 target 挂载 `console.rs` + `console_tests.rs`,守 Ctrl+C 继承语义),并记「被测对象(console 启动准备)归 09 篇」;但 09 篇无对应改造子步,本篇改造节 1 已把 `prepare_gui_process` 改造锚在 M3.1。归属缝隙,且对照目标在 console 本体下的理据变化(本体与目标同形态后是否仍独立必要)待裁。默认:对照目标保留作独立回归锚,子步由本篇 M3.4 承接。
3. **M3–M6 二次启动过渡形态**:单实例移交(loopback + runtime.port 锁)属 04 篇 M7;M3 无 probe 臂,二次启动的自然形态 = 双进程各自单窗互不感知(违背单窗定位的过渡态),或显式拒绝(第二实例提示退出)。默认:放行双进程(照抄 pebrel 砍 mux 臂的自然形态),M7 移交收口;裁决后回改 `main.rs` 注释的「拒绝」表述。
4. **壳层类型行补登 01 篇锚点表**(执行项):边界条 6 登记的 `TerminalView`/`TerminalElement`/`TerminalSession`/`EventProxy`/`SltermWorkspace`/壳 `Settings` 等类型锚点行,M3 落地时补登 01 篇类型锚点表。
