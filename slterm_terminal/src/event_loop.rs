//! The main event loop which performs I/O on the pseudoterminal.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind, Read, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use log::error;
use polling::{Event as PollingEvent, Events, PollMode, Poller};

use crate::event::{self, Event, EventListener, WindowSize};
use crate::grid::Dimensions as _;
use crate::index::{Column, Line};
use crate::osc_cwd::OscEvent;
use crate::sync::FairMutex;
use crate::term::Term;
use crate::{thread, tty};
use vte::ansi;

/// Max bytes to read from the PTY before forced terminal synchronization.
pub(crate) const READ_BUFFER_SIZE: usize = 0x10_0000;

/// Max bytes to read from the PTY while the terminal is locked.
const MAX_LOCKED_READ: usize = u16::MAX as usize;

/// ConPTY 光标对账的静默期：最后一次 resize 提交后等这么久再探针，给
/// conhost 的内部整理（缓冲区塌缩/重锚）和 shell 的 resize 反应留时间。
const ALIGN_DELAY: Duration = Duration::from_millis(120);

/// Resize diagnostics are opt-in because PTY reads are a hot path. The trace
/// records the local grid state at the exact stream boundaries where bytes are
/// parsed or a resize is committed (`SLTERM_RESIZE_TRACE=1`).
fn resize_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SLTERM_RESIZE_TRACE").is_some())
}

fn trace_terminal_state<U: EventListener>(stage: &str, terminal: &Term<U>) {
    if !resize_trace_enabled() {
        return;
    }

    // PowerShell's configured prompt uses U+276F. Recording every visible
    // occurrence exposes stale prompt rows immediately after a reflow.
    let mut prompt_rows = Vec::new();
    for row in 0..terminal.screen_lines() {
        if (0..terminal.columns())
            .any(|column| terminal.grid()[Line(row as i32)][Column(column)].c == '❯')
        {
            prompt_rows.push(row);
        }
    }

    let cursor = terminal.grid().cursor.point;
    eprintln!(
        "[slterm:resize-trace] {stage} grid={}x{} cursor={}:{} wrap={} history={} display_offset={} prompts={prompt_rows:?}",
        terminal.columns(),
        terminal.screen_lines(),
        cursor.line.0,
        cursor.column.0,
        terminal.grid().cursor.input_needs_wrap,
        terminal.history_size(),
        terminal.grid().display_offset(),
    );
}

/// One coalesced resize waiting at a stream boundary.
///
/// `notify_pty` records whether the child must see the resize: the geometry is
/// the same either way, only the child's awareness of it differs.
#[derive(Copy, Clone, Debug)]
struct PendingResize {
    window_size: WindowSize,
    notify_pty: bool,
}

/// conhost 的一次光标真值读数。
#[derive(Copy, Clone, Debug)]
struct ConhostCursor {
    /// 视口相对行（0 基）。
    row: usize,
    /// conhost 自己的视口高度，用来判断新尺寸有没有落地。
    rows: u16,
}

/// 向 conhost 要光标真值：临时 `AttachConsole` 到 ConPTY 子进程的控制台，
/// 读 `GetConsoleScreenBufferInfo`，换算成视口相对行（0 基）。
///
/// 进程同一时刻只能挂一个控制台，多 pane 并发对账用全局锁串行化；每次
/// 探针都 attach→读→detach，窗口只有微秒级。Slterm 主进程是 GUI 子系统
/// （自身无控制台），detach 后回到无控制台状态，不影响任何组件。失败
/// （子进程已退出、权限等）一律返回 None，对账静默放弃。
fn conpty_cursor_probe(pid: u32) -> Option<ConhostCursor> {
    use std::sync::Mutex;

    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        AttachConsole, CONSOLE_SCREEN_BUFFER_INFO, FreeConsole, GetConsoleScreenBufferInfo,
    };

    static ATTACH_LOCK: Mutex<()> = Mutex::new(());
    let _guard = ATTACH_LOCK.lock().ok()?;

    // SAFETY: 纯 Win32 控制台句柄操作；FreeConsole 对无控制台进程是无害
    // no-op，收尾的 FreeConsole 恢复 GUI 进程的无控制台状态。
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return None;
        }
        let mut conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let handle = CreateFileW(
            conout.as_mut_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            FreeConsole();
            return None;
        }
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        let ok = GetConsoleScreenBufferInfo(handle, &mut info);
        CloseHandle(handle);
        FreeConsole();
        if ok == 0 {
            return None;
        }
        let row = i32::from(info.dwCursorPosition.Y) - i32::from(info.srWindow.Top);
        let rows = i32::from(info.srWindow.Bottom) - i32::from(info.srWindow.Top) + 1;
        Some(ConhostCursor {
            row: usize::try_from(row).ok()?,
            rows: u16::try_from(rows.max(0)).ok()?,
        })
    }
}

/// 本地 PTY 与远端传输共用的有状态终端字节流处理器。
/// OSC 提取必须紧贴 VT 解析，避免两类会话产生不同的目录、命令和图片状态。
#[derive(Default)]
pub struct StreamProcessor {
    parser: ansi::Processor,
    cwd_sniffer: crate::osc_cwd::CwdSniffer,
    window_size: Option<WindowSize>,
    /// DA1/DSR 接管：窗口外 DSR 剥离门控（平台后端经
    /// `EventedPty::strip_dsr_queries` 注入；Win10 家族 = true——键事件
    /// 输入下 CPR 应答即 F3 毒键；Win11+ = false——DSR 透传交 Term 实答）。
    strip_dsr: bool,
    /// 启动窗口：首个非全剥块通过后置位（BE-13 跨边界残留窗口保持——
    /// 整块剥绝不置位，窗口对迟到的启动序列保持开放）。
    startup_drained: bool,
    /// 跨块查询前缀残片扣留（≤3 字节：ESC / ESC[ / ESC[0 / ESC[6）——
    /// 半条序列不送 parser，下块拼回，EOF 时冲刷不丢字节。
    pending_query: Vec<u8>,
}

impl StreamProcessor {
    pub fn resize(&mut self, window_size: WindowSize) {
        self.window_size = Some(window_size);
    }

    pub fn next_sync_timeout(&self) -> Option<Instant> {
        self.parser.sync_timeout().sync_timeout()
    }

    pub fn sync_bytes_count(&self) -> usize {
        self.parser.sync_bytes_count()
    }

    pub fn stop_sync<U: EventListener>(&mut self, terminal: &mut Term<U>) {
        terminal.cancel_redraw_anchor();
        self.parser.stop_sync(terminal);
    }

    /// 注入窗口外 DSR 剥离门控（`EventLoop::new`/spawn 时由 PTY 后端注入；
    /// 默认 false = 透传交 Term 自答）。
    pub fn set_strip_dsr(&mut self, strip_dsr: bool) {
        self.strip_dsr = strip_dsr;
    }

    /// DA1/DSR 全量接管：每读块处理序（旧栈 reader_loop 形态并入）。
    ///
    /// 拼接上轮扣留残片 → `split_trailing_query_prefix` 扣块尾半条序列 →
    /// `mirror_da1_query`/`mirror_dsr_query` 检测（剥离前的原始字节上）→
    /// 按需代答（DA1 每查必答 `\x1b[?64;22c`；启动窗口内 DSR 代答 CPR
    /// `\x1b[1;1R`，绝不盲注）→ `apply_output_strip`（启动窗口内剥启动
    /// 序列+DA1+DSR；窗口外剥 DA1、按 `strip_dsr` 门控剥 DSR）。
    ///
    /// 返回 `None` = 整块剥离（残片或启动序列独占）——不置 drained，
    /// 调用方跳过本轮继续读；`Some(out)` 恒非空，置 drained。代答经
    /// `writer`（PTY stdin）注入，与键盘输入同通道严格序。
    fn strip_and_answer<W: Write>(&mut self, bytes: &[u8], writer: &mut W) -> Option<Vec<u8>> {
        // 跨块防裂:拼接上轮扣留残片,再扣留本块尾部查询前缀。
        let mut frame = std::mem::take(&mut self.pending_query);
        frame.extend_from_slice(bytes);
        let (body, rest) = split_trailing_query_prefix(frame);
        self.pending_query = rest;
        if body.is_empty() {
            // 整块均为查询前缀残片(≤3B)——待下轮补全。
            return None;
        }

        // DA1 全量接管:每次查询都代答(查询已不透传 Term,core 是唯一
        // 应答方;应答身份与 priming/Term 自答统一 `?64;22c`,D02-1)。
        if mirror_da1_query(&body) {
            inject_da1_response(writer);
        }

        // DSR 握手按需代答:仅启动窗口内(ConPTY VtIo 握手查询)。问什么
        // 答什么,绝不盲注(Win10 捆绑 OpenConsole 握手发 DA1 不发 DSR,
        // 盲注 CPR 会被键事件模式解析为 F3 吞键)。窗口外 DSR 不代答——
        // Win11+ 透传 Term 实答真实位置;Win10 家族由 apply_output_strip
        // 门控剥离。
        if should_answer_dsr(self.startup_drained, &body) {
            inject_cpr_response(writer);
        }

        let out = apply_output_strip(self.startup_drained, self.strip_dsr, &body)?;
        self.startup_drained = true;
        Some(out)
    }

