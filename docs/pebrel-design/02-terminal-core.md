# 02 终端核心详细设计

> pebrel-design 分片 02/12。上游 spec:`docs/pebrel-refactor/SPEC.md`（总表）+ `docs/pebrel-refactor/02-terminal-core.md`（spec 分片，含采纳点编号）；骨架:`docs/pebrel-design/00-roadmap.md`；改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是「终端 `Grid` / revision / OSC 类型」的锚点归属篇（01 篇类型锚点表已登记），三层渲染合同的快照类型在此签名级定义，他篇只许 `use`。引 pebrel 一律符号名 + 文件路径（baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死），禁行号。

## 目标形态

`slterm_terminal` = 纯领域 core crate：拥有 grid、VT、PTY、输入编码、OSC 嗅探、选择/搜索、渲染合同快照；**不依赖 GPUI / 视图 / 窗口状态 / 产品概念**（`AGENTS.md` 纪律随迁）。它是替换 xterm.js 的内核：VT 解析、网格、scrollback、语义 prompt、渲染快照全在 core；GPUI 壳只消费 `RenderSnapshot` + `Event`，并把键盘/鼠标/布局观察翻译成 `Msg` 与 `CellMetrics`。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `src/index.rs` | pebrel 照抄 | `Line`/`Column`/`Point`/`Side`/`Range` 坐标系 |
| `src/term/`（mod/cell/clear/color/damage/keyboard/prompt/redraw_anchor/renderable/search） | pebrel 照抄 | `Term` 模型、VT 语义、损伤、kitty 模式栈、语义 prompt |
| `src/grid/`（mod/resize/row/storage） | pebrel 照抄 | 环形 scrollback、reflow、ConPTY 行锚定 |
| `src/render.rs` + `src/render/boxdraw.rs` | pebrel 照抄 | 三层渲染合同（本篇锚点）、内建几何字形 |
| `src/osc_cwd.rs` | pebrel 照抄 | OSC tee 嗅探状态机（OSC 类型锚点） |
| `src/event.rs` | pebrel 照抄 | `Event` 枚举 = core→壳边界契约 |
| `src/event_loop.rs` | pebrel 改造 | 读循环 + `StreamProcessor` + resize 流边界；slTerminal 微批/pending 并入 |
| `src/selection.rs` / `src/vi_mode.rs` | pebrel 照抄 | 四态选择、vi  motion |
| `src/sync.rs` / `src/thread.rs` | pebrel 照抄 | `FairMutex`、线程命名 |
| `src/tty/mod.rs` | pebrel 裁剪 | `Options`/`Shell`/trait 面；`connection_shell()` 与 unix 模块不迁 |
| `src/tty/windows/`（conpty/blocking/child/cmd_prompt/environment/spawn/mod） | pebrel 缝合 slTerminal 五件套 | ConPTY 生命周期全链 |
| `src/lib.rs` | 改造 | 模块面 + `pty_trace`（改名 `SLTERM_BOOT_TRACE`） |

`tty/unix.rs`、`src/tty/connection.sh`/`connection.ps1`、`completion.sh`/`completion.ps1`、`proxy.ps1`、`test/bash_input.rs`、`proxy_tests.rs` 不迁（spec 分片 02 不采纳点 1/10 与 unix 全砍）。

### 对 app 暴露的公共面（`lib.rs` 出口，全 crate 仅此集合）

`event::{Event, EventListener, WindowSize, Notify, OnResize}`、`event_loop::{EventLoop, EventLoopSender, Msg, StreamProcessor}`、`grid::{Grid, Dimensions}`、`index::{Line, Column, Point, Side, Range}`、`osc_cwd::{CwdSniffer, OscEvent}`、`render::{CellMetrics, TerminalViewport, ViewportTracker, ViewportChange, SnapshotConfig, RenderSnapshot, boxdraw}`、`selection::{Selection, SelectionType, SelectionRange}`、`sync::FairMutex`、`term::{Term, TermMode, Config, Osc52, ClipboardType, Colors, cell::Flags}`、`tty::{Options, Shell, ChildEvent, EventedPty, EventedReadWrite}` 及 windows 侧 `Pty`、`refresh_environment`、`ConptyStatus`。

### 与 GPUI 渲染层的缝合（viewport 协议 → GPUI 自绘）

```
GPUI 布局帧 ──content rect(logical px)──▶ ViewportTracker::observe
                                              │ Option<ViewportChange>
                    ┌─────────────────────────┤ viewport.revision 单调
                    ▼ grid_changed            ▼ pixel_changed（行列不变字号变也报）
              Msg::ResizeGrid（拖拽期逐帧 reflow，不通知子进程）   Msg::Resize（落定，通知子进程）
                    └───────────────▶ EventLoop ──resize 流边界──▶ Term::resize + ResizePseudoConsole

EventLoop 读侧 ──StreamProcessor::feed──▶ FairMutex<Term> ──Event::Wakeup──▶ 壳
壳渲染帧 ──RenderSnapshot::capture(&term, &cfg)──▶ segments/bg_runs/box_glyphs/cursor（纯数据，无引用）
                                                  └─▶ GPUI element：palette 解析 Color → 字形 atlas / boxdraw 几何
```

硬规则（编码进类型与测试，照抄 `src/render.rs` 文件头合同）：终端内容只有 cell grid 一种表示；快照携单调 `revision`，过期 resize 永不覆盖新 resize；上报 PTY 的像素恒为 `cols × cell_width` 精确积；行列不变但像素变必报。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用；本篇涉及 WebView / IPC / Tauri / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **vte 0.15 走 crates.io，不 fork**（spec 采纳点 1）。OSC 缺口由 `CwdSniffer` tee 永久补齐；DEC 2026 同步更新由 vte 原生 `sync_timeout`/`stop_sync` 承担。任何「改解析器」的冲动都是否决项。
3. **core 纪律**（`AGENTS.md` 随迁为 `slterm_terminal/AGENTS.md`）：不依赖产品视图/窗口状态/GPUI；输入修复须分别验证原生控制台、翻译字节流、协商协议三方读取；字节流/解析/回放测试覆盖分块边界、静默源、EOF；热路径变更附代表性成本证据；重大协议/缓冲/线程/所有权决策登记 `architecture/notes/slterm_terminal/<capability>/`（改名，因果 note 九段结构归 01 篇）。
4. **类型锚点归属**：`Grid`/`Line`/`Column`/`Point`/`TermMode`/`OscEvent`/`CwdSniffer`/`Event`/`RenderSnapshot` 及成员/`TerminalViewport`/`ViewportTracker`/`CellMetrics`/`Selection*`/`Colors`/`Flags`/`WindowSize` 唯一定义在本篇；他篇（05 布局、06 设置、08 补全、09 系统、11 测试）只许 `use` 或经 facade 传参，禁重定义、禁别名漂移。`Term<T>` 的泛型参数 `T: EventListener` 是 core 反向通知壳的唯一通道，壳侧实现归 M3 app 骨架（本篇给出 trait 契约，不定义壳实现）。
5. **平台分支收敛**：业务 `#[cfg(windows)]` 只存在于 `tty/windows/`；core 其余部分平台无关。`terminfo` 探测不建（spec 不采纳点 11），`TERM=xterm-256color`/`COLORTERM=truecolor`/`TERM_PROGRAM=slterm` 定死注入。
6. **热路径形态即基准**：1MiB 读缓冲 / 64KiB 持锁上限 / `Storage` 环形零搬移 / `Row` occ 占用段 / `RenderSnapshot` 单趟无引用——改动须按 core 纪律附成本证据，禁止为「整洁」回退已验证形态（spec 优化方向 9）。
7. **DA1/DSR/CPR 红线**：禁盲注（没有等待者的应答字节 = 杂散键）；每次查询必答、谁问谁答；Win10 键事件模式下 CPR 字节即毒（ADR-0022 教训，缝合进本篇设计）。
8. **复制语义定位约束**：Ctrl+Shift+C = 复制（选中才写剪贴板），Ctrl+C = 恒透传 `\x03` 中断。core 只保字节与 `selection_to_string` 语义；剪贴板写入、反馈 UI 归壳。

## 关键类型与签名

> 均为草稿级签名（照抄部分的签名与 pebrel 一致，改名点已标）。`T: EventListener` 为反向通知通道。

### 坐标系（`index.rs`，照抄锚点）

```rust
// index.rs —— 全 core 与渲染合同共用的坐标系
pub struct Line(pub i32);          // 可为负：scrollback 向上越界
pub struct Column(pub usize);
pub struct Point<L = Line> { pub line: L, pub column: Column }
pub enum Side { Left, Right }      // 选区字符间边界
pub struct Range<L = Line> { pub start: Point<L>, pub end: Point<L> }
pub enum Direction { Right, Left, Up, Down }
```

