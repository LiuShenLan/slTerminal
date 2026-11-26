//! The main event loop which performs I/O on the pseudoterminal.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
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

/// 每个 worker 的命令队列容量；发送方不得在前台阻塞等待空位。
const COMMAND_CHANNEL_CAPACITY: usize = 64;

/// Max bytes to read from the PTY before forced terminal synchronization.
pub(crate) const READ_BUFFER_SIZE: usize = 64 * 1024;

/// Max bytes to read from the PTY while the terminal is locked.
const MAX_LOCKED_READ: usize = u16::MAX as usize;

/// ConPTY 光标对账的静默期：最后一次 resize 提交后等这么久再探针，给
/// conhost 的内部整理（缓冲区塌缩/重锚）和 shell 的 resize 反应留时间。
const ALIGN_DELAY: Duration = Duration::from_millis(120);

/// 输出传输关闭后等待子进程退出事件的宽限。正常退出时输出 EOF 与退出
/// 事件的到达顺序不固定；超过该宽限仍无退出事件，说明读取端先失效而
/// 子进程仍存活，必须转为传输故障，避免终端永久停留在运行态。
const OUTPUT_CLOSED_GRACE: Duration = Duration::from_secs(2);

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
    output_watermark: tty::OutputWatermark,
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
    /// bundled 宿主首条 DA1 的迟到应答抑制；由后端注入，命中一次后清除。
    bringup_da1_pending: bool,
    /// 启动窗口：首个非全剥块通过后置位（跨边界残留窗口保持——
    /// 整块剥绝不置位，窗口对迟到的启动序列保持开放）。
    startup_drained: bool,
    /// 跨块查询前缀残片扣留（≤3 字节：ESC / ESC[ / ESC[0 / ESC[6）——
    /// 半条序列不送 parser，下块拼回，EOF 时冲刷不丢字节。
    pending_query: Vec<u8>,
    /// OSC 原始负载闸门：解析器内部会累积整条 OSC，超长未终止负载
    /// 必须在进入解析器前截断。
    osc_gate: crate::osc_cwd::OscGate,
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

    /// DA1/DSR 在剥离前的原始字节上检测并按需代答；跨块残片由下轮拼回。
    /// bundled 首条宿主 DA1 只剥离，其余 DA1 每查必答；DSR 仅启动窗口内代答。
    /// 返回 `None` = 整块剥离且不置 drained；`Some(out)` 恒非空并置 drained。
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

        // bundled 首条宿主 DA1 只剥离：迟答会成为 shell 输入；其余逐条代答。
        let queries = count_da1_queries(&body);
        let skip = queries > 0 && std::mem::take(&mut self.bringup_da1_pending);
        for _ in usize::from(skip)..queries {
            inject_da1_response(writer);
        }

        // DSR 握手按需代答:仅启动窗口内(ConPTY VtIo 握手查询)。问什么
        // 答什么,绝不盲注(握手查询集随宿主变——Win11+bundled OpenConsole
        // 1.24 实测 DA1+DSR 双发;盲注 CPR 在无查询时会被键事件模式解析
        // 为 F3 吞键)。窗口外 DSR 不代答——
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
            let filtered = self.osc_gate.filter(&bytes[advanced..offset]);
            self.advance(terminal, &filtered);
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
        let filtered = self.osc_gate.filter(&bytes[advanced..]);
        self.advance(terminal, &filtered);
    }
}

// ─── DA1/DSR 接管纯函数族 ───
// 应用 DA1 由 reader 逐条代答，`Term::identify_terminal` 仅作漏网兜底；
// bundled 首条宿主 DA1 与一切无查询盲注均禁止回写。

/// DA1 应答注入（ESC[c / ESC[0c → ESC[?64;22c，VT220 + 色彩能力）。
/// 缺应答 Ink 启动阻塞约 60s；调用方只跳过 bundled 首条宿主查询。
/// 查询不透传，core 是唯一应答方；检测纯函数为 `count_da1_queries`。
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
///   本轮且不置 drained（跨边界残留窗口保持开放）。窗口内行为与
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

