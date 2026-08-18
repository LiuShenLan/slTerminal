/// 文件系统模块 — 文件读/写命令
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
use crate::error::{io_error, AppError};
use crate::state::validate_path_within_root;
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::ipc::Channel;
use tauri::State;

/// CRLF 检测样本最大字节数（取前 64KB 判定原文件行尾风格）
const CRLF_SAMPLE_MAX_BYTES: usize = 65536;

/// 目录条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    /// 文件/目录名
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件大小（字节），仅文件时有值
    pub size: Option<u64>,
    /// 最后修改时间（Unix 毫秒），仅文件时有值
    pub modified: Option<u64>,
}

/// 在 spawn_blocking 中执行阻塞任务，统一将 JoinError（含闭包 panic）映射为 AppError::TaskJoin
async fn spawn_blocking_task<F, R>(f: F) -> Result<R, AppError>
where
    F: FnOnce() -> Result<R, AppError> + Send + 'static,
    R: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 从 State 提取 project_root（仅命令包装层使用——内核直接接收 root，测试无需构造 State）
fn extract_root(state: &State<'_, AppState>) -> Result<Option<PathBuf>, AppError> {
    // 锁错误为内部不变量错误：用户可见消息保留语义，poisoning 技术细节进 tracing（BE-15）
    let guard = state.project_root.read().map_err(|e| {
        tracing::warn!(error = %e, "获取 project_root 读锁失败");
        AppError::IoKind {
            kind: "lock".into(),
            message: "获取 project_root 锁失败".into(),
        }
    })?;
    Ok((*guard).clone())
}

/// 文件分块读取块大小（256KB）——控制单块内存与 IPC 峰值（BE-03）
const READ_CHUNK_BYTES: usize = 256 * 1024;

/// 文件读取大小上限（10MB）——超限拒绝，保护内存（BE-03 由前端校验移至后端）
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// 文件分块读取块（Channel 推送，camelCase 与前端对齐）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadChunk {
    /// 分块数据（UTF-8 文本，多字节字符跨块不切散）
    pub data: String,
    /// 是否终态（终态 data 恒为空串，表示发送序列结束）
    pub done: bool,
}

/// 读取文件内容（UTF-8 文本，Channel 分块推送）
///
/// 先 metadata 校验大小 ≤10MB（超限 Err），再按 256KB 分块读取推送。
/// 发送序列 = 若干 {data, done:false} + 终态 {data:"", done:true}。
#[tauri::command]
pub async fn fs_read_file(
    path: String,
    on_chunk: Channel<FsReadChunk>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // State 仅做提取，业务逻辑在 fs_read_file_impl（测试直接调内核）
    fs_read_file_impl(
        path,
        move |chunk| {
            on_chunk.send(chunk).map_err(|e| {
                tracing::warn!(error = %e, "fs_read_file 分块推送失败");
                AppError::IoKind {
                    kind: "ipc".into(),
                    message: "读取文件失败".into(),
                }
            })
        },
        extract_root(&state)?,
    )
    .await
}

/// fs_read_file 命令内核：路径 sandbox 校验 + 大小上限校验 + 分块读取推送
///
/// 分块经 send 回调推送——tauri::ipc::Channel 无法在 L1 构造（无 webview 上下文），
/// send 回调使内核可测（HFN-08 先例：内核直接接收依赖而非 State）。
async fn fs_read_file_impl<F>(path: String, send: F, root: Option<PathBuf>) -> Result<(), AppError>
where
    F: FnMut(FsReadChunk) -> Result<(), AppError> + Send + 'static,
{
    // 路径 sandbox 校验
    validate_path_within_root(&root, Path::new(&path))?;

    spawn_blocking_task(move || read_file_chunked(&path, send)).await
}

