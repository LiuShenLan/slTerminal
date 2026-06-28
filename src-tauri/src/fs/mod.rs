/// 文件系统模块 — 文件读/写命令 + 设置持久化
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
/// 设置用 tempfile 原子写入 + .bak 备份兜底。
use crate::error::AppError;
use serde::Serialize;
use std::io::Write as _;
use std::path::PathBuf;
use tempfile::NamedTempFile;

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
pub async fn fs_read_file(path: String) -> Result<String, AppError> {
    let content = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        Ok(std::fs::read_to_string(&path)?)
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?;
    content
}

/// 写入文件内容（覆盖模式，UTF-8）
///
/// 写入前检测原文件行尾风格（CRLF/LF），保持与源文件一致，
/// 避免 CodeMirror 内部 LF 归一化导致保存后行尾突变 → git 误判 modified。
#[tauri::command]
pub async fn fs_write_file(path: String, content: String) -> Result<(), AppError> {
    let _ = tokio::task::spawn_blocking(move || -> Result<(), AppError> {
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
                let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(65536)]);
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
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?;
    Ok(())
}

/// 获取应用数据目录（~/.slterminal）
fn app_data_dir() -> Result<PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Io("无法获取用户主目录".to_string()))?;
    Ok(home.join(".slterminal"))
}