/// 统计输出字节流中的 DA1 终端查询（ESC[c 或 ESC[0c）。
/// DA2 (ESC[>c) 和 XTVERSION (ESC[>0q) 不计入——两者走不同的检测路径。
/// 滑动窗口扫描，在 read 块（拼接扣尾后）上调用。
fn count_da1_queries(data: &[u8]) -> usize {
    let mut count = 0;
    for i in 0..data.len().saturating_sub(2) {
        if data[i] == 0x1b && data[i + 1] == b'[' {
            let rest = &data[i + 2..];
            // ESC[c — 不含额外参数的标准 DA1 查询。
            if rest.first() == Some(&b'c') {
                count += 1;
            }
            // ESC[0c — 含前导 0 的变体。
            if rest.len() >= 2 && rest[0] == b'0' && rest[1] == b'c' {
                count += 1;
            }
        }
    }
    count
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
    /// captures the received-output watermark and drains only chunks up to that
    /// mark against the old geometry. Later output stays queued for the new
    /// geometry, so a continuous producer cannot postpone the resize forever.
    ResizeGrid(WindowSize),

    /// 只由 worker 修改显示滚动位置。
    Scroll(crate::grid::Scroll),

    /// 自动拖选：先滚动，再用滚动后的视口更新选区终点。
    ScrollSelection {
        scroll: crate::grid::Scroll,
        lines: usize,
        point: crate::index::Point<usize>,
        side: crate::index::Side,
    },

    /// 只由 worker 修改选区。
    Select(crate::selection::SelectionCommand),

    /// 只读普通文本搜索；worker 返回当前画面与回滚的匹配位置。
    SearchLiteral {
        query: String,
        reply: mpsc::SyncSender<Vec<crate::term::search::Match>>,
    },

    /// 只读导出当前画面与回滚文本；worker 返回输出 revision 与纯文本。
    ReadText {
        reply: mpsc::SyncSender<crate::term::TextSnapshot>,
    },

    /// 配色变化也只进入所属 worker，随后发布新帧。
    SetColorScheme(bool),
}

fn apply_display_command<T: EventListener>(terminal: &mut Term<T>, msg: &Msg) -> bool {
    match msg {
        Msg::Scroll(scroll) => terminal.scroll_display(*scroll),
        Msg::ScrollSelection {
            scroll,
            lines,
            point,
            side,
        } => {
            terminal.scroll_display(*scroll);
            let point = terminal.visual_viewport_to_point(*lines, *point);
            if let Some(selection) = terminal.selection.as_mut() {
                selection.update(point, *side);
            }
        }
        Msg::Select(crate::selection::SelectionCommand::Set(selection)) => {
            terminal.selection = selection.clone();
        }
        Msg::Select(crate::selection::SelectionCommand::Update { point, side }) => {
            if let Some(selection) = terminal.selection.as_mut() {
                selection.update(*point, *side);
            }
        }
        Msg::Select(crate::selection::SelectionCommand::Extend(motion)) => {
            terminal.extend_selection(*motion);
        }
        Msg::Select(crate::selection::SelectionCommand::SelectAll) => terminal.select_all(),
        Msg::Select(crate::selection::SelectionCommand::Clear) => terminal.selection = None,
        Msg::SetColorScheme(dark) => terminal.set_color_scheme(*dark),
        Msg::SearchLiteral { query, reply } => {
            let _ = reply.send(terminal.search_literal(query));
            return false;
        }
        Msg::Input(_)
        | Msg::Shutdown
        | Msg::Resize(_)
        | Msg::ResizeGrid(_)
        | Msg::ReadText { .. } => {
            return false;
        }
    }
    true
}

#[cfg(feature = "test-support")]
pub fn apply_display_command_for_test<T: EventListener>(terminal: &mut Term<T>, msg: &Msg) -> bool {
    apply_display_command(terminal, msg)
}

