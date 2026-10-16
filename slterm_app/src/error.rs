//! 通用错误类型(01 篇类型锚点:`slterm_app::error`,全 crate 共享)。
//!
//! M3 最小面:io / pty / gpui 三包。新增变体先回 01 篇锚点表登记归属,
//! 禁领域各自另立 envelope。

use std::fmt;

/// 应用级错误 envelope。
#[derive(Debug)]
pub enum AppError {
    /// 平台/文件 I/O 失败;启动参数非法也走此包(`ErrorKind::InvalidInput`)。
    Io(std::io::Error),
    /// 终端会话(PTY)启动失败。M3 载体即 io,独立变体让调用点语义可区分
    /// (spawn 失败 vs 普通文件读写失败的报告路径不同)。
    Pty(std::io::Error),
    /// GPUI 开窗/消息循环失败。
    Gpui(anyhow::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Pty(error) => write!(f, "pty error: {error}"),
            Self::Gpui(error) => write!(f, "gpui error: {error}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::Pty(error) => Some(error),
            Self::Gpui(error) => Some(error.as_ref()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        Self::Gpui(error)
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn display_names_the_package_and_carries_the_cause() {
        let io = AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "bad flag",
        ));
        assert_eq!(io.to_string(), "io error: bad flag");
        let pty = AppError::Pty(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no shell",
        ));
        assert_eq!(pty.to_string(), "pty error: no shell");
        let gpui = AppError::Gpui(anyhow::anyhow!("no display"));
        assert_eq!(gpui.to_string(), "gpui error: no display");
        for error in [io, pty, gpui] {
            assert!(std::error::Error::source(&error).is_some());
        }
    }

    #[test]
    fn io_errors_convert_into_the_io_package() {
        let error: AppError = std::io::Error::other("x").into();
        assert!(matches!(error, AppError::Io(_)));
    }
}
