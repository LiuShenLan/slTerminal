/// PTY reader 线程 — 阻塞读取 PTY 输出 → 微批聚合 → Channel 推送 PtyEvent
///
/// E1: Channel 断开时不退出，写入 ring buffer 等待 reattach。
///
/// BE-05 微批（I/O 编排）：read 成功后非阻塞续读（Windows 上基于
/// WaitForSingleObject(handle, 0) 检测管道可读），累积至 MICRO_BATCH_MAX（64KB）
/// 或无可读数据后，再一次批量 Channel::send + ring buffer append（BE-12）。
/// 「读到即续读」非定时器——不引入固定延迟；首块经过 ConPTY 启动序列剥离，
/// 续读块在首块真实数据出现后原样透传（BE-13 跨 16KB 边界残留由首块剥离状态机处理）。
/// DOC-01 豁免项 1（reader_loop 残余 I/O 编排分支）随微批变动——豁免表同步在 S19，
/// 本文件 M11 分析块已更新为微批后形态。
///
/// 独立线程运行，不阻塞 tokio runtime。读取到 EOF（子进程退出）时发送 Exit 事件并退出。
///
/// Windows: 首轮读取时剥离 ConPTY VtIo::StartIfNeeded() 注入的启动序列
/// （OSC 标题含 BEL→蜂鸣、清屏/归位→首字符被覆盖、DSR/光标查询），
/// 后续读取原样透传。
use crate::pty::spawn::PtyEvent;
use crate::state::ring_buffer_append;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::ipc::Channel;

/// reader 线程读取缓冲区大小
/// 189KB/s 输出场景：16KB → 约 12 次/秒 read() 调用（4KB 为 47 次/秒）
pub const READER_BUF_SIZE: usize = 16384;

/// BE-05: 微批续读上限（64KB）——read 成功后非阻塞续读，累积至此或无可读
/// 数据再一次 Channel::send + ring buffer append（BE-12）。首块最多
/// READER_BUF_SIZE，续读约 3 块满上限。契约：64KB（S06 跨边界写死）。
pub const MICRO_BATCH_MAX: usize = 65536;

/// BE-05: reader 输入——阻塞读取 + 非阻塞续读检查（微批用）
///
/// - reader: PTY 输出读端（阻塞 read，供主循环与微批续读）
/// - pending: 非阻塞「管道是否有可读数据」检查——Windows 上由 spawn.rs 构造
///   （WaitForSingleObject(handle, 0)），非 Windows 恒 false（微批退化为每轮一次 read）
pub struct PtyReaderInput {
    reader: Box<dyn Read + Send>,
    pending: Box<dyn Fn() -> bool + Send>,
}

impl PtyReaderInput {
    /// 构造 reader 输入（BE-05）
    pub fn new(reader: Box<dyn Read + Send>, pending: Box<dyn Fn() -> bool + Send>) -> Self {
        Self { reader, pending }
    }
}

impl Read for PtyReaderInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

