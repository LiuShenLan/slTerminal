use serde::Serialize;
use thiserror::Error;

/// 应用统一错误类型，所有 Tauri 命令返回 Result<_, AppError>
#[allow(dead_code)]
#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppError {
    #[error("IO 错误: {0}")]
    Io(String),

    #[error("PTY 错误: {0}")]
    Pty(String),

    #[error("Git 错误: {0}")]
    Git(String),

    #[error("序列化错误: {0}")]
    Serde(String),

    #[error("未知错误: {0}")]
    Unknown(String),

    #[error("会话未找到: {0}")]
    SessionNotFound(String),

    #[error("Channel 发送失败: {0}")]
    ChannelSend(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

/// portable-pty 使用 anyhow::Error，通过 Display 转换
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Pty(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serde(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serialization() {
        let err = AppError::Io("测试错误".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("测试错误"));
        assert!(json.contains("io"));
    }

    /// 确保所有变体的 Display 不 panic 且输出非空
    #[test]
    fn test_all_error_variants_display() {
        let errors: Vec<AppError> = vec![
            AppError::Io("磁盘已满".to_string()),
            AppError::Pty("PTY 进程崩溃".to_string()),
            AppError::Git("rebase 冲突".to_string()),
            AppError::Serde("JSON 键缺失".to_string()),
            AppError::Unknown("未分类错误".to_string()),
            AppError::SessionNotFound("uuid-12345".to_string()),
            AppError::ChannelSend("channel 已关闭".to_string()),
        ];
        for err in &errors {
            let display = format!("{err}");
            assert!(!display.is_empty(), "Display 输出不应为空: {err:?}");
        }
    }

    /// 验证 From<std::io::Error> 转换为 Io 变体
    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let app_err: AppError = io_err.into();
        match app_err {
            AppError::Io(msg) => assert!(
                msg.contains("文件不存在"),
                "消息应包含原始错误信息，实际: {msg}"
            ),
            other => panic!("std::io::Error 应转为 AppError::Io，实际: {other:?}"),
        }
    }

    /// 验证 From<anyhow::Error> 转换为 Pty 变体
    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("PTY spawn 超时");
        let app_err: AppError = anyhow_err.into();
        match app_err {
            AppError::Pty(msg) => assert!(
                msg.contains("PTY spawn 超时"),
                "消息应包含原始错误信息，实际: {msg}"
            ),
            other => panic!("anyhow::Error 应转为 AppError::Pty，实际: {other:?}"),
        }
    }

    /// 验证 SessionNotFound 的 JSON 序列化包含 sessionId（camelCase）
    #[test]
    fn test_session_not_found_serialization() {
        let err = AppError::SessionNotFound("test-session-456".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(
            json.contains("test-session-456"),
            "JSON 应包含 sessionId 值，实际: {json}"
        );
        assert!(
            json.contains("sessionNotFound"),
            "camelCase 序列化应包含 sessionNotFound 键，实际: {json}"
        );
    }
}
