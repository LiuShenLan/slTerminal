use serde::Serialize;
use std::path::Path;
use thiserror::Error;

/// 应用统一错误类型，所有 Tauri 命令返回 Result<_, AppError>
///
/// ## 消息语义化约定（BE-15）
/// - `message` 是用户可见消息：业务语义 + 必要上下文（涉及文件操作时含路径，BE-13）；
///   原始技术细节（底层 io 错误文本等）在调用点进 tracing 日志，不暴露给前端。
/// - `IoKind`/`Notify` 等宽变体继续承载异构错误——**不拆变体体系**，
///   异构性经「消息语义化 + tracing 日志」收敛（语义见各变体注释）。
/// - 需要「业务语义 + 路径」的 io 错误用 [`io_error`]，勿在命令内直接 `?` 走 From。
#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppError {
    /// 通用 IO 错误——承载异构底层错误（无路径上下文时的兜底转换；带路径场景用 [`io_error`]）
    #[error("IO 错误({kind}): {message}")]
    IoKind { kind: String, message: String },

    #[error("PTY 错误: {0}")]
    Pty(String),

    #[error("Git 错误: {0}")]
    Git(String),

    #[error("序列化错误: {0}")]
    Serde(String),

    /// 配置 JSON 解析失败（配置文件损坏场景，如 hooks 配置读取）
    #[error("配置解析失败: {0}")]
    ConfigParse(String),

    #[error("未知错误: {0}")]
    Unknown(String),

    #[error("会话未找到: {0}")]
    SessionNotFound(String),

    #[error("异步任务异常: {0}")]
    TaskJoin(String),

    /// 文件监听错误——承载异构错误，消息 = 业务语义，技术细节在调用点进 tracing
    #[error("文件监听错误: {0}")]
    Notify(String),

    #[error("参数校验错误: {0}")]
    Validation(String),

    #[error("路径不在允许范围内: {0}")]
    PathNotAllowed(String),
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

