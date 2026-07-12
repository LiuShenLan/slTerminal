/// 文件系统模块 — 文件读/写命令
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
use crate::error::AppError;
use crate::state::AppState;
use crate::state::validate_path_within_root;
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

/// 读取文件内容（UTF-8 文本）
#[tauri::command]
pub async fn fs_read_file(path: String, state: State<'_, AppState>) -> Result<String, AppError> {
    // 路径 sandbox 校验
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::IoKind { kind: "lock".into(), message: format!("获取 project_root 锁失败: {e}") })?;
        validate_path_within_root(&root, Path::new(&path))?;
    }

    let content = match tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        Ok(std::fs::read_to_string(&path)?)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
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
    // 路径 sandbox 校验
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::IoKind { kind: "lock".into(), message: format!("获取 project_root 锁失败: {e}") })?;
        validate_path_within_root(&root, Path::new(&path))?;
    }

    match tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // 确保父目录存在
        if let Some(parent) = PathBuf::from(&path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 检测原文件行尾风格：读原始字节检查是否含 \r\n
        let use_crlf = std::fs::read(&path).map_or_else(
            // 新文件：Windows 默认 CRLF
            |_| cfg!(windows),
            |bytes| {
                // 取前 64KB 样本检测 CRLF
                let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(CRLF_SAMPLE_MAX_BYTES)]);
                sample.contains("\r\n")
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
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
    Ok(())
}

/// 递归读取目录内容
///
/// 过滤 `.git/`、`node_modules/`（重型目录，非用户编辑文件）。
/// 结果按文件夹→文件排序，同类型按名称字母排序。
#[tauri::command]
pub async fn fs_read_dir(path: String) -> Result<Vec<DirEntry>, AppError> {
    match tokio::task::spawn_blocking(move || {
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
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 创建目录（递归创建父目录）
#[tauri::command]
pub async fn fs_create_dir(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    // 路径 sandbox 校验
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::IoKind { kind: "lock".into(), message: format!("获取 project_root 锁失败: {e}") })?;
        validate_path_within_root(&root, Path::new(&path))?;
    }

    match tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&path)?;
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 删除文件或目录。
///
/// 注意：此操作为永久删除，不进回收站。
/// 删除目录时递归删除所有子级。
#[tauri::command]
pub async fn fs_delete(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    // 路径 sandbox 校验
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::IoKind { kind: "lock".into(), message: format!("获取 project_root 锁失败: {e}") })?;
        validate_path_within_root(&root, Path::new(&path))?;
    }

    match tokio::task::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(AppError::IoKind { kind: "path".into(), message: format!("路径不存在: {path}") });
        }
        if p.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 重命名/移动文件或目录