### Grid 族（`grid/`，照抄锚点）

```rust
// grid/mod.rs
pub struct Grid<T> {
    lines: Vec<Row<T>>,           // 逻辑环形视；物理存储在 Storage
    raw: Storage<T>,
    cols: usize, max_scroll_limit: usize,
    display_offset: usize,        // 视口向 scrollback 的偏移
    scrolled_out: usize,          // 滚出视口的绝对行数（prompt mark/图片锚定的稳定行号基）
    scroll_limit: usize,
    scroll_region: Range<Line>,
    reflow_on_grow: bool,
}
impl<T: GridCell> Grid<T> {
    pub fn new(lines: usize, columns: usize, max_scroll_limit: usize) -> Grid<T>;
    pub fn set_reflow_on_grow(&mut self, enabled: bool);
    pub fn update_history(&mut self, history_size: usize);
    pub fn scroll_display(&mut self, scroll: Scroll);
    pub fn clear_history(&mut self);
    pub fn display_iter(&self) -> GridIterator<'_, T>;
    pub fn display_iter_from(&self, origin: Line, lines: usize, cols: usize) -> GridIterator<'_, T>;
    pub fn iter_from(&self, point: Point) -> GridIterator<'_, T>;
    pub fn display_offset(&self) -> usize;
    pub fn scrolled_out(&self) -> usize;      // 绝对行号稳定性的根基
}
pub trait Dimensions { fn total_lines(&self) -> usize; fn screen_lines(&self) -> usize; fn columns(&self) -> usize; }
pub trait GridCell: Sized { fn is_empty(&self) -> bool; fn len(&self) -> usize; }
pub struct Indexed<T> { pub point: Point, pub cell: &T }
pub enum Scroll { Lines(isize), PageUp, PageDown, Top, Bottom }
pub trait BidirectionalIterator: Iterator { fn prev(&mut self) -> Option<Self::Item>; }

// grid/storage.rs —— 环形缓冲：zero 偏移 + rotate 模加，滚动零行搬移
pub struct Storage<T> { inner: Vec<Row<T>>, zero: usize, ... }
// grid/row.rs —— 占用计数：occ 之后单元格恒等于模板
pub struct Row<T> { inner: Vec<T>, occ: usize, ... }
// grid/resize.rs
impl<T: GridCell> Grid<T> {
    pub fn resize(&mut self, lines: usize, cols: usize);          // 通用 reflow
    pub fn resize_conpty(&mut self, lines: usize, cols: usize);   // ConPTY 行锚定变体
}
```

### Term 模型与模式（`term/mod.rs`，照抄锚点）

```rust
// bitflags! TermMode: u32 —— 位全集（协议号注释随迁，一个不漏）：
//   SHOW_CURSOR(1) APP_CURSOR(<<1) APP_KEYPAD(<<2) MOUSE_REPORT_CLICK(<<3)
//   BRACKETED_PASTE(<<4) SGR_MOUSE(<<5) MOUSE_MOTION(<<6) LINE_WRAP(<<7)
//   LINE_FEED_NEW_LINE(<<8) ORIGIN(<<9) INSERT(<<10) FOCUS_IN_OUT(<<11)
//   ALT_SCREEN(<<12) MOUSE_DRAG(<<13) UTF8_MOUSE(<<14) ALTERNATE_SCROLL(<<15)
//   VI(<<16) URGENCY_HINTS(<<17)
//   kitty 五位: DISAMBIGUATE_ESC_CODES(<<18) REPORT_EVENT_TYPES(<<19)
//     REPORT_ALTERNATE_KEYS(<<20) REPORT_ALL_KEYS_AS_ESC(<<21) REPORT_ASSOCIATED_TEXT(<<22)
//   WIN32_INPUT_MODE(<<23, DECSET 9001) COLOR_SCHEME_UPDATES(<<24, DECSET 2031)
//   聚合: MOUSE_MODE = CLICK|MOTION|DRAG ; KITTY_KEYBOARD_PROTOCOL = kitty 五位
// Default = SHOW_CURSOR | LINE_WRAP | ALTERNATE_SCROLL | URGENCY_HINTS

pub struct Term<T> {
    grid: Grid<Cell>, inactive_grid: Grid<Cell>,   // 主/备双网格，swap_alt 互换
    mode: TermMode, selection: Option<Selection>,
    vi_mode_cursor: ViModeCursor,
    colors: Colors, tabs: TabStops,
    title: Option<String>, title_stack: Vec<Option<String>>,
    keyboard_mode_stack: Vec<KeyboardModes>,       // kitty 栈：主/备各一份
    inactive_keyboard_mode_stack: Vec<KeyboardModes>,
    damage: TermDamageState,
    slterm_prompt_marks: VecDeque<usize>,          // 改名：nebula_prompt_marks
    slterm_prompt_active: bool, slterm_prompt_input: Option<(usize, Column)>,
    bringup_da1_pending: bool,                     // DA1 priming 一次性吞重答
    cursor_style / cursor_blinking_override / color_scheme_dark / config / event_proxy ...
}
pub struct Config {
    pub scrolling_history: usize,        // 默认 10000（终值见待沉淀 D02-3）
    pub default_cursor_style: CursorStyle,
    pub vi_mode_cursor_style: Option<CursorStyle>,
    pub semantic_escape_chars: String,
    pub kitty_keyboard: bool,
    pub osc52: Osc52,
    pub suppress_bringup_da1: bool,      // priming 会话置位，语义注释随迁
    pub conpty_resize: bool,             // ConPTY 行锚定 resize
}
pub enum Osc52 { Disabled, OnlyCopy, CopyPaste }
pub enum ClipboardType { Clipboard, Selection, Primary, Secondary }
```

VT Handler 方法面（`impl vte::ansi::Handler for Term<T>`）按 spec 采纳点 2 全量照抄：光标移动/插入删除/滚动/清屏/滚动区/save-restore、SGR 全语义（含 INVERSE 交换）、private mode 全家（含 9001/2031 按编号特判）、charset G0–G3、标题栈、hyperlink、OSC 52/颜色动态序列、CSI 14t/18t、kitty `push_keyboard_mode`/`pop_keyboard_modes`/`report_keyboard_mode`。

kitty ↔ TermMode 双向映射与三态应用（`term/keyboard.rs` 的 `KEYBOARD_FLAGS`、`From<KeyboardModes> for TermMode` 及反向、`set_keyboard_mode` 的 Replace/Union/Difference）：`CSI =` 修改当前帧（含隐式基帧）而非裸栈顶——Nebula 对上游的关键修正，契约由 `keyboard_contract_tests.rs` 五例锁死随迁。

### OSC 嗅探（`osc_cwd.rs`，照抄锚点）

```rust
pub enum OscEvent {
    Cwd(String),                              // OSC 7 file:// URI / 9;9 原生路径
    PromptMark,                               // 133;A
    PromptInput,                              // 133;B
    CommandStart,                             // 133;C
    CommandDone { exit_code: Option<i32> },   // 133;D 首参；裸 D 为 None
    UserVar { name: String, value: String },  // 1337 SetUserVar（值上限 8KB）
    Notify(String),                           // OSC 9
    Progress { state: u8, value: Option<u8> },// 9;4 原始 state 不上收窄
    InlineImage { data: Vec<u8>, width: u32, height: u32 }, // 1337 File=…inline=1，尺寸取编码头
}
// RemoteHook 不迁（OSC 777 远端通道砍，spec 不采纳点 4）
pub struct CwdSniffer { phase: Phase, payload: Vec<u8>, interested: bool }
impl CwdSniffer {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<(usize, OscEvent)>;
    // 事件携带「刚过终结符」的字节偏移——reader 在偏移处切开 parser.advance
}
// 有界常量（契约值，随迁）：MAX_PAYLOAD 4KB / MAX_IMAGE_PAYLOAD 12MB /
//   MAX_IMAGE_PIXELS 16M 像素（image bomb 防线）/ MAX_HOOK_PAYLOAD 96KB 不迁
```

### Event 边界契约（`event.rs`，照抄锚点；`AiHookEnvelope` 不迁）