/// 分块读取文件并通过 send 推送（块 256KB，UTF-8 边界回退）
///
/// 发送序列 = 若干 {data, done:false} + 终态 {data:"", done:true}；
/// 空文件直接发终态；文件含非法/残缺 UTF-8 → Err（与旧 read_to_string 行为一致）。
fn read_file_chunked<F>(path: &str, mut send: F) -> Result<(), AppError>
where
    F: FnMut(FsReadChunk) -> Result<(), AppError>,
{
    // 先 metadata 校验大小上限——超限 Err，避免大文件全量读入
    let meta = std::fs::metadata(path).map_err(|e| io_error("读取文件", Path::new(path), e))?;
    if meta.len() > MAX_FILE_SIZE_BYTES {
        return Err(AppError::IoKind {
            kind: "size".into(),
            message: format!("文件过大（超过 10MB 上限），已拒绝打开以保护内存: {path}"),
        });
    }

    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| io_error("读取文件", Path::new(path), e))?;
    let mut buf = vec![0u8; READ_CHUNK_BYTES];
    // 跨块残留：上一块回退的 UTF-8 多字节字符尾部，合并到下一块开头完整还原
    let mut remainder: Vec<u8> = Vec::new();

    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| io_error("读取文件", Path::new(path), e))?;
        remainder.extend_from_slice(&buf[..n]);

        if remainder.is_empty() {
            break; // EOF 且无残留
        }

        // UTF-8 边界回退：只发送合法前缀——多字节字符尾部（含跨块残缺序列）
        // 留在 remainder，下一轮与后续字节合并
        let valid_len = std::str::from_utf8(&remainder)
            .map(|s| s.len())
            .unwrap_or_else(|e| e.valid_up_to());
        if valid_len > 0 {
            let data = String::from_utf8(remainder.drain(..valid_len).collect()).map_err(|e| {
                tracing::warn!(error = %e, "文件编码错误（非 UTF-8）");
                AppError::IoKind {
                    kind: "utf8".into(),
                    message: format!("文件编码错误（非 UTF-8）: {path}"),
                }
            })?;
            send(FsReadChunk { data, done: false })?;
        }

        if n == 0 {
            // EOF：剩余字节若非空则文件含非法/残缺 UTF-8 序列（不完整字符尾部到文件结尾）
            if !remainder.is_empty() {
                return Err(AppError::IoKind {
                    kind: "utf8".into(),
                    message: format!("文件编码错误（非 UTF-8）: {path}"),
                });
            }
            break;
        }
    }

    // 终态：空数据 + done:true
    send(FsReadChunk {
        data: String::new(),
        done: true,
    })?;
    Ok(())
}

/// 写入文件内容（覆盖模式，UTF-8）
///
/// 写入前检测原文件行尾风格（CRLF/LF），保持与源文件一致，
/// 避免 CodeMirror 内部 LF 归一化导致保存后行尾突变 → git 误判 modified。
#[tauri::command]
pub async fn fs_write_file(
    path: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // State 仅做提取，业务逻辑在 fs_write_file_impl（测试直接调内核）
    fs_write_file_impl(path, content, extract_root(&state)?).await
}

/// fs_write_file 命令内核：路径 sandbox 校验（验证父目录）+ 行尾保持写盘
async fn fs_write_file_impl(
    path: String,
    content: String,
    root: Option<PathBuf>,
) -> Result<(), AppError> {
    // 路径 sandbox 校验（验证父目录——文件可能尚不存在，如新建/另存为）
    let check_path = Path::new(&path).parent().unwrap_or(Path::new("."));
    validate_path_within_root(&root, check_path)?;

    spawn_blocking_task(move || -> Result<(), AppError> {
        // 确保父目录存在
        if let Some(parent) = PathBuf::from(&path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error("创建目录", parent, e))?;
        }

        // 检测原文件行尾风格：只读前 CRLF_SAMPLE_MAX_BYTES 字节样本（避免全量读大文件）
        let use_crlf = std::fs::File::open(&path).map_or_else(
            // 新文件：Windows 默认 CRLF
            |_| cfg!(windows),
            |mut file| {
                use std::io::Read;
                let mut buf = vec![0u8; CRLF_SAMPLE_MAX_BYTES];
                let n = file.read(&mut buf).unwrap_or(0);
                String::from_utf8_lossy(&buf[..n]).contains("\r\n")
            },
        );

        // 保持与原文件一致的行尾风格
        let final_content = if use_crlf {
            // 将 LF 转为 CRLF（跳过已有的 CRLF）
            content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            content
        };

        std::fs::write(&path, &final_content)
            .map_err(|e| io_error("写入文件", Path::new(&path), e))?;
        Ok(())
    })
    .await
}

