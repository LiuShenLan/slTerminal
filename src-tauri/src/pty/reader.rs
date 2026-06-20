/// PTY reader 线程 — 阻塞读取 PTY 输出 → Channel 推送 PtyEvent
///
/// 独立线程运行，不阻塞 tokio runtime。读取到 EOF（子进程退出）时发送 Exit 事件并退出。
///
/// Windows: 首轮读取时剥离 ConPTY VtIo::StartIfNeeded() 注入的启动序列
/// （OSC 标题含 BEL→蜂鸣、清屏/归位→首字符被覆盖、DSR/光标查询），
/// 后续读取原样透传。
use crate::pty::spawn::PtyEvent;
use std::io::Read;
use tauri::ipc::Channel;

/// reader 线程主循环
///
/// - 循环读取 PTY 输出，通过 Channel 发送 Output 事件
/// - Ok(0) = EOF → 发 Exit 事件 → 退出
/// - 错误 → 尝试发 Exit(Some(-1)) → 退出
/// - Windows 首轮读取剥离 ConPTY 启动注入序列
pub fn reader_loop(mut reader: Box<dyn Read + Send>, on_output: Channel<PtyEvent>) {
    let mut buf = [0u8; 4096];
    let mut startup_drained = false;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF — 子进程已退出
                let _ = on_output.send(PtyEvent::Exit { code: Some(0) });
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
                if on_output.send(PtyEvent::Output { bytes }).is_err() {
                    // Channel 已关闭（前端断开），停止读取
                    break;
                }
            }
            Err(e) => {
                tracing::warn!("PTY reader 错误: {e}");
                let _ = on_output.send(PtyEvent::Exit { code: Some(-1) });
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
            // 仅剥离 ConPTY 启动注入的 OSC 0（窗口标题）和 OSC 2（窗口标题变体），
            // 保留 shell 集成的 OSC 7（CWD）/ 9;9（ConEmu CWD）/ 133（提示符边界）
            if i + 1 < data.len() && data[i + 1] == b']' {
                if i + 2 < data.len() && (data[i + 2] == b'0' || data[i + 2] == b'2') {
                    if let Some(end) = find_osc_end(&data[i..]) {
                        i += end;
                        continue;
                    }
                }
                // 非 ConPTY 启动 OSC，保留
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
                // 非启动序列的 CSI，保留
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

/// 定位 OSC 序列的结束位置（含终结符），未终结返回 None
fn find_osc_end(data: &[u8]) -> Option<usize> {
    // data[0] == ESC, data[1] == ']'
    for j in 2..data.len() {
        match data[j] {
            0x07 => return Some(j + 1), // BEL 终结
            0x1b if j + 1 < data.len() && data[j + 1] == b'\\' => return Some(j + 2), // ST 终结
            _ => {}
        }
    }
    None // 跨缓冲区，未终结
}

/// 匹配已知的 ConPTY 启动 CSI 序列，返回序列字节长度；不匹配返回 None
fn match_csi_startup(data: &[u8]) -> Option<usize> {
    // data[0] == ESC, data[1] == '['
    if data.len() < 3 {
        return None;
    }
    match data[2] {
        // ESC [ H — 光标归位
        b'H' => Some(3),
        // ESC [ 2 J / ESC [ 3 J — 清屏/擦除回滚
        b'2' | b'3' if data.len() >= 4 && data[3] == b'J' => Some(4),
        // ESC [ 6 n — DSR 光标查询
        b'6' if data.len() >= 4 && data[3] == b'n' => Some(4),
        // ESC [ ? 2 5 h / l — 光标显隐
        b'?' if data.len() >= 6
            && data[3] == b'2'
            && data[4] == b'5'
            && (data[5] == b'h' || data[5] == b'l') =>
        {
            Some(6)
        }
        // ESC [ s / ESC [ u — 保存/恢复光标（部分 ConPTY 版本注入）
        b's' | b'u' => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_osc_title_bel() {
        // \x1b]0;pwsh\x07 → 应全部剥离
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
        // ConPTY 启动序列后紧跟 shell 输出
        let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[HPS C:\\> ";
        assert_eq!(strip_conpty_startup(input), b"PS C:\\> ");
    }

    #[test]
    fn test_preserve_osc7_cwd() {
        // OSC 7（我们的 shell 集成 CWD 序列）应保留
        let input = b"\x1b]7;file:///C:/Users\x1b\\";
        assert_eq!(strip_conpty_startup(input), input);
    }
}
