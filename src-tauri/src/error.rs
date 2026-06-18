use serde::Serialize;
use thiserror::Error;

/// 应用统一错误类型，所有 Tauri 命令返回 Result<_, AppError>
// Phase 0 占位变体，后续 phase 构造；保留 -D warnings 全局生效
#[allow(dead_code)]
#[derive(Debug, Error, Serialize)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serialization() {
        let err = AppError::Io("测试错误".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("测试错误"));
        assert!(json.contains("Io"));
    }
}