/// 递归读取目录内容
///
/// 过滤 `.git/`、`node_modules/`（重型目录，非用户编辑文件）。
/// 结果按文件夹→文件排序，同类型按名称字母排序。
#[tauri::command]
pub async fn fs_read_dir(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<DirEntry>, AppError> {
    // State 仅做提取，业务逻辑在 fs_read_dir_impl（测试直接调内核）
    fs_read_dir_impl(path, extract_root(&state)?).await
}

/// fs_read_dir 命令内核：路径 sandbox 校验 + 阻塞列目录
async fn fs_read_dir_impl(path: String, root: Option<PathBuf>) -> Result<Vec<DirEntry>, AppError> {
    // 路径 sandbox 校验
    validate_path_within_root(&root, Path::new(&path))?;

    spawn_blocking_task(move || {
        let mut entries: Vec<DirEntry> = Vec::new();
        let dir =
            std::fs::read_dir(&path).map_err(|e| io_error("读取目录", Path::new(&path), e))?;

        for entry in dir {
            let entry = entry.map_err(|e| io_error("读取目录", Path::new(&path), e))?;
            let name = entry.file_name().to_string_lossy().to_string();

            // 过滤重型目录
            if name == ".git" {
                continue;
            }

            let file_type = entry
                .file_type()
                .map_err(|e| io_error("读取目录", Path::new(&path), e))?;
            let is_dir = file_type.is_dir();
            let path_str = entry.path().to_string_lossy().replace('\\', "/");

            let (size, modified) = if is_dir {
                (None, None)
            } else {
                let meta = entry
                    .metadata()
                    .map_err(|e| io_error("读取目录", Path::new(&path), e))?;
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64);
                (Some(meta.len()), mtime)
            };

            entries.push(DirEntry {
                name,
                path: path_str,
                is_dir,
                size,
                modified,
            });
        }

        // 按文件夹→文件排序，同类型按名称字母排序
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Ok(entries)
    })
    .await
}

/// 创建目录（递归创建父目录）
#[tauri::command]
pub async fn fs_create_dir(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    // State 仅做提取，业务逻辑在 fs_create_dir_impl（测试直接调内核）
    fs_create_dir_impl(path, extract_root(&state)?).await
}

/// fs_create_dir 命令内核：路径 sandbox 校验（验证父目录）+ 递归创建
async fn fs_create_dir_impl(path: String, root: Option<PathBuf>) -> Result<(), AppError> {
    // 路径 sandbox 校验（验证父目录——目标目录可能尚不存在）
    let check_path = Path::new(&path).parent().unwrap_or(Path::new("."));
    validate_path_within_root(&root, check_path)?;

    spawn_blocking_task(move || {
        std::fs::create_dir_all(&path).map_err(|e| io_error("创建目录", Path::new(&path), e))?;
        Ok(())
    })
    .await
}

/// 删除文件或目录。
///
/// 注意：此操作为永久删除，不进回收站。
/// 删除目录时递归删除所有子级。
#[tauri::command]
pub async fn fs_delete(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    // State 仅做提取，业务逻辑在 fs_delete_impl（测试直接调内核）
    fs_delete_impl(path, extract_root(&state)?).await
}

/// fs_delete 命令内核：路径 sandbox 校验 + 删除文件/递归删除目录
async fn fs_delete_impl(path: String, root: Option<PathBuf>) -> Result<(), AppError> {
    // 路径 sandbox 校验
    validate_path_within_root(&root, Path::new(&path))?;

    spawn_blocking_task(move || {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(AppError::IoKind {
                kind: "path".into(),
                message: format!("路径不存在: {path}"),
            });
        }
        if p.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| io_error("删除", Path::new(&path), e))?;
        } else {
            std::fs::remove_file(&path).map_err(|e| io_error("删除", Path::new(&path), e))?;
        }
        Ok(())
    })
    .await
}

/// 重命名/移动文件或目录
///
/// 目标为已存在文件时覆盖（先删除再 rename，Windows 兼容），目标为已存在目录时返回错误。
#[tauri::command]
pub async fn fs_rename(
    src: String,
    dst: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // State 仅做提取，业务逻辑在 fs_rename_impl（测试直接调内核）
    fs_rename_impl(src, dst, extract_root(&state)?).await
}

/// fs_rename 命令内核：路径 sandbox 校验 + 覆盖已有文件/拒绝已有目录语义
async fn fs_rename_impl(src: String, dst: String, root: Option<PathBuf>) -> Result<(), AppError> {
    // 路径 sandbox 校验（源路径必须存在，目标路径验证父目录——目标可能尚不存在）
    validate_path_within_root(&root, Path::new(&src))?;
    let dst_parent = Path::new(&dst).parent().unwrap_or(Path::new("."));
    validate_path_within_root(&root, dst_parent)?;

    spawn_blocking_task(move || {
        // 目标为已存在目录 → 拒绝覆盖（防止静默递归删除）
        let dst_path = PathBuf::from(&dst);
        if dst_path.exists() {
            if dst_path.is_dir() {
                return Err(AppError::IoKind {
                    kind: "path".into(),
                    message: format!("目标路径是已有目录，无法覆盖: {dst}"),
                });
            }
            // 目标为已存在文件 → 先删除再 rename（Windows 上 std::fs::rename 不覆盖已有文件）
            std::fs::remove_file(&dst_path).map_err(|e| io_error("删除目标文件", &dst_path, e))?;
        }
        std::fs::rename(&src, &dst).map_err(|e| io_error("重命名", Path::new(&src), e))?;
        Ok(())
    })
    .await
}