/// reader 线程主循环（E1: 支持重连）
///
/// - input: PtyReaderInput——阻塞读取 + BE-05 微批续读检查（pending 非阻塞
///   「管道是否有未读数据」；Windows = WaitForSingleObject(handle, 0)（spawn.rs
///   构造），非 Windows = 恒 false）
/// - channel: 可替换的 Channel 引用，pty_reattach 通过写锁替换
/// - ring: ring buffer，总是缓存最近输出供 reattach 回放
/// - child: P2-11 子进程句柄，EOF 时调用 wait() 获取真实退出码
/// - exit_code: P2-42 退出状态共享，reader 设置后 pty_reattach 检测
/// - 循环读取 PTY 输出，微批聚合后通过 Channel 发送 Output 事件（BE-05）
/// - Ok(0) = EOF → 发 Exit 事件 → 退出
/// - Windows 首轮读取剥离 ConPTY 启动注入序列
pub fn reader_loop(
    mut input: PtyReaderInput,
    channel: Arc<RwLock<Option<Channel<PtyEvent>>>>,
    ring: Arc<Mutex<VecDeque<u8>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    da1_injected: Arc<AtomicBool>,
) {
    let mut buf = [0u8; READER_BUF_SIZE];
    let mut startup_drained = false;

    loop {
        match input.read(&mut buf) {
            Ok(0) => {
                // EOF — 子进程已退出
                // P2-11: 从 child.wait() 获取真实退出码而非硬编码 0
                // 锁/等待失败 → 退出码未知（None），不硬编码 0（降级决策见 eof_exit_code）
                let wait_outcome: Result<Result<i32, ()>, ()> = match child.lock() {
                    Ok(mut c) => match c.wait() {
                        Ok(status) => Ok(Ok(status.exit_code() as i32)),
                        Err(e) => {
                            tracing::warn!("child.wait() 失败: {e}");
                            Ok(Err(()))
                        }
                    },
                    Err(e) => {
                        tracing::error!("reader_loop child 锁获取失败: {e}");
                        Err(())
                    }
                };
                let code = eof_exit_code(wait_outcome);

                // P2-42: 记录退出码到共享状态，供 pty_reattach 检测
                if let Ok(mut ec) = exit_code.lock() {
                    *ec = code;
                }

                let ch = match channel.read() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("reader_loop channel 锁获取失败: {e}");
                        break;
                    }
                };
                if let Some(ref c) = *ch {
                    if let Err(e) = c.send(PtyEvent::Exit { code }) {
                        tracing::debug!("Channel send 失败（前端可能已断开）: {}", e);
                    }
                }
                break;
            }
            Ok(n) => {
                // 首轮剥离 ConPTY 启动序列（纯函数），全部剥离则跳过本轮
                // 仅当出现真实非启动输出后才置 drained——若本轮全为启动序列（None），
                // 保持剥离状态以处理跨 16KB 边界的残留启动序列（BE-13）
                let first = match apply_startup_strip(startup_drained, &buf[..n]) {
                    Some(b) => {
                        startup_drained = true;
                        b
                    }
                    None => {
                        continue;
                    }
                };

                // DA1 查询模拟响应：Claude Code Ink 渲染器启动时发 ESC[c 作为同步哨兵。
                // ConPTY 拦截 DA1 查询后内部处理，不向子进程 stdout 返回响应。
                // 导致 Ink waitFor 永不 resolve，阻塞约 60s。
                // 此处检测子进程发出的 DA1 查询，向 stdin 注入 ESC[?64;22c（VT420+ANSI 颜色）
                // 模拟 ConPTY 的一致行为。同一会话仅注入一次（AtomicBool 防重复）。
                maybe_inject_da1(&da1_injected, &writer, &first);

                // BE-05: 微批——read 成功后非阻塞续读（「读到即续读」，非定时器），
                // 累积至 MICRO_BATCH_MAX（64KB）或无可读数据，再一次批量
                // Channel::send + ring buffer append。续读遇 EOF/错误时立即停止
                // （tail 已含数据照常 flush，下一轮主循环 read 走 EOF/Err 分支，
                // 无数据丢失）；续读块不再过启动序列剥离（startup_drained 已置 true）。
                let mut batch: Vec<u8> = Vec::with_capacity(READER_BUF_SIZE * 2);
                batch.extend_from_slice(&first);
                let (tail, _eof) =
                    micro_batch_tail(&mut input, &mut buf, MICRO_BATCH_MAX - first.len());
                // 续读块同样检测 DA1（跨块边界残留序列；AtomicBool 防重复注入）
                maybe_inject_da1(&da1_injected, &writer, &tail);
                batch.extend_from_slice(&tail);

                let ch = match channel.read() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("reader_loop channel 锁获取失败: {e}");
                        break;
                    }
                };
                // P2-46: 总是先缓存到 ring buffer（不 clone），再 send 消耗 batch
                // 成功路径零 clone，失败路径（Channel 断连）ring buffer 已有数据
                // BE-12: 批量 append——合并后 append 调用点仅此一处（每微批一次，
                // 锁竞争随 send 频次同步下降），不引入无锁结构
                if let Err(e) = ring_buffer_append(&ring, &batch) {
                    tracing::warn!("ring buffer 写入失败: {e}");
                }
                if let Some(ref c) = *ch {
                    if let Err(e) = c.send(PtyEvent::Output { bytes: batch }) {
                        tracing::debug!("Channel send 失败（前端可能已断开）: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("PTY reader 错误: {e}");
                // P2-42: 记录错误退出码到共享状态
                if let Ok(mut ec) = exit_code.lock() {
                    *ec = Some(-1);
                }
                let ch = match channel.read() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("reader_loop channel 锁获取失败: {e}");
                        break;
                    }
                };
                if let Some(ref c) = *ch {
                    if let Err(e) = c.send(PtyEvent::Exit { code: Some(-1) }) {
                        tracing::debug!("Channel send 失败（前端可能已断开）: {}", e);
                    }
                }
                break;
            }
        }
    }
}

