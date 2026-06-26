use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use tauri::ipc::Channel;

use crate::pty::spawn::PtyEvent;

/// PTY 会话 — 持有 master（读写/缩放）、子进程、writer 和 reader 线程句柄
pub struct PtySession {
    /// PTY master 端，用于 resize；Mutex 包裹以满足 Sync 要求
    pub master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    /// 子进程句柄，用于 kill 和获取退出码
    pub child: Mutex<Box<dyn portable_pty::Child + Send>>,
    /// 共享 writer — take_writer 仅一次，Arc<Mutex> 供所有 pty_write 共享
    pub writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    /// reader 线程句柄，pty_kill 时 join 回收
    pub reader_handle: Option<JoinHandle<()>>,
    /// 可替换 Channel（E1: pty_reattach 时替换）
    pub channel: Arc<RwLock<Option<Channel<PtyEvent>>>>,
    /// 输出回放缓冲区（E1: 64KB FIFO，Channel 断开后缓存最近输出）
    pub output_ring: Arc<Mutex<VecDeque<u8>>>,
}

/// PTY 全局状态 — HashMap 按 session_id 索引，加 spawn 串行锁
pub struct PtyState {
    /// session_id → PtySession
    pub sessions: RwLock<HashMap<String, PtySession>>,
    /// ConPTY spawn 串行化锁（Windows 并发 spawn 会卡死输出管道）
    pub spawn_lock: Mutex<()>,
}

impl Default for PtyState {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyState {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            spawn_lock: Mutex::new(()),
        }
    }
}

/// 应用全局状态，各模块通过 AppState 共享资源
pub struct AppState {
    pub pty: PtyState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            pty: PtyState::new(),
        }
    }
}

/// ring buffer 最大容量（64KB，保留最近约 1000+ 行终端输出）
const RING_BUFFER_CAPACITY: usize = 65536;

/// 向 ring buffer 追加数据，超过容量时从头部丢弃旧数据
pub fn ring_buffer_append(ring: &Mutex<VecDeque<u8>>, data: &[u8]) {
    let mut buf = ring.lock().unwrap();
    buf.extend(data);
    // FIFO: 超过容量时从头部丢弃
    while buf.len() > RING_BUFFER_CAPACITY {
        // 按粗略行粒度丢弃，避免截断 UTF-8 字符
        let drain_len = 1024usize.min(buf.len());
        for _ in 0..drain_len {
            buf.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_state_new_empty() {
        let pty = PtyState::new();
        assert!(
            pty.sessions.read().unwrap().is_empty(),
            "新建 PtyState 的 sessions 应为空"
        );
    }

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert!(
            state.pty.sessions.read().unwrap().is_empty(),
            "AppState::new() 应成功创建并持有空的 PtyState"
        );
    }

    #[test]
    fn test_ring_buffer_append_fifo() {
        let ring = Mutex::new(VecDeque::new());
        let data: Vec<u8> = (0..255).collect(); // 255 bytes
        ring_buffer_append(&ring, &data);
        let buf = ring.lock().unwrap();
        assert_eq!(buf.len(), 255);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let ring = Mutex::new(VecDeque::new());
        // 写 70KB 数据，应触发淘汰
        let data: Vec<u8> = vec![b'A'; 71680]; // 70KB
        ring_buffer_append(&ring, &data);
        let buf = ring.lock().unwrap();
        assert!(buf.len() <= RING_BUFFER_CAPACITY);
        // 缓冲区应包含最近写入的数据（尾部是 A）
        assert_eq!(buf[buf.len() - 1], b'A');
    }
}