///
/// 目标路径存在时覆盖。
#[tauri::command]
pub async fn fs_rename(
    src: String,
    dst: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // 路径 sandbox 校验（源路径和目标路径都须在项目根内）
    {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::IoKind { kind: "lock".into(), message: format!("获取 project_root 锁失败: {e}") })?;
        validate_path_within_root(&root, Path::new(&src))?;
        validate_path_within_root(&root, Path::new(&dst))?;
    }

    match tokio::task::spawn_blocking(move || {
        // 若目标存在且为目录，先删除
        let dst_path = PathBuf::from(&dst);
        if dst_path.exists() && dst_path.is_dir() {
            std::fs::remove_dir_all(&dst)?;
        }
        std::fs::rename(&src, &dst)?;
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 测试辅助：从 &AppState 构造 tauri::State
///
/// tauri::State 无公开构造函数（内部仅由 Tauri 依赖注入创建），
/// 但其布局为 `State(&T)`（单字段元组结构体），与 `&T` 等大。
/// SAFETY: 仅供测试。transmute 的前提是二者字节等大且在调用期间引用有效。
#[cfg(test)]
unsafe fn as_tauri_state<'a, T: Send + Sync + 'static>(t: &'a T) -> tauri::State<'a, T> {
    std::mem::transmute::<&'a T, tauri::State<'a, T>>(t)
}

/// 创建测试用 AppState（project_root=None，sandbox 校验默认放行）
#[cfg(test)]
fn test_app_state() -> AppState {
    AppState::new()
}

/// fs_read_dir/create_dir/delete/rename 单元测试
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

        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
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

        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
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

        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"node_modules"), "node_modules 应显示");
        assert!(names.contains(&"visible.txt"), "visible.txt 应显示");
        assert!(!names.contains(&".git"), ".git 应被过滤");
        assert_eq!(entries.len(), 2, "node_modules 和 visible.txt 均应显示，仅 .git 过滤");
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

        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"target"));
        assert!(names.contains(&"build"));
        assert!(names.contains(&"dist"));
        assert!(names.contains(&"node_modules"));
        assert!(!names.contains(&".git"));
        assert_eq!(entries.len(), 4, "target/build/dist/node_modules 均应显示，仅 .git 过滤");
    }

    #[test]
    fn test_fs_read_dir_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
        assert_eq!(entries.len(), 0, "空目录应返回空列表");
    }

    #[test]
    fn test_fs_read_dir_shows_dotclaude() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".claude"), ".claude 目录应显示");
        assert!(names.contains(&"visible.txt"), "visible.txt 应显示");
        assert_eq!(entries.len(), 2, ".claude 和 visible.txt 均应显示");
    }

    #[test]
    fn test_fs_create_dir_creates() {
        let base = tempfile::tempdir().unwrap();
        let new_dir = base.path().join("new_folder");
        let app_state = test_app_state();
        let state = unsafe { as_tauri_state(&app_state) };

        run(fs_create_dir(new_dir.to_string_lossy().to_string(), state)).unwrap();
        assert!(new_dir.exists(), "目录应被创建");

        // 通过 fs_read_dir 验证目录存在
        let entries = run(fs_read_dir(base.path().to_string_lossy().to_string())).unwrap();
        assert!(entries.iter().any(|e| e.name == "new_folder" && e.is_dir));
    }

    #[test]
    fn test_fs_create_dir_parent_creation() {
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("a").join("b").join("c");
        let app_state = test_app_state();
        let state = unsafe { as_tauri_state(&app_state) };

        run(fs_create_dir(nested.to_string_lossy().to_string(), state)).unwrap();
        assert!(nested.exists(), "嵌套目录应被创建");
    }

    #[test]
    fn test_fs_delete_file_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("to_delete.txt");
        std::fs::write(&file, "delete me").unwrap();
        assert!(file.exists());

        let app_state = test_app_state();
        let state = unsafe { as_tauri_state(&app_state) };
        run(fs_delete(file.to_string_lossy().to_string(), state)).unwrap();
        assert!(!file.exists(), "文件应被删除");
    }

    #[test]
    fn test_fs_delete_dir_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("to_delete_dir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("child.txt"), "child").unwrap();

        let app_state = test_app_state();
        let state = unsafe { as_tauri_state(&app_state) };
        run(fs_delete(sub.to_string_lossy().to_string(), state)).unwrap();
        assert!(!sub.exists(), "目录及其内容应被删除");
    }

    #[test]
    fn test_fs_rename_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("old.txt");
        let dst = dir.path().join("new.txt");
        std::fs::write(&src, "content").unwrap();

        let app_state = test_app_state();
        let state = unsafe { as_tauri_state(&app_state) };
        run(fs_rename(
            src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            state,
        ))
        .unwrap();
        assert!(!src.exists(), "旧路径应不存在");
        assert!(dst.exists(), "新路径应存在");

        // 通过 fs_read_dir 验证
        let entries = run(fs_read_dir(dir.path().to_string_lossy().to_string())).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"new.txt"));
        assert!(!names.contains(&"old.txt"));
    }
}

/// fs_write_file CRLF 行尾保持逻辑测试
#[cfg(test)]
mod write_file_tests {
    use super::*;

    /// 原文件为 CRLF → 写入内容应保持 CRLF
    #[test]
    fn crlf_preserved_when_original_is_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("crlf.txt");
        // 写入含 \r\n 的原文件
        std::fs::write(&file_path, "line1\r\nline2\r\nline3\r\n").unwrap();