/// The main event loop.
///
/// Handles all the PTY I/O and runs the PTY parser which updates terminal
/// state.
pub struct EventLoop<T: tty::EventedPty, U: EventListener> {
    poll: Arc<Poller>,
    pty: T,
    rx: PeekableReceiver<Msg>,
    tx: SyncSender<Msg>,
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
        let (tx, rx) = mpsc::sync_channel(COMMAND_CHANNEL_CAPACITY);
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
                Msg::Input(input) => {
                    // 防御纵深：旧路径或测试注入的空项也不得进入写队列。
                    if !input.is_empty() {
                        state.write_list.push_back(input);
                    }
                }
                Msg::Resize(window_size) => {
                    resize = Some(PendingResize {
                        window_size,
                        notify_pty: true,
                        output_watermark: self.pty.output_watermark(),
                    })
                }
                Msg::ResizeGrid(window_size) => {
                    let notify_pty = resize.is_some_and(|pending| pending.notify_pty);
                    resize = Some(PendingResize {
                        window_size,
                        notify_pty,
                        output_watermark: self.pty.output_watermark(),
                    });
                }
                Msg::Shutdown => return Err(()),
                Msg::Scroll(_)
                | Msg::ScrollSelection { .. }
                | Msg::Select(_)
                | Msg::SearchLiteral { .. }
                | Msg::SetColorScheme(_) => {
                    if apply_display_command(&mut self.terminal.lock(), &msg) {
                        state.display_dirty = true;
                    }
                }
                Msg::ReadText { reply } => {
                    // 文本快照在 worker 内同锁捕获；游标绑定最近发布的帧
                    // revision，任何网格或显示边界都保守地使旧页失效。
                    let terminal = self.terminal.lock();
                    let text = terminal.scrollback_text();
                    drop(terminal);
                    let _ = reply.send(crate::term::TextSnapshot {
                        revision: state.frame_revision,
                        text,
                    });
                }
            }
        }

        Ok(resize)
    }

    fn publish_frame(&mut self, state: &mut State) {
        state.frame_revision = state
            .frame_revision
            .checked_add(1)
            .expect("终端帧 revision 不得回绕");
        let frame =
            crate::render::TerminalFrame::capture(&self.terminal.lock(), state.frame_revision);
        self.event_proxy.send_event(Event::Frame(Arc::new(frame)));
    }

    /// 向 conhost 要光标真值并把本地网格滚到同一坐标系。
    ///
    /// `expect_rows` 非 None 时只采信视口高度已经等于该值的读数：
    /// `ResizePseudoConsole` 之后 conhost 若还停在旧几何，它报的行号属于上一
    /// 个坐标系，照它对账会把网格滚到更错的位置。宁可放弃这一次——死线上的
    /// 兜底探针会再来一遍。
    fn realign_to_conpty(&mut self, stage: &str, expect_rows: Option<u16>, state: &mut State) {
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
        self.publish_frame(state);
    }

    #[inline]
    fn pty_read(&mut self, state: &mut State, buf: &mut [u8]) -> io::Result<usize> {
        self.pty_read_until(state, buf, tty::OutputWatermark::MAX)
    }

    fn pty_read_until(
        &mut self,
        state: &mut State,
        buf: &mut [u8],
        watermark: tty::OutputWatermark,
    ) -> io::Result<usize> {
        let mut processed = 0;
        // DA1/DSR 接管族的产出累积（剥离后字节）；替代旧 unprocessed 的原位
        // 累积——剥离只删不增，但跨块残片拼接使进出不再同缓冲。
        let mut stripped: Vec<u8> = Vec::new();

        // Reserve the next terminal lock for PTY reading.
        let _terminal_lease = Some(self.terminal.lease());
        let mut terminal = None;

        loop {
            // Read from the PTY.
            match self.pty.read_before(watermark, buf) {
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

        drop(terminal);
        drop(_terminal_lease);

        // Queue terminal redraw unless all processed bytes were synchronized.
        if state.stream.sync_bytes_count() < processed && processed > 0 {
            self.publish_frame(state);
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

    pub fn spawn(mut self) -> EventLoopHandle<T, U> {
        let tx = self.channel();
        let join = thread::spawn_named("PTY reader", move || {
            let mut state = State::default();
            // DA1/DSR 接管门控由 PTY 后端注入（Windows ConPTY 按 OS build
            // 与宿主能力预计算；其他后端使用默认值）。
            state.stream.set_strip_dsr(self.pty.strip_dsr_queries());
            state.stream.bringup_da1_pending = self.pty.suppress_bringup_da1();
            let mut buf = [0u8; READ_BUFFER_SIZE];

            let poll_opts = PollMode::Level;
            let mut interest = PollingEvent::readable(0);

            // Register TTY through EventedRW interface.
            if let Err(err) = unsafe { self.pty.register(&self.poll, interest, poll_opts) } {
                error!("Event loop registration error: {err}");
                return (self, state);
            }
            self.publish_frame(&mut state);

            let mut events = Events::with_capacity(NonZeroUsize::new(1024).unwrap());

            // Reason the transport died without a child exit, reported after
            // the loop: without it the app never learns the session is gone
            // and the tab turns into a zombie (unresponsive input, no notice).
            let mut failure: Option<String> = None;
            // 子进程退出只停止 PTY I/O；worker 继续服务滚动、选区与复制帧，
            // 直到显式 Shutdown 才析构 PTY 和进程句柄。
            let mut pty_active = true;

            'event_loop: loop {
                // Wakeup the event loop when a synchronized update timeout or
                // the pending ConPTY align deadline was reached.
                let transport_deadline = state.output_closed_at.map(|at| at + OUTPUT_CLOSED_GRACE);
                let deadline = [
                    state.stream.next_sync_timeout(),
                    state.align_at,
                    transport_deadline,
                ]
                .into_iter()
                .flatten()
                .min();
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
                    self.realign_to_conpty("align", None, &mut state);
                }

                // 读取线程先于子进程事件失效（EOF/读取错误/BrokenPipe）：
                // 先记下关闭时刻并继续轮询退出事件；宽限耗尽仍无退出事件
                // 才把传输关闭转成故障，避免终端永久停留在运行态。
                if pty_active && state.output_closed_at.is_none() && self.pty.output_closed() {
                    state.output_closed_at = Some(Instant::now());
                }
                if let Some(closed_at) = state.output_closed_at
                    && Instant::now() >= closed_at + OUTPUT_CLOSED_GRACE
                {
                    failure = Some("PTY output closed without a child exit event".to_owned());
                    break 'event_loop;
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
                        self.publish_frame(&mut state);
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
                    output_watermark,
                }) = pending_resize
                {
                    // A resize is a stream boundary, not a UI-side property.
                    // Drain everything already readable while the grid still
                    // has the old geometry; otherwise old-width absolute CUP
                    // output can be parsed into a new-width grid. This is the
                    // Drain only chunks published at the request watermark while
                    // the grid still has the old geometry; later chunks retain
                    // FIFO order and are parsed after the resize lands.
                    if pty_active {
                        loop {
                            match self.pty_read_until(&mut state, &mut buf, output_watermark) {
                                Ok(processed) if processed >= MAX_LOCKED_READ => continue,
                                Ok(_) => break,
                                Err(err) => {
                                    error!("PTY read before resize failed: {err}");
                                    failure = Some(format!("PTY read before resize failed: {err}"));
                                    break 'event_loop;
                                }
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
                    if notify_pty && pty_active {
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
                        self.realign_to_conpty(
                            "align-sync",
                            Some(window_size.num_lines),
                            &mut state,
                        );
                        // conhost 事后才做的塌缩/重锚探不到，留一次死线兜底；新
                        // resize 顺延死线，风暴天然合并成一次探针。
                        state.align_at = Some(Instant::now() + ALIGN_DELAY);
                        self.publish_frame(&mut state);
                    } else {
                        self.publish_frame(&mut state);
                    }
                }

                if std::mem::take(&mut state.display_dirty) {
                    self.publish_frame(&mut state);
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
                                    // 已读入队列的输出必须在最终帧之前全部解析；
                                    // 队列读取是非阻塞的，读到 0 即排空。
                                    loop {
                                        match self.pty_read(&mut state, &mut buf) {
                                            Ok(0) => break,
                                            Ok(_) => continue,
                                            Err(error) => {
                                                crate::pty_trace(&format!(
                                                    "pty drain after exit failed: {error}"
                                                ));
                                                break;
                                            }
                                        }
                                    }
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
                                self.publish_frame(&mut state);
                                let _ = self.pty.deregister(&self.poll);
                                pty_active = false;
                                interest.writable = false;
                                continue 'event_loop;
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
                if pty_active && needs_write != interest.writable {
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
                self.publish_frame(&mut state);
            }

            // The evented instances are not dropped here so deregister them explicitly.
            if pty_active {
                let _ = self.pty.deregister(&self.poll);
            }

            (self, state)
        });
        EventLoopHandle {
            join: Some(join),
            tx,
        }
    }
}
include!("test_support.rs");
/// PTY 事件循环线程句柄——防泄漏包装（名额归还链路）。
///
/// 裸 `JoinHandle` drop = detach：线程结束后返回值 `(EventLoop, State)`
/// 永存于线程结构，内含 Pty 永不析构——会话名额与 OS 句柄一并泄漏。
/// 本包装把取回编码进类型：显式 [`join`](EventLoopHandle::join) 取终态；
/// 未 join 而 drop 时兜底发 `Msg::Shutdown` 并 join（send 经
/// `Poller::notify` 唤醒读循环，无死等）。
#[must_use = "PTY 线程句柄：显式 join() 取终态，或 drop 走兜底关闭链"]
pub struct EventLoopHandle<T: tty::EventedPty, U: EventListener> {
    join: Option<JoinHandle<(EventLoop<T, U>, State)>>,
    tx: EventLoopSender,
}

impl<T: tty::EventedPty, U: EventListener> EventLoopHandle<T, U> {
    /// 正常关闭链：发 Shutdown、join 取回事件循环与终态。重复 Shutdown
    /// 无害（线程已退出时 send 失败忽略）；线程 panic 以 Err 上抛，不在
    /// 此处 unwrap——调用方可能在 unwind 路径（Drop）上，二次 panic 即
    /// abort。
    pub fn join(mut self) -> std::thread::Result<(EventLoop<T, U>, State)> {
        let join = self.join.take().expect("EventLoopHandle 二次 join");
        let _ = self.tx.send(Msg::Shutdown);
        join.join()
    }
}

impl<T: tty::EventedPty, U: EventListener> Drop for EventLoopHandle<T, U> {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            // 未显式 join 的兜底：唤醒退出、join 取回并即弃——EventLoop
            // (Pty) 在此析构，名额归还。线程 panic 时 unwind 已析构，Err 无害。
            let _ = self.tx.send(Msg::Shutdown);
            let _ = join.join();
        }
    }
}

/// Helper type which tracks how much of a buffer has been written.
struct Writing {
    source: Cow<'static, [u8]>,
    written: usize,
}

pub struct Notifier(pub EventLoopSender);

impl Notifier {
    /// 非阻塞发送 PTY 输入；空输入不产生命令。
    pub fn notify<B>(&self, bytes: B) -> Result<(), EventLoopSendError>
    where
        B: Into<Cow<'static, [u8]>>,
    {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Ok(());
        }
        self.0.try_send(Msg::Input(bytes))
    }

    /// 非阻塞提交完整 resize；调用方负责处理背压。
    pub fn on_resize(&mut self, window_size: WindowSize) -> Result<(), EventLoopSendError> {
        self.0.try_send(Msg::Resize(window_size))
    }

    /// Reflow the grid to `window_size` and leave the child on its old size.
    /// See [`Msg::ResizeGrid`] for why the two halves are split.
    pub fn on_resize_grid(&mut self, window_size: WindowSize) -> Result<(), EventLoopSendError> {
        self.0.try_send(Msg::ResizeGrid(window_size))
    }
}

#[derive(Debug)]
pub enum EventLoopSendError {
    /// Error polling the event loop.
    Io(io::Error),

    /// 非阻塞命令队列已满；调用方必须显式处理，不能假装已送达。
    QueueFull,

    /// worker 已断开，命令未送达。
    Disconnected,
}

impl Display for EventLoopSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EventLoopSendError::Io(err) => err.fmt(f),
            EventLoopSendError::QueueFull => f.write_str("terminal command queue is full"),
            EventLoopSendError::Disconnected => f.write_str("terminal worker is disconnected"),
        }
    }
}