```rust
pub enum Event {
    MouseCursorDirty, Title(String), ResetTitle,
    CwdReport(String),
    InlineImage { data: Arc<Vec<u8>>, abs_line: usize, width: f32, height: f32 },
    CommandStart, CommandDone { exit_code: Option<i32> },
    UserVar { name: String, value: String },
    Notify(String), Progress { state: u8, value: Option<u8> },
    ClipboardStore(ClipboardType, String),
    ClipboardLoad(ClipboardType, Arc<dyn Fn(&str) -> String + Sync + Send>),
    ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send>),
    PtyWrite(String), TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Sync + Send>),
    CursorBlinkingChange, Wakeup, Bell, Exit,
    ChildExit(ExitStatus), PtyFailure(String),
}
pub trait EventListener { fn send_event(&self, _event: Event) {} }
pub struct WindowSize { pub num_lines: u16, pub num_cols: u16, pub cell_width: u16, pub cell_height: u16 }
pub trait OnResize { fn on_resize(&mut self, window_size: WindowSize); }
pub trait Notify { fn notify<B: Into<Cow<'static, [u8]>>>(&self, _: B); }
```

### 三层渲染合同快照（`render.rs`，本篇核心锚点）

```rust
pub struct CellMetrics { pub cell_width: f32, pub cell_height: f32, pub scale: f32 }
// device_cell_width/height = (logical × scale).round().max(1)
pub const MIN_COLS: u16 = 2; pub const MIN_ROWS: u16 = 1;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TerminalViewport {
    pub cols: u16, pub rows: u16,
    pub cell_width_px: u16, pub cell_height_px: u16,   // 设备像素，报 PTY 的单位
    pub revision: u64,                                  // 单调；过期 resize 禁覆盖新值
}
impl TerminalViewport {
    pub fn from_content_size(width: f32, height: f32, metrics: &CellMetrics, revision: u64) -> Self;
    pub fn text_area_width_px(&self) -> u32;    // 恒 = cols × cell_width_px
    pub fn text_area_height_px(&self) -> u32;
    pub fn window_size(&self) -> WindowSize;
    pub fn grid_eq(&self, other: &Self) -> bool;
    pub fn pixel_eq(&self, other: &Self) -> bool;
}
pub struct ViewportChange { pub viewport: TerminalViewport, pub grid_changed: bool, pub pixel_changed: bool }
pub struct ViewportTracker { current: Option<TerminalViewport>, issued: u64 }
impl ViewportTracker {
    pub fn observe(&mut self, width: f32, height: f32, metrics: &CellMetrics) -> Option<ViewportChange>;
    // 亚细胞抖动不产事件；每次返回的 revision 严格递增
}

pub struct SnapCell { pub col: u16, pub text: String /* 含零宽组合符 */, pub fg: Color,
    pub bg: Color /* INVERSE 已在源头交换进 bg */, pub bold: bool, pub italic: bool,
    pub underline: bool, pub strikethrough: bool }
pub struct TextSegment { pub row: u16, pub start_col: u16, pub wide: bool, pub cells: Vec<SnapCell> }
impl TextSegment { pub fn step(&self) -> u16 { if self.wide { 2 } else { 1 } } }
pub struct CellRun { pub row: u16, pub start: u16, pub end: u16 }          // 选区几何
pub struct BgRun { pub row: u16, pub start: u16, pub end: u16, pub color: Color } // 默认背景源头抑制
pub struct CursorSnapshot { pub row: u16, pub col: u16, pub shape: CursorShape, pub wide: bool,
    pub cell_ch: char, pub cell_flags: Flags, pub cell_bg: Color }
pub struct BoxGlyph { pub row: u16, pub col: u16, pub wide: bool, pub ch: char, pub fg: Color, pub bold: bool }

pub struct RenderSnapshot {
    pub cols: u16, pub rows: u16, pub display_offset: usize,
    pub color_overrides: Colors,           // OSC 4/10/11… 269 槽
    pub bg_runs: Vec<BgRun>, pub selection_runs: Vec<CellRun>,
    pub segments: Vec<TextSegment>, pub box_glyphs: Vec<BoxGlyph>,  // 互不相交
    pub cursor: Option<CursorSnapshot>,
}
pub struct SnapshotConfig { pub rows: u16, pub cols: u16 }
impl RenderSnapshot {
    pub fn capture<T: EventListener>(term: &Term<T>, cfg: &SnapshotConfig) -> Self;
    // 锁内单趟、零引用：锁放下后才做调色/绘制
}
```

分段铁律（连测试语义照抄）：分段只由内容与宽度类决定，光标/焦点/闪烁绝不掺入——否则行缓存键随闪烁相位抖动、可见跳字。宽字符首格/占位格是一个可渲染单元：命中任一格按整字高亮、仅首格写两列 run；复制出的文本保持完整字符（基准用例 `capture_selects_complete_wide_cells_and_copy_keeps_complete_text`）。

`boxdraw`（`render/boxdraw.rs`）：`is_builtin`/`Rect`/`Primitive::{Rect,Poly}`/`primitives`/`Geom`——U+2500–259F、U+1FB00–1FB3B、U+1FB82–1FB8B、U+E0B0–E0B4 与 U+E0B6（`e0b5` 刻意不内建）；单元格局部逻辑像素坐标、内部换算设备像素整数吸附、Sutherland–Hodgman 裁剪；`░▒▓` 以 alpha 64/128/192 表浓度。根治理由：框线/块/Powerline 必须精确盖满单元格并无缝拼接，CJK 字体不保证——几何是唯一权威。

### 选择 / 搜索 / vi（照抄锚点）

```rust
// selection.rs
pub enum SelectionType { Simple, Block, Semantic, Lines }
pub struct Selection { ty, start: Anchor, end: Anchor, ... }
pub struct SelectionRange { pub start: Point, pub end: Point, pub is_block: bool }
impl SelectionRange { pub fn contains(&self, point: Point) -> bool;
    pub fn contains_cell(&self, indexed: &Indexed<Cell>, cursor: Point, shape: CursorShape) -> bool; }
// term/mod.rs 导出：selection_to_string / line_to_string（Ctrl+Shift+C 唯一取词来源）
// term/search.rs：RegexSearch / LazyDfa / search_next / regex_search_left|right / RegexIter /
//   bracket_search / semantic_search_left|right / inline_search_left|right / line_search_left|right
// vi_mode.rs：ViMotion 全表 + ViModeCursor::motion/scroll（core 备能力，壳是否暴露键位归壳）
```

### 同步原语与事件循环

```rust
// sync.rs —— FairMutex：渲染快照走 unfair 快通道，reader 用 lease 预约防 UI 饿死
pub struct FairMutex<T> { data: Mutex<T>, next: Mutex<()> }
impl<T> FairMutex<T> {
    pub fn new(data: T) -> Self;
    pub fn lease(&self) -> MutexGuard<'_, ()>;
    pub fn lock(&self) -> MutexGuard<'_, T>;
    pub fn lock_unfair(&self) -> MutexGuard<'_, T>;
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>>;
}

// event_loop.rs
pub struct StreamProcessor { parser: ansi::Processor, cwd_sniffer: CwdSniffer, window_size: Option<WindowSize> }
impl StreamProcessor {
    pub fn resize(&mut self, window_size: WindowSize);
    pub fn next_sync_timeout(&self) -> Option<Instant>;
    pub fn sync_bytes_count(&self) -> usize;
    pub fn stop_sync<U: EventListener>(&mut self, terminal: &mut Term<U>);
    pub fn feed<U: EventListener>(&mut self, terminal: &mut Term<U>, event_proxy: &U, bytes: &[u8]);
    // 内部 advance：按嗅探偏移切 parser.advance，4096 分块，~1MiB sync 缓冲预撤 redraw_anchor
}
pub enum Msg {
    Input(Cow<'static, [u8]>),   // 键盘/编码器产出
    Shutdown,
    Resize(WindowSize),          // 通知子进程 + 本地 reflow
    ResizeGrid(WindowSize),      // 仅本地 reflow（拖拽期逐帧），不通知子进程
}
pub struct EventLoop<T: EventedPty, U: EventListener> { ... }
impl<T, U> EventLoop<T, U> {
    pub fn new(terminal: Arc<FairMutex<Term<U>>>, event_proxy: U, pty: T,
               drain_on_exit: bool) -> io::Result<Self>;   // ref_test 参数不迁（不采纳点 7）
    pub fn channel(&self) -> EventLoopSender;
    pub fn spawn(self) -> JoinHandle<(Self, State)>;
}
pub struct EventLoopSender { ... }   // standalone() 保留（测试/独立通道）；sink() 不迁（不采纳点 6）
// 常量契约：READ_BUFFER_SIZE 1MiB / MAX_LOCKED_READ 64KiB / ALIGN_DELAY 120ms
// conpty_cursor_probe：FreeConsole + AttachConsole(pid) + GetConsoleScreenBufferInfo 取光标真值
// resize_trace / SLTERM_RESIZE_TRACE：按 `❯` 提示符行取证（NEBULA_RESIZE_TRACE 改名）
```

