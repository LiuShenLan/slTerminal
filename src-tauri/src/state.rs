use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use tauri::{ipc::Channel, State};

use crate::error::AppError;
use crate::notify::pool::{LruWatcherPool, WATCHER_POOL_CAPACITY};
use crate::pty::spawn::PtyEvent;

/// PTY 会话 — 持有 master（读写/缩放）、子进程、writer 和 reader 线程句柄
pub struct PtySession {
    /// PTY master 端，用于 resize；Arc<Mutex<>> 包裹以支持跨线程访问（BE-01: pty_resize 在 spawn_blocking 内需要 clone）
    pub master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
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
    /// DA1 注入防重复标志（同一会话只注入一次 ESC[?64;22c 响应）
    pub da1_injected: Arc<AtomicBool>,
    /// Windows Job Object 句柄（孤儿防护，drop 时 CloseHandle）
    /// 非 Windows 平台为零大小占位类型
    pub job_object: Option<crate::pty::spawn::JobHandle>,
    /// SEC-08: 前端 panel ID，用于校验 pty_write/resize/kill 的调用方归属
    pub panel_id: String,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

/// PTY 全局状态 — HashMap 按 session_id 索引，加 spawn 串行锁
pub struct PtyState {
    /// session_id → PtySession
    pub sessions: RwLock<HashMap<String, PtySession>>,
    /// ConPTY spawn 串行化锁（Windows 并发 spawn 会卡死输出管道）
    /// BE-01: Arc 包装以支持 pty_spawn 在 spawn_blocking 内获取锁
    pub spawn_lock: Arc<Mutex<()>>,
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
            spawn_lock: Arc::new(Mutex::new(())),
        }
    }
}

/// git 仓库缓存 — workdir → Repository 的简易 LRU（容量 8，BE-09）
///
/// 原 HashMap 无上限无淘汰（注释「目录切换时清除」失实——已核实无任何清理点），
/// 现改容量上限 LRU，零新依赖手实现：HashMap 存值 + Vec<PathBuf> 维护访问顺序
/// （front = 最近使用 MRU，back = 最久未用 LRU，超容量淘汰尾部）。
///
/// 消费方（git/mod.rs get_or_open_repo）仅把缓存当「该 workdir 已被 discover 校验」
/// 的标记：命中后仍从磁盘 Repository::open 独立实例，故淘汰 Repository 值无资源泄漏。
pub const GIT_REPO_CACHE_CAPACITY: usize = 8;

pub struct GitRepoCache {
    map: HashMap<PathBuf, git2::Repository>,
    /// 访问顺序：front = 最近使用（MRU），back = 最久未用（LRU，淘汰对象）
    lru: Vec<PathBuf>,
    capacity: usize,
}

impl GitRepoCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            lru: Vec::new(),
            capacity,
        }
    }

    /// 前缀匹配查找：search 在某个缓存 workdir 子树内则命中（MRU→LRU 顺序，
    /// 命中即 touch 为 MRU；不含反向匹配，防子仓库误命中）。未命中返回 None。
    pub(crate) fn find_workdir(&mut self, search: &Path) -> Option<PathBuf> {
        let idx = self.lru.iter().position(|wd| search.starts_with(wd))?;
        let wd = self.lru.remove(idx);
        self.lru.insert(0, wd.clone());
        Some(wd)
    }

    /// 插入（同 key 替换值或新增）并 touch 为 MRU；超容量时淘汰 LRU 尾部
    pub(crate) fn insert(&mut self, key: PathBuf, repo: git2::Repository) {
        self.map.insert(key.clone(), repo);
        if let Some(idx) = self.lru.iter().position(|k| *k == key) {
            self.lru.remove(idx);
        }
        self.lru.insert(0, key);
        // 淘汰最久未用（尾部），直至回到容量内
        while self.lru.len() > self.capacity {
            if let Some(evicted) = self.lru.pop() {
                self.map.remove(&evicted);
            }
        }
    }

    /// 当前缓存条目数
    pub fn len(&self) -> usize {
        self.lru.len()
    }

    /// 缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.lru.is_empty()
    }
}

/// 应用全局状态，各模块通过 AppState 共享资源
pub struct AppState {
    pub pty: PtyState,
    /// 文件系统监听器池（按项目根路径缓存，容量见 WATCHER_POOL_CAPACITY，LRU 淘汰）
    pub file_watchers: Mutex<LruWatcherPool>,
    /// 当前项目根路径（由前端打开项目时设置，用于路径 sandbox 校验）
    pub project_root: RwLock<Option<PathBuf>>,
    /// set_project_root 串行化锁（SEC-16：A→B 快速切换时慢 canonicalize 的 A
    /// 不得后写回覆盖 B——整个 canonicalize+apply 过程互斥）
    pub project_root_lock: tokio::sync::Mutex<()>,
    /// git 仓库缓存：workdir → Repository，LRU 容量 GIT_REPO_CACHE_CAPACITY（BE-09）
    pub git_repo_cache: Mutex<GitRepoCache>,
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
            file_watchers: Mutex::new(LruWatcherPool::new(WATCHER_POOL_CAPACITY)),
            project_root: RwLock::new(None),
            project_root_lock: tokio::sync::Mutex::new(()),
            git_repo_cache: Mutex::new(GitRepoCache::new(GIT_REPO_CACHE_CAPACITY)),
        }
    }
}

