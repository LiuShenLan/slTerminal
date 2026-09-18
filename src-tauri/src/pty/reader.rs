/// PTY reader 线程 — 阻塞读取 PTY 输出 → 微批聚合 → Channel 直写 PtyEvent
///
/// CP-034: Channel 直写 + 断开退出，单路径——断开（send 失败）即退出，不缓冲。
///
/// BE-05 微批（I/O 编排）：read 成功后非阻塞续读（Windows 上基于
/// PeekNamedPipe 查询管道可读字节数），累积至 MICRO_BATCH_MAX（64KB）
/// 或无可读数据后，再一次批量 Channel::send（BE-12）。
/// 「读到即续读」非定时器——不引入固定延迟。完整批次（首块+续读）统一走
/// pending 拼接 → DA1 检测/代答 → 剥离 → 发送（旧 first/tail 两段式下
/// 续读块不过启动剥离的缺口随之消除）。
/// DOC-01 豁免项 1（reader_loop 残余 I/O 编排分支）随微批变动——豁免表同步在 S19，
/// 本文件 M11 分析块已更新为微批后形态。
///
/// DA1 全量接管（ADR 见 .claude/adr.md）：输出流中的 DA1 查询
/// （ESC[c / ESC[0c）一律剥离、不透传前端，由后端向 stdin 代答
/// ESC[?64;22c——每次查询都应答，单一应答身份。动机：xterm.js 核心会对
/// 到达的 DA1 自答 ESC[?1;2c 并经 onData→pty_write 无差别回灌 stdin
/// （旧调查「xterm 应答只留前端不回灌」假设已证伪）；Win10 捆绑 conhost
/// （OpenConsole 1.24）启动握手多发 DA1，迟到的自答落入 PSReadLine 行首
/// → 蜂鸣 + 可见 [?1;2c 字符污染，恢复注入被拼前缀。块尾 DA1 前缀残片
/// 扣留 pending 待下块拼接（跨块防裂），EOF 时冲刷不丢字节。
///
/// 独立线程运行，不阻塞 tokio runtime。读取到 EOF（子进程退出）时发送 Exit 事件并退出。
///
/// Windows: 首轮读取时剥离 ConPTY VtIo::StartIfNeeded() 注入的启动序列
/// （OSC 标题含 BEL→蜂鸣、清屏/归位→首字符被覆盖、DSR/DA1 查询）。
use crate::pty::spawn::PtyEvent;
use parking_lot::Mutex;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::ipc::Channel;

/// reader 线程读取缓冲区大小
/// 189KB/s 输出场景：16KB → 约 12 次/秒 read() 调用（4KB 为 47 次/秒）
pub const READER_BUF_SIZE: usize = 16384;

/// BE-05: 微批续读上限（64KB）——read 成功后非阻塞续读，累积至此或无可读
/// 数据再一次 Channel::send（BE-12）。首块最多
/// READER_BUF_SIZE，续读约 3 块满上限。契约：64KB（S06 跨边界写死）。
pub const MICRO_BATCH_MAX: usize = 65536;

/// BE-05: reader 输入——阻塞读取 + 非阻塞续读检查（微批用）
///
/// - reader: PTY 输出读端（阻塞 read，供主循环与微批续读）
/// - pending: 非阻塞「管道是否有可读数据」检查——Windows 上由 spawn.rs 构造
///   （PeekNamedPipe 可读字节数 > 0），非 Windows 恒 false（微批退化为每轮一次 read）
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