### tty 面（缝合后的终态签名）

```rust
// tty/mod.rs
pub struct Options { pub shell: Option<Shell>, pub working_directory: Option<PathBuf>,
    pub drain_on_exit: bool, pub env: HashMap<String, String>,
    pub env_is_complete: bool, pub escape_args: bool,
    pub conpty_sideload: bool }   // cfg(windows) 字段全域化;conpty_sideload 由壳读 settings 键注入(归 06 键域),core 不内嵌路径推导
pub struct Shell { program: String, args: Vec<String> }
pub enum ChildEvent { Exited(Option<ExitStatus>) }
pub trait EventedReadWrite { type Reader: io::Read; type Writer: io::Write;
    unsafe fn register(...); fn reregister(...); fn deregister(...);
    fn reader(&mut self) -> &mut Self::Reader; fn writer(&mut self) -> &mut Self::Writer; }
pub trait EventedPty: EventedReadWrite { fn next_child_event(&mut self) -> Option<ChildEvent>;
    fn child_pid(&self) -> Option<u32> { None } }

// tty/windows/mod.rs
pub struct Pty { backend: Conpty /* 必须首字段 */, conout: UnblockedReader<AnonRead>,
                 conin: UnblockedWriter<AnonWrite>, child_watcher: ChildExitWatcher }
pub fn new(config: &Options, window_size: WindowSize) -> Result<Pty>;  // _window_id 参数不迁
pub fn refresh_environment(options: &mut Options) -> io::Result<()>;   // 注册表环境重建

// tty/windows/conpty.rs
pub struct ConptyApi { create: CreatePseudoConsoleFn, resize: ResizePseudoConsoleFn,
                       close: ClosePseudoConsoleFn, sideloaded: bool, _library: Option<ConptyLibrary> }
impl ConptyApi {
    fn new() -> Self;            // 侧载优先 + 系统 API 回退，双路同型
    fn load_conpty() -> Option<Self>;   // 完整文件对 + 绝对路径 LoadLibraryW + 三符号
}
pub struct ConptyStatus { pub bundled: bool, pub fallback_reason: Option<String>, ... }
// 新增（slTerminal 并入）：pub fn conpty_status() -> &'static ConptyStatus —— 回退状态可观测

// tty/windows/spawn.rs（新建，slTerminal 并入件落位）
pub struct ConptyInputModes { pub inherit_cursor: bool /*0x1*/, pub resize_quirk: bool /*0x2*/,
    pub win32_input_mode: bool /*0x4*/, pub passthrough_mode: bool /*0x8, 默认 false*/ }
pub fn compute_conpty_flags(build_number: u32, bundled: bool, modes: &ConptyInputModes) -> u32;
static SPAWN_LOCK: ... ; pub const MAX_PTY_SESSIONS: usize = 32;      // BE-01
pub struct JobHandle(...);                                           // 孤儿防护
fn job_name(pid: u32) -> String; fn job_limits() -> JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
fn add_to_job_object(pid: u32) -> Result<JobHandle, Error>;
fn create_and_assign_job(pid: u32, name_wide: &[u16]) -> Result<JobHandle, Error>;

// tty/windows/shell.rs（新建，slTerminal 并入件落位）
pub enum ShellKind { Pwsh, PowerShell, Cmd, Other }      // Other 经白名单拒绝
pub fn resolve_shell(user_shell: Option<&str>) -> Result<Shell, Error>;   // pwsh→powershell→cmd 探测
fn validate_shell_allowlist(program: &str) -> Result<(), Error>;          // SEC-01/SEC-15 深检
fn build_pwsh_command(pwsh: &str) -> Shell;                               // -NoLogo -NoExit -EncodedCommand
fn encode_utf16le_base64(script: &str) -> String;                         // B17 守卫用例随迁
fn strip_da1_queries / strip_dsr_queries / split_trailing_query_prefix /
    mirror_da1_query / mirror_dsr_query / should_answer_dsr /
    inject_da1_response / inject_cpr_response / strip_conpty_startup /
    apply_output_strip / plan_cleanup_after_join_timeout / micro_batch_tail
    —— reader.rs 纯函数族整体迁入 event_loop 读侧（模块形态见改造节）
```

## 数据流与状态机

### PTY 输出流（core 主链）

```
conout(匿名管道) → UnblockedReader::try_read(1MiB 缓冲, waker 补投)
  → EventLoop 读线程：FairMutex lease 预约 → lock(≤64KiB 持锁)
  → StreamProcessor::feed：
      ① CwdSniffer::feed 得 [(offset, OscEvent)]
      ② 逐事件 parser.advance(bytes[advanced..offset])——prompt mark 落在光标恰在新鲜提示符行
      ③ 事件映射：PromptMark→Term::slterm_add_prompt_mark；C/D→Event::CommandStart/Done；
         Cwd→Event::CwdReport；UserVar/Notify/Progress/InlineImage→对应 Event
      ④ DA1/DSR 接管纯函数在此链上执行（见下）
  → 壳 EventListener::send_event(Wakeup/…) → 壳择机 RenderSnapshot::capture
```

事件间字节保线序：远程与本地会话不可能产生不同 cwd/命令状态（`StreamProcessor` 文件头注释因果链照抄）。`PtyFailure` 传输死因上抛——会话变僵尸的根因治理。

### DA1/DSR 接管状态机（双方案合流，缝合设计）

新世界应答身份唯一性矩阵：

| 时机 | 查询方 | 处置 | 应答字节 |
| --- | --- | --- | --- |
| sideloaded host 启动握手 | OpenConsole | priming：spawn 前预写进 conin，host 读到的第一字节 | `\x1b[?64;22c` |
| host 起后首个 DA1 | OpenConsole | `bringup_da1_pending` 一次性吞掉 Term 自答（`identify_terminal` 消费并跳过） | 不重答 |
| 会话内 DA1 查询 | shell/CLI | reader 检测→代答（每次查询必答、谁问谁答） | 统一身份（见 D02-1） |
| 启动窗口 DSR `ESC[6n` | conhost VtIo 握手 | 代答 `\x1b[1;1R`（启动期光标恒 1;1） | CPR |
| 窗口外 DSR，Win11 | 应用 | 交 Term `device_status` 自答（真实光标位置）——旧世界「透传 xterm 实答」已无 xterm，core 是唯一应答者 | `\x1b[{row};{col}R` |
| 窗口外 DSR，Win10 家族 | 未钉死发起方 | 剥离不答（键事件模式 CPR→F3 吞键毒链，ADR-0022） | 无 |

禁盲注红线贯穿：无等待者的应答字节 = 注入应用输入的杂散键。每读块处理序照抄 slTerminal reader_loop：`split_trailing_query_prefix` 扣块尾半条序列 → `mirror_da1_query`/`mirror_dsr_query` 检测 → 按需代答 → `apply_output_strip`（启动窗口内剥启动序列+DA1+DSR；窗口外剥 DA1、按 `strip_dsr` 门控剥 DSR）→ EOF 冲刷 pending 不丢数据。

### resize 流边界状态机

```
壳布局帧 → ViewportTracker::observe
  ├─ 拖拽中：Msg::ResizeGrid —— drain_recv_channel 合并（一次 drain 只最新尺寸；
  │   grid-only 不顶掉 full）→ Term::resize 本地 reflow（视口所见即真实几何）
  └─ 落定：Msg::Resize —— 事件循环内严格序：
      ① 按旧几何排空可读字节（旧宽绝对 CUP 不进新网格）
      ② Term::resize（conpty_resize 时走 resize_conpty 行锚定）
      ③ ResizePseudoConsole（同步）
      ④ 同步对账一次 + 120ms ALIGN_DELAY 死线兜底
         （conpty_cursor_probe 全局锁串行化，失败静默放弃）
      ⑤ 同步更新超时与对账死线合并进 poll deadline
差额 ≥ 视口高放弃对账；选区随内容 rotate。conpty_realign 双向滚动对账
（conhost 顶端对齐塌缩 ↔ 变宽变窄折行的双方向差）照抄，字节取证注释归机制文档。
```

### ConPTY 加载决策流（Win10 兼容全链）