/// 对路径做 canonicalize；若路径不存在则上溯到最近存在的祖先目录，
/// canonicalize 后再拼接不存在的剩余部分。
fn canonicalize_or_ancestor(target: &Path) -> std::io::Result<PathBuf> {
    match dunce::canonicalize(target) {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            // 上溯到最近存在的祖先
            let mut current = target.to_path_buf();
            let mut remainder: Vec<std::ffi::OsString> = Vec::new();
            while !current.exists() {
                if let Some(name) = current.file_name() {
                    remainder.push(name.to_os_string());
                }
                match current.parent() {
                    Some(parent) => current = parent.to_path_buf(),
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "无法定位目标路径的任何存在祖先",
                        ));
                    }
                }
            }
            // 此时 current 存在，对其 canonicalize
            let canonical_ancestor = dunce::canonicalize(&current)?;
            // 拼接剩余部分（逆序还原）
            let mut result = canonical_ancestor;
            for component in remainder.into_iter().rev() {
                result = result.join(component);
            }
            Ok(result)
        }
    }
}

/// 验证目标路径是否在项目根目录子树内（路径 sandbox）
///
/// 相对路径先以 project_root 为基准 join 成绝对路径，再 dunce::canonicalize。
/// 目标不存在时上溯到最近存在的祖先目录，canonicalize 后再拼接剩余部分做校验。
/// project_root 未设置时拒绝（#[cfg(test)] 豁免，避免每个测试都需设置 project_root）。
pub fn validate_path_within_root(
    root_opt: &Option<PathBuf>,
    target: &Path,
) -> Result<(), AppError> {
    let root = match root_opt {
        Some(r) => r,
        None => {
            // 测试豁免：project_root 未设置时放行
            if cfg!(test) {
                return Ok(());
            }
            return Err(AppError::IoKind {
                kind: "path".into(),
                message: "项目根路径未设置，拒绝文件访问".into(),
            });
        }
    };
    let canonical_root = dunce::canonicalize(root).map_err(|e| AppError::IoKind {
        kind: "path".into(),
        message: format!("无法解析项目根路径: {e}"),
    })?;

    // 相对路径先以 project_root 为基准 join 成绝对路径
    let target = if target.is_relative() {
        canonical_root.join(target)
    } else {
        target.to_path_buf()
    };

    // canonicalize 目标；不存在时上溯到最近存在的祖先再拼接
    let canonical_target = canonicalize_or_ancestor(&target).map_err(|_| AppError::IoKind {
        kind: "path".into(),
        message: "目标路径不在项目范围内或无法解析".into(),
    })?;

    if !canonical_target.starts_with(&canonical_root) {
        return Err(AppError::IoKind {
            kind: "path".into(),
            message: "路径超出项目范围".into(),
        });
    }
    Ok(())
}

/// 设置当前项目根路径（由前端打开项目时调用）
///
/// canonicalize 后写入 AppState.project_root，用于后续文件操作的路径 sandbox 校验。
/// BE-04: 异步化——canonicalize 为磁盘 I/O，在 spawn_blocking 中执行，不阻塞 IPC worker。
/// SEC-14: canonicalize 失败/目录不可读 → 返回 Err 且清空旧 root（防沙箱误放行旧路径）。
#[tauri::command]
pub async fn set_project_root(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    set_project_root_impl(&state.project_root, &state.project_root_lock, path).await
}

/// set_project_root 命令内核（BE-04/SEC-14，供 L1 测试直接调用，无需构造 tauri::State）
async fn set_project_root_impl(
    project_root: &RwLock<Option<PathBuf>>,
    lock: &tokio::sync::Mutex<()>,
    path: String,
) -> Result<(), AppError> {
    // SEC-16: 持锁至函数尾——canonicalize 与 apply 全程互斥，
    // A→B 快速切换时慢 canonicalize 的 A 不得在 B 写入后再后写回覆盖 B
    let _guard = lock.lock().await;
    // BE-04: canonicalize 在 spawn_blocking 中执行（磁盘 I/O 不占 IPC worker）
    let canonical = match tokio::task::spawn_blocking(move || -> Result<PathBuf, AppError> {
        dunce::canonicalize(Path::new(&path)).map_err(|e| AppError::IoKind {
            kind: "path".into(),
            message: format!("无法解析项目路径: {e}"),
        })
    })
    .await
    {
        Ok(inner) => inner,
        // 闭包 panic 等 join 失败同样视为失败路径（SEC-14: 清空旧 root）
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    };

    apply_project_root(project_root, canonical)
}

