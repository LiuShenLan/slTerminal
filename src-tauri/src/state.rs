use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use tauri::ipc::Channel;

use crate::error::AppError;
use crate::notify::FileWatcher;
use crate::pty::spawn::PtyEvent;

/// PTY 会话 — 持有 master（读写/缩放）、子进程、writer 和 reader 线程句柄
pub struct PtySession {
    /// PTY master 端，用于 resize；Mutex 包裹以满足 Sync 要求
    pub master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    /// 子进程句柄，用于 kill 和获取退出码
    /// P2-11: 改为 Arc<Mutex<>> 以在 reader 线程中调用 child.wait() 获取真实退出码
    pub child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
    /// 共享 writer — take_writer 仅一次，Arc<Mutex> 供所有 pty_write 共享
    pub writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    /// reader 线程句柄，pty_kill 时 join 回收
    pub reader_handle: Option<JoinHandle<()>>,
    /// 可替换 Channel（E1: pty_reattach 时替换）
    pub channel: Arc<RwLock<Option<Channel<PtyEvent>>>>,
    /// 输出回放缓冲区（E1: 256KB FIFO，Channel 断开后缓存最近输出）
    pub output_ring: Arc<Mutex<VecDeque<u8>>>,
    /// P2-42: 子进程退出码（reader 线程在 EOF/错误时设置，pty_reattach 检测后发送 Exit）
    pub exit_code: Arc<Mutex<Option<i32>>>,
    /// Windows Job Object 句柄（孤儿防护，drop 时 CloseHandle）
    /// 此 #[cfg] 虽在 state.rs，但 PtySession 是 PTY 概念，
    /// 符合架构约束 #9 "等明确处" 精神
    #[cfg(windows)]
    pub job_object: Option<crate::pty::spawn::JobHandle>,
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
    /// 文件系统监听器（由前端 fs_watch 命令按需创建/替换）
    pub file_watcher: Mutex<Option<FileWatcher>>,
    /// 当前项目根路径（由前端打开项目时设置，用于路径 sandbox 校验）
    pub project_root: RwLock<Option<PathBuf>>,
    /// git 仓库缓存：workdir → Repository（目录切换时清除）
    pub git_repo_cache: Mutex<HashMap<PathBuf, git2::Repository>>,
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
            file_watcher: Mutex::new(None),
            project_root: RwLock::new(None),
            git_repo_cache: Mutex::new(HashMap::new()),
        }
    }
}

/// ring buffer 最大容量（256KB，保留最近约 4000+ 行终端输出）
const RING_BUFFER_CAPACITY: usize = 262144; // 256KB

/// 向 ring buffer 追加数据，超过容量时从头部丢弃旧数据
/// P2-47: 淘汰到 \\n 边界，避免行内 UTF-8 序列截断
pub fn ring_buffer_append(ring: &Mutex<VecDeque<u8>>, data: &[u8]) -> Result<(), AppError> {
    let mut buf = ring.lock().map_err(|e| AppError::Pty(format!("锁获取失败: {e}")))?;
    buf.extend(data);
    // FIFO: 超过容量时从头部以行为粒度丢弃
    while buf.len() > RING_BUFFER_CAPACITY {
        let drain_target = 1024usize.min(buf.len());
        // 在 drain_target 范围内找最后一个 \\n，对齐行边界
        let prefix: Vec<u8> = buf.iter().take(drain_target).copied().collect();
        let drain_len = prefix
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(drain_target, |pos| pos + 1); // 包括 \\n 本身；无换行时按原量淘汰（罕见，仅超长行）
        for _ in 0..drain_len {
            buf.pop_front();
        }
    }
    Ok(())
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
        assert!(
            state.file_watcher.lock().unwrap().is_none(),
            "AppState::new() 初始时 file_watcher 应为 None"
        );
        assert!(
            state.project_root.read().unwrap().is_none(),
            "AppState::new() 初始时 project_root 应为 None"
        );
        assert!(
            state.git_repo_cache.lock().unwrap().is_empty(),
            "AppState::new() 初始时 git_repo_cache 应为空"
        );
    }

    #[test]
    fn test_ring_buffer_append_fifo() {
        let ring = Mutex::new(VecDeque::new());
        let data: Vec<u8> = (0..255).collect(); // 255 bytes
        ring_buffer_append(&ring, &data).unwrap();
        let buf = ring.lock().unwrap();
        assert_eq!(buf.len(), 255);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let ring = Mutex::new(VecDeque::new());
        // 写 280KB 数据（超过 256KB 容量），应触发淘汰
        let data: Vec<u8> = vec![b'A'; 286720]; // 280KB
        ring_buffer_append(&ring, &data).unwrap();
        let buf = ring.lock().unwrap();
        assert!(buf.len() <= RING_BUFFER_CAPACITY);
        // 缓冲区应包含最近写入的数据（尾部是 A）
        assert_eq!(buf[buf.len() - 1], b'A');
    }

    /// P2-47: 淘汰时以 \\n 为边界，不截断行
    #[test]
    fn test_ring_buffer_eviction_at_newline_boundary() {
        let ring = Mutex::new(VecDeque::new());
        // 每行 100 字节 + \\n，填到超过容量
        let line = [b'X'; 100];
        let mut total = 0usize;
        while total < RING_BUFFER_CAPACITY + 10240 {
            ring_buffer_append(&ring, &line).unwrap();
            ring_buffer_append(&ring, b"\n").unwrap();
            total += 101;
        }
        let buf = ring.lock().unwrap();
        assert!(buf.len() <= RING_BUFFER_CAPACITY);
        // 淘汰后第一个字节应是完整行起始 'X'（非截断的中间字节）
        assert_eq!(buf[0], b'X', "淘汰后应以完整行起始，避免截断");
    }
}