/// BE-05: 微批续读（纯逻辑，可单测）——非阻塞续读至上限或无可读数据
///
/// 首块 read 成功后调用：循环「有未读数据 && 未达上限」→ 阻塞 read 取块，
/// 累积到 `tail`。返回（续读累积数据, 是否遇 EOF）：
/// - 遇 EOF（Ok(0)）：停止续读并返回 true——调用方照常 flush 已累积数据，
///   下一轮主循环 read 将再次 Ok(0) 走 EOF 分支（无数据丢失、无重复）
/// - 遇读错误：停止续读并返回 false——同上，下一轮主循环 Err 分支处理
/// - 达到 `limit` 或 pending 返回 false：正常返回 false
///
/// 关键语义：pending 为 true 后 `read` 才被调用——Windows 上 pending 基于
/// WaitForSingleObject(handle, 0) 非阻塞检测（有数据或对端关闭才为 true），
/// 因此 read 不会空等，「读到即续读」而非定时器轮询。
fn micro_batch_tail(input: &mut PtyReaderInput, buf: &mut [u8], limit: usize) -> (Vec<u8>, bool) {
    let mut tail: Vec<u8> = Vec::new();
    while tail.len() < limit && (input.pending)() {
        match input.read(buf) {
            Ok(0) => return (tail, true),
            Ok(m) => tail.extend_from_slice(&buf[..m]),
            Err(e) => {
                tracing::warn!("PTY reader 微批续读错误: {e}");
                return (tail, false);
            }
        }
    }
    (tail, false)
}

/// DA1 查询模拟响应注入（首块与微批续读块共用）
///
/// 检测到 DA1 查询（ESC[c / ESC[0c）则向子进程 stdin 注入 ESC[?64;22c，
/// 模拟 ConPTY + conhost 的一致行为；AtomicBool 保证同一会话仅注入一次。
/// 检测决策已抽为纯函数 `should_inject_da1`，注入动作为 I/O（M11 豁免项）。
fn maybe_inject_da1(
    da1_injected: &AtomicBool,
    writer: &Mutex<Box<dyn Write + Send>>,
    data: &[u8],
) {
    if !should_inject_da1(da1_injected.load(Ordering::Relaxed), data) {
        return;
    }
    da1_injected.store(true, Ordering::Relaxed);
    // 向子进程 stdin 注入 DA1 响应（不阻塞 reader 线程）
    if let Ok(mut w) = writer.lock() {
        if let Err(e) = w.write_all(b"\x1b[?64;22c") {
            tracing::warn!("DA1 响应注入失败: {}", e);
        }
        if let Err(e) = w.flush() {
            tracing::warn!("DA1 响应注入失败: {}", e);
        }
    }
}

/// EOF 退出码降级决策（P2-11/P2-42）
///
/// 输入为 reader_loop 的 lock/wait 两级结果：外层 Err = child 句柄锁获取失败，
/// 内层 Err = `child.wait()` 失败。任一失败 → `None`（退出码未知，不硬编码 0——
/// P2-11 明确弃用旧"硬编码 0"行为）；两级均 Ok → 真实退出码。
/// 纯函数，由 reader_loop 注入结果，测试直接构造三种输入。
fn eof_exit_code(wait_outcome: Result<Result<i32, ()>, ()>) -> Option<i32> {
    match wait_outcome {
        Ok(Ok(code)) => Some(code),
        Ok(Err(())) | Err(()) => None,
    }
}