/// 应用 canonicalize 结果（SEC-14 核心逻辑）：
/// 成功 → 写入新 root；失败 → 返回 Err 且清空旧 root（防沙箱误放行旧路径）
fn apply_project_root(
    project_root: &RwLock<Option<PathBuf>>,
    canonical: Result<PathBuf, AppError>,
) -> Result<(), AppError> {
    let canonical = match canonical {
        Ok(c) => c,
        Err(e) => {
            // SEC-14: 失败时清空旧 root，防止沙箱继续放行已失效的旧路径
            match project_root.write() {
                Ok(mut root) => *root = None,
                // BE-24：锁中毒时旧 root 无法清空——接受语义偏差但可观测化（登记见 src-tauri/CLAUDE.md）
                Err(lock_err) => {
                    tracing::warn!("project_root 写锁中毒，旧 root 未能清空: {lock_err}");
                }
            }
            return Err(e);
        }
    };
    let mut root = project_root.write().map_err(|e| AppError::IoKind {
        kind: "lock".into(),
        message: format!("获取 project_root 锁失败: {e}"),
    })?;
    *root = Some(canonical);
    Ok(())
}

/// ring buffer 最大容量（256KB，保留最近约 4000+ 行终端输出）
const RING_BUFFER_CAPACITY: usize = 262144; // 256KB