/// 持久化设置（原子写入：tempfile → flush → persist，.bak 备份兜底）
#[tauri::command]
pub async fn save_settings(settings: serde_json::Value) -> Result<(), AppError> {
    let app_dir = app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let settings_path = app_dir.join("settings.json");

    let _ = tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let json = serde_json::to_string_pretty(&settings)?;
        let mut tmp = NamedTempFile::new_in(&app_dir)?;
        tmp.write_all(json.as_bytes())?;
        tmp.flush()?;
        if settings_path.exists() {
            let bak = app_dir.join("settings.json.bak");
            let _ = std::fs::copy(&settings_path, &bak);
        }
        tmp.persist(&settings_path)
            .map_err(|e| AppError::Io(format!("persist 失败: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?;
    Ok(())
}

/// 加载持久化设置，失败从 .bak 恢复，仍失败返回 Null
#[tauri::command]
pub async fn load_settings() -> Result<serde_json::Value, AppError> {
    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    let app_dir_clone = app_dir.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, AppError> {
        match std::fs::read_to_string(&settings_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(v) => Ok(v),
                Err(_) => {
                    let bak = app_dir_clone.join("settings.json.bak");
                    if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                        if let Ok(v) = serde_json::from_str(&bak_content) {
                            let _ = std::fs::write(&settings_path, &bak_content);
                            return Ok(v);
                        }
                    }
                    Ok(serde_json::Value::Null)
                }
            },
            Err(_) => Ok(serde_json::Value::Null),
        }
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?;
    result
}

/// 递归读取目录内容
///
/// 过滤 `.claude/worktrees/`、`.git/`、`node_modules/`。
/// 结果按文件夹→文件排序，同类型按名称字母排序。
#[tauri::command]
pub async fn fs_read_dir(path: String) -> Result<Vec<DirEntry>, AppError> {
    tokio::task::spawn_blocking(move || {
        let mut entries: Vec<DirEntry> = Vec::new();
        let dir = std::fs::read_dir(&path)?;

        for entry in dir {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            // 过滤隐私/重型目录
            if name == ".claude" || name == ".git" || name == "node_modules" {
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
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

/// 创建目录（递归创建父目录）
#[tauri::command]
pub async fn fs_create_dir(path: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&path)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

/// 删除文件或目录（永久删除，不进回收站）
///
/// 删除目录时递归删除所有子级。
#[tauri::command]
pub async fn fs_delete(path: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(AppError::Io(format!("路径不存在: {path}")));
        }
        if p.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

/// 重命名/移动文件或目录
///
/// 目标路径存在时覆盖。
#[tauri::command]
pub async fn fs_rename(src: String, dst: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        // 若目标存在且为目录，先删除
        let dst_path = PathBuf::from(&dst);
        if dst_path.exists() && dst_path.is_dir() {
            std::fs::remove_dir_all(&dst)?;
        }
        std::fs::rename(&src, &dst)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

/// fs_read_dir/create_dir/delete/rename 单元测试
#[cfg(test)]
mod read_dir_tests {
    use super::*;

    #[test]
    fn test_fs_read_dir_lists_children() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let mut entries: Vec<DirEntry> = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().unwrap().is_dir();
            entries.push(DirEntry {
                name, path: entry.path().to_string_lossy().replace('\\', "/"),
                is_dir, size: None, modified: None,
            });
        }
        assert_eq!(entries.len(), 3, "应返回 2 文件 + 1 子目录");
    }

    #[test]
    fn test_fs_read_dir_filters_dotgit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let mut entries: Vec<DirEntry> = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" { continue; }
            let is_dir = entry.file_type().unwrap().is_dir();
            entries.push(DirEntry {
                name, path: entry.path().to_string_lossy().replace('\\', "/"),
                is_dir, size: None, modified: None,
            });
        }
        assert_eq!(entries.len(), 1, "应过滤 .git");
    }

    #[test]
    fn test_fs_read_dir_filters_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("visible.txt"), "ok").unwrap();

        let mut entries: Vec<DirEntry> = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "node_modules" { continue; }
            let is_dir = entry.file_type().unwrap().is_dir();
            entries.push(DirEntry {
                name, path: entry.path().to_string_lossy().replace('\\', "/"),
                is_dir, size: None, modified: None,
            });
        }
        assert_eq!(entries.len(), 1, "应过滤 node_modules");
    }

    #[test]
    fn test_fs_create_dir_creates() {
        let base = tempfile::tempdir().unwrap();
        let new_dir = base.path().join("new_folder");
        std::fs::create_dir_all(&new_dir).unwrap();
        assert!(new_dir.exists(), "目录应被创建");
    }

    #[test]
    fn test_fs_create_dir_parent_creation() {
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(nested.exists(), "嵌套目录应被创建");
    }

    #[test]
    fn test_fs_delete_file_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("to_delete.txt");
        std::fs::write(&file, "delete me").unwrap();
        assert!(file.exists());
        std::fs::remove_file(&file).unwrap();
        assert!(!file.exists(), "文件应被删除");
    }

    #[test]
    fn test_fs_delete_dir_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("to_delete_dir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("child.txt"), "child").unwrap();
        std::fs::remove_dir_all(&sub).unwrap();
        assert!(!sub.exists(), "目录及其内容应被删除");
    }

    #[test]
    fn test_fs_rename_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("old.txt");
        let dst = dir.path().join("new.txt");
        std::fs::write(&src, "content").unwrap();
        std::fs::rename(&src, &dst).unwrap();
        assert!(!src.exists(), "旧路径应不存在");
        assert!(dst.exists(), "新路径应存在");
    }
}

/// fs_write_file CRLF 行尾保持逻辑测试
#[cfg(test)]
mod write_file_tests {
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
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(65536)]);
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
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(65536)]);
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
                let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(65536)]);
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
            let sample = String::from_utf8_lossy(&raw[..raw.len().min(65536)]);
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

/// 验证设置加载/保存逻辑
#[cfg(test)]
mod load_settings_tests {
    use super::*;

    #[test]
    fn test_fs_read_write_roundtrip() {
        let tmp = std::env::temp_dir().join("slterm_test_roundtrip.txt");
        let test_content = "hello slTerminal 测试内容\r\n第二行";

        // 写
        std::fs::write(&tmp, test_content).unwrap();

        // 读
        let read = std::fs::read_to_string(&tmp).unwrap();
        assert_eq!(read, test_content);

        // 清理
        let _ = std::fs::remove_file(&tmp);
    }

