use serde::Serialize;
use thiserror::Error;

/// 应用统一错误类型，所有 Tauri 命令返回 Result<_, AppError>
#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppError {
    #[error("IO 错误({kind}): {message}")]
    IoKind { kind: String, message: String },

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

    #[error("异步任务异常: {0}")]
    TaskJoin(String),

    #[error("文件监听错误: {0}")]
    Notify(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoKind {
            kind: format!("{:?}", e.kind()),
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serde(e.to_string())
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        AppError::Git(e.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        AppError::TaskJoin(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serialization() {
        let err = AppError::IoKind { kind: "NotFound".into(), message: "测试错误".into() };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("测试错误"));
        assert!(json.contains("ioKind"));
    }

    /// 确保所有变体的 Display 不 panic 且输出非空
    #[test]
    fn test_all_error_variants_display() {
        let errors: Vec<AppError> = vec![
            AppError::IoKind { kind: "Other".into(), message: "磁盘已满".to_string() },
            AppError::Pty("PTY 进程崩溃".to_string()),
            AppError::Git("rebase 冲突".to_string()),
            AppError::Serde("JSON 键缺失".to_string()),
            AppError::Unknown("未分类错误".to_string()),
            AppError::SessionNotFound("uuid-12345".to_string()),
            AppError::TaskJoin("join error".to_string()),
            AppError::Notify("watcher 启动失败".to_string()),
        ];
        for err in &errors {
            let display = format!("{err}");
            assert!(!display.is_empty(), "Display 输出不应为空: {err:?}");
        }
    }

    /// 验证 From<std::io::Error> 转换为 IoKind 变体并保留 ErrorKind
    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let app_err: AppError = io_err.into();
        match app_err {
            AppError::IoKind { kind, message } => {
                assert!(kind.contains("NotFound"), "kind 应保留 ErrorKind::NotFound，实际: {kind}");
                assert!(
                    message.contains("文件不存在"),
                    "消息应包含原始错误信息，实际: {message}"
                );
            }
            other => panic!("std::io::Error 应转为 AppError::IoKind，实际: {other:?}"),
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