/// 向 ring buffer 追加数据，超过容量时从头部丢弃旧数据
/// P2-47: 淘汰到 \\n 边界，避免行内 UTF-8 序列截断
pub fn ring_buffer_append(ring: &Mutex<VecDeque<u8>>, data: &[u8]) -> Result<(), AppError> {
    let mut buf = ring
        .lock()
        .map_err(|e| AppError::Pty(format!("锁获取失败: {e}")))?;
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
mod state_tests {
    use super::*;

    #[test]
    fn pty_state_new_empty() {
        let pty = PtyState::new();
        assert!(
            pty.sessions.read().unwrap().is_empty(),
            "新建 PtyState 的 sessions 应为空"
        );
    }

    #[test]
    fn app_state_new() {
        let state = AppState::new();
        assert!(
            state.pty.sessions.read().unwrap().is_empty(),
            "AppState::new() 应成功创建并持有空的 PtyState"
        );
        assert!(
            state.file_watchers.lock().unwrap().is_empty(),
            "AppState::new() 初始时 file_watchers 池应为空"
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
    fn ring_buffer_append_fifo() {
        let ring = Mutex::new(VecDeque::new());
        let data: Vec<u8> = (0..255).collect(); // 255 bytes
        ring_buffer_append(&ring, &data).unwrap();
        let buf = ring.lock().unwrap();
        assert_eq!(buf.len(), 255);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn ring_buffer_eviction() {
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
    fn ring_buffer_eviction_at_newline_boundary() {
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

    /// PTY-05: 无换行长行淘汰边界①——淘汰量恰好 1024（map_or 的 or 分支原量淘汰）
    #[test]
    fn ring_buffer_eviction_long_line_exact_1024() {
        let ring = Mutex::new(VecDeque::new());
        // 单条无换行超长行：容量 + 恰好 1024 字节
        // 1024 字节窗口内无换行 → drain_len = drain_target = 1024，一轮淘汰后恰好回落到容量
        let data = vec![b'A'; RING_BUFFER_CAPACITY + 1024];
        ring_buffer_append(&ring, &data).unwrap();
        let buf = ring.lock().unwrap();
        assert_eq!(
            buf.len(),
            RING_BUFFER_CAPACITY,
            "淘汰量恰好 1024 时应恰好回落到容量"
        );
        assert_eq!(buf[buf.len() - 1], b'A', "剩余尾部应为最新写入字节");
    }

    /// PTY-05: 无换行长行淘汰边界②——超 1024 且不能整除（多轮原量淘汰）
    #[test]
    fn ring_buffer_eviction_long_line_exceed_1024() {
        let ring = Mutex::new(VecDeque::new());
        // 单条无换行超长行：容量 + 5000 字节（5000 = 4×1024 + 904）
        // 无换行时每轮按 1024 原量淘汰：4 轮后剩 904 仍超容量 → 第 5 轮再淘汰 1024，回落至容量 - 120
        let data = vec![b'B'; RING_BUFFER_CAPACITY + 5000];
        ring_buffer_append(&ring, &data).unwrap();
        let buf = ring.lock().unwrap();
        assert!(
            buf.len() <= RING_BUFFER_CAPACITY,
            "淘汰后长度不应超过容量，实际 {}",
            buf.len()
        );
        assert_eq!(
            buf.len(),
            RING_BUFFER_CAPACITY - 120,
            "无换行超长行应按 1024 原量多轮淘汰"
        );
        assert_eq!(buf[buf.len() - 1], b'B', "剩余尾部应为最新写入字节");
    }

    /// PTY-05: 无换行长行淘汰边界③——数据中含换行（rposition 分支按行对齐，超长行不截断）
    #[test]
    fn ring_buffer_eviction_long_line_with_newline() {
        let ring = Mutex::new(VecDeque::new());
        // 先铺满短行（101 字节/行：100 字节 X + 换行），再追加一条 3000 字节无换行超长行
        // 淘汰窗口（1024 字节）内存在换行 → drain_len = 最后一个 \n 位置 + 1（10 整行 1010 字节）
        // 超长行整体保留在尾部，不被截断
        let line = [b'X'; 100];
        let mut total = 0usize;
        while total + 101 <= RING_BUFFER_CAPACITY {
            ring_buffer_append(&ring, &line).unwrap();
            ring_buffer_append(&ring, b"\n").unwrap();
            total += 101;
        }
        let long_line = vec![b'Y'; 3000];
        ring_buffer_append(&ring, &long_line).unwrap();
        let buf = ring.lock().unwrap();
        assert!(
            buf.len() <= RING_BUFFER_CAPACITY,
            "淘汰后长度不应超过容量，实际 {}",
            buf.len()
        );
        assert_eq!(buf[0], b'X', "淘汰后应以完整行起始（行边界对齐）");
        // 尾部无换行超长行应完整保留（未被截断）
        let mut tail = buf.iter().skip(buf.len() - 3000);
        assert!(tail.all(|&b| b == b'Y'), "尾部无换行超长行应完整保留");
    }
}

/// validate_path_within_root 路径沙箱测试
#[cfg(test)]
mod sandbox_tests {
    use super::*;

    /// root_opt 为 None → 跳过校验，返回 Ok
    #[test]
    fn validate_root_none_allows_any_path() {
        let result = validate_path_within_root(&None, std::path::Path::new("C:\\any\\path"));
        assert!(result.is_ok(), "root_opt=None 应放行任意路径");
    }

    /// 路径在根内 → 返回 Ok
    #[test]
    fn validate_path_inside_root() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("inside.txt");
        std::fs::write(&child, "ok").unwrap();

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &child);
        assert!(result.is_ok(), "根内路径应放行");
    }

    /// 子目录路径在根内 → 返回 Ok
    #[test]
    fn validate_subdir_inside_root() {
        let root = tempfile::tempdir().unwrap();
        let subdir = root.path().join("sub").join("deep");
        std::fs::create_dir_all(&subdir).unwrap();
        let file = subdir.join("test.txt");
        std::fs::write(&file, "ok").unwrap();

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &file);
        assert!(result.is_ok(), "子目录内文件应放行");
    }

    /// 路径在根外 → 返回 Err
    #[test]
    fn validate_path_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("outside.txt");
        std::fs::write(&file, "bad").unwrap();

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &file);
        assert!(result.is_err(), "根外路径应拒绝");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("超出项目范围"), "错误消息应含'超出项目范围'");
    }

    /// 目标路径不存在（根内） → 上溯到最近存在的祖先后放行
    #[test]
    fn validate_nonexistent_path_accepted() {
        let root = tempfile::tempdir().unwrap();
        // 创建一个已存在的父目录，再指向其中不存在的文件
        let parent = root.path().join("existing_parent");
        std::fs::create_dir(&parent).unwrap();
        let nonexistent = parent.join("not_created_yet.txt");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &nonexistent);
        assert!(result.is_ok(), "根内不存在的路径应上溯到祖先后放行");
    }

    /// 不存在的路径在根外 → 沿父级上溯后仍在根外，应拒绝
    #[test]
    fn validate_nonexistent_path_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let nonexistent = outside.path().join("not_exists.txt");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &nonexistent);
        assert!(result.is_err(), "根外不存在的路径应拒绝");
    }

    /// 路径穿越攻击（../ 逃逸） → 应拒绝
    #[test]
    fn validate_path_traversal_rejected() {
        let root = tempfile::tempdir().unwrap();
        // 构造 root/sub/../outside → 应该在 canonicalize 后逃出 root
        let subdir = root.path().join("sub");
        std::fs::create_dir(&subdir).unwrap();
        let traversal = subdir.join("..").join("..").join("Windows");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &traversal);
        // ../ 逃逸经 canonicalize 后应解析到根外路径
        assert!(result.is_err(), "路径穿越应拒绝");
    }

    /// PTY-11: 相对路径含 .. 穿越沙箱根 → 应以根为基准 join 后拒绝（SEC-01 防线）
    #[test]
    fn validate_relative_path_traversal_rejected() {
        let root = tempfile::tempdir().unwrap();
        // 相对路径 ".." 以 canonical_root 为基准 join → canonicalize 后解析到根外
        let traversal = std::path::Path::new("..").join("sltest_escape_pty11.txt");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &traversal);
        assert!(result.is_err(), "相对路径 .. 穿越应拒绝");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("超出项目范围"), "错误消息应含'超出项目范围'");
    }

    /// PTY-11: 相对路径正常放行 → 以根为基准 join 后放行
    #[test]
    fn validate_relative_path_accepted() {
        let root = tempfile::tempdir().unwrap();
        // 相对路径（含不存在的中层目录）以 canonical_root 为基准 join → 上溯到根后放行
        let relative = std::path::Path::new("sub_dir").join("rel.txt");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &relative);
        assert!(result.is_ok(), "根内相对路径应放行");
    }

    /// 符号链接指向根外 → 应拒绝
    #[cfg(windows)]
    #[test]
    fn validate_symlink_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("target.txt");
        std::fs::write(&outside_file, "data").unwrap();

        let link = root.path().join("link.txt");
        // Windows 符号链接（需管理员权限，优雅降级）
        let symlink_result = std::os::windows::fs::symlink_file(&outside_file, &link);
        if symlink_result.is_err() {
            // 无权限创建符号链接时跳过测试
            return;
        }

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &link);
        assert!(result.is_err(), "指向根外的符号链接应拒绝");
    }

    /// root 本身是 symlink → 应仍能正确校验
    #[cfg(windows)]
    #[test]
    fn validate_root_is_symlink() {
        let original_root = tempfile::tempdir().unwrap();
        let file = original_root.path().join("data.txt");
        std::fs::write(&file, "ok").unwrap();

        // 创建到 root 的 symlink
        let link_parent = tempfile::tempdir().unwrap();
        let link_root = link_parent.path().join("linked_root");
        let symlink_result = std::os::windows::fs::symlink_dir(&original_root, &link_root);
        if symlink_result.is_err() {
            return; // 无权限，跳过
        }

        let linked_file = link_root.join("data.txt");
        let result = validate_path_within_root(&Some(link_root), &linked_file);
        assert!(result.is_ok(), "symlink 根内路径应放行");
    }

    /// 根路径自身 canonicalize 失败 → 返回 Err
    #[test]
    fn validate_root_canonicalize_fails() {
        let result = validate_path_within_root(
            &Some(std::path::PathBuf::from("Z:\\nonexistent_drive\\root")),
            std::path::Path::new("Z:\\nonexistent_drive\\root\\file.txt"),
        );
        assert!(result.is_err(), "根 canonicalize 失败应返回 Err");
    }

    // ---- canonicalize_or_ancestor 纯函数测试 ----

    /// 已存在文件直接 canonicalize
    #[test]
    fn canonicalize_or_ancestor_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("real.txt");
        std::fs::write(&file, "data").unwrap();

        let result = canonicalize_or_ancestor(&file).unwrap();
        let expected = dunce::canonicalize(&file).unwrap();
        assert_eq!(result, expected, "已存在文件应直接 canonicalize");
    }

    /// 仅叶子不存在 → 上溯到父目录后拼接
    #[test]
    fn canonicalize_or_ancestor_one_level_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        let missing = dir.path().join("sub").join("ghost.txt");

        let result = canonicalize_or_ancestor(&missing).unwrap();
        // 结果应在 dir 子树内
        let canonical_dir = dunce::canonicalize(dir.path()).unwrap();
        assert!(
            result.starts_with(&canonical_dir),
            "拼接后路径应在 root 内，实际: {result:?}"
        );
        assert!(
            result.ends_with("ghost.txt"),
            "应以缺失文件名结尾，实际: {result:?}"
        );
    }

    /// 多层不存在 → 上溯到最近存在祖先后逐层拼接
    #[test]
    fn canonicalize_or_ancestor_deep_missing() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("base");
        std::fs::create_dir(&existing).unwrap();
        let deep_missing = existing.join("a").join("b").join("c").join("deep.txt");

        let result = canonicalize_or_ancestor(&deep_missing).unwrap();
        assert!(result.ends_with("deep.txt"), "应以文件名结尾");
        // 中间路径组件应保留（b/c/deep.txt 的祖先是 base → canonicalize 后拼接 a/b/c/deep.txt）
        let result_str = result.to_string_lossy();
        assert!(result_str.contains("a"), "应保留中间层 a");
        assert!(result_str.contains("b"), "应保留中间层 b");
    }

    /// root 本身就是最近存在祖先
    #[test]
    fn canonicalize_or_ancestor_root_itself_is_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("orphan.txt");

        let result = canonicalize_or_ancestor(&missing).unwrap();
        let canonical_root = dunce::canonicalize(dir.path()).unwrap();
        assert_eq!(result, canonical_root.join("orphan.txt"));
    }

    /// 路径包含 .. 逃逸 → 上溯祖先后 canonicalize 解析真实位置
    #[test]
    fn canonicalize_or_ancestor_path_traversal_via_dotdot() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        // root/sub 存在
        std::fs::create_dir_all(root.path().join("sub")).unwrap();
        // 构造: root/sub/../../outside_tempdir/nonexistent.txt
        // 上溯应找到 outside_tempdir（存在，但在 root 外）
        let traversal = root
            .path()
            .join("sub")
            .join("..")
            .join("..")
            .join(outside.path().file_name().unwrap())
            .join("nonexistent.txt");

        let result = canonicalize_or_ancestor(&traversal).unwrap();
        // 最近存在祖先是 outside tempdir → canonicalize 后应解析到根外
        let canonical_root = dunce::canonicalize(root.path()).unwrap();
        assert!(
            !result.starts_with(&canonical_root),
            ".. 逃逸应使结果逃出 root，实际: {result:?}"
        );
    }

    /// 祖先有符号链接 → canonicalize 解析 symlink 真实指向
    #[cfg(windows)]
    #[test]
    fn canonicalize_or_ancestor_symlink_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        // 在 root 内创建 symlink 指向外部目录
        let link = root.path().join("link_to_outside");
        let symlink_result = std::os::windows::fs::symlink_dir(&outside, &link);
        if symlink_result.is_err() {
            return; // 无权限，跳过
        }
        // link/exists.txt 存在，link/ghost.txt 不存在
        std::fs::write(link.join("exists.txt"), "ok").unwrap();
        let missing = link.join("ghost.txt");

        let result = canonicalize_or_ancestor(&missing).unwrap();
        // 上溯祖先 link.join("ghost.txt") → ghost.txt 不存在 → parent = link
        // link 存在 → canonicalize → 解析到 outside 目录
        // 再 join "ghost.txt" → outside/ghost.txt
        let canonical_outside = dunce::canonicalize(outside.path()).unwrap();
        assert_eq!(result, canonical_outside.join("ghost.txt"));
    }

    /// 所有祖先都不存在 → 返回 Err
    #[test]
    fn canonicalize_or_ancestor_no_ancestor_exists() {
        let nonexistent = std::path::Path::new("Z:\\not_a_real_drive\\a\\b\\c.txt");
        let result = canonicalize_or_ancestor(nonexistent);
        assert!(result.is_err(), "无存在祖先时应返回 Err");
    }

    // ---- canonicalize_or_ancestor 相对路径分支（PTY-13②）----
    // 相对路径以进程 cwd 为基准解析；测试内 chdir 到 tempdir 构造场景，测毕恢复原 cwd

    /// 直接传相对路径（文件存在）→ 以 cwd 为基准 canonicalize
    #[test]
    fn canonicalize_or_ancestor_relative_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rel.txt"), "data").unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = canonicalize_or_ancestor(std::path::Path::new("rel.txt"));
        std::env::set_current_dir(original).unwrap();

        let result = result.unwrap();
        let expected = dunce::canonicalize(dir.path().join("rel.txt")).unwrap();
        assert_eq!(
            result, expected,
            "相对路径存在时应解析为 cwd 基准下的真实路径"
        );
    }

    /// 直接传相对路径（叶子缺失、最近存在祖先为 cwd 下的子目录）→ 上溯到子目录后拼接
    #[test]
    fn canonicalize_or_ancestor_relative_missing_leaf_with_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = canonicalize_or_ancestor(&std::path::Path::new("sub").join("ghost.txt"));
        std::env::set_current_dir(original).unwrap();

        let result = result.unwrap();
        let expected = dunce::canonicalize(dir.path().join("sub"))
            .unwrap()
            .join("ghost.txt");
        assert_eq!(result, expected, "应上溯到存在的子目录后拼接缺失叶子");
    }

    /// 直接传相对路径（叶子缺失且无存在祖先——上溯到空路径后终止）→ 返回 Err，不 panic
    #[test]
    fn canonicalize_or_ancestor_relative_no_ancestor_exists() {
        let dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = canonicalize_or_ancestor(std::path::Path::new("ghost_only.txt"));
        std::env::set_current_dir(original).unwrap();

        assert!(result.is_err(), "相对路径叶子缺失且无存在祖先时应返回 Err");
    }

    // ---- validate_path_within_root 新语义测试 ----

    /// 多层不存在的目录结构在根内 → 放行
    #[test]
    fn validate_nonexistent_deep_path_accepted() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("src");
        std::fs::create_dir(&existing).unwrap();
        let deep = existing
            .join("components")
            .join("ui")
            .join("NewFeature.tsx");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &deep);
        assert!(result.is_ok(), "根内多层不存在路径应放行");
    }

    /// 不存在的路径含 .. 穿越到根外 → 拒绝
    #[test]
    fn validate_nonexistent_path_traversal_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        // root/sub 存在
        std::fs::create_dir_all(root.path().join("sub")).unwrap();
        // root/sub/../../outside_tempdir/nonexistent.txt
        let traversal = root
            .path()
            .join("sub")
            .join("..")
            .join("..")
            .join(outside.path().file_name().unwrap())
            .join("ghost.txt");

        let result = validate_path_within_root(&Some(root.path().to_path_buf()), &traversal);
        assert!(result.is_err(), ".. 逃逸的不存在路径应拒绝");
    }
}