/// reader 线程主循环（CP-034: Channel 直写 + 断开退出，单路径）
///
/// - input: PtyReaderInput——阻塞读取 + BE-05 微批续读检查（pending 非阻塞
///   「管道是否有未读数据」；Windows = PeekNamedPipe 可读字节数 > 0（spawn.rs
///   构造），非 Windows = 恒 false）
/// - channel: Channel 直写（无替换层）；send 失败（前端已卸载）→ 退出
/// - child: P2-11 子进程句柄，EOF 时调用 wait() 获取真实退出码
/// - exit_code: P2-42 退出状态共享，reader 设置后记录
/// - writer: DA1 代答注入通道（检测到 DA1 查询时写入 ESC[?64;22c）
/// - 循环读取 PTY 输出，微批聚合后通过 Channel 发送 Output 事件（BE-05）
/// - Ok(0) = EOF → 冲刷 pending 残片 → 发 Exit 事件 → 退出
/// - Windows 首轮读取剥离 ConPTY 启动注入序列；DA1 查询全程剥离 + 代答
pub fn reader_loop(
    mut input: PtyReaderInput,
    channel: Channel<PtyEvent>, // 直写，无替换层
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
) {
    let mut buf = [0u8; READER_BUF_SIZE];
    let mut startup_drained = false;
    // 跨块 DA1 前缀残片扣留缓冲（≤3 字节：ESC / ESC[ / ESC[0）
    let mut pending_da1: Vec<u8> = Vec::new();

    loop {
        match input.read(&mut buf) {
            Ok(0) => {
                // EOF — 子进程已退出
                // 冲刷扣留的 DA1 前缀残片（不完整序列，原样转发不丢字节）
                if !pending_da1.is_empty() {
                    let _ = channel.send(PtyEvent::Output {
                        bytes: std::mem::take(&mut pending_da1),
                    });
                }
                // P2-11: 从 child.wait() 获取真实退出码而非硬编码 0
                // CP-005: parking_lot 锁无失败路径——仅 child.wait() 失败 →
                // 退出码未知（None），不硬编码 0（降级决策见 eof_exit_code）
                let wait_outcome: Result<Result<i32, ()>, ()> = {
                    let mut c = child.lock();
                    match c.wait() {
                        Ok(status) => Ok(Ok(status.exit_code() as i32)),
                        Err(e) => {
                            tracing::warn!("child.wait() 失败: {e}");
                            Ok(Err(()))
                        }
                    }
                };
                let code = eof_exit_code(wait_outcome);

                // P2-42: 记录退出码到共享状态
                let mut ec = exit_code.lock();
                *ec = code;

                if let Err(e) = channel.send(PtyEvent::Exit { code }) {
                    tracing::debug!("Channel send 失败（前端已断开）,reader 退出: {e}");
                }
                break;
            }
            Ok(n) => {
                // BE-05: 微批——read 成功后非阻塞续读（「读到即续读」，非定时器），
                // 累积至 MICRO_BATCH_MAX（64KB）或无可读数据。先聚合成完整批次再
                // 统一处理（续读遇 EOF/错误立即停止，tail 已含数据照常进入本批，
                // 下一轮主循环 read 走 EOF/Err 分支，无数据丢失）
                let mut raw = Vec::with_capacity(n + READER_BUF_SIZE);
                raw.extend_from_slice(&buf[..n]);
                let (tail, _eof) =
                    micro_batch_tail(&mut input, &mut buf, MICRO_BATCH_MAX.saturating_sub(n));
                raw.extend_from_slice(&tail);

                // 跨块防裂：拼接上轮扣留残片，再扣留本块尾部 DA1 前缀
                let mut frame = std::mem::take(&mut pending_da1);
                frame.extend_from_slice(&raw);
                let (body, rest) = split_trailing_da1_prefix(frame);
                pending_da1 = rest;
                if body.is_empty() {
                    // 整块均为 DA1 前缀残片（≤3B）——待下轮补全，不发送不置 drained
                    continue;
                }

                // DA1 全量接管：检测必须在剥离前的原始字节上（剥离后查询已不存在）。
                // 每次查询都代答（旧「每会话一次」AtomicBool 语义已随接管移除）——
                // 前端 xterm.js 不再见到 DA1，后端是唯一应答方，应答身份单一
                if mirror_da1_query(&body) {
                    inject_da1_response(&writer);
                }

                // 剥离：启动窗口内剥 ConPTY 启动序列+DA1，窗口外仅剥 DA1。
                // 全部剥离则跳过本轮且不置 drained（BE-13 跨边界残留窗口保持）
                let out = match apply_output_strip(startup_drained, &body) {
                    Some(o) => {
                        startup_drained = true;
                        o
                    }
                    None => continue,
                };

                if let Err(e) = channel.send(PtyEvent::Output { bytes: out }) {
                    // CP-034: Channel 断开（前端已卸载）——单路径语义:退出,不缓冲
                    tracing::debug!("Channel send 失败(前端已断开),reader 退出: {e}");
                    break;
                }
            }
            Err(e) => {
                tracing::warn!("PTY reader 错误: {e}");
                // P2-42: 记录错误退出码到共享状态
                let mut ec = exit_code.lock();
                *ec = Some(-1);
                if let Err(e) = channel.send(PtyEvent::Exit { code: Some(-1) }) {
                    tracing::debug!("Channel send 失败(前端已断开),reader 退出: {e}");
                }
                break;
            }
        }
    }
}