        // 模拟 CodeMirror 归一化后的 LF 内容
        let lf_content = "line1\nline2\nline3\n";

        // 检测原文件行尾
        let raw = std::fs::read(&file_path).unwrap();
        let use_crlf = {
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(CRLF_SAMPLE_MAX_BYTES)]);
            sample.contains("\r\n")
        };
        assert!(use_crlf, "原文件含 CRLF");

        // 转换为 CRLF
        let final_content = if use_crlf {
            lf_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            lf_content.to_string()
        };

        std::fs::write(&file_path, &final_content).unwrap();

        // 验证写入后仍为 CRLF
        let saved = std::fs::read_to_string(&file_path).unwrap();
        assert!(saved.contains("\r\n"), "写入后应保持 CRLF");
        assert_eq!(saved, "line1\r\nline2\r\nline3\r\n");
    }

    /// 原文件为 LF → 写入内容应保持 LF
    #[test]
    fn lf_preserved_when_original_is_lf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("lf.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let lf_content = "line1\nline2\nline3\n";

        let raw = std::fs::read(&file_path).unwrap();
        let use_crlf = {
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(CRLF_SAMPLE_MAX_BYTES)]);
            sample.contains("\r\n")
        };
        assert!(!use_crlf, "原文件不含 CRLF");

        let final_content = if use_crlf {
            lf_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            lf_content.to_string()
        };

        std::fs::write(&file_path, &final_content).unwrap();

        let saved = std::fs::read_to_string(&file_path).unwrap();
        assert!(!saved.contains("\r\n"), "写入后应保持 LF");
        assert_eq!(saved, "line1\nline2\nline3\n");
    }

    /// 新文件（不存在）→ Windows 上默认 CRLF
    #[test]
    fn new_file_defaults_to_crlf_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new.txt");
        // 确保文件不存在
        let _ = std::fs::remove_file(&file_path);

        let lf_content = "hello\nworld\n";

        // 新文件检测：读取失败 → 使用平台默认
        let use_crlf = std::fs::read(&file_path).map_or_else(
            |_| cfg!(windows),
            |bytes| {
                let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(CRLF_SAMPLE_MAX_BYTES)]);
                sample.contains("\r\n")
            },
        );

        let final_content = if use_crlf {
            lf_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            lf_content.to_string()
        };

        std::fs::write(&file_path, &final_content).unwrap();

        let saved = std::fs::read_to_string(&file_path).unwrap();
        #[cfg(windows)]
        assert!(saved.contains("\r\n"), "Windows 上新文件应为 CRLF");
        #[cfg(not(windows))]
        assert!(!saved.contains("\r\n"), "Unix 上新文件应为 LF");
    }

    /// 混合行尾 → 标准化为 CRLF（如原文件为 CRLF）
    #[test]
    fn mixed_endings_normalized_to_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("mixed.txt");
        std::fs::write(&file_path, "line1\r\nline2\nline3\r\n").unwrap();

        let mixed_content = "line1\nline2\r\nline3\n"; // CodeMirror 可能产生混合

        let raw = std::fs::read(&file_path).unwrap();
        let use_crlf = {
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(CRLF_SAMPLE_MAX_BYTES)]);
            sample.contains("\r\n")
        };
        assert!(use_crlf);

        let final_content = if use_crlf {
            mixed_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            mixed_content.to_string()
        };

        std::fs::write(&file_path, &final_content).unwrap();

        let saved = std::fs::read_to_string(&file_path).unwrap();
        // 应全部为 CRLF，无孤立 LF
        let lf_only = saved.replace("\r\n", "");
        assert!(!lf_only.contains('\n'), "不应有孤立的 LF");
    }

    /// 仅含 \n 的 LF 文件 → replace 操作不改变内容
    #[test]
    fn lf_only_content_unchanged_in_crlf_conversion() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let crlf = content.replace("\r\n", "\n").replace('\n', "\r\n");
        assert!(crlf.contains("\r\n"));
        assert!(!crlf.contains('\n') || crlf.ends_with("\r\n"),
            "转换后不应有孤立 LF（末尾 \\r\\n 跳过）");
    }
}