/// set_project_root 命令测试（BE-04 异步化 + SEC-14 失败清空旧 root）
#[cfg(test)]
mod project_root_tests {
    use super::*;

    /// BE-04: 异步化后成功路径行为不变——canonicalize 后写入 project_root
    #[test]
    fn set_project_root_success_sets_canonical_root() {
        let app_state = AppState::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                path,
            ))
            .unwrap();

        let root = app_state.project_root.read().unwrap().clone().unwrap();
        let expected = dunce::canonicalize(dir.path()).unwrap();
        assert_eq!(
            root, expected,
            "成功时 project_root 应写入 canonicalize 后的路径"
        );
    }

    /// SEC-14: 失败路径——构造不存在路径调用，返回 Err 且清空旧 root
    #[test]
    fn set_project_root_failure_clears_old_root() {
        let app_state = AppState::new();
        let dir = tempfile::tempdir().unwrap();

        // 先设置有效 root
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                dir.path().to_string_lossy().to_string(),
            ))
            .unwrap();
        assert!(
            app_state.project_root.read().unwrap().is_some(),
            "前置：旧 root 应存在"
        );

        // 构造不存在路径（父目录亦不存在）→ canonicalize 失败 → Err 且清空旧 root
        let nonexistent = dir
            .path()
            .join("no_such_dir")
            .join("deeper")
            .to_string_lossy()
            .to_string();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                nonexistent,
            ));

        assert!(result.is_err(), "不存在路径应返回 Err");
        assert!(
            app_state.project_root.read().unwrap().is_none(),
            "失败后旧 root 应被清空（防沙箱误放行旧路径）"
        );
    }

    /// SEC-14: 失败时保留原始错误信息，不吞错
    #[test]
    fn set_project_root_failure_preserves_error_message() {
        let app_state = AppState::new();
        let nonexistent = "Z:\\definitely_not_a_drive_xyz\\no_such_dir".to_string();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                nonexistent,
            ));

        assert!(result.is_err(), "不存在路径应返回 Err");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("无法解析项目路径"), "错误消息应保留原错误信息");
    }

    /// SEC-14: 失败清空对后续成功设置无影响——可再次成功设置新 root
    #[test]
    fn set_project_root_recovers_after_failure() {
        let app_state = AppState::new();
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        std::fs::create_dir(&first).unwrap();

        // 失败一次（清空旧 root）
        let nonexistent = dir.path().join("no_such_dir").to_string_lossy().to_string();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                nonexistent,
            ))
            .unwrap_err();

        // 再次成功设置
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &tokio::sync::Mutex::new(()),
                first.to_string_lossy().to_string(),
            ))
            .unwrap();
        let root = app_state.project_root.read().unwrap().clone().unwrap();
        assert_eq!(
            root,
            dunce::canonicalize(&first).unwrap(),
            "失败清空后应可重新成功设置"
        );
    }

    /// SEC-16: 并发 set_project_root_impl 串行化——A→B 快速切换时慢 canonicalize 的 A
    /// 不得后写回覆盖 B（Mutex 保证 canonicalize+apply 全程互斥）；
    /// 两调用均 Ok、最终 root 为 A/B 之一且非 None、顺序调用 B 后 root == B
    #[test]
    fn set_project_root_serializes_concurrent_calls() {
        let app_state = AppState::new();
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let path_a = dir_a.path().to_string_lossy().to_string();
        let path_b = dir_b.path().to_string_lossy().to_string();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let (result_a, result_b) = rt.block_on(async {
            tokio::join!(
                set_project_root_impl(
                    &app_state.project_root,
                    &app_state.project_root_lock,
                    path_a,
                ),
                set_project_root_impl(
                    &app_state.project_root,
                    &app_state.project_root_lock,
                    // path_b 传 clone——String 非 Copy，join! 分支 move 后末尾顺序调用仍需再用
                    path_b.clone(),
                ),
            )
        });

        assert!(result_a.is_ok(), "并发 A 调用应 Ok");
        assert!(result_b.is_ok(), "并发 B 调用应 Ok");
        // 串行化保证 root 为 A/B 之一（非交错写回产物），且非 None
        let root = app_state.project_root.read().unwrap().clone().unwrap();
        let canonical_a = dunce::canonicalize(dir_a.path()).unwrap();
        let canonical_b = dunce::canonicalize(dir_b.path()).unwrap();
        assert!(
            root == canonical_a || root == canonical_b,
            "并发后 root 应为 A/B 之一，实际: {root:?}"
        );

        // 再顺序调用 B → root 稳定为 B
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_project_root_impl(
                &app_state.project_root,
                &app_state.project_root_lock,
                path_b,
            ))
            .unwrap();
        let root = app_state.project_root.read().unwrap().clone().unwrap();
        assert_eq!(root, canonical_b, "顺序调用 B 后 root 应为 B");
    }
}