impl std::error::Error for EventLoopSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventLoopSendError::Io(err) => err.source(),
            EventLoopSendError::QueueFull | EventLoopSendError::Disconnected => None,
        }
    }
}

#[derive(Clone)]
pub struct EventLoopSender {
    sender: SyncSender<Msg>,
    poller: Arc<Poller>,
}

impl EventLoopSender {
    /// 为非 PTY 传输创建消息通道，继续复用应用既有的输入、缩放和关闭协议。
    pub fn standalone() -> io::Result<(Self, Receiver<Msg>)> {
        let (sender, receiver) = mpsc::sync_channel(COMMAND_CHANNEL_CAPACITY);
        Ok((
            Self {
                sender,
                poller: Arc::new(Poller::new()?),
            },
            receiver,
        ))
    }

    /// 阻塞发送只供后台关闭路径使用；前台命令必须走 `try_send`。
    /// 空输入不是命令：无副作用地接受，不占用队列。
    pub fn send(&self, msg: Msg) -> Result<(), EventLoopSendError> {
        if is_empty_input(&msg) {
            return Ok(());
        }
        self.sender
            .send(msg)
            .map_err(|_| EventLoopSendError::Disconnected)?;
        self.poller.notify().map_err(EventLoopSendError::Io)
    }

    /// 非阻塞发送命令；满载和断连都明确返回，不丢弃后伪报成功。
    pub fn try_send(&self, msg: Msg) -> Result<(), EventLoopSendError> {
        if is_empty_input(&msg) {
            return Ok(());
        }
        self.sender.try_send(msg).map_err(|error| match error {
            TrySendError::Full(_) => EventLoopSendError::QueueFull,
            TrySendError::Disconnected(_) => EventLoopSendError::Disconnected,
        })?;
        self.poller.notify().map_err(EventLoopSendError::Io)
    }
}