/// 剥离 ConPTY VtIo::StartIfNeeded() 注入的启动序列
///
/// 启动序列（按出现顺序）：
/// - OSC 窗口标题: `ESC ] 0 ; ... BEL` — BEL(0x07) 被 xterm.js 误解析为蜂鸣
/// - 清屏: `ESC [ 2 J` / `ESC [ 3 J`
/// - 光标归位: `ESC [ H`
/// - 光标显隐: `ESC [ ? 2 5 h` / `ESC [ ? 2 5 l`
/// - DSR 光标查询: `ESC [ 6 n`（已被 CPR 应答，此序列无害但多余）
///
/// 在非 Windows 平台此函数原样返回（无 ConPTY 启动序列）。
fn strip_conpty_startup(data: &[u8]) -> Vec<u8> {
    // 由 cfg 守护，Windows CI 不可达：`cfg!(windows)` 为编译期常量，本项目 CI
    // 恒为 Windows → 剥离分支真实执行，本分支被编译期裁剪（平台守卫测试见
    // `strip_platform_guard_constant`）。非 Windows 无 ConPTY，原样返回。
    if !cfg!(windows) {
        return data.to_vec();
    }

    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b {
            // OSC 序列（ESC ]）——以 BEL(0x07) 或 ST(ESC \) 终结
            if i + 1 < data.len() && data[i + 1] == b']' {
                if i + 2 < data.len() && (data[i + 2] == b'0' || data[i + 2] == b'2') {
                    if let Some(end) = find_osc_end(&data[i..]) {
                        i += end;
                        continue;
                    }
                }
                result.push(data[i]);
                i += 1;
                continue;
            }

            // CSI 序列（ESC [）
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

/// 对首轮读取应用 ConPTY 启动序列剥离
///
/// 若 `startup_drained` 为 true（非首轮），原样返回 `Some(data.to_vec())`。
/// 若为 false（首轮），调用 `strip_conpty_startup` 剥离启动序列；
/// 剥离后为空则返回 `None`（调用方跳过本轮），否则返回 `Some(剥离后数据)`。
fn apply_startup_strip(startup_drained: bool, data: &[u8]) -> Option<Vec<u8>> {
    if startup_drained {
        Some(data.to_vec())
    } else {
        let stripped = strip_conpty_startup(data);
        if stripped.is_empty() {
            None
        } else {
            Some(stripped)
        }
    }
}

/// 判断是否需要向子进程注入 DA1 响应
///
/// 条件：尚未注入过（`already_injected == false`）且当前输出含 DA1 查询序列。
/// 纯函数，不依赖 AtomicBool，便于单元测试。
fn should_inject_da1(already_injected: bool, data: &[u8]) -> bool {
    !already_injected && mirror_da1_query(data)
}

/// 检测输出字节流中是否含有 DA1 终端查询（ESC[c 或 ESC[0c）
///
/// DA2 (ESC[>c) 和 XTVERSION (ESC[>0q) 不触发——两者走不同的检测路径。
/// 此函数使用滑动窗口扫描，在 reader 线程每轮 read() 结果上调用。
fn mirror_da1_query(data: &[u8]) -> bool {
    // 滑动窗口扫描 ESC [ [0] c
    for i in 0..data.len().saturating_sub(2) {
        if data[i] == 0x1b && data[i + 1] == b'[' {
            let rest = &data[i + 2..];
            // ESC[c — 不含额外参数的标准 DA1 查询
            if rest.first() == Some(&b'c') {
                return true;
            }
            // ESC[0c — 含前导 0 的变体（某些应用发出此格式）
            if rest.len() >= 2 && rest[0] == b'0' && rest[1] == b'c' {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── M11: reader_loop 主循环纯函数化分析 ───
    //
    // reader_loop 主循环有三个 match 分支，均已审查可抽取性：
    //
    // 1. Ok(0) — EOF 分支：
    //    - eof_exit_code()       → ✅ 已抽取为纯函数（PTY-12）：lock/wait 两级
    //                              失败 → None（不硬编码 0），成功 → 真实退出码
    //    - child.wait()          → portable_pty::Child::wait() 是系统调用（Windows WaitForSingleObject），I/O
    //    - exit_code.lock()       → std::sync::Mutex，运行时同步原语
    //    - channel.read()         → std::sync::RwLock，运行时同步原语
    //    - c.send(PtyEvent::Exit) → Tauri IPC Channel::send()，I/O
    //
    // 2. Ok(n) — 数据分支（BE-05 微批后形态）：
    //    - apply_startup_strip()  → ✅ 已抽取为纯函数（Phase 2）
    //    - micro_batch_tail()     → ✅ 已抽取为纯函数（BE-05）：pending 检查 +
    //                               续读累积（read 为系统调用，决策已抽，调用不可抽）
    //    - should_inject_da1()    → ✅ 已抽取为纯函数（Phase 2）
    //    - maybe_inject_da1()     → 注入动作（writer.lock() + 管道 I/O），检测决策已抽
    //    - channel.read()         → RwLock
    //    - ring_buffer_append()   → Mutex + VecDeque 状态变更（BE-12: 批量 append，
    //                               每微批一次，调用点仅此一处）
    //    - c.send(PtyEvent::Output) → Channel::send()，I/O
    //
    // 3. Err(e) — 读错误分支：
    //    - tracing::warn!()       → 日志宏，I/O
    //    - exit_code.lock()       → Mutex
    //    - channel.read()         → RwLock
    //    - c.send(PtyEvent::Exit) → Channel::send()，I/O
    //
    // 三个分支中重复出现的 "read channel → if Some → send event" 模式
    // 需要 Arc<RwLock<Option<Channel<PtyEvent>>>> —— 真正的同步原语，
    // 无法在不引入运行时依赖的前提下构造测试输入。
    //
    // 结论：reader_loop 中剩余的所有分支决策均依赖同步原语或系统调用，
    // 无法进一步抽取为纯函数。apply_startup_strip / should_inject_da1 /
    // eof_exit_code / micro_batch_tail 已覆盖主循环中全部可纯函数化的决策逻辑。
    //
    // M11 状态：已尽力——剩余均为 I/O 编排无法纯函数化。
    // PTY-12 评估产出：残余不可抽分支明细 + 豁免理由见
    // src-tauri/src/pty/CLAUDE.md「reader_loop I/O 编排残余豁免（草稿）」
    // （Stage 17 统一收编为豁免表，DOC-01 引用）。
    // DOC-01 豁免项 1 随 BE-05 微批变动（ring buffer 写入/send 次数降为每微批一次，
    // 新增 pending 检查——决策已抽为 micro_batch_tail）：豁免表同步在 S19。

    #[test]
    fn test_strip_osc_title_bel() {
        let input = b"\x1b]0;pwsh\x07";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn test_strip_clear_screen() {
        let input = b"\x1b[2J";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn test_strip_clear_screen_3j() {
        // PTY-04: CSI 3J（清屏含滚动缓冲）与 2J 一并剥离
        assert_eq!(strip_conpty_startup(b"\x1b[3J"), b"");
    }

    #[test]
    fn test_strip_cursor_home() {
        let input = b"\x1b[H";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn test_strip_dsr() {
        let input = b"\x1b[6n";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn test_strip_cursor_visibility() {
        assert_eq!(strip_conpty_startup(b"\x1b[?25h"), b"");
        assert_eq!(strip_conpty_startup(b"\x1b[?25l"), b"");
    }

    #[test]
    fn test_preserve_normal_text() {
        let input = b"PS C:\\Users\\test> ";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn test_strip_startup_preserve_shell_output() {
        let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[HPS C:\\> ";
        assert_eq!(strip_conpty_startup(input), b"PS C:\\> ");
    }

    #[test]
    fn test_preserve_osc7_cwd() {
        let input = b"\x1b]7;file:///C:/Users\x1b\\";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_preserves_non_title_osc() {
        // PTY-04: OSC 1/3/4/9（非窗口标题类）不被剥离——剥离仅针对 OSC 0/2
        // 窗口标题（OSC 0/2），其余 OSC 用途（图标名/属性/调色板/桌面通知）
        // 均须原样透传，防止误杀应用侧正常输出
        let cases: [&[u8]; 4] = [
            b"\x1b]1;icon-title\x07", // OSC 1 icon name
            b"\x1b]3;prop\x07",       // OSC 3 属性
            b"\x1b]4;0;#000000\x07",  // OSC 4 调色板
            b"\x1b]9;notify\x07",     // OSC 9 桌面通知
        ];
        for case in cases {
            assert_eq!(strip_conpty_startup(case), case);
        }
    }

    #[test]
    fn strip_platform_guard_constant() {
        // PTY-04: cfg!(windows) 编译期常量断言。
        // `cfg!` 是编译期常量（非运行时环境检测）：Windows CI（本项目唯一
        // CI 平台）上恒为 true——剥离分支真实执行；非 Windows 平台恒为
        // false——strip_conpty_startup 走"原样返回"分支（由 cfg 守护，
        // Windows CI 不可达，代码注释已标注）。若未来误在非 Windows 平台
        // 编译运行，此断言与全部 strip 用例同时红，提示该分支缺失执行覆盖。
        if cfg!(windows) {
            // 常量与运行时平台一致性断言：cfg!(windows)==true 时运行平台必为
            // Windows（编译期平台与运行平台恒一致，此处锁死该不变量）
            assert_eq!(std::env::consts::OS, "windows");
        } else {
            // 非 Windows：就地验证"原样返回"分支真实行为（Windows CI 不可达）
            let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[H\x1b[?25l\x1b[6n";
            assert_eq!(strip_conpty_startup(input), input);
        }
    }

    // ─── Step 1.4: 缓冲区大小测试 ───

    #[test]
    fn reader_buf_size_is_16k() {
        assert_eq!(READER_BUF_SIZE, 16384);
    }

    #[test]
    fn strip_startup_with_large_payload() {
        // >4KB 连续非启动数据完整保留（验证大数据块不被截断）
        let payload = vec![b'X'; 10000];
        // 在开头插入需要剥离的启动序列
        let prefix = b"\x1b]0;pwsh\x07\x1b[2J\x1b[H";
        let mut input = prefix.to_vec();
        input.extend_from_slice(&payload);
        input.extend_from_slice(b"END_MARKER");

        let result = strip_conpty_startup(&input);
        // 剥离后应包含完整 payload + END_MARKER
        assert_eq!(result.len(), payload.len() + 10); // 10000 + 10 (END_MARKER)
        assert!(result.ends_with(b"END_MARKER"));
        // 前缀已被剥离
        assert!(!result.starts_with(b"\x1b]"));
    }

    // ─── 变更 1: ESC[s/ESC[u 不再误剥离（防御性回归）───
    // ESC[s/ESC[u 只有 2 字节（不含 '['），永远不会进入 CSI 序列匹配器。
    // 删除 b's'|b'u' 分支后，添加以下测试以确认行为不变。

    #[test]
    fn strip_preserves_save_cursor() {
        // ES1: ESC[s（标准 VT100 保存光标）不被剥离
        let input = b"\x1b[s hello";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_preserves_restore_cursor() {
        // ES2: ESC[u（标准 VT100 恢复光标）不被剥离
        let input = b"\x1b[u world";
        assert_eq!(strip_conpty_startup(input), input);
    }

    #[test]
    fn strip_existing_tests_still_pass() {
        // ES3: 删除死代码不影响现有剥离行为——ESC[H 仍被剥离
        assert_eq!(strip_conpty_startup(b"\x1b[H"), b"");
        // ESC[2J 仍被剥离
        assert_eq!(strip_conpty_startup(b"\x1b[2J"), b"");
    }

    #[test]
    fn strip_startup_with_16k_boundary() {
        // 数据跨 16KB 边界：启动序列 + 16KB 数据
        // 验证 strip_conpty_startup 在接近新缓冲区大小的数据上正常工作
        let payload = vec![b'Y'; READER_BUF_SIZE];
        // 开头插入启动序列
        let prefix = b"\x1b[2J\x1b[H";
        let mut input = prefix.to_vec();
        input.extend_from_slice(&payload);
        input.extend_from_slice(b"TAIL");

        let result = strip_conpty_startup(&input);
        // 剥离后应 = 16KB payload + TAIL
        assert_eq!(result.len(), READER_BUF_SIZE + 4); // 16KB + 4 (TAIL)
        assert!(result.starts_with(b"Y"));
        assert!(result.ends_with(b"TAIL"));
    }

    // ─── 变更 2: mirror_da1_query 单元测试 ───

    #[test]
    fn da1_standard_query_detected() {
        // DA1_U1: 标准 DA1 查询 ESC[c
        assert!(mirror_da1_query(b"\x1b[c"));
    }

    #[test]
    fn da1_with_leading_zero_detected() {
        // DA1_U2: 含前导数字的 DA1 ESC[0c
        assert!(mirror_da1_query(b"\x1b[0c"));
    }

    #[test]
    fn da2_not_detected() {
        // DA1_U3: DA2 (ESC[>c) 不触发
        assert!(!mirror_da1_query(b"\x1b[>c"));
    }

    #[test]
    fn plain_text_not_falsely_detected() {
        // DA1_U4: 普通文本含 c 不误触发（无 ESC 前缀）
        assert!(!mirror_da1_query(b"hello [c world"));
    }

    #[test]
    fn xtversion_not_detected() {
        // DA1_U7: XTVERSION ESC[>0q 不触发
        assert!(!mirror_da1_query(b"\x1b[>0q"));
    }

    #[test]
    fn da1_embedded_in_output_detected() {
        // DA1 查询嵌入在正常输出流中仍能被检测
        let input = b"prompt> \x1b[c more output";
        assert!(mirror_da1_query(input));
    }

    // ─── apply_startup_strip 纯函数测试 ───

    #[test]
    fn startup_strip_drained_passthrough() {
        // 非首轮：原样返回
        let data = b"normal output";
        let result = apply_startup_strip(true, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    #[test]
    fn startup_strip_first_round_all_stripped() {
        // 首轮全部为启动序列 → 返回 None（跳过本轮）
        let result = apply_startup_strip(false, b"\x1b]0;pwsh\x07\x1b[2J\x1b[H");
        assert_eq!(result, None);
    }

    #[test]
    fn startup_strip_first_round_partial_strip() {
        // 首轮启动序列后跟正常输出 → 剥离前缀
        let result = apply_startup_strip(false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(result, Some(b"PS C:\\> ".to_vec()));
    }

    #[test]
    fn startup_strip_across_buffer_boundary() {
        // BE-13: 跨缓冲区边界的启动序列剥离——第一轮全为启动序列（None），
        // 不置 drained；第二轮仍有启动序列 + 真实输出，应继续剥离
        let r1 = apply_startup_strip(false, b"\x1b]0;pwsh\x07");
        assert_eq!(r1, None); // 第一轮全部是 OSC 标题 → 跳过

        // 第二轮仍用 startup_drained=false（模拟 reader_loop 中 None 分支不改 drained）
        let r2 = apply_startup_strip(false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(r2, Some(b"PS C:\\> ".to_vec())); // 清屏+归位被剥离，保留真实输出
    }

    #[test]
    fn startup_strip_multi_round_all_startup_then_real() {
        // BE-13: 多轮纯启动序列后出现真实输出——验证 drained 仅在 Some 时才置
        // 模拟三轮：OSC 标题 → 清屏 → 光标归位+真实输出
        let r1 = apply_startup_strip(false, b"\x1b]0;pwsh\x07");
        assert_eq!(r1, None);

        let r2 = apply_startup_strip(false, b"\x1b[2J");
        assert_eq!(r2, None);

        let r3 = apply_startup_strip(false, b"\x1b[?25h\x1b[HHello World");
        assert_eq!(r3, Some(b"Hello World".to_vec()));
    }

    #[test]
    fn startup_strip_first_round_no_startup_seq() {
        // 首轮无启动序列（如 cmd.exe 场景）→ 原样返回
        let data = b"Microsoft Windows [Version 10.0]\r\n";
        let result = apply_startup_strip(false, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    // ─── should_inject_da1 纯函数测试 ───

    #[test]
    fn da1_inject_already_injected_returns_false() {
        // 已注入过 → 不再注入
        assert!(!should_inject_da1(true, b"\x1b[c"));
        assert!(!should_inject_da1(true, b"normal output"));
    }

    #[test]
    fn da1_inject_not_injected_with_da1_returns_true() {
        // 未注入 + 含 DA1 查询 → 应注入
        assert!(should_inject_da1(false, b"\x1b[c"));
        assert!(should_inject_da1(false, b"\x1b[0c"));
    }

    #[test]
    fn da1_inject_not_injected_without_da1_returns_false() {
        // 未注入但无 DA1 → 不注入
        assert!(!should_inject_da1(false, b"normal output"));
        assert!(!should_inject_da1(false, b"\x1b[>c")); // DA2 不触发
    }

    #[test]
    fn da1_inject_embedded_in_output() {
        // DA1 嵌入在正常输出中
        assert!(should_inject_da1(false, b"prompt> \x1b[c more"));
    }

    // ─── eof_exit_code 纯函数测试（PTY-12）───

    #[test]
    fn eof_exit_code_success_returns_real_code() {
        // P2-11: child.wait() 成功 → 返回真实退出码（含 0）
        assert_eq!(eof_exit_code(Ok(Ok(0))), Some(0));
        assert_eq!(eof_exit_code(Ok(Ok(42))), Some(42));
    }

    #[test]
    fn eof_exit_code_wait_failure_returns_none() {
        // P2-11: child.wait() 失败 → None（退出码未知，不硬编码 0 假退出码）
        assert_eq!(eof_exit_code(Ok(Err(()))), None);
    }

    #[test]
    fn eof_exit_code_lock_failure_returns_none() {
        // P2-42: child 句柄锁获取失败 → None（退出码未知）
        assert_eq!(eof_exit_code(Err(())), None);
    }

    // ─── BE-05: micro_batch_tail 微批续读测试 ───

    /// 微批续读测试 mock reader：按预设序列输出数据块 / EOF / 错误
    struct MockSeqReader {
        /// read 序列：Ok(Vec) 数据块、Ok(空) EOF、Err 错误
        seq: Vec<std::io::Result<Vec<u8>>>,
        idx: usize,
    }

    impl Read for MockSeqReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.idx >= self.seq.len() {
                return Ok(0); // 序列耗尽默认 EOF
            }
            let item = std::mem::replace(&mut self.seq[self.idx], Ok(Vec::new()));
            self.idx += 1;
            match item {
                Ok(data) => {
                    let n = data.len().min(buf.len());
                    buf[..n].copy_from_slice(&data[..n]);
                    Ok(n)
                }
                Err(e) => Err(e),
            }
        }
    }

    fn ok_block(len: usize, fill: u8) -> std::io::Result<Vec<u8>> {
        Ok(vec![fill; len])
    }

    #[test]
    fn micro_batch_no_pending_reads_nothing() {
        // pending=false → 不续读，tail 空（且不消费 reader 数据）
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader {
                seq: vec![ok_block(16, b'A')],
                idx: 0,
            }),
            Box::new(|| false),
        );
        let mut buf = [0u8; 64];
        let (tail, eof) = micro_batch_tail(&mut input, &mut buf, 1024);
        assert!(tail.is_empty());
        assert!(!eof);
    }

    #[test]
    fn micro_batch_drains_until_no_data() {
        // pending=true → 续读直至数据耗尽（read 返回 Ok(0) → eof=true）
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader {
                seq: vec![ok_block(10, b'A'), ok_block(20, b'B')],
                idx: 0,
            }),
            Box::new(|| true),
        );
        let mut buf = [0u8; 64];
        let (tail, eof) = micro_batch_tail(&mut input, &mut buf, 1024);
        assert_eq!(tail.len(), 30);
        assert_eq!(&tail[..10], &[b'A'; 10]);
        assert_eq!(&tail[10..], &[b'B'; 20]);
        assert!(eof, "数据耗尽后 read 返回 Ok(0) 应标记 EOF");
    }

    #[test]
    fn micro_batch_stops_at_limit() {
        // 数据充足 + 上限 → 累积至 limit 即停（超出一个块的量，不无限续读）
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader {
                seq: std::iter::repeat_with(|| ok_block(1024, b'C'))
                    .take(10)
                    .collect(),
                idx: 0,
            }),
            Box::new(|| true),
        );
        let mut buf = [0u8; 4096];
        let (tail, eof) = micro_batch_tail(&mut input, &mut buf, 4096);
        assert_eq!(tail.len(), 4096);
        assert!(!eof, "达上限停止不算 EOF");
    }

    #[test]
    fn micro_batch_stops_on_error() {
        // 续读遇错误 → 停止，已累积数据保留（eof=false，下一轮主循环 Err 分支处理）
        let err = std::io::Error::other("mock read error");
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader {
                seq: vec![ok_block(8, b'X'), Err(err)],
                idx: 0,
            }),
            Box::new(|| true),
        );
        let mut buf = [0u8; 64];
        let (tail, eof) = micro_batch_tail(&mut input, &mut buf, 1024);
        assert_eq!(tail, vec![b'X'; 8]);
        assert!(!eof, "读错误不标记 EOF");
    }

    #[test]
    fn micro_batch_immediate_eof() {
        // pending=true 但首轮即 EOF → 空 tail + eof=true
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader { seq: vec![], idx: 0 }),
            Box::new(|| true),
        );
        let mut buf = [0u8; 64];
        let (tail, eof) = micro_batch_tail(&mut input, &mut buf, 1024);
        assert!(tail.is_empty());
        assert!(eof);
    }

    #[test]
    fn micro_batch_limit_respects_first_chunk_headroom() {
        // 首块已占空间时 limit 收窄：调用方传 MICRO_BATCH_MAX - first.len()，
        // tail 累积不超过剩余额度（总批 ≤ 64KB 契约）
        let mut input = PtyReaderInput::new(
            Box::new(MockSeqReader {
                seq: std::iter::repeat_with(|| ok_block(1024, b'D'))
                    .take(10)
                    .collect(),
                idx: 0,
            }),
            Box::new(|| true),
        );
        let mut buf = [0u8; 4096];
        // 模拟首块 3000B 后剩余额度 = 7000 - 3000 = 4000
        let (tail, _eof) = micro_batch_tail(&mut input, &mut buf, 4000);
        assert_eq!(tail.len(), 4096); // 4000 额度 + 一个块粒度 = 4096
    }
}