/// fs_read_dir/create_dir/delete/rename 命令内核单元测试
///
/// 测试直接调用命令内核（fs_*_impl）而非 tauri::State——内核接收 root: Option<PathBuf>，
/// 消除 unsafe 内存转换构造 State 的 UB 风险与脆弱性（HFN-08）。
#[cfg(test)]
mod read_dir_tests {
    use super::*;

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    #[test]
    fn test_fs_read_dir_lists_children() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert_eq!(entries.len(), 3, "应返回 2 文件 + 1 子目录");

        // 验证 DirEntry 结构体字段
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub"));

        // 验证排序：文件夹在前
        let first_is_dir = entries.first().unwrap().is_dir;
        assert!(first_is_dir, "首个条目应为目录（文件夹优先排序）");

        // 验证文件具有 size 和 modified 字段
        for entry in &entries {
            if !entry.is_dir {
                assert!(entry.size.is_some(), "文件应具有 size");
                assert!(entry.modified.is_some(), "文件应具有 modified");
            }
        }
    }

    #[test]
    fn test_fs_read_dir_filters_dotgit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert_eq!(entries.len(), 1, "应过滤 .git，仅返回 visible.txt");
        assert_eq!(entries[0].name, "visible.txt");
    }

    #[test]
    fn test_fs_read_dir_shows_node_modules() {
        // node_modules 不再硬编码过滤——子树懒加载保证性能，目录条目可见无影响
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"node_modules"), "node_modules 应显示");
        assert!(names.contains(&"visible.txt"), "visible.txt 应显示");
        assert!(!names.contains(&".git"), ".git 应被过滤");
        assert_eq!(
            entries.len(),
            2,
            "node_modules 和 visible.txt 均应显示，仅 .git 过滤"
        );
    }

    #[test]
    fn test_fs_read_dir_shows_large_build_dirs() {
        // target/build/dist 等构建产物目录不硬编码过滤，依赖懒加载控制性能
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::create_dir(dir.path().join("build")).unwrap();
        std::fs::create_dir(dir.path().join("dist")).unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"target"));
        assert!(names.contains(&"build"));
        assert!(names.contains(&"dist"));
        assert!(names.contains(&"node_modules"));
        assert!(!names.contains(&".git"));
        assert_eq!(
            entries.len(),
            4,
            "target/build/dist/node_modules 均应显示，仅 .git 过滤"
        );
    }

    #[test]
    fn test_fs_read_dir_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert_eq!(entries.len(), 0, "空目录应返回空列表");
    }

    #[test]
    fn test_fs_read_dir_shows_dotclaude() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".claude"), ".claude 目录应显示");
        assert!(names.contains(&"visible.txt"), "visible.txt 应显示");
        assert_eq!(entries.len(), 2, ".claude 和 visible.txt 均应显示");
    }

    #[test]
    fn test_fs_create_dir_creates() {
        let base = tempfile::tempdir().unwrap();
        let new_dir = base.path().join("new_folder");

        run(fs_create_dir_impl(
            new_dir.to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert!(new_dir.exists(), "目录应被创建");

        // 通过 fs_read_dir 验证目录存在
        let entries = run(fs_read_dir_impl(
            base.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert!(entries.iter().any(|e| e.name == "new_folder" && e.is_dir));
    }

    #[test]
    fn test_fs_create_dir_parent_creation() {
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("a").join("b").join("c");

        run(fs_create_dir_impl(
            nested.to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert!(nested.exists(), "嵌套目录应被创建");
    }

    #[test]
    fn test_fs_delete_file_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("to_delete.txt");
        std::fs::write(&file, "delete me").unwrap();
        assert!(file.exists());

        run(fs_delete_impl(file.to_string_lossy().to_string(), None)).unwrap();
        assert!(!file.exists(), "文件应被删除");
    }

    #[test]
    fn test_fs_delete_dir_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("to_delete_dir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("child.txt"), "child").unwrap();

        run(fs_delete_impl(sub.to_string_lossy().to_string(), None)).unwrap();
        assert!(!sub.exists(), "目录及其内容应被删除");
    }

    #[test]
    fn test_fs_rename_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("old.txt");
        let dst = dir.path().join("new.txt");
        std::fs::write(&src, "content").unwrap();

        run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        assert!(!src.exists(), "旧路径应不存在");
        assert!(dst.exists(), "新路径应存在");

        // 通过 fs_read_dir 验证
        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            None,
        ))
        .unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"new.txt"));
        assert!(!names.contains(&"old.txt"));
    }
}