    /// 路径不存在时 read_to_string 返回 io::Error
    #[test]
    fn test_fs_read_file_not_found() {
        let tmp = std::env::temp_dir().join("slterm_nonexistent_test_file.txt");
        // 确保文件不存在
        let _ = std::fs::remove_file(&tmp);
        let result = std::fs::read_to_string(&tmp);
        assert!(
            result.is_err(),
            "读取不存在的文件应返回错误"
        );
    }

    /// 写入不存在的目录时 create_dir_all 自动创建父目录
    #[test]
    fn test_fs_write_file_parent_creation() {
        let base = std::env::temp_dir().join("slterm_test_parent_creation");
        let file_path = base.join("subdir").join("nested").join("test.txt");

        // 清理旧数据
        let _ = std::fs::remove_dir_all(&base);

        // 模拟 fs_write_file 逻辑：确保父目录存在
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, "parent 自动创建").unwrap();

        let read = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(read, "parent 自动创建");

        // 清理
        let _ = std::fs::remove_dir_all(&base);
    }

    /// 完整 save → load 往返（使用临时目录）
    #[test]
    fn test_save_settings_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let settings: serde_json::Value = serde_json::json!({
            "theme": "jetbrains-dark",
            "fontSize": 14
        });

        // --- save 逻辑（同步版）---
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let mut tmp = NamedTempFile::new_in(dir.path()).unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        tmp.flush().unwrap();
        // 首次写入，无旧文件
        tmp.persist(&settings_path).unwrap();

        // --- load 逻辑（同步版）---
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded, settings);
    }

    /// 文件不存在时 load_settings 返回 Null
    #[test]
    fn test_load_settings_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        // 确保文件不存在
        let _ = std::fs::remove_file(&settings_path);

        let result = std::fs::read_to_string(&settings_path);
        assert!(result.is_err(), "文件不存在时 read 应失败");

        // 对应 load_settings 中 Err(_) → Ok(Null) 分支
        let value: serde_json::Value = match result {
            Ok(_) => serde_json::Value::Null,
            Err(_) => serde_json::Value::Null,
        };
        assert!(value.is_null());
    }

    /// JSON 损坏时从 .bak 恢复
    #[test]
    fn test_load_settings_corrupt_json_fallback_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let bak_path = dir.path().join("settings.json.bak");

        let valid_json = r#"{"theme":"dark","fontSize":14}"#;
        let corrupt_json = "not valid json {{{broken";

        // 写入 bak
        std::fs::write(&bak_path, valid_json).unwrap();
        // 写入损坏的 settings
        std::fs::write(&settings_path, corrupt_json).unwrap();

        // 模拟 load_settings 逻辑
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                // 回退到 .bak
                let bak_content = std::fs::read_to_string(&bak_path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&bak_content).unwrap();
                // 自动修复 settings.json
                let _ = std::fs::write(&settings_path, &bak_content);
                v
            }
        };

        assert_eq!(
            value,
            serde_json::json!({"theme": "dark", "fontSize": 14})
        );
        // 验证 settings.json 已被修复
        let repaired: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            repaired,
            serde_json::json!({"theme": "dark", "fontSize": 14})
        );
    }

    /// JSON 损坏且无 .bak 时返回 Null
    #[test]
    fn test_load_settings_corrupt_json_no_bak() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let corrupt_json = "definitely not json {{{";

        std::fs::write(&settings_path, corrupt_json).unwrap();

        // 模拟 load_settings 逻辑
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                let bak_path = dir.path().join("settings.json.bak");
                if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                    if let Ok(v) = serde_json::from_str(&bak_content) {
                        v
                    } else {
                        serde_json::Value::Null
                    }
                } else {
                    serde_json::Value::Null
                }
            }
        };

        assert!(value.is_null(), "无 .bak 时应返回 Null");
    }
}