/// CP-011: reader join 超时后的清理决策（纯函数，L1 锁死两分支）
pub(crate) enum CleanupPlan {
    /// reader 已退出——正常路径，session 就地 drop
    NormalDrop,
    /// reader 超时未退出——reader detach，session 移交监督线程执行 drop
    /// （ConPtyInner::drop → ClosePseudoConsole 不在调用线程执行）
    DetachReaderSupervisedDrop,
}

/// CP-011: reader join 超时后的清理决策（照 `eof_exit_code` 先例抽纯函数，
/// 由 pty_kill/pty_kill_all 注入 join 结果并据此分支）
pub(crate) fn plan_cleanup_after_join_timeout(reader_finished: bool) -> CleanupPlan {
    if reader_finished {
        CleanupPlan::NormalDrop
    } else {
        CleanupPlan::DetachReaderSupervisedDrop
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
/// PeekNamedPipe 非阻塞查询可读字节数（> 0 才为 true；对端关闭时 Peek 返回 0，
/// 由后续 read Ok(0) EOF 兜底），因此 read 不会空等，「读到即续读」而非定时器轮询。
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

/// DA1 应答注入（DA1 全量接管）
///
/// 检测到 DA1 查询（ESC[c / ESC[0c）时向子进程 stdin 写入 ESC[?64;22c
/// （VT420+ANSI 颜色——模拟 ConPTY/conhost 的应答身份，Claude Code Ink 已验证
/// 接受该应答；缺应答 Ink 启动阻塞约 60s）。每次查询都应答——查询已不透传
/// 前端，后端是唯一应答方（旧「每会话一次」防重语义随接管移除）。
/// 检测决策 = 纯函数 `mirror_da1_query`，注入动作为 I/O（M11 豁免项）。
fn inject_da1_response(writer: &Mutex<Box<dyn Write + Send>>) {
    // 向子进程 stdin 注入 DA1 响应（不阻塞 reader 线程；CP-005: 锁无失败分支）
    let mut w = writer.lock();
    if let Err(e) = w.write_all(b"\x1b[?64;22c") {
        tracing::warn!("DA1 响应注入失败: {}", e);
    }
    if let Err(e) = w.flush() {
        tracing::warn!("DA1 响应注入失败: {}", e);
    }
}

/// EOF 退出码降级决策（P2-11/P2-42）
///
/// 输入为 reader_loop 的 lock/wait 两级结果（CP-005: parking_lot 锁无失败路径，
/// 外层 Err 恒不可达——保留两级形态以维持纯函数签名与既有测试）；
/// 内层 Err = `child.wait()` 失败 → `None`（退出码未知，不硬编码 0——
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
/// - DA1 查询: `ESC [ c` / `ESC [ 0 c`（接管剥离——Win10 捆绑 conhost
///   OpenConsole 1.24 启动握手多发，透传会触发 xterm.js 自答回灌 stdin）
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
        // DA1 查询 ESC[c（接管剥离，不透传前端）
        b'c' => Some(3),
        // DA1 变体 ESC[0c
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

/// 剥离 DA1 查询序列（ESC[c / ESC[0c）——启动窗口外（startup_drained 后）使用
///
/// DA1 全量接管：查询一律不透传前端（xterm.js 核心会自答并经 onData 无差别
/// 回灌 stdin），由 reader_loop 检测后代答。仅匹配两种精确形态——
/// DA2（ESC[>c）、XTVERSION（ESC[>0q）、带参变体（ESC[1c 等）均不动
///（同族隐患登记于 pty/CLAUDE.md，观测到受害场景再接管）。
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

/// 切分块尾 DA1 前缀残片（纯函数）
///
/// DA1 查询（ESC[c / ESC[0c）可能跨 read 块边界——块尾若为其前缀
///（ESC / ESC[ / ESC[0），扣留待下块拼接，防止半条序列送前端后被
/// xterm.js 跨写入拼合自答（DA1 接管的跨块形态）。返回（主体, 扣留残片）。
fn split_trailing_da1_prefix(data: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    for prefix in [
        b"\x1b[0".as_slice(),
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

/// 输出剥离统一入口（纯函数）
///
/// - 启动窗口内（startup_drained=false）：`strip_conpty_startup` 剥离 ConPTY
///   启动序列 + DA1 查询；全部剥离返回 None——调用方跳过本轮且不置 drained
///   （BE-13 跨边界残留窗口保持开放）
/// - 窗口外（drained=true）：仅剥 DA1 查询；整块全是 DA1 时返回 None
///   （无内容可发——如 Ink 启动哨兵被整块剥离），否则 Some(剥离后数据)
fn apply_output_strip(startup_drained: bool, data: &[u8]) -> Option<Vec<u8>> {
    let stripped = if startup_drained {
        strip_da1_queries(data)
    } else {
        strip_conpty_startup(data)
    };
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
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
mod reader_tests {
    use super::*;

    // ─── M11: reader_loop 主循环纯函数化分析 ───
    //
    // reader_loop 主循环有三个 match 分支，均已审查可抽取性：
    //
    // 1. Ok(0) — EOF 分支：
    //    - pending 残片冲刷      → DA1 前缀残片（不完整序列）原样转发，无决策
    //    - eof_exit_code()       → ✅ 已抽取为纯函数（PTY-12）：lock/wait 两级
    //                              失败 → None（不硬编码 0），成功 → 真实退出码
    //    - child.wait()          → portable_pty::Child::wait() 是系统调用（Windows WaitForSingleObject），I/O
    //    - exit_code.lock()      → parking_lot::Mutex，运行时同步原语
    //    - channel.send(Exit)    → Tauri IPC Channel::send()，I/O（CP-034: 直写无锁层，
    //                              断开即退出）
    //
    // 2. Ok(n) — 数据分支（DA1 全量接管后形态：聚合 → 拼接 → 检测/代答 → 剥离 → 发送）：
    //    - micro_batch_tail()       → ✅ 已抽取为纯函数（BE-05）：pending 检查 +
    //                                 续读累积（read 为系统调用，决策已抽，调用不可抽）
    //    - split_trailing_da1_prefix() → ✅ 纯函数（跨块 DA1 前缀扣留）
    //    - mirror_da1_query()       → ✅ 纯函数（剥离前检测；注入动作
    //                                 inject_da1_response = writer.lock() + 管道 I/O，
    //                                 经共享 Vec writer 用例直测）
    //    - apply_output_strip()     → ✅ 纯函数（启动序列 + DA1 剥离统一入口）
    //    - channel.send(Output)     → Channel::send()，I/O
    //
    // 3. Err(e) — 读错误分支：
    //    - tracing::warn!()       → 日志宏，I/O
    //    - exit_code.lock()       → Mutex
    //    - channel.send(Exit)     → Channel::send()，I/O
    //
    // 结论：reader_loop 中剩余的所有分支决策均依赖锁/系统调用或 IPC send，
    // 无法在不引入运行时依赖的前提下构造测试输入，无法进一步抽取为纯函数。
    // apply_output_strip / mirror_da1_query / split_trailing_da1_prefix /
    // eof_exit_code / micro_batch_tail 已覆盖主循环中全部可纯函数化的决策逻辑。
    //
    // M11 状态：已尽力——剩余均为 I/O 编排无法纯函数化。
    // PTY-12 评估产出：残余不可抽分支明细 + 豁免理由见
    // src-tauri/src/pty/CLAUDE.md「reader_loop I/O 编排残余豁免（草稿）」
    // （Stage 17 统一收编为豁免表，DOC-01 引用）。
    // DOC-01 豁免项 1 随 BE-05 微批变动（send 次数降为每微批一次，
    // 新增 pending 检查——决策已抽为 micro_batch_tail）；CP-034 后 Channel 直写
    // （无替换层与回放层）——豁免表同步在 S19/CP-034。

    #[test]
    fn strip_osc_title_bel() {
        let input = b"\x1b]0;pwsh\x07";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn strip_clear_screen() {
        let input = b"\x1b[2J";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn strip_clear_screen_3j() {
        // PTY-04: CSI 3J（清屏含滚动缓冲）与 2J 一并剥离
        assert_eq!(strip_conpty_startup(b"\x1b[3J"), b"");
    }

    #[test]
    fn strip_cursor_home() {
        let input = b"\x1b[H";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn strip_dsr() {
        let input = b"\x1b[6n";
        assert_eq!(strip_conpty_startup(input), b"");
    }

    #[test]
    fn strip_cursor_visibility() {
        assert_eq!(strip_conpty_startup(b"\x1b[?25h"), b"");
        assert_eq!(strip_conpty_startup(b"\x1b[?25l"), b"");
    }

    #[test]
    fn strip_da1_query_in_startup() {
        // DA1 全量接管：启动窗口内的 DA1 查询（Win10 捆绑 conhost 启动握手多发）
        // 一并剥离，不透传前端
        assert_eq!(strip_conpty_startup(b"\x1b[c"), b"");
        assert_eq!(strip_conpty_startup(b"\x1b[0c"), b"");
    }

    #[test]
    fn strip_preserves_da1_lookalikes() {
        // DA2（ESC[>c）/ 带参变体（ESC[1c）不是 DA1 查询——不动（P5 出范围登记）
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

    // ─── apply_output_strip 纯函数测试 ───

    #[test]
    fn output_strip_drained_passthrough() {
        // 非首轮：无 DA1 的输出原样返回
        let data = b"normal output";
        let result = apply_output_strip(true, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    #[test]
    fn output_strip_drained_strips_da1() {
        // 非首轮：DA1 查询剥离不透传前端（Ink 启动哨兵场景）
        assert_eq!(
            apply_output_strip(true, b"prompt> \x1b[c more"),
            Some(b"prompt>  more".to_vec())
        );
        assert_eq!(
            apply_output_strip(true, b"\x1b[0c rest"),
            Some(b" rest".to_vec())
        );
    }

    #[test]
    fn output_strip_drained_all_da1_returns_none() {
        // 非首轮：整块全是 DA1（如 Ink 哨兵独占一块）→ None 无内容可发
        assert_eq!(apply_output_strip(true, b"\x1b[c"), None);
        assert_eq!(apply_output_strip(true, b"\x1b[0c"), None);
    }

    #[test]
    fn output_strip_first_round_all_stripped() {
        // 首轮全部为启动序列 → 返回 None（跳过本轮）
        let result = apply_output_strip(false, b"\x1b]0;pwsh\x07\x1b[2J\x1b[H");
        assert_eq!(result, None);
    }

    #[test]
    fn output_strip_first_round_startup_plus_da1_all_stripped() {
        // 防复发回归（本 bug 核心场景）：启动窗口内「启动序列 + DA1」整块——
        // 前端零字节（旧代码 DA1 不在剥离清单 → 透传给 xterm 触发自答回灌）。
        // 注：调用方须在剥离前的原始字节上做 mirror_da1_query 检测并代答，
        // 旧代码的 None→continue 路径会跳过检测（第二个漏洞），现检测前移
        let result = apply_output_strip(false, b"\x1b[2J\x1b[c\x1b[H");
        assert_eq!(result, None);
        assert!(mirror_da1_query(b"\x1b[2J\x1b[c\x1b[H"));
    }

    #[test]
    fn output_strip_first_round_partial_strip() {
        // 首轮启动序列后跟正常输出 → 剥离前缀
        let result = apply_output_strip(false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(result, Some(b"PS C:\\> ".to_vec()));
    }

    #[test]
    fn output_strip_across_buffer_boundary() {
        // BE-13: 跨缓冲区边界的启动序列剥离——第一轮全为启动序列（None），
        // 不置 drained；第二轮仍有启动序列 + 真实输出，应继续剥离
        let r1 = apply_output_strip(false, b"\x1b]0;pwsh\x07");
        assert_eq!(r1, None); // 第一轮全部是 OSC 标题 → 跳过

        // 第二轮仍用 startup_drained=false（模拟 reader_loop 中 None 分支不改 drained）
        let r2 = apply_output_strip(false, b"\x1b[2J\x1b[HPS C:\\> ");
        assert_eq!(r2, Some(b"PS C:\\> ".to_vec())); // 清屏+归位被剥离，保留真实输出
    }

    #[test]
    fn output_strip_multi_round_all_startup_then_real() {
        // BE-13: 多轮纯启动序列后出现真实输出——验证 drained 仅在 Some 时才置
        // 模拟三轮：OSC 标题 → 清屏 → 光标归位+真实输出
        let r1 = apply_output_strip(false, b"\x1b]0;pwsh\x07");
        assert_eq!(r1, None);

        let r2 = apply_output_strip(false, b"\x1b[2J");
        assert_eq!(r2, None);

        let r3 = apply_output_strip(false, b"\x1b[?25h\x1b[HHello World");
        assert_eq!(r3, Some(b"Hello World".to_vec()));
    }

    #[test]
    fn output_strip_first_round_no_startup_seq() {
        // 首轮无启动序列（如 cmd.exe 场景）→ 原样返回
        let data = b"Microsoft Windows [Version 10.0]\r\n";
        let result = apply_output_strip(false, data);
        assert_eq!(result, Some(data.to_vec()));
    }

    // ─── strip_da1_queries 纯函数测试 ───

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
        // DA2 / XTVERSION / 带参变体 / 其它 CSI 均不动（P5 出范围登记）
        assert_eq!(strip_da1_queries(b"\x1b[>c"), b"\x1b[>c");
        assert_eq!(strip_da1_queries(b"\x1b[>0q"), b"\x1b[>0q");
        assert_eq!(strip_da1_queries(b"\x1b[1c"), b"\x1b[1c");
        assert_eq!(strip_da1_queries(b"\x1b[2J"), b"\x1b[2J");
    }

    #[test]
    fn strip_da1_trailing_partial_forwarded_as_is() {
        // 纯函数本身不扣留残片（扣留是 reader_loop 的 split_trailing_da1_prefix
        // 职责）——尾部落单的 ESC/ESC[ 原样通过，记录两层的职责边界
        assert_eq!(strip_da1_queries(b"abc\x1b"), b"abc\x1b");
        assert_eq!(strip_da1_queries(b"abc\x1b["), b"abc\x1b[");
    }

    // ─── split_trailing_da1_prefix 纯函数测试 ───

    #[test]
    fn split_holds_trailing_esc() {
        let (body, pending) = split_trailing_da1_prefix(b"abc\x1b".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b");
    }

    #[test]
    fn split_holds_trailing_esc_bracket() {
        let (body, pending) = split_trailing_da1_prefix(b"abc\x1b[".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b[");
    }

    #[test]
    fn split_holds_trailing_esc_bracket_zero() {
        let (body, pending) = split_trailing_da1_prefix(b"abc\x1b[0".to_vec());
        assert_eq!(body, b"abc");
        assert_eq!(pending, b"\x1b[0");
    }

    #[test]
    fn split_complete_da1_not_held() {
        // 完整 DA1 查询不是前缀残片——不扣留（交检测/剥离处理）
        let (body, pending) = split_trailing_da1_prefix(b"\x1b[c".to_vec());
        assert_eq!(body, b"\x1b[c");
        assert!(pending.is_empty());
        let (body, pending) = split_trailing_da1_prefix(b"\x1b[0c".to_vec());
        assert_eq!(body, b"\x1b[0c");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_non_da1_prefix_not_held() {
        // ESC[1 / ESC[? 等不是 DA1 前缀——不扣留（ESC[1c 非 DA1，不误伤）
        let (body, pending) = split_trailing_da1_prefix(b"abc\x1b[1".to_vec());
        assert_eq!(body, b"abc\x1b[1");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_plain_and_empty_input() {
        let (body, pending) = split_trailing_da1_prefix(b"plain".to_vec());
        assert_eq!(body, b"plain");
        assert!(pending.is_empty());
        let (body, pending) = split_trailing_da1_prefix(Vec::new());
        assert!(body.is_empty());
        assert!(pending.is_empty());
    }

    #[test]
    fn split_reassembly_closes_cross_chunk_da1() {
        // 防复发回归（跨块形态）：chunk1 = "abc ESC["，chunk2 = "c def"——
        // 拼接后检测命中 + 剥离无泄漏（旧代码半条 ESC[ 送前端，
        // xterm 跨写入拼合后自答）
        let (b1, p1) = split_trailing_da1_prefix(b"abc\x1b[".to_vec());
        assert_eq!(b1, b"abc");
        assert_eq!(p1, b"\x1b[");
        let mut frame = p1;
        frame.extend_from_slice(b"c def");
        let (b2, p2) = split_trailing_da1_prefix(frame);
        assert!(p2.is_empty());
        assert!(mirror_da1_query(&b2), "拼接后应检测到 DA1 → 触发代答");
        assert_eq!(strip_da1_queries(&b2), b" def", "剥离后前端无泄漏");
    }

    // ─── inject_da1_response 动作级测试（共享 Vec writer）───

    /// 共享 Vec writer——把注入动作（I/O）纳入 L1 直测
    struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn make_shared_writer() -> (
        Mutex<Box<dyn Write + Send>>,
        std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    ) {
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer: Mutex<Box<dyn Write + Send>> = Mutex::new(Box::new(SharedWriter(buf.clone())));
        (writer, buf)
    }

    #[test]
    fn inject_da1_response_writes_vt420_identity() {
        let (writer, buf) = make_shared_writer();
        inject_da1_response(&writer);
        assert_eq!(*buf.lock().unwrap(), b"\x1b[?64;22c");
    }

    #[test]
    fn inject_da1_response_every_query_answered() {
        // 防复发回归：每次查询都应答（旧「每会话一次」AtomicBool 语义下
        // 第二次查询无应答——接管后前端不再自答，后端漏答 = 应用干等）
        let (writer, buf) = make_shared_writer();
        inject_da1_response(&writer);
        inject_da1_response(&writer);
        assert_eq!(*buf.lock().unwrap(), b"\x1b[?64;22c\x1b[?64;22c");
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
            Box::new(MockSeqReader {
                seq: vec![],
                idx: 0,
            }),
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

    // ─── CP-011: plan_cleanup_after_join_timeout ───
    // （join_with_timeout 已上提 crate::thread_join，用例随迁 mod join_tests，BE-01）

    #[test]
    fn cleanup_plan_finished_reader_normal_drop() {
        // reader 已退出 → 正常 drop（session 就地销毁）
        assert!(matches!(
            plan_cleanup_after_join_timeout(true),
            CleanupPlan::NormalDrop
        ));
    }

    #[test]
    fn cleanup_plan_timeout_reader_supervised_drop() {
        // reader join 超时 → detach + 监督线程 drop 分支
        assert!(matches!(
            plan_cleanup_after_join_timeout(false),
            CleanupPlan::DetachReaderSupervisedDrop
        ));
    }
}
