/// 文件系统模块 — 文件读/写命令
///
/// 阻塞 I/O 用 spawn_blocking 包装，不阻塞 tokio runtime。
use crate::error::AppError;
use std::path::PathBuf;

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