/// fs_write_file CRLF 行尾保持测试（HFN-01：直接调命令内核，固定输入/输出字节断言）
///
/// 测试不重写行尾检测与行尾转换逻辑——只构造固定输入字节，
/// 经真实 fs_write_file_impl 写盘后断言固定输出字节（避免与实现同构脱节）。
#[cfg(test)]
mod write_file_tests {
    use super::*;

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    /// 原文件为 CRLF → 写入 LF 内容后保持 CRLF
    #[test]
    fn crlf_preserved_when_original_is_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("crlf.txt");
        // 原文件为 CRLF
        std::fs::write(&file_path, "line1\r\nline2\r\nline3\r\n").unwrap();
        // 模拟 CodeMirror 归一化后的 LF 内容
        let lf_content = "line1\nline2\nline3\n".to_string();

        run(fs_write_file_impl(
            file_path.to_string_lossy().to_string(),
            lf_content,
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();

        // 固定输出字节断言：LF 内容被转为 CRLF 写盘
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            b"line1\r\nline2\r\nline3\r\n",
            "原文件为 CRLF 时写入内容应保持 CRLF"
        );
    }

    /// 原文件为 LF → 写入内容原样保持 LF
    #[test]
    fn lf_preserved_when_original_is_lf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("lf.txt");
        // 原文件为 LF
        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        run(fs_write_file_impl(
            file_path.to_string_lossy().to_string(),
            "line1\nline2\nline3\n".to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();

        // 固定输出字节断言：LF 原样保持
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            b"line1\nline2\nline3\n",
            "原文件为 LF 时写入内容应保持 LF"
        );
    }

    /// 新文件（不存在）→ Windows 上默认 CRLF，Unix 上默认 LF
    #[test]
    fn new_file_defaults_to_crlf_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new.txt");
        // 确保文件不存在
        let _ = std::fs::remove_file(&file_path);

        run(fs_write_file_impl(
            file_path.to_string_lossy().to_string(),
            "hello\nworld\n".to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();

        // 固定输出字节断言：平台默认行尾
        #[cfg(windows)]
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            b"hello\r\nworld\r\n",
            "Windows 上新文件默认 CRLF"
        );
        #[cfg(not(windows))]
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            b"hello\nworld\n",
            "Unix 上新文件默认 LF"
        );
    }

    /// 混合行尾 → 原文件含 CRLF 时全部归一为 CRLF（无孤立 LF）
    #[test]
    fn mixed_endings_normalized_to_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("mixed.txt");
        // 原文件含 CRLF（判定目标行尾为 CRLF）
        std::fs::write(&file_path, "line1\r\nline2\nline3\r\n").unwrap();
        // CodeMirror 可能产生混合行尾内容
        let mixed_content = "line1\nline2\r\nline3\n".to_string();

        run(fs_write_file_impl(
            file_path.to_string_lossy().to_string(),
            mixed_content,
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();

        // 固定输出字节断言：混合行尾全部归一为 CRLF
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            b"line1\r\nline2\r\nline3\r\n",
            "混合行尾应全部归一为 CRLF"
        );
    }
}

