/// PTY reader 线程 — 阻塞读取 PTY 输出 → Channel 推送 PtyEvent
///
/// 独立线程运行，不阻塞 tokio runtime。读取到 EOF（子进程退出）时发送 Exit 事件并退出。
use crate::pty::spawn::PtyEvent;
use std::io::Read;
use tauri::ipc::Channel;

/// reader 线程主循环
///
/// - 循环读取 PTY 输出，通过 Channel 发送 Output 事件
/// - Ok(0) = EOF → 发 Exit 事件 → 退出
/// - 错误 → 尝试发 Exit(Some(-1)) → 退出
pub fn reader_loop(mut reader: Box<dyn Read + Send>, on_output: Channel<PtyEvent>) {
    let mut buf = [0u8; 4096];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF — 子进程已退出
                let _ = on_output.send(PtyEvent::Exit { code: Some(0) });
                break;
            }
            Ok(n) => {
                let bytes = buf[..n].to_vec();
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