    /// EOF 冲刷：子进程退出后扣留的查询前缀残片原样吐出（不完整序列不再
    /// 有下块，原样进 parser 不丢字节）。
    fn take_pending_query(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_query)
    }

    fn advance<U: EventListener>(&mut self, terminal: &mut Term<U>, bytes: &[u8]) {
        // VTE 0.15 在约 2 MiB 时强制提交同步缓冲；提前取消兼容锚点，
        // 但不强制结束/截断输出。分块保证单次大输入也不会绕过检查。
        for chunk in bytes.chunks(4096) {
            if self.parser.sync_bytes_count() >= 1024 * 1024 {
                terminal.cancel_redraw_anchor();
            }
            self.parser.advance(terminal, chunk);
        }
    }

    pub fn feed<U: EventListener>(
        &mut self,
        terminal: &mut Term<U>,
        event_proxy: &U,
        bytes: &[u8],
    ) {
        let osc_events = self.cwd_sniffer.feed(bytes);
        let mut advanced = 0;
        for (offset, event) in osc_events {
            // Titles and shell identity/cwd reports must retain wire order:
            // coalescing a remote cwd past a parent prompt would attribute that
            // directory to the parent shell's completion history.
            self.advance(terminal, &bytes[advanced..offset]);
            advanced = offset;
            match event {
                OscEvent::Cwd(cwd) => event_proxy.send_event(Event::CwdReport(cwd)),
                OscEvent::CommandStart => {
                    terminal.slterm_end_prompt();
                    event_proxy.send_event(Event::CommandStart);
                }
                OscEvent::CommandDone { exit_code } => {
                    terminal.slterm_end_prompt();
                    event_proxy.send_event(Event::CommandDone { exit_code })
                }
                OscEvent::UserVar { name, value } => {
                    event_proxy.send_event(Event::UserVar { name, value })
                }
                OscEvent::Notify(text) => event_proxy.send_event(Event::Notify(text)),
                OscEvent::Progress { state, value } => {
                    event_proxy.send_event(Event::Progress { state, value })
                }
                OscEvent::PromptMark => {
                    terminal.slterm_add_prompt_mark();
                }
                OscEvent::PromptInput => terminal.slterm_mark_prompt_input(),
                OscEvent::InlineImage {
                    data,
                    width,
                    height,
                } => {
                    let (cell_w, cell_h) = self.window_size.map_or((9.0, 20.0), |ws| {
                        (f32::from(ws.cell_width), f32::from(ws.cell_height))
                    });
                    let max_w = terminal.columns() as f32 * cell_w;
                    let scale = (max_w / width as f32).min(1.0);
                    let disp_w = width as f32 * scale;
                    let disp_h = height as f32 * scale;
                    let rows = (disp_h / cell_h).ceil().max(1.0) as usize;
                    let abs_line = terminal.slterm_cursor_abs_line();
                    for _ in 0..=rows {
                        self.advance(terminal, b"\r\n");
                    }
                    event_proxy.send_event(Event::InlineImage {
                        data: std::sync::Arc::new(data),
                        abs_line,
                        width: disp_w,
                        height: disp_h,
                    });
                }
            }
        }
        self.advance(terminal, &bytes[advanced..]);
    }
}

// ─── DA1/DSR 接管纯函数族（旧栈 `pty/reader.rs` 语义资产并入）───
//
// 应答身份唯一性（D02-1，三处同身份 `\x1b[?64;22c`）：
// - priming 预写（sideloaded 启动握手，tty/windows/conpty.rs）；
// - Term 自答（`Term::identify_terminal`，reader 漏网时的兜底层，
//   `bringup_da1_pending` 一次性吞重答——正常态 DA1 在此族剥离，不可达）；
// - reader 代答（本族，会话内每查询必答的主答层）。
//
// 红线：禁盲注——没有等待者的应答字节 = 注入应用输入的杂散键；每次查询
// 必答、谁问谁答。

/// DA1 应答注入：检测到 DA1 查询（ESC[c / ESC[0c）时向子进程 stdin 写入
/// ESC[?64;22c（VT220 级别 + 色彩能力——对全屏 CLI 兼容面最宽；缺应答
/// Ink 启动阻塞约 60s）。每次查询都应答——查询已不透传，core 是唯一
/// 应答方。检测决策 = 纯函数 `mirror_da1_query`，注入动作为 I/O。
fn inject_da1_response<W: Write>(writer: &mut W) {
    if let Err(e) = writer.write_all(b"\x1b[?64;22c") {
        error!("DA1 response injection failed: {e}");
    }
    if let Err(e) = writer.flush() {
        error!("DA1 response flush failed: {e}");
    }
}

/// CPR 应答注入（DSR 握手按需代答）：启动窗口内检测到 DSR 光标查询
/// （ESC[6n，ConPTY VtIo::StartIfNeeded 握手）时向子进程 stdin 写入
/// ESC[1;1R（启动期光标恒在 1;1）。仅在握手真实发出时才应答——盲注
/// CPR 会落入应用输入被解析为 F3 键（PSReadLine CharacterSearch 吞掉
/// 下一个输入字符）。检测决策 = 纯函数 `should_answer_dsr`。
fn inject_cpr_response<W: Write>(writer: &mut W) {
    if let Err(e) = writer.write_all(b"\x1b[1;1R") {
        error!("CPR response injection failed: {e}");
    }
    if let Err(e) = writer.flush() {
        error!("CPR response flush failed: {e}");
    }
}

/// 剥离 ConPTY VtIo::StartIfNeeded() 注入的启动序列。
///
/// 启动序列（按出现顺序）：
/// - OSC 窗口标题: `ESC ] 0 ; ... BEL` — BEL(0x07) 触发蜂鸣
/// - 清屏: `ESC [ 2 J` / `ESC [ 3 J`
/// - 光标归位: `ESC [ H`
/// - 光标显隐: `ESC [ ? 2 5 h` / `ESC [ ? 2 5 l`
/// - DSR 光标查询: `ESC [ 6 n`（已被 CPR 应答，此序列无害但多余）
/// - DA1 查询: `ESC [ c` / `ESC [ 0 c`（接管剥离——conhost 启动握手多发，
///   透传会触发二次自答回灌 stdin）
fn strip_conpty_startup(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b {
            // OSC 序列（ESC ]）——以 BEL(0x07) 或 ST(ESC \) 终结。
            if i + 1 < data.len() && data[i + 1] == b']' {
                if i + 2 < data.len()
                    && (data[i + 2] == b'0' || data[i + 2] == b'2')
                    && let Some(end) = find_osc_end(&data[i..])
                {
                    i += end;
                    continue;
                }
                result.push(data[i]);
                i += 1;
                continue;
            }

            // CSI 序列（ESC [）。
            if i + 1 < data.len() && data[i + 1] == b'[' {
                if let Some(len) = match_csi_startup(&data[i..]) {
                    i += len;
                    continue;
                }
                result.push(data[i]);
                i += 1;
                continue;
            }
        }

        result.push(data[i]);
        i += 1;
    }
    result
}

fn find_osc_end(data: &[u8]) -> Option<usize> {
    for j in 2..data.len() {
        match data[j] {
            0x07 => return Some(j + 1),
            0x1b if j + 1 < data.len() && data[j + 1] == b'\\' => return Some(j + 2),
            _ => {}
        }
    }
    None
}

fn match_csi_startup(data: &[u8]) -> Option<usize> {
    if data.len() < 3 {
        return None;
    }
    match data[2] {
        b'H' => Some(3),
        // DA1 查询 ESC[c（接管剥离，不透传）。
        b'c' => Some(3),
        // DA1 变体 ESC[0c。
        b'0' if data.len() >= 4 && data[3] == b'c' => Some(4),
        b'2' | b'3' if data.len() >= 4 && data[3] == b'J' => Some(4),
        b'6' if data.len() >= 4 && data[3] == b'n' => Some(4),
        b'?' if data.len() >= 6
            && data[3] == b'2'
            && data[4] == b'5'
            && (data[5] == b'h' || data[5] == b'l') =>
        {
            Some(6)
        }
        _ => None,
    }
}

/// 剥离 DA1 查询序列（ESC[c / ESC[0c）——启动窗口外（startup_drained 后）
/// 使用。仅匹配两种精确形态——DA2（ESC[>c）、XTVERSION（ESC[>0q）、带参
/// 变体（ESC[1c 等）均不动。
fn strip_da1_queries(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' {
            // ESC[c
            if data[i + 2] == b'c' {
                i += 3;
                continue;
            }
            // ESC[0c
            if i + 3 < data.len() && data[i + 2] == b'0' && data[i + 3] == b'c' {
                i += 4;
                continue;
            }
        }
        out.push(data[i]);
        i += 1;
    }
    out
}

/// 剥离 DSR 光标查询序列（ESC[6n）——启动窗口外、Win10 家族门控开启时
/// 使用。
///
/// Win10 家族 conhost 键事件输入把 CPR 应答（CSI 1;1R）解析为 F3 键
/// （PSReadLine CharacterSearch 吞掉下一个输入字符）——该传输层上 DSR
/// 只能剥离不答（发起方现状拿到的本就是 F3 垃圾，剥离严格不劣）。仅
/// 匹配精确形态 ESC[6n——ESC[5n（设备状态）/ ESC[?6n（私有模式）/
/// ESC[65n 等同族不动（判别集与 `mirror_dsr_query` 一致）。块尾残片扣留
/// 由 `split_trailing_query_prefix` 负责（ESC[6 已在前缀表）。
fn strip_dsr_queries(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b
            && i + 3 < data.len()
            && data[i + 1] == b'['
            && data[i + 2] == b'6'
            && data[i + 3] == b'n'
        {
            i += 4;
            continue;
        }
        out.push(data[i]);
        i += 1;
    }
    out
}