/// 命令内核层单元测试（TE-14）：参数透传、错误映射、sandbox 校验分支
/// 以及 SEC-04 fs_rename 覆盖已有文件 / 拒绝已有目录行为验证
///
/// 测试直接调用命令内核（fs_*_impl，root 直接传 Option<PathBuf>），
/// 无需构造 tauri::State——State 仅为命令包装层提取 root 的通道（HFN-08）。
#[cfg(test)]
mod command_wrapper_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    // ===== fs_read_file =====

    /// 驱动 fs_read_file_impl 并收集全部分块（Channel 无法在 L1 构造——send 回调收集）
    fn run_read_file(path: String, root: Option<PathBuf>) -> Result<Vec<FsReadChunk>, AppError> {
        let chunks: Arc<Mutex<Vec<FsReadChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let collector = chunks.clone();
        run(fs_read_file_impl(
            path,
            move |chunk| {
                collector.lock().unwrap().push(chunk);
                Ok(())
            },
            root,
        ))?;
        let collected = chunks.lock().unwrap().clone();
        Ok(collected)
    }

    #[test]
    fn test_fs_read_file_returns_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("read.txt");
        std::fs::write(&file, "hello world").unwrap();

        let chunks = run_read_file(
            file.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        )
        .unwrap();
        // 单块 + 终态，拼接还原一致
        let joined: String = chunks
            .iter()
            .filter(|c| !c.done)
            .map(|c| c.data.as_str())
            .collect();
        assert_eq!(joined, "hello world");
        assert!(chunks.last().unwrap().done, "末块应为终态（done=true）");
        assert!(
            chunks.last().unwrap().data.is_empty(),
            "终态块 data 应为空串"
        );
    }

    #[test]
    fn test_fs_read_file_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ghost.txt");

        let result = run_read_file(
            file.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        );
        assert!(result.is_err(), "不存在的文件应返回错误");
    }

    #[test]
    fn test_fs_read_file_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("secret.txt");
        std::fs::write(&file, "secret").unwrap();

        let result = run_read_file(
            file.to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        );
        assert!(result.is_err(), "根外路径应被沙箱拒绝");
    }

    // ===== fs_write_file =====

    #[test]
    fn test_fs_write_file_writes_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("output.txt");

        run(fs_write_file_impl(
            file.to_string_lossy().to_string(),
            "hello world".to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();
        assert!(file.exists());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello world");
    }

    #[test]
    fn test_fs_write_file_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("evil.txt");

        let result = run(fs_write_file_impl(
            file.to_string_lossy().to_string(),
            "bad".to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "根外路径应被沙箱拒绝");
    }

    // ===== fs_read_dir =====

    #[test]
    fn test_fs_read_dir_returns_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();

        let entries = run(fs_read_dir_impl(
            dir.path().to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a.txt");
    }

    #[test]
    fn test_fs_read_dir_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("ghost_dir");

        let result = run(fs_read_dir_impl(
            nonexistent.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ));
        assert!(result.is_err(), "不存在的目录应返回错误");
    }

    #[test]
    fn test_fs_read_dir_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        let result = run(fs_read_dir_impl(
            outside.path().to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "根外路径应被沙箱拒绝");
    }

    // ===== fs_rename（SEC-04：覆盖文件 / 拒绝目录） =====

    #[test]
    fn test_fs_rename_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        std::fs::write(&src, "new content").unwrap();
        std::fs::write(&dst, "old content").unwrap();

        run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();
        assert!(!src.exists(), "源文件应不存在");
        assert_eq!(
            std::fs::read_to_string(&dst).unwrap(),
            "new content",
            "目标文件应为源文件内容（覆盖成功）"
        );
    }

    #[test]
    fn test_fs_rename_rejects_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("file.txt");
        let dst = dir.path().join("existing_dir");
        std::fs::write(&src, "keep").unwrap();
        std::fs::create_dir(&dst).unwrap();

        let result = run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ));
        assert!(result.is_err(), "目标为已有目录应返回错误");
        assert!(src.exists(), "源文件应保留（未被移动）");
        assert!(dst.exists(), "目标目录应保留（未被递归删除）");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("目录") || msg.contains("无法覆盖"),
            "错误消息应说明目标为目录"
        );
    }

    #[test]
    fn test_fs_rename_source_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let src = outside.path().join("file.txt");
        let dst = root.path().join("dst.txt");
        std::fs::write(&src, "data").unwrap();

        let result = run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "源路径在根外应被沙箱拒绝");
    }

    #[test]
    fn test_fs_rename_dest_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let src = root.path().join("src.txt");
        std::fs::write(&src, "data").unwrap();

        let result = run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            outside
                .path()
                .join("escape.txt")
                .to_string_lossy()
                .to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "目标路径在根外应被沙箱拒绝");
    }

    // ===== BE-13: 错误消息含路径上下文 =====

    /// 读取不存在的文件 → 错误消息含完整路径
    #[test]
    fn test_fs_read_file_error_message_contains_path() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("ghost.txt");

        let err = run_read_file(
            ghost.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&ghost.to_string_lossy().to_string()),
            "错误消息应含文件路径，实际: {msg}"
        );
    }

    /// 读取不存在的目录 → 错误消息含完整路径
    #[test]
    fn test_fs_read_dir_error_message_contains_path() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("ghost_dir");

        let err = run(fs_read_dir_impl(
            ghost.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&ghost.to_string_lossy().to_string()),
            "错误消息应含目录路径，实际: {msg}"
        );
    }

    /// 父路径为文件（create_dir_all 失败）→ 错误消息含业务语义「创建目录失败」+ 路径
    #[test]
    fn test_fs_write_file_error_message_contains_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker.txt");
        std::fs::write(&blocker, "x").unwrap();
        // 父路径是文件 → 创建父目录必然失败（Windows/Unix 均如此）
        let target = blocker.join("child.txt");

        let err = run(fs_write_file_impl(
            target.to_string_lossy().to_string(),
            "data".to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("创建目录失败") && msg.contains("blocker.txt"),
            "错误消息应含业务语义与路径，实际: {msg}"
        );
    }

    /// 重命名不存在的源 → 错误消息含完整路径
    #[test]
    fn test_fs_rename_error_message_contains_path() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("missing.txt");
        let dst = dir.path().join("new.txt");

        let err = run(fs_rename_impl(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&src.to_string_lossy().to_string()),
            "错误消息应含源路径，实际: {msg}"
        );
    }

    // ===== fs 异常路径（HFN-04） =====

    /// fs_delete 删除不存在的路径 → 返回错误（不静默成功）
    #[test]
    fn test_fs_delete_nonexistent_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("ghost.txt");
        assert!(!ghost.exists(), "前置：文件不存在");

        let result = run(fs_delete_impl(
            ghost.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ));
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("不存在"),
            "错误消息应说明路径不存在，实际: {msg}"
        );
    }

    /// fs_create_dir 目标在 root 外 → 沙箱拒绝（目录不被创建）
    #[test]
    fn test_fs_create_dir_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("evil_dir");

        let result = run(fs_create_dir_impl(
            target.to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "根外目录创建应被沙箱拒绝");
        assert!(!target.exists(), "沙箱拒绝后目录不应被创建");
    }

    /// fs_delete 目标在 root 外 → 沙箱拒绝（文件不被删除）
    #[test]
    fn test_fs_delete_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("secret.txt");
        std::fs::write(&file, "secret").unwrap();

        let result = run(fs_delete_impl(
            file.to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        ));
        assert!(result.is_err(), "根外删除应被沙箱拒绝");
        assert!(file.exists(), "沙箱拒绝后文件应保留");
    }

    /// spawn_blocking 闭包 panic → JoinError → 统一映射为 AppError::TaskJoin
    #[test]
    fn test_spawn_blocking_panic_maps_to_task_join() {
        // 闭包内 panic 经 spawn_blocking 捕获为 JoinError，命令内核统一映射为 TaskJoin
        let result = run(spawn_blocking_task(|| -> Result<(), AppError> {
            panic!("模拟阻塞任务 panic");
        }));
        match result {
            Err(AppError::TaskJoin(msg)) => {
                assert!(
                    msg.contains("panicked") || msg.contains("panic"),
                    "TaskJoin 消息应含 panic 信息，实际: {msg}"
                );
            }
            other => panic!("闭包 panic 应映射为 AppError::TaskJoin，实际: {other:?}"),
        }
    }
}

