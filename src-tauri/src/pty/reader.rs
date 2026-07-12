/// PTY reader 线程 — 阻塞读取 PTY 输出 → Channel 推送 PtyEvent
///
/// E1: Channel 断开时不退出，写入 ring buffer 等待 reattach。
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

/// reader 线程主循环（E1: 支持重连）
///
/// - channel: 可替换的 Channel 引用，pty_reattach 通过写锁替换
/// - ring: ring buffer，总是缓存最近输出供 reattach 回放
/// - child: P2-11 子进程句柄，EOF 时调用 wait() 获取真实退出码
/// - exit_code: P2-42 退出状态共享，reader 设置后 pty_reattach 检测
/// - 循环读取 PTY 输出，通过 Channel 发送 Output 事件
/// - Ok(0) = EOF → 发 Exit 事件 → 退出
/// - Windows 首轮读取剥离 ConPTY 启动注入序列
pub fn reader_loop(
    mut reader: Box<dyn Read + Send>,
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
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF — 子进程已退出
                // P2-11: 从 child.wait() 获取真实退出码而非硬编码 0
                let code = match child.lock() {
                    Ok(mut c) => {
                        match c.wait() {
                            Ok(status) => Some(status.exit_code() as i32),
                            Err(e) => {
                                tracing::warn!("child.wait() 失败: {e}");
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("reader_loop child 锁获取失败: {e}");
                        None
                    }
                };

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
                    let _ = c.send(PtyEvent::Exit { code });
                }
                break;
            }
            Ok(n) => {
                // 首轮剥离 ConPTY 启动序列（纯函数），全部剥离则跳过本轮
                let bytes = match apply_startup_strip(startup_drained, &buf[..n]) {
                    Some(b) => {
                        startup_drained = true;
                        b
                    }
                    None => {
                        startup_drained = true;
                        continue;
                    }
                };

                let ch = match channel.read() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("reader_loop channel 锁获取失败: {e}");
                        break;
                    }
                };
                // DA1 查询模拟响应：Claude Code Ink 渲染器启动时发 ESC[c 作为同步哨兵。
                // ConPTY 拦截 DA1 查询后内部处理，不向子进程 stdout 返回响应。
                // 导致 Ink waitFor 永不 resolve，阻塞约 60s。
                // 此处检测子进程发出的 DA1 查询，向 stdin 注入 ESC[?64;22c（VT420+ANSI 颜色）
                // 模拟 ConPTY 的一致行为。同一会话仅注入一次（AtomicBool 防重复）。
                if should_inject_da1(da1_injected.load(Ordering::Relaxed), &bytes) {
                    da1_injected.store(true, Ordering::Relaxed);
                    // 向子进程 stdin 注入 DA1 响应（不阻塞 reader 线程）
                    if let Ok(mut w) = writer.lock() {
                        let _ = w.write_all(b"\x1b[?64;22c");
                        let _ = w.flush();
                    }
                }
                // P2-46: 总是先缓存到 ring buffer（不 clone），再 send 消耗 bytes
                // 成功路径零 clone，失败路径（Channel 断连）ring buffer 已有数据
                if let Err(e) = ring_buffer_append(&ring, &bytes) {
                    tracing::warn!("ring buffer 写入失败: {e}");
                }
                if let Some(ref c) = *ch {
                    let _ = c.send(PtyEvent::Output { bytes }); // move bytes，不 clone
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
                    let _ = c.send(PtyEvent::Exit { code: Some(-1) });
                }
                break;
            }
        }
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
    // 非 Windows 无 ConPTY，原样返回
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
    //    - child.wait()          → portable_pty::Child::wait() 是系统调用（Windows WaitForSingleObject），I/O
    //    - exit_code.lock()       → std::sync::Mutex，运行时同步原语
    //    - channel.read()         → std::sync::RwLock，运行时同步原语
    //    - c.send(PtyEvent::Exit) → Tauri IPC Channel::send()，I/O
    //
    // 2. Ok(n) — 数据分支：
    //    - apply_startup_strip()  → ✅ 已抽取为纯函数（Phase 2）
    //    - channel.read()         → RwLock
    //    - should_inject_da1()    → ✅ 已抽取为纯函数（Phase 2）
    //    - writer.lock()+write_all()+flush() → Mutex + 管道 I/O
    //    - ring_buffer_append()   → Mutex + VecDeque 状态变更
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
    // 无法进一步抽取为纯函数。apply_startup_strip 和 should_inject_da1
    // 已覆盖主循环中全部可纯函数化的决策逻辑。
    //
    // M11 状态：已尽力——剩余均为 I/O 编排无法纯函数化。

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
}