```
spawn 请求
  → SPAWN_LOCK 持锁（仅 create + spawn 段，BE-12 锁界）
  → 容量判定：sessions 写锁内原子复查，MAX_PTY_SESSIONS 命中即 kill 已 spawn 子进程
  → ConptyApi 决策：
      ① options.conpty_sideload（壳读 settings 键注入，键域归 06;core 侧零路径逻辑）?
      ② 候选目录完整文件对（exe 旁 runtime/ 或 exe 目录）conpty.dll + OpenConsole.exe
         —— 齐则绝对路径 LoadLibraryW（PATH 同名文件不进认证边界）+ GetProcAddress 三符号
      ③ Win10 且文件对缺失：include_bytes! NuGet 捆绑 → %LOCALAPPDATA% 幂等提取 → 回到 ②
         （extraction 失败静默回退；加载/提取失败原因记入 ConptyStatus.fallback_reason）
      ④ 最终回退：系统 CreatePseudoConsole/Resize/Close 函数指针
  → sideloaded? priming 预写 \x1b[?64;22c（仅 sideloaded；in-box 会把无请求应答漏成输入）
  → flags = compute_conpty_flags(build, bundled, modes)（矩阵见下）
      · 0x4 位：bundled || build ≥ 21376 才置位；系统 host 置位被拒(E_INVALIDARG)
        → 退 flags=0 重试臂保留（preview build 兜底，自门控降级端到端成立）
      · 0x8 位：默认恒关——真实 claude 全屏滚轮失效实测，翻转须过人工实测门禁
  → spawn_shell（环境块 = refresh_environment 注册表快照 ∪ convert_custom_env
      大小写不敏感排序去重双 NUL ∪ 固定注入 TERM/COLORTERM/TERM_PROGRAM/SLTERM_PANE_ID）
  → Job Object 指派（KILL_ON_JOB_CLOSE）→ 释放 SPAWN_LOCK
```

`ConptyStatus` 启动后可查（壳 toast 提示降级,eg归 09）；`fallback_reason` 与 warn 日志同源零漂移。

### kitty 键盘模式栈状态机

主/备屏各持 `keyboard_mode_stack`（深度上限照抄）:`push_keyboard_mode` 入栈、`CSI =` 按 Replace/Union/Difference 修改**当前帧**（含隐式基帧）而非裸栈顶、`swap_alt` 换栈并按栈顶重放、`reset_state` 与配置翻转清双栈。降级链：host 不支持 0x4 → 永不收 DECSET 9001 → `WIN32_INPUT_MODE` 不置位 → 编码器恒走传统 VT 路径（端到端自门控，不硬编码 build 判断）。`WIN32_INPUT_MODE` 位是 Win32 输入行为的唯一开关——全仓不做任何 `SetConsoleMode` 控制台模式位操作。

### alt screen / 同步更新 / 重绘锚

- `swap_alt`：切屏不清 scrollback、光标样式各自恢复、kitty 栈互换；Vi 与 alt 互斥假设贯穿 damage/prompt。
- DEC 2026:`parser.sync_timeout()` 进 poll deadline;`stop_sync` 先 `cancel_redraw_anchor` 再提交；壳帧渲染只在 sync 窗口外提交；`redraw_anchor` 在同步更新完整收尾后按 3 行键（WRAPLINE 行/纯边框行归一）在新网格找唯一匹配恢复阅读位置，用户操作/迟到输出即取消，扫描上限 `MAX_SCAN_CELLS`。
- 语义 prompt（`term/prompt.rs`，方法族 `slterm_*` 改名）:prompt mark 只记主屏、行号严格递增去重、滚出史即剪枝；input 边界随滚动增长解析，reflow/reset 丢弃；全宽 prompt 的 `input_needs_wrap` 特例在案。OSC 133 A/B/C/D 事件序由 `StreamProcessor` 偏移切分保序——slTerminal 旧「133;A 恢复注入闸门」(`markPromptReady`）语义由此模型替代。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单（薄写；改名一律以 01 篇单点表为准）

| pebrel 源（路径 · 符号） | 缝合点 | 关键因果链一句 |
| --- | --- | --- |
| `Cargo.toml` vte 0.15（`default-features=false, features=["std","ansi"]`） | `slterm_terminal/Cargo.toml` | 解析器零维护 fork,OSC 缺口由 tee 补齐 |
| `src/term/mod.rs` `Term` + `impl Handler for Term` | 全量 | claude/codex 全屏 TUI 实战检验的完备 VT 语义 |
| `src/term/mod.rs` `TermMode` bitflags | 全量 | kitty 五标志 + 9001/2031 一位不漏 |
| `src/term/keyboard.rs` `KEYBOARD_FLAGS` 双向映射 + 三态应用 | 全量 | `CSI =` 修当前帧是上游关键修正 |
| `src/term/keyboard_contract_tests.rs` 五例 | 改名随迁 | 嵌套 push/pop、alt 各存修改帧、溢出/reset 契约 |
| `src/grid/mod.rs` `Grid`/`Scroll`/`Dimensions`/`GridIterator` | 全量 | `display_iter_from` 支撑拖拽期裁剪渲染 |
| `src/grid/storage.rs` `Storage` 环形 + `zero` 偏移 | 全量 | 滚动零行搬移是热路径根基 |
| `src/grid/row.rs` `Row`/`occ` | 全量 | occ 后单元格恒等于模板，操作只按占用段 |
| `src/grid/resize.rs` `resize`/`resize_conpty`/`cursor_anchor` | 全量 | shrink 无条件折行、grow 按 WRAPLINE 重并，往返无损 |
| `src/term/damage.rs` `LineDamageBounds`/`TermDamage`/`TermDamageState` | 全量 | 行级损伤最小重绘 |
| `src/term/redraw_anchor.rs` `RedrawAnchor` 三阶段 | 全量 | 同步更新清屏后阅读定位，动画帧不观察半提交 |
| `src/render.rs` 三层合同全套（含测试语义） | 本篇锚点 | 替换 xterm.js 的核心接口 |
| `src/render/boxdraw.rs` 纯几何指令 | 全量 | 字体（尤其 CJK）不保证无缝，几何是唯一权威 |
| `src/osc_cwd.rs` `CwdSniffer`/`OscEvent` | 砍 `RemoteHook` | vte 不解码的 OSC 全由裸字节 tee 提取 |
| `src/event_loop.rs` `StreamProcessor::feed`/`advance` 偏移切分 | 全量 | prompt mark 落点光标精确、事件间字节保线序 |
| `src/tty/windows/conpty.rs` `ConptyApi`/`load_conpty`/`bundled_conpty_dir` | 缝 NuGet 提取 | 完整文件对 + 绝对路径加载 = 认证边界 |
| `src/tty/windows/conpty.rs` `new` DA1 priming | 缝接管族 | sideloaded 握手从每 pane 等待变瞬时 |
| `src/tty/windows/conpty.rs` 0x4 尝试 + E_INVALIDARG 回退 | 缝矩阵 | 降级端到端自门控 |
| `src/tty/windows/blocking.rs` `UnblockedReader`/`UnblockedWriter` | 全量 | waker 补投堵残帧丢失；drain_detached 防 Close 死锁 |
| `src/tty/windows/child.rs` `ChildExitWatcher` | 全量 | 线程池等待 + Drop 阻塞反注册 |
| `src/tty/windows/mod.rs` `Pty` 字段序 + Drop 纪律 | 缝 Job Object | backend 首字段保 drop 序 |
| `src/event_loop.rs` `EventLoop` 全套（resize 流边界/120ms/对账 probe） | 缝微批/pending | resize 是流边界：旧几何字节绝不进新网格 |
| `src/term/mod.rs` `conpty_realign` | 全量 | 双方向滚动对账，字节取证注释随迁 |
| `src/tty/windows/environment.rs` `refresh_environment` | 缝固定注入 | 装软件不重启即拿新 PATH |
| `src/tty/windows/cmd_prompt.rs` `prepare` | UserVar 键改名 | cmd 也有语义 prompt；幂等不突变调用方 |
| `src/selection.rs` 四态模型 + `contains_cell` | 全量 | 宽字符整字语义 |
| `src/term/search.rs` `RegexSearch` 双向 DFA | 全量 | 壳搜索框直接消费 |
| `src/vi_mode.rs` `ViMotion`/`ViModeCursor` | 全量 | core 备能力，零成本 |
| `src/sync.rs` `FairMutex` 四件套 | 全量 | reader/UI 并发正确性根基 |
| `src/event.rs` `Event`/`EventListener`/`WindowSize`/`OnResize` | 砍 `AiHookEnvelope` | 事件枚举即 core→壳边界契约 |
| `src/lib.rs` `pty_trace` | 改名 `SLTERM_BOOT_TRACE` | ConPTY bring-up 各阶段埋点 |
| `scripts/win32_input_matrix.ps1` + baseline + probe 三件套 | 壳侧 GPUI 化,eg归 11 | PostMessage 最小实例键矩阵基线守门 |
| `nebula_terminal/AGENTS.md` | 改名随迁 | core 纪律（禁依赖视图/GPUI） |
| `architecture/notes/nebula_terminal/term/` 两条 note | 改名归 `slterm_terminal` | native prompt input 边界、DEC 2031 静默订阅因果档 |

### 改名映射引用

本篇全部改名服从 01 篇「改名映射单点表」;本篇领域内补充的改名项（`nebula_*` 方法族 → `slterm_*`、`NEBULA_BOOT_TRACE`/`NEBULA_RESIZE_TRACE` → `SLTERM_*`、`SetUserVar` 键 `pebrel_cmd_prompt` → `slterm_cmd_prompt`、`architecture/notes/nebula_terminal/` → `architecture/notes/slterm_terminal/`）属 spec 分片 02 优化方向 3 的既定映射，落地时并入单点表不另立。

### 缝合点

1. **`Term<T>` 泛型通道**：壳在 M3 实现 `EventListener`（送 GPUI 主线程队列）;core 不感知 GPUI。这是 core→壳唯一反向边。
2. **viewport 协议 → GPUI 布局**:GPUI element 每帧把 content rect（已扣 padding/tab 栏）喂 `ViewportTracker`;revision 过期事件壳必须丢弃。
3. **快照 → GPUI 自绘**:`RenderSnapshot` 的 `Color` 是 vte 未解析值，壳用主题 palette + `color_overrides` 解析；`box_glyphs` 走 boxdraw 几何，永不进字形 atlas。
4. **键盘编码三路径**（kitty/Win32 input/legacy):core 出 `TermMode`（协议状态机），编码器归壳（归 M3，本篇只定模式位契约）;win32_input_matrix 基线守三路径端到端。
5. **per-pane 环境装配**:`Options.env` 由壳组装，`SLTERM_PANE_ID` 等变量族归 03 篇定义，core 只负责注入时机（`convert_custom_env` 末尾叠加）。
6. **`ConptyInputModes` 设置键**：设置段形态归 06 篇键域枚举单源；core 侧 DTO 字段语义（0x1/0x2/0x4/0x8 矩阵 + 默认三态零漂移）本篇锚定。
7. **OSC 52/133 消费端**：挂 `Event::ClipboardStore`/`CommandStart`/`CommandDone`，消费组件归壳（见改造节）。
8. **复制合同**：`selection_to_string` → 壳剪贴板（选中才写）;§5.2 反馈归壳归 M3+。
9. **inline image**:`Event::InlineImage` 嗅探层照抄发出；壳呈现（有界解码资源）归壳分片，本篇不承诺。
10. **真机门禁**：Win10 捆绑/flags 矩阵/0x8 翻转/claude 全屏滚轮的实机验证点归 M2 出口 + 发布前核验归 12 篇。

