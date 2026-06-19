use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

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
}

/// PTY 全局状态 — HashMap 按 session_id 索引，加 spawn 串行锁
pub struct PtyState {
    /// session_id → PtySession
    pub sessions: RwLock<HashMap<String, PtySession>>,
    /// ConPTY spawn 串行化锁（Windows 并发 spawn 会卡死输出管道）
    pub spawn_lock: Mutex<()>,
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

impl AppState {
    pub fn new() -> Self {
        Self {
            pty: PtyState::new(),
        }
    }
}
