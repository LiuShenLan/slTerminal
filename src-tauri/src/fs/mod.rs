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
}