/// 把底层 io 错误转换为用户可见的 AppError::IoKind（BE-13/BE-15 消息语义化约定）：
///
/// - `message` = 「业务动作失败 + 路径」——路径上下文在调用点注入（BE-13），用户可读；
/// - 原始 io 错误文本（ErrorKind + 系统描述）只进 tracing 日志，不暴露给前端（BE-15）。
///
/// 仅用于「用户主动触发的文件操作」等需要把失败告知用户的场景；
/// 无路径上下文或纯内部错误保留 `From<std::io::Error>`（BE-13 不改动该 From）。
pub(crate) fn io_error(action: &str, path: &Path, e: std::io::Error) -> AppError {
    tracing::warn!(target: "app_error", error = %e, path = %path.display(), "{action}失败");
    AppError::IoKind {
        kind: format!("{:?}", e.kind()),
        message: format!("{action}失败: {}", path.display()),
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn app_error_serialization() {
        let err = AppError::IoKind {
            kind: "NotFound".into(),
            message: "测试错误".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("测试错误"));
        assert!(json.contains("ioKind"));
    }

    /// 确保所有变体的 Display 不 panic 且输出非空
    #[test]
    fn all_error_variants_display() {
        let errors: Vec<AppError> = vec![
            AppError::IoKind {
                kind: "Other".into(),
                message: "磁盘已满".to_string(),
            },
            AppError::Pty("PTY 进程崩溃".to_string()),
            AppError::Git("rebase 冲突".to_string()),
            AppError::Serde("JSON 键缺失".to_string()),
            AppError::ConfigParse("配置文件 JSON 损坏".to_string()),
            AppError::Unknown("未分类错误".to_string()),
            AppError::SessionNotFound("uuid-12345".to_string()),
            AppError::TaskJoin("join error".to_string()),
            AppError::Notify("watcher 启动失败".to_string()),
            AppError::Validation("非法 layer".to_string()),
            AppError::PathNotAllowed("C:\\outside\\project".to_string()),
        ];
        for err in &errors {
            let display = format!("{err}");
            assert!(!display.is_empty(), "Display 输出不应为空: {err:?}");
        }
    }

    /// 验证 From<std::io::Error> 转换为 IoKind 变体并保留 ErrorKind
    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let app_err: AppError = io_err.into();
        match app_err {
            AppError::IoKind { kind, message } => {
                assert!(
                    kind.contains("NotFound"),
                    "kind 应保留 ErrorKind::NotFound，实际: {kind}"
                );
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
    fn session_not_found_serialization() {
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

    /// ConfigParse 序列化形态——与前端 parseAppError（FE-02）契约对齐：
    /// camelCase 变体名 `configParse` + 消息原文保留（变体总数 10+1=11）
    #[test]
    fn config_parse_serialization() {
        let err = AppError::ConfigParse("配置文件 JSON 损坏".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            v["configParse"], "配置文件 JSON 损坏",
            "camelCase 序列化应含 configParse 键且消息原样保留，实际: {json}"
        );
    }

    /// io_error 辅助（BE-13/BE-15）：消息 = 业务动作 + 路径；kind 保留 ErrorKind；
    /// 原始 io 错误文本只进 tracing，不进用户可见消息
    #[test]
    fn io_error_helper_injects_path_context() {
        let path = Path::new("C:\\data\\settings.json");
        let io_err =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "拒绝访问（技术细节）");
        let err = io_error("保存设置", path, io_err);
        match err {
            AppError::IoKind { kind, message } => {
                assert_eq!(kind, "PermissionDenied", "kind 应保留 ErrorKind");
                assert!(
                    message.contains("保存设置失败") && message.contains("settings.json"),
                    "消息应含业务语义 + 路径，实际: {message}"
                );
                assert!(
                    !message.contains("拒绝访问"),
                    "技术细节不应进用户可见消息，实际: {message}"
                );
            }
            other => panic!("io_error 应返回 AppError::IoKind，实际: {other:?}"),
        }
    }

    // ── SPE-03: 三个 From 实现（变体 + 消息契约） ──

    /// serde_json::Error → AppError::Serde 转换（变体 + 消息原样保留）
    #[test]
    fn from_serde_json_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let expected = serde_err.to_string();
        let app_err: AppError = serde_err.into();
        match app_err {
            AppError::Serde(msg) => {
                assert_eq!(msg, expected, "消息应原样保留 serde_json 错误文本");
                assert!(
                    msg.contains("expected"),
                    "serde 错误消息应含具体原因，实际: {msg}"
                );
            }
            other => panic!("serde_json::Error 应转为 AppError::Serde，实际: {other:?}"),
        }
    }

    /// git2::Error → AppError::Git 转换（变体 + 消息原样保留）
    #[test]
    fn from_git2_error() {
        let git_err = git2::Error::from_str("模拟 git 错误");
        let expected = git_err.to_string();
        let app_err: AppError = git_err.into();
        match app_err {
            AppError::Git(msg) => {
                assert_eq!(msg, expected, "消息应原样保留 git2 错误文本");
                assert!(
                    msg.contains("模拟 git 错误"),
                    "git 错误消息应保留原文，实际: {msg}"
                );
            }
            other => panic!("git2::Error 应转为 AppError::Git，实际: {other:?}"),
        }
    }

    /// tokio::task::JoinError → AppError::TaskJoin 转换（变体 + 消息原样保留）
    #[test]
    fn from_join_error() {
        // 用 spawn_blocking 内 panic 构造真实 JoinError（panic 被 JoinError 捕获，不扩散到测试线程）
        // 注意：spawn_blocking 必须先在 runtime 上下文内执行——Rust 求值顺序是
        // 先求值 block_on 的参数表达式，若参数直接写 spawn_blocking(...)，
        // 则在无 runtime 的测试线程上调用会 panic 'there is no reactor running'。
        // 故把 spawn_blocking 放入 block_on 的 async 块内，await 拿到 JoinError。
        let rt = tokio::runtime::Runtime::new().unwrap();
        let join_err = rt.block_on(async {
            tokio::task::spawn_blocking(|| panic!("模拟阻塞任务 panic"))
                .await
                .unwrap_err()
        });
        let expected = join_err.to_string();
        let app_err: AppError = join_err.into();
        match app_err {
            AppError::TaskJoin(msg) => {
                assert_eq!(msg, expected, "消息应原样保留 JoinError 文本");
                assert!(!msg.is_empty());
            }
            other => panic!("JoinError 应转为 AppError::TaskJoin，实际: {other:?}"),
        }
    }
}