/// fs_read_file 分块读取核心测试（BE-03）：多块拼接还原 / 多字节跨界 / 超限拒绝 / 空文件终态
///
/// 直接测同步核心 read_file_chunked（send 回调注入收集，无需构造 tauri::ipc::Channel——
/// L1 无 webview 运行时上下文，见 pty/CLAUDE.md 豁免项 1）。
#[cfg(test)]
mod read_file_chunked_tests {
    use super::*;

    /// 收集分块，返回（全部分块, 核心执行结果）
    fn collect_chunks(path: &Path) -> (Vec<FsReadChunk>, Result<(), AppError>) {
        let mut chunks: Vec<FsReadChunk> = Vec::new();
        let result = read_file_chunked(&path.to_string_lossy(), |chunk| {
            chunks.push(chunk);
            Ok(())
        });
        (chunks, result)
    }

    /// 数据块拼接还原原文
    fn join_data(chunks: &[FsReadChunk]) -> String {
        chunks
            .iter()
            .filter(|c| !c.done)
            .map(|c| c.data.as_str())
            .collect()
    }

    /// 多块文件：分块拼接还原一致 + 发送序列契约（数据块 done:false，终态恰一个且 data 空）
    #[test]
    fn test_read_file_chunked_multi_chunk_joins_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("multi.txt");
        // 600KB ASCII——超过一块（256KB），至少 2 个数据块
        let content = "abcdefghij".repeat(60 * 1024);
        assert!(content.len() > READ_CHUNK_BYTES, "前置：内容超过单块大小");
        std::fs::write(&file, &content).unwrap();

        let (chunks, result) = collect_chunks(&file);
        result.unwrap();