/// GitRepoCache LRU 测试（BE-09：容量淘汰、命中复用、同 key 替换）
#[cfg(test)]
mod git_repo_cache_tests {
    use super::*;
    use tempfile::tempdir;

    /// 构造一个真实 git 仓库实例（git2::Repository::init，无需外部 git CLI）
    fn make_repo() -> git2::Repository {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap()
    }

    /// 容量淘汰：超容量插入时淘汰最久未用（LRU 尾部）项
    #[test]
    fn evicts_oldest_when_over_capacity() {
        let mut cache = GitRepoCache::new(2);
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let dir_c = tempdir().unwrap();
        cache.insert(dir_a.path().to_path_buf(), make_repo());
        cache.insert(dir_b.path().to_path_buf(), make_repo());
        assert_eq!(cache.len(), 2);

        // 插入第三个 → 淘汰最久未用的 A（B 次新、C 最新）
        cache.insert(dir_c.path().to_path_buf(), make_repo());
        assert_eq!(cache.len(), 2, "超容量后应回落到容量内");
        assert!(
            cache.find_workdir(dir_a.path()).is_none(),
            "最久未用的 A 应被淘汰"
        );
        assert!(cache.find_workdir(dir_b.path()).is_some());
        assert!(cache.find_workdir(dir_c.path()).is_some());
    }