## 改造 / 移植 / 新建设计

### 1. slTerminal pty 五件套并入（厚写主体）

提取路径：`git show HEAD:src-tauri/src/pty/<file>`(M0 已删旧栈，归 01 篇）。并入总原则：pebrel 的 ConPTY 加载/生命周期链为**宿主实现**（portable-pty 绕行层的 `ConPtyMaster`/`RawChild`/`create_conpty_pair` 等结构不并入，其领域规则以 pebrel 链代码形态重写）；slTerminal 五件套提供**领域规则增量**。

**冲突裁决表（同能不同实现，以何者为准）:**

| 能力 | pebrel 实现 | slTerminal 实现 | 裁决 |
| --- | --- | --- | --- |
| ConPTY 来源 | `bundled_conpty_dir` 文件对检查（runtime/ 或 exe 目录） | `include_bytes!` NuGet 嵌入 + `%LOCALAPPDATA%` 提取 | **两阶段合并**:①pebrel 目录对检查为认证边界（完整对 + 绝对路径）;②缺失且 Win10 时走 slTerminal 提取（`ensure_extracted` 幂等、`write_if_size_differs` 按尺寸判重）再回到①;③系统 API 终回退。`ConptyStatus` 观测保留 |
| flags 决策 | 硬编码 0x4 + 被拒退 0 重试 | `compute_conpty_flags` 矩阵 + build 门控（21376) | **矩阵为准**:0x1/0x2 直取，0x4 按 `bundled \|\| build≥阈值` 置位，0x8 默认关；pebrel 的 E_INVALIDARG 重试臂作为「矩阵判了 0x4 但系统 host 拒」的兜底保留。守卫用例 `conpty_flags_default_matrix_matches_legacy_tristate` 随迁（默认矩阵↔旧三态零漂移） |
| shell 解析 | `resolved_default_shell` 字面量 powershell | `resolve_shell` 存在性探测 pwsh→powershell→cmd + 白名单深检 | **slTerminal 为准**：探测顺序 + `ALLOWED_SHELLS` 白名单（SEC-01/SEC-15)；pebrel 的 bash/WSL 探测不迁 |
| pwsh 集成载体 | profile 后加载脚本（`nebula_prompt_script_path` 落盘法） | `-EncodedCommand` 内联 `shell-integration.ps1` | **slTerminal 为准**(spec 不采纳点 8):AMSI/ASR 文件写入红线；pebrel 的 `-NoProfile` 禁忌与「profile 先于集成」原则同 B17，合流为同一守卫 |
| 参数转义 | `push_escaped_arg`(stdlib C 运行时规则改写） | `build_cmdline`（同宗） | **落位时择优留一**（开放问题 2)；两者同源，先各留实现、M2 内对比测试后删一 |
| DA1 启动握手 | priming 预写 + `bringup_da1_pending` 吞重答 | reader 每查询代答 `\x1b[?64;22c`、启动剥离 | **互补共存**:priming 管 sideloaded 启动握手，接管族管会话内查询；身份统一见 D02-1 |
| DSR | Term `device_status` 自答 | 窗口内代答 / 窗口外 Win10 剥 Win11 透传 | **Term 为唯一应答者**：窗口外交还 Term 自答（真实光标）;Win10 家族窗口外剥离臂重检键事件毒链条件后保留 |
| 进程销毁 | `TerminateProcess` + `drain_detached` | Job Object `KILL_ON_JOB_CLOSE` + 监督线程 3s 超时销毁 | **双保险并存**:Drop 序 = TerminateProcess → Job 兜底（父崩溃路径） → drain_detached → Close;`ClosePseudoConsole` 永久阻塞（上游 Discussion #17716）双保险（前置 drain + 后置监督超时）并留 |
| spawn 并发 | 无 | `SPAWN_LOCK` + `MAX_PTY_SESSIONS` + 写锁复查 | **slTerminal 为准**(ConPTY 并发 spawn 死锁实测红线）；锁界照 BE-12（仅 create+spawn 段） |
| 环境构造 | `refresh_environment` 注册表快照 + `convert_custom_env` | 直接继承父进程块 + 固定注入 | **pebrel 链为准 + 注入清单保留**：注册表环境重建是净改进；`TERM`/`COLORTERM`/`TERM_PROGRAM=slterm`/`SLTERM_PANE_ID` 末尾叠加 |
| 读侧缓冲 | 1MiB 直读 + FairMutex | 16KiB + `micro_batch_tail` 微批 + PeekNamedPipe pending | **1MiB 为主 + pending 判定并入**:GPUI 壳无 IPC，微批聚合自然退化；`pending` 判定（防半帧唤醒丢失）作为 `try_read` 侧判定保留 |

**五件套落位：**

- `conpty_api.rs` → `tty/windows/conpty.rs` 内 `ConptyApi` 缝合（两阶段来源 + 状态观测）；纯函数 `should_bundle`/`conhost_input_corrupts_cpr`（阈值 21376)/`extraction_dir_from`/`ensure_extracted` 迁入同模块；`CONPTY_WIN11_MIN_BUILD` 与 `compute_conpty_flags` 共用单常量。
- `spawn.rs`（规则层）→ flags 矩阵入 `conpty.rs`;`SPAWN_LOCK`/`MAX_PTY_SESSIONS`/容量复查入 `tty/windows/spawn.rs`（新建，包住 `conpty::new` 的串行化壳）;Job Object 族入同文件（`job_name`/`job_limits` 纯函数 + 守卫用例 `job_limits_contains_kill_on_job_close`/`job_name_format_contains_pid` 随迁）。
- `reader.rs`（纯函数族）→ `event_loop.rs` 读侧：`apply_output_strip`/`strip_da1_queries`/`strip_dsr_queries`/`split_trailing_query_prefix`/`mirror_da1_query`/`mirror_dsr_query`/`should_answer_dsr` 为纯函数（L1 全保）;`inject_da1_response`/`inject_cpr_response` 改为对 `UnblockedWriter` 注入（共享 Vec writer 测试形态保留）;`plan_cleanup_after_join_timeout`/`CleanupPlan` 并入读线程收尾；`micro_batch_tail` 降级为 `try_read` 后 pending 判定辅助。
- `shell.rs` → `tty/windows/shell.rs`（新建）:`ShellKind`/`resolve_shell`/`validate_shell_allowlist`/`which_full_path`/`paths_match`/`file_identity`/`reparse_entry_identity`/`fallback_identity_match` 全迁（AEL reparse 身份比对红线，任一侧证据缺失即拒绝）;`build_pwsh_command`/`build_pwsh_info`/`encode_utf16le_base64` 全迁（B17 守卫 `pwsh_args_no_noprofile_b17` 随迁）;`Shell` 枚举收敛三 exe。
- `win_build.rs` → `tty/windows/mod.rs` 平台工具（`get_windows_build_number`);`conpty_sideload_enabled` 不迁内嵌路径推导形态，改壳读 settings 键经 `Options.conpty_sideload` 注入（spec 不采纳点 3;pebrel `ui_config.rs` 计算 `suppress_bringup_da1` 为同形态先例）。