        let data_chunks: Vec<&FsReadChunk> = chunks.iter().filter(|c| !c.done).collect();
        assert!(
            data_chunks.len() >= 2,
            "应产生至少 2 个数据块，实际: {}",
            data_chunks.len()
        );
        assert_eq!(
            chunks.iter().filter(|c| c.done).count(),
            1,
            "终态块应恰为 1 个"
        );
        let terminal = chunks.last().unwrap();
        assert!(terminal.done, "末块应为终态");
        assert!(terminal.data.is_empty(), "终态块 data 应为空串");
        for c in &data_chunks {
            assert!(!c.done, "数据块 done 应为 false");
            assert!(c.data.len() <= READ_CHUNK_BYTES, "单块大小不应超过 256KB");
        }
        assert_eq!(join_data(&chunks), content, "分块拼接后应与原文一致");
    }

    /// 多字节字符跨界：3 字节汉字「界」(E7 95 8C) 从 256KB 边界前 1 字节开始——首块切点落在字符中部
    #[test]
    fn test_read_file_chunked_utf8_boundary_not_split() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("utf8.txt");
        let mut content = vec![b'a'; READ_CHUNK_BYTES - 1];
        content.extend_from_slice("界".as_bytes());
        content.extend_from_slice("后".repeat(1000).as_bytes());
        std::fs::write(&file, &content).unwrap();

        let (chunks, result) = collect_chunks(&file);
        result.unwrap();

        let data_chunks: Vec<&FsReadChunk> = chunks.iter().filter(|c| !c.done).collect();
        assert!(
            data_chunks.len() >= 2,
            "应有多个数据块，实际: {}",
            data_chunks.len()
        );
        // 字节级还原断言：跨块字符若被切散则字节序列必不等
        assert_eq!(
            join_data(&chunks).as_bytes(),
            &content[..],
            "跨块多字节字符不得切散，拼接后字节应与原文一致"
        );
    }

    /// 4 字节 emoji「😀」(F0 9F 98 80) 从 256KB 边界前 2 字节开始——首块切点落在字符中部
    #[test]
    fn test_read_file_chunked_utf8_4byte_boundary_not_split() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("utf8_4b.txt");
        let mut content = vec![b'x'; READ_CHUNK_BYTES - 2];
        content.extend_from_slice("😀".as_bytes());
        content.push(b'y');
        std::fs::write(&file, &content).unwrap();

        let (chunks, result) = collect_chunks(&file);
        result.unwrap();
        assert_eq!(
            join_data(&chunks).as_bytes(),
            &content[..],
            "4 字节字符跨块不得切散，拼接后字节应与原文一致"
        );
    }

    /// 超限拒绝：>10MB 文件 metadata 校验即 Err，不发送任何块
    #[test]
    fn test_read_file_chunked_over_limit_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.txt");
        // 稀疏文件 10MB+1 字节——不写内容，metadata 长度即超限
        std::fs::File::create(&file)
            .unwrap()
            .set_len(MAX_FILE_SIZE_BYTES + 1)
            .unwrap();

        let (chunks, result) = collect_chunks(&file);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("10MB"),
            "超限错误消息应含 10MB 上限，实际: {err}"
        );
        assert!(chunks.is_empty(), "超限拒绝不应发送任何块");
    }

    /// 恰好 10MB：允许读取，全量拼接还原
    #[test]
    fn test_read_file_chunked_at_limit_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("limit.txt");
        // 稀疏文件恰好 10MB——读出全 NUL（合法 UTF-8 的 U+0000）
        std::fs::File::create(&file)
            .unwrap()
            .set_len(MAX_FILE_SIZE_BYTES)
            .unwrap();

        let (chunks, result) = collect_chunks(&file);
        result.unwrap();
        assert_eq!(
            join_data(&chunks).len() as u64,
            MAX_FILE_SIZE_BYTES,
            "恰好 10MB 应完整读出"
        );
        assert!(chunks.last().unwrap().done, "末块应为终态");
    }

    /// 空文件：直接终态（{data:"", done:true}），无数据块
    #[test]
    fn test_read_file_chunked_empty_file_terminal_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("empty.txt");
        std::fs::write(&file, "").unwrap();

        let (chunks, result) = collect_chunks(&file);
        result.unwrap();
        assert_eq!(
            chunks.len(),
            1,
            "空文件应只发终态块，实际: {}",
            chunks.len()
        );
        assert!(chunks[0].done, "终态块 done 应为 true");
        assert!(chunks[0].data.is_empty(), "终态块 data 应为空串");
    }

    /// 非法 UTF-8（含非法字节）：拒绝且不发送终态（行为同旧 read_to_string）
    #[test]
    fn test_read_file_chunked_invalid_utf8_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("binary.txt");
        // 0xFF/0xFE 为非法 UTF-8 字节
        std::fs::write(&file, b"abc\xff\xfe").unwrap();

        let (chunks, result) = collect_chunks(&file);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("UTF-8") || err.contains("编码"),
            "错误消息应说明编码问题，实际: {err}"
        );
        assert!(!chunks.iter().any(|c| c.done), "编码错误时不得发送终态");
    }

    /// 残缺多字节尾部到文件结尾（不完整字符）：拒绝（行为同旧 read_to_string）
    #[test]
    fn test_read_file_chunked_incomplete_tail_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("truncated.txt");
        // 3 字节汉字只有前 2 字节，文件即以残缺序列结尾
        std::fs::write(&file, b"abc\xe7\x95").unwrap();

        let (chunks, result) = collect_chunks(&file);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("UTF-8") || err.contains("编码"),
            "错误消息应说明编码问题，实际: {err}"
        );
        assert!(!chunks.iter().any(|c| c.done), "编码错误时不得发送终态");
    }
}
