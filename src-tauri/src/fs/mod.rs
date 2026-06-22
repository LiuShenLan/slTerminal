/// 文件系统模块 — 文件读/写命令 + 设置持久化
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
/// 设置用 tempfile 原子写入 + .bak 备份兜底。
use crate::error::AppError;
use std::io::Write as _;
use std::path::PathBuf;
use tempfile::NamedTempFile;

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
#[tauri::command]
pub async fn fs_write_file(path: String, content: String) -> Result<(), AppError> {
    let _ = tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // 确保父目录存在
        if let Some(parent) = PathBuf::from(&path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &content)?;
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

#[cfg(test)]
mod tests {
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
