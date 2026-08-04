/// 文件系统模块 — 文件读/写命令
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
use crate::error::AppError;
use crate::state::validate_path_within_root;
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
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
    let guard = state.project_root.read().map_err(|e| AppError::IoKind {
        kind: "lock".into(),
        message: format!("获取 project_root 锁失败: {e}"),
    })?;
    Ok((*guard).clone())
}

/// 读取文件内容（UTF-8 文本）
#[tauri::command]
pub async fn fs_read_file(path: String, state: State<'_, AppState>) -> Result<String, AppError> {
    // State 仅做提取，业务逻辑在 fs_read_file_impl（测试直接调内核）
    fs_read_file_impl(path, extract_root(&state)?).await
}

/// fs_read_file 命令内核：路径 sandbox 校验 + 阻塞读文件
async fn fs_read_file_impl(path: String, root: Option<PathBuf>) -> Result<String, AppError> {
    // 路径 sandbox 校验
    validate_path_within_root(&root, Path::new(&path))?;

    let content = spawn_blocking_task(move || -> Result<String, AppError> {
        Ok(std::fs::read_to_string(&path)?)
    })
    .await?;
    Ok(content)
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
            std::fs::create_dir_all(parent)?;
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

        std::fs::write(&path, &final_content)?;
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
        let dir = std::fs::read_dir(&path)?;

        for entry in dir {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            // 过滤重型目录
            if name == ".git" {
                continue;
            }

            let file_type = entry.file_type()?;
            let is_dir = file_type.is_dir();
            let path_str = entry.path().to_string_lossy().replace('\\', "/");

            let (size, modified) = if is_dir {
                (None, None)
            } else {
                let meta = entry.metadata()?;
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
        std::fs::create_dir_all(&path)?;
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
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
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
            std::fs::remove_file(&dst_path)?;
        }
        std::fs::rename(&src, &dst)?;
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

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    // ===== fs_read_file =====

    #[test]
    fn test_fs_read_file_returns_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("read.txt");
        std::fs::write(&file, "hello world").unwrap();

        let content = run(fs_read_file_impl(
            file.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ))
        .unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_fs_read_file_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ghost.txt");

        let result = run(fs_read_file_impl(
            file.to_string_lossy().to_string(),
            Some(dir.path().to_path_buf()),
        ));
        assert!(result.is_err(), "不存在的文件应返回错误");
    }

    #[test]
    fn test_fs_read_file_outside_root_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("secret.txt");
        std::fs::write(&file, "secret").unwrap();

        let result = run(fs_read_file_impl(
            file.to_string_lossy().to_string(),
            Some(root.path().to_path_buf()),
        ));
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