/// 空输入写不出任何字节：不得进入写队列，否则 `write(&[])` 的 `Ok(0)`
/// 会被当作背压而永久卡住后续输入。
fn is_empty_input(msg: &Msg) -> bool {
    matches!(msg, Msg::Input(bytes) if bytes.is_empty())
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
    /// 显示命令已改变网格外观，下一边界必须发布新帧。
    display_dirty: bool,
    /// 当前 worker 内单调递增的不可变帧代次。
    frame_revision: u64,
    /// ConPTY 光标对账的死线：resize 提交后 `ALIGN_DELAY` 触发，见
    /// `conpty_cursor_probe`。
    align_at: Option<Instant>,
    /// 首次观察到输出传输关闭的时刻；`OUTPUT_CLOSED_GRACE` 内仍无子进程
    /// 退出事件则报告传输故障。
    output_closed_at: Option<Instant>,
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
        // 空项写不出任何字节：直接跳过，避免 `write(&[])` 的 `Ok(0)`
        // 被当作背压永久停在同一项。
        loop {
            match self.write_list.pop_front() {
                Some(item) if item.is_empty() => continue,
                Some(item) => {
                    self.writing = Some(Writing::new(item));
                    break;
                }
                None => {
                    self.writing = None;
                    break;
                }
            }
        }
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
mod tests;