**DA1/DSR 双方案合流细节**（数据流节状态机的实现注记）：旧世界三处应答身份（priming `?61c`、Term 自答 `?6c`、reader 代答 `?64;22c`）合并为单一身份 `?64;22c`(D02-1 已裁，slTerminal 现役值，声明 VT220 级别 + 色彩能力，对全屏 CLI 兼容面最宽；M2.4 真机复测钉死）。priming 的预写字节与统一身份同步改；`suppress_bringup_da1` 语义不变（吞的是 Term 自答，不是 reader 代答——reader 只代答会话内查询，启动握手由 priming 覆盖，两条链在偏移切分处正交）。

### 2. OSC 52/133 壳侧消费语义上移

xterm.js 时代的 `oscHandlers.ts`/`useCommandDetection.ts` 消费逻辑上移为壳侧组件（归 M3+，本篇定义挂接契约）:

- **OSC 52** → 挂 `Event::ClipboardStore`：仅放行选择器 `c`；读请求（`ClipboardLoad`）恒拒绝；壳策略上限 1MB 与 core 嗅探上限 4KB 分层各守（嗅探层先挡大头，壳层挡策略）;**焦点门控**:后台 pane 不得静默改剪贴板。
- **OSC 133** → 挂 `Event::CommandStart`/`CommandDone`:`CommandDone.exit_code` 双真退出信号（与 `ChildExit` 并列）;133;C 首 token 精确匹配 CLI profile（覆盖 `claude --resume` 变体）上移为壳侧「命令 profile 匹配器」;B12 顺序约束（setAgentSession 先于 onTabStateChange）在新世界由壳内事件序天然保证，约束文本不迁移。
- **133;A 恢复注入闸门**(`markPromptReady`）由 `Term::slterm_add_prompt_mark` 语义 prompt 模型替代（事件序保序，无注入竞争）。
- **cwd 报告** → `Event::CwdReport`：新 tab/分屏继承焦点 pane 目录（归 05 篇消费）。

### 3. xterm.js 消亡清单 → GPUI 自绘对应物映射

| xterm.js 资产（`src/panels/terminal/`） | 消亡语境 | 新世界对应物（本篇锚点 / 归篇） |
| --- | --- | --- |
| `@xterm/xterm` 6.1 VT 解析内核 | 消亡 | `Term` + vte 0.15（本篇） |
| `useXterm`/`useTerminalInstance` | 消亡 | `EventLoop` + `FairMutex<Term>`（本篇） |
| addon-fit 尺寸装配 | 消亡 | `CellMetrics` + `ViewportTracker`（本篇锚点） |
| addon-webgl/`webgl.ts` 渲染装配、SwiftShader 检测/退避 | 消亡无对应 | GPUI 渲染栈自管（能力不迁移） |
| 屏幕渲染（buffer→DOM） | 消亡 | `RenderSnapshot` + boxdraw + 壳调色板解析（本篇锚点；palette 归 06） |
| 输入法（xterm IME 合成） | 消亡 | GPUI 文本输入合成归壳（归 M3） |
| 选择/复制（xterm 选区模型） | 消亡 | `Selection` 四态 + `selection_to_string`（本篇锚点）；Ctrl+Shift+C 壳键位归 M3 |
| 滚动行为（scrollback 浏览） | 消亡 | `Grid::scroll_display` + `display_offset`（本篇锚点）；滚轮→`Scroll` 翻译归壳 |
| `vtExtensions.kittyKeyboard` 被动启用 | 消亡 | TermMode 状态机 + 壳编码器（本篇契约） |
| DEC 2026 前后包裹合帧 | 消亡 | vte 原生 sync + 壳帧渲染自然消除 |
| `windowsPty.buildNumber` 钳制（ADR-0004) | 消亡 | Win32 input flag 自门控降级（本篇） |
| `scrollback: 5000` 配置 | 消亡 | `Config.scrolling_history`（值见 D02-3） |
| `drawBoldTextInBrightColors` | 消亡 | 壳调色板解析策略归 06 |
| `Channel<PtyEvent>`/`src/ipc/pty.ts`/bytes→number[] 序列化/pty 命令五件套 | 消亡 | event_loop 直送壳事件队列（本篇）;「前端绝不碰 OS」约束随边界消失，问题域由「core 不依赖视图」承接 |
| `TerminalRegistry.ts`(panelId→sessionId Map + 订阅） | 消亡（跨进程边界消失） | 壳内「终端面板注册表」（对象标识 + 订阅，归 M3 壳；「会话元数据唯一单点」约束以 Rust 形态重建，硬约束 #8 问题域延续） |

### 4. 复制与中断的合同位置（定位约束承接）

- Ctrl+Shift+C → 壳查 `Term.selection` → `selection_to_string()` → 有选区才写系统剪贴板 → §5.2 反馈（勾+说明→恢复，共享时限实现归壳单一权威）。
- Ctrl+C → 不经 core 语义判断，壳直接编码 `\x03` 经 `Msg::Input` 下发（CP-020 双路径幂等语义由壳键位层保证）。
- 中断的本地提示（页签 attention）归壳归 09;core 只保字节语义与 `Event::Bell`。

### 5. 新建：pty 集成测试层

`tests/` 下重建（真机 `--test-threads=1` 纪律）：cmd.exe 真 spawn 生命周期、OSC 133 A/C/D 真 shell 生命周期、`run_powershell_integration_case` 范式（集成脚本 BOM、用户 profile 优先、SetUserVar 协议、自定义 prompt 不被覆盖）；「真 PTY 端到端编辑回归」范式由 bash_input 迁 pwsh（键事件/kitty/编辑擦除实机守门）。测试基础设施形态归 11 篇，本篇只列用例点。

## 测试点清单

| 用例点 | 层级/形态 | 来源 |
| --- | --- | --- |
| `keyboard_contract_tests` 五例（嵌套/alt 栈/溢出/reset） | L1 随迁 | pebrel |
| grid：环形 rotate/occ 回收/shrink-grow 往返无损/conpty 行锚定/cursor_anchor 骑内容重推导 | L1 随迁 | pebrel |
| 渲染合同：`viewport_floor_division_and_clamps`/`viewport_reports_exact_text_area_pixels`/`tracker_coalesces_and_advances_revision`/`capture_*` 全族（宽字符整字、deferred resize 裁剪、hidden DECSCUSR、boxdraw 路由、分段忽略光标） | L1 随迁 | pebrel |
| OSC 嗅探：跨块拆分、前缀早弃、`MAX_*` 有界族、133;D 负数 exit round-trip、1337 b64 名值、图片头尺寸嗅探、9;4 原始 state 不收窄 | L1 随迁 | pebrel |
| damage/redraw_anchor：逆序 undamaged/expand 合区间、同步窗口 3 行锚、WRAPLINE/边框行特殊键、用户操作取消 | L1 随迁 | pebrel |
| selection/search/vi：四态 rotate、语义扩展、双向 DFA、bracket_search | L1 随迁 | pebrel |
| event_loop 纯函数：drain 合并序、grid-only 不顶 full、sync 缓冲预撤锚点 | L1 随迁 | pebrel |
| **flags 矩阵守卫族**：`conpty_flags_default_matrix_matches_legacy_tristate`（默认矩阵↔旧三态）+ 7 条三态用例 + 0x8 默认关断言 | L1 并入 | slTerminal spawn.rs |
| **B17 守卫** `pwsh_args_no_noprofile_b17` + EncodedCommand 编码 round-trip | L1 并入 | slTerminal shell.rs |
| **shell 白名单族**:allowlist 正反 10 例 + AEL reparse 身份比对 + 双侧证据缺失即拒绝 | L1 并入 | slTerminal shell.rs |
| **DA1/DSR 纯函数族**:strip/mirror/split/should_answer 全用例随迁（含 `strip_preserves_da1_lookalikes`、16K 边界、`split_trailing_query_prefix` 半条序列扣留）+ 毒链回归（盲注 CPR→F3 吞键防复发用例） | L1 并入 | slTerminal reader.rs |
| **Job Object 纯函数**:`job_name_format_contains_pid`/`job_limits_contains_kill_on_job_close` | L1 并入 | slTerminal spawn.rs |
| **ConPTY 加载决策**:`should_bundle` 阈值正反、`conhost_input_corrupts_cpr` 边界、`extraction_dir_from`、`ensure_extracted` 幂等、`write_if_size_differs` 尺寸判重、Win11 not-attempted | L1 并入 | slTerminal conpty_api.rs |
| **容量/串行化**:BE-01 上限判定语义、锁内判定+写锁复查原子性 | L1 并入 | slTerminal spawn.rs |
| **cmd_prompt**：幂等、自定义 prompt 保留、非 cmd 不注入、空前缀、prefix-only 升级 | L1 随迁 | pebrel |
| **environment**:REG_EXPAND_SZ 两遍展开、MORE_DATA 翻倍、Path 追加、键删除追踪、大小写不敏感排序去重 | L1 随迁 | pebrel |
| **ConPTY 集成**（真机、串行）:cmd.exe spawn→write→kill 全链、shell 退出 watcher、drain_detached 无挂起、Job 孤儿回收 | L1 集成测试 | 双源合并重建 |
| **真 shell 集成范式**:pwsh 集成脚本 BOM/profile 优先/OSC 133 生命周期/SetUserVar/自定义 prompt 不覆盖 | L1 集成（真机） | pebrel 范式 + slTerminal 既有豁免口径 |
| **win32_input_matrix 基线**（kt/Win32 input/legacy 三编码路径，chord 人工步骤保留） | 真机基线回归,eg归 11 | 双源 |
| 改名守卫：新代码树零旧名（禁名门禁逐 crate 通过归 01 篇；本篇管 `nebula_*`/`NEBULA_*` 在 core 内清零） | 门禁 | 01 篇机制 |

## 阶段归属与出口标准

本片全部属 **M2 终端核心**(00-roadmap 既定），细分四步，出口全为可机验项：

**M2.1 骨架拷贝与裁剪改名**:pebrel `nebula_terminal/` → `slterm_terminal/`，砍 unix/`connection.sh`/`connection.ps1`/`completion.sh`/`completion.ps1`/`proxy.ps1`/`proxy_tests`/`bash_input`/`ref_test`/`sink`/`RemoteHook` 全链/`AiHookEnvelope`；按 01 篇单点表 + 本篇补充改名项改名。出口 = `cargo check` 过；禁名门禁过（core 内 `nebula`/`pebrel`/`NEBULA_` 零残留，豁免仅 01 篇登记的审计锚点）;file-budgets 新 root 登记。

**M2.2 L1 测试移植转绿**：照抄节清单的 pebrel 用例全量随迁改名（用例名不变）,「目标形态」模块表逐项编译到位。出口 = `cargo test` 全绿（单线程纪律）;L1 用例数不降（对照 M2.1 前基线清单）。

**M2.3 五件套并入**：按改造节 1 落位；冲突裁决表逐项落地；合流状态机（DA1/DSR/flags/加载）实现 + 并入用例全绿。出口 = 并入用例全绿（flags 守卫/B17/白名单/DA1-DSR 纯函数/Job 纯函数/加载决策）;`compute_conpty_flags` 默认矩阵零漂移；新代码树零旧名复核。

**M2.4 集成层与基线**:ConPTY 集成用例 + pwsh 真 shell 范式重建；win32_input_matrix 三件套 GPUI 化归 11 篇（本篇提供基线语义与门禁点）。出口 = 集成用例在真机 `--test-threads=1` 全绿；真机门禁清单（Win10 捆绑、flags 三态、claude 全屏滚轮、DA1 身份验证）执行记录落 `architecture/notes/slterm_terminal/`。

M2 总出口对齐 00-roadmap:`cargo test` 全绿（含 ConPTY 集成用例）;win32 输入矩阵脚本 GPUI 化改造编译过，基线建基与常挂比对归 M3 出口（M2 无可驱动 exe)。M2 全程无可运行产品（纯库态），符合过渡期不可用清单。

## 待沉淀决策

> [待沉淀] **D02-1 · DA1 应答统一身份的终值**。真实权衡：priming 预写字节、Term 自答、reader 代答三处必须同身份，候选 `?64;22c`(VT220+色彩，对全屏 CLI 兼容面最宽，slTerminal 现役）与 pebrel priming 的 `?61c` 分歧——身份越高，老旧 conhost 握手与新 CLI 能力协商的行为差异越难预测。难逆点：身份钉死后变更 = 全部 CLI 的可感知行为变更，回归面不可自动化穷尽。意外因素：双后端（sideloaded/in-box）握手路径不同，in-box 永不走 priming,Term/reader 自答身份是唯一暴露面；验证依赖真实 claude 全屏实机。截止：M2.3 合流实现时初选 + M2.4 真机验证后钉死，记录入 adr.md。

> [待沉淀] **D02-2 · ConPTY 运行时发行形态（单文件 exe × 目录文件对 × %LOCALAPPDATA% 提取）的终态组合**。真实权衡：pebrel 的 exe 旁 `runtime/` 目录对免提取、无 Defender 首扫延迟，但破坏单文件发布形态；slTerminal 的 `include_bytes!` 保持单文件、适配 zip 便携形态，但每次首启付提取成本与杀软扫描税。难逆点：发布形态随安装器归 12 篇固化后，改形态 = 安装器 + 自动更新链 + 用户数据目录的三方联动变更。意外因素：Win10 NuGet 捆绑只惠及 build < 21376,Win11 永远走系统 API——提取逻辑的死代码面与维护税随 Win10 退役演变。截止：M2.3 加载链落地时定（默认：两阶段合并形态照改造节 1 实施，目录对检查为认证边界不变；单文件 exe 定位约束优先，提取成本登记实测）。

> [待沉淀] **D02-3 · `Config.scrolling_history` 终值**。真实权衡：xterm.js 现役 5000 vs pebrel 默认 10000——翻倍内存换回溯深度；且 ConPTY 行锚定 resize 的成本随 scrollback 占用段线性增长，大 scrollback + 频繁拖拽 resize 是真实热路径组合。难逆点：发布后用户形成 scrollback 数据预期，下调即「丢历史」感知，迁移补偿无意义（历史行本不可迁移）。意外因素：AI CLI 长会话（claude 全屏数小时）的 scrollback 压力显著高于人操终端，现役 5000 是 xterm.js 时代的经验值，未经新链实测。截止：M2.2 测试基线期定初值（默认 10000 随 pebrel,M3 首窗口实测后再评，改动走设置键归 06 篇）。

## 开放问题

1. **参数转义双实现择优**:`push_escaped_arg` 与 `build_cmdline` 同源但细节分歧（空串、引号内引号、尾反斜杠）,M2.3 内以用例对比后删一；若两者行为差异有真实 CLI 触发面（如含空格/引号的 profile 路径），保留差异并登记 note。
2. **终端面板注册表（TerminalRegistry 对应物）的锚点边界**：硬约束 #8「会话元数据唯一单点」的 Rust 重建体（pane 对象标识 → 终端会话句柄 + 订阅）归 M3 壳实现；但 pane/tab 标识类型锚点归 05 篇——注册表本体与 05 篇类型的消费关系在 M3 集成时校准，本篇不 preempt。
3. **窗口外 DSR 发起方身份**：slTerminal 现状「发起方未钉死，处置与发起方无关」在 Term 自答形态下仍成立（自答用真实光标，对任何发起方都正确）——若未来观测到依赖特定应答位置的应用差异，重新评估 Win10 剥离臂的适用范围。
4. **inline image 开关默认值**：嗅探照抄发出 `Event::InlineImage`，壳是否默认呈现（有界解码资源归壳）归壳分片；若壳默认关闭，core 事件是否保留仍发出（调试可见性）随壳裁定，本篇不阻塞。
5. **真机门禁的执行归属**:M2.4 真机清单（Win10 捆绑/flags/滚轮/DA1）与 12 篇发布核验链的关系——默认 M2.4 做能力验证、12 篇做发布复核，避免双重门禁口径漂移。
