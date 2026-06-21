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

    #[error("工作树错误: {0}")]
    Worktree(String),

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
}
