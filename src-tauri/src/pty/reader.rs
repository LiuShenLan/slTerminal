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
use std::io::Read;
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
                let bytes = if startup_drained {
                    buf[..n].to_vec()
                } else {
                    startup_drained = true;
                    let stripped = strip_conpty_startup(&buf[..n]);
                    if stripped.is_empty() {
                        continue; // 全部被剥离，读下一轮
                    }
                    stripped
                };

                let ch = match channel.read() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("reader_loop channel 锁获取失败: {e}");
                        break;
                    }
                };
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
        b's' | b'u' => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
