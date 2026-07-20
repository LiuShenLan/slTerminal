use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use tauri::{ipc::Channel, State};

use crate::error::AppError;
use crate::notify::pool::LruWatcherPool;
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

/// 应用全局状态，各模块通过 AppState 共享资源
pub struct AppState {
    pub pty: PtyState,
    /// 文件系统监听器池（按项目根路径缓存，最多 5 个，LRU 淘汰）
    pub file_watchers: Mutex<LruWatcherPool>,
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
            file_watchers: Mutex::new(LruWatcherPool::new(5)),
            project_root: RwLock::new(None),
            git_repo_cache: Mutex::new(HashMap::new()),
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
pub fn validate_path_within_root(root_opt: &Option<PathBuf>, target: &Path) -> Result<(), AppError> {
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
#[tauri::command]
pub fn set_project_root(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let canonical = dunce::canonicalize(Path::new(&path)).map_err(|e| AppError::IoKind {
        kind: "path".into(),
        message: format!("无法解析项目路径: {e}"),
    })?;
    let mut root = state.project_root.write().map_err(|e| AppError::IoKind {
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &child,
        );
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &file,
        );
        assert!(result.is_ok(), "子目录内文件应放行");
    }

    /// 路径在根外 → 返回 Err
    #[test]
    fn validate_path_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("outside.txt");
        std::fs::write(&file, "bad").unwrap();

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &file,
        );
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &nonexistent,
        );
        assert!(result.is_ok(), "根内不存在的路径应上溯到祖先后放行");
    }

    /// 不存在的路径在根外 → 沿父级上溯后仍在根外，应拒绝
    #[test]
    fn validate_nonexistent_path_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let nonexistent = outside.path().join("not_exists.txt");

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &nonexistent,
        );
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &traversal,
        );
        // ../ 逃逸经 canonicalize 后应解析到根外路径
        assert!(result.is_err(), "路径穿越应拒绝");
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &link,
        );
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
        let result = validate_path_within_root(
            &Some(link_root),
            &linked_file,
        );
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

    // ---- validate_path_within_root 新语义测试 ----

    /// 多层不存在的目录结构在根内 → 放行
    #[test]
    fn validate_nonexistent_deep_path_accepted() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("src");
        std::fs::create_dir(&existing).unwrap();
        let deep = existing.join("components").join("ui").join("NewFeature.tsx");

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &deep,
        );
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

        let result = validate_path_within_root(
            &Some(root.path().to_path_buf()),
            &traversal,
        );
        assert!(result.is_err(), ".. 逃逸的不存在路径应拒绝");
    }
}