    /// 命中复用：find_workdir 命中后 touch 为 MRU，后续淘汰跳过它
    #[test]
    fn hit_touches_recently_used() {
        let mut cache = GitRepoCache::new(2);
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let dir_c = tempdir().unwrap();
        cache.insert(dir_a.path().to_path_buf(), make_repo());
        cache.insert(dir_b.path().to_path_buf(), make_repo());

        // 访问 A → A 变 MRU（B 成 LRU）
        assert_eq!(
            cache.find_workdir(dir_a.path()).unwrap(),
            dir_a.path().to_path_buf()
        );

        // 插入 C → 应淘汰 B（A 已被 touch 保留）
        cache.insert(dir_c.path().to_path_buf(), make_repo());
        assert!(cache.find_workdir(dir_b.path()).is_none(), "B 应被淘汰");
        assert!(cache.find_workdir(dir_a.path()).is_some(), "A 应保留");
        assert!(cache.find_workdir(dir_c.path()).is_some());
    }

    /// 同 key 再次 insert：替换值并 touch 为 MRU（条目数不增）
    #[test]
    fn insert_existing_key_replaces_and_touches() {
        let mut cache = GitRepoCache::new(2);
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let dir_c = tempdir().unwrap();
        cache.insert(dir_a.path().to_path_buf(), make_repo());
        cache.insert(dir_b.path().to_path_buf(), make_repo());

        // 同 key 再插入 → touch 为 MRU，条目数不变
        cache.insert(dir_a.path().to_path_buf(), make_repo());
        assert_eq!(cache.len(), 2, "同 key 插入不应增加条目");

        // 插入 C → 淘汰 B（A 已 touch）
        cache.insert(dir_c.path().to_path_buf(), make_repo());
        assert!(cache.find_workdir(dir_b.path()).is_none(), "B 应被淘汰");
        assert!(cache.find_workdir(dir_a.path()).is_some(), "A 应保留");
    }