/// 切分块尾查询前缀残片（纯函数）。
///
/// 接管的查询序列（DA1：ESC[c / ESC[0c；DSR：ESC[6n）可能跨 read 块
/// 边界——块尾若为其前缀（ESC / ESC[ / ESC[0 / ESC[6），扣留待下块拼接，
/// 防止半条序列送 parser 后被跨写入拼合自答。扣留只延迟不丢字节
/// （ESC[6 也是 CUP `ESC[6;..H` 的前缀，随下块原样拼回）。
/// 返回（主体, 扣留残片）。
fn split_trailing_query_prefix(data: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    for prefix in [
        b"\x1b[0".as_slice(),
        b"\x1b[6".as_slice(),
        b"\x1b[".as_slice(),
        b"\x1b".as_slice(),
    ] {
        if data.ends_with(prefix) {
            let at = data.len() - prefix.len();
            return (data[..at].to_vec(), data[at..].to_vec());
        }
    }
    (data, Vec::new())
}

/// 输出剥离统一入口（纯函数）。
///
/// - 启动窗口内（startup_drained=false）：`strip_conpty_startup` 剥离
///   ConPTY 启动序列 + DA1 + DSR 查询；全部剥离返回 None——调用方跳过
///   本轮且不置 drained（BE-13 跨边界残留窗口保持开放）。窗口内行为与
///   门控无关。
/// - 窗口外（drained=true）：剥 DA1 查询；`strip_dsr=true`（Win10 家族
///   门控）时一并剥 DSR 查询（ESC[6n）——键事件输入下 CPR 应答即 F3
///   毒键，不透传不代答；`false`（Win11+）DSR 维持透传交 Term 实答。
///   整块全是查询时返回 None（无内容可发），否则 Some(剥离后数据)。
fn apply_output_strip(startup_drained: bool, strip_dsr: bool, data: &[u8]) -> Option<Vec<u8>> {
    let stripped = if startup_drained {
        let after_da1 = strip_da1_queries(data);
        if strip_dsr {
            strip_dsr_queries(&after_da1)
        } else {
            after_da1
        }
    } else {
        strip_conpty_startup(data)
    };
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

/// 检测输出字节流中是否含有 DA1 终端查询（ESC[c 或 ESC[0c）。
///
/// DA2 (ESC[>c) 和 XTVERSION (ESC[>0q) 不触发——两者走不同的检测路径。
/// 滑动窗口扫描，在 read 块（拼接扣尾后）上调用。
fn mirror_da1_query(data: &[u8]) -> bool {
    for i in 0..data.len().saturating_sub(2) {
        if data[i] == 0x1b && data[i + 1] == b'[' {
            let rest = &data[i + 2..];
            // ESC[c — 不含额外参数的标准 DA1 查询。
            if rest.first() == Some(&b'c') {
                return true;
            }
            // ESC[0c — 含前导 0 的变体。
            if rest.len() >= 2 && rest[0] == b'0' && rest[1] == b'c' {
                return true;
            }
        }
    }
    false
}

/// 检测输出字节流中是否含有 DSR 光标位置查询（ESC[6n）。
///
/// 仅匹配精确形态 ESC[6n（ConPTY VtIo 握手查询）；ESC[5n（设备状态
/// 查询）、ESC[?6n 等同族不动。
fn mirror_dsr_query(data: &[u8]) -> bool {
    for i in 0..data.len().saturating_sub(3) {
        if data[i] == 0x1b && data[i + 1] == b'[' && data[i + 2] == b'6' && data[i + 3] == b'n' {
            return true;
        }
    }
    false
}

/// DSR 代答决策（纯函数）。
///
/// 仅启动窗口内（startup_drained=false）的 DSR 由 core 代答 CPR——
/// ConPTY VtIo 握手期查询，应答只要求格式合法（启动期光标恒在 1;1）。
/// 启动窗口外的 DSR 不代答：Win11+ 透传 Term 实答真实位置；Win10 家族
/// 由 `apply_output_strip` 按 strip_dsr 门控剥离。
fn should_answer_dsr(startup_drained: bool, data: &[u8]) -> bool {
    !startup_drained && mirror_dsr_query(data)
}

/// Messages that may be sent to the `EventLoop`.
#[derive(Debug)]
pub enum Msg {
    /// Data that should be written to the PTY.
    Input(Cow<'static, [u8]>),

    /// Indicates that the `EventLoop` should shut down, as Slterm is shutting down.
    Shutdown,

    /// Instruction to resize the PTY.
    Resize(WindowSize),

    /// Reflow the local grid to a new geometry without telling the child.
    ///
    /// Keep local grid reflow separate from notifying the child. The legacy shell
    /// has always done this (`window_context/split.rs`: grids every drag tick,
    /// PTYs on settle). The
    /// two halves have opposite cost profiles: a client-side reflow is cheap and
    /// reversible, while every `ResizePseudoConsole` makes conhost rewrap its
    /// own buffer, and those rewraps accumulate cursor-row drift that nothing
    /// can undo. So the grid follows the pointer frame by frame — the viewport
    /// on screen is always a real reflow of the real geometry — and only the
    /// child is debounced.
    ///
    /// Goes through the same channel as `Resize` on purpose: the resize branch
    /// drains everything readable against the old geometry first, so absolute
    /// CUP sequences produced at the old width are never parsed into the new
    /// grid. A UI thread reaching into `Term::resize` directly would skip that.
    ResizeGrid(WindowSize),
}

/// The main event loop.
///
/// Handles all the PTY I/O and runs the PTY parser which updates terminal
/// state.
pub struct EventLoop<T: tty::EventedPty, U: EventListener> {
    poll: Arc<Poller>,
    pty: T,
    rx: PeekableReceiver<Msg>,
    tx: Sender<Msg>,
    terminal: Arc<FairMutex<Term<U>>>,
    event_proxy: U,
    drain_on_exit: bool,
}

impl<T, U> EventLoop<T, U>
where
    T: tty::EventedPty + event::OnResize + Send + 'static,
    U: EventListener + Send + 'static,
{
    /// Create a new event loop.
    pub fn new(
        terminal: Arc<FairMutex<Term<U>>>,
        event_proxy: U,
        pty: T,
        drain_on_exit: bool,
    ) -> io::Result<EventLoop<T, U>> {
        let (tx, rx) = mpsc::channel();
        let poll = Poller::new()?.into();
        Ok(EventLoop {
            poll,
            pty,
            tx,
            rx: PeekableReceiver::new(rx),
            terminal,
            event_proxy,
            drain_on_exit,
        })
    }

    pub fn channel(&self) -> EventLoopSender {
        EventLoopSender {
            sender: self.tx.clone(),
            poller: self.poll.clone(),
        }
    }

    /// Drain the channel.
    ///
    /// Returns `Err(())` when a shutdown message was received; otherwise the
    /// last resize in this channel batch is returned for an ordered commit.
    fn drain_recv_channel(&mut self, state: &mut State) -> Result<Option<PendingResize>, ()> {
        // Resize storms (live window drags) queue faster than
        // ResizePseudoConsole drains them — the console host performs a full
        // viewport reflow per call. Within one drain only the newest size
        // matters: intermediate sizes carry no information (the next message
        // supersedes them), and the final size is never dropped because it is
        // always the last one seen. The slower the host reflows, the more
        // sizes pile up per drain and the harder the coalescing works.
        //
        // Grid-only and full resizes coalesce into the same slot, and a full
        // resize never loses to a later grid-only one: the child must still be
        // told about the geometry it is already producing output for. The
        // reverse is fine — a full resize supersedes a pending grid-only one,
        // because it reflows the grid too.
        let mut resize: Option<PendingResize> = None;
        while let Some(msg) = self.rx.recv() {
            match msg {
                Msg::Input(input) => state.write_list.push_back(input),
                Msg::Resize(window_size) => {
                    resize = Some(PendingResize {
                        window_size,
                        notify_pty: true,
                    })
                }
                Msg::ResizeGrid(window_size) => {
                    let notify_pty = resize.is_some_and(|pending| pending.notify_pty);
                    resize = Some(PendingResize {
                        window_size,
                        notify_pty,
                    });
                }
                Msg::Shutdown => return Err(()),
            }
        }

        Ok(resize)
    }

    /// 向 conhost 要光标真值并把本地网格滚到同一坐标系。
    ///
    /// `expect_rows` 非 None 时只采信视口高度已经等于该值的读数：
    /// `ResizePseudoConsole` 之后 conhost 若还停在旧几何，它报的行号属于上一
    /// 个坐标系，照它对账会把网格滚到更错的位置。宁可放弃这一次——死线上的
    /// 兜底探针会再来一遍。
    fn realign_to_conpty(&mut self, stage: &str, expect_rows: Option<u16>) {
        let Some(probe) = self.pty.child_pid().and_then(conpty_cursor_probe) else {
            return;
        };
        if resize_trace_enabled() {
            eprintln!(
                "[slterm:resize-trace] {stage} conhost row={} rows={} expect_rows={expect_rows:?}",
                probe.row, probe.rows,
            );
        }
        if expect_rows.is_some_and(|rows| rows != probe.rows) {
            return;
        }
        let mut terminal = self.terminal.lock();
        trace_terminal_state(&format!("before-{stage}"), &terminal);
        terminal.conpty_realign(probe.row);
        trace_terminal_state(&format!("after-{stage}"), &terminal);
        drop(terminal);
        self.event_proxy.send_event(Event::Wakeup);
    }

    #[inline]
    fn pty_read(&mut self, state: &mut State, buf: &mut [u8]) -> io::Result<usize> {
        let mut processed = 0;
        // DA1/DSR 接管族的产出累积（剥离后字节）；替代旧 unprocessed 的原位
        // 累积——剥离只删不增，但跨块残片拼接使进出不再同缓冲。
        let mut stripped: Vec<u8> = Vec::new();

        // Reserve the next terminal lock for PTY reading.
        let _terminal_lease = Some(self.terminal.lease());
        let mut terminal = None;

        loop {
            // Read from the PTY.
            match self.pty.reader().read(buf) {
                // This is received on Windows/macOS when no more data is readable from the PTY.
                Ok(0) if stripped.is_empty() => break,
                Ok(got) => {
                    // Startup profiling: the process-wide first PTY output ≈
                    // the console host finished its bring-up handshake and
                    // the shell started talking. The first chunks are dumped
                    // escaped so a silent boot can be aligned with the host's
                    // handshake byte for byte.
                    {
                        use std::sync::atomic::{AtomicUsize, Ordering};
                        static CHUNKS: AtomicUsize = AtomicUsize::new(0);
                        let index = CHUNKS.fetch_add(1, Ordering::Relaxed);
                        if index == 0 {
                            crate::pty_trace("first conout bytes");
                        }
                        if index < 12 {
                            let shown = &buf[..got.min(200)];
                            crate::pty_trace(&format!(
                                "conout chunk {index} ({got} bytes): {}",
                                shown.escape_ascii()
                            ));
                        }
                    }
                    // DA1/DSR 全量接管：扣尾防裂 → 检测代答（原始字节上）→
                    // 剥离。None = 整块剥离（残片/启动序列独占），不置
                    // drained，继续读；代答直写子进程 stdin。
                    if let Some(out) = state
                        .stream
                        .strip_and_answer(&buf[..got], self.pty.writer())
                    {
                        stripped.extend_from_slice(&out);
                    }
                }
                Err(err) => match err.kind() {
                    ErrorKind::Interrupted | ErrorKind::WouldBlock => {
                        // Go back to mio if we are caught up on parsing and the PTY would block.
                        if stripped.is_empty() {
                            break;
                        }
                    }
                    _ => return Err(err),
                },
            }

            // 整块剥离后无字节可提交：继续读，不碰终端锁。
            if stripped.is_empty() {
                continue;
            }

            // Attempt to lock the terminal.
            let terminal = match &mut terminal {
                Some(terminal) => terminal,
                None => terminal.insert(match self.terminal.try_lock_unfair() {
                    // Force block if we are at the buffer size limit.
                    None if stripped.len() >= READ_BUFFER_SIZE => self.terminal.lock_unfair(),
                    None => continue,
                    Some(terminal) => terminal,
                }),
            };

            state
                .stream
                .feed(&mut **terminal, &self.event_proxy, &stripped);
            trace_terminal_state("after-pty-read", terminal);

            processed += stripped.len();
            stripped.clear();

            // Assure we're not blocking the terminal too long unnecessarily.
            if processed >= MAX_LOCKED_READ {
                break;
            }
        }

        // Queue terminal redraw unless all processed bytes were synchronized.
        if state.stream.sync_bytes_count() < processed && processed > 0 {
            self.event_proxy.send_event(Event::Wakeup);
        }

        // Boot profiling: a readable wake that carried no bytes is the
        // signature of a lost or spurious wakeup; report only the first few.
        if processed == 0 {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static EMPTY_READS: AtomicUsize = AtomicUsize::new(0);
            if EMPTY_READS.fetch_add(1, Ordering::Relaxed) < 8 {
                crate::pty_trace("pty_read: readable wake with no bytes");
            }
        }

        Ok(processed)
    }

    #[inline]
    fn pty_write(&mut self, state: &mut State) -> io::Result<()> {
        state.ensure_next();

        'write_many: while let Some(mut current) = state.take_current() {
            'write_one: loop {
                match self.pty.writer().write(current.remaining_bytes()) {
                    Ok(0) => {
                        state.set_current(Some(current));
                        break 'write_many;
                    }
                    Ok(n) => {
                        {
                            use std::sync::atomic::{AtomicUsize, Ordering};
                            static WRITES: AtomicUsize = AtomicUsize::new(0);
                            let index = WRITES.fetch_add(1, Ordering::Relaxed);
                            if index < 12 {
                                let shown = &current.remaining_bytes()[..n.min(200)];
                                crate::pty_trace(&format!(
                                    "conin write {index} ({n} bytes): {}",
                                    shown.escape_ascii()
                                ));
                            }
                        }
                        current.advance(n);
                        if current.finished() {
                            state.goto_next();
                            break 'write_one;
                        }
                    }
                    Err(err) => {
                        state.set_current(Some(current));
                        match err.kind() {
                            ErrorKind::Interrupted | ErrorKind::WouldBlock => break 'write_many,
                            _ => return Err(err),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn spawn(mut self) -> JoinHandle<(Self, State)> {
        thread::spawn_named("PTY reader", move || {
            let mut state = State::default();
            // DA1/DSR 接管:DSR 剥离门控由 PTY 后端注入(Windows ConPTY 按
            // OS build 预计算;其他后端默认 false = 透传交 Term 自答)。
            state.stream.set_strip_dsr(self.pty.strip_dsr_queries());
            let mut buf = [0u8; READ_BUFFER_SIZE];

            let poll_opts = PollMode::Level;
            let mut interest = PollingEvent::readable(0);

            // Register TTY through EventedRW interface.
            if let Err(err) = unsafe { self.pty.register(&self.poll, interest, poll_opts) } {
                error!("Event loop registration error: {err}");
                return (self, state);
            }

            let mut events = Events::with_capacity(NonZeroUsize::new(1024).unwrap());

            // Reason the transport died without a child exit, reported after
            // the loop: without it the app never learns the session is gone
            // and the tab turns into a zombie (unresponsive input, no notice).
            let mut failure: Option<String> = None;

            'event_loop: loop {
                // Wakeup the event loop when a synchronized update timeout or
                // the pending ConPTY align deadline was reached.
                let deadline = match (state.stream.next_sync_timeout(), state.align_at) {
                    (Some(sync), Some(align)) => Some(sync.min(align)),
                    (sync, align) => sync.or(align),
                };
                let timeout = deadline.map(|at| at.saturating_duration_since(Instant::now()));

                events.clear();
                if let Err(err) = self.poll.wait(&mut events, timeout) {
                    match err.kind() {
                        ErrorKind::Interrupted => continue,
                        _ => {
                            error!("Event loop polling error: {err}");
                            failure = Some(format!("event loop polling error: {err}"));
                            break 'event_loop;
                        }
                    }
                }

                // ConPTY 光标对账到点：向 conhost 要真值并把本地网格滚到同
                // 一坐标系（机理见 `Term::conpty_realign`）。这是兜底的一次
                // ——resize 边界上已经同步对过一次账，这里只补 conhost 事后
                // 才塌缩的情况，所以不校验视口高度。
                if state.align_at.is_some_and(|at| Instant::now() >= at) {
                    state.align_at = None;
                    self.realign_to_conpty("align", None);
                }

                // Handle synchronized update timeout. The align deadline can
                // wake the loop early with empty events — only a genuinely
                // expired sync window may force-stop the synchronized update.
                if events.is_empty() && self.rx.peek().is_none() {
                    if state
                        .stream
                        .next_sync_timeout()
                        .is_some_and(|st| Instant::now() >= st)
                    {
                        state.stream.stop_sync(&mut *self.terminal.lock());
                        self.event_proxy.send_event(Event::Wakeup);
                    }
                    continue;
                }

                // Handle channel events, if there are any.
                let pending_resize = match self.drain_recv_channel(&mut state) {
                    Ok(resize) => resize,
                    Err(()) => break,
                };

                if let Some(PendingResize {
                    window_size,
                    notify_pty,
                }) = pending_resize
                {
                    // A resize is a stream boundary, not a UI-side property.
                    // Drain everything already readable while the grid still
                    // has the old geometry; otherwise old-width absolute CUP
                    // output can be parsed into a new-width grid. This is the
                    // Drain readable bytes against the old grid before the
                    // size change lands, so CUP sequences keep their geometry.
                    loop {
                        match self.pty_read(&mut state, &mut buf) {
                            Ok(processed) if processed >= MAX_LOCKED_READ => continue,
                            Ok(_) => break,
                            Err(err) => {
                                error!("PTY read before resize failed: {err}");
                                failure = Some(format!("PTY read before resize failed: {err}"));
                                break 'event_loop;
                            }
                        }
                    }

                    // Match ConPTY's order: reflow the client model
                    // first, then ask ConPTY to resize. ResizePseudoConsole is
                    // synchronous; repaint bytes it produces are consumed by
                    // the normal readable-event path below against this grid.
                    {
                        let mut terminal = self.terminal.lock();
                        trace_terminal_state("before-resize", &terminal);
                        terminal.resize(window_size);
                        trace_terminal_state(
                            if notify_pty {
                                "after-resize"
                            } else {
                                "after-resize-grid"
                            },
                            &terminal,
                        );
                    }
                    // Grid-only: the child keeps the geometry it is producing
                    // output for, so conhost's buffer is untouched and there is
                    // nothing to reconcile against — skip both the
                    // ResizePseudoConsole and the align probe. The reflow above
                    // is what keeps the viewport on screen honest while a drag
                    // is still in flight.
                    if notify_pty {
                        self.pty.on_resize(window_size);
                        state.stream.resize(window_size);
                        // 对账的唯一可信时点就是这里：`ResizePseudoConsole` 是同步
                        // 的，返回时 conhost 已按新宽度 rewrap 完毕，而本地网格还是
                        // 纯 reflow 的结果——两侧都没有被重绘字节动过，行号差就是
                        // 两种折行语义的真实差值。等到下面 pty_read 把 PSReadLine
                        // 的绝对 CUP 解析进来，光标已经被搬到 conhost 的坐标上，
                        // 差值归零，判据就永久丢失了（字节取证：分屏后 after-resize
                        // 是 cursor=15/prompts=[15]，120ms 后的 before-align 已经变成
                        // cursor=19/prompts=[15]——错位既成事实却测不出来）。
                        self.realign_to_conpty("align-sync", Some(window_size.num_lines));
                        // conhost 事后才做的塌缩/重锚探不到，留一次死线兜底；新
                        // resize 顺延死线，风暴天然合并成一次探针。
                        state.align_at = Some(Instant::now() + ALIGN_DELAY);
                        self.event_proxy.send_event(Event::Wakeup);
                    } else {
                        self.event_proxy.send_event(Event::Wakeup);
                    }
                }

                for event in events.iter() {
                    match event.key {
                        tty::PTY_CHILD_EVENT_TOKEN => {
                            if let Some(tty::ChildEvent::Exited(status)) =
                                self.pty.next_child_event()
                            {
                                if let Some(status) = status {
                                    self.event_proxy.send_event(Event::ChildExit(status));
                                }
                                if self.drain_on_exit {
                                    let _ = self.pty_read(&mut state, &mut buf);
                                }
                                // DA1/DSR 接管 EOF 冲刷：扣尾防裂留下的残片是
                                // 不完整转义序列，EOF 时永远没有后续字节来拼——
                                // 不丢字节是接管族红线，原样喂进解析器（残片
                                // 序列对解析器无害，截断转义按无效序列消化）。
                                // 与 drain_on_exit 无关：残片是已读入的字节，
                                // 不属于「排空管道」语义。
                                let pending = state.stream.take_pending_query();
                                if !pending.is_empty() {
                                    let mut terminal = self.terminal.lock();
                                    state
                                        .stream
                                        .feed(&mut *terminal, &self.event_proxy, &pending);
                                    drop(terminal);
                                }
                                self.terminal.lock().exit();
                                self.event_proxy.send_event(Event::Wakeup);
                                break 'event_loop;
                            }
                        }

                        tty::PTY_READ_WRITE_TOKEN => {
                            if event.is_interrupt() {
                                // Don't try to do I/O on a dead PTY.
                                continue;
                            }

                            if event.readable
                                && let Err(err) = self.pty_read(&mut state, &mut buf)
                            {
                                error!("Error reading from PTY in event loop: {err}");
                                failure = Some(format!("PTY read failed: {err}"));
                                break 'event_loop;
                            }

                            if event.writable
                                && let Err(err) = self.pty_write(&mut state)
                            {
                                error!("Error writing to PTY in event loop: {err}");
                                failure = Some(format!("PTY write failed: {err}"));
                                break 'event_loop;
                            }
                        }
                        _ => (),
                    }
                }

                // Register write interest if necessary.
                let needs_write = state.needs_write();
                if needs_write != interest.writable {
                    interest.writable = needs_write;

                    // Re-register with new interest.
                    self.pty
                        .reregister(&self.poll, interest, poll_opts)
                        .unwrap();
                }
            }

            // Announce a transport death exactly like a child exit so the
            // app's existing teardown runs (`exit()` triggers `Event::Exit`).
            // The child-exit and shutdown paths leave `failure` unset.
            if let Some(reason) = failure {
                self.event_proxy.send_event(Event::PtyFailure(reason));
                self.terminal.lock().exit();
                self.event_proxy.send_event(Event::Wakeup);
            }

            // The evented instances are not dropped here so deregister them explicitly.
            let _ = self.pty.deregister(&self.poll);

            (self, state)
        })
    }
}

/// Helper type which tracks how much of a buffer has been written.
struct Writing {
    source: Cow<'static, [u8]>,
    written: usize,
}

pub struct Notifier(pub EventLoopSender);

impl event::Notify for Notifier {
    fn notify<B>(&self, bytes: B)
    where
        B: Into<Cow<'static, [u8]>>,
    {
        let bytes = bytes.into();
        // Terminal hangs if we send 0 bytes through.
        if bytes.is_empty() {
            return;
        }

        let _ = self.0.send(Msg::Input(bytes));
    }
}

impl event::OnResize for Notifier {
    fn on_resize(&mut self, window_size: WindowSize) {
        let _ = self.0.send(Msg::Resize(window_size));
    }
}

impl Notifier {
    /// Reflow the grid to `window_size` and leave the child on its old size.
    /// See [`Msg::ResizeGrid`] for why the two halves are split.
    pub fn on_resize_grid(&mut self, window_size: WindowSize) {
        let _ = self.0.send(Msg::ResizeGrid(window_size));
    }
}

#[derive(Debug)]
pub enum EventLoopSendError {
    /// Error polling the event loop.
    Io(io::Error),

    /// Error sending a message to the event loop.
    Send(mpsc::SendError<Msg>),
}

impl Display for EventLoopSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EventLoopSendError::Io(err) => err.fmt(f),
            EventLoopSendError::Send(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for EventLoopSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventLoopSendError::Io(err) => err.source(),
            EventLoopSendError::Send(err) => err.source(),
        }
    }
}

#[derive(Clone)]
pub struct EventLoopSender {
    sender: Sender<Msg>,
    poller: Arc<Poller>,
}

impl EventLoopSender {
    /// 为非 PTY 传输创建消息通道，继续复用应用既有的输入、缩放和关闭协议。
    pub fn standalone() -> io::Result<(Self, Receiver<Msg>)> {
        let (sender, receiver) = mpsc::channel();
        Ok((
            Self {
                sender,
                poller: Arc::new(Poller::new()?),
            },
            receiver,
        ))
    }

    pub fn send(&self, msg: Msg) -> Result<(), EventLoopSendError> {
        self.sender.send(msg).map_err(EventLoopSendError::Send)?;
        self.poller.notify().map_err(EventLoopSendError::Io)
    }
}

/// All of the mutable state needed to run the event loop.
///
/// Contains list of items to write, current write state, etc. Anything that
/// would otherwise be mutated on the `EventLoop` goes here.
#[derive(Default)]
pub struct State {
    write_list: VecDeque<Cow<'static, [u8]>>,
    writing: Option<Writing>,
    stream: StreamProcessor,
    /// ConPTY 光标对账的死线：resize 提交后 `ALIGN_DELAY` 触发，见
    /// `conpty_cursor_probe`。
    align_at: Option<Instant>,
}

impl State {
    #[inline]
    fn ensure_next(&mut self) {
        if self.writing.is_none() {
            self.goto_next();
        }
    }

    #[inline]
    fn goto_next(&mut self) {
        self.writing = self.write_list.pop_front().map(Writing::new);
    }

    #[inline]
    fn take_current(&mut self) -> Option<Writing> {
        self.writing.take()
    }

    #[inline]
    fn needs_write(&self) -> bool {
        self.writing.is_some() || !self.write_list.is_empty()
    }

    #[inline]
    fn set_current(&mut self, new: Option<Writing>) {
        self.writing = new;
    }
}

impl Writing {
    #[inline]
    fn new(c: Cow<'static, [u8]>) -> Writing {
        Writing {
            source: c,
            written: 0,
        }
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        self.written += n;
    }

    #[inline]
    fn remaining_bytes(&self) -> &[u8] {
        &self.source[self.written..]
    }

    #[inline]
    fn finished(&self) -> bool {
        self.written >= self.source.len()
    }
}

struct PeekableReceiver<T> {
    rx: Receiver<T>,
    peeked: Option<T>,
}

impl<T> PeekableReceiver<T> {
    fn new(rx: Receiver<T>) -> Self {
        Self { rx, peeked: None }
    }

    fn peek(&mut self) -> Option<&T> {
        if self.peeked.is_none() {
            self.peeked = self.rx.try_recv().ok();
        }

        self.peeked.as_ref()
    }

    fn recv(&mut self) -> Option<T> {
        if self.peeked.is_some() {
            self.peeked.take()
        } else {
            match self.rx.try_recv() {
                Err(TryRecvError::Disconnected) => panic!("event loop channel closed"),
                res => res.ok(),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    use crate::event::VoidListener;
    use crate::term::Config;
    use crate::term::test::TermSize;

    #[test]
    fn animation_snapshots_cannot_observe_a_partial_synchronized_update() {
        use crate::render::{RenderSnapshot, SnapshotConfig};
        let mut term = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
        let mut stream = StreamProcessor::default();
        let cfg = SnapshotConfig { rows: 2, cols: 20 };
        stream.feed(&mut term, &VoidListener, b"old");
        let before = RenderSnapshot::capture(&term, &cfg);
        for bytes in [
            b"\x1b[?20".as_slice(),
            b"26h\r",
            b"new",
            b"\x1b[10G",
            b"\x1b[?2026",
        ] {
            stream.feed(&mut term, &VoidListener, bytes);
            // Animation frames read the grid even without a Wakeup.
            let frame = RenderSnapshot::capture(&term, &cfg);
            assert_eq!(
                frame.cursor.as_ref().unwrap().col,
                before.cursor.as_ref().unwrap().col
            );
            assert_eq!(term.grid()[Line(0)][Column(0)].c, 'o');
        }
        stream.feed(&mut term, &VoidListener, b"l");
        assert_eq!(RenderSnapshot::capture(&term, &cfg).cursor.unwrap().col, 9);
        assert_eq!(term.grid()[Line(0)][Column(0)].c, 'n');
        stream.feed(&mut term, &VoidListener, b"\x1b[?2026h\rtimeout");
        assert_eq!(term.grid()[Line(0)][Column(0)].c, 'n');
        stream.stop_sync(&mut term);
        assert_eq!(term.grid()[Line(0)][Column(0)].c, 't');
    }

    #[test]
    fn shell_identity_cwd_and_title_keep_wire_order_across_chunk_boundaries() {
        #[derive(Clone, Default)]
        struct Listener(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
        impl EventListener for Listener {
            fn send_event(&self, event: Event) {
                let label = match event {
                    Event::UserVar { value, .. } => format!("shell:{value}"),
                    Event::CwdReport(cwd) => format!("cwd:{cwd}"),
                    Event::Title(title) => format!("title:{title}"),
                    _ => return,
                };
                self.0.lock().unwrap().push(label);
            }
        }
        let bytes = b"\x1b]1337;SetUserVar=slterm_shell=cmVtb3Rl\x07\x1b]7;file://box/remote\x07\x1b]2;remote\x07\x1b]1337;SetUserVar=slterm_shell=bG9jYWw=\x07\x1b]7;file://localhost/local\x07";
        for split in 0..=bytes.len() {
            let listener = Listener::default();
            let size = TermSize::new(80, 24);
            let mut terminal = Term::new(Config::default(), &size, listener.clone());
            let mut stream = StreamProcessor::default();
            stream.feed(&mut terminal, &listener, &bytes[..split]);
            stream.feed(&mut terminal, &listener, &bytes[split..]);
            assert_eq!(
                *listener.0.lock().unwrap(),
                [
                    "shell:remote",
                    "cwd:/remote",
                    "title:remote",
                    "shell:local",
                    "cwd:/local"
                ],
                "split at {split}"
            );
        }
    }

    #[test]
    fn semantic_input_boundary_survives_chunking_and_scrolling() {
        use crate::index::{Column, Line, Point};

        let bytes = b"\x1b]133;A\x07[first]\r\n>\x1b]133;B\x07pause";
        for split in 0..=bytes.len() {
            let mut terminal = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
            let mut stream = StreamProcessor::default();
            stream.feed(&mut terminal, &VoidListener, &bytes[..split]);
            stream.feed(&mut terminal, &VoidListener, &bytes[split..]);
            assert_eq!(
                terminal.slterm_prompt_input_point(),
                Some(Point::new(Line(1), Column(1)))
            );
            stream.feed(&mut terminal, &VoidListener, b"\r\n");
            assert_eq!(
                terminal.slterm_prompt_input_point(),
                Some(Point::new(Line(0), Column(1)))
            );
            stream.feed(&mut terminal, &VoidListener, b"\x1b]133;C\x07");
            assert_eq!(terminal.slterm_prompt_input_point(), None);
        }
    }

    #[test]
    fn input_boundaries_require_a_prompt_and_do_not_survive_reflow_or_reset() {
        let mut terminal = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
        let mut stream = StreamProcessor::default();
        stream.feed(&mut terminal, &VoidListener, b"\x1b]133;B\x07");
        assert_eq!(terminal.slterm_prompt_input_point(), None);
        for ending in [b"\x1bc".as_slice(), b"\x1b]133;D;0\x07", b"\x1b]133;A\x07"] {
            stream.feed(
                &mut terminal,
                &VoidListener,
                b"\x1b]133;A\x07\x1b]133;B\x07",
            );
            assert!(terminal.slterm_prompt_input_point().is_some());
            stream.feed(&mut terminal, &VoidListener, ending);
            assert_eq!(terminal.slterm_prompt_input_point(), None);
        }
        stream.feed(
            &mut terminal,
            &VoidListener,
            b"\x1b]133;A\x07\x1b]133;B\x07",
        );
        terminal.resize(TermSize::new(10, 2));
        assert_eq!(terminal.slterm_prompt_input_point(), None);
    }

    #[test]
    fn shell_semantic_events_track_the_active_prompt() {
        let size = TermSize::new(80, 24);
        let mut terminal = Term::new(Config::default(), &size, VoidListener);
        let listener = VoidListener;
        let mut stream = StreamProcessor::default();

        stream.feed(&mut terminal, &listener, b"\x1b]133;A\x07custom prompt :: ");
        assert!(terminal.slterm_prompt_active());

        stream.feed(&mut terminal, &listener, b"\x1b]133;C\x07");
        assert!(!terminal.slterm_prompt_active());

        stream.feed(&mut terminal, &listener, b"\x1b]133;A\x07next prompt :: ");
        assert!(terminal.slterm_prompt_active());

        stream.feed(&mut terminal, &listener, b"\x1b]133;D;0\x07");
        assert!(!terminal.slterm_prompt_active());
    }

    // ─── DA1/DSR 接管族（旧栈 reader_tests 语义迁移 + 状态机新增）───

    #[test]
    fn strip_osc_title_bel() {
        assert_eq!(strip_conpty_startup(b"\x1b]0;pwsh\x07"), b"");
    }

    #[test]
    fn strip_clear_screen() {
        assert_eq!(strip_conpty_startup(b"\x1b[2J"), b"");
    }

    #[test]
    fn strip_clear_screen_3j() {
        // CSI 3J（清屏含滚动缓冲）与 2J 一并剥离。
        assert_eq!(strip_conpty_startup(b"\x1b[3J"), b"");
    }

    #[test]
    fn strip_cursor_home() {
        assert_eq!(strip_conpty_startup(b"\x1b[H"), b"");
    }

    #[test]
    fn strip_dsr() {
        assert_eq!(strip_conpty_startup(b"\x1b[6n"), b"");
    }

    #[test]
    fn strip_cursor_visibility() {
        assert_eq!(strip_conpty_startup(b"\x1b[?25h"), b"");
        assert_eq!(strip_conpty_startup(b"\x1b[?25l"), b"");
    }

    #[test]
    fn strip_da1_query_in_startup() {
        // DA1 全量接管：启动窗口内的 DA1 查询一并剥离，不透传。
        assert_eq!(strip_conpty_startup(b"\x1b[c"), b"");
        assert_eq!(strip_conpty_startup(b"\x1b[0c"), b"");
    }

    #[test]
    fn strip_preserves_da1_lookalikes() {
        // DA2（ESC[>c）/ 带参变体（ESC[1c）不是 DA1 查询——不动。
        assert_eq!(strip_conpty_startup(b"\x1b[>c"), b"\x1b[>c");
        assert_eq!(strip_conpty_startup(b"\x1b[1c"), b"\x1b[1c");
    }

    #[test]
    fn preserve_normal_text() {
        let input = b"PS C:\\Users\\test> ";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_startup_preserve_shell_output() {
        let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[HPS C:\\> ";
        assert_eq!(strip_conpty_startup(input), b"PS C:\\> ");
    }

    #[test]
    fn preserve_osc7_cwd() {
        let input = b"\x1b]7;file:///C:/Users\x1b\\";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_preserves_non_title_osc() {
        // OSC 1/3/4/9（非窗口标题类）不被剥离——剥离仅针对 OSC 0/2。
        let cases: [&[u8]; 4] = [
            b"\x1b]1;icon-title\x07",
            b"\x1b]3;prop\x07",
            b"\x1b]4;0;#000000\x07",
            b"\x1b]9;notify\x07",
        ];
        for case in cases {
            assert_eq!(strip_conpty_startup(case), case);
        }
    }

    #[test]
    fn strip_startup_with_large_payload() {
        // >4KB 连续非启动数据完整保留（验证大数据块不被截断）。
        let payload = vec![b'X'; 10000];
        let prefix = b"\x1b]0;pwsh\x07\x1b[2J\x1b[H";
        let mut input = prefix.to_vec();
        input.extend_from_slice(&payload);
        input.extend_from_slice(b"END_MARKER");

        let result = strip_conpty_startup(&input);
        assert_eq!(result.len(), payload.len() + 10);
        assert!(result.ends_with(b"END_MARKER"));
        assert!(!result.starts_with(b"\x1b]"));
    }

    #[test]
    fn strip_preserves_save_cursor() {
        // ESC[s（标准 VT100 保存光标）不被剥离。
        let input = b"\x1b[s hello";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_preserves_restore_cursor() {
        // ESC[u（标准 VT100 恢复光标）不被剥离。
        let input = b"\x1b[u world";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_startup_with_read_buffer_boundary() {
        // 读缓冲量级（READ_BUFFER_SIZE = 1MiB）数据上剥离正常工作——
        // 旧栈 16K 边界例按新缓冲尺寸改写（语义：近缓冲上限不截断）。
        let payload = vec![b'Y'; READ_BUFFER_SIZE];
        let prefix = b"\x1b[2J\x1b[H";
        let mut input = prefix.to_vec();
        input.extend_from_slice(&payload);
        input.extend_from_slice(b"TAIL");

        let result = strip_conpty_startup(&input);
        assert_eq!(result.len(), READ_BUFFER_SIZE + 4);
        assert!(result.starts_with(b"Y"));
        assert!(result.ends_with(b"TAIL"));
    }

    // ─── mirror_da1_query 纯函数族 ───

    #[test]
    fn da1_standard_query_detected() {
        assert!(mirror_da1_query(b"\x1b[c"));
    }

    #[test]
    fn da1_with_leading_zero_detected() {
        assert!(mirror_da1_query(b"\x1b[0c"));
    }

    #[test]
    fn da2_not_detected() {
        assert!(!mirror_da1_query(b"\x1b[>c"));
    }

    #[test]
    fn plain_text_not_falsely_detected() {
        assert!(!mirror_da1_query(b"hello [c world"));
    }

    #[test]
    fn xtversion_not_detected() {
        assert!(!mirror_da1_query(b"\x1b[>0q"));
    }

    #[test]
    fn da1_embedded_in_output_detected() {
        let input = b"prompt> \x1b[c more output";
        assert!(mirror_da1_query(input));
    }

    // ─── mirror_dsr_query / should_answer_dsr 纯函数族 ───

    #[test]
    fn dsr_standard_query_detected() {
        // DSR 光标位置查询 ESC[6n（ConPTY VtIo 握手形态）。
        assert!(mirror_dsr_query(b"\x1b[6n"));
    }

    #[test]
    fn dsr_embedded_in_startup_burst_detected() {
        // 启动突发内嵌 DSR（OSC 标题 + 清屏 + DSR 同块）。
        let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[6n";
        assert!(mirror_dsr_query(input));
    }

    #[test]
    fn dsr_lookalikes_not_detected() {
        // ESC[5n（设备状态）/ ESC[?6n（私有模式）/ ESC[65n 均非握手 DSR。
        assert!(!mirror_dsr_query(b"\x1b[5n"));
        assert!(!mirror_dsr_query(b"\x1b[?6n"));
        assert!(!mirror_dsr_query(b"\x1b[65n"));
        // 不完整前缀（跨块残片形态）不检测——由扣留层接管。
        assert!(!mirror_dsr_query(b"\x1b[6"));
        // 普通文本不误触发。
        assert!(!mirror_dsr_query(b"hello [6n world"));
    }

    #[test]
    fn dsr_answered_only_in_startup_window() {
        // 启动窗口内（drained=false）DSR → 代答 CPR。
        assert!(should_answer_dsr(false, b"\x1b[6n"));
        // 启动窗口外（drained=true）DSR → 不代答：Win11+ 透传 Term 实答；
        // Win10 家族经 apply_output_strip 门控剥离（CPR 应答即 F3 毒键）。
        assert!(!should_answer_dsr(true, b"\x1b[6n"));
        // 窗口内无 DSR → 不代答（绝不盲注——Win10 捆绑 conhost 握手发 DA1
        // 不发 DSR，盲注 CPR 会被解析为 F3 吞键，防复发锚点）。
        assert!(!should_answer_dsr(false, b"\x1b[2J\x1b[H"));
    }

    // ─── apply_output_strip 纯函数族 ───

    #[test]
    fn output_strip_drained_passthrough() {
        // 非首轮：无 DA1 的输出原样返回。
        let data = b"normal output";
        let result = apply_output_strip(true, false, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    #[test]
    fn output_strip_drained_strips_da1() {
        // 非首轮：DA1 查询剥离不透传（Ink 启动哨兵场景）。
        assert_eq!(
            apply_output_strip(true, false, b"prompt> \x1b[c more"),
            Some(b"prompt>  more".to_vec())
        );
        assert_eq!(
            apply_output_strip(true, false, b"\x1b[0c rest"),
            Some(b" rest".to_vec())
        );
    }

    #[test]
    fn output_strip_drained_all_da1_returns_none() {
        // 非首轮：整块全是 DA1 → None 无内容可发。
        assert_eq!(apply_output_strip(true, false, b"\x1b[c"), None);
        assert_eq!(apply_output_strip(true, false, b"\x1b[0c"), None);
    }

    #[test]
    fn output_strip_drained_dsr_gated_stripped() {
        // 毒链防复发回归（核心场景）：窗口外 DSR + 门控开启（Win10 家族）→
        // 剥离不透传（旧代码透传 → Term 自答 ESC[1;1R 回灌 stdin →
        // conhost 键事件解析为 F3 → PSReadLine CharacterSearch 吞键）。
        assert_eq!(
            apply_output_strip(true, true, b"prompt> \x1b[6nrest"),
            Some(b"prompt> rest".to_vec())
        );
        // 整块全是 DSR → None 无内容可发。
        assert_eq!(apply_output_strip(true, true, b"\x1b[6n"), None);
        // DA1 与 DSR 同块 → 一并剥离。
        assert_eq!(
            apply_output_strip(true, true, b"\x1b[c\x1b[6nx"),
            Some(b"x".to_vec())
        );
    }

    #[test]
    fn output_strip_drained_dsr_ungated_passthrough() {
        // 门控关闭（Win11+）→ 窗口外 DSR 维持透传（Term 实答真实光标位置，
        // 零回归锁——Win11 主用例行为不变）。
        assert_eq!(
            apply_output_strip(true, false, b"prompt> \x1b[6nrest"),
            Some(b"prompt> \x1b[6nrest".to_vec())
        );
    }

    #[test]
    fn output_strip_first_round_unaffected_by_gate() {
        // 窗口内剥离（strip_conpty_startup 恒含 DSR）在两门控值下行为一致。
        let input = b"\x1b[2J\x1b[6nPS> ";
        assert_eq!(
            apply_output_strip(false, true, input),
            Some(b"PS> ".to_vec())
        );
        assert_eq!(
            apply_output_strip(false, false, input),
            Some(b"PS> ".to_vec())
        );
    }

    #[test]
    fn output_strip_first_round_all_stripped() {
        // 首轮全部为启动序列 → 返回 None（跳过本轮）。
        let result = apply_output_strip(false, false, b"\x1b]0;pwsh\x07\x1b[2J\x1b[H");
        assert_eq!(result, None);
    }

    #[test]
    fn output_strip_first_round_startup_plus_da1_all_stripped() {
        // 防复发回归：启动窗口内「启动序列 + DA1」整块 → 零字节（旧代码 DA1
        // 不在剥离清单 → 透传触发自答回灌）。调用方须在剥离前的原始字节上
        // 做 mirror_da1_query 检测并代答（检测前移，None 分支不漏答）。
        let result = apply_output_strip(false, false, b"\x1b[2J\x1b[c\x1b[H");
        assert_eq!(result, None);
        assert!(mirror_da1_query(b"\x1b[2J\x1b[c\x1b[H"));
    }

    #[test]
    fn output_strip_first_round_partial_strip() {
        // 首轮启动序列后跟正常输出 → 剥离前缀。
        let result = apply_output_strip(false, false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(result, Some(b"PS C:\\> ".to_vec()));
    }

    #[test]
    fn output_strip_across_buffer_boundary() {
        // BE-13：跨缓冲区边界的启动序列剥离——第一轮全为启动序列（None）
        // 不置 drained；第二轮仍有启动序列 + 真实输出，应继续剥离。
        let r1 = apply_output_strip(false, false, b"\x1b]0;pwsh\x07");
        assert_eq!(r1, None);
        let r2 = apply_output_strip(false, false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(r2, Some(b"PS C:\\> ".to_vec()));
    }

    #[test]
    fn output_strip_multi_round_all_startup_then_real() {
        // BE-13：多轮纯启动序列后出现真实输出——drained 仅在 Some 时才置。
        assert_eq!(apply_output_strip(false, false, b"\x1b]0;pwsh\x07"), None);
        assert_eq!(apply_output_strip(false, false, b"\x1b[2J"), None);
        let r3 = apply_output_strip(false, false, b"\x1b[?25h\x1b[HHello World");
        assert_eq!(r3, Some(b"Hello World".to_vec()));
    }

    #[test]
    fn output_strip_first_round_no_startup_seq() {
        // 首轮无启动序列（如 cmd.exe 场景）→ 原样返回。
        let data = b"Microsoft Windows [Version 10.0]\r\n";
        let result = apply_output_strip(false, false, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    // ─── strip_da1_queries 纯函数族 ───

    #[test]
    fn strip_da1_removes_both_forms() {
        assert_eq!(strip_da1_queries(b"\x1b[c"), b"");
        assert_eq!(strip_da1_queries(b"\x1b[0c"), b"");
    }

    #[test]
    fn strip_da1_preserves_surrounding_bytes() {
        assert_eq!(strip_da1_queries(b"ab\x1b[ccd\x1b[0cef"), b"abcdef");
    }

    #[test]
    fn strip_da1_preserves_non_da1_sequences() {
        // DA2 / XTVERSION / 带参变体 / 其它 CSI 均不动。
        assert_eq!(strip_da1_queries(b"\x1b[>c"), b"\x1b[>c");
        assert_eq!(strip_da1_queries(b"\x1b[>0q"), b"\x1b[>0q");
        assert_eq!(strip_da1_queries(b"\x1b[1c"), b"\x1b[1c");
        assert_eq!(strip_da1_queries(b"\x1b[2J"), b"\x1b[2J");
    }

    #[test]
    fn strip_da1_trailing_partial_forwarded_as_is() {
        // 纯函数本身不扣留残片（扣留是 split_trailing_query_prefix 职责）
        // ——尾部落单的 ESC/ESC[ 原样通过，记录两层的职责边界。
        assert_eq!(strip_da1_queries(b"abc\x1b"), b"abc\x1b");
        assert_eq!(strip_da1_queries(b"abc\x1b["), b"abc\x1b[");
    }

    // ─── strip_dsr_queries 纯函数族（Win10 门控剥离）───

    #[test]
    fn strip_dsr_removes_exact_form() {
        assert_eq!(strip_dsr_queries(b"\x1b[6n"), b"");
    }

    #[test]
    fn strip_dsr_preserves_surrounding_bytes() {
        assert_eq!(strip_dsr_queries(b"ab\x1b[6ncd"), b"abcd");
    }

    #[test]
    fn strip_dsr_removes_multiple_occurrences() {
        assert_eq!(strip_dsr_queries(b"\x1b[6nx\x1b[6ny"), b"xy");
    }

    #[test]
    fn strip_dsr_preserves_lookalikes() {
        // ESC[5n（设备状态）/ ESC[?6n（私有模式）/ ESC[65n 同族不动——
        // 判别集与 mirror_dsr_query 一致。
        assert_eq!(strip_dsr_queries(b"\x1b[5n"), b"\x1b[5n");
        assert_eq!(strip_dsr_queries(b"\x1b[?6n"), b"\x1b[?6n");
        assert_eq!(strip_dsr_queries(b"\x1b[65n"), b"\x1b[65n");
        assert_eq!(strip_dsr_queries(b"\x1b[2J"), b"\x1b[2J");
    }

    #[test]
    fn strip_dsr_trailing_partial_forwarded_as_is() {
        // 纯函数本身不扣留残片——尾部落单 ESC/ESC[/ESC[6 原样通过。
        assert_eq!(strip_dsr_queries(b"abc\x1b"), b"abc\x1b");
        assert_eq!(strip_dsr_queries(b"abc\x1b[6"), b"abc\x1b[6");
    }

    // ─── split_trailing_query_prefix 纯函数族 ───

    #[test]
    fn split_holds_trailing_esc() {
        let (body, pending) = split_trailing_query_prefix(b"abc\x1b".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b");
    }

    #[test]
    fn split_holds_trailing_esc_bracket() {
        let (body, pending) = split_trailing_query_prefix(b"abc\x1b[".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b[");
    }

    #[test]
    fn split_holds_trailing_esc_bracket_zero() {
        let (body, pending) = split_trailing_query_prefix(b"abc\x1b[0".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b[0");
    }

    #[test]
    fn split_holds_trailing_esc_bracket_six() {
        // ESC[6 是 DSR 握手查询前缀——扣留防跨块裂（扣留只延迟不丢字节，
        // ESC[6;..H 等 CUP 前缀同形不误伤，随下块原样拼回）。
        let (body, pending) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b[6");
    }

    #[test]
    fn split_complete_da1_not_held() {
        // 完整 DA1 查询不是前缀残片——不扣留（交检测/剥离处理）。
        let (body, pending) = split_trailing_query_prefix(b"\x1b[c".to_vec());
        assert_eq!(body, b"\x1b[c");
        assert!(pending.is_empty());
        let (body, pending) = split_trailing_query_prefix(b"\x1b[0c".to_vec());
        assert_eq!(body, b"\x1b[0c");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_non_da1_prefix_not_held() {
        // ESC[1 / ESC[? 等不是 DA1 前缀——不扣留（ESC[1c 非 DA1，不误伤）。
        let (body, pending) = split_trailing_query_prefix(b"abc\x1b[1".to_vec());
        assert_eq!(body, b"abc\x1b[1");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_plain_and_empty_input() {
        let (body, pending) = split_trailing_query_prefix(b"plain".to_vec());
        assert_eq!(body, b"plain");
        assert!(pending.is_empty());
        let (body, pending) = split_trailing_query_prefix(Vec::new());
        assert!(body.is_empty());
        assert!(pending.is_empty());
    }

    #[test]
    fn split_reassembly_closes_cross_chunk_da1() {
        // 防复发回归（跨块形态）：chunk1 = "abc ESC["，chunk2 = "c def"——
        // 拼接后检测命中 + 剥离无泄漏（旧代码半条 ESC[ 送前端跨写入拼合
        // 自答）。
        let (b1, p1) = split_trailing_query_prefix(b"abc\x1b[".to_vec());
        assert_eq!(b1, b"abc");
        assert_eq!(p1, b"\x1b[");
        let mut frame = p1;
        frame.extend_from_slice(b"c def");
        let (b2, p2) = split_trailing_query_prefix(frame);
        assert!(p2.is_empty());
        assert!(mirror_da1_query(&b2), "拼接后应检测到 DA1 → 触发代答");
        assert_eq!(strip_da1_queries(&b2), b" def", "剥离后前端无泄漏");
    }

    #[test]
    fn split_reassembly_closes_cross_chunk_dsr() {
        // 跨块 DSR 防裂回归：chunk1 尾 = "ESC[6"，chunk2 头 = "n rest"——
        // 拼接后 mirror_dsr_query 命中（漏扣留则半条 ESC[6 送前端跨写入
        // 拼合后以自身位置实答，后端代答丢失）。
        let (b1, p1) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
        assert_eq!(b1, b"abc");
        assert_eq!(p1, b"\x1b[6");
        let mut frame = p1;
        frame.extend_from_slice(b"n rest");
        let (b2, p2) = split_trailing_query_prefix(frame);
        assert!(p2.is_empty());
        assert!(mirror_dsr_query(&b2), "拼接后应检测到 DSR → 触发 CPR 代答");
    }

    #[test]
    fn split_reassembly_closes_cross_chunk_dsr_strip() {
        // 跨块 DSR 防裂（Win10 门控剥离形态）：拼接后剥离无泄漏——漏扣留
        // 则半条 ESC[6 送前端跨写入拼合自答回灌 → F3 吞键复现。
        let (_b1, p1) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
        let mut frame = p1;
        frame.extend_from_slice(b"n rest");
        let (b2, p2) = split_trailing_query_prefix(frame);
        assert!(p2.is_empty());
        assert_eq!(
            strip_dsr_queries(&b2),
            b" rest",
            "门控剥离后前端无 DSR 泄漏"
        );
    }

    // ─── inject_da1_response / inject_cpr_response 动作级（Vec 直当 writer）───

    #[test]
    fn inject_da1_response_writes_vt420_identity() {
        let mut buf: Vec<u8> = Vec::new();
        inject_da1_response(&mut buf);
        assert_eq!(buf, b"\x1b[?64;22c");
    }

    #[test]
    fn inject_da1_response_every_query_answered() {
        // 防复发回归：每次查询都应答（旧「每会话一次」语义下第二次查询
        // 无应答——接管后前端不再自答，后端漏答 = 应用干等）。
        let mut buf: Vec<u8> = Vec::new();
        inject_da1_response(&mut buf);
        inject_da1_response(&mut buf);
        assert_eq!(buf, b"\x1b[?64;22c\x1b[?64;22c");
    }

    #[test]
    fn inject_cpr_response_writes_home_position() {
        // DSR 握手代答：启动期光标恒在 1;1。
        let mut buf: Vec<u8> = Vec::new();
        inject_cpr_response(&mut buf);
        assert_eq!(buf, b"\x1b[1;1R");
    }

    // ─── strip_and_answer 状态机族（接管合并防回归，新增）───

    #[test]
    fn strip_and_answer_first_block_all_startup_keeps_window_open() {
        // BE-13：整块启动序列 → None 且不置 drained，窗口保持开放。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        assert_eq!(sp.strip_and_answer(b"\x1b]0;pwsh\x07\x1b[2J", &mut w), None);
        assert!(!sp.startup_drained);
        // 第二轮仍按窗口内语义剥离。
        let out = sp.strip_and_answer(b"\x1b[HPS> ", &mut w);
        assert_eq!(out, Some(b"PS> ".to_vec()));
        assert!(sp.startup_drained);
    }

    #[test]
    fn strip_and_answer_da1_answered_and_stripped_every_time() {
        // 每查必答 + 不透传：首轮 DA1 → 代答 + 剥离；次轮再来一条仍答。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        assert_eq!(
            sp.strip_and_answer(b"\x1b[2J\x1b[chello", &mut w),
            Some(b"hello".to_vec())
        );
        assert_eq!(
            sp.strip_and_answer(b"\x1b[c world", &mut w),
            Some(b" world".to_vec())
        );
        assert_eq!(w, b"\x1b[?64;22c\x1b[?64;22c");
    }

    #[test]
    fn strip_and_answer_dsr_answered_only_in_window() {
        // 启动窗口内 DSR → CPR 代答且剥离；窗口外再来 DSR → 不答。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        assert_eq!(
            sp.strip_and_answer(b"\x1b[6nhi", &mut w),
            Some(b"hi".to_vec())
        );
        assert_eq!(w, b"\x1b[1;1R");
        let _ = sp.strip_and_answer(b"x\x1b[6ny", &mut w);
        assert_eq!(w, b"\x1b[1;1R", "窗口外 DSR 不再代答");
    }

    #[test]
    fn strip_and_answer_dsr_outside_window_gated_by_strip_dsr() {
        // 门控开启（Win10 家族）：窗口外 DSR 剥离不透传。
        let mut sp = StreamProcessor::default();
        sp.set_strip_dsr(true);
        let mut w: Vec<u8> = Vec::new();
        let _ = sp.strip_and_answer(b"ready", &mut w);
        assert_eq!(
            sp.strip_and_answer(b"a\x1b[6nb", &mut w),
            Some(b"ab".to_vec())
        );

        // 门控关闭（Win11+）：窗口外 DSR 透传交 Term 实答。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        let _ = sp.strip_and_answer(b"ready", &mut w);
        assert_eq!(
            sp.strip_and_answer(b"a\x1b[6nb", &mut w),
            Some(b"a\x1b[6nb".to_vec())
        );
    }

    #[test]
    fn strip_and_answer_cross_chunk_da1_reassembly() {
        // 跨块防裂：chunk1 尾 ESC[ → None + 扣留；chunk2 拼回后检测命中
        // 代答 + 剥离（半条序列绝不送 parser）。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        assert_eq!(
            sp.strip_and_answer(b"abc\x1b[", &mut w),
            Some(b"abc".to_vec())
        );
        assert_eq!(
            sp.strip_and_answer(b"c def", &mut w),
            Some(b" def".to_vec())
        );
        assert_eq!(w, b"\x1b[?64;22c", "拼回后 DA1 检测命中并代答");
    }

    #[test]
    fn strip_and_answer_eof_pending_flush() {
        // EOF 冲刷：扣留残片原样吐出，不丢字节。
        let mut sp = StreamProcessor::default();
        let mut w: Vec<u8> = Vec::new();
        let _ = sp.strip_and_answer(b"tail\x1b[6", &mut w);
        assert_eq!(sp.take_pending_query(), b"\x1b[6");
        assert!(sp.take_pending_query().is_empty(), "冲刷后无残留");
    }
}