    /// 前缀匹配：search 在缓存 workdir 子树内命中；子树外不命中
    #[test]
    fn find_only_matches_subtree_prefix() {
        let mut cache = GitRepoCache::new(GIT_REPO_CACHE_CAPACITY);
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        cache.insert(dir_a.path().to_path_buf(), make_repo());
        cache.insert(dir_b.path().to_path_buf(), make_repo());

        // 深层路径（workdir 子树内）命中
        let deep = dir_a.path().join("sub").join("deep").join("file.txt");
        assert_eq!(
            cache.find_workdir(&deep).unwrap(),
            dir_a.path().to_path_buf(),
            "子树内路径应命中对应 workdir"
        );
        // 目录本身命中
        assert!(cache.find_workdir(dir_a.path()).is_some());
        // 完全无关路径不命中
        let outside = tempdir().unwrap();
        assert!(
            cache.find_workdir(outside.path()).is_none(),
            "无关路径不应命中"
        );
    }

    /// 空缓存：find 返回 None，len/is_empty 正确
    #[test]
    fn empty_cache_find_returns_none() {
        let mut cache = GitRepoCache::new(GIT_REPO_CACHE_CAPACITY);
        let dir = tempdir().unwrap();
        assert!(cache.find_workdir(dir.path()).is_none());
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    /// 容量常量契约：GIT_REPO_CACHE_CAPACITY = 8（BE-09 跨边界契约）
    #[test]
    fn cache_capacity_contract_is_eight() {
        assert_eq!(GIT_REPO_CACHE_CAPACITY, 8);
    }
}
